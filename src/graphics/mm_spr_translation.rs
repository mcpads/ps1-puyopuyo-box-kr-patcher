//! `MM_SPR.BTX` 일본어 설명문의 번역 모집단과 category profile.
//!
//! 원문·bbox·픽셀 digest는 [`super::text_catalog`]가 보호한다. 이 모듈은
//! 그 catalog의 일본어 17개 block을 안정 ID로 전수 연결하고, 사람 승인
//! 전인 항목과 비배포 PoC 승인 항목을 구분한다. 영문 label은 이 모집단에
//! 넣지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use super::text_catalog::{is_japanese_translation_target, TextAssetSpec, MM_SPR_TEXT_ASSET};
use super::text_profile::{
    BakedTextEffect, BakedTextProfile, HorizontalAlign, MAPLESTORY_LIGHT_FONT_ID,
};

pub const MM_SPR_FIRST_DESCRIPTION_BLOCK_ID: &str = "BTXB-0002";
pub const MM_SPR_TRANSLATION_SCHEMA_VERSION: &str = "mm-spr-translation-v1";

const TRANSLATION_MANIFEST: &str = include_str!("../../assets/translations/btx/mm_spr.tsv");
const TRANSLATION_HEADER: &str = "schema_version\tarchive_paths\tasset_name\tasset_sha256\t\
                                  block_id\tblock_pixels_sha256\tbbox_x\tbbox_y\tbbox_width\t\
                                  bbox_height\tprotected_source\tcategory\tko\tstatus";
const BODY_RAMP: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];

/// `MM_SPR.BTX` mode 설명문의 원본 행 수별 style category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmSprTextCategory {
    ModeDescriptionOneLine,
    ModeDescriptionTwoLines,
    ModeDescriptionThreeLines,
}

impl MmSprTextCategory {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ModeDescriptionOneLine => "mode-description-1-line",
            Self::ModeDescriptionTwoLines => "mode-description-2-lines",
            Self::ModeDescriptionThreeLines => "mode-description-3-lines",
        }
    }

    pub const fn line_count(self) -> usize {
        match self {
            Self::ModeDescriptionOneLine => 1,
            Self::ModeDescriptionTwoLines => 2,
            Self::ModeDescriptionThreeLines => 3,
        }
    }

    /// category별 독립적인 version 관리 raster profile.
    pub const fn profile(self) -> &'static BakedTextProfile<'static> {
        match self {
            Self::ModeDescriptionOneLine => &MM_SPR_MODE_DESCRIPTION_PROFILE,
            Self::ModeDescriptionTwoLines => &MM_SPR_MODE_DESCRIPTION_TWO_LINE_PROFILE,
            Self::ModeDescriptionThreeLines => &MM_SPR_MODE_DESCRIPTION_THREE_LINE_PROFILE,
        }
    }

    fn parse(value: &str, line: usize) -> Result<Self, MmSprTranslationError> {
        match value {
            "mode-description-1-line" => Ok(Self::ModeDescriptionOneLine),
            "mode-description-2-lines" => Ok(Self::ModeDescriptionTwoLines),
            "mode-description-3-lines" => Ok(Self::ModeDescriptionThreeLines),
            _ => Err(MmSprTranslationError::InvalidManifest(format!(
                "{line}행 category 미지원: {value:?}"
            ))),
        }
    }
}

/// 한 행 mode 설명문의 첫 승인 profile.
pub static MM_SPR_MODE_DESCRIPTION_PROFILE: BakedTextProfile<'static> = BakedTextProfile {
    id: "mm-spr-mode-description",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 12.0,
    baseline: 13,
    line_count: 1,
    line_height: 16,
    tracking: 0,
    align: HorizontalAlign::Center,
    background: 0,
    body_ramp: &BODY_RAMP,
    effect: BakedTextEffect::None,
};

/// 두 행 mode 설명문 profile. 번역문 자체는 아직 승인하지 않았다.
pub static MM_SPR_MODE_DESCRIPTION_TWO_LINE_PROFILE: BakedTextProfile<'static> = BakedTextProfile {
    id: "mm-spr-mode-description-2-line",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 12.0,
    baseline: 13,
    line_count: 2,
    line_height: 16,
    tracking: 0,
    align: HorizontalAlign::Center,
    background: 0,
    body_ramp: &BODY_RAMP,
    effect: BakedTextEffect::None,
};

