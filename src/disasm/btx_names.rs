//! 11개 EXE와 자기 GRP VDD의 BTX 이름표 연결.
//!
//! 각 EXE는 별도 빌드이므로 다른 EXE의 이름 주소나 참조를 이식하지 않는다.
//! 이 모듈은 GRP의 물리 BTX 엔트리와 EXE 안의 NUL 종료 ASCII 이름표를
//! 전수 연결하고 정적 주소 참조를 기록한다. 이름표와 정적 참조는 loader
//! 후보 근거이지, 정상 플레이의 asset 소비나 화면 표시 증거가 아니다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::coords::{normalize_kseg, ExeMap};

use super::xref::{find_address_loads, DEFAULT_WINDOW};
use super::{linear, trace};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxNameConsumerSpec {
    pub exe: &'static str,
    pub archive: &'static str,
    pub expected_btx: usize,
}

pub const BOX_BTX_NAME_CONSUMERS: [BtxNameConsumerSpec; 11] = [
    BtxNameConsumerSpec {
        exe: "SLPS_031.14",
        archive: "/FIL_GRP.VDD;1",
        expected_btx: 11,
    },
    BtxNameConsumerSpec {
        exe: "GOCHA1.EXE",
        archive: "/GOCHA1/TUU_GRP.VDD;1",
        expected_btx: 168,
    },
    BtxNameConsumerSpec {
        exe: "GOCHA2.EXE",
        archive: "/GOCHA2/GOTY_GRP.VDD;1",
        expected_btx: 17,
    },
    BtxNameConsumerSpec {
        exe: "HORI2.EXE",
        archive: "/HORI2/HORI2GRP.VDD;1",
        expected_btx: 12,
    },
    BtxNameConsumerSpec {
        exe: "HORI4.EXE",
        archive: "/HORI4/HORI4GRP.VDD;1",
        expected_btx: 12,
    },
    BtxNameConsumerSpec {
        exe: "KOUSIKI.EXE",
        archive: "/KOUSIKI/KOU_GRP.VDD;1",
        expected_btx: 10,
    },
    BtxNameConsumerSpec {
        exe: "MINPUYO.EXE",
        archive: "/MINPUYO/MIN_GRP.VDD;1",
        expected_btx: 16,
    },
    BtxNameConsumerSpec {
        exe: "PCOMI.EXE",
        archive: "/PCOMI/PC1_GRP.VDD;1",
        expected_btx: 5,
    },
    BtxNameConsumerSpec {
        exe: "PUYO1.EXE",
        archive: "/PUYO1/PP1_GRP.VDD;1",
        expected_btx: 143,
    },
    BtxNameConsumerSpec {
        exe: "PUYO2.EXE",
        archive: "/PUYO2/PP2_GRP.VDD;1",
        expected_btx: 215,
    },
    BtxNameConsumerSpec {
        exe: "QUEST.EXE",
        archive: "/QUEST/RPG_GRP.VDD;1",
        expected_btx: 123,
    },
];

pub const BOX_BTX_NAME_ANALYSIS_SHA256: [u8; 32] = [
    0xA0, 0xA1, 0xBE, 0x59, 0x40, 0xB0, 0x77, 0x23, 0x26, 0x0B, 0x7F, 0x2B, 0x53, 0x02, 0x99, 0x87,
    0xD3, 0x36, 0x67, 0xA4, 0x81, 0x47, 0x7B, 0xAA, 0x86, 0xEB, 0x9D, 0xAD, 0x89, 0x4F, 0xAC, 0x11,
];

/// 물리 BTX 하나와 자기 EXE의 이름표 연결.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxNameLink {
    pub exe: &'static str,
    pub archive: &'static str,
    pub asset_name: String,
    pub exe_text_offset: usize,
    pub runtime_addr: u32,
    pub static_ref_sites: Vec<u32>,
    pub entry_reached_ref_sites: Vec<u32>,
    pub table_runtime_start: u32,
    pub table_runtime_end: u32,
    pub table_static_ref_sites: Vec<u32>,
    pub table_entry_reached_ref_sites: Vec<u32>,
    pub descriptor_pointer_offset: usize,
    pub descriptor_pointer_runtime: u32,
    pub descriptor_runtime_start: u32,
    pub descriptor_runtime_end: u32,
    pub descriptor_static_ref_sites: Vec<u32>,
    pub descriptor_entry_reached_ref_sites: Vec<u32>,
}

