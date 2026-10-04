//! `COM_FONT.BTX` short renderer의 runtime 관측 지점.
//!
//! 9개 EXE는 같은 역할의 renderer를 각각 따로 가진다. 이 모듈은 기존
//! BTX cell lookup 분석에서 short renderer를 고른 뒤, cell lookup과 네
//! clipping 분기를 통과한 primitive 할당 호출의 주소·live register를
//! 지원 원본 명령열에서 다시 검증한다. 주소가 비슷하다는 이유로 한 EXE의
//! 관측식을 다른 EXE에 복제하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::isa::{FlowKind, Instruction, InstructionKind as K, Reg};

use super::btx_cell::{BtxCellAnalysis, BtxCellConsumer, BtxRendererFamily};
use super::linear;

const CELL_RECORD_BYTES: i16 = 12;
const PRIMITIVE_SEARCH_WORDS: usize = 96;
const CLIPPING_BRANCHES: usize = 4;

/// `COM_FONT.BTX`를 가진 EXE와 원본 GRP의 대응.
pub const BOX_COMMON_FONT_DRAW_SOURCES: [(&str, &str); 9] = [
    ("GOCHA1.EXE", "/GOCHA1/TUU_GRP.VDD;1"),
    ("GOCHA2.EXE", "/GOCHA2/GOTY_GRP.VDD;1"),
    ("HORI2.EXE", "/HORI2/HORI2GRP.VDD;1"),
    ("HORI4.EXE", "/HORI4/HORI4GRP.VDD;1"),
    ("KOUSIKI.EXE", "/KOUSIKI/KOU_GRP.VDD;1"),
    ("MINPUYO.EXE", "/MINPUYO/MIN_GRP.VDD;1"),
    ("PUYO1.EXE", "/PUYO1/PP1_GRP.VDD;1"),
    ("PUYO2.EXE", "/PUYO2/PP2_GRP.VDD;1"),
    ("QUEST.EXE", "/QUEST/RPG_GRP.VDD;1"),
];

/// 한 EXE에서 같은 object·cell의 lookup과 primitive 진입을 묶는 관측 사양.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontDrawSite {
    pub exe: &'static str,
    pub archive: &'static str,
    pub function_runtime: u32,
    pub renderer_source_bytes: usize,
    pub renderer_source_sha256: [u8; 32],
    pub lookup_runtime: u32,
    pub primitive_runtime: u32,
    pub primitive_target_runtime: u32,
    pub object_register: Reg,
    pub descriptor_register: Reg,
    pub cell_index_register: Reg,
    pub cell_record_register: Reg,
    pub cell_fields_register: Reg,
    pub resource_field_offset: i16,
    pub cell_field_offset: i16,
}

/// 9개 EXE 관측 지점의 검증 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontDrawSiteAnalysis {
    pub sites: Vec<CommonFontDrawSite>,
    pub analysis_sha256: [u8; 32],
}

/// 관측 지점 source 검증 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonFontDrawSiteError {
    DuplicateExecutable(&'static str),
    TextLengthMismatch {
        exe: &'static str,
        expected: usize,
        actual: usize,
    },
    UnalignedText(&'static str),
    MissingExecutable(&'static str),
    MissingShortRenderer(&'static str),
    DuplicateShortRenderer(&'static str),
    AddressOutsideText {
        exe: &'static str,
        runtime: u32,
    },
    InstructionMismatch {
        exe: &'static str,
        runtime: u32,
        role: &'static str,
    },
    PrimitiveCallMissing(&'static str),
    ClippingBranchMismatch {
        exe: &'static str,
        expected: usize,
        actual: usize,
    },
    RegisterClobbered {
        exe: &'static str,
        role: &'static str,
        register: Reg,
        runtime: u32,
    },
    AnalysisDigestMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

impl fmt::Display for CommonFontDrawSiteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateExecutable(exe) => write!(f, "COM_FONT draw EXE 입력 중복: {exe}"),
            Self::TextLengthMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe} COM_FONT draw text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText(exe) => {
                write!(f, "{exe} COM_FONT draw text가 4바이트 정렬이 아님")
            }
            Self::MissingExecutable(exe) => write!(f, "COM_FONT draw EXE 입력 누락: {exe}"),
            Self::MissingShortRenderer(exe) => {
                write!(f, "{exe} COM_FONT short renderer lookup이 없음")
            }
            Self::DuplicateShortRenderer(exe) => {
                write!(f, "{exe} COM_FONT short renderer lookup이 둘 이상임")
            }
            Self::AddressOutsideText { exe, runtime } => {
                write!(f, "{exe} COM_FONT draw 주소가 text 밖임: 0x{runtime:08X}")
            }
            Self::InstructionMismatch { exe, runtime, role } => {
                write!(f, "{exe} COM_FONT draw {role} 명령 불일치: 0x{runtime:08X}")
            }
            Self::PrimitiveCallMissing(exe) => {
                write!(f, "{exe} COM_FONT primitive 할당 호출을 찾지 못함")
            }
            Self::ClippingBranchMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe} COM_FONT clipping 분기 불일치: expected={expected}, actual={actual}"
            ),
            Self::RegisterClobbered {
                exe,
                role,
                register,
                runtime,
            } => write!(
                f,
                "{exe} COM_FONT draw {role} register {register}가 관측 전 덮어써짐: \
                 0x{runtime:08X}"
            ),
            Self::AnalysisDigestMismatch { expected, actual } => write!(
                f,
                "COM_FONT draw site digest 불일치: expected={}, actual={}",
                hex_digest(expected),
                hex_digest(actual)
            ),
        }
    }
}