/// 세 행 mode 설명문 profile. 번역문 자체는 아직 승인하지 않았다.
pub static MM_SPR_MODE_DESCRIPTION_THREE_LINE_PROFILE: BakedTextProfile<'static> =
    BakedTextProfile {
        id: "mm-spr-mode-description-3-line",
        font_id: MAPLESTORY_LIGHT_FONT_ID,
        font_size: 12.0,
        baseline: 13,
        line_count: 3,
        line_height: 16,
        tracking: 0,
        align: HorizontalAlign::Center,
        background: 0,
        body_ramp: &BODY_RAMP,
        effect: BakedTextEffect::None,
    };

/// 번역문이 현재 얻은 사람 승인 범위.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmSprTranslationStatus {
    PendingHumanTranslation,
    DraftHumanReviewRequired,
    PocApproved,
    Approved,
}

impl MmSprTranslationStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PendingHumanTranslation => "pending-human-translation",
            Self::DraftHumanReviewRequired => "draft-human-review-required",
            Self::PocApproved => "poc-approved",
            Self::Approved => "approved",
        }
    }

    fn parse(value: &str, line: usize) -> Result<Self, MmSprTranslationError> {
        match value {
            "pending-human-translation" => Ok(Self::PendingHumanTranslation),
            "draft-human-review-required" => Ok(Self::DraftHumanReviewRequired),
            "poc-approved" => Ok(Self::PocApproved),
            "approved" => Ok(Self::Approved),
            _ => Err(MmSprTranslationError::InvalidManifest(format!(
                "{line}행 번역 상태 미지원: {value:?}"
            ))),
        }
    }

    const fn is_poc_build_eligible(self) -> bool {
        matches!(self, Self::PocApproved | Self::Approved)
    }
}

/// catalog 보호 필드에 연결되는 `MM_SPR` 번역 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmSprTranslationEntry {
    pub archive_paths: String,
    pub asset_name: String,
    pub asset_sha256: String,
    pub block_id: String,
    pub block_pixels_sha256: String,
    pub bbox_x: usize,
    pub bbox_y: usize,
    pub bbox_width: usize,
    pub bbox_height: usize,
    pub protected_source: String,
    pub category: MmSprTextCategory,
    pub status: MmSprTranslationStatus,
    pub ko: String,
}

/// manifest 검증을 통과해 개발·PoC 합성에 사용할 수 있는 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmSprApprovedPocTranslation {
    pub block_id: String,
    pub text: String,
    pub category: MmSprTextCategory,
}

/// 사람이 검토할 `MM_SPR` 파생 TSV의 schema version.
pub const MM_SPR_REVIEW_SCHEMA_VERSION: &str = "mm-spr-review-v1";

/// 번역 모집단과 상태 요약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmSprTranslationReport {
    pub japanese_targets: usize,
    pub poc_approved: usize,
    pub approved: usize,
    pub draft_human_review_required: usize,
    pub pending_human_translation: usize,
    pub one_line: usize,
    pub two_lines: usize,
    pub three_lines: usize,
}

/// `MM_SPR` 번역 manifest 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MmSprTranslationError {
    InvalidManifest(String),
    EmptyManifest,
    DuplicateBlockId(String),
    UnknownBlockId(String),
    ReferenceOnlyBlock(String),
    MissingJapaneseTarget(String),
    NonCanonicalOrder(String),
    ProtectedAssetMismatch(String),
    ProtectedSourceMismatch {
        id: String,
        expected: String,
        actual: String,
    },
    SourceLineCountMismatch {
        id: String,
        expected: usize,
        actual: usize,
    },
    SourceHeightMismatch {
        id: String,
        expected: usize,
        actual: usize,
    },
    StatusTranslationMismatch(String),
    InvalidTranslation(String),
    TranslationLineCountMismatch {
        id: String,
        expected: usize,
        actual: usize,
    },
    NoApprovedPocTranslations,
    NotPocBuildEligible(String),
}

