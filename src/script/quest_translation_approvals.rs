//! QUEST 공통 의존성과 번역문에 대한 명시적 사람 승인 입력.
//!
//! 용어 승인은 `quest_term_candidates.tsv`의 `approved_ko`가 소유한다.
//! 이 모듈은 전역 결정 자료에서 공통 의존성과 번역문만 가져와 각 행을
//! 현재 기준선 digest에 결속한다. 후보나 독립 검토 결과를 승인으로
//! 추론하지 않으며, version 관리 승인 TSV에 실제 행이 있어야만 승인으로
//! 센다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::quest_terminology::QuestTerminologyDecision;
use super::quest_translation_decisions::{
    render_quest_translation_batch_decisions_tsv, QuestTranslationDecisionWorkbook,
};
use super::quest_translation_review::is_complete_korean_review_candidate;

const APPROVALS: &str = include_str!("../../assets/translations/quest_approvals.tsv");
pub const QUEST_TRANSLATION_APPROVAL_COLUMNS: &str =
    "row_kind\tdecision_id\tbaseline_sha256\tselected_ko\tapproval_notes";

/// 승인 입력이 가리키는 사람 결정의 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuestTranslationApprovalKind {
    SharedDependency,
    TranslationEntry,
}

impl QuestTranslationApprovalKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SharedDependency => "shared-dependency-decision",
            Self::TranslationEntry => "translation-entry-decision",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "shared-dependency-decision" => Some(Self::SharedDependency),
            "translation-entry-decision" => Some(Self::TranslationEntry),
            _ => None,
        }
    }
}

/// 현재 결정 자료에 결속된 승인 대상 한 행.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationApprovalBaseline {
    pub kind: QuestTranslationApprovalKind,
    pub decision_id: String,
    pub baseline_sha256: String,
}

/// version 관리 TSV에서 사람이 명시적으로 승인한 값.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationApproval {
    pub kind: QuestTranslationApprovalKind,
    pub decision_id: String,
    pub baseline_sha256: String,
    pub selected_korean: String,
    pub notes: String,
}

/// 승인 입력과 현재 전역 결정 자료의 결합 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationApprovalAnalysis {
    pub baselines: Vec<QuestTranslationApprovalBaseline>,
    pub approvals: Vec<QuestTranslationApproval>,
    pub approved_dependencies: usize,
    pub approved_entries: usize,
    pub entry_approvals_waiting_for_terms: usize,
    pub entry_approvals_waiting_for_dependencies: usize,
    pub build_eligible_entries: usize,
    pub approved_hangul_repertoire: String,
    pub manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationApprovalError(String);

impl fmt::Display for QuestTranslationApprovalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestTranslationApprovalError {}

