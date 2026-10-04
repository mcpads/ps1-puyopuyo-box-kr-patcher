//! QUEST 밖 SEQ의 적재 오버레이 footer와 raw 계열 경계.
//!
//! 비-QUEST `.SEQ`는 모두 같은 script blob이 아니다. 이 모듈은 파일 끝의
//! 44바이트 적재 footer를 원본 계약으로 검증해 실행 오버레이와 불투명 raw를
//! 분리한다. 오버레이 안의 가나 run도 아직 text record나 쓰기 span이 아니며,
//! `DRAC_SEQ.SEQ`의 header→record→text block→repertoire 내부 pointer
//! chain 역시 실제 생성자 제5인자 소비가 닫히기 전에는 정적 root 증거로만
//! 취급한다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::RuntimeAssetMap;

use super::subgame_seq_text_candidates::SubgameSeqTextCandidateAnalysis;

pub const SUBGAME_SEQ_LAYOUT_SCHEMA_VERSION: &str = "subgame-seq-layouts-v1";
pub const SUBGAME_SEQ_OVERLAY_FOOTER_LEN: usize = 44;
pub const BOX_SUBGAME_SEQ_LAYOUT_FILES: usize = 324;
pub const BOX_SUBGAME_SEQ_OVERLAYS: usize = 207;
pub const BOX_SUBGAME_SEQ_RAW_OPAQUE: usize = 117;
pub const BOX_SUBGAME_SEQ_OVERLAY_CANDIDATES: usize = 738;
pub const BOX_SUBGAME_SEQ_OVERLAY_CODE_UNITS: usize = 6_866;
pub const BOX_SUBGAME_SEQ_RAW_CANDIDATES: usize = 0;
pub const BOX_SUBGAME_SEQ_RAW_CODE_UNITS: usize = 0;

/// 지원 원본 archive별 두 layout의 정확한 분모.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxSubgameSeqLayoutExpectation {
    pub archive: &'static str,
    pub overlays: usize,
    pub raw_opaque: usize,
}

pub const BOX_SUBGAME_SEQ_LAYOUT_EXPECTATIONS: [BoxSubgameSeqLayoutExpectation; 6] = [
    layout_expectation("/FIL_SEQ.VDD;1", 1, 0),
    layout_expectation("/GOCHA1/TUU_SEQ.VDD;1", 0, 49),
    layout_expectation("/KOUSIKI/KOU_SEQ.VDD;1", 2, 0),
    layout_expectation("/PCOMI/PC1_SEQ.VDD;1", 2, 0),
    layout_expectation("/PUYO1/PP1_SEQ.VDD;1", 101, 18),
    layout_expectation("/PUYO2/PP2_SEQ.VDD;1", 101, 50),
];

const fn layout_expectation(
    archive: &'static str,
    overlays: usize,
    raw_opaque: usize,
) -> BoxSubgameSeqLayoutExpectation {
    BoxSubgameSeqLayoutExpectation {
        archive,
        overlays,
        raw_opaque,
    }
}

/// 오버레이 적재 base별 파일 수.
pub const BOX_SUBGAME_SEQ_LOAD_BASE_COUNTS: [(u32, usize); 8] = [
    (0x800A_6800, 1),
    (0x8019_9A70, 3),
    (0x8019_E270, 15),
    (0x801A_6270, 8),
    (0x801A_E270, 1),
    (0x801B_0270, 6),
    (0x801B_5270, 171),
    (0x801E_FA70, 2),
];

/// 324행 layout 보고서의 지원 원본 digest.
pub const BOX_SUBGAME_SEQ_LAYOUT_REPORT_SHA256: [u8; 32] = [
    0x96, 0x73, 0xf3, 0x8a, 0xe4, 0xfc, 0x99, 0x39, 0xb5, 0x4d, 0xd4, 0x24, 0x25, 0x78, 0x51, 0x81,
    0xd6, 0xd3, 0x1d, 0x62, 0x6e, 0xde, 0x28, 0xec, 0x6b, 0x0b, 0x46, 0xba, 0x82, 0x14, 0xf4, 0xfa,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubgameSeqLayoutKind {
    Overlay,
    RawOpaque,
}

impl SubgameSeqLayoutKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Overlay => "overlay",
            Self::RawOpaque => "raw-opaque",
        }
    }
}

