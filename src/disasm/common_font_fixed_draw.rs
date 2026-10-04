//! 고정 `COM_FONT.BTX` descriptor를 직접 소비하는 command writer.
//!
//! 공용 renderer는 object의 resource와 cell을 함께 읽지만, 일부 object
//! callback은 resource를 읽지 않고 특정 descriptor의 cell table을 직접
//! 고른다. 이 모듈은 object cell → 12바이트 record → 8바이트 command
//! commit의 typed 명령열을 전 EXE에서 찾고, 해당 고정 resource가 그 EXE의
//! `COM_FONT.BTX`인지 descriptor 원장과 교차한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::{ExeMap, EXE_MAPS};
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::btx_names::BtxNameAnalysis;
use super::linear;

const SOURCE_WORDS: usize = 70;
const SOURCE_BYTES: usize = SOURCE_WORDS * 4;
const DESCRIPTOR_NAME_POINTER_OFFSET: u32 = 4;
const DESCRIPTOR_CELL_TABLE_OFFSET: u32 = 0x1C;
const DESCRIPTOR_STRIDE: u32 = 36;
const BOX_COMMON_FONT_EXECUTABLES: usize = 9;

/// 지원 원본의 9개 고정 COM_FONT direct draw와 source digest.
pub const BOX_COMMON_FONT_FIXED_DRAW_SHA256: [u8; 32] = [
    0x7F, 0x72, 0x42, 0x1E, 0x4C, 0xF4, 0xA1, 0x02, 0xDD, 0x05, 0xF1, 0x92, 0x71, 0x15, 0x24, 0x6D,
    0x35, 0xE6, 0xA8, 0xB6, 0x27, 0x6D, 0x20, 0xB5, 0xD8, 0xEC, 0x09, 0x29, 0xA4, 0xE0, 0x07, 0x98,
];

/// 고정 descriptor를 읽어 8바이트 command를 기록하는 함수 하나.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommonFontFixedDrawSite {
    pub exe: &'static str,
    pub function_runtime: u32,
    pub source_bytes: usize,
    pub source_sha256: [u8; 32],
    pub descriptor_root_pointer_runtime: u32,
    pub descriptor_table_pointer_offset: u32,
    pub common_font_resource_id: u16,
    pub command_buffer_runtime: u32,
    pub command_count_gp_offset: i16,
    pub cell_field_offset: i16,
    pub draw_flags_field_offset: i16,
    pub commit_runtime: u32,
}