impl fmt::Display for MmSprTranslationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManifest(message) => {
                write!(f, "MM_SPR 번역 manifest 형식 오류: {message}")
            }
            Self::EmptyManifest => f.write_str("MM_SPR 번역 manifest가 비어 있음"),
            Self::DuplicateBlockId(id) => write!(f, "MM_SPR 번역 block ID 중복: {id}"),
            Self::UnknownBlockId(id) => write!(f, "MM_SPR catalog에 없는 번역 block: {id}"),
            Self::ReferenceOnlyBlock(id) => {
                write!(
                    f,
                    "{id}: 일본어가 없는 reference-only block이 번역 manifest에 있음"
                )
            }
            Self::MissingJapaneseTarget(id) => {
                write!(f, "{id}: 일본어 번역 대상이 manifest에서 누락됨")
            }
            Self::NonCanonicalOrder(id) => {
                write!(
                    f,
                    "{id}: MM_SPR 번역 manifest의 block ID 순서가 canonical이 아님"
                )
            }
            Self::ProtectedAssetMismatch(id) => {
                write!(f, "{id}: MM_SPR 보호 자산 필드 drift")
            }
            Self::ProtectedSourceMismatch {
                id,
                expected,
                actual,
            } => write!(
                f,
                "{id}: 보호 원문 불일치: expected={expected:?}, actual={actual:?}"
            ),
            Self::SourceLineCountMismatch {
                id,
                expected,
                actual,
            } => write!(
                f,
                "{id}: 원문 행 수와 category 불일치: expected={expected}, actual={actual}"
            ),
            Self::SourceHeightMismatch {
                id,
                expected,
                actual,
            } => write!(
                f,
                "{id}: bbox 높이와 category 불일치: expected={expected}, actual={actual}"
            ),
            Self::StatusTranslationMismatch(id) => {
                write!(f, "{id}: 번역 상태와 한국어 입력 유무가 맞지 않음")
            }
            Self::InvalidTranslation(id) => {
                write!(f, "{id}: 한국어 번역문의 공백·한글·일본어 잔존 계약 불일치")
            }
            Self::TranslationLineCountMismatch {
                id,
                expected,
                actual,
            } => write!(
                f,
                "{id}: 번역문 행 수와 category 불일치: expected={expected}, actual={actual}"
            ),
            Self::NoApprovedPocTranslations => {
                f.write_str("MM_SPR manifest에 PoC 승인 번역이 하나도 없음")
            }
            Self::NotPocBuildEligible(id) => {
                write!(
                    f,
                    "{id}: PoC build 적격 상태가 아니라 입력으로 사용할 수 없음"
                )
            }
        }
    }
}

impl Error for MmSprTranslationError {}

fn has_hangul_syllable(text: &str) -> bool {
    text.chars()
        .any(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
}

fn parse_usize(field: &str, value: &str, line: usize) -> Result<usize, MmSprTranslationError> {
    value.parse().map_err(|_| {
        MmSprTranslationError::InvalidManifest(format!(
            "{line}행 {field}가 usize가 아님: {value:?}"
        ))
    })
}

fn unescape_tsv_field(value: &str, line: usize) -> Result<String, MmSprTranslationError> {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let escaped = characters.next().ok_or_else(|| {
            MmSprTranslationError::InvalidManifest(format!("{line}행 TSV escape가 끝에서 잘림"))
        })?;
        match escaped {
            '\\' => output.push('\\'),
            't' => output.push('\t'),
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            _ => {
                return Err(MmSprTranslationError::InvalidManifest(format!(
                    "{line}행 알 수 없는 TSV escape: \\{escaped}"
                )))
            }
        }
    }
    Ok(output)
}

