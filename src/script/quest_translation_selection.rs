//! 독립 검토와 수동 조판 결과를 QUEST stream 선택으로 결합한다.
//!
//! `review-agrees`와 `review-proposes-change`는 검토자가 제시한 한국어를
//! 선택하고, `needs-human-decision`은 원문을 유지한다. 이 선택은 전수
//! 재삽입의 기술 입력을 만들기 위한 것이며 사람 승인이나 제품 적격으로
//! 승격하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::quest_translation_review::is_complete_korean_review_candidate;
use super::{QuestTranslationIndependentReviewStatus, QuestTranslationReviewAnalysis};

pub const QUEST_TRANSLATION_SELECTION_SCHEMA_VERSION: &str = "quest-translation-selection-v1";
/// 독립 검토와 수동 조판 상태에서 결정되는 비배포 선택 방식.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestTranslationSelectionDisposition {
    /// 독립 검토안으로 기술 재삽입을 시도한다.
    SelectReviewedKorean,
    /// 사람이 작성한 composition 문장으로 비배포 재삽입을 시도한다.
    SelectManualComposition,
    /// 명시적 수동 조판이 없으므로 지원 원본의 stream을 그대로 둔다.
    RetainSource,
}

impl QuestTranslationSelectionDisposition {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SelectReviewedKorean => "select-reviewed-korean",
            Self::SelectManualComposition => "select-manual-composition",
            Self::RetainSource => "retain-source",
        }
    }
}

/// 검토 단위 하나의 비배포 선택 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationSelectionEntry {
    pub stream_id: String,
    pub batch_id: String,
    pub review_status: QuestTranslationIndependentReviewStatus,
    pub disposition: QuestTranslationSelectionDisposition,
    pub protected_source: String,
    pub draft_korean: String,
    pub reviewed_korean: String,
    pub selected_korean: Option<String>,
    pub selected_hangul: String,
    pub decision_dependencies: Vec<String>,
    pub translation_approved: bool,
    pub product_eligible: bool,
}

/// 검증된 수동 composition에서 비배포 선택에 반영할 문장 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestManualCompositionSelection {
    pub stream_id: String,
    pub batch_id: String,
    pub selected_korean: String,
}

/// 580개 검토 단위의 선택과 기존 초벌 cache 차이.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationSelectionAnalysis {
    pub entries: Vec<QuestTranslationSelectionEntry>,
    pub selected_entries: usize,
    pub retained_source_entries: usize,
    /// 현재 선택문 중 원래 독립 검토가 동의였던 수. 수동 작성 축과 겹칠 수 있다.
    pub selected_entries_with_review_agreement: usize,
    /// 현재 선택문 중 원래 독립 검토가 수정 제안이었던 수. 수동 작성 축과 겹칠 수 있다.
    pub selected_entries_with_review_proposal: usize,
    /// 현재 선택문 중 수동 composition이 표시문을 소유하는 수.
    pub selected_manual_compositions: usize,
    pub selected_batches: usize,
    pub selected_hangul_repertoire: String,
    pub draft_hangul_repertoire: String,
    pub new_cache_glyphs: String,
    pub draft_only_glyphs: String,
    pub translation_approved_entries: usize,
    pub product_eligible_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationSelectionError(String);

impl fmt::Display for QuestTranslationSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestTranslationSelectionError {}

fn error(message: impl Into<String>) -> QuestTranslationSelectionError {
    QuestTranslationSelectionError(message.into())
}

fn is_hangul(character: char) -> bool {
    matches!(character, '\u{ac00}'..='\u{d7a3}')
}

fn hangul_repertoire(value: &str) -> BTreeSet<char> {
    value
        .chars()
        .filter(|character| is_hangul(*character))
        .collect()
}

