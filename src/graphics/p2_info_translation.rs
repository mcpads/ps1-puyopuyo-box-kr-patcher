//! `P2_INFO4/5.BTX` fixed-cell 설명 page의 원문 대조형 번역 manifest.
//!
//! 원문과 geometry는 typed catalog에 대조하고, 번역 승인과 공유 raster
//! profile 승인을 분리한다. 두 승인이 끝나기 전에는 제품 입력이 아니다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::font::P2_INFO_PAGE_DRAFT_RASTER_PROFILE;

use super::text_catalog::{
    is_japanese_translation_target, TextAssetSpec, P2_INFO4_TEXT_ASSET, P2_INFO5_TEXT_ASSET,
};

pub const P2_INFO_TRANSLATION_SCHEMA_VERSION: &str = "p2-info-page-translation-v1";

const MANIFEST: &str = include_str!("../../assets/translations/btx/p2_info_pages.tsv");
const HEADER: &str = "schema_version\tcategory\tarchive_paths\tasset_name\tasset_sha256\t\
                      block_id\tblock_pixels_sha256\tbbox_x\tbbox_y\tbbox_width\tbbox_height\t\
                      protected_source\tko\tprofile_id\tprofile_status\ttranslation_status";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P2InfoTranslationStatus {
    PendingHumanTranslation,
    DraftHumanReviewRequired,
    Approved,
}

impl P2InfoTranslationStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PendingHumanTranslation => "pending-human-translation",
            Self::DraftHumanReviewRequired => "draft-human-review-required",
            Self::Approved => "approved",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P2InfoProfileStatus {
    DraftHumanReviewRequired,
    Approved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2InfoTranslationEntry {
    pub category: String,
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
    pub ko: String,
    pub profile_id: String,
    pub profile_status: P2InfoProfileStatus,
    pub translation_status: P2InfoTranslationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct P2InfoTranslationReport {
    pub entries: usize,
    pub pending_translations: usize,
    pub draft_translations: usize,
    pub approved_translations: usize,
    pub profile_approved: bool,
    pub product_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2InfoTranslationError(String);

impl fmt::Display for P2InfoTranslationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P2 INFO page 번역 manifest 실패: {}", self.0)
    }
}

impl Error for P2InfoTranslationError {}

fn error(message: impl Into<String>) -> P2InfoTranslationError {
    P2InfoTranslationError(message.into())
}

fn parse_usize(field: &str, value: &str, line: usize) -> Result<usize, P2InfoTranslationError> {
    value
        .parse()
        .map_err(|_| error(format!("{line}행 {field}가 usize가 아님: {value:?}")))
}

fn unescape(value: &str, line: usize) -> Result<String, P2InfoTranslationError> {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let escaped = characters
            .next()
            .ok_or_else(|| error(format!("{line}행 TSV escape가 끝에서 잘림")))?;
        match escaped {
            '\\' => output.push('\\'),
            't' => output.push('\t'),
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            _ => {
                return Err(error(format!(
                    "{line}행 알 수 없는 TSV escape: \\{escaped}"
                )))
            }
        }
    }
    Ok(output)
}

fn parse_translation_status(
    value: &str,
    line: usize,
) -> Result<P2InfoTranslationStatus, P2InfoTranslationError> {
    match value {
        "pending-human-translation" => Ok(P2InfoTranslationStatus::PendingHumanTranslation),
        "draft-human-review-required" => Ok(P2InfoTranslationStatus::DraftHumanReviewRequired),
        "approved" => Ok(P2InfoTranslationStatus::Approved),
        _ => Err(error(format!("{line}행 번역 상태 미지원: {value}"))),
    }
}

fn parse_profile_status(
    value: &str,
    line: usize,
) -> Result<P2InfoProfileStatus, P2InfoTranslationError> {
    match value {
        "draft-profile-human-review-required" => Ok(P2InfoProfileStatus::DraftHumanReviewRequired),
        "approved" => Ok(P2InfoProfileStatus::Approved),
        _ => Err(error(format!("{line}행 profile 상태 미지원: {value}"))),
    }
}