fn error(message: impl Into<String>) -> QuestTranslationApprovalError {
    QuestTranslationApprovalError(message.into())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn has_japanese_character(value: &str) -> bool {
    value.chars().any(|character| {
        matches!(
            character,
            '\u{3040}'..='\u{30ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{ff66}'..='\u{ff9f}'
        )
    })
}

#[derive(Debug)]
struct BaselineContext {
    baseline: QuestTranslationApprovalBaseline,
    protected_source: Option<String>,
    matched_term_ids: Vec<String>,
    dependency_ids: Vec<String>,
}

fn approval_baselines(
    workbook: &QuestTranslationDecisionWorkbook,
) -> Result<Vec<BaselineContext>, QuestTranslationApprovalError> {
    let mut baselines = Vec::with_capacity(workbook.dependencies.len() + workbook.entries);
    let mut keys = BTreeSet::new();

    for dependency in &workbook.dependencies {
        let canonical = format!(
            "{}\t{}\t{}\t{}\t{}",
            QuestTranslationApprovalKind::SharedDependency.label(),
            dependency.id,
            dependency.occurrences,
            dependency.batch_ids.join(","),
            dependency.stream_ids.join(","),
        );
        let key = (
            QuestTranslationApprovalKind::SharedDependency,
            dependency.id.clone(),
        );
        if !keys.insert(key) {
            return Err(error(format!(
                "QUEST 승인 기준선의 공통 의존성 ID가 중복됨: {}",
                dependency.id
            )));
        }
        baselines.push(BaselineContext {
            baseline: QuestTranslationApprovalBaseline {
                kind: QuestTranslationApprovalKind::SharedDependency,
                decision_id: dependency.id.clone(),
                baseline_sha256: sha256(canonical.as_bytes()),
            },
            protected_source: None,
            matched_term_ids: Vec::new(),
            dependency_ids: Vec::new(),
        });
    }

    for batch in &workbook.batch_decisions {
        let batch_report_sha256 =
            sha256(render_quest_translation_batch_decisions_tsv(batch).as_bytes());
        for entry in &batch.entries {
            let stream_id = &entry.review.batch_entry.stream_id;
            let canonical = format!(
                "{}\t{}\t{}\t{}\t{}",
                QuestTranslationApprovalKind::TranslationEntry.label(),
                batch.batch_id,
                batch.review_asset_sha256,
                stream_id,
                batch_report_sha256,
            );
            let key = (
                QuestTranslationApprovalKind::TranslationEntry,
                stream_id.clone(),
            );
            if !keys.insert(key) {
                return Err(error(format!(
                    "QUEST 승인 기준선의 번역 stream ID가 중복됨: {stream_id}"
                )));
            }
            baselines.push(BaselineContext {
                baseline: QuestTranslationApprovalBaseline {
                    kind: QuestTranslationApprovalKind::TranslationEntry,
                    decision_id: stream_id.clone(),
                    baseline_sha256: sha256(canonical.as_bytes()),
                },
                protected_source: Some(entry.review.batch_entry.protected_source.clone()),
                matched_term_ids: entry.terms.iter().map(|term| term.id.clone()).collect(),
                dependency_ids: entry.review.decision_dependencies.clone(),
            });
        }
    }

    Ok(baselines)
}

fn parse_approvals(
    input: &str,
) -> Result<Vec<QuestTranslationApproval>, QuestTranslationApprovalError> {
    if !input.ends_with('\n') || input.contains('\r') {
        return Err(error("QUEST 승인 TSV는 LF 하나로 끝나고 CR이 없어야 함"));
    }
    let mut lines = input.lines();
    if lines.next() != Some(QUEST_TRANSLATION_APPROVAL_COLUMNS) {
        return Err(error("QUEST 승인 TSV header 불일치"));
    }

    let mut approvals = Vec::new();
    let mut keys = BTreeSet::new();
    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        if line.is_empty() {
            return Err(error(format!("QUEST 승인 TSV {line_number}행이 비어 있음")));
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        let [kind, decision_id, baseline_sha256, selected_korean, notes] = fields.as_slice() else {
            return Err(error(format!(
                "QUEST 승인 TSV {line_number}행 열 수 불일치"
            )));
        };
        let kind = QuestTranslationApprovalKind::parse(kind)
            .ok_or_else(|| error(format!("QUEST 승인 TSV {line_number}행 row kind 오류")))?;
        if decision_id.is_empty()
            || !is_lower_hex_digest(baseline_sha256)
            || selected_korean.is_empty()
            || notes.is_empty()
        {
            return Err(error(format!(
                "QUEST 승인 TSV {line_number}행 필수값 또는 digest 오류"
            )));
        }
        if has_japanese_character(selected_korean) {
            return Err(error(format!(
                "QUEST 승인 TSV {line_number}행 선택값에 일본어가 남음"
            )));
        }
        let key = (kind, (*decision_id).to_owned());
        if !keys.insert(key) {
            return Err(error(format!(
                "QUEST 승인 TSV {line_number}행 결정 ID 중복: {decision_id}"
            )));
        }
        approvals.push(QuestTranslationApproval {
            kind,
            decision_id: (*decision_id).to_owned(),
            baseline_sha256: (*baseline_sha256).to_owned(),
            selected_korean: (*selected_korean).to_owned(),
            notes: (*notes).to_owned(),
        });
    }
    Ok(approvals)
}

/// 현재 결정 자료와 호출자가 제공한 승인 TSV를 결합한다.
///
/// 검토 UI처럼 version 관리 승인 파일을 실행 중에 다시 읽어야 하는 경로가
/// `include_str!` 입력과 같은 검사를 재사용하도록 제공한다.
pub fn audit_quest_translation_approval_manifest(
    workbook: &QuestTranslationDecisionWorkbook,
    input: &str,
) -> Result<QuestTranslationApprovalAnalysis, QuestTranslationApprovalError> {
    let baseline_contexts = approval_baselines(workbook)?;
    let baseline_by_key = baseline_contexts
        .iter()
        .map(|context| {
            (
                (context.baseline.kind, context.baseline.decision_id.as_str()),
                context,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut approvals = parse_approvals(input)?;

    for approval in &approvals {
        let key = (approval.kind, approval.decision_id.as_str());
        let baseline = baseline_by_key.get(&key).ok_or_else(|| {
            error(format!(
                "QUEST 승인 TSV가 현재 결정 자료에 없는 행을 가리킴: {}:{}",
                approval.kind.label(),
                approval.decision_id
            ))
        })?;
        if approval.baseline_sha256 != baseline.baseline.baseline_sha256 {
            return Err(error(format!(
                "QUEST 승인 기준선 drift: {}:{}",
                approval.kind.label(),
                approval.decision_id
            )));
        }
        if approval.kind == QuestTranslationApprovalKind::TranslationEntry {
            let protected_source = baseline
                .protected_source
                .as_deref()
                .expect("번역 승인 기준선에는 보호 원문이 있음");
            if approval.selected_korean.contains(['{', '}']) {
                return Err(error(format!(
                    "{}: 승인 번역문에 미해결 placeholder가 남음",
                    approval.decision_id
                )));
            }
            if !is_complete_korean_review_candidate(protected_source, &approval.selected_korean) {
                return Err(error(format!(
                    "{}: 승인 번역문이 한국어 완성 후보가 아님",
                    approval.decision_id
                )));
            }
        }
    }

    approvals.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then(left.decision_id.cmp(&right.decision_id))
    });
    let approved_dependency_ids = approvals
        .iter()
        .filter(|approval| approval.kind == QuestTranslationApprovalKind::SharedDependency)
        .map(|approval| approval.decision_id.as_str())
        .collect::<BTreeSet<_>>();
    let approved_terms = workbook
        .terms
        .iter()
        .filter(|term| term.decision == QuestTerminologyDecision::HumanApproved)
        .map(|term| term.id.as_str())
        .collect::<BTreeSet<_>>();
    let approved_dependencies = approved_dependency_ids.len();
    let approved_entries = approvals
        .iter()
        .filter(|approval| approval.kind == QuestTranslationApprovalKind::TranslationEntry)
        .count();
    let mut entry_approvals_waiting_for_terms = 0;
    let mut entry_approvals_waiting_for_dependencies = 0;
    let mut build_eligible_entries = 0;
    for approval in approvals
        .iter()
        .filter(|approval| approval.kind == QuestTranslationApprovalKind::TranslationEntry)
    {
        let baseline = baseline_by_key
            .get(&(approval.kind, approval.decision_id.as_str()))
            .expect("검증을 통과한 승인 행은 기준선에 존재함");
        let terms_ready = baseline
            .matched_term_ids
            .iter()
            .all(|term_id| approved_terms.contains(term_id.as_str()));
        let dependencies_ready = baseline
            .dependency_ids
            .iter()
            .all(|dependency_id| approved_dependency_ids.contains(dependency_id.as_str()));
        entry_approvals_waiting_for_terms += usize::from(!terms_ready);
        entry_approvals_waiting_for_dependencies += usize::from(!dependencies_ready);
        build_eligible_entries += usize::from(terms_ready && dependencies_ready);
    }
    let approved_hangul_repertoire = approvals
        .iter()
        .filter(|approval| approval.kind == QuestTranslationApprovalKind::TranslationEntry)
        .flat_map(|approval| approval.selected_korean.chars())
        .filter(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<String>();

    Ok(QuestTranslationApprovalAnalysis {
        baselines: baseline_contexts
            .into_iter()
            .map(|context| context.baseline)
            .collect(),
        approvals,
        approved_dependencies,
        approved_entries,
        entry_approvals_waiting_for_terms,
        entry_approvals_waiting_for_dependencies,
        build_eligible_entries,
        approved_hangul_repertoire,
        manifest_sha256: sha256(input.as_bytes()),
    })
}

/// 현재 결정 자료와 version 관리 사람 승인 입력을 결합한다.
pub fn audit_quest_translation_approvals(
    workbook: &QuestTranslationDecisionWorkbook,
) -> Result<QuestTranslationApprovalAnalysis, QuestTranslationApprovalError> {
    audit_quest_translation_approval_manifest(workbook, APPROVALS)
}

/// 사람이 승인 TSV에 복사할 결정 ID와 현재 기준선 digest를 만든다.
pub fn render_quest_translation_approval_template_tsv(
    analysis: &QuestTranslationApprovalAnalysis,
) -> String {
    let mut output = format!("{QUEST_TRANSLATION_APPROVAL_COLUMNS}\n");
    for baseline in &analysis.baselines {
        writeln!(
            output,
            "{}\t{}\t{}\t\t",
            baseline.kind.label(),
            baseline.decision_id,
            baseline.baseline_sha256,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

/// 검증된 승인 행 전체를 안정 순서의 TSV로 직렬화한다.
///
/// 호출자는 결과 전체를 [`audit_quest_translation_approval_manifest`]로 다시
/// 검사한 뒤에만 저장해야 한다.
pub fn render_quest_translation_approval_manifest_tsv(
    approvals: &[QuestTranslationApproval],
) -> String {
    let mut approvals = approvals.to_vec();
    approvals.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then(left.decision_id.cmp(&right.decision_id))
    });
    let mut output = format!("{QUEST_TRANSLATION_APPROVAL_COLUMNS}\n");
    for approval in approvals {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}",
            approval.kind.label(),
            approval.decision_id,
            approval.baseline_sha256,
            approval.selected_korean,
            approval.notes,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

/// 현재 결정 자료와 version 관리 승인 입력을 매번 다시 검증한다.
pub fn verify_box_quest_translation_approvals(
    workbook: &QuestTranslationDecisionWorkbook,
) -> Result<QuestTranslationApprovalAnalysis, QuestTranslationApprovalError> {
    audit_quest_translation_approvals(workbook)
}

#[cfg(test)]
#[path = "quest_translation_approvals_tests.rs"]
mod tests;
