//! typed BTX catalog에서 version 관리 번역 manifest가 소유하는 범위.
//!
//! 이 분모는 현재 사람이 판정해 Rust catalog로 승격한 baked text만
//! 나타낸다. SEQ·EXE를 포함한 게임 전체 번역 모집단이나 완료율이 아니다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use super::btx_raster_translation_manifest::{
    general_btx_raster_translation_entries, GeneralRasterManifestError,
    GeneralRasterTranslationStatus,
};
use super::g_mess_translation::{
    gocha_result_message_entries, BtxRasterTranslationStatus, GmessTranslationError,
};
use super::mm_spr_translation::{
    escape_tsv_field, mm_spr_translation_entries, push_tsv_row, MmSprTranslationError,
    MmSprTranslationStatus,
};
use super::p2_info_translation::{
    p2_info_translation_entries, P2InfoTranslationError, P2InfoTranslationStatus,
};
use super::text_catalog::{
    is_japanese_translation_target, verify_text_catalog_specs, TextAssetSpec, TextCatalogError,
    BOX_TEXT_ASSETS,
};

/// typed BTX 일본어 대상 전체를 내보내는 검토 TSV의 schema version.
pub const BTX_TRANSLATION_REVIEW_SCHEMA_VERSION: &str = "btx-translation-review-v1";

/// typed BTX 일본어 대상의 번역 관리 상태 요약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslationCoverageReport {
    pub catalog_japanese_targets: usize,
    pub managed_targets: usize,
    pub poc_approved: usize,
    pub approved: usize,
    pub draft_human_review_required: usize,
    pub pending_human_translation: usize,
    pub scope_excluded_protected_no_write: usize,
    pub unmanaged_targets: usize,
}

#[derive(Debug, Clone)]
struct ManagedTranslation {
    block_id: String,
    protected_source: String,
    status: ManagedTranslationStatus,
    category: String,
    profile_id: String,
    translation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManagedTranslationStatus {
    PendingHumanTranslation,
    DraftHumanReviewRequired,
    PocApproved,
    Approved,
    ScopeExcludedProtectedNoWrite,
}

impl ManagedTranslationStatus {
    const fn label(self) -> &'static str {
        match self {
            Self::PendingHumanTranslation => "pending-human-translation",
            Self::DraftHumanReviewRequired => "draft-human-review-required",
            Self::PocApproved => "poc-approved",
            Self::Approved => "approved",
            Self::ScopeExcludedProtectedNoWrite => "scope-excluded-protected-no-write",
        }
    }
}

/// typed BTX 번역 coverage 계약 오류.
#[derive(Debug, PartialEq, Eq)]
pub enum TranslationCoverageError {
    Catalog(TextCatalogError),
    MmSprManifest(MmSprTranslationError),
    GmessManifest(GmessTranslationError),
    GeneralRasterManifest(GeneralRasterManifestError),
    P2InfoManifest(P2InfoTranslationError),
    DuplicateManagedBlock(String),
    UnknownManagedBlock(String),
    ReferenceOnlyManagedBlock(String),
    ProtectedSourceMismatch {
        id: String,
        expected: String,
        actual: String,
    },
    ManagedCountMismatch {
        managed: usize,
        statuses: usize,
    },
}

impl fmt::Display for TranslationCoverageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(source) => write!(f, "BTX text catalog 검증 실패: {source}"),
            Self::MmSprManifest(source) => write!(f, "MM_SPR 번역 manifest 검증 실패: {source}"),
            Self::GmessManifest(source) => write!(f, "G_MESS 번역 manifest 검증 실패: {source}"),
            Self::GeneralRasterManifest(source) => {
                write!(f, "일반 raster BTX 번역 manifest 검증 실패: {source}")
            }
            Self::P2InfoManifest(source) => {
                write!(f, "P2 INFO page 번역 manifest 검증 실패: {source}")
            }
            Self::DuplicateManagedBlock(id) => write!(f, "번역 관리 block ID 중복: {id}"),
            Self::UnknownManagedBlock(id) => write!(f, "typed BTX catalog에 없는 관리 block: {id}"),
            Self::ReferenceOnlyManagedBlock(id) => {
                write!(f, "{id}: reference-only block이 번역 관리 범위에 있음")
            }
            Self::ProtectedSourceMismatch {
                id,
                expected,
                actual,
            } => write!(
                f,
                "{id}: 번역 관리 보호 원문 불일치: expected={expected:?}, actual={actual:?}"
            ),
            Self::ManagedCountMismatch { managed, statuses } => write!(
                f,
                "번역 관리 상태 합계 불일치: managed={managed}, statuses={statuses}"
            ),
        }
    }
}

impl Error for TranslationCoverageError {}

impl From<TextCatalogError> for TranslationCoverageError {
    fn from(source: TextCatalogError) -> Self {
        Self::Catalog(source)
    }
}

