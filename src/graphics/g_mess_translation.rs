//! `G_MESS.BTX` 결과 문장 21개의 전수 번역 초안과 독립 raster profile.
//!
//! 번역 파일은 보호 원문과 한국어를 같은 행에 둔다. 현재 모든 행은 사람
//! 검토 대기이므로 draft 합성 검증에는 쓸 수 있지만 제품 patch에는 쓸 수
//! 없다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use super::btx_raster_family::proposed_family_for_asset;
use super::btx_raster_surface_policy::flat_background_indices_for_asset;
use super::text_catalog::{is_japanese_translation_target, TextAssetSpec, G_MESS_TEXT_ASSET};
use super::text_patch::{
    patch_baked_text, BakedTextPatchError, BakedTextPatchReport, BakedTextReplacement,
};
use super::text_profile::{
    BakedFontBook, BakedTextEffect, BakedTextProfile, HorizontalAlign, MAPLESTORY_LIGHT_FONT_ID,
};

pub const BTX_RASTER_TRANSLATION_SCHEMA_VERSION: &str = "btx-raster-translation-v1";
pub const GOCHA_RESULT_MESSAGE_FAMILY_ID: &str = "gocha-result-messages";
pub const GOCHA_RESULT_MESSAGE_CATEGORY: &str = "gocha-result-message-line";
pub const GOCHA_RESULT_MESSAGE_PROFILE_ID: &str = "gocha-result-message-maplestory-light-12";

const MANIFEST: &str = include_str!("../../assets/translations/btx/gocha_result_messages.tsv");
const MANIFEST_HEADER: &str = "schema_version\tsemantic_family\tasset_name\tblock_id\t\
                              protected_source\tko\tcategory\tprofile_id\tstatus";
const BODY_RAMP: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];

/// 결과 문장 한 행의 Maplestory Light 제안 profile.
pub static GOCHA_RESULT_MESSAGE_PROFILE: BakedTextProfile<'static> = BakedTextProfile {
    id: GOCHA_RESULT_MESSAGE_PROFILE_ID,
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 12.0,
    baseline: 13,
    line_count: 1,
    line_height: 16,
    tracking: 0,
    align: HorizontalAlign::Left { padding: 0 },
    background: 0,
    body_ramp: &BODY_RAMP,
    effect: BakedTextEffect::None,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtxRasterTranslationStatus {
    DraftHumanReviewRequired,
    Approved,
}

impl BtxRasterTranslationStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DraftHumanReviewRequired => "draft-human-review-required",
            Self::Approved => "approved",
        }
    }

    const fn is_approved(self) -> bool {
        matches!(self, Self::Approved)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxRasterTranslationEntry {
    pub semantic_family: &'static str,
    pub asset_name: &'static str,
    pub block_id: &'static str,
    pub protected_source: &'static str,
    pub ko: &'static str,
    pub category: &'static str,
    pub profile_id: &'static str,
    pub status: BtxRasterTranslationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxRasterTranslationReport {
    pub entries: usize,
    pub drafts: usize,
    pub approved: usize,
    pub product_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GmessTranslationError {
    Manifest(String),
    UnapprovedDrafts { drafts: usize },
    FontBook(String),
    Patch(BakedTextPatchError),
}

impl fmt::Display for GmessTranslationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest(message) => write!(f, "G_MESS 번역 manifest 실패: {message}"),
            Self::UnapprovedDrafts { drafts } => write!(
                f,
                "G_MESS에 사람 미승인 번역 초안이 {drafts}개 있어 제품 patch를 거부함"
            ),
            Self::FontBook(message) => write!(f, "G_MESS font book 구성 실패: {message}"),
            Self::Patch(source) => write!(f, "G_MESS baked text patch 실패: {source}"),
        }
    }
}

impl Error for GmessTranslationError {}

fn error(message: impl Into<String>) -> GmessTranslationError {
    GmessTranslationError::Manifest(message.into())
}