/// 파일 끝 11개 little-endian word로 선언된 적재 계약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqOverlayFooter {
    pub footer_offset: usize,
    pub load_base: u32,
    pub initialized_len: u32,
    pub bss_addr: u32,
    pub bss_len: u32,
    pub next_addr: u32,
}

impl SubgameSeqOverlayFooter {
    pub fn runtime_map(self) -> Option<RuntimeAssetMap> {
        RuntimeAssetMap::new(self.load_base, self.initialized_len)
    }
}

/// 파일별 layout과 기존 가나 run 하한의 교차 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqLayoutRow {
    pub scope: &'static str,
    pub archive: String,
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub bytes: usize,
    pub kind: SubgameSeqLayoutKind,
    pub footer: Option<SubgameSeqOverlayFooter>,
    pub candidates: usize,
    pub code_units: usize,
    pub root: Option<SubgameSeqRootEvidence>,
}

/// 대표 DRAC 오버레이에서 원본 word로 확인한 내부 root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqRootEvidence {
    pub module_root_pointer_field_offset: usize,
    pub module_root_pointer_field_runtime: u32,
    pub module_root_offset: usize,
    pub module_root_runtime: u32,
    pub child_pointer_field_offset: usize,
    pub child_pointer_field_runtime: u32,
    pub child_offset: usize,
    pub child_runtime: u32,
    pub text_block_pointer_field_offset: usize,
    pub text_block_pointer_field_runtime: u32,
    pub text_block_offset: usize,
    pub text_block_runtime: u32,
    pub pointer_field_offset: usize,
    pub pointer_field_runtime: u32,
    pub target_offset: usize,
    pub target_runtime: u32,
    pub text_block_to_repertoire_bytes: usize,
    pub required_candidate_offsets: &'static [usize],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqLayoutAnalysis {
    pub rows: Vec<SubgameSeqLayoutRow>,
    pub overlays: usize,
    pub raw_opaque: usize,
    pub overlay_candidates: usize,
    pub overlay_code_units: usize,
    pub raw_candidates: usize,
    pub raw_code_units: usize,
    pub load_base_counts: BTreeMap<u32, usize>,
    pub roots: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqLayoutError(String);

impl fmt::Display for SubgameSeqLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqLayoutError {}

fn error(message: impl Into<String>) -> SubgameSeqLayoutError {
    SubgameSeqLayoutError(message.into())
}

const DRAC_PUYO1_CANDIDATES: &[(usize, &str)] = &[
    (0x1AAC, "あいうおかきけこはふまもよりれをテトン「」、！？地獄・　　　　　さしたてとなにのんっゃじばぷコスー前通勝負美少女"),
    (0x1B22, "お前っ！"),
    (0x1B32, "ここを通りたければ"),
];
const DRAC_PUYO2_CANDIDATES: &[(usize, &str)] = &[
    (0x1A18, "あいうおかきけこはふまもよりれをテトン「」、！？地獄・　　　　　さしたてとなにのんっゃじばぷコスー前通勝負美少女"),
    (0x1A8E, "お前っ！"),
    (0x1A9E, "ここを通りたければ"),
];

#[derive(Debug, Clone, Copy)]
struct RootSpec {
    archive: &'static str,
    sequence: &'static str,
    sequence_sha256: &'static str,
    bytes: usize,
    load_base: u32,
    initialized_len: u32,
    bss_addr: u32,
    next_addr: u32,
    text_block_offset: usize,
    repertoire_offset: usize,
    candidates: &'static [(usize, &'static str)],
}

