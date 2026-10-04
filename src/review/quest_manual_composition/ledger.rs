//! 사람이 작성한 QUEST 수동 composition TSV의 구문과 스키마 검증.

use std::collections::BTreeSet;

use super::model::{
    composition_error, sha256_text, QuestCompositionCandidateBasis, QuestManualCompositionError,
    QuestManualCompositionSegment, QuestManualCompositionStatus,
    QUEST_MANUAL_COMPOSITION_SCHEMA_VERSION, QUEST_MANUAL_COMPOSITION_SEGMENT_SCHEMA_VERSION,
};

const COMPOSITIONS: &str =
    include_str!("../../../assets/translations/quest_manual_compositions.tsv");
const SEGMENTS: &str =
    include_str!("../../../assets/translations/quest_manual_composition_segments.tsv");

pub(super) const COMPOSITION_HEADER: &str = "schema_version\tcomposition_id\tbatch_id\tstream_id\tcandidate_basis\tselected_ko\tselected_text_sha256\tsource_stream_sha256\tgeometry_profile_id\tgeometry_sha256\tstatus\tnotes";
pub(super) const SEGMENT_HEADER: &str = "schema_version\tcomposition_id\tsegment_order\tinput_phase\tphase_line\tselected_text_start\tselected_text_end\ttext\tfollowing_control_token_index\tfollowing_control_code";

#[derive(Debug)]
pub(super) struct CompositionRow {
    pub(super) id: String,
    pub(super) batch_id: String,
    pub(super) stream_id: String,
    pub(super) candidate_basis: QuestCompositionCandidateBasis,
    pub(super) selected_korean: String,
    pub(super) selected_text_sha256: String,
    pub(super) source_stream_sha256: String,
    pub(super) geometry_profile_id: Option<String>,
    pub(super) geometry_sha256: Option<String>,
    pub(super) status: QuestManualCompositionStatus,
    pub(super) notes: String,
}

#[derive(Debug)]
pub(super) struct SegmentRow {
    pub(super) composition_id: String,
    pub(super) segment: QuestManualCompositionSegment,
}

#[derive(Debug)]
pub(super) struct QuestManualCompositionLedger {
    pub(super) compositions: Vec<CompositionRow>,
    pub(super) segments: Vec<SegmentRow>,
}

fn is_japanese(character: char) -> bool {
    matches!(
        character,
        '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'
    )
}

pub(super) fn contains_hangul(text: &str) -> bool {
    text.chars()
        .any(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
}

/// 한글 없는 대사는 원문과 같은 구두점일 때만 허용한다. 말줄임표 한 글자는
/// 원문의 중점 세 글자와 대응하며, 점 개수와 다른 기호는 바꾸지 않는다.
pub(super) fn canonical_source_punctuation(text: &str) -> Option<String> {
    let normalized = crate::font::normalize_quest_display_text(text).ok()?;
    let mut punctuation = String::new();
    for character in normalized.chars().filter(|c| !c.is_whitespace()) {
        if character == '…' {
            punctuation.push_str("・・・");
        } else if character.is_ascii_punctuation()
            || matches!(
                character,
                '・' | '。' | '、' | '！' | '？' | '：' | '〜' | '「' | '」' | '（' | '）' | '／'
            )
        {
            punctuation.push(character);
        } else {
            return None;
        }
    }
    (!punctuation.is_empty()).then_some(punctuation)
}

fn parse_sha256(value: &str, subject: &str) -> Result<String, QuestManualCompositionError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(composition_error(format!("{subject}: SHA-256 형식 오류")));
    }
    Ok(value.to_ascii_lowercase())
}