impl From<MmSprTranslationError> for TranslationCoverageError {
    fn from(source: MmSprTranslationError) -> Self {
        Self::MmSprManifest(source)
    }
}

impl From<GmessTranslationError> for TranslationCoverageError {
    fn from(source: GmessTranslationError) -> Self {
        Self::GmessManifest(source)
    }
}

impl From<GeneralRasterManifestError> for TranslationCoverageError {
    fn from(source: GeneralRasterManifestError) -> Self {
        Self::GeneralRasterManifest(source)
    }
}

impl From<P2InfoTranslationError> for TranslationCoverageError {
    fn from(source: P2InfoTranslationError) -> Self {
        Self::P2InfoManifest(source)
    }
}

fn verify_coverage(
    specs: &[TextAssetSpec<'_>],
    managed: &[ManagedTranslation],
) -> Result<TranslationCoverageReport, TranslationCoverageError> {
    let catalog = verify_text_catalog_specs(specs)?;
    let mut ids = BTreeSet::new();
    let mut poc_approved = 0usize;
    let mut approved = 0usize;
    let mut draft_human_review_required = 0usize;
    let mut pending_human_translation = 0usize;
    let mut scope_excluded_protected_no_write = 0usize;

    for entry in managed {
        if !ids.insert(entry.block_id.as_str()) {
            return Err(TranslationCoverageError::DuplicateManagedBlock(
                entry.block_id.to_string(),
            ));
        }
        let block = specs
            .iter()
            .flat_map(|spec| spec.blocks)
            .find(|block| block.id == entry.block_id)
            .ok_or_else(|| {
                TranslationCoverageError::UnknownManagedBlock(entry.block_id.to_string())
            })?;
        if !is_japanese_translation_target(block.source) {
            return Err(TranslationCoverageError::ReferenceOnlyManagedBlock(
                entry.block_id.to_string(),
            ));
        }
        if block.source != entry.protected_source {
            return Err(TranslationCoverageError::ProtectedSourceMismatch {
                id: entry.block_id.to_string(),
                expected: block.source.to_string(),
                actual: entry.protected_source.to_string(),
            });
        }
        match entry.status {
            ManagedTranslationStatus::PocApproved => poc_approved += 1,
            ManagedTranslationStatus::Approved => approved += 1,
            ManagedTranslationStatus::PendingHumanTranslation => {
                pending_human_translation += 1;
            }
            ManagedTranslationStatus::DraftHumanReviewRequired => {
                draft_human_review_required += 1;
            }
            ManagedTranslationStatus::ScopeExcludedProtectedNoWrite => {
                scope_excluded_protected_no_write += 1;
            }
        }
    }

    let managed_targets = ids.len();
    let statuses = poc_approved
        + approved
        + draft_human_review_required
        + pending_human_translation
        + scope_excluded_protected_no_write;
    if statuses != managed_targets {
        return Err(TranslationCoverageError::ManagedCountMismatch {
            managed: managed_targets,
            statuses,
        });
    }

    Ok(TranslationCoverageReport {
        catalog_japanese_targets: catalog.japanese_target_blocks,
        managed_targets,
        poc_approved,
        approved,
        draft_human_review_required,
        pending_human_translation,
        scope_excluded_protected_no_write,
        unmanaged_targets: catalog
            .japanese_target_blocks
            .checked_sub(managed_targets)
            .ok_or(TranslationCoverageError::ManagedCountMismatch {
                managed: managed_targets,
                statuses: catalog.japanese_target_blocks,
            })?,
    })
}

/// 현재 typed BTX catalog와 모든 version 관리 번역 manifest의 연결을 검증한다.
pub fn verify_translation_coverage() -> Result<TranslationCoverageReport, TranslationCoverageError>
{
    let managed = managed_translations()?;
    verify_coverage(BOX_TEXT_ASSETS, &managed)
}

fn managed_translations() -> Result<Vec<ManagedTranslation>, TranslationCoverageError> {
    let mut managed = Vec::new();
    for entry in mm_spr_translation_entries()? {
        let status = match entry.status {
            MmSprTranslationStatus::PendingHumanTranslation => {
                ManagedTranslationStatus::PendingHumanTranslation
            }
            MmSprTranslationStatus::DraftHumanReviewRequired => {
                ManagedTranslationStatus::DraftHumanReviewRequired
            }
            MmSprTranslationStatus::PocApproved => ManagedTranslationStatus::PocApproved,
            MmSprTranslationStatus::Approved => ManagedTranslationStatus::Approved,
        };
        managed.push(ManagedTranslation {
            block_id: entry.block_id,
            protected_source: entry.protected_source,
            status,
            category: entry.category.label().to_string(),
            profile_id: entry.category.profile().id.to_string(),
            translation: entry.ko,
        });
    }
    for entry in gocha_result_message_entries()? {
        let status = match entry.status {
            BtxRasterTranslationStatus::DraftHumanReviewRequired => {
                ManagedTranslationStatus::DraftHumanReviewRequired
            }
            BtxRasterTranslationStatus::Approved => ManagedTranslationStatus::Approved,
        };
        managed.push(ManagedTranslation {
            block_id: entry.block_id.to_string(),
            protected_source: entry.protected_source.to_string(),
            status,
            category: entry.category.to_string(),
            profile_id: entry.profile_id.to_string(),
            translation: entry.ko.to_string(),
        });
    }
    for entry in general_btx_raster_translation_entries()? {
        let status = match entry.status {
            GeneralRasterTranslationStatus::PendingHumanTranslation => {
                ManagedTranslationStatus::PendingHumanTranslation
            }
            GeneralRasterTranslationStatus::DraftHumanReviewRequired => {
                ManagedTranslationStatus::DraftHumanReviewRequired
            }
            GeneralRasterTranslationStatus::Approved => ManagedTranslationStatus::Approved,
            GeneralRasterTranslationStatus::ScopeExcludedProtectedNoWrite => {
                ManagedTranslationStatus::ScopeExcludedProtectedNoWrite
            }
        };
        let category = entry.category_id.unwrap_or(entry.semantic_family);
        managed.push(ManagedTranslation {
            block_id: entry.block_id,
            protected_source: entry.protected_source,
            status,
            category,
            profile_id: entry.profile_id.unwrap_or_default(),
            translation: entry.ko,
        });
    }
    for entry in p2_info_translation_entries()? {
        let status = match entry.translation_status {
            P2InfoTranslationStatus::PendingHumanTranslation => {
                ManagedTranslationStatus::PendingHumanTranslation
            }
            P2InfoTranslationStatus::DraftHumanReviewRequired => {
                ManagedTranslationStatus::DraftHumanReviewRequired
            }
            P2InfoTranslationStatus::Approved => ManagedTranslationStatus::Approved,
        };
        managed.push(ManagedTranslation {
            block_id: entry.block_id,
            protected_source: entry.protected_source,
            status,
            category: entry.category,
            profile_id: entry.profile_id,
            translation: entry.ko,
        });
    }
    Ok(managed)
}

/// typed BTX 일본어 대상 전체의 관리 상태를 결정적 TSV로 만든다.
///
/// `unmanaged` 항목은 아직 translation manifest·profile이 없음을 뜻한다.
/// 이 파생물을 사람이 수정해도 제품 입력에는 반영되지 않는다.
pub fn render_translation_review_tsv() -> Result<String, TranslationCoverageError> {
    let managed = managed_translations()?;
    render_translation_review_tsv_from(BOX_TEXT_ASSETS, &managed)
}

fn render_translation_review_tsv_from(
    specs: &[TextAssetSpec<'_>],
    managed: &[ManagedTranslation],
) -> Result<String, TranslationCoverageError> {
    let coverage = verify_coverage(specs, managed)?;
    let mut output = String::new();
    output.push_str(
        "schema_version\tarchive_paths\tasset_name\tasset_sha256\tblock_id\t\
         block_pixels_sha256\tstatus\tcategory\tprofile_id\tbbox_x\tbbox_y\t\
         bbox_width\tbbox_height\tprotected_source\tko\n",
    );
    let mut rows = 0usize;
    let mut targets = specs
        .iter()
        .flat_map(|spec| {
            spec.blocks
                .iter()
                .filter(|block| is_japanese_translation_target(block.source))
                .map(move |block| (spec, block))
        })
        .collect::<Vec<_>>();
    targets.sort_by_key(|(_, block)| block.id);

    for (spec, block) in targets {
        let archive_paths = spec.archive_paths.join("\n");
        let managed_entry = managed.iter().find(|entry| entry.block_id == block.id);
        let (status, category, profile_id, translation) = match managed_entry {
            Some(entry) => (
                entry.status.label(),
                entry.category.as_str(),
                entry.profile_id.as_str(),
                entry.translation.as_str(),
            ),
            None => ("unmanaged", "", "", ""),
        };
        push_tsv_row(
            &mut output,
            &[
                BTX_TRANSLATION_REVIEW_SCHEMA_VERSION.to_string(),
                escape_tsv_field(&archive_paths),
                spec.name.to_string(),
                spec.expected_decompressed_sha256.to_string(),
                block.id.to_string(),
                block.expected_pixels_sha256.to_string(),
                status.to_string(),
                category.to_string(),
                profile_id.to_string(),
                block.bbox.x.to_string(),
                block.bbox.y.to_string(),
                block.bbox.width.to_string(),
                block.bbox.height.to_string(),
                escape_tsv_field(block.source),
                escape_tsv_field(translation),
            ],
        );
        rows += 1;
    }
    if rows != coverage.catalog_japanese_targets {
        return Err(TranslationCoverageError::ManagedCountMismatch {
            managed: rows,
            statuses: coverage.catalog_japanese_targets,
        });
    }
    Ok(output)
}

#[cfg(test)]
#[path = "translation_coverage_tests.rs"]
mod tests;