const ROOT_SPECS: [RootSpec; 2] = [
    RootSpec {
        archive: "/PUYO1/PP1_SEQ.VDD;1",
        sequence: "DRAC_SEQ.SEQ",
        sequence_sha256: "9881efd1577a7e00c0eca38ce9f8575d2756e7aafa3b0dc637477b79c273d51e",
        bytes: 0x1DEC,
        load_base: 0x801B_5270,
        initialized_len: 0x1DC0,
        bss_addr: 0x801B_7030,
        next_addr: 0x801B_7060,
        text_block_offset: 0x1A20,
        repertoire_offset: 0x1AAC,
        candidates: DRAC_PUYO1_CANDIDATES,
    },
    RootSpec {
        archive: "/PUYO2/PP2_SEQ.VDD;1",
        sequence: "DRAC_SEQ.SEQ",
        sequence_sha256: "02a71d36d234250e2c8010a90cde668ad007643d6f48b2edfae478476d8a78c6",
        bytes: 0x1D4C,
        load_base: 0x801B_5270,
        initialized_len: 0x1D20,
        bss_addr: 0x801B_6F90,
        next_addr: 0x801B_6FC0,
        text_block_offset: 0x198C,
        repertoire_offset: 0x1A18,
        candidates: DRAC_PUYO2_CANDIDATES,
    },
];

/// 한 SEQ를 footer 계약에 따라 분류한다.
pub fn parse_subgame_seq_overlay_footer(
    raw: &[u8],
) -> Result<Option<SubgameSeqOverlayFooter>, SubgameSeqLayoutError> {
    if raw.len() < SUBGAME_SEQ_OVERLAY_FOOTER_LEN {
        return Ok(None);
    }
    let footer_offset = raw.len() - SUBGAME_SEQ_OVERLAY_FOOTER_LEN;
    let mut words = [0u32; 11];
    for (index, word) in words.iter_mut().enumerate() {
        let offset = footer_offset + index * 4;
        *word = u32::from_le_bytes(raw[offset..offset + 4].try_into().expect("4바이트 word"));
    }

    let has_signature = words[9] == 0x0000_8000 && words[10] == 0x0080_0000;
    if !has_signature {
        return Ok(None);
    }
    let initialized_len = u32::try_from(footer_offset)
        .map_err(|_| error("subgame SEQ initialized 길이가 u32를 넘음"))?;
    let expected_bss_addr = words[3]
        .checked_add(initialized_len)
        .ok_or_else(|| error("subgame SEQ bss 주소 overflow"))?;
    let expected_next_addr = expected_bss_addr
        .checked_add(0x30)
        .ok_or_else(|| error("subgame SEQ next 주소 overflow"))?;
    let valid = raw.len().is_multiple_of(4)
        && words[0..3] == [0, 0, 0]
        && words[4] == initialized_len
        && words[5] == expected_bss_addr
        && words[6] == 0x2C
        && words[7] == expected_next_addr
        && words[8] == 0;
    if !valid {
        return Err(error(format!(
            "subgame SEQ overlay signature는 있으나 footer 계약이 깨짐: words={words:08X?}"
        )));
    }
    let footer = SubgameSeqOverlayFooter {
        footer_offset,
        load_base: words[3],
        initialized_len,
        bss_addr: words[5],
        bss_len: words[6],
        next_addr: words[7],
    };
    if footer.runtime_map().is_none() {
        return Err(error("subgame SEQ runtime 사상 overflow"));
    }
    Ok(Some(footer))
}