pub(super) fn parse_quest_manual_composition_rows(
    input: &str,
) -> Result<Vec<CompositionRow>, QuestManualCompositionError> {
    let mut lines = input.lines();
    if lines.next().unwrap_or_default() != COMPOSITION_HEADER {
        return Err(composition_error("QUEST 수동 composition header 불일치"));
    }

    let mut rows = Vec::new();
    let mut ids = BTreeSet::new();
    let mut stream_ids = BTreeSet::new();
    for (index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            return Err(composition_error(format!(
                "QUEST 수동 composition 빈 행: line={}",
                index + 2
            )));
        }
        let columns = line.split('\t').collect::<Vec<_>>();
        let [schema_version, composition_id, batch_id, stream_id, candidate_basis, selected_korean, selected_text_sha256, source_stream_sha256, geometry_profile_id, geometry_sha256, status, notes] =
            columns.as_slice()
        else {
            return Err(composition_error(format!(
                "QUEST 수동 composition 열 수 불일치: line={}",
                index + 2
            )));
        };
        if *schema_version != QUEST_MANUAL_COMPOSITION_SCHEMA_VERSION {
            return Err(composition_error(format!(
                "QUEST 수동 composition schema 불일치: {schema_version}"
            )));
        }
        if [composition_id, batch_id, stream_id, selected_korean, notes]
            .iter()
            .any(|value| value.is_empty())
        {
            return Err(composition_error(format!(
                "QUEST 수동 composition 필수값 누락: line={}",
                index + 2
            )));
        }
        if !ids.insert(*composition_id) {
            return Err(composition_error(format!(
                "QUEST 수동 composition ID 중복: {composition_id}"
            )));
        }
        if !stream_ids.insert(*stream_id) {
            return Err(composition_error(format!(
                "QUEST 수동 composition stream 중복: {stream_id}"
            )));
        }
        let candidate_basis =
            QuestCompositionCandidateBasis::parse(candidate_basis).ok_or_else(|| {
                composition_error(format!("{composition_id}: candidate basis 미지원"))
            })?;
        let status = QuestManualCompositionStatus::parse(status)
            .ok_or_else(|| composition_error(format!("{composition_id}: status 미지원")))?;
        if selected_korean.contains(['{', '}'])
            || selected_korean.chars().any(char::is_control)
            || selected_korean.chars().any(is_japanese)
            || (!contains_hangul(selected_korean)
                && canonical_source_punctuation(selected_korean).is_none())
        {
            return Err(composition_error(format!(
                "{composition_id}: 수동 선택문에 미지원 문자·placeholder 또는 일본어가 있음"
            )));
        }
        let actual_selected_text_sha256 = sha256_text(selected_korean);
        let selected_text_sha256 = parse_sha256(selected_text_sha256, composition_id)?;
        if actual_selected_text_sha256 != selected_text_sha256 {
            return Err(composition_error(format!(
                "{composition_id}: 수동 선택문 SHA-256 불일치"
            )));
        }
        let source_stream_sha256 = parse_sha256(source_stream_sha256, composition_id)?;
        let geometry_profile_id =
            (!geometry_profile_id.is_empty()).then(|| (*geometry_profile_id).to_owned());
        let geometry_sha256 = (!geometry_sha256.is_empty())
            .then(|| parse_sha256(geometry_sha256, composition_id))
            .transpose()?;
        if geometry_profile_id.is_some() != geometry_sha256.is_some() {
            return Err(composition_error(format!(
                "{composition_id}: geometry ID와 SHA-256은 함께 있어야 함"
            )));
        }
        if status == QuestManualCompositionStatus::ManualDraftNeedsGeometry
            && geometry_profile_id.is_some()
        {
            return Err(composition_error(format!(
                "{composition_id}: geometry 대기 상태에 geometry가 결속됨"
            )));
        }
        rows.push(CompositionRow {
            id: (*composition_id).to_owned(),
            batch_id: (*batch_id).to_owned(),
            stream_id: (*stream_id).to_owned(),
            candidate_basis,
            selected_korean: (*selected_korean).to_owned(),
            selected_text_sha256,
            source_stream_sha256,
            geometry_profile_id,
            geometry_sha256,
            status,
            notes: (*notes).to_owned(),
        });
    }
    if rows.is_empty() {
        return Err(composition_error("QUEST 수동 composition 원장이 비어 있음"));
    }
    Ok(rows)
}

fn parse_control_code(
    value: &str,
    composition_id: &str,
) -> Result<u8, QuestManualCompositionError> {
    let Some(hex) = value.strip_prefix("FF") else {
        return Err(composition_error(format!(
            "{composition_id}: following control 표기 오류: {value}"
        )));
    };
    u8::from_str_radix(hex, 16).map_err(|_| {
        composition_error(format!(
            "{composition_id}: following control 값 오류: {value}"
        ))
    })
}