fn parse_translation_manifest(
    raw: &str,
) -> Result<Vec<MmSprTranslationEntry>, MmSprTranslationError> {
    if !raw.ends_with('\n') || raw.contains('\r') {
        return Err(MmSprTranslationError::InvalidManifest(
            "LF로 끝나야 하며 CR을 포함할 수 없음".to_string(),
        ));
    }
    if raw.lines().next() != Some(TRANSLATION_HEADER) {
        return Err(MmSprTranslationError::InvalidManifest(
            "header 불일치".to_string(),
        ));
    }
    raw.lines()
        .skip(1)
        .enumerate()
        .map(|(index, row)| {
            let line = index + 2;
            let fields = row.split('\t').collect::<Vec<_>>();
            let [schema, archives, asset, asset_sha, block, block_sha, x, y, width, height, source, category, ko, status] =
                fields.as_slice()
            else {
                return Err(MmSprTranslationError::InvalidManifest(format!(
                    "{line}행 field 수 불일치"
                )));
            };
            if *schema != MM_SPR_TRANSLATION_SCHEMA_VERSION {
                return Err(MmSprTranslationError::InvalidManifest(format!(
                    "{line}행 schema 불일치"
                )));
            }
            Ok(MmSprTranslationEntry {
                archive_paths: unescape_tsv_field(archives, line)?,
                asset_name: (*asset).to_string(),
                asset_sha256: (*asset_sha).to_string(),
                block_id: (*block).to_string(),
                block_pixels_sha256: (*block_sha).to_string(),
                bbox_x: parse_usize("bbox_x", x, line)?,
                bbox_y: parse_usize("bbox_y", y, line)?,
                bbox_width: parse_usize("bbox_width", width, line)?,
                bbox_height: parse_usize("bbox_height", height, line)?,
                protected_source: unescape_tsv_field(source, line)?,
                category: MmSprTextCategory::parse(category, line)?,
                ko: unescape_tsv_field(ko, line)?,
                status: MmSprTranslationStatus::parse(status, line)?,
            })
        })
        .collect()
}

fn normalized_translation(
    entry: &MmSprTranslationEntry,
) -> Result<Option<&str>, MmSprTranslationError> {
    if entry.status == MmSprTranslationStatus::PendingHumanTranslation {
        return if entry.ko.is_empty() {
            Ok(None)
        } else {
            Err(MmSprTranslationError::StatusTranslationMismatch(
                entry.block_id.clone(),
            ))
        };
    }
    if entry.ko.is_empty()
        || entry.ko.contains('\r')
        || entry.ko.trim() != entry.ko
        || !has_hangul_syllable(&entry.ko)
        || is_japanese_translation_target(&entry.ko)
    {
        return Err(MmSprTranslationError::InvalidTranslation(
            entry.block_id.clone(),
        ));
    }
    let actual = entry.ko.split('\n').count();
    let expected = entry.category.line_count();
    if actual != expected {
        return Err(MmSprTranslationError::TranslationLineCountMismatch {
            id: entry.block_id.clone(),
            expected,
            actual,
        });
    }
    Ok(Some(&entry.ko))
}

