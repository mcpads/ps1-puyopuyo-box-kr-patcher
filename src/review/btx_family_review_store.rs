//! BTX family 사람 관측 한 건을 현재 source 기준선에 결속해 저장한다.
//!
//! browser나 HTTP는 이 모듈을 우회해 원장을 만들 수 없다. 기존 원장을
//! 전수 검증하고 한 선택을 합친 전체 원장을 다시 검증한 뒤, 같은
//! filesystem의 임시 파일로 원자 교체한다.

use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::graphics::btx_family_candidates::BtxFamilyCandidateAnalysis;

use super::{
    audit_btx_family_review_decisions, render_btx_family_review_decisions_tsv,
    BtxFamilyReviewAnalysis, BtxFamilyReviewDecision, BtxFamilyReviewDecisionKind,
};

static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 사람이 한 family 대표를 보고 명시적으로 남긴 선택.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyReviewSelection {
    pub family_key: String,
    pub baseline_sha256: String,
    pub decision: BtxFamilyReviewDecisionKind,
    pub notes: String,
}

/// 저장 전에 전수 검증한 새 원장과 파생 상태.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyReviewManifestUpdate {
    pub manifest: String,
    pub analysis: BtxFamilyReviewAnalysis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BtxFamilyReviewStoreError {
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
    MissingFamily(String),
    BaselineDrift {
        family_key: String,
        expected_sha256: String,
        actual_sha256: String,
    },
    PendingSelection,
    InvalidManifest(String),
}

impl fmt::Display for BtxFamilyReviewStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(message) => write!(formatter, "BTX family 검토 원장 경로 오류: {message}"),
            Self::Io {
                action,
                path,
                message,
            } => write!(
                formatter,
                "BTX family 검토 원장 {action} 실패 ({}): {message}",
                path.display()
            ),
            Self::Locked(path) => write!(
                formatter,
                "BTX family 검토 원장을 다른 writer가 저장 중임: {}",
                path.display()
            ),
            Self::ConcurrentChange {
                expected_sha256,
                actual_sha256,
            } => write!(
                formatter,
                "BTX family 검토 원장이 다른 process에서 변경됨: expected={expected_sha256}, actual={actual_sha256}; 다시 불러와야 함"
            ),
            Self::MissingFamily(family_key) => write!(
                formatter,
                "BTX family 검토 대상이 현재 source-derived 후보에 없음: {family_key}"
            ),
            Self::BaselineDrift {
                family_key,
                expected_sha256,
                actual_sha256,
            } => write!(
                formatter,
                "BTX family 검토 기준선 drift: {family_key}, expected={expected_sha256}, actual={actual_sha256}; 다시 불러와야 함"
            ),
            Self::PendingSelection => write!(
                formatter,
                "pending-human-review는 사람이 제출하는 관측 결과가 아님"
            ),
            Self::InvalidManifest(message) => {
                write!(formatter, "BTX family 검토 원장 검증 실패: {message}")
            }
        }
    }
}

impl Error for BtxFamilyReviewStoreError {}

fn invalid_manifest(error: impl fmt::Display) -> BtxFamilyReviewStoreError {
    BtxFamilyReviewStoreError::InvalidManifest(error.to_string())
}

fn io_error(
    action: &'static str,
    path: &Path,
    error: impl fmt::Display,
) -> BtxFamilyReviewStoreError {
    BtxFamilyReviewStoreError::Io {
        action,
        path: path.to_owned(),
        message: error.to_string(),
    }
}