fn has_hangul_syllable(text: &str) -> bool {
    text.chars()
        .any(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
}

fn parse_status(raw: &str) -> Result<BtxRasterTranslationStatus, GmessTranslationError> {
    match raw {
        "draft-human-review-required" => Ok(BtxRasterTranslationStatus::DraftHumanReviewRequired),
        "approved" => Ok(BtxRasterTranslationStatus::Approved),
        _ => Err(error(format!("알 수 없는 번역 상태: {raw}"))),
    }
}

fn parse_manifest(
    raw: &'static str,
) -> Result<Vec<BtxRasterTranslationEntry>, GmessTranslationError> {
    if !raw.ends_with('\n') || raw.contains('\r') {
        return Err(error("manifest는 LF로 끝나야 하며 CR을 포함할 수 없음"));
    }
    let mut lines = raw.lines();
    if lines.next() != Some(MANIFEST_HEADER) {
        return Err(error("manifest header 불일치"));
    }
    lines
        .enumerate()
        .map(|(index, line)| {
            let fields = line.split('\t').collect::<Vec<_>>();
            let [schema, family, asset, block, source, ko, category, profile, status] =
                fields.as_slice()
            else {
                return Err(error(format!("manifest {}행 field 수 불일치", index + 2)));
            };
            if *schema != BTX_RASTER_TRANSLATION_SCHEMA_VERSION {
                return Err(error(format!("manifest {}행 schema 불일치", index + 2)));
            }
            Ok(BtxRasterTranslationEntry {
                semantic_family: family,
                asset_name: asset,
                block_id: block,
                protected_source: source,
                ko,
                category,
                profile_id: profile,
                status: parse_status(status)?,
            })
        })
        .collect()
}

fn verify_manifest(
    raw: &'static str,
    spec: TextAssetSpec<'_>,
) -> Result<(Vec<BtxRasterTranslationEntry>, BtxRasterTranslationReport), GmessTranslationError> {
    let family =
        proposed_family_for_asset(spec.name).map_err(|source| error(source.to_string()))?;
    if family.id != GOCHA_RESULT_MESSAGE_FAMILY_ID {
        return Err(error(format!(
            "{}의 semantic family가 G_MESS 결과 문장과 다름: {}",
            spec.name, family.id
        )));
    }
    verify_manifest_with_profile(
        raw,
        spec,
        GOCHA_RESULT_MESSAGE_FAMILY_ID,
        GOCHA_RESULT_MESSAGE_CATEGORY,
        &GOCHA_RESULT_MESSAGE_PROFILE,
    )
}

fn verify_manifest_with_profile(
    raw: &'static str,
    spec: TextAssetSpec<'_>,
    family_id: &str,
    category: &str,
    profile: &BakedTextProfile<'_>,
) -> Result<(Vec<BtxRasterTranslationEntry>, BtxRasterTranslationReport), GmessTranslationError> {
    let entries = parse_manifest(raw)?;
    let mut ids = BTreeSet::new();
    let mut approved = 0usize;
    for entry in &entries {
        if !ids.insert(entry.block_id) {
            return Err(error(format!("번역 block ID 중복: {}", entry.block_id)));
        }
        if entry.semantic_family != family_id
            || entry.asset_name != spec.name
            || entry.category != category
            || entry.profile_id != profile.id
        {
            return Err(error(format!(
                "{}의 family·asset·profile 연결 불일치",
                entry.block_id
            )));
        }
        let block = spec
            .blocks
            .iter()
            .find(|block| block.id == entry.block_id)
            .ok_or_else(|| error(format!("catalog에 없는 번역 block: {}", entry.block_id)))?;
        if block.source != entry.protected_source {
            return Err(error(format!("{}의 보호 원문 불일치", entry.block_id)));
        }
        if !is_japanese_translation_target(block.source)
            || entry.ko.is_empty()
            || entry.ko.trim() != entry.ko
            || !has_hangul_syllable(entry.ko)
            || is_japanese_translation_target(entry.ko)
            || entry.ko.contains(['\t', '\n', '\r'])
            || block.bbox.height != profile.line_height
        {
            return Err(error(format!(
                "{}의 번역·행·bbox 계약 불일치",
                entry.block_id
            )));
        }
        approved += usize::from(entry.status.is_approved());
    }
    let catalog_ids = spec
        .blocks
        .iter()
        .filter(|block| is_japanese_translation_target(block.source))
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    if ids != catalog_ids {
        return Err(error("G_MESS 일본어 catalog와 번역 block 집합 불일치"));
    }
    let drafts = entries.len() - approved;
    let product_eligible = approved == entries.len();
    Ok((
        entries,
        BtxRasterTranslationReport {
            entries: ids.len(),
            drafts,
            approved,
            product_eligible,
        },
    ))
}

pub fn gocha_result_message_entries(
) -> Result<Vec<BtxRasterTranslationEntry>, GmessTranslationError> {
    verify_manifest(MANIFEST, G_MESS_TEXT_ASSET).map(|(entries, _)| entries)
}

pub fn verify_gocha_result_message_manifest(
) -> Result<BtxRasterTranslationReport, GmessTranslationError> {
    verify_manifest(MANIFEST, G_MESS_TEXT_ASSET).map(|(_, report)| report)
}

fn patch_entries(
    original: &[u8],
    entries: &[BtxRasterTranslationEntry],
) -> Result<(Vec<u8>, BakedTextPatchReport), GmessTranslationError> {
    let backgrounds = flat_background_indices_for_asset(G_MESS_TEXT_ASSET)
        .map_err(|source| error(source.to_string()))?;
    if backgrounds.len() != entries.len()
        || entries.iter().any(|entry| {
            backgrounds.get(entry.block_id) != Some(&GOCHA_RESULT_MESSAGE_PROFILE.background)
        })
    {
        return Err(error("G_MESS profile background와 surface 정책 불일치"));
    }
    let fonts = BakedFontBook::with_bundled_maplestory_light()
        .map_err(|source| GmessTranslationError::FontBook(source.to_string()))?;
    let replacements = entries
        .iter()
        .map(|entry| BakedTextReplacement {
            block_id: entry.block_id,
            text: entry.ko,
            profile: &GOCHA_RESULT_MESSAGE_PROFILE,
        })
        .collect::<Vec<_>>();
    patch_baked_text(original, G_MESS_TEXT_ASSET, &fonts, &replacements)
        .map_err(GmessTranslationError::Patch)
}

/// 사람 승인 전인 21개 초안의 fit과 실제 asset 합성만 검증한다.
pub fn patch_gocha_result_message_drafts(
    original: &[u8],
) -> Result<(Vec<u8>, BakedTextPatchReport), GmessTranslationError> {
    let (entries, _) = verify_manifest(MANIFEST, G_MESS_TEXT_ASSET)?;
    patch_entries(original, &entries)
}

/// 제품 경로는 모든 행이 승인되기 전까지 실패 폐쇄한다.
pub fn patch_gocha_result_messages(
    original: &[u8],
) -> Result<(Vec<u8>, BakedTextPatchReport), GmessTranslationError> {
    let (entries, report) = verify_manifest(MANIFEST, G_MESS_TEXT_ASSET)?;
    require_product_eligible(report)?;
    patch_entries(original, &entries)
}

fn require_product_eligible(
    report: BtxRasterTranslationReport,
) -> Result<(), GmessTranslationError> {
    if report.product_eligible {
        Ok(())
    } else {
        Err(GmessTranslationError::UnapprovedDrafts {
            drafts: report.drafts,
        })
    }
}

#[cfg(test)]
#[path = "g_mess_translation_tests.rs"]
mod tests;