/// 전 EXE 검출 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontFixedDrawAnalysis {
    pub scanned_executables: usize,
    pub common_font_executables: usize,
    pub sites: Vec<CommonFontFixedDrawSite>,
    pub analysis_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonFontFixedDrawError {
    EmptyInput,
    DuplicateExecutable(&'static str),
    TextLengthMismatch {
        exe: &'static str,
        expected: usize,
        actual: usize,
    },
    UnalignedText(&'static str),
    BoxExecutableCount {
        expected: usize,
        actual: usize,
    },
    MissingBoxExecutable(&'static str),
    CommonFontExecutableCount {
        expected: usize,
        actual: usize,
    },
    SiteCount {
        expected: usize,
        actual: usize,
    },
    DuplicateCommonFontLink(&'static str),
    DescriptorPointerLayout(&'static str),
    AnalysisDigestMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

impl fmt::Display for CommonFontFixedDrawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "고정 COM_FONT draw를 분석할 EXE가 없음"),
            Self::DuplicateExecutable(exe) => {
                write!(f, "고정 COM_FONT draw 분석 EXE 중복: {exe}")
            }
            Self::TextLengthMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe} 고정 COM_FONT draw text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText(exe) => {
                write!(f, "{exe} 고정 COM_FONT draw text가 4바이트 정렬이 아님")
            }
            Self::BoxExecutableCount { expected, actual } => write!(
                f,
                "고정 COM_FONT draw EXE 분모 불일치: expected={expected}, actual={actual}"
            ),
            Self::MissingBoxExecutable(exe) => {
                write!(f, "고정 COM_FONT draw 분석 입력 누락: {exe}")
            }
            Self::CommonFontExecutableCount { expected, actual } => write!(
                f,
                "고정 COM_FONT draw 자산 EXE 수 불일치: expected={expected}, actual={actual}"
            ),
            Self::SiteCount { expected, actual } => write!(
                f,
                "고정 COM_FONT direct draw 수 불일치: expected={expected}, actual={actual}"
            ),
            Self::DuplicateCommonFontLink(exe) => {
                write!(f, "{exe} COM_FONT.BTX descriptor link가 중복됨")
            }
            Self::DescriptorPointerLayout(exe) => {
                write!(
                    f,
                    "{exe} COM_FONT.BTX descriptor pointer가 36바이트 격자가 아님"
                )
            }
            Self::AnalysisDigestMismatch { expected, actual } => write!(
                f,
                "고정 COM_FONT direct draw digest 불일치: expected={}, actual={}",
                hex_digest(expected),
                hex_digest(actual)
            ),
        }
    }
}

impl Error for CommonFontFixedDrawError {}

/// 입력 EXE의 고정 descriptor direct draw를 찾고 COM_FONT resource만 남긴다.
pub fn audit_common_font_fixed_draws<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    names: &BtxNameAnalysis,
) -> Result<CommonFontFixedDrawAnalysis, CommonFontFixedDrawError> {
    let mut seen = BTreeSet::new();
    let mut sites = Vec::new();
    let mut common_font_executables = 0usize;

    for (map, text) in executables {
        if !seen.insert(map.name) {
            return Err(CommonFontFixedDrawError::DuplicateExecutable(map.name));
        }
        if text.len() != map.text_len as usize {
            return Err(CommonFontFixedDrawError::TextLengthMismatch {
                exe: map.name,
                expected: map.text_len as usize,
                actual: text.len(),
            });
        }
        let Some(common_font_resource_id) = common_font_resource_id(map.name, names)? else {
            continue;
        };
        common_font_executables += 1;
        let instructions =
            linear::sweep(map, text).ok_or(CommonFontFixedDrawError::UnalignedText(map.name))?;
        for (index, _) in instructions.iter().enumerate() {
            let Some(site) = match_fixed_draw(map, text, &instructions, index) else {
                continue;
            };
            if site.common_font_resource_id == common_font_resource_id {
                sites.push(site);
            }
        }
    }

    if seen.is_empty() {
        return Err(CommonFontFixedDrawError::EmptyInput);
    }
    sites.sort();
    sites.dedup();
    let analysis_sha256 = digest_sites(&sites);
    Ok(CommonFontFixedDrawAnalysis {
        scanned_executables: seen.len(),
        common_font_executables,
        sites,
        analysis_sha256,
    })
}

/// 지원 원본의 11개 EXE 분모를 요구한다.
pub fn verify_box_common_font_fixed_draws<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    names: &BtxNameAnalysis,
) -> Result<CommonFontFixedDrawAnalysis, CommonFontFixedDrawError> {
    let inputs = executables.into_iter().collect::<Vec<_>>();
    if inputs.len() != EXE_MAPS.len() {
        return Err(CommonFontFixedDrawError::BoxExecutableCount {
            expected: EXE_MAPS.len(),
            actual: inputs.len(),
        });
    }
    for map in EXE_MAPS {
        if !inputs
            .iter()
            .any(|(input_map, _)| input_map.name == map.name)
        {
            return Err(CommonFontFixedDrawError::MissingBoxExecutable(map.name));
        }
    }
    let analysis = audit_common_font_fixed_draws(inputs, names)?;
    if analysis.common_font_executables != BOX_COMMON_FONT_EXECUTABLES {
        return Err(CommonFontFixedDrawError::CommonFontExecutableCount {
            expected: BOX_COMMON_FONT_EXECUTABLES,
            actual: analysis.common_font_executables,
        });
    }
    if analysis.sites.len() != BOX_COMMON_FONT_EXECUTABLES {
        return Err(CommonFontFixedDrawError::SiteCount {
            expected: BOX_COMMON_FONT_EXECUTABLES,
            actual: analysis.sites.len(),
        });
    }
    if analysis.analysis_sha256 != BOX_COMMON_FONT_FIXED_DRAW_SHA256 {
        return Err(CommonFontFixedDrawError::AnalysisDigestMismatch {
            expected: BOX_COMMON_FONT_FIXED_DRAW_SHA256,
            actual: analysis.analysis_sha256,
        });
    }
    Ok(analysis)
}