/// 현재 원장에 한 관측을 합치고 전체 결과를 다시 검증한다.
pub fn apply_btx_family_review_decision(
    candidates: &BtxFamilyCandidateAnalysis,
    current_manifest: &str,
    expected_manifest_sha256: &str,
    selection: BtxFamilyReviewSelection,
) -> Result<BtxFamilyReviewManifestUpdate, BtxFamilyReviewStoreError> {
    if selection.decision == BtxFamilyReviewDecisionKind::PendingHumanReview {
        return Err(BtxFamilyReviewStoreError::PendingSelection);
    }
    let current = audit_btx_family_review_decisions(candidates, current_manifest)
        .map_err(invalid_manifest)?;
    if current.manifest_sha256 != expected_manifest_sha256 {
        return Err(BtxFamilyReviewStoreError::ConcurrentChange {
            expected_sha256: expected_manifest_sha256.to_owned(),
            actual_sha256: current.manifest_sha256,
        });
    }

    let matching = current
        .decisions
        .iter()
        .find(|decision| decision.family_key == selection.family_key)
        .ok_or_else(|| BtxFamilyReviewStoreError::MissingFamily(selection.family_key.clone()))?;
    if matching.baseline_sha256 != selection.baseline_sha256 {
        return Err(BtxFamilyReviewStoreError::BaselineDrift {
            family_key: selection.family_key,
            expected_sha256: matching.baseline_sha256.clone(),
            actual_sha256: selection.baseline_sha256,
        });
    }

    let mut decisions = current.decisions;
    decisions.retain(|decision| decision.family_key != selection.family_key);
    decisions.push(BtxFamilyReviewDecision {
        family_key: selection.family_key,
        baseline_sha256: selection.baseline_sha256,
        decision: selection.decision,
        notes: selection.notes,
    });
    let manifest = render_btx_family_review_decisions_tsv(&decisions);
    let analysis =
        audit_btx_family_review_decisions(candidates, &manifest).map_err(invalid_manifest)?;
    Ok(BtxFamilyReviewManifestUpdate { manifest, analysis })
}

struct LockFile {
    path: PathBuf,
}

impl Drop for LockFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn lock_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}lock",
        path.extension()
            .and_then(|extension| extension.to_str())
            .map_or_else(String::new, |extension| format!("{extension}."))
    ))
}

fn acquire_lock(path: &Path) -> Result<LockFile, BtxFamilyReviewStoreError> {
    let lock_path = lock_path(path);
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(BtxFamilyReviewStoreError::Locked(lock_path));
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

fn temporary_path(path: &Path) -> Result<PathBuf, BtxFamilyReviewStoreError> {
    let parent = path.parent().ok_or_else(|| {
        BtxFamilyReviewStoreError::InvalidPath(format!("부모 디렉토리가 없음: {}", path.display()))
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
            BtxFamilyReviewStoreError::InvalidPath(format!(
                "UTF-8 파일명이 아님: {}",
                path.display()
            ))
        })?;
    let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), sequence)))
}

fn read_manifest(path: &Path) -> Result<String, BtxFamilyReviewStoreError> {
    fs::read_to_string(path).map_err(|error| io_error("읽기", path, error))
}

/// 관측 원장을 같은 경로의 lock과 비교 후 교체 방식으로 저장한다.
pub fn save_btx_family_review_decision(
    path: &Path,
    candidates: &BtxFamilyCandidateAnalysis,
    expected_manifest_sha256: &str,
    selection: BtxFamilyReviewSelection,
) -> Result<BtxFamilyReviewAnalysis, BtxFamilyReviewStoreError> {
    let _lock = acquire_lock(path)?;
    let current_manifest = read_manifest(path)?;
    let update = apply_btx_family_review_decision(
        candidates,
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
        let actual = audit_btx_family_review_decisions(candidates, &before_replace)
            .map(|analysis| analysis.manifest_sha256)
            .unwrap_or_else(|_| "invalid-manifest".to_owned());
        return Err(BtxFamilyReviewStoreError::ConcurrentChange {
            expected_sha256: expected_manifest_sha256.to_owned(),
            actual_sha256: actual,
        });
    }

    fs::rename(&temporary_path, path).map_err(|error| io_error("원자적 교체", path, error))?;
    temporary.committed = true;

    let saved = read_manifest(path)?;
    if saved != update.manifest {
        return Err(BtxFamilyReviewStoreError::ConcurrentChange {
            expected_sha256: update.analysis.manifest_sha256,
            actual_sha256: audit_btx_family_review_decisions(candidates, &saved)
                .map(|analysis| analysis.manifest_sha256)
                .unwrap_or_else(|_| "invalid-manifest".to_owned()),
        });
    }
    audit_btx_family_review_decisions(candidates, &saved).map_err(invalid_manifest)
}

#[cfg(test)]
#[path = "btx_family_review_store_tests.rs"]
mod tests;