impl Error for CommonFontDrawSiteError {}

/// 지원 원본의 9개 관측 지점 digest. 원본 source와 register 역할을 함께 고정한다.
pub const BOX_COMMON_FONT_DRAW_SITE_SHA256: [u8; 32] = [
    0x5A, 0x1D, 0x8C, 0x38, 0x27, 0x69, 0xC9, 0x56, 0xF1, 0x11, 0x05, 0x4B, 0x63, 0x64, 0x68, 0x9A,
    0x66, 0x74, 0xB8, 0x60, 0x42, 0x53, 0x8B, 0xC3, 0xB1, 0x5B, 0x84, 0x93, 0xEF, 0x7B, 0xDB, 0x84,
];

/// 입력된 EXE 중 `COM_FONT.BTX`를 가진 short renderer 관측 지점을 분석한다.
pub fn analyze_common_font_draw_sites<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    cell_analysis: &BtxCellAnalysis,
) -> Result<CommonFontDrawSiteAnalysis, CommonFontDrawSiteError> {
    let sources = BOX_COMMON_FONT_DRAW_SOURCES
        .iter()
        .copied()
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut sites = Vec::new();

    for (map, text) in executables {
        let Some(&archive) = sources.get(map.name) else {
            continue;
        };
        if !seen.insert(map.name) {
            return Err(CommonFontDrawSiteError::DuplicateExecutable(map.name));
        }
        if text.len() != map.text_len as usize {
            return Err(CommonFontDrawSiteError::TextLengthMismatch {
                exe: map.name,
                expected: map.text_len as usize,
                actual: text.len(),
            });
        }
        let instructions =
            linear::sweep(map, text).ok_or(CommonFontDrawSiteError::UnalignedText(map.name))?;
        let mut consumers = cell_analysis.consumers.iter().filter(|consumer| {
            consumer.exe == map.name
                && consumer.renderer_family == Some(BtxRendererFamily::ShortRenderer)
        });
        let consumer = consumers
            .next()
            .ok_or(CommonFontDrawSiteError::MissingShortRenderer(map.name))?;
        if consumers.next().is_some() {
            return Err(CommonFontDrawSiteError::DuplicateShortRenderer(map.name));
        }
        sites.push(analyze_site(map, text, &instructions, archive, consumer)?);
    }

    sites.sort_by_key(|site| site.exe);
    let analysis_sha256 = digest_sites(&sites);
    Ok(CommonFontDrawSiteAnalysis {
        sites,
        analysis_sha256,
    })
}

/// 지원 원본의 9개 EXE를 빠짐없이 요구하고 검토한 결과와 대조한다.
pub fn verify_box_common_font_draw_sites<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    cell_analysis: &BtxCellAnalysis,
) -> Result<CommonFontDrawSiteAnalysis, CommonFontDrawSiteError> {
    let analysis = analyze_common_font_draw_sites(executables, cell_analysis)?;
    for (exe, _) in BOX_COMMON_FONT_DRAW_SOURCES {
        if !analysis.sites.iter().any(|site| site.exe == exe) {
            return Err(CommonFontDrawSiteError::MissingExecutable(exe));
        }
    }
    if analysis.analysis_sha256 != BOX_COMMON_FONT_DRAW_SITE_SHA256 {
        return Err(CommonFontDrawSiteError::AnalysisDigestMismatch {
            expected: BOX_COMMON_FONT_DRAW_SITE_SHA256,
            actual: analysis.analysis_sha256,
        });
    }
    Ok(analysis)
}