/// 해제된 SEQ와 기존 text 후보 분석을 교차해 layout을 결정한다.
pub fn audit_subgame_seq_layouts<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    text: &SubgameSeqTextCandidateAnalysis,
) -> Result<SubgameSeqLayoutAnalysis, SubgameSeqLayoutError> {
    let mut input = BTreeMap::<(String, String), &'a [u8]>::new();
    for (archive, sequence, raw) in sources {
        let key = (archive.to_string(), sequence.to_string());
        if input.insert(key.clone(), raw).is_some() {
            return Err(error(format!(
                "subgame SEQ layout 입력 중복: {}:{}",
                key.0, key.1
            )));
        }
    }
    if input.len() != text.summaries.len() {
        return Err(error(format!(
            "subgame SEQ layout/text 파일 수 불일치: layout={}, text={}",
            input.len(),
            text.summaries.len()
        )));
    }

    let mut rows = Vec::with_capacity(input.len());
    for summary in &text.summaries {
        let key = (summary.archive.clone(), summary.sequence.clone());
        let raw = input.get(&key).ok_or_else(|| {
            error(format!(
                "subgame SEQ text summary의 원본이 없음: {}:{}",
                summary.archive, summary.sequence
            ))
        })?;
        let sha256: [u8; 32] = Sha256::digest(raw).into();
        if raw.len() != summary.bytes || sha256 != summary.sequence_sha256 {
            return Err(error(format!(
                "subgame SEQ layout/text 원본 불일치: {}:{}",
                summary.archive, summary.sequence
            )));
        }
        let footer = parse_subgame_seq_overlay_footer(raw)?;
        let kind = if footer.is_some() {
            SubgameSeqLayoutKind::Overlay
        } else {
            SubgameSeqLayoutKind::RawOpaque
        };
        if kind == SubgameSeqLayoutKind::RawOpaque && summary.candidates != 0 {
            return Err(error(format!(
                "raw-opaque SEQ에 가나 run 후보가 있음: {}:{} ({})",
                summary.archive, summary.sequence, summary.candidates
            )));
        }
        rows.push(SubgameSeqLayoutRow {
            scope: summary.scope,
            archive: summary.archive.clone(),
            sequence: summary.sequence.clone(),
            sequence_sha256: sha256,
            bytes: raw.len(),
            kind,
            footer,
            candidates: summary.candidates,
            code_units: summary.code_units,
            root: None,
        });
    }

    let overlays = rows
        .iter()
        .filter(|row| row.kind == SubgameSeqLayoutKind::Overlay)
        .count();
    let raw_opaque = rows.len() - overlays;
    let overlay_candidates = rows
        .iter()
        .filter(|row| row.kind == SubgameSeqLayoutKind::Overlay)
        .map(|row| row.candidates)
        .sum();
    let overlay_code_units = rows
        .iter()
        .filter(|row| row.kind == SubgameSeqLayoutKind::Overlay)
        .map(|row| row.code_units)
        .sum();
    let raw_candidates = rows
        .iter()
        .filter(|row| row.kind == SubgameSeqLayoutKind::RawOpaque)
        .map(|row| row.candidates)
        .sum();
    let raw_code_units = rows
        .iter()
        .filter(|row| row.kind == SubgameSeqLayoutKind::RawOpaque)
        .map(|row| row.code_units)
        .sum();
    let mut load_base_counts = BTreeMap::new();
    for footer in rows.iter().filter_map(|row| row.footer) {
        *load_base_counts.entry(footer.load_base).or_insert(0) += 1;
    }

    Ok(SubgameSeqLayoutAnalysis {
        rows,
        overlays,
        raw_opaque,
        overlay_candidates,
        overlay_code_units,
        raw_candidates,
        raw_code_units,
        load_base_counts,
        roots: 0,
    })
}

