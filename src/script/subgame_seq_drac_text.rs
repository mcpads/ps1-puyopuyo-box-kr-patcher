//! 대표 DRAC SEQ의 구조용 repertoire와 pointer-rooted message record 경계.
//!
//! 기존 가나 run 하한은 구조용 56자 repertoire를 대사 후보로 포함하는 한편,
//! 가나가 없는 `勝負？`를 놓친다. 이 모듈은 PP1/PP2의 exact pointer field
//! 5개와 동일한 message block 5개를 원본에서 다시 읽어 두 모집단을
//! 분리한다. 생성자 간접 호출과 runtime 표시가 열려 있으므로 아직 쓰기
//! 경계나 번역 승인으로 승격하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::text::decode_sjis;

use super::{SubgameSeqLayoutAnalysis, SubgameSeqTextCandidateAnalysis};

pub const SUBGAME_SEQ_DRAC_TEXT_SCHEMA_VERSION: &str = "subgame-seq-drac-text-v1";
pub const BOX_SUBGAME_SEQ_DRAC_TEXT_ROWS: usize = 12;
pub const BOX_SUBGAME_SEQ_DRAC_MESSAGE_RECORDS: usize = 10;
pub const BOX_SUBGAME_SEQ_DRAC_MESSAGE_SPANS: usize = 26;
pub const BOX_SUBGAME_SEQ_DRAC_MESSAGE_CODE_UNITS: usize = 178;
pub const BOX_SUBGAME_SEQ_DRAC_MESSAGE_PROTECTED_BYTES: usize = 124;
pub const BOX_SUBGAME_SEQ_DRAC_REPERTOIRES: usize = 2;
pub const BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_CODE_UNITS: usize = 112;
pub const BOX_SUBGAME_SEQ_DRAC_KANA_CANDIDATES_IN_MESSAGES: usize = 24;
pub const BOX_SUBGAME_SEQ_DRAC_KANA_CANDIDATES_IN_REPERTOIRES: usize = 2;
pub const BOX_SUBGAME_SEQ_DRAC_MISSED_MESSAGE_SPANS: usize = 2;

/// 지원 원본에서 생성한 12행 TSV의 SHA-256.
pub const BOX_SUBGAME_SEQ_DRAC_TEXT_REPORT_SHA256: [u8; 32] = [
    0x9F, 0xD8, 0xA9, 0x33, 0x6F, 0x33, 0x48, 0x73, 0x17, 0xC5, 0xAA, 0x40, 0x63, 0xC9, 0x51, 0xC6,
    0x95, 0x7E, 0xDF, 0x04, 0xB2, 0x48, 0x4D, 0xFE, 0x78, 0x04, 0x7E, 0x25, 0x60, 0xA4, 0xB5, 0x7A,
];

const LOAD_BASE: u32 = 0x801B_5270;
const REPERTOIRE_BYTES: usize = 112;
const REPERTOIRE_SEPARATOR: [u8; 2] = [0x65, 0x65];

const MESSAGE_1: &[&str] = &["お前っ！"];
const MESSAGE_2: &[&str] = &["ここを通りたければ", "あたしと勝負してい", "きなさい！"];
const MESSAGE_3: &[&str] = &["勝負？", "ま、まさか", "美少女コンテスト！？"];
const MESSAGE_4: &[&str] = &["ふふん、", "あたしの美しさに", "かなうとおもって・・・"];
const MESSAGE_5: &[&str] = &["じゃなあーいっ！！", "勝負は", "ぷよぷよ地獄よっ！"];
const EXPECTED_MESSAGE_SPANS: [&[&str]; 5] =
    [MESSAGE_1, MESSAGE_2, MESSAGE_3, MESSAGE_4, MESSAGE_5];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DracTextSpec {
    scope: &'static str,
    archive: &'static str,
    sequence: &'static str,
    repertoire_offset: usize,
    pointer_fields: [usize; 5],
    message_offsets: [usize; 5],
    message_end_offset: usize,
    tail_offset: usize,
}