/// 검토 가능한 결정적 TSV.
pub fn render_common_font_fixed_draws_tsv(analysis: &CommonFontFixedDrawAnalysis) -> String {
    let mut output = String::from(
        "exe\tfunction_runtime\tsource_bytes\tsource_sha256\tdescriptor_root_pointer_runtime\t\
         descriptor_table_pointer_offset\tcommon_font_resource_id\tcommand_buffer_runtime\t\
         command_count_gp_offset\tcell_field_offset\tdraw_flags_field_offset\tcommit_runtime\n",
    );
    for site in &analysis.sites {
        writeln!(
            output,
            "{}\t0x{:08X}\t{}\t{}\t0x{:08X}\t0x{:04X}\t{}\t0x{:08X}\t0x{:04X}\t0x{:04X}\t0x{:04X}\t0x{:08X}",
            site.exe,
            site.function_runtime,
            site.source_bytes,
            hex_digest(&site.source_sha256),
            site.descriptor_root_pointer_runtime,
            site.descriptor_table_pointer_offset,
            site.common_font_resource_id,
            site.command_buffer_runtime,
            site.command_count_gp_offset,
            site.cell_field_offset,
            site.draw_flags_field_offset,
            site.commit_runtime,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn common_font_resource_id(
    exe: &'static str,
    names: &BtxNameAnalysis,
) -> Result<Option<u16>, CommonFontFixedDrawError> {
    let links = names
        .links
        .iter()
        .filter(|link| link.exe == exe && link.asset_name.eq_ignore_ascii_case("COM_FONT.BTX"))
        .collect::<Vec<_>>();
    let [link] = links.as_slice() else {
        return if links.is_empty() {
            Ok(None)
        } else {
            Err(CommonFontFixedDrawError::DuplicateCommonFontLink(exe))
        };
    };
    let delta = link
        .descriptor_pointer_runtime
        .checked_sub(link.descriptor_runtime_start)
        .and_then(|delta| delta.checked_sub(DESCRIPTOR_NAME_POINTER_OFFSET))
        .filter(|delta| delta.is_multiple_of(DESCRIPTOR_STRIDE))
        .ok_or(CommonFontFixedDrawError::DescriptorPointerLayout(exe))?;
    let resource = u16::try_from(delta / DESCRIPTOR_STRIDE)
        .map_err(|_| CommonFontFixedDrawError::DescriptorPointerLayout(exe))?;
    Ok(Some(resource))
}

fn match_fixed_draw(
    map: &'static ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    start: usize,
) -> Option<CommonFontFixedDrawSite> {
    let source = instructions.get(start..start + SOURCE_WORDS)?;
    if source[0].kind
        != (K::Addu {
            rd: Reg::T0,
            rs: Reg::A0,
            rt: Reg::ZERO,
        })
    {
        return None;
    }
    let K::Lw {
        rt: Reg::A1,
        base: Reg::T0,
        offset: x_field,
    } = source[1].kind
    else {
        return None;
    };
    let K::Lw {
        rt: Reg::V0,
        base: Reg::T0,
        offset: x_delta_field,
    } = source[2].kind
    else {
        return None;
    };
    let K::Lw {
        rt: Reg::A3,
        base: Reg::T0,
        offset: y_field,
    } = source[3].kind
    else {
        return None;
    };
    let K::Lhu {
        rt: Reg::A0,
        base: Reg::T0,
        offset: cell_field,
    } = source[4].kind
    else {
        return None;
    };
    if source[5].kind
        != (K::Addu {
            rd: Reg::A1,
            rs: Reg::A1,
            rt: Reg::V0,
        })
    {
        return None;
    }
    let K::Lui {
        rt: Reg::V0,
        imm: descriptor_root_high,
    } = source[6].kind
    else {
        return None;
    };
    if source[7].kind
        != (K::Sll {
            rd: Reg::V1,
            rt: Reg::A0,
            sa: 1,
        })
    {
        return None;
    }
    let K::Lw {
        rt: Reg::V0,
        base: Reg::V0,
        offset: descriptor_root_low,
    } = source[8].kind
    else {
        return None;
    };
    if source[9].kind
        != (K::Addu {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::A0,
        })
    {
        return None;
    }
    let K::Lw {
        rt: Reg::V0,
        base: Reg::V0,
        offset: descriptor_table_offset,
    } = source[10].kind
    else {
        return None;
    };
    if source[11].kind
        != (K::Sll {
            rd: Reg::V1,
            rt: Reg::V1,
            sa: 2,
        })
        || source[12].kind
            != (K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            })
        || source[14].kind
            != (K::Addiu {
                rt: Reg::A2,
                rs: Reg::V0,
                imm: 12,
            })
    {
        return None;
    }

    if x_delta_field.checked_add(0x22) != Some(cell_field)
        || x_field.checked_add(0x1A) != Some(cell_field)
        || y_field.checked_add(0x16) != Some(cell_field)
    {
        return None;
    }
    let descriptor_table_offset = u32::try_from(descriptor_table_offset).ok()?;
    let resource_delta = descriptor_table_offset.checked_sub(DESCRIPTOR_CELL_TABLE_OFFSET)?;
    if !resource_delta.is_multiple_of(DESCRIPTOR_STRIDE) {
        return None;
    }
    let common_font_resource_id = u16::try_from(resource_delta / DESCRIPTOR_STRIDE).ok()?;
    let draw_flags_field = cell_field.checked_add(8)?;

    let K::Lui {
        rt: Reg::A0,
        imm: command_buffer_high,
    } = source[47].kind
    else {
        return None;
    };
    let K::Lw {
        rt: Reg::V0,
        base: Reg::GP,
        offset: command_count_gp_offset,
    } = source[48].kind
    else {
        return None;
    };
    let K::Addiu {
        rt: Reg::A0,
        rs: Reg::A0,
        imm: command_buffer_low,
    } = source[49].kind
    else {
        return None;
    };
    if source[50].kind
        != (K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 3,
        })
        || source[51].kind
            != (K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            })
        || source[52].kind
            != (K::Sh {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0,
            })
        || source[53].kind
            != (K::Sh {
                rt: Reg::A3,
                base: Reg::V0,
                offset: 2,
            })
        || source[54].kind
            != (K::Lhu {
                rt: Reg::A1,
                base: Reg::T0,
                offset: cell_field,
            })
        || source[55].kind
            != (K::Sb {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: 6,
            })
        || source[56].kind
            != (K::Lw {
                rt: Reg::V1,
                base: Reg::GP,
                offset: command_count_gp_offset,
            })
        || source[58].kind
            != (K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 3,
            })
        || source[59].kind
            != (K::Sh {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 4,
            })
        || source[60].kind
            != (K::Lbu {
                rt: Reg::V0,
                base: Reg::T0,
                offset: draw_flags_field,
            })
        || source[61].kind
            != (K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::A0,
            })
        || source[62].kind
            != (K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 7,
            })
        || source[63].kind
            != (K::Sb {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 7,
            })
        || source[64].kind
            != (K::Lw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: command_count_gp_offset,
            })
        || source[66].kind
            != (K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            })
        || source[67].kind
            != (K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: command_count_gp_offset,
            })
        || source[68].kind != (K::Jr { rs: Reg::RA })
        || source[69].kind
            != (K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            })
    {
        return None;
    }

    let function_runtime = source[0].addr;
    let text_offset = map.text_offset(function_runtime)?;
    let source_bytes = text.get(text_offset..text_offset + SOURCE_BYTES)?;
    let source_sha256 = Sha256::digest(source_bytes).into();
    let descriptor_root_pointer_runtime =
        ((descriptor_root_high as u32) << 16).wrapping_add(descriptor_root_low as i32 as u32);
    let command_buffer_runtime =
        ((command_buffer_high as u32) << 16).wrapping_add(command_buffer_low as i32 as u32);

    Some(CommonFontFixedDrawSite {
        exe: map.name,
        function_runtime,
        source_bytes: SOURCE_BYTES,
        source_sha256,
        descriptor_root_pointer_runtime,
        descriptor_table_pointer_offset: descriptor_table_offset,
        common_font_resource_id,
        command_buffer_runtime,
        command_count_gp_offset,
        cell_field_offset: cell_field,
        draw_flags_field_offset: draw_flags_field,
        commit_runtime: source[67].addr,
    })
}

fn digest_sites(sites: &[CommonFontFixedDrawSite]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for site in sites {
        digest.update(site.exe.as_bytes());
        digest.update(site.function_runtime.to_le_bytes());
        digest.update((site.source_bytes as u64).to_le_bytes());
        digest.update(site.source_sha256);
        digest.update(site.descriptor_root_pointer_runtime.to_le_bytes());
        digest.update(site.descriptor_table_pointer_offset.to_le_bytes());
        digest.update(site.common_font_resource_id.to_le_bytes());
        digest.update(site.command_buffer_runtime.to_le_bytes());
        digest.update(site.command_count_gp_offset.to_le_bytes());
        digest.update(site.cell_field_offset.to_le_bytes());
        digest.update(site.draw_flags_field_offset.to_le_bytes());
        digest.update(site.commit_runtime.to_le_bytes());
    }
    digest.finalize().into()
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "common_font_fixed_draw_tests.rs"]
mod common_font_fixed_draw_tests;