/// runtime capture가 바로 사용할 수 있는 결정적 TSV를 만든다.
pub fn render_common_font_draw_sites_tsv(analysis: &CommonFontDrawSiteAnalysis) -> String {
    let mut output = String::from(
        "exe\tarchive\tfunction_runtime\trenderer_source_bytes\t\
         renderer_source_sha256\tlookup_runtime\tprimitive_runtime\t\
         primitive_target_runtime\tobject_register\tdescriptor_register\t\
         cell_index_register\tcell_record_register\tcell_fields_register\t\
         resource_field_offset\tcell_field_offset\n",
    );
    for site in &analysis.sites {
        writeln!(
            output,
            "{}\t{}\t0x{:08X}\t{}\t{}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t0x{:04X}",
            site.exe,
            site.archive,
            site.function_runtime,
            site.renderer_source_bytes,
            hex_digest(&site.renderer_source_sha256),
            site.lookup_runtime,
            site.primitive_runtime,
            site.primitive_target_runtime,
            site.object_register,
            site.descriptor_register,
            site.cell_index_register,
            site.cell_record_register,
            site.cell_fields_register,
            site.resource_field_offset as u16,
            site.cell_field_offset as u16,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn analyze_site(
    map: &'static ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    archive: &'static str,
    consumer: &BtxCellConsumer,
) -> Result<CommonFontDrawSite, CommonFontDrawSiteError> {
    let function_runtime = consumer
        .function_candidate
        .ok_or(CommonFontDrawSiteError::MissingShortRenderer(map.name))?;
    let function_index = instruction_index(map, instructions, function_runtime)?;
    let cell_load_index = instruction_index(map, instructions, consumer.cell_load)?;
    let lookup_index = instruction_index(map, instructions, consumer.cell_table_load)?;
    let cell_entry_index = instruction_index(map, instructions, consumer.cell_entry)?;

    match instructions[function_index + 2].kind {
        K::Addu {
            rd,
            rs: Reg::A0,
            rt: Reg::ZERO,
        } if rd == consumer.object_register => {}
        _ => {
            return Err(CommonFontDrawSiteError::InstructionMismatch {
                exe: map.name,
                runtime: function_runtime + 8,
                role: "object 보존",
            });
        }
    }
    let cell_index_register = match instructions[cell_load_index].kind {
        K::Lhu { rt, base, offset }
            if base == consumer.object_register && offset == consumer.cell_field_offset =>
        {
            rt
        }
        _ => {
            return Err(CommonFontDrawSiteError::InstructionMismatch {
                exe: map.name,
                runtime: consumer.cell_load,
                role: "cell index load",
            });
        }
    };
    let (cell_table_register, descriptor_register) = match instructions[lookup_index].kind {
        K::Lw {
            rt,
            base,
            offset: 0x001C,
        } => (rt, base),
        _ => {
            return Err(CommonFontDrawSiteError::InstructionMismatch {
                exe: map.name,
                runtime: consumer.cell_table_load,
                role: "cell table lookup",
            });
        }
    };
    let cell_record_register = match instructions[cell_entry_index].kind {
        K::Addu { rd, rs, rt } if rs == cell_table_register || rt == cell_table_register => rd,
        _ => {
            return Err(CommonFontDrawSiteError::InstructionMismatch {
                exe: map.name,
                runtime: consumer.cell_entry,
                role: "cell record 계산",
            });
        }
    };
    let fields_index = cell_entry_index + 2;
    let cell_fields_register = match instructions.get(fields_index).map(|item| item.kind) {
        Some(K::Addiu {
            rt,
            rs,
            imm: CELL_RECORD_BYTES,
        }) if rs == cell_record_register => rt,
        _ => {
            return Err(CommonFontDrawSiteError::InstructionMismatch {
                exe: map.name,
                runtime: consumer.cell_entry + 8,
                role: "cell record field pointer",
            });
        }
    };
    if !matches!(
        instructions[cell_entry_index + 1].flow_kind(),
        FlowKind::Branch { .. }
    ) {
        return Err(CommonFontDrawSiteError::InstructionMismatch {
            exe: map.name,
            runtime: consumer.cell_entry + 4,
            role: "record pointer delay-slot branch",
        });
    }

    let primitive_index = instructions
        .iter()
        .enumerate()
        .skip(fields_index + 1)
        .take(PRIMITIVE_SEARCH_WORDS)
        .find_map(|(index, instruction)| matches!(instruction.kind, K::Jal { .. }).then_some(index))
        .ok_or(CommonFontDrawSiteError::PrimitiveCallMissing(map.name))?;
    let primitive_target_runtime = match instructions[primitive_index].kind {
        K::Jal { target } => target,
        _ => unreachable!("JAL만 선택했다"),
    };
    if !matches!(
        instructions.get(primitive_index + 1).map(|item| item.kind),
        Some(K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0
        })
    ) {
        return Err(CommonFontDrawSiteError::InstructionMismatch {
            exe: map.name,
            runtime: instructions[primitive_index].addr + 4,
            role: "primitive call delay slot",
        });
    }

    let clipping_targets = instructions[fields_index + 1..primitive_index]
        .iter()
        .rev()
        .filter_map(|instruction| match instruction.flow_kind() {
            FlowKind::Branch { taken } => Some(taken),
            _ => None,
        })
        .take(CLIPPING_BRANCHES)
        .collect::<Vec<_>>();
    let clipping_target_count = clipping_targets
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len();
    if clipping_targets.len() != CLIPPING_BRANCHES || clipping_target_count != 1 {
        return Err(CommonFontDrawSiteError::ClippingBranchMismatch {
            exe: map.name,
            expected: CLIPPING_BRANCHES,
            actual: clipping_targets.len(),
        });
    }

    require_live_register(
        map.name,
        "object",
        instructions,
        function_index + 2,
        primitive_index,
        consumer.object_register,
    )?;
    require_live_register(
        map.name,
        "descriptor",
        instructions,
        instruction_index(map, instructions, consumer.descriptor_entry)?,
        primitive_index,
        descriptor_register,
    )?;
    require_live_register(
        map.name,
        "cell index",
        instructions,
        cell_load_index,
        lookup_index,
        cell_index_register,
    )?;
    require_live_register(
        map.name,
        "cell record",
        instructions,
        cell_entry_index,
        primitive_index,
        cell_record_register,
    )?;
    require_live_register(
        map.name,
        "cell fields",
        instructions,
        fields_index,
        primitive_index,
        cell_fields_register,
    )?;

    let renderer_source_bytes = instructions[primitive_index]
        .addr
        .checked_add(8)
        .and_then(|end| end.checked_sub(function_runtime))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or(CommonFontDrawSiteError::AddressOutsideText {
            exe: map.name,
            runtime: instructions[primitive_index].addr,
        })?;
    let source_offset =
        map.text_offset(function_runtime)
            .ok_or(CommonFontDrawSiteError::AddressOutsideText {
                exe: map.name,
                runtime: function_runtime,
            })?;
    let renderer_source = text
        .get(source_offset..source_offset + renderer_source_bytes)
        .ok_or(CommonFontDrawSiteError::AddressOutsideText {
            exe: map.name,
            runtime: function_runtime,
        })?;

    Ok(CommonFontDrawSite {
        exe: map.name,
        archive,
        function_runtime,
        renderer_source_bytes,
        renderer_source_sha256: Sha256::digest(renderer_source).into(),
        lookup_runtime: consumer.cell_table_load,
        primitive_runtime: instructions[primitive_index].addr,
        primitive_target_runtime,
        object_register: consumer.object_register,
        descriptor_register,
        cell_index_register,
        cell_record_register,
        cell_fields_register,
        resource_field_offset: consumer.resource_field_offset,
        cell_field_offset: consumer.cell_field_offset,
    })
}

fn instruction_index(
    map: &ExeMap,
    instructions: &[Instruction],
    runtime: u32,
) -> Result<usize, CommonFontDrawSiteError> {
    let offset = map
        .text_offset(runtime)
        .ok_or(CommonFontDrawSiteError::AddressOutsideText {
            exe: map.name,
            runtime,
        })?;
    if !offset.is_multiple_of(4) || offset / 4 >= instructions.len() {
        return Err(CommonFontDrawSiteError::AddressOutsideText {
            exe: map.name,
            runtime,
        });
    }
    Ok(offset / 4)
}

fn require_live_register(
    exe: &'static str,
    role: &'static str,
    instructions: &[Instruction],
    producer: usize,
    consumer: usize,
    register: Reg,
) -> Result<(), CommonFontDrawSiteError> {
    if let Some(instruction) = instructions[producer + 1..consumer]
        .iter()
        .find(|instruction| instruction.dest_reg() == Some(register))
    {
        return Err(CommonFontDrawSiteError::RegisterClobbered {
            exe,
            role,
            register,
            runtime: instruction.addr,
        });
    }
    Ok(())
}

fn digest_sites(sites: &[CommonFontDrawSite]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for site in sites {
        digest.update(site.exe.as_bytes());
        digest.update([0]);
        digest.update(site.archive.as_bytes());
        digest.update([0]);
        digest.update(site.function_runtime.to_le_bytes());
        digest.update(site.renderer_source_bytes.to_le_bytes());
        digest.update(site.renderer_source_sha256);
        digest.update(site.lookup_runtime.to_le_bytes());
        digest.update(site.primitive_runtime.to_le_bytes());
        digest.update(site.primitive_target_runtime.to_le_bytes());
        digest.update([
            site.object_register.0,
            site.descriptor_register.0,
            site.cell_index_register.0,
            site.cell_record_register.0,
            site.cell_fields_register.0,
        ]);
        digest.update(site.resource_field_offset.to_le_bytes());
        digest.update(site.cell_field_offset.to_le_bytes());
    }
    digest.finalize().into()
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "common_font_draw_tests.rs"]
mod common_font_draw_tests;
