//! QUEST 번역 선택을 현재 승인 기준선에 결속하고 원자적으로 저장한다.
//!
//! browser나 HTTP는 이 모듈을 우회해 승인 TSV를 만들 수 없다. 기존 원장을
//! 먼저 전수 검증하고, 선택 한 건을 합친 전체 원장을 다시 검증한 뒤 같은
//! filesystem의 임시 파일을 원자적으로 교체한다.

use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::script::{
    audit_quest_translation_approval_manifest, render_quest_translation_approval_manifest_tsv,
    QuestTranslationApproval, QuestTranslationApprovalAnalysis, QuestTranslationApprovalKind,
    QuestTranslationDecisionWorkbook,
};

static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 사람이 한 승인 행에 명시적으로 남긴 선택.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestApprovalSelection {
    pub kind: QuestTranslationApprovalKind,
    pub decision_id: String,
    pub baseline_sha256: String,
    pub selected_korean: String,
    pub notes: String,
}

/// 저장 전에 전수 검증한 새 승인 원장과 파생 상태.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestApprovalManifestUpdate {
    pub manifest: String,
    pub analysis: QuestTranslationApprovalAnalysis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestApprovalStoreError {
    InvalidPath(String),
    Io {
        action: &'static str,
        path: PathBuf,
        message: String,
    },
    Locked(PathBuf),
    ConcurrentChange {
        expected_sha256: String,
        actual_sha256: String,
    },
    MissingDecision {
        kind: QuestTranslationApprovalKind,
        decision_id: String,
    },
    DecisionKindMismatch {
        expected: QuestTranslationApprovalKind,
        actual: QuestTranslationApprovalKind,
        decision_id: String,
    },
    BaselineDrift {
        decision_id: String,
        expected_sha256: String,
        actual_sha256: String,
    },
    InvalidManifest(String),
}

impl fmt::Display for QuestApprovalStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(message) => write!(f, "QUEST 승인 원장 경로 오류: {message}"),
            Self::Io {
                action,
                path,
                message,
            } => write!(
                f,
                "QUEST 승인 원장 {action} 실패 ({}): {message}",
                path.display()
            ),
            Self::Locked(path) => write!(
                f,
                "QUEST 승인 원장을 다른 writer가 저장 중임: {}",
                path.display()
            ),
            Self::ConcurrentChange {
                expected_sha256,
                actual_sha256,
            } => write!(
                f,
                "QUEST 승인 원장이 다른 process에서 변경됨: expected={expected_sha256}, actual={actual_sha256}; 다시 불러와야 함"
            ),
            Self::MissingDecision { kind, decision_id } => write!(
                f,
                "QUEST 승인 대상이 현재 결정 자료에 없음: {}:{decision_id}",
                kind.label()
            ),
            Self::DecisionKindMismatch {
                expected,
                actual,
                decision_id,
            } => write!(
                f,
                "QUEST 승인 대상 kind 불일치: {decision_id}, expected={}, actual={}",
                expected.label(),
                actual.label()
            ),
            Self::BaselineDrift {
                decision_id,
                expected_sha256,
                actual_sha256,
            } => write!(
                f,
                "QUEST 승인 기준선 drift: {decision_id}, expected={expected_sha256}, actual={actual_sha256}; 다시 불러와야 함"
            ),
            Self::InvalidManifest(message) => {
                write!(f, "QUEST 승인 원장 검증 실패: {message}")
            }
        }
    }
}

impl Error for QuestApprovalStoreError {}

fn invalid_manifest(error: impl fmt::Display) -> QuestApprovalStoreError {
    QuestApprovalStoreError::InvalidManifest(error.to_string())
}

fn io_error(
    action: &'static str,
    path: &Path,
    error: impl fmt::Display,
) -> QuestApprovalStoreError {
    QuestApprovalStoreError::Io {
        action,
        path: path.to_owned(),
        message: error.to_string(),
    }
}