fn parse_usize(
    value: &str,
    composition_id: &str,
    field: &str,
) -> Result<usize, QuestManualCompositionError> {
    value
        .parse::<usize>()
        .map_err(|_| composition_error(format!("{composition_id}: {field} 숫자 오류: {value}")))
}

fn parse_quest_manual_composition_segments(
    input: &str,
) -> Result<Vec<SegmentRow>, QuestManualCompositionError> {
    let mut lines = input.lines();
    if lines.next().unwrap_or_default() != SEGMENT_HEADER {
        return Err(composition_error(
            "QUEST 수동 composition segment header 불일치",
        ));
    }
    let mut rows = Vec::new();
    for (index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            return Err(composition_error(format!(
                "QUEST 수동 composition segment 빈 행: line={}",
                index + 2
            )));
        }
        let columns = line.split('\t').collect::<Vec<_>>();
        let [schema_version, composition_id, segment_order, input_phase, phase_line, text_start, text_end, text, following_control_token_index, following_control_code] =
            columns.as_slice()
        else {
            return Err(composition_error(format!(
                "QUEST 수동 composition segment 열 수 불일치: line={}",
                index + 2
            )));
        };
        if *schema_version != QUEST_MANUAL_COMPOSITION_SEGMENT_SCHEMA_VERSION {
            return Err(composition_error(format!(
                "QUEST 수동 composition segment schema 불일치: {schema_version}"
            )));
        }
        if composition_id.is_empty() || text.is_empty() || text.trim() != *text {
            return Err(composition_error(format!(
                "QUEST 수동 composition segment 필수값·바깥 공백 오류: line={}",
                index + 2
            )));
        }
        let order = parse_usize(segment_order, composition_id, "segment_order")?;
        let input_phase = parse_usize(input_phase, composition_id, "input_phase")?;
        let phase_line = parse_usize(phase_line, composition_id, "phase_line")?;
        if order == 0 || input_phase == 0 || phase_line == 0 {
            return Err(composition_error(format!(
                "{composition_id}: order/input_phase/phase_line은 1부터 시작해야 함"
            )));
        }
        rows.push(SegmentRow {
            composition_id: (*composition_id).to_owned(),
            segment: QuestManualCompositionSegment {
                order,
                input_phase,
                phase_line,
                selected_text_start: parse_usize(
                    text_start,
                    composition_id,
                    "selected_text_start",
                )?,
                selected_text_end: parse_usize(text_end, composition_id, "selected_text_end")?,
                text: (*text).to_owned(),
                following_control_token_index: parse_usize(
                    following_control_token_index,
                    composition_id,
                    "following_control_token_index",
                )?,
                following_control_code: parse_control_code(following_control_code, composition_id)?,
            },
        });
    }
    if rows.is_empty() {
        return Err(composition_error(
            "QUEST 수동 composition segment 원장이 비어 있음",
        ));
    }
    Ok(rows)
}

pub(super) fn parse_quest_manual_composition_ledger(
    compositions_input: &str,
    segments_input: &str,
) -> Result<QuestManualCompositionLedger, QuestManualCompositionError> {
    Ok(QuestManualCompositionLedger {
        compositions: parse_quest_manual_composition_rows(compositions_input)?,
        segments: parse_quest_manual_composition_segments(segments_input)?,
    })
}

pub(super) fn read_versioned_quest_manual_composition_ledger(
) -> Result<QuestManualCompositionLedger, QuestManualCompositionError> {
    parse_quest_manual_composition_ledger(COMPOSITIONS, SEGMENTS)
}

pub(super) fn versioned_quest_manual_composition_batch_ids(
) -> Result<Vec<String>, QuestManualCompositionError> {
    let rows = parse_quest_manual_composition_rows(COMPOSITIONS)?;
    let mut seen = BTreeSet::new();
    let mut batch_ids = Vec::new();
    for row in rows {
        if seen.insert(row.batch_id.clone()) {
            batch_ids.push(row.batch_id);
        }
    }
    Ok(batch_ids)
}
