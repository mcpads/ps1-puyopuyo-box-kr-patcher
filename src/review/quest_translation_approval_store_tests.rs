use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::script::{
    audit_quest_translation_approval_manifest, QuestSeqTranslationUnitPolicy,
    QuestTranslationBatchEntry, QuestTranslationBatchRole, QuestTranslationContextConfidence,
    QuestTranslationDecisionAnalysis, QuestTranslationDecisionDependency,
    QuestTranslationDecisionEntry, QuestTranslationIndependentReviewStatus,
    QuestTranslationReviewAction, QuestTranslationReviewEntry, QuestTranslationReviewRequirement,
    QuestTranslationWorkStatus, QUEST_TRANSLATION_APPROVAL_COLUMNS,
};

static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn create() -> Self {
        let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ps1-puyopuyo-box-quest-approval-store-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn approval_path(&self) -> PathBuf {
        self.0.join("quest_approvals.tsv")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn workbook() -> QuestTranslationDecisionWorkbook {
    let batch_entry = QuestTranslationBatchEntry {
        batch_id: "scene-a".to_owned(),
        batch_context_sha256: "batch-context".to_owned(),
        scene_id: "scene".to_owned(),
        scene_title: "장면".to_owned(),
        sequence_context: "장면 문맥".to_owned(),
        source_evidence: "원본 근거".to_owned(),
        scene_order: 1,
        branch: "shared".to_owned(),
        stream_id: "stream-a".to_owned(),
        role: QuestTranslationBatchRole::Translate,
        speaker_id: "arle".to_owned(),
        speaker: "아르르".to_owned(),
        addressee_id: "other".to_owned(),
        addressee: "상대".to_owned(),
        confidence: QuestTranslationContextConfidence::Confirmed,
        context_note: "확정 문맥".to_owned(),
        protected_raw_hex: "0000".to_owned(),
        protected_source: "アルル「やった！".to_owned(),
        translation_policy: QuestSeqTranslationUnitPolicy::JapaneseTranslationCandidate,
        control_signature: "end".to_owned(),
        runtime_evidence: None,
        matched_term_ids: Vec::new(),
        matched_term_decisions: Vec::new(),
        korean_draft: "아르르「해냈다!".to_owned(),
        work_status: QuestTranslationWorkStatus::DraftNeedsIndependentReview,
        work_notes: "초벌".to_owned(),
    };
    let review = QuestTranslationReviewEntry {
        batch_entry,
        requirement: QuestTranslationReviewRequirement::HumanContextDecision,
        independent_review_status: Some(
            QuestTranslationIndependentReviewStatus::NeedsHumanDecision,
        ),
        reviewed_korean: "아르르「해냈어!".to_owned(),
        decision_dependencies: vec!["quest.style.arle".to_owned()],
        review_notes: "말투 선택 필요".to_owned(),
        review_asset_sha256: Some("review-asset".to_owned()),
    };
    let batch = QuestTranslationDecisionAnalysis {
        batch_id: "scene-a".to_owned(),
        review_asset_sha256: "review-asset".to_owned(),
        translation_batch_report_sha256: "translation-batch".to_owned(),
        terminology_report_sha256: "terminology".to_owned(),
        entries: vec![QuestTranslationDecisionEntry {
            review,
            terms: Vec::new(),
            review_action: QuestTranslationReviewAction::ResolveDependenciesAndApprove,
        }],
        affected_terms: 0,
        review_agreements: 0,
        review_proposals: 0,
        review_human_decisions: 1,
        pending_term_decisions: 0,
        conflicting_term_decisions: 0,
        approved_term_decisions: 0,
        pending_human_decisions: 1,
        build_eligible_entries: 0,
    };
    QuestTranslationDecisionWorkbook {
        batch_decisions: vec![batch],
        terms: Vec::new(),
        dependencies: vec![QuestTranslationDecisionDependency {
            id: "quest.style.arle".to_owned(),
            batch_ids: vec!["scene-a".to_owned()],
            stream_ids: vec!["stream-a".to_owned()],
            occurrences: 1,
        }],
        entries: 1,
        review_agreements: 0,
        review_proposals: 0,
        review_human_decisions: 1,
        pending_term_decisions: 0,
        conflicting_term_decisions: 0,
        approved_term_decisions: 0,
        dependency_occurrences: 1,
        build_eligible_entries: 0,
    }
}

fn blank_manifest() -> String {
    format!("{QUEST_TRANSLATION_APPROVAL_COLUMNS}\n")
}

fn selection(
    workbook: &QuestTranslationDecisionWorkbook,
    kind: QuestTranslationApprovalKind,
    decision_id: &str,
    selected_korean: &str,
) -> QuestApprovalSelection {
    let analysis = audit_quest_translation_approval_manifest(workbook, &blank_manifest()).unwrap();
    let baseline = analysis
        .baselines
        .iter()
        .find(|baseline| baseline.kind == kind && baseline.decision_id == decision_id)
        .unwrap();
    QuestApprovalSelection {
        kind,
        decision_id: decision_id.to_owned(),
        baseline_sha256: baseline.baseline_sha256.clone(),
        selected_korean: selected_korean.to_owned(),
        notes: "원문과 두 후보를 사람이 대조함".to_owned(),
    }
}

fn manifest_sha256(workbook: &QuestTranslationDecisionWorkbook, manifest: &str) -> String {
    audit_quest_translation_approval_manifest(workbook, manifest)
        .unwrap()
        .manifest_sha256
}

fn write_blank_manifest(path: &Path) -> String {
    let manifest = blank_manifest();
    fs::write(path, &manifest).unwrap();
    manifest
}

#[test]
fn 초벌_검토안_직접수정은_같은_domain_검사를_거쳐_선택된다() {
    let workbook = workbook();
    let mut manifest = blank_manifest();
    for selected in ["아르르「해냈다!", "아르르「해냈어!", "아르르「좋았어!"] {
        let digest = manifest_sha256(&workbook, &manifest);
        let update = apply_quest_translation_approval(
            &workbook,
            &manifest,
            &digest,
            selection(
                &workbook,
                QuestTranslationApprovalKind::TranslationEntry,
                "stream-a",
                selected,
            ),
        )
        .unwrap();
        assert_eq!(update.analysis.approvals[0].selected_korean, selected);
        manifest = update.manifest;
    }
}

#[test]
fn 저장한_선택은_원장에서_다시_읽히고_미승인의존성을_별도로_남긴다() {
    let directory = TestDirectory::create();
    let path = directory.approval_path();
    let workbook = workbook();
    let blank = write_blank_manifest(&path);
    let digest = manifest_sha256(&workbook, &blank);

    let saved = save_quest_translation_approval(
        &path,
        &workbook,
        &digest,
        selection(
            &workbook,
            QuestTranslationApprovalKind::TranslationEntry,
            "stream-a",
            "아르르「해냈어!",
        ),
    )
    .unwrap();

    assert_eq!(saved.approved_entries, 1);
    assert_eq!(saved.entry_approvals_waiting_for_dependencies, 1);
    assert_eq!(saved.build_eligible_entries, 0);
    let reloaded =
        audit_quest_translation_approval_manifest(&workbook, &fs::read_to_string(&path).unwrap())
            .unwrap();
    assert_eq!(reloaded.approvals, saved.approvals);
}

#[test]
fn 같은_원장_digest를_쓴_두번째_writer는_기존_byte를_보존하고_실패한다() {
    let directory = TestDirectory::create();
    let path = directory.approval_path();
    let workbook = workbook();
    let blank = write_blank_manifest(&path);
    let digest = manifest_sha256(&workbook, &blank);
    save_quest_translation_approval(
        &path,
        &workbook,
        &digest,
        selection(
            &workbook,
            QuestTranslationApprovalKind::TranslationEntry,
            "stream-a",
            "아르르「해냈다!",
        ),
    )
    .unwrap();
    let first_writer_bytes = fs::read(&path).unwrap();

    let error = save_quest_translation_approval(
        &path,
        &workbook,
        &digest,
        selection(
            &workbook,
            QuestTranslationApprovalKind::SharedDependency,
            "quest.style.arle",
            "활달한 반말을 유지한다",
        ),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        QuestApprovalStoreError::ConcurrentChange { .. }
    ));
    assert_eq!(fs::read(&path).unwrap(), first_writer_bytes);
}

#[test]
fn 없는_id_kind불일치와_stale_baseline은_구분해_거부한다() {
    let workbook = workbook();
    let manifest = blank_manifest();
    let digest = manifest_sha256(&workbook, &manifest);
    let mut missing = selection(
        &workbook,
        QuestTranslationApprovalKind::TranslationEntry,
        "stream-a",
        "아르르「해냈다!",
    );
    missing.decision_id = "missing".to_owned();
    assert!(matches!(
        apply_quest_translation_approval(&workbook, &manifest, &digest, missing),
        Err(QuestApprovalStoreError::MissingDecision { .. })
    ));

    let mut wrong_kind = selection(
        &workbook,
        QuestTranslationApprovalKind::TranslationEntry,
        "stream-a",
        "아르르「해냈다!",
    );
    wrong_kind.kind = QuestTranslationApprovalKind::SharedDependency;
    assert!(matches!(
        apply_quest_translation_approval(&workbook, &manifest, &digest, wrong_kind),
        Err(QuestApprovalStoreError::DecisionKindMismatch { .. })
    ));

    let mut stale = selection(
        &workbook,
        QuestTranslationApprovalKind::TranslationEntry,
        "stream-a",
        "아르르「해냈다!",
    );
    stale.baseline_sha256 = "0".repeat(64);
    assert!(matches!(
        apply_quest_translation_approval(&workbook, &manifest, &digest, stale),
        Err(QuestApprovalStoreError::BaselineDrift { .. })
    ));
}

#[test]
fn 유효하지_않은_번역문은_파일을_바꾸지_않는다() {
    let directory = TestDirectory::create();
    let path = directory.approval_path();
    let workbook = workbook();
    let blank = write_blank_manifest(&path);
    let digest = manifest_sha256(&workbook, &blank);

    for invalid in ["アルル「해냈다!", "{quest.character.arle}「해냈다!"] {
        let error = save_quest_translation_approval(
            &path,
            &workbook,
            &digest,
            selection(
                &workbook,
                QuestTranslationApprovalKind::TranslationEntry,
                "stream-a",
                invalid,
            ),
        )
        .unwrap_err();
        assert!(matches!(error, QuestApprovalStoreError::InvalidManifest(_)));
        assert_eq!(fs::read_to_string(&path).unwrap(), blank);
    }
}

#[test]
fn lock이_있으면_저장을_시작하지_않는다() {
    let directory = TestDirectory::create();
    let path = directory.approval_path();
    let workbook = workbook();
    let blank = write_blank_manifest(&path);
    let digest = manifest_sha256(&workbook, &blank);
    fs::write(path.with_extension("tsv.lock"), "writer\n").unwrap();

    let error = save_quest_translation_approval(
        &path,
        &workbook,
        &digest,
        selection(
            &workbook,
            QuestTranslationApprovalKind::TranslationEntry,
            "stream-a",
            "아르르「해냈다!",
        ),
    )
    .unwrap_err();

    assert!(matches!(error, QuestApprovalStoreError::Locked(_)));
    assert_eq!(fs::read_to_string(&path).unwrap(), blank);
}