fn verify_manifest(
    spec: TextAssetSpec<'_>,
    entries: &[MmSprTranslationEntry],
) -> Result<MmSprTranslationReport, MmSprTranslationError> {
    if entries.is_empty() {
        return Err(MmSprTranslationError::EmptyManifest);
    }

    let mut ids = BTreeSet::new();
    let mut report = MmSprTranslationReport {
        japanese_targets: 0,
        poc_approved: 0,
        approved: 0,
        draft_human_review_required: 0,
        pending_human_translation: 0,
        one_line: 0,
        two_lines: 0,
        three_lines: 0,
    };
    let expected_archives = spec.archive_paths.join("\n");
    let mut previous_id: Option<&str> = None;
    for entry in entries {
        if !ids.insert(entry.block_id.as_str()) {
            return Err(MmSprTranslationError::DuplicateBlockId(
                entry.block_id.clone(),
            ));
        }
        if previous_id.is_some_and(|previous| previous > entry.block_id.as_str()) {
            return Err(MmSprTranslationError::NonCanonicalOrder(
                entry.block_id.clone(),
            ));
        }
        previous_id = Some(&entry.block_id);
        let block = spec
            .blocks
            .iter()
            .find(|block| block.id == entry.block_id)
            .ok_or_else(|| MmSprTranslationError::UnknownBlockId(entry.block_id.clone()))?;
        if !is_japanese_translation_target(block.source) {
            return Err(MmSprTranslationError::ReferenceOnlyBlock(
                entry.block_id.clone(),
            ));
        }
        if entry.archive_paths != expected_archives
            || entry.asset_name != spec.name
            || entry.asset_sha256 != spec.expected_decompressed_sha256
            || entry.block_pixels_sha256 != block.expected_pixels_sha256
            || (
                entry.bbox_x,
                entry.bbox_y,
                entry.bbox_width,
                entry.bbox_height,
            ) != (
                block.bbox.x,
                block.bbox.y,
                block.bbox.width,
                block.bbox.height,
            )
        {
            return Err(MmSprTranslationError::ProtectedAssetMismatch(
                entry.block_id.clone(),
            ));
        }
        if block.source != entry.protected_source {
            return Err(MmSprTranslationError::ProtectedSourceMismatch {
                id: entry.block_id.clone(),
                expected: block.source.to_string(),
                actual: entry.protected_source.clone(),
            });
        }

        let expected_lines = entry.category.line_count();
        let actual_lines = entry.protected_source.split('\n').count();
        if actual_lines != expected_lines {
            return Err(MmSprTranslationError::SourceLineCountMismatch {
                id: entry.block_id.clone(),
                expected: expected_lines,
                actual: actual_lines,
            });
        }
        let expected_height = expected_lines * entry.category.profile().line_height;
        if block.bbox.height != expected_height {
            return Err(MmSprTranslationError::SourceHeightMismatch {
                id: entry.block_id.clone(),
                expected: expected_height,
                actual: block.bbox.height,
            });
        }
        normalized_translation(entry)?;

        report.japanese_targets += 1;
        match entry.status {
            MmSprTranslationStatus::PendingHumanTranslation => {
                report.pending_human_translation += 1;
            }
            MmSprTranslationStatus::DraftHumanReviewRequired => {
                report.draft_human_review_required += 1;
            }
            MmSprTranslationStatus::PocApproved => report.poc_approved += 1,
            MmSprTranslationStatus::Approved => report.approved += 1,
        }
        match entry.category {
            MmSprTextCategory::ModeDescriptionOneLine => report.one_line += 1,
            MmSprTextCategory::ModeDescriptionTwoLines => report.two_lines += 1,
            MmSprTextCategory::ModeDescriptionThreeLines => report.three_lines += 1,
        }
    }

    for block in spec
        .blocks
        .iter()
        .filter(|block| is_japanese_translation_target(block.source))
    {
        if !ids.contains(block.id) {
            return Err(MmSprTranslationError::MissingJapaneseTarget(
                block.id.to_string(),
            ));
        }
    }
    Ok(report)
}

/// 고정 catalog와 번역 manifest의 전수 연결을 검증한다.
pub fn verify_mm_spr_translation_manifest() -> Result<MmSprTranslationReport, MmSprTranslationError>
{
    let entries = parse_translation_manifest(TRANSLATION_MANIFEST)?;
    verify_manifest(MM_SPR_TEXT_ASSET, &entries)
}

/// 보호 원본 검증을 통과한 `MM_SPR` 번역 항목을 canonical 순서로 반환한다.
pub fn mm_spr_translation_entries() -> Result<Vec<MmSprTranslationEntry>, MmSprTranslationError> {
    let entries = parse_translation_manifest(TRANSLATION_MANIFEST)?;
    verify_manifest(MM_SPR_TEXT_ASSET, &entries)?;
    Ok(entries)
}