fn disposition(
    status: QuestTranslationIndependentReviewStatus,
) -> QuestTranslationSelectionDisposition {
    match status {
        QuestTranslationIndependentReviewStatus::ReviewAgrees
        | QuestTranslationIndependentReviewStatus::ReviewProposesChange => {
            QuestTranslationSelectionDisposition::SelectReviewedKorean
        }
        QuestTranslationIndependentReviewStatus::NeedsHumanDecision => {
            QuestTranslationSelectionDisposition::RetainSource
        }
    }
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn summarize_entries(
    mut entries: Vec<QuestTranslationSelectionEntry>,
) -> QuestTranslationSelectionAnalysis {
    for entry in &mut entries {
        entry.selected_hangul = entry
            .selected_korean
            .as_deref()
            .map(hangul_repertoire)
            .unwrap_or_default()
            .into_iter()
            .collect();
    }

    let selected_entries = entries
        .iter()
        .filter(|entry| entry.selected_korean.is_some())
        .count();
    let retained_source_entries = entries.len() - selected_entries;
    let selected_entries_with_review_agreement = entries
        .iter()
        .filter(|entry| {
            entry.review_status == QuestTranslationIndependentReviewStatus::ReviewAgrees
                && entry.selected_korean.is_some()
        })
        .count();
    let selected_entries_with_review_proposal = entries
        .iter()
        .filter(|entry| {
            entry.review_status == QuestTranslationIndependentReviewStatus::ReviewProposesChange
                && entry.selected_korean.is_some()
        })
        .count();
    let selected_manual_compositions = entries
        .iter()
        .filter(|entry| {
            entry.disposition == QuestTranslationSelectionDisposition::SelectManualComposition
                && entry.selected_korean.is_some()
        })
        .count();
    let selected_batches = entries
        .iter()
        .filter(|entry| entry.selected_korean.is_some())
        .map(|entry| entry.batch_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let selected_hangul = entries
        .iter()
        .filter_map(|entry| entry.selected_korean.as_deref())
        .flat_map(hangul_repertoire)
        .collect::<BTreeSet<_>>();
    let draft_hangul = entries
        .iter()
        .flat_map(|entry| hangul_repertoire(&entry.draft_korean))
        .collect::<BTreeSet<_>>();
    let new_cache_glyphs = selected_hangul
        .difference(&draft_hangul)
        .copied()
        .collect::<String>();
    let draft_only_glyphs = draft_hangul
        .difference(&selected_hangul)
        .copied()
        .collect::<String>();
    let translation_approved_entries = entries
        .iter()
        .filter(|entry| entry.translation_approved)
        .count();
    let product_eligible_entries = entries
        .iter()
        .filter(|entry| entry.product_eligible)
        .count();

    QuestTranslationSelectionAnalysis {
        entries,
        selected_entries,
        retained_source_entries,
        selected_entries_with_review_agreement,
        selected_entries_with_review_proposal,
        selected_manual_compositions,
        selected_batches,
        selected_hangul_repertoire: selected_hangul.into_iter().collect(),
        draft_hangul_repertoire: draft_hangul.into_iter().collect(),
        new_cache_glyphs,
        draft_only_glyphs,
        translation_approved_entries,
        product_eligible_entries,
    }
}

/// 수동 composition 문장을 기존 비배포 선택에 덮고 파생 집계를 다시 만든다.
pub fn apply_quest_manual_composition_selections(
    base: &QuestTranslationSelectionAnalysis,
    selections: &[QuestManualCompositionSelection],
) -> Result<QuestTranslationSelectionAnalysis, QuestTranslationSelectionError> {
    let mut base_stream_ids = BTreeSet::new();
    for entry in &base.entries {
        if !base_stream_ids.insert(entry.stream_id.as_str()) {
            return Err(error(format!(
                "QUEST 수동 composition 기준 stream ID 중복: {}",
                entry.stream_id
            )));
        }
        if !matches!(
            (entry.disposition, entry.selected_korean.as_deref()),
            (
                QuestTranslationSelectionDisposition::SelectReviewedKorean,
                Some(_)
            ) | (QuestTranslationSelectionDisposition::RetainSource, None)
        ) || entry.translation_approved
            || entry.product_eligible
        {
            return Err(error(format!(
                "{}: QUEST 수동 composition 기준 선택이 비승인 기본 상태가 아님",
                entry.stream_id
            )));
        }
    }

    let mut entries = base.entries.clone();
    let mut selected_stream_ids = BTreeSet::new();
    for selection in selections {
        if !selected_stream_ids.insert(selection.stream_id.as_str()) {
            return Err(error(format!(
                "QUEST 수동 composition 선택 stream ID 중복: {}",
                selection.stream_id
            )));
        }
        let matching_entries = entries
            .iter()
            .filter(|entry| entry.stream_id == selection.stream_id)
            .count();
        if matching_entries != 1 {
            return Err(error(format!(
                "QUEST 수동 composition 대상 stream 분모 불일치: {}, matches={matching_entries}",
                selection.stream_id
            )));
        }
        let entry = entries
            .iter_mut()
            .find(|entry| entry.stream_id == selection.stream_id)
            .expect("정확히 한 수동 composition 대상 stream을 확인함");
        if entry.batch_id != selection.batch_id {
            return Err(error(format!(
                "{}: QUEST 수동 composition batch ID 불일치: expected={}, actual={}",
                selection.stream_id, entry.batch_id, selection.batch_id
            )));
        }
        if selection.selected_korean.contains(['{', '}'])
            || !is_complete_korean_review_candidate(
                &entry.protected_source,
                &selection.selected_korean,
            )
        {
            return Err(error(format!(
                "{}: QUEST 수동 composition 선택문이 불완전 한국어임",
                selection.stream_id
            )));
        }

        entry.disposition = QuestTranslationSelectionDisposition::SelectManualComposition;
        entry.selected_korean = Some(selection.selected_korean.clone());
        entry.translation_approved = false;
        entry.product_eligible = false;
    }

    let analysis = summarize_entries(entries);
    if analysis.selected_manual_compositions != selections.len()
        || analysis.translation_approved_entries != 0
        || analysis.product_eligible_entries != 0
    {
        return Err(error("QUEST 수동 composition 선택 최종 집계 drift"));
    }
    Ok(analysis)
}

/// 명시적 수동 composition만 development 삽입 대상으로 고른다.
///
/// 독립 검토문과 후보문은 번역·용량 검토 자료일 뿐 표시 조판이 아니다. 모든
/// stream을 먼저 원문 유지로 되돌린 뒤 사람이 작성한 composition만 고른다.
pub fn select_quest_manual_compositions_for_development(
    base: &QuestTranslationSelectionAnalysis,
    selections: &[QuestManualCompositionSelection],
) -> Result<QuestTranslationSelectionAnalysis, QuestTranslationSelectionError> {
    if base.translation_approved_entries != 0
        || base.product_eligible_entries != 0
        || base.selected_manual_compositions != 0
    {
        return Err(error(
            "QUEST 수동 development 선택의 기준 상태가 비승인 기본 검토안이 아님",
        ));
    }

    let mut stream_ids = BTreeSet::new();
    let mut entries = base.entries.clone();
    for entry in &mut entries {
        if !stream_ids.insert(entry.stream_id.as_str()) {
            return Err(error(format!(
                "QUEST 수동 development 선택 stream ID 중복: {}",
                entry.stream_id
            )));
        }
        match (entry.disposition, entry.selected_korean.as_deref()) {
            (QuestTranslationSelectionDisposition::SelectReviewedKorean, Some(_))
            | (QuestTranslationSelectionDisposition::RetainSource, None) => {}
            _ => {
                return Err(error(format!(
                    "{}: QUEST 수동 development 선택 기준 disposition 불일치",
                    entry.stream_id
                )));
            }
        }
        entry.disposition = QuestTranslationSelectionDisposition::RetainSource;
        entry.selected_korean = None;
        entry.translation_approved = false;
        entry.product_eligible = false;
    }

    let retained = summarize_entries(entries);
    apply_quest_manual_composition_selections(&retained, selections)
}

/// 독립 검토 결과를 승인과 분리된 전수 선택으로 바꾼다.
pub fn audit_quest_translation_selection(
    reviews: &QuestTranslationReviewAnalysis,
) -> Result<QuestTranslationSelectionAnalysis, QuestTranslationSelectionError> {
    if reviews.reviewed_entries != reviews.entries.len() || reviews.unreviewed_drafts != 0 {
        return Err(error("QUEST 번역 선택 입력에 미검토 번역이 남음"));
    }

    let mut stream_ids = BTreeSet::new();
    let mut entries = Vec::with_capacity(reviews.entries.len());
    for review in &reviews.entries {
        if !stream_ids.insert(review.batch_entry.stream_id.as_str()) {
            return Err(error(format!(
                "QUEST 번역 선택 stream ID 중복: {}",
                review.batch_entry.stream_id
            )));
        }
        let status = review.independent_review_status.ok_or_else(|| {
            error(format!(
                "{}: QUEST 번역 선택의 독립 검토 상태 누락",
                review.batch_entry.stream_id
            ))
        })?;
        let disposition = disposition(status);
        let selected_korean = match disposition {
            QuestTranslationSelectionDisposition::SelectReviewedKorean => {
                if !review.decision_dependencies.is_empty()
                    || review.reviewed_korean.contains(['{', '}'])
                    || !is_complete_korean_review_candidate(
                        &review.batch_entry.protected_source,
                        &review.reviewed_korean,
                    )
                {
                    return Err(error(format!(
                        "{}: QUEST 번역 선택문에 미해결 결정 또는 불완전 한국어가 있음",
                        review.batch_entry.stream_id
                    )));
                }
                Some(review.reviewed_korean.clone())
            }
            QuestTranslationSelectionDisposition::SelectManualComposition => {
                return Err(error("QUEST 기본 번역 선택에 manual composition이 들어옴"));
            }
            QuestTranslationSelectionDisposition::RetainSource => {
                if review.decision_dependencies.is_empty() {
                    return Err(error(format!(
                        "{}: 사람 결정 유지 행에 결정 의존성이 없음",
                        review.batch_entry.stream_id
                    )));
                }
                None
            }
        };
        let selected_hangul = selected_korean
            .as_deref()
            .map(hangul_repertoire)
            .unwrap_or_default()
            .into_iter()
            .collect::<String>();
        entries.push(QuestTranslationSelectionEntry {
            stream_id: review.batch_entry.stream_id.clone(),
            batch_id: review.batch_entry.batch_id.clone(),
            review_status: status,
            disposition,
            protected_source: review.batch_entry.protected_source.clone(),
            draft_korean: review.batch_entry.korean_draft.clone(),
            reviewed_korean: review.reviewed_korean.clone(),
            selected_korean,
            selected_hangul,
            decision_dependencies: review.decision_dependencies.clone(),
            translation_approved: false,
            product_eligible: false,
        });
    }
    entries.sort_by(|left, right| left.stream_id.cmp(&right.stream_id));

    let analysis = summarize_entries(entries);
    if analysis.selected_manual_compositions != 0 {
        return Err(error("QUEST 기본 번역 선택에 수동 composition이 들어옴"));
    }
    Ok(analysis)
}

/// 전수 선택을 사람이 원문과 검토안에 대조할 수 있는 TSV로 만든다.
pub fn render_quest_translation_selection_tsv(
    analysis: &QuestTranslationSelectionAnalysis,
) -> String {
    let mut output = String::from(
        "schema_version\tstream_id\tbatch_id\treview_status\tdisposition\tprotected_source\t\
         draft_ko\treview_ko\tselected_ko\tselected_hangul\tdecision_dependencies\t\
         translation_approved\tproduct_eligible\n",
    );
    for entry in &analysis.entries {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tno\tno",
            QUEST_TRANSLATION_SELECTION_SCHEMA_VERSION,
            entry.stream_id,
            entry.batch_id,
            entry.review_status.label(),
            entry.disposition.label(),
            escape_tsv(&entry.protected_source),
            escape_tsv(&entry.draft_korean),
            escape_tsv(&entry.reviewed_korean),
            entry
                .selected_korean
                .as_deref()
                .map(escape_tsv)
                .unwrap_or_default(),
            entry.selected_hangul,
            entry.decision_dependencies.join(","),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

/// 현재 독립 검토 결과에서 비배포 선택을 매번 다시 계산한다.
pub fn verify_box_quest_translation_selection(
    reviews: &QuestTranslationReviewAnalysis,
) -> Result<QuestTranslationSelectionAnalysis, QuestTranslationSelectionError> {
    audit_quest_translation_selection(reviews)
}

/// 현재 번역 선택 결과의 lineage digest.
pub fn quest_translation_selection_report_sha256(
    analysis: &QuestTranslationSelectionAnalysis,
) -> String {
    format!(
        "{:x}",
        Sha256::digest(render_quest_translation_selection_tsv(analysis).as_bytes())
    )
}

#[cfg(test)]
#[path = "quest_translation_selection_tests.rs"]
mod tests;
