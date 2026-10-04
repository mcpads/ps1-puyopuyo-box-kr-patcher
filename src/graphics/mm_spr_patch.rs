//! 셸 `MM_SPR.BTX` 첫 일본어 설명문 PoC.
//!
//! 번역문과 style은 version 관리 입력이다. 범용 baked renderer의 경계를
//! 실제 `BTXB-0002` 원본 계약에 적용하지만, 다른 문장으로 자동 확대하지
//! 않는다.

use std::error::Error;
use std::fmt;

use super::mm_spr_translation::{
    approved_poc_translation, approved_poc_translations, mm_spr_translation_entries,
    MmSprTextCategory, MmSprTranslationError, MmSprTranslationStatus,
    MM_SPR_FIRST_DESCRIPTION_BLOCK_ID,
};
use super::text_catalog::MM_SPR_TEXT_ASSET;
use super::text_patch::{
    patch_baked_text, BakedTextPatchError, BakedTextPatchReport, BakedTextReplacement,
};
use super::text_profile::BakedFontBook;

/// 첫 셸 설명문 PoC 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MmSprPatchError {
    Translation(MmSprTranslationError),
    FontBook(String),
    Patch(BakedTextPatchError),
}

impl fmt::Display for MmSprPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Translation(source) => write!(f, "MM_SPR 번역 manifest 실패: {source}"),
            Self::FontBook(source) => write!(f, "baked font book 구성 실패: {source}"),
            Self::Patch(source) => write!(f, "MM_SPR 설명문 patch 실패: {source}"),
        }
    }
}

impl Error for MmSprPatchError {}

fn patch_replacements(
    original: &[u8],
    replacements: &[BakedTextReplacement<'_>],
) -> Result<(Vec<u8>, BakedTextPatchReport), MmSprPatchError> {
    let fonts = BakedFontBook::with_bundled_maplestory_light()
        .map_err(|source| MmSprPatchError::FontBook(source.to_string()))?;
    patch_baked_text(original, MM_SPR_TEXT_ASSET, &fonts, replacements)
        .map_err(MmSprPatchError::Patch)
}

/// `MM_SPR.BTX`의 첫 일본어 설명문 하나만 한글로 합성한다.
pub fn patch_mm_spr_first_description(
    original: &[u8],
) -> Result<(Vec<u8>, BakedTextPatchReport), MmSprPatchError> {
    let translation = approved_poc_translation(MM_SPR_FIRST_DESCRIPTION_BLOCK_ID)
        .map_err(MmSprPatchError::Translation)?;
    patch_replacements(
        original,
        &[BakedTextReplacement {
            block_id: MM_SPR_FIRST_DESCRIPTION_BLOCK_ID,
            text: &translation,
            profile: MmSprTextCategory::ModeDescriptionOneLine.profile(),
        }],
    )
}

/// manifest에서 PoC build 적격인 `MM_SPR.BTX` 설명문 전부를 합성한다.
pub fn patch_mm_spr_approved_poc_translations(
    original: &[u8],
) -> Result<(Vec<u8>, BakedTextPatchReport), MmSprPatchError> {
    let approved = approved_poc_translations().map_err(MmSprPatchError::Translation)?;
    let replacements = approved
        .iter()
        .map(|entry| BakedTextReplacement {
            block_id: &entry.block_id,
            text: &entry.text,
            profile: entry.category.profile(),
        })
        .collect::<Vec<_>>();
    patch_replacements(original, &replacements)
}

/// 사람 검토용 초안을 포함한 현재 `MM_SPR.BTX` 설명문 전부를 합성한다.
///
/// 이 결과는 bbox·font profile의 정적 수용성만 검증하며 제품 입력이나 승인
/// 산출물이 아니다.
pub fn patch_mm_spr_review_translations(
    original: &[u8],
) -> Result<(Vec<u8>, BakedTextPatchReport), MmSprPatchError> {
    let entries = mm_spr_translation_entries().map_err(MmSprPatchError::Translation)?;
    let replacements = entries
        .iter()
        .filter(|entry| entry.status != MmSprTranslationStatus::PendingHumanTranslation)
        .map(|entry| BakedTextReplacement {
            block_id: &entry.block_id,
            text: &entry.ko,
            profile: entry.category.profile(),
        })
        .collect::<Vec<_>>();
    patch_replacements(original, &replacements)
}