impl BtxNameLink {
    pub fn static_evidence(&self) -> &'static str {
        if !self.entry_reached_ref_sites.is_empty() {
            "entry_trace_static_ref"
        } else if !self.static_ref_sites.is_empty() {
            "linear_static_ref"
        } else if !self.table_entry_reached_ref_sites.is_empty() {
            "entry_trace_table_ref"
        } else if !self.table_static_ref_sites.is_empty() {
            "linear_table_ref"
        } else if !self.descriptor_entry_reached_ref_sites.is_empty() {
            "entry_trace_descriptor_ref"
        } else if !self.descriptor_static_ref_sites.is_empty() {
            "linear_descriptor_ref"
        } else {
            "no_static_ref"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxNameConsumerSummary {
    pub exe: &'static str,
    pub archive: &'static str,
    pub physical_btx: usize,
    pub literal_names: usize,
    pub statically_referenced_assets: usize,
    pub entry_referenced_assets: usize,
    pub table_bytes: usize,
    pub table_static_ref_sites: Vec<u32>,
    pub table_entry_reached_ref_sites: Vec<u32>,
    pub descriptor_pointers: usize,
    pub descriptor_span_bytes: usize,
    pub descriptor_static_ref_sites: Vec<u32>,
    pub descriptor_entry_reached_ref_sites: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxNameAnalysis {
    pub summaries: Vec<BtxNameConsumerSummary>,
    pub links: Vec<BtxNameLink>,
    pub analysis_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BtxNameError {
    EmptySpecs,
    DuplicateSpecExecutable(&'static str),
    DuplicateSpecArchive(&'static str),
    DuplicateExecutable(&'static str),
    DuplicateArchive(String),
    UnexpectedExecutable(&'static str),
    UnexpectedArchive(String),
    MissingExecutable(&'static str),
    MissingArchive(&'static str),
    TextLengthMismatch {
        exe: &'static str,
        expected: usize,
        actual: usize,
    },
    UnalignedText(&'static str),
    PhysicalCountMismatch {
        archive: &'static str,
        expected: usize,
        actual: usize,
    },
    MissingLiteral {
        exe: &'static str,
        asset: String,
    },
    DuplicateLiteral {
        exe: &'static str,
        asset: String,
        count: usize,
    },
    MissingDescriptorPointer {
        exe: &'static str,
        asset: String,
    },
    DuplicateDescriptorPointer {
        exe: &'static str,
        asset: String,
        count: usize,
    },
    DescriptorLayout {
        exe: &'static str,
        reason: String,
    },
    ExtraLiteral {
        exe: &'static str,
        asset: String,
    },
    BoxSpecCount {
        expected: usize,
        actual: usize,
    },
    AnalysisDigestMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

impl fmt::Display for BtxNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySpecs => write!(f, "BTX 이름표 소비 spec이 없음"),
            Self::DuplicateSpecExecutable(exe) => {
                write!(f, "BTX 이름표 spec EXE 중복: {exe}")
            }
            Self::DuplicateSpecArchive(archive) => {
                write!(f, "BTX 이름표 spec GRP 중복: {archive}")
            }
            Self::DuplicateExecutable(exe) => write!(f, "BTX 이름표 입력 EXE 중복: {exe}"),
            Self::DuplicateArchive(archive) => {
                write!(f, "BTX 이름표 입력 GRP 중복: {archive}")
            }
            Self::UnexpectedExecutable(exe) => {
                write!(f, "BTX 이름표 spec 밖 EXE 입력: {exe}")
            }
            Self::UnexpectedArchive(archive) => {
                write!(f, "BTX 이름표 spec 밖 GRP 입력: {archive}")
            }
            Self::MissingExecutable(exe) => write!(f, "BTX 이름표 입력 EXE 누락: {exe}"),
            Self::MissingArchive(archive) => write!(f, "BTX 이름표 입력 GRP 누락: {archive}"),
            Self::TextLengthMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe} text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText(exe) => write!(f, "{exe} text 길이가 4바이트 정렬이 아님"),
            Self::PhysicalCountMismatch {
                archive,
                expected,
                actual,
            } => write!(
                f,
                "{archive} 물리 BTX 수 불일치: expected={expected}, actual={actual}"
            ),
            Self::MissingLiteral { exe, asset } => {
                write!(f, "{exe}: {asset} NUL 종료 이름표가 없음")
            }
            Self::DuplicateLiteral { exe, asset, count } => {
                write!(f, "{exe}: {asset} 이름표가 {count}개로 중복")
            }
            Self::MissingDescriptorPointer { exe, asset } => {
                write!(
                    f,
                    "{exe}: {asset} 이름 주소를 담은 descriptor 포인터가 없음"
                )
            }
            Self::DuplicateDescriptorPointer { exe, asset, count } => write!(
                f,
                "{exe}: {asset} 이름 주소 descriptor 포인터가 {count}개로 중복"
            ),
            Self::DescriptorLayout { exe, reason } => {
                write!(f, "{exe}: BTX descriptor layout 불일치: {reason}")
            }
            Self::ExtraLiteral { exe, asset } => {
                write!(f, "{exe}: 자기 GRP에 없는 BTX 이름표: {asset}")
            }
            Self::BoxSpecCount { expected, actual } => write!(
                f,
                "BOX BTX 이름표 spec 수 불일치: expected={expected}, actual={actual}"
            ),
            Self::AnalysisDigestMismatch { expected, actual } => write!(
                f,
                "BOX BTX 이름표 분석 digest 불일치: expected={}, actual={}",
                hex_digest(expected),
                hex_digest(actual)
            ),
        }
    }
}

impl Error for BtxNameError {}

/// 선언한 EXE ↔ GRP 쌍의 모든 물리 BTX 이름을 연결한다.
pub fn audit_btx_name_consumers<'a>(
    specs: &[BtxNameConsumerSpec],
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<BtxNameAnalysis, BtxNameError> {
    if specs.is_empty() {
        return Err(BtxNameError::EmptySpecs);
    }
    let mut spec_executables = BTreeSet::new();
    let mut spec_archives = BTreeSet::new();
    for spec in specs {
        if !spec_executables.insert(spec.exe) {
            return Err(BtxNameError::DuplicateSpecExecutable(spec.exe));
        }
        if !spec_archives.insert(spec.archive) {
            return Err(BtxNameError::DuplicateSpecArchive(spec.archive));
        }
    }

    let mut executable_inputs = BTreeMap::new();
    for (map, text) in executables {
        if executable_inputs.insert(map.name, (map, text)).is_some() {
            return Err(BtxNameError::DuplicateExecutable(map.name));
        }
    }
    let mut archive_inputs = BTreeMap::new();
    for (archive_id, archive) in archives {
        if archive_inputs.insert(archive_id, archive).is_some() {
            return Err(BtxNameError::DuplicateArchive(archive_id.to_string()));
        }
    }
    if let Some(&exe) = executable_inputs
        .keys()
        .find(|&&exe| !spec_executables.contains(exe))
    {
        return Err(BtxNameError::UnexpectedExecutable(exe));
    }
    if let Some(&archive) = archive_inputs
        .keys()
        .find(|&&archive| !spec_archives.contains(archive))
    {
        return Err(BtxNameError::UnexpectedArchive(archive.to_string()));
    }

    let mut summaries = Vec::with_capacity(specs.len());
    let mut links = Vec::new();
    for spec in specs {
        let &(map, text) = executable_inputs
            .get(spec.exe)
            .ok_or(BtxNameError::MissingExecutable(spec.exe))?;
        if text.len() != map.text_len as usize {
            return Err(BtxNameError::TextLengthMismatch {
                exe: spec.exe,
                expected: map.text_len as usize,
                actual: text.len(),
            });
        }
        let archive = archive_inputs
            .get(spec.archive)
            .copied()
            .ok_or(BtxNameError::MissingArchive(spec.archive))?;
        let btx_names = archive
            .entries()
            .iter()
            .filter(|entry| entry.name().to_ascii_uppercase().ends_with(".BTX"))
            .map(|entry| entry.name().to_string())
            .collect::<Vec<_>>();
        if btx_names.len() != spec.expected_btx {
            return Err(BtxNameError::PhysicalCountMismatch {
                archive: spec.archive,
                expected: spec.expected_btx,
                actual: btx_names.len(),
            });
        }

        let literal_rows = scan_btx_literals(text);
        let mut literals_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (name, offset) in literal_rows {
            literals_by_name.entry(name).or_default().push(offset);
        }
        let archive_names = btx_names.iter().cloned().collect::<BTreeSet<_>>();
        if let Some(extra) = literals_by_name
            .keys()
            .find(|name| !archive_names.contains(*name))
        {
            return Err(BtxNameError::ExtraLiteral {
                exe: spec.exe,
                asset: extra.clone(),
            });
        }
        for asset_name in &btx_names {
            let offsets =
                literals_by_name
                    .get(asset_name)
                    .ok_or_else(|| BtxNameError::MissingLiteral {
                        exe: spec.exe,
                        asset: asset_name.clone(),
                    })?;
            if offsets.len() != 1 {
                return Err(BtxNameError::DuplicateLiteral {
                    exe: spec.exe,
                    asset: asset_name.clone(),
                    count: offsets.len(),
                });
            }
        }
        let mut descriptor_pointer_offsets = BTreeMap::new();
        for asset_name in &btx_names {
            let literal_offset = literals_by_name[asset_name][0];
            let literal_runtime = map.load_addr + literal_offset as u32;
            let pointer_bytes = literal_runtime.to_le_bytes();
            let pointer_offsets = text
                .windows(pointer_bytes.len())
                .enumerate()
                .filter_map(|(offset, window)| (window == pointer_bytes).then_some(offset))
                .collect::<Vec<_>>();
            match pointer_offsets.as_slice() {
                [] => {
                    return Err(BtxNameError::MissingDescriptorPointer {
                        exe: spec.exe,
                        asset: asset_name.clone(),
                    });
                }
                [pointer_offset] => {
                    descriptor_pointer_offsets.insert(asset_name.clone(), *pointer_offset);
                }
                _ => {
                    return Err(BtxNameError::DuplicateDescriptorPointer {
                        exe: spec.exe,
                        asset: asset_name.clone(),
                        count: pointer_offsets.len(),
                    });
                }
            }
        }

        let instructions = linear::sweep(map, text).ok_or(BtxNameError::UnalignedText(spec.exe))?;
        let address_loads = find_address_loads(&instructions, DEFAULT_WINDOW);
        let entry_trace = trace(map, text, &[]);
        let table_text_start = literals_by_name
            .values()
            .flatten()
            .copied()
            .min()
            .expect("물리 BTX가 있으면 이름표도 하나 이상 있다");
        let table_text_end = literals_by_name
            .iter()
            .flat_map(|(name, offsets)| offsets.iter().map(move |offset| offset + name.len() + 1))
            .max()
            .expect("물리 BTX가 있으면 이름표도 하나 이상 있다");
        let table_runtime_start = map.load_addr + table_text_start as u32;
        let table_runtime_end = map.load_addr + table_text_end as u32;
        let mut table_static_ref_sites = BTreeSet::new();
        let mut table_entry_reached_ref_sites = BTreeSet::new();
        for load in &address_loads {
            let value = normalize_kseg(load.value);
            if value < table_runtime_start || value >= table_runtime_end {
                continue;
            }
            table_static_ref_sites.insert(load.lui_addr);
            if entry_trace.instructions.contains_key(&load.lui_addr)
                && entry_trace.instructions.contains_key(&load.combine_addr)
            {
                table_entry_reached_ref_sites.insert(load.lui_addr);
            }
        }
        let table_static_ref_sites = table_static_ref_sites.into_iter().collect::<Vec<_>>();
        let table_entry_reached_ref_sites = table_entry_reached_ref_sites
            .into_iter()
            .collect::<Vec<_>>();
        let first_btx_pointer = descriptor_pointer_offsets
            .values()
            .copied()
            .min()
            .expect("물리 BTX가 있으면 descriptor 포인터도 하나 이상 있다");
        for (asset_name, &pointer_offset) in &descriptor_pointer_offsets {
            if !(pointer_offset - first_btx_pointer).is_multiple_of(36) {
                return Err(BtxNameError::DescriptorLayout {
                    exe: spec.exe,
                    reason: format!(
                        "{asset_name} 포인터 0x{pointer_offset:X}가 첫 BTX 포인터 \
                         0x{first_btx_pointer:X}의 36바이트 격자에 없음"
                    ),
                });
            }
        }
        let descriptor_text_start =
            first_btx_pointer
                .checked_sub(40)
                .ok_or_else(|| BtxNameError::DescriptorLayout {
                    exe: spec.exe,
                    reason: "첫 BTX 포인터 앞 block/VDD descriptor 공간이 없음".to_string(),
                })?;
        let vdd_pointer_offset = descriptor_text_start + 4;
        let archive_name = spec
            .archive
            .rsplit('/')
            .next()
            .and_then(|name| name.split(';').next())
            .expect("고정 archive spec은 파일명을 갖는다");
        let archive_literal_offsets = find_exact_nul_literals(text, archive_name.as_bytes());
        let [archive_literal_offset] = archive_literal_offsets.as_slice() else {
            return Err(BtxNameError::DescriptorLayout {
                exe: spec.exe,
                reason: format!(
                    "{archive_name} 이름표 수가 1이 아님: {}",
                    archive_literal_offsets.len()
                ),
            });
        };
        let expected_vdd_pointer = map.load_addr + *archive_literal_offset as u32;
        let actual_vdd_pointer = text
            .get(vdd_pointer_offset..vdd_pointer_offset + 4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("길이를 검사했다")))
            .ok_or_else(|| BtxNameError::DescriptorLayout {
                exe: spec.exe,
                reason: "VDD descriptor 포인터가 text 범위 밖임".to_string(),
            })?;
        if actual_vdd_pointer != expected_vdd_pointer {
            return Err(BtxNameError::DescriptorLayout {
                exe: spec.exe,
                reason: format!(
                    "VDD descriptor 포인터 expected=0x{expected_vdd_pointer:08X}, \
                     actual=0x{actual_vdd_pointer:08X}"
                ),
            });
        }
        let descriptor_text_end = descriptor_pointer_offsets
            .values()
            .copied()
            .max()
            .expect("물리 BTX가 있으면 descriptor 포인터도 하나 이상 있다")
            .checked_add(36)
            .ok_or_else(|| BtxNameError::DescriptorLayout {
                exe: spec.exe,
                reason: "마지막 descriptor 범위 overflow".to_string(),
            })?;
        if descriptor_text_end > text.len() {
            return Err(BtxNameError::DescriptorLayout {
                exe: spec.exe,
                reason: "마지막 descriptor가 text 범위 밖임".to_string(),
            });
        }
        let descriptor_runtime_start = map.load_addr + descriptor_text_start as u32;
        let descriptor_runtime_end = map.load_addr + descriptor_text_end as u32;
        let mut descriptor_static_ref_sites = BTreeSet::new();
        let mut descriptor_entry_reached_ref_sites = BTreeSet::new();
        for load in &address_loads {
            let value = normalize_kseg(load.value);
            if value < descriptor_runtime_start || value >= descriptor_runtime_end {
                continue;
            }
            descriptor_static_ref_sites.insert(load.lui_addr);
            if entry_trace.instructions.contains_key(&load.lui_addr)
                && entry_trace.instructions.contains_key(&load.combine_addr)
            {
                descriptor_entry_reached_ref_sites.insert(load.lui_addr);
            }
        }
        let descriptor_static_ref_sites =
            descriptor_static_ref_sites.into_iter().collect::<Vec<_>>();
        let descriptor_entry_reached_ref_sites = descriptor_entry_reached_ref_sites
            .into_iter()
            .collect::<Vec<_>>();
        let summary_start = links.len();
        for asset_name in btx_names {
            let offsets = literals_by_name
                .get(&asset_name)
                .expect("이름표 누락은 span 계산 전에 거부했다");
            let [exe_text_offset] = offsets.as_slice() else {
                unreachable!("이름표 중복은 span 계산 전에 거부했다");
            };
            let runtime_addr = map.load_addr + *exe_text_offset as u32;
            let descriptor_pointer_offset = descriptor_pointer_offsets[&asset_name];
            let descriptor_pointer_runtime = map.load_addr + descriptor_pointer_offset as u32;
            let mut static_ref_sites = BTreeSet::new();
            let mut entry_reached_ref_sites = BTreeSet::new();
            for load in &address_loads {
                if normalize_kseg(load.value) != runtime_addr {
                    continue;
                }
                static_ref_sites.insert(load.lui_addr);
                if entry_trace.instructions.contains_key(&load.lui_addr)
                    && entry_trace.instructions.contains_key(&load.combine_addr)
                {
                    entry_reached_ref_sites.insert(load.lui_addr);
                }
            }
            links.push(BtxNameLink {
                exe: spec.exe,
                archive: spec.archive,
                asset_name,
                exe_text_offset: *exe_text_offset,
                runtime_addr,
                static_ref_sites: static_ref_sites.into_iter().collect(),
                entry_reached_ref_sites: entry_reached_ref_sites.into_iter().collect(),
                table_runtime_start,
                table_runtime_end,
                table_static_ref_sites: table_static_ref_sites.clone(),
                table_entry_reached_ref_sites: table_entry_reached_ref_sites.clone(),
                descriptor_pointer_offset,
                descriptor_pointer_runtime,
                descriptor_runtime_start,
                descriptor_runtime_end,
                descriptor_static_ref_sites: descriptor_static_ref_sites.clone(),
                descriptor_entry_reached_ref_sites: descriptor_entry_reached_ref_sites.clone(),
            });
        }
        let pair_links = &links[summary_start..];
        summaries.push(BtxNameConsumerSummary {
            exe: spec.exe,
            archive: spec.archive,
            physical_btx: pair_links.len(),
            literal_names: literals_by_name.len(),
            statically_referenced_assets: pair_links
                .iter()
                .filter(|link| !link.static_ref_sites.is_empty())
                .count(),
            entry_referenced_assets: pair_links
                .iter()
                .filter(|link| !link.entry_reached_ref_sites.is_empty())
                .count(),
            table_bytes: table_text_end - table_text_start,
            table_static_ref_sites,
            table_entry_reached_ref_sites,
            descriptor_pointers: descriptor_pointer_offsets.len(),
            descriptor_span_bytes: descriptor_text_end - descriptor_text_start,
            descriptor_static_ref_sites,
            descriptor_entry_reached_ref_sites,
        });
    }

    let analysis_sha256 = digest_links(&links);
    Ok(BtxNameAnalysis {
        summaries,
        links,
        analysis_sha256,
    })
}

/// 지원 원본의 11쌍과 물리 BTX 732개를 검증한다.
pub fn verify_box_btx_name_consumers<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<BtxNameAnalysis, BtxNameError> {
    let analysis = audit_btx_name_consumers(&BOX_BTX_NAME_CONSUMERS, executables, archives)?;
    if analysis.summaries.len() != BOX_BTX_NAME_CONSUMERS.len() {
        return Err(BtxNameError::BoxSpecCount {
            expected: BOX_BTX_NAME_CONSUMERS.len(),
            actual: analysis.summaries.len(),
        });
    }
    if analysis.analysis_sha256 != BOX_BTX_NAME_ANALYSIS_SHA256 {
        return Err(BtxNameError::AnalysisDigestMismatch {
            expected: BOX_BTX_NAME_ANALYSIS_SHA256,
            actual: analysis.analysis_sha256,
        });
    }
    Ok(analysis)
}

/// 물리 BTX별 EXE 이름표와 정적 근거의 결정적 TSV.
pub fn render_btx_name_consumers_tsv(analysis: &BtxNameAnalysis) -> String {
    let mut out = String::from(
        "exe\tarchive\tasset_name\texe_text_offset\truntime_addr\tstatic_ref_count\
         \tentry_reached_ref_count\ttable_runtime_start\ttable_runtime_end\
         \ttable_static_ref_count\ttable_entry_reached_ref_count\tstatic_evidence\
         \tdescriptor_pointer_offset\tdescriptor_pointer_runtime\
         \tdescriptor_runtime_start\tdescriptor_runtime_end\
         \tdescriptor_static_ref_count\tdescriptor_entry_reached_ref_count\
         \truntime_verdict\tstatic_ref_sites\tentry_reached_ref_sites\
         \ttable_static_ref_sites\ttable_entry_reached_ref_sites\
         \tdescriptor_static_ref_sites\tdescriptor_entry_reached_ref_sites\n",
    );
    for link in &analysis.links {
        writeln!(
            out,
            "{}\t{}\t{}\t0x{:06X}\t0x{:08X}\t{}\t{}\t0x{:08X}\t0x{:08X}\t{}\
             \t{}\t{}\t0x{:06X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\
             \tunverified\t{}\t{}\t{}\t{}\t{}\t{}",
            link.exe,
            link.archive,
            link.asset_name,
            link.exe_text_offset,
            link.runtime_addr,
            link.static_ref_sites.len(),
            link.entry_reached_ref_sites.len(),
            link.table_runtime_start,
            link.table_runtime_end,
            link.table_static_ref_sites.len(),
            link.table_entry_reached_ref_sites.len(),
            link.static_evidence(),
            link.descriptor_pointer_offset,
            link.descriptor_pointer_runtime,
            link.descriptor_runtime_start,
            link.descriptor_runtime_end,
            link.descriptor_static_ref_sites.len(),
            link.descriptor_entry_reached_ref_sites.len(),
            hex_addresses(&link.static_ref_sites),
            hex_addresses(&link.entry_reached_ref_sites),
            hex_addresses(&link.table_static_ref_sites),
            hex_addresses(&link.table_entry_reached_ref_sites),
            hex_addresses(&link.descriptor_static_ref_sites),
            hex_addresses(&link.descriptor_entry_reached_ref_sites),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    out
}

fn scan_btx_literals(text: &[u8]) -> Vec<(String, usize)> {
    const SUFFIX: &[u8] = b".BTX\0";
    let mut out = Vec::new();
    for suffix_start in 0..=text.len().saturating_sub(SUFFIX.len()) {
        if text.get(suffix_start..suffix_start + SUFFIX.len()) != Some(SUFFIX) {
            continue;
        }
        let mut start = suffix_start;
        while start > 0 && is_asset_base_byte(text[start - 1]) {
            start -= 1;
        }
        if start == suffix_start {
            continue;
        }
        let Some(name) = std::str::from_utf8(&text[start..suffix_start + SUFFIX.len() - 1]).ok()
        else {
            continue;
        };
        out.push((name.to_string(), start));
    }
    out
}

fn find_exact_nul_literals(text: &[u8], name: &[u8]) -> Vec<usize> {
    let mut needle = name.to_vec();
    needle.push(0);
    text.windows(needle.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == needle).then_some(offset))
        .collect()
}

fn is_asset_base_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn digest_links(links: &[BtxNameLink]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for link in links {
        digest.update(link.exe.as_bytes());
        digest.update([0]);
        digest.update(link.archive.as_bytes());
        digest.update([0]);
        digest.update(link.asset_name.as_bytes());
        digest.update([0]);
        digest.update((link.exe_text_offset as u64).to_le_bytes());
        for site in &link.static_ref_sites {
            digest.update(site.to_le_bytes());
        }
        digest.update([0xFF]);
        for site in &link.entry_reached_ref_sites {
            digest.update(site.to_le_bytes());
        }
        digest.update([0xFE]);
        digest.update(link.table_runtime_start.to_le_bytes());
        digest.update(link.table_runtime_end.to_le_bytes());
        for site in &link.table_static_ref_sites {
            digest.update(site.to_le_bytes());
        }
        digest.update([0xFD]);
        for site in &link.table_entry_reached_ref_sites {
            digest.update(site.to_le_bytes());
        }
        digest.update([0xFC]);
        digest.update((link.descriptor_pointer_offset as u64).to_le_bytes());
        digest.update(link.descriptor_pointer_runtime.to_le_bytes());
        digest.update(link.descriptor_runtime_start.to_le_bytes());
        digest.update(link.descriptor_runtime_end.to_le_bytes());
        for site in &link.descriptor_static_ref_sites {
            digest.update(site.to_le_bytes());
        }
        digest.update([0xFB]);
        for site in &link.descriptor_entry_reached_ref_sites {
            digest.update(site.to_le_bytes());
        }
        digest.update([0xFA]);
    }
    digest.finalize().into()
}

fn hex_addresses(addresses: &[u32]) -> String {
    addresses
        .iter()
        .map(|address| format!("0x{address:08X}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "btx_names_tests.rs"]
mod btx_names_tests;