fn has_hangul_syllable(text: &str) -> bool {
    text.chars()
        .any(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
}

fn verify_translation(
    entry: &P2InfoTranslationEntry,
    line: usize,
) -> Result<(), P2InfoTranslationError> {
    if entry.translation_status == P2InfoTranslationStatus::PendingHumanTranslation {
        if !entry.ko.is_empty() {
            return Err(error(format!("{line}행 대기 번역에 한국어가 있음")));
        }
        return Ok(());
    }
    if entry.ko.is_empty()
        || entry.ko.trim() != entry.ko
        || !has_hangul_syllable(&entry.ko)
        || is_japanese_translation_target(&entry.ko)
    {
        return Err(error(format!("{line}행 번역문 내용 계약 불일치")));
    }
    let lines = entry.ko.split('\n').collect::<Vec<_>>();
    if lines.len() > 16 || lines.iter().any(|row| row.chars().count() > 16) {
        return Err(error(format!("{line}행 번역문이 16x16 page 범위를 넘음")));
    }
    Ok(())
}

fn parse_manifest(raw: &str) -> Result<Vec<P2InfoTranslationEntry>, P2InfoTranslationError> {
    if !raw.ends_with('\n') || raw.contains('\r') {
        return Err(error("manifest는 LF로 끝나야 하며 CR을 포함할 수 없음"));
    }
    let mut lines = raw.lines();
    if lines.next() != Some(HEADER) {
        return Err(error("manifest header 불일치"));
    }
    lines
        .enumerate()
        .map(|(index, row)| {
            let line = index + 2;
            let fields = row.split('\t').collect::<Vec<_>>();
            let [schema, category, archives, asset, asset_sha, block, block_sha, x, y, width, height, source, ko, profile_id, profile_status, translation_status] =
                fields.as_slice()
            else {
                return Err(error(format!("{line}행 field 수 불일치")));
            };
            if *schema != P2_INFO_TRANSLATION_SCHEMA_VERSION {
                return Err(error(format!("{line}행 schema 불일치")));
            }
            let entry = P2InfoTranslationEntry {
                category: (*category).to_string(),
                archive_paths: unescape(archives, line)?,
                asset_name: (*asset).to_string(),
                asset_sha256: (*asset_sha).to_string(),
                block_id: (*block).to_string(),
                block_pixels_sha256: (*block_sha).to_string(),
                bbox_x: parse_usize("bbox_x", x, line)?,
                bbox_y: parse_usize("bbox_y", y, line)?,
                bbox_width: parse_usize("bbox_width", width, line)?,
                bbox_height: parse_usize("bbox_height", height, line)?,
                protected_source: unescape(source, line)?,
                ko: unescape(ko, line)?,
                profile_id: (*profile_id).to_string(),
                profile_status: parse_profile_status(profile_status, line)?,
                translation_status: parse_translation_status(translation_status, line)?,
            };
            verify_translation(&entry, line)?;
            Ok(entry)
        })
        .collect()
}

fn verify_manifest(
    raw: &str,
) -> Result<(Vec<P2InfoTranslationEntry>, P2InfoTranslationReport), P2InfoTranslationError> {
    verify_manifest_against(
        raw,
        &[P2_INFO4_TEXT_ASSET, P2_INFO5_TEXT_ASSET],
        P2_INFO_PAGE_DRAFT_RASTER_PROFILE.id,
    )
}

fn verify_manifest_against(
    raw: &str,
    specs: &[TextAssetSpec<'_>],
    profile_id: &str,
) -> Result<(Vec<P2InfoTranslationEntry>, P2InfoTranslationReport), P2InfoTranslationError> {
    let entries = parse_manifest(raw)?;
    if entries.len() != specs.len() {
        return Err(error(format!(
            "번역 분모 불일치: manifest={}, catalog={}",
            entries.len(),
            specs.len()
        )));
    }
    let mut ids = BTreeSet::new();
    let mut categories = BTreeSet::new();
    let mut pending_translations = 0usize;
    let mut draft_translations = 0usize;
    let mut approved_translations = 0usize;
    let mut previous_id = None;
    for entry in &entries {
        if previous_id.is_some_and(|previous| previous >= entry.block_id.as_str()) {
            return Err(error("manifest가 block ID canonical 순서가 아님"));
        }
        previous_id = Some(entry.block_id.as_str());
        if !ids.insert(entry.block_id.as_str()) || !categories.insert(entry.category.as_str()) {
            return Err(error("block ID 또는 category 중복"));
        }
        let spec = specs
            .iter()
            .find(|spec| spec.name == entry.asset_name)
            .ok_or_else(|| error(format!("catalog에 없는 asset: {}", entry.asset_name)))?;
        verify_entry_source(entry, *spec)?;
        if entry.profile_id != profile_id {
            return Err(error(format!("{}의 profile ID 불일치", entry.block_id)));
        }
        match entry.translation_status {
            P2InfoTranslationStatus::PendingHumanTranslation => pending_translations += 1,
            P2InfoTranslationStatus::DraftHumanReviewRequired => draft_translations += 1,
            P2InfoTranslationStatus::Approved => approved_translations += 1,
        }
    }
    if entries
        .windows(2)
        .any(|pair| pair[0].profile_status != pair[1].profile_status)
    {
        return Err(error("공유 INFO page profile 승인 상태가 행마다 다름"));
    }
    let profile_approved = entries
        .first()
        .is_some_and(|entry| entry.profile_status == P2InfoProfileStatus::Approved);
    let report = P2InfoTranslationReport {
        entries: entries.len(),
        pending_translations,
        draft_translations,
        approved_translations,
        profile_approved,
        product_eligible: approved_translations == entries.len() && profile_approved,
    };
    Ok((entries, report))
}

fn verify_entry_source(
    entry: &P2InfoTranslationEntry,
    spec: TextAssetSpec<'_>,
) -> Result<(), P2InfoTranslationError> {
    let [block] = spec.blocks else {
        return Err(error(format!("{}의 catalog block 분모 불일치", spec.name)));
    };
    if entry.archive_paths != spec.archive_paths.join("\n")
        || entry.asset_sha256 != spec.expected_decompressed_sha256
        || entry.block_id != block.id
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
        || entry.protected_source != block.source
    {
        return Err(error(format!("{}의 보호 원본 필드 drift", entry.block_id)));
    }
    Ok(())
}

pub fn verify_p2_info_translation_manifest(
) -> Result<P2InfoTranslationReport, P2InfoTranslationError> {
    verify_manifest(MANIFEST).map(|(_, report)| report)
}

pub fn p2_info_translation_entries() -> Result<Vec<P2InfoTranslationEntry>, P2InfoTranslationError>
{
    verify_manifest(MANIFEST).map(|(entries, _)| entries)
}

#[cfg(test)]
#[path = "p2_info_translation_tests.rs"]
mod tests;