/// 지원 원본의 layout 분모와 대표 DRAC 내부 root를 고정한다.
pub fn verify_box_subgame_seq_layouts<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    text: &SubgameSeqTextCandidateAnalysis,
) -> Result<SubgameSeqLayoutAnalysis, SubgameSeqLayoutError> {
    let sources = sources.into_iter().collect::<Vec<_>>();
    let raw_by_key = sources
        .iter()
        .map(|(archive, sequence, raw)| ((archive.to_string(), sequence.to_string()), *raw))
        .collect::<BTreeMap<_, _>>();
    let mut analysis = audit_subgame_seq_layouts(sources.iter().copied(), text)?;

    for expectation in BOX_SUBGAME_SEQ_LAYOUT_EXPECTATIONS {
        let rows = analysis
            .rows
            .iter()
            .filter(|row| row.archive == expectation.archive)
            .collect::<Vec<_>>();
        let actual = [
            rows.iter()
                .filter(|row| row.kind == SubgameSeqLayoutKind::Overlay)
                .count(),
            rows.iter()
                .filter(|row| row.kind == SubgameSeqLayoutKind::RawOpaque)
                .count(),
        ];
        let expected = [expectation.overlays, expectation.raw_opaque];
        if actual != expected {
            return Err(error(format!(
                "{} subgame SEQ layout 분모 불일치: expected={expected:?}, actual={actual:?}",
                expectation.archive
            )));
        }
    }
    let totals = [
        analysis.rows.len(),
        analysis.overlays,
        analysis.raw_opaque,
        analysis.overlay_candidates,
        analysis.overlay_code_units,
        analysis.raw_candidates,
        analysis.raw_code_units,
    ];
    let expected_totals = [
        BOX_SUBGAME_SEQ_LAYOUT_FILES,
        BOX_SUBGAME_SEQ_OVERLAYS,
        BOX_SUBGAME_SEQ_RAW_OPAQUE,
        BOX_SUBGAME_SEQ_OVERLAY_CANDIDATES,
        BOX_SUBGAME_SEQ_OVERLAY_CODE_UNITS,
        BOX_SUBGAME_SEQ_RAW_CANDIDATES,
        BOX_SUBGAME_SEQ_RAW_CODE_UNITS,
    ];
    if totals != expected_totals {
        return Err(error(format!(
            "subgame SEQ layout 합계 불일치: expected={expected_totals:?}, actual={totals:?}"
        )));
    }
    let expected_bases = BOX_SUBGAME_SEQ_LOAD_BASE_COUNTS
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    if analysis.load_base_counts != expected_bases {
        return Err(error(format!(
            "subgame SEQ load base 분포 불일치: expected={expected_bases:08X?}, actual={:08X?}",
            analysis.load_base_counts
        )));
    }

    for spec in ROOT_SPECS {
        let key = (spec.archive.to_string(), spec.sequence.to_string());
        let raw = raw_by_key.get(&key).ok_or_else(|| {
            error(format!(
                "DRAC root 원본이 없음: {}:{}",
                spec.archive, spec.sequence
            ))
        })?;
        let row = analysis
            .rows
            .iter_mut()
            .find(|row| row.archive == spec.archive && row.sequence == spec.sequence)
            .ok_or_else(|| {
                error(format!(
                    "DRAC root 행이 없음: {}:{}",
                    spec.archive, spec.sequence
                ))
            })?;
        let footer = row
            .footer
            .ok_or_else(|| error("DRAC root SEQ가 overlay가 아님"))?;
        let actual_sha = hex(&row.sequence_sha256);
        if actual_sha != spec.sequence_sha256
            || row.bytes != spec.bytes
            || footer.load_base != spec.load_base
            || footer.initialized_len != spec.initialized_len
            || footer.bss_addr != spec.bss_addr
            || footer.next_addr != spec.next_addr
        {
            return Err(error(format!(
                "DRAC root source 계약 불일치: {}:{}",
                spec.archive, spec.sequence
            )));
        }
        let map = footer
            .runtime_map()
            .ok_or_else(|| error("DRAC runtime 사상 overflow"))?;
        if read_u32(raw, 0)? != 3 {
            return Err(error(format!(
                "DRAC overlay header kind 불일치: {}:{}",
                spec.archive, spec.sequence
            )));
        }
        let (module_root_offset, module_root_runtime) = resolve_internal_pointer(raw, map, 0x0004)?;
        let (child_offset, child_runtime) = resolve_internal_pointer(raw, map, 0x0B3C)?;
        let (text_block_offset, text_block_runtime) = resolve_internal_pointer(raw, map, 0x0B58)?;
        let (target_offset, target_runtime) = resolve_internal_pointer(raw, map, 0x0B68)?;
        let actual_chain = [
            module_root_offset,
            child_offset,
            text_block_offset,
            target_offset,
        ];
        let expected_chain = [
            0x0B38,
            0x0B48,
            spec.text_block_offset,
            spec.repertoire_offset,
        ];
        if actual_chain != expected_chain {
            return Err(error(format!(
                "DRAC 내부 pointer chain 불일치: {}:{} expected={expected_chain:X?}, actual={actual_chain:X?}",
                spec.archive,
                spec.sequence,
            )));
        }
        let text_block_to_repertoire_bytes = target_offset
            .checked_sub(text_block_offset)
            .ok_or_else(|| error("DRAC repertoire가 text block 선두보다 앞섬"))?;
        if text_block_to_repertoire_bytes != 0x8C {
            return Err(error(format!(
                "DRAC text block/repertoire 간격 불일치: expected=0x8C, actual=0x{text_block_to_repertoire_bytes:X}"
            )));
        }
        for &(offset, expected_text) in spec.candidates {
            let candidate = text
                .candidates
                .iter()
                .find(|candidate| {
                    candidate.archive == spec.archive
                        && candidate.sequence == spec.sequence
                        && candidate.offset == offset
                })
                .ok_or_else(|| {
                    error(format!(
                        "DRAC 필수 후보가 없음: {}:0x{offset:X}",
                        spec.sequence
                    ))
                })?;
            if candidate.decoded.as_deref() != Some(expected_text) {
                return Err(error(format!(
                    "DRAC 필수 후보 원문 불일치: {}:0x{offset:X}",
                    spec.sequence
                )));
            }
        }
        row.root = Some(SubgameSeqRootEvidence {
            module_root_pointer_field_offset: 0x0004,
            module_root_pointer_field_runtime: map
                .runtime_addr(0x0004)
                .ok_or_else(|| error("DRAC module root field runtime 변환 실패"))?,
            module_root_offset,
            module_root_runtime,
            child_pointer_field_offset: 0x0B3C,
            child_pointer_field_runtime: map
                .runtime_addr(0x0B3C)
                .ok_or_else(|| error("DRAC child field runtime 변환 실패"))?,
            child_offset,
            child_runtime,
            text_block_pointer_field_offset: 0x0B58,
            text_block_pointer_field_runtime: map
                .runtime_addr(0x0B58)
                .ok_or_else(|| error("DRAC text block field runtime 변환 실패"))?,
            text_block_offset,
            text_block_runtime,
            pointer_field_offset: 0x0B68,
            pointer_field_runtime: map
                .runtime_addr(0x0B68)
                .ok_or_else(|| error("DRAC pointer field runtime 변환 실패"))?,
            target_offset,
            target_runtime,
            text_block_to_repertoire_bytes,
            required_candidate_offsets: match spec.archive {
                "/PUYO1/PP1_SEQ.VDD;1" => &[0x1AAC, 0x1B22, 0x1B32],
                "/PUYO2/PP2_SEQ.VDD;1" => &[0x1A18, 0x1A8E, 0x1A9E],
                _ => unreachable!("ROOT_SPECS는 두 archive만 가진다"),
            },
        });
    }
    analysis.roots = analysis
        .rows
        .iter()
        .filter(|row| row.root.is_some())
        .count();
    if analysis.roots != ROOT_SPECS.len() {
        return Err(error(format!(
            "subgame SEQ root 수 불일치: expected={}, actual={}",
            ROOT_SPECS.len(),
            analysis.roots
        )));
    }

    let report = render_subgame_seq_layouts_tsv(&analysis);
    let actual_report_sha256: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if actual_report_sha256 != BOX_SUBGAME_SEQ_LAYOUT_REPORT_SHA256 {
        return Err(error(format!(
            "subgame SEQ layout report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_LAYOUT_REPORT_SHA256),
            hex(&actual_report_sha256)
        )));
    }
    Ok(analysis)
}