fn approved_poc_translations_from(
    spec: TextAssetSpec<'_>,
    entries: &[MmSprTranslationEntry],
) -> Result<Vec<MmSprApprovedPocTranslation>, MmSprTranslationError> {
    verify_manifest(spec, entries)?;
    let approved = entries
        .iter()
        .filter(|entry| entry.status.is_poc_build_eligible())
        .map(|entry| {
            let text = normalized_translation(entry)?.ok_or_else(|| {
                MmSprTranslationError::StatusTranslationMismatch(entry.block_id.clone())
            })?;
            Ok(MmSprApprovedPocTranslation {
                block_id: entry.block_id.clone(),
                text: text.to_string(),
                category: entry.category,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if approved.is_empty() {
        return Err(MmSprTranslationError::NoApprovedPocTranslations);
    }
    Ok(approved)
}

/// manifest 순서대로 사람에게 PoC 승인을 받은 항목 전부를 반환한다.
pub fn approved_poc_translations() -> Result<Vec<MmSprApprovedPocTranslation>, MmSprTranslationError>
{
    let entries = mm_spr_translation_entries()?;
    approved_poc_translations_from(MM_SPR_TEXT_ASSET, &entries)
}

/// 사람에게 PoC 승인을 받은 번역문만 반환한다.
pub fn approved_poc_translation(block_id: &str) -> Result<String, MmSprTranslationError> {
    let entries = mm_spr_translation_entries()?;
    let entry = entries
        .iter()
        .find(|entry| entry.block_id == block_id)
        .ok_or_else(|| MmSprTranslationError::UnknownBlockId(block_id.to_string()))?;
    if !entry.status.is_poc_build_eligible() {
        return Err(MmSprTranslationError::NotPocBuildEligible(
            block_id.to_string(),
        ));
    }
    normalized_translation(entry)?
        .map(str::to_string)
        .ok_or_else(|| MmSprTranslationError::NotPocBuildEligible(block_id.to_string()))
}

pub(super) fn escape_tsv_field(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            _ => escaped.push(character),
        }
    }
    escaped
}

pub(super) fn push_tsv_row(output: &mut String, fields: &[String]) {
    for (index, field) in fields.iter().enumerate() {
        if index != 0 {
            output.push('\t');
        }
        output.push_str(field);
    }
    output.push('\n');
}

/// 현재 manifest를 사람이 검토할 수 있는 결정적 TSV 파생물로 만든다.
///
/// 이 출력은 제품 빌드 입력이 아니다. 보호 원문과 상태의 단일 기준은
/// catalog와 이 manifest이며, 사람이 수정한 TSV를 다시 읽는 경로는 없다.
pub fn render_mm_spr_review_tsv() -> Result<String, MmSprTranslationError> {
    let entries = mm_spr_translation_entries()?;
    render_mm_spr_review_tsv_from(MM_SPR_TEXT_ASSET, &entries)
}

fn render_mm_spr_review_tsv_from(
    spec: TextAssetSpec<'_>,
    entries: &[MmSprTranslationEntry],
) -> Result<String, MmSprTranslationError> {
    verify_manifest(spec, entries)?;

    let mut output = String::new();
    output.push_str(
        "schema_version\tarchive_path\tasset_name\tasset_sha256\tblock_id\t\
         block_pixels_sha256\tstatus\tcategory\tprofile_id\tbbox_x\tbbox_y\t\
         bbox_width\tbbox_height\tprotected_source\tko\n",
    );
    for entry in entries {
        let block = spec
            .blocks
            .iter()
            .find(|block| block.id == entry.block_id)
            .ok_or_else(|| MmSprTranslationError::UnknownBlockId(entry.block_id.clone()))?;
        let translation = normalized_translation(entry)?.unwrap_or_default();
        push_tsv_row(
            &mut output,
            &[
                MM_SPR_REVIEW_SCHEMA_VERSION.to_string(),
                spec.archive_paths[0].to_string(),
                spec.name.to_string(),
                spec.expected_decompressed_sha256.to_string(),
                entry.block_id.clone(),
                block.expected_pixels_sha256.to_string(),
                entry.status.label().to_string(),
                entry.category.label().to_string(),
                entry.category.profile().id.to_string(),
                block.bbox.x.to_string(),
                block.bbox.y.to_string(),
                block.bbox.width.to_string(),
                block.bbox.height.to_string(),
                escape_tsv_field(&entry.protected_source),
                escape_tsv_field(translation),
            ],
        );
    }
    Ok(output)
}

#[cfg(test)]
#[path = "mm_spr_translation_tests.rs"]
mod tests;