const SPECS: [DracTextSpec; 2] = [
    DracTextSpec {
        scope: "PUYO1",
        archive: "/PUYO1/PP1_SEQ.VDD;1",
        sequence: "DRAC_SEQ.SEQ",
        repertoire_offset: 0x1AAC,
        pointer_fields: [0x0D0C, 0x0D28, 0x0D48, 0x0D68, 0x0D84],
        message_offsets: [0x1B1E, 0x1B2C, 0x1B68, 0x1B9A, 0x1BD6],
        message_end_offset: 0x1C0E,
        tail_offset: 0x1C2C,
    },
    DracTextSpec {
        scope: "PUYO2",
        archive: "/PUYO2/PP2_SEQ.VDD;1",
        sequence: "DRAC_SEQ.SEQ",
        repertoire_offset: 0x1A18,
        pointer_fields: [0x0C80, 0x0C9C, 0x0CBC, 0x0CDC, 0x0CF8],
        message_offsets: [0x1A8A, 0x1A98, 0x1AD4, 0x1B06, 0x1B42],
        message_end_offset: 0x1B7A,
        tail_offset: 0x1B9C,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubgameSeqDracTextKind {
    Repertoire,
    Message,
}

impl SubgameSeqDracTextKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Repertoire => "repertoire",
            Self::Message => "message",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracSjisSpan {
    pub offset: usize,
    pub raw: Vec<u8>,
    pub decoded: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracTextRow {
    pub scope: &'static str,
    pub archive: &'static str,
    pub sequence: &'static str,
    pub kind: SubgameSeqDracTextKind,
    pub index: usize,
    pub pointer_field_offset: usize,
    pub pointer_field_runtime: u32,
    pub record_offset: usize,
    pub record_runtime: u32,
    pub raw: Vec<u8>,
    pub raw_sha256: [u8; 32],
    pub spans: Vec<SubgameSeqDracSjisSpan>,
    pub protected_bytes: usize,
    pub kana_candidate_overlaps: usize,
    pub missed_sjis_spans: usize,
}

impl SubgameSeqDracTextRow {
    pub fn code_units(&self) -> usize {
        self.spans
            .iter()
            .map(|span| span.decoded.chars().count())
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracTextAnalysis {
    pub rows: Vec<SubgameSeqDracTextRow>,
    pub message_records: usize,
    pub message_spans: usize,
    pub message_code_units: usize,
    pub message_protected_bytes: usize,
    pub repertoires: usize,
    pub repertoire_code_units: usize,
    pub kana_candidates_in_messages: usize,
    pub kana_candidates_in_repertoires: usize,
    pub missed_message_spans: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracTextError(String);

impl fmt::Display for SubgameSeqDracTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqDracTextError {}

fn error(message: impl Into<String>) -> SubgameSeqDracTextError {
    SubgameSeqDracTextError(message.into())
}

fn is_sjis_lead(byte: u8) -> bool {
    matches!(byte, 0x81..=0x9F | 0xE0..=0xEF)
}

fn scan_sjis_spans(raw: &[u8], base_offset: usize) -> Vec<SubgameSeqDracSjisSpan> {
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    while cursor + 1 < raw.len() {
        if !is_sjis_lead(raw[cursor]) || decode_sjis(&raw[cursor..cursor + 2]).is_err() {
            cursor += 1;
            continue;
        }
        let start = cursor;
        cursor += 2;
        while cursor + 1 < raw.len()
            && is_sjis_lead(raw[cursor])
            && decode_sjis(&raw[cursor..cursor + 2]).is_ok()
        {
            cursor += 2;
        }
        let span_raw = raw[start..cursor].to_vec();
        let decoded = decode_sjis(&span_raw).expect("개별 pair를 검증한 연속 span");
        spans.push(SubgameSeqDracSjisSpan {
            offset: base_offset + start,
            raw: span_raw,
            decoded,
        });
    }
    spans
}

fn read_u32(raw: &[u8], offset: usize) -> Result<u32, SubgameSeqDracTextError> {
    let bytes = raw
        .get(offset..offset + 4)
        .ok_or_else(|| error(format!("DRAC u32 범위 이탈: +0x{offset:04X}")))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 slice")))
}

fn candidate_overlaps(
    text: &SubgameSeqTextCandidateAnalysis,
    spec: DracTextSpec,
    start: usize,
    end: usize,
) -> usize {
    text.candidates
        .iter()
        .filter(|candidate| {
            candidate.archive == spec.archive
                && candidate.sequence == spec.sequence
                && candidate.offset >= start
                && candidate
                    .offset
                    .checked_add(candidate.raw.len())
                    .is_some_and(|candidate_end| candidate_end <= end)
        })
        .count()
}

fn missed_spans(
    text: &SubgameSeqTextCandidateAnalysis,
    spec: DracTextSpec,
    spans: &[SubgameSeqDracSjisSpan],
) -> usize {
    spans
        .iter()
        .filter(|span| {
            !text.candidates.iter().any(|candidate| {
                candidate.archive == spec.archive
                    && candidate.sequence == spec.sequence
                    && candidate.offset == span.offset
                    && candidate.raw == span.raw
            })
        })
        .count()
}

fn audit_subgame_seq_drac_text<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    text: &SubgameSeqTextCandidateAnalysis,
    layouts: &SubgameSeqLayoutAnalysis,
) -> Result<SubgameSeqDracTextAnalysis, SubgameSeqDracTextError> {
    let mut source_map = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        let key = (archive.to_string(), sequence.to_string());
        if source_map.insert(key.clone(), raw).is_some() {
            return Err(error(format!("DRAC text 입력 중복: {}:{}", key.0, key.1)));
        }
    }

    let mut rows = Vec::with_capacity(BOX_SUBGAME_SEQ_DRAC_TEXT_ROWS);
    let mut message_block_baseline: Option<Vec<u8>> = None;
    let mut repertoire_baseline: Option<Vec<u8>> = None;

    for spec in SPECS {
        let raw = source_map
            .get(&(spec.archive.to_string(), spec.sequence.to_string()))
            .ok_or_else(|| {
                error(format!(
                    "DRAC text 원본 누락: {}:{}",
                    spec.archive, spec.sequence
                ))
            })?;
        let layout = layouts
            .rows
            .iter()
            .find(|row| row.archive == spec.archive && row.sequence == spec.sequence)
            .ok_or_else(|| {
                error(format!(
                    "DRAC layout 누락: {}:{}",
                    spec.archive, spec.sequence
                ))
            })?;
        let footer = layout
            .footer
            .ok_or_else(|| error("DRAC overlay footer 누락"))?;
        let root = layout
            .root
            .ok_or_else(|| error("DRAC root evidence 누락"))?;
        if footer.load_base != LOAD_BASE
            || root.pointer_field_offset != 0x0B68
            || root.target_offset != spec.repertoire_offset
        {
            return Err(error(format!("{} DRAC layout/root 불일치", spec.scope)));
        }
        let map = footer
            .runtime_map()
            .ok_or_else(|| error("DRAC runtime 사상 overflow"))?;

        let repertoire_end = spec
            .repertoire_offset
            .checked_add(REPERTOIRE_BYTES)
            .ok_or_else(|| error("DRAC repertoire 끝 offset 초과"))?;
        let separator_end = repertoire_end
            .checked_add(REPERTOIRE_SEPARATOR.len())
            .ok_or_else(|| error("DRAC repertoire separator 끝 offset 초과"))?;
        if separator_end != spec.message_offsets[0]
            || raw.get(repertoire_end..separator_end) != Some(REPERTOIRE_SEPARATOR.as_slice())
        {
            return Err(error(format!(
                "{} DRAC repertoire separator 불일치",
                spec.scope
            )));
        }
        let repertoire_raw = raw
            .get(spec.repertoire_offset..repertoire_end)
            .ok_or_else(|| error("DRAC repertoire 범위 이탈"))?
            .to_vec();
        if let Some(baseline) = &repertoire_baseline {
            if baseline != &repertoire_raw {
                return Err(error("PUYO1/2 DRAC repertoire 바이트 불일치"));
            }
        } else {
            repertoire_baseline = Some(repertoire_raw.clone());
        }
        let repertoire_spans = scan_sjis_spans(&repertoire_raw, spec.repertoire_offset);
        if repertoire_spans.len() != 1
            || repertoire_spans[0].decoded.chars().count() != REPERTOIRE_BYTES / 2
        {
            return Err(error(format!(
                "{} DRAC repertoire decode 불일치",
                spec.scope
            )));
        }
        let repertoire_pointer_runtime = map
            .runtime_addr(root.pointer_field_offset)
            .ok_or_else(|| error("DRAC repertoire pointer field runtime 변환 실패"))?;
        let repertoire_runtime = map
            .runtime_addr(spec.repertoire_offset)
            .ok_or_else(|| error("DRAC repertoire runtime 변환 실패"))?;
        let repertoire_candidates =
            candidate_overlaps(text, spec, spec.repertoire_offset, repertoire_end);
        rows.push(SubgameSeqDracTextRow {
            scope: spec.scope,
            archive: spec.archive,
            sequence: spec.sequence,
            kind: SubgameSeqDracTextKind::Repertoire,
            index: 0,
            pointer_field_offset: root.pointer_field_offset,
            pointer_field_runtime: repertoire_pointer_runtime,
            record_offset: spec.repertoire_offset,
            record_runtime: repertoire_runtime,
            raw_sha256: Sha256::digest(&repertoire_raw).into(),
            raw: repertoire_raw,
            protected_bytes: 0,
            missed_sjis_spans: missed_spans(text, spec, &repertoire_spans),
            spans: repertoire_spans,
            kana_candidate_overlaps: repertoire_candidates,
        });

        let message_block = raw
            .get(spec.message_offsets[0]..spec.message_end_offset)
            .ok_or_else(|| error("DRAC message block 범위 이탈"))?
            .to_vec();
        if let Some(baseline) = &message_block_baseline {
            if baseline != &message_block {
                return Err(error("PUYO1/2 DRAC message block 바이트 불일치"));
            }
        } else {
            message_block_baseline = Some(message_block);
        }
        if raw
            .get(spec.message_end_offset..spec.tail_offset)
            .ok_or_else(|| error("DRAC text/tail padding 범위 이탈"))?
            .iter()
            .any(|&byte| byte != 0)
        {
            return Err(error(format!(
                "{} DRAC text/tail padding이 0이 아님",
                spec.scope
            )));
        }

        for (index, ((&pointer_field, &start), expected_spans)) in spec
            .pointer_fields
            .iter()
            .zip(&spec.message_offsets)
            .zip(EXPECTED_MESSAGE_SPANS)
            .enumerate()
        {
            let end = spec
                .message_offsets
                .get(index + 1)
                .copied()
                .unwrap_or(spec.message_end_offset);
            let pointer_runtime = read_u32(raw, pointer_field)?;
            let pointer_target = map.file_offset(pointer_runtime).ok_or_else(|| {
                error(format!(
                    "{} message {} pointer가 overlay 밖임",
                    spec.scope,
                    index + 1
                ))
            })?;
            if pointer_target != start {
                return Err(error(format!(
                    "{} message {} pointer target 불일치: expected=+0x{start:04X}, actual=+0x{pointer_target:04X}",
                    spec.scope,
                    index + 1,
                )));
            }
            let record_raw = raw
                .get(start..end)
                .ok_or_else(|| error("DRAC message record 범위 이탈"))?
                .to_vec();
            let spans = scan_sjis_spans(&record_raw, start);
            let actual_text = spans
                .iter()
                .map(|span| span.decoded.as_str())
                .collect::<Vec<_>>();
            if actual_text != expected_spans {
                return Err(error(format!(
                    "{} DRAC message {} text span 불일치: expected={:?}, actual={actual_text:?}",
                    spec.scope,
                    index + 1,
                    expected_spans,
                )));
            }
            let sjis_bytes = spans.iter().map(|span| span.raw.len()).sum::<usize>();
            let protected_bytes = record_raw
                .len()
                .checked_sub(sjis_bytes)
                .ok_or_else(|| error("DRAC message 보호 byte 계산 underflow"))?;
            let pointer_field_runtime = map
                .runtime_addr(pointer_field)
                .ok_or_else(|| error("DRAC message pointer field runtime 변환 실패"))?;
            let record_runtime = map
                .runtime_addr(start)
                .ok_or_else(|| error("DRAC message record runtime 변환 실패"))?;
            let overlaps = candidate_overlaps(text, spec, start, end);
            rows.push(SubgameSeqDracTextRow {
                scope: spec.scope,
                archive: spec.archive,
                sequence: spec.sequence,
                kind: SubgameSeqDracTextKind::Message,
                index: index + 1,
                pointer_field_offset: pointer_field,
                pointer_field_runtime,
                record_offset: start,
                record_runtime,
                raw_sha256: Sha256::digest(&record_raw).into(),
                raw: record_raw,
                protected_bytes,
                kana_candidate_overlaps: overlaps,
                missed_sjis_spans: missed_spans(text, spec, &spans),
                spans,
            });
        }
    }

    let message_rows = rows
        .iter()
        .filter(|row| row.kind == SubgameSeqDracTextKind::Message)
        .collect::<Vec<_>>();
    let repertoire_rows = rows
        .iter()
        .filter(|row| row.kind == SubgameSeqDracTextKind::Repertoire)
        .collect::<Vec<_>>();
    let message_characters = message_rows
        .iter()
        .flat_map(|row| row.spans.iter())
        .flat_map(|span| span.decoded.chars())
        .collect::<BTreeSet<_>>();
    for repertoire in &repertoire_rows {
        let repertoire_characters = repertoire.spans[0].decoded.chars().collect::<BTreeSet<_>>();
        if !message_characters.is_subset(&repertoire_characters) {
            return Err(error(format!(
                "{} DRAC message 문자가 repertoire에 없음",
                repertoire.scope
            )));
        }
    }

    Ok(SubgameSeqDracTextAnalysis {
        message_records: message_rows.len(),
        message_spans: message_rows.iter().map(|row| row.spans.len()).sum(),
        message_code_units: message_rows.iter().map(|row| row.code_units()).sum(),
        message_protected_bytes: message_rows.iter().map(|row| row.protected_bytes).sum(),
        repertoires: repertoire_rows.len(),
        repertoire_code_units: repertoire_rows.iter().map(|row| row.code_units()).sum(),
        kana_candidates_in_messages: message_rows
            .iter()
            .map(|row| row.kana_candidate_overlaps)
            .sum(),
        kana_candidates_in_repertoires: repertoire_rows
            .iter()
            .map(|row| row.kana_candidate_overlaps)
            .sum(),
        missed_message_spans: message_rows.iter().map(|row| row.missed_sjis_spans).sum(),
        rows,
    })
}

/// 지원 원본의 두 DRAC 사본에서 repertoire와 message record를 분리한다.
pub fn verify_box_subgame_seq_drac_text<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    text: &SubgameSeqTextCandidateAnalysis,
    layouts: &SubgameSeqLayoutAnalysis,
) -> Result<SubgameSeqDracTextAnalysis, SubgameSeqDracTextError> {
    let analysis = audit_subgame_seq_drac_text(sources, text, layouts)?;
    let actual = [
        analysis.rows.len(),
        analysis.message_records,
        analysis.message_spans,
        analysis.message_code_units,
        analysis.message_protected_bytes,
        analysis.repertoires,
        analysis.repertoire_code_units,
        analysis.kana_candidates_in_messages,
        analysis.kana_candidates_in_repertoires,
        analysis.missed_message_spans,
    ];
    let expected = [
        BOX_SUBGAME_SEQ_DRAC_TEXT_ROWS,
        BOX_SUBGAME_SEQ_DRAC_MESSAGE_RECORDS,
        BOX_SUBGAME_SEQ_DRAC_MESSAGE_SPANS,
        BOX_SUBGAME_SEQ_DRAC_MESSAGE_CODE_UNITS,
        BOX_SUBGAME_SEQ_DRAC_MESSAGE_PROTECTED_BYTES,
        BOX_SUBGAME_SEQ_DRAC_REPERTOIRES,
        BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_CODE_UNITS,
        BOX_SUBGAME_SEQ_DRAC_KANA_CANDIDATES_IN_MESSAGES,
        BOX_SUBGAME_SEQ_DRAC_KANA_CANDIDATES_IN_REPERTOIRES,
        BOX_SUBGAME_SEQ_DRAC_MISSED_MESSAGE_SPANS,
    ];
    if actual != expected {
        return Err(error(format!(
            "DRAC text 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }
    let report = render_subgame_seq_drac_text_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_DRAC_TEXT_REPORT_SHA256 {
        return Err(error(format!(
            "DRAC text report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_DRAC_TEXT_REPORT_SHA256),
            hex(&digest),
        )));
    }
    Ok(analysis)
}

pub fn render_subgame_seq_drac_text_tsv(analysis: &SubgameSeqDracTextAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tscope\tarchive\tsequence\tkind\tindex\tpointer_field_offset\t\
         pointer_field_runtime\trecord_offset\trecord_runtime\traw_bytes\traw_sha256\t\
         sjis_spans\tcode_units\tprotected_bytes\tkana_candidate_overlaps\t\
         missed_sjis_spans\tsource\tstatic_status\n",
    );
    for row in &analysis.rows {
        let source = row
            .spans
            .iter()
            .map(|span| span.decoded.as_str())
            .collect::<Vec<_>>()
            .join(" / ");
        let status = match row.kind {
            SubgameSeqDracTextKind::Repertoire => "structural-repertoire-not-translation",
            SubgameSeqDracTextKind::Message => "pointer-rooted-message-consumer-pending",
        };
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t0x{:08X}\t0x{:04X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            SUBGAME_SEQ_DRAC_TEXT_SCHEMA_VERSION,
            row.scope,
            row.archive,
            row.sequence,
            row.kind.label(),
            row.index,
            row.pointer_field_offset,
            row.pointer_field_runtime,
            row.record_offset,
            row.record_runtime,
            row.raw.len(),
            hex(&row.raw_sha256),
            row.spans.len(),
            row.code_units(),
            row.protected_bytes,
            row.kana_candidate_overlaps,
            row.missed_sjis_spans,
            source,
            status,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_drac_text_tests.rs"]
mod subgame_seq_drac_text_tests;