/// 현재 원장에 한 선택을 합치고 전체 결과를 검증한다.
///
/// `expected_manifest_sha256`은 화면을 만들 때 읽은 원장의 digest여야 한다.
/// 파일 I/O 없이도 HTTP 밖에서 같은 domain 검사를 직접 실행할 수 있다.
pub fn apply_quest_translation_approval(
    workbook: &QuestTranslationDecisionWorkbook,
    current_manifest: &str,
    expected_manifest_sha256: &str,
    selection: QuestApprovalSelection,
) -> Result<QuestApprovalManifestUpdate, QuestApprovalStoreError> {
    let current = audit_quest_translation_approval_manifest(workbook, current_manifest)
        .map_err(invalid_manifest)?;
    if current.manifest_sha256 != expected_manifest_sha256 {
        return Err(QuestApprovalStoreError::ConcurrentChange {
            expected_sha256: expected_manifest_sha256.to_owned(),
            actual_sha256: current.manifest_sha256,
        });
    }

    let matching_baseline = current.baselines.iter().find(|baseline| {
        baseline.kind == selection.kind && baseline.decision_id == selection.decision_id
    });
    let Some(matching_baseline) = matching_baseline else {
        let same_id = current
            .baselines
            .iter()
            .find(|baseline| baseline.decision_id == selection.decision_id);
        if let Some(same_id) = same_id {
            return Err(QuestApprovalStoreError::DecisionKindMismatch {
                expected: same_id.kind,
                actual: selection.kind,
                decision_id: selection.decision_id,
            });
        }
        return Err(QuestApprovalStoreError::MissingDecision {
            kind: selection.kind,
            decision_id: selection.decision_id,
        });
    };
    if matching_baseline.baseline_sha256 != selection.baseline_sha256 {
        return Err(QuestApprovalStoreError::BaselineDrift {
            decision_id: selection.decision_id,
            expected_sha256: matching_baseline.baseline_sha256.clone(),
            actual_sha256: selection.baseline_sha256,
        });
    }

    let mut approvals = current.approvals;
    approvals.retain(|approval| {
        approval.kind != selection.kind || approval.decision_id != selection.decision_id
    });
    approvals.push(QuestTranslationApproval {
        kind: selection.kind,
        decision_id: selection.decision_id,
        baseline_sha256: selection.baseline_sha256,
        selected_korean: selection.selected_korean,
        notes: selection.notes,
    });
    let manifest = render_quest_translation_approval_manifest_tsv(&approvals);
    let analysis =
        audit_quest_translation_approval_manifest(workbook, &manifest).map_err(invalid_manifest)?;
    Ok(QuestApprovalManifestUpdate { manifest, analysis })
}

struct LockFile {
    path: PathBuf,
}

impl Drop for LockFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn acquire_lock(path: &Path) -> Result<LockFile, QuestApprovalStoreError> {
    let lock_path = path.with_extension(format!(
        "{}lock",
        path.extension()
            .and_then(|extension| extension.to_str())
            .map_or_else(String::new, |extension| format!("{extension}."))
    ));
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(QuestApprovalStoreError::Locked(lock_path));
        }
        Err(error) => return Err(io_error("lock 생성", &lock_path, error)),
    };
    let lock = LockFile { path: lock_path };
    writeln!(file, "{}", std::process::id())
        .and_then(|()| file.sync_all())
        .map_err(|error| io_error("lock 기록", &lock.path, error))?;
    Ok(lock)
}

struct TemporaryFile {
    path: PathBuf,
    committed: bool,
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn temporary_path(path: &Path) -> Result<PathBuf, QuestApprovalStoreError> {
    let parent = path.parent().ok_or_else(|| {
        QuestApprovalStoreError::InvalidPath(format!("부모 디렉토리가 없음: {}", path.display()))
    })?;
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            QuestApprovalStoreError::InvalidPath(format!("UTF-8 파일명이 아님: {}", path.display()))
        })?;
    let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), sequence)))
}

fn read_manifest(path: &Path) -> Result<String, QuestApprovalStoreError> {
    fs::read_to_string(path).map_err(|error| io_error("읽기", path, error))
}

/// 승인 원장을 비교 후 교체 방식으로 저장한다.
///
/// 같은 경로의 lock으로 이 writer들을 직렬화하고, 저장 직전 byte가 최초
/// 읽기와 달라졌으면 임시 파일을 버리고 원본을 보존한다.
pub fn save_quest_translation_approval(
    path: &Path,
    workbook: &QuestTranslationDecisionWorkbook,
    expected_manifest_sha256: &str,
    selection: QuestApprovalSelection,
) -> Result<QuestTranslationApprovalAnalysis, QuestApprovalStoreError> {
    let _lock = acquire_lock(path)?;
    let current_manifest = read_manifest(path)?;
    let update = apply_quest_translation_approval(
        workbook,
        &current_manifest,
        expected_manifest_sha256,
        selection,
    )?;

    let temporary_path = temporary_path(path)?;
    let mut temporary = TemporaryFile {
        path: temporary_path.clone(),
        committed: false,
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)
        .map_err(|error| io_error("임시 파일 생성", &temporary_path, error))?;
    file.write_all(update.manifest.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| io_error("임시 파일 기록", &temporary_path, error))?;

    let before_replace = read_manifest(path)?;
    if before_replace.as_bytes() != current_manifest.as_bytes() {
        let actual = audit_quest_translation_approval_manifest(workbook, &before_replace)
            .map(|analysis| analysis.manifest_sha256)
            .unwrap_or_else(|_| "invalid-manifest".to_owned());
        return Err(QuestApprovalStoreError::ConcurrentChange {
            expected_sha256: expected_manifest_sha256.to_owned(),
            actual_sha256: actual,
        });
    }

    fs::rename(&temporary_path, path).map_err(|error| io_error("원자적 교체", path, error))?;
    temporary.committed = true;

    let saved = read_manifest(path)?;
    if saved != update.manifest {
        return Err(QuestApprovalStoreError::ConcurrentChange {
            expected_sha256: update.analysis.manifest_sha256,
            actual_sha256: audit_quest_translation_approval_manifest(workbook, &saved)
                .map(|analysis| analysis.manifest_sha256)
                .unwrap_or_else(|_| "invalid-manifest".to_owned()),
        });
    }
    audit_quest_translation_approval_manifest(workbook, &saved).map_err(invalid_manifest)
}

#[cfg(test)]
#[path = "quest_translation_approval_store_tests.rs"]
mod tests;