/// 오버레이 footer와 raw 경계, 대표 root를 한 행씩 내보낸다.
pub fn render_subgame_seq_layouts_tsv(analysis: &SubgameSeqLayoutAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tscope\tarchive\tsequence\tsequence_sha256\tbytes\tlayout\t\
         footer_offset\tload_base\tinitialized_len\tbss_addr\tbss_len\tnext_addr\t\
         candidates\tcode_units\tmodule_root_pointer_file_runtime\tmodule_root_file_runtime\t\
         child_pointer_file_runtime\tchild_file_runtime\ttext_block_pointer_file_runtime\t\
         text_block_file_runtime\trepertoire_pointer_file_runtime\trepertoire_file_runtime\t\
         text_block_to_repertoire_bytes\t\
         required_candidate_offsets\tverdict\n",
    );
    for row in &analysis.rows {
        let footer = row.footer;
        let root = row.root;
        let required_offsets = root
            .map(|root| {
                root.required_candidate_offsets
                    .iter()
                    .map(|offset| format!("0x{offset:04X}"))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        let verdict = if root.is_some() {
            "static-root-pointer-chain-confirmed-runtime-display-unverified"
        } else if row.kind == SubgameSeqLayoutKind::Overlay {
            "overlay-layout-confirmed-content-boundary-unverified"
        } else {
            "raw-opaque-no-current-kana-run-not-no-text"
        };
        let columns = [
            SUBGAME_SEQ_LAYOUT_SCHEMA_VERSION.to_string(),
            row.scope.to_string(),
            row.archive.clone(),
            row.sequence.clone(),
            hex(&row.sequence_sha256),
            row.bytes.to_string(),
            row.kind.as_str().to_string(),
            optional_offset(footer.map(|footer| footer.footer_offset)),
            optional_runtime(footer.map(|footer| footer.load_base)),
            optional_u32(footer.map(|footer| footer.initialized_len)),
            optional_runtime(footer.map(|footer| footer.bss_addr)),
            optional_u32(footer.map(|footer| footer.bss_len)),
            optional_runtime(footer.map(|footer| footer.next_addr)),
            row.candidates.to_string(),
            row.code_units.to_string(),
            root.map(|root| {
                file_runtime(
                    root.module_root_pointer_field_offset,
                    root.module_root_pointer_field_runtime,
                )
            })
            .unwrap_or_default(),
            root.map(|root| file_runtime(root.module_root_offset, root.module_root_runtime))
                .unwrap_or_default(),
            root.map(|root| {
                file_runtime(
                    root.child_pointer_field_offset,
                    root.child_pointer_field_runtime,
                )
            })
            .unwrap_or_default(),
            root.map(|root| file_runtime(root.child_offset, root.child_runtime))
                .unwrap_or_default(),
            root.map(|root| {
                file_runtime(
                    root.text_block_pointer_field_offset,
                    root.text_block_pointer_field_runtime,
                )
            })
            .unwrap_or_default(),
            root.map(|root| file_runtime(root.text_block_offset, root.text_block_runtime))
                .unwrap_or_default(),
            root.map(|root| file_runtime(root.pointer_field_offset, root.pointer_field_runtime))
                .unwrap_or_default(),
            root.map(|root| file_runtime(root.target_offset, root.target_runtime))
                .unwrap_or_default(),
            root.map(|root| format!("0x{:X}", root.text_block_to_repertoire_bytes))
                .unwrap_or_default(),
            required_offsets,
            verdict.to_string(),
        ];
        writeln!(output, "{}", columns.join("\t")).expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn read_u32(raw: &[u8], offset: usize) -> Result<u32, SubgameSeqLayoutError> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| error("subgame SEQ u32 field overflow"))?;
    let bytes = raw
        .get(offset..end)
        .ok_or_else(|| error(format!("subgame SEQ u32 field 범위 밖: 0x{offset:X}")))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

fn resolve_internal_pointer(
    raw: &[u8],
    map: RuntimeAssetMap,
    pointer_field_offset: usize,
) -> Result<(usize, u32), SubgameSeqLayoutError> {
    if map.runtime_addr(pointer_field_offset).is_none() {
        return Err(error(format!(
            "subgame SEQ pointer field가 initialized 범위 밖: 0x{pointer_field_offset:X}"
        )));
    }
    let target_runtime = read_u32(raw, pointer_field_offset)?;
    let target_offset = map.file_offset(target_runtime).ok_or_else(|| {
        error(format!(
            "subgame SEQ pointer target이 initialized 범위 밖: field=0x{pointer_field_offset:X}, target=0x{target_runtime:08X}"
        ))
    })?;
    Ok((target_offset, target_runtime))
}

fn optional_offset(value: Option<usize>) -> String {
    value
        .map(|value| format!("0x{value:04X}"))
        .unwrap_or_default()
}

fn file_runtime(file_offset: usize, runtime: u32) -> String {
    format!("0x{file_offset:04X}@0x{runtime:08X}")
}

fn optional_runtime(value: Option<u32>) -> String {
    value
        .map(|value| format!("0x{value:08X}"))
        .unwrap_or_default()
}

fn optional_u32(value: Option<u32>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_layouts_tests.rs"]
mod subgame_seq_layouts_tests;
