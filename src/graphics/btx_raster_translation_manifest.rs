//! 일반 raster BTX의 원문 대조형 번역·category profile 입력.
//!
//! `G_MESS.BTX`는 전용 manifest와 profile을 계속 단일 기준으로 사용한다.
//! 이 모듈은 `G_MESS`를 제외한 218개 block과 76개 family/profile 후보 조합을
//! 검증한다. 이 가운데 `BOXOPT` 46개는 문구 rectangle을 고르는 sparse
//! selector atlas이며, code→glyph font와 같은 저장 방식으로 취급하지 않는다.
//! 자동으로 만든 family·profile 후보는 사람 승인을 뜻하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::btx_patch_strategy::{
    verify_box_btx_patch_strategy, BtxPatchStrategyError, BtxRasterProfileCandidateGroup,
    BtxRasterProfileCandidateMember,
};
use super::g_mess_translation::{
    verify_gocha_result_message_manifest, BtxRasterTranslationReport, GmessTranslationError,
    GOCHA_RESULT_MESSAGE_FAMILY_ID,
};
use super::text_catalog::is_japanese_translation_target;
use super::text_profile::{BakedTextEffect, BakedTextProfile, HorizontalAlign};

pub const BTX_RASTER_GENERAL_TRANSLATION_SCHEMA_VERSION: &str = "btx-raster-general-translation-v1";
pub const BTX_RASTER_CATEGORY_PROFILE_SCHEMA_VERSION: &str = "btx-raster-category-profile-v1";
const TRANSLATION_MANIFEST: &str = include_str!("../../assets/translations/btx/raster_general.tsv");
const PROFILE_MANIFEST: &str = include_str!("../../assets/translations/btx/raster_profiles.tsv");
const TRANSLATION_HEADER: &str = "schema_version\tsemantic_family\tprofile_candidate_index\t\
                                  archive_paths\tasset_name\tasset_sha256\tblock_id\t\
                                  block_pixels_sha256\tbbox_x\tbbox_y\tbbox_width\tbbox_height\t\
                                  protected_source\tko\tstatus";
const PROFILE_HEADER: &str = "schema_version\tsemantic_family\tprofile_candidate_index\t\
                              pixel_format\tbbox_height\tsource_lines\tpalette_indices\t\
                              category_id\tprofile_id\tfont_id\tfont_size\tbaseline\tline_count\t\
                              line_height\ttracking\talign\tbackground\tbody_ramp\teffect\tstatus";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneralRasterTranslationStatus {
    PendingHumanTranslation,
    DraftHumanReviewRequired,
    Approved,
    ScopeExcludedProtectedNoWrite,
}

impl GeneralRasterTranslationStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PendingHumanTranslation => "pending-human-translation",
            Self::DraftHumanReviewRequired => "draft-human-review-required",
            Self::Approved => "approved",
            Self::ScopeExcludedProtectedNoWrite => "scope-excluded-protected-no-write",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralRasterTranslationEntry {
    pub semantic_family: String,
    pub profile_candidate_index: usize,
    pub category_id: Option<String>,
    pub profile_id: Option<String>,
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
    pub status: GeneralRasterTranslationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneralRasterProfileStatus {
    CandidateHumanApprovalRequired,
    DevelopmentDraftHumanReviewRequired,
    Approved,
    ScopeExcludedProtectedNoWrite,
}

impl GeneralRasterProfileStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::CandidateHumanApprovalRequired => "profile-candidate-human-approval-required",
            Self::DevelopmentDraftHumanReviewRequired => "development-draft-human-review-required",
            Self::Approved => "approved",
            Self::ScopeExcludedProtectedNoWrite => "scope-excluded-protected-no-write",
        }
    }
}

/// 사람 승인 여부와 분리해 개발 합성에 사용할 수 있는 명시적 raster 설정.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneralRasterProfileConfiguration {
    pub category_id: String,
    pub profile_id: String,
    pub font_id: String,
    pub font_size: f32,
    pub baseline: i32,
    pub line_count: usize,
    pub line_height: usize,
    pub tracking: usize,
    pub align: HorizontalAlign,
    pub background: u8,
    pub body_ramp: Vec<u8>,
    pub effect: BakedTextEffect,
}

impl GeneralRasterProfileConfiguration {
    pub fn baked_text_profile(&self) -> BakedTextProfile<'_> {
        BakedTextProfile {
            id: &self.profile_id,
            font_id: &self.font_id,
            font_size: self.font_size,
            baseline: self.baseline,
            line_count: self.line_count,
            line_height: self.line_height,
            tracking: self.tracking,
            align: self.align,
            background: self.background,
            body_ramp: &self.body_ramp,
            effect: self.effect,
        }
    }
}

/// 보호 geometry와 선택한 개발·승인 설정을 결속한 profile 한 행.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneralRasterProfileAssignment {
    pub semantic_family: String,
    pub profile_candidate_index: usize,
    pub pixel_format: String,
    pub bbox_height: usize,
    pub source_lines: usize,
    pub palette_indices: Vec<u8>,
    pub configuration: Option<GeneralRasterProfileConfiguration>,
    pub status: GeneralRasterProfileStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RasterProfileEntry {
    semantic_family: String,
    profile_candidate_index: usize,
    pixel_format: String,
    bbox_height: usize,
    source_lines: usize,
    palette_indices: Vec<u8>,
    category_id: String,
    profile_id: String,
    font_id: String,
    font_size: String,
    baseline: String,
    line_count: String,
    line_height: String,
    tracking: String,
    align: String,
    background: String,
    body_ramp: String,
    effect: String,
    status: GeneralRasterProfileStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralRasterManifestReport {
    pub translation_blocks: usize,
    pub translation_assets: usize,
    pub profile_assignments: usize,
    pub semantic_families: usize,
    pub pending_translations: usize,
    pub draft_translations: usize,
    pub approved_translations: usize,
    pub scope_excluded_translations: usize,
    pub pending_profiles: usize,
    pub development_profiles: usize,
    pub approved_profiles: usize,
    pub scope_excluded_profiles: usize,
    pub delegated_g_mess_blocks: usize,
    pub covered_raster_blocks: usize,
    pub all_in_scope_human_approved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralRasterManifestError(String);

impl fmt::Display for GeneralRasterManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "일반 raster BTX manifest 실패: {}", self.0)
    }
}

impl Error for GeneralRasterManifestError {}

impl From<BtxPatchStrategyError> for GeneralRasterManifestError {
    fn from(source: BtxPatchStrategyError) -> Self {
        Self(format!("patch strategy 검증 실패: {source}"))
    }
}

impl From<GmessTranslationError> for GeneralRasterManifestError {
    fn from(source: GmessTranslationError) -> Self {
        Self(format!("G_MESS 위임 manifest 검증 실패: {source}"))
    }
}

fn error(message: impl Into<String>) -> GeneralRasterManifestError {
    GeneralRasterManifestError(message.into())
}

fn parse_usize(field: &str, value: &str, line: usize) -> Result<usize, GeneralRasterManifestError> {
    value
        .parse()
        .map_err(|_| error(format!("{line}행 {field}가 usize가 아님: {value:?}")))
}

fn unescape_tsv_field(value: &str, line: usize) -> Result<String, GeneralRasterManifestError> {
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

fn verify_file_shape(
    raw: &str,
    header: &str,
    label: &str,
) -> Result<(), GeneralRasterManifestError> {
    if !raw.ends_with('\n') || raw.contains('\r') {
        return Err(error(format!(
            "{label}는 LF로 끝나야 하며 CR을 포함할 수 없음"
        )));
    }
    if raw.lines().next() != Some(header) {
        return Err(error(format!("{label} header 불일치")));
    }
    Ok(())
}

fn parse_translation_status(
    value: &str,
    line: usize,
) -> Result<GeneralRasterTranslationStatus, GeneralRasterManifestError> {
    match value {
        "pending-human-translation" => Ok(GeneralRasterTranslationStatus::PendingHumanTranslation),
        "draft-human-review-required" => {
            Ok(GeneralRasterTranslationStatus::DraftHumanReviewRequired)
        }
        "approved" => Ok(GeneralRasterTranslationStatus::Approved),
        "scope-excluded-protected-no-write" => {
            Ok(GeneralRasterTranslationStatus::ScopeExcludedProtectedNoWrite)
        }
        _ => Err(error(format!("{line}행 번역 상태 미지원: {value}"))),
    }
}

fn has_hangul_syllable(text: &str) -> bool {
    text.chars()
        .any(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
}

fn parse_translation_manifest(
    raw: &str,
) -> Result<Vec<GeneralRasterTranslationEntry>, GeneralRasterManifestError> {
    verify_file_shape(raw, TRANSLATION_HEADER, "번역 manifest")?;
    raw.lines()
        .skip(1)
        .enumerate()
        .map(|(index, row)| {
            let line = index + 2;
            let fields = row.split('\t').collect::<Vec<_>>();
            let [schema, family, candidate, archives, asset, asset_sha, block, block_sha, x, y, width, height, source, ko, status] =
                fields.as_slice()
            else {
                return Err(error(format!("{line}행 번역 field 수 불일치")));
            };
            if *schema != BTX_RASTER_GENERAL_TRANSLATION_SCHEMA_VERSION {
                return Err(error(format!("{line}행 번역 schema 불일치")));
            }
            let entry = GeneralRasterTranslationEntry {
                semantic_family: (*family).to_string(),
                profile_candidate_index: parse_usize("profile_candidate_index", candidate, line)?,
                category_id: None,
                profile_id: None,
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
                ko: unescape_tsv_field(ko, line)?,
                status: parse_translation_status(status, line)?,
            };
            match entry.status {
                GeneralRasterTranslationStatus::PendingHumanTranslation if !entry.ko.is_empty() => {
                    return Err(error(format!("{line}행 대기 번역에 한국어가 있음")));
                }
                GeneralRasterTranslationStatus::DraftHumanReviewRequired
                | GeneralRasterTranslationStatus::Approved
                | GeneralRasterTranslationStatus::ScopeExcludedProtectedNoWrite
                    if entry.ko.is_empty()
                        || entry.ko.trim() != entry.ko
                        || !has_hangul_syllable(&entry.ko)
                        || is_japanese_translation_target(&entry.ko) =>
                {
                    return Err(error(format!("{line}행 번역문 내용 계약 불일치")));
                }
                _ => {}
            }
            Ok(entry)
        })
        .collect()
}

fn parse_profile_status(
    value: &str,
    line: usize,
) -> Result<GeneralRasterProfileStatus, GeneralRasterManifestError> {
    match value {
        "profile-candidate-human-approval-required" => {
            Ok(GeneralRasterProfileStatus::CandidateHumanApprovalRequired)
        }
        "development-draft-human-review-required" => {
            Ok(GeneralRasterProfileStatus::DevelopmentDraftHumanReviewRequired)
        }
        "approved" => Ok(GeneralRasterProfileStatus::Approved),
        "scope-excluded-protected-no-write" => {
            Ok(GeneralRasterProfileStatus::ScopeExcludedProtectedNoWrite)
        }
        _ => Err(error(format!("{line}행 profile 상태 미지원: {value}"))),
    }
}

fn parse_palette_indices(value: &str, line: usize) -> Result<Vec<u8>, GeneralRasterManifestError> {
    let (exclude, values) = value
        .strip_prefix("all-except:")
        .map_or((false, value), |values| (true, values));
    let mut listed = BTreeSet::new();
    if !values.is_empty() {
        for value in values.split(',') {
            let index = value.parse::<u8>().map_err(|_| {
                error(format!(
                    "{line}행 palette_indices가 u8 목록 또는 all-except 목록이 아님"
                ))
            })?;
            if !listed.insert(index) {
                return Err(error(format!(
                    "{line}행 palette_indices index 중복: {index}"
                )));
            }
        }
    }
    let canonical = listed
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    if canonical != values {
        return Err(error(format!(
            "{line}행 palette_indices가 canonical 오름차순이 아님"
        )));
    }
    Ok(if exclude {
        (0u16..=255)
            .map(|index| index as u8)
            .filter(|index| !listed.contains(index))
            .collect()
    } else {
        listed.into_iter().collect()
    })
}

fn parse_profile_manifest(
    raw: &str,
) -> Result<Vec<RasterProfileEntry>, GeneralRasterManifestError> {
    verify_file_shape(raw, PROFILE_HEADER, "profile manifest")?;
    raw.lines()
        .skip(1)
        .enumerate()
        .map(|(index, row)| {
            let line = index + 2;
            let fields = row.split('\t').collect::<Vec<_>>();
            let [schema, family, candidate, pixel_format, bbox_height, source_lines, palette, category_id, profile_id, font_id, font_size, baseline, line_count, line_height, tracking, align, background, body_ramp, effect, status] =
                fields.as_slice()
            else {
                return Err(error(format!("{line}행 profile field 수 불일치")));
            };
            if *schema != BTX_RASTER_CATEGORY_PROFILE_SCHEMA_VERSION {
                return Err(error(format!("{line}행 profile schema 불일치")));
            }
            let entry = RasterProfileEntry {
                semantic_family: (*family).to_string(),
                profile_candidate_index: parse_usize("profile_candidate_index", candidate, line)?,
                pixel_format: (*pixel_format).to_string(),
                bbox_height: parse_usize("bbox_height", bbox_height, line)?,
                source_lines: parse_usize("source_lines", source_lines, line)?,
                palette_indices: parse_palette_indices(palette, line)?,
                category_id: (*category_id).to_string(),
                profile_id: (*profile_id).to_string(),
                font_id: (*font_id).to_string(),
                font_size: (*font_size).to_string(),
                baseline: (*baseline).to_string(),
                line_count: (*line_count).to_string(),
                line_height: (*line_height).to_string(),
                tracking: (*tracking).to_string(),
                align: (*align).to_string(),
                background: (*background).to_string(),
                body_ramp: (*body_ramp).to_string(),
                effect: (*effect).to_string(),
                status: parse_profile_status(status, line)?,
            };
            verify_profile_configuration(&entry, line)?;
            Ok(entry)
        })
        .collect()
}

fn verify_profile_configuration(
    entry: &RasterProfileEntry,
    line: usize,
) -> Result<(), GeneralRasterManifestError> {
    let configurable = [
        entry.category_id.as_str(),
        entry.profile_id.as_str(),
        entry.font_id.as_str(),
        entry.font_size.as_str(),
        entry.baseline.as_str(),
        entry.line_count.as_str(),
        entry.line_height.as_str(),
        entry.tracking.as_str(),
        entry.align.as_str(),
        entry.background.as_str(),
        entry.body_ramp.as_str(),
        entry.effect.as_str(),
    ];
    if matches!(
        entry.status,
        GeneralRasterProfileStatus::CandidateHumanApprovalRequired
            | GeneralRasterProfileStatus::ScopeExcludedProtectedNoWrite
    ) {
        if configurable.iter().any(|value| !value.is_empty()) {
            return Err(error(format!("{line}행 설정 없는 profile에 설정값이 있음")));
        }
        return Ok(());
    }
    if configurable.iter().any(|value| value.is_empty()) {
        return Err(error(format!("{line}행 설정된 profile의 설정값이 누락됨")));
    }
    let font_size = entry
        .font_size
        .parse::<f32>()
        .map_err(|_| error(format!("{line}행 font_size가 숫자가 아님")))?;
    if !font_size.is_finite() || font_size <= 0.0 {
        return Err(error(format!("{line}행 font_size 범위 불일치")));
    }
    entry
        .baseline
        .parse::<i32>()
        .map_err(|_| error(format!("{line}행 baseline이 i32가 아님")))?;
    let line_count = parse_usize("line_count", &entry.line_count, line)?;
    let line_height = parse_usize("line_height", &entry.line_height, line)?;
    parse_usize("tracking", &entry.tracking, line)?;
    let background = entry
        .background
        .parse::<u8>()
        .map_err(|_| error(format!("{line}행 background가 u8이 아님")))?;
    if line_count == 0
        || line_height == 0
        || line_count
            .checked_mul(line_height)
            .is_none_or(|height| height > entry.bbox_height)
    {
        return Err(error(format!(
            "{line}행 line_count·line_height가 bbox와 맞지 않음"
        )));
    }
    parse_align(&entry.align, line)?;
    let effect = parse_effect(&entry.effect, line)?;
    let effect_palette = match effect {
        BakedTextEffect::None => None,
        BakedTextEffect::Shadow { palette_index, .. }
        | BakedTextEffect::Outline { palette_index } => Some(palette_index),
    };
    let palette = entry
        .palette_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if !palette.contains(&background) {
        return Err(error(format!("{line}행 background가 원본 palette 밖임")));
    }
    let mut ramp = BTreeSet::new();
    for value in entry.body_ramp.split(',') {
        let index = value
            .parse::<u8>()
            .map_err(|_| error(format!("{line}행 body_ramp가 u8 목록이 아님")))?;
        if !palette.contains(&index) {
            return Err(error(format!("{line}행 body_ramp가 원본 palette 밖임")));
        }
        if !ramp.insert(index) {
            return Err(error(format!("{line}행 body_ramp index 중복")));
        }
    }
    if effect_palette.is_some_and(|index| !palette.contains(&index)) {
        return Err(error(format!("{line}행 effect가 원본 palette 밖임")));
    }
    Ok(())
}

fn public_profile_assignment(
    entry: RasterProfileEntry,
    line: usize,
) -> Result<GeneralRasterProfileAssignment, GeneralRasterManifestError> {
    let configuration = if matches!(
        entry.status,
        GeneralRasterProfileStatus::DevelopmentDraftHumanReviewRequired
            | GeneralRasterProfileStatus::Approved
    ) {
        Some(GeneralRasterProfileConfiguration {
            category_id: entry.category_id,
            profile_id: entry.profile_id,
            font_id: entry.font_id,
            font_size: entry
                .font_size
                .parse::<f32>()
                .map_err(|_| error(format!("{line}행 font_size가 숫자가 아님")))?,
            baseline: entry
                .baseline
                .parse::<i32>()
                .map_err(|_| error(format!("{line}행 baseline이 i32가 아님")))?,
            line_count: parse_usize("line_count", &entry.line_count, line)?,
            line_height: parse_usize("line_height", &entry.line_height, line)?,
            tracking: parse_usize("tracking", &entry.tracking, line)?,
            align: parse_align(&entry.align, line)?,
            background: entry
                .background
                .parse::<u8>()
                .map_err(|_| error(format!("{line}행 background가 u8이 아님")))?,
            body_ramp: parse_palette_indices(&entry.body_ramp, line)?,
            effect: parse_effect(&entry.effect, line)?,
        })
    } else {
        None
    };
    Ok(GeneralRasterProfileAssignment {
        semantic_family: entry.semantic_family,
        profile_candidate_index: entry.profile_candidate_index,
        pixel_format: entry.pixel_format,
        bbox_height: entry.bbox_height,
        source_lines: entry.source_lines,
        palette_indices: entry.palette_indices,
        configuration,
        status: entry.status,
    })
}

fn parse_align(value: &str, line: usize) -> Result<HorizontalAlign, GeneralRasterManifestError> {
    if value == "center" {
        return Ok(HorizontalAlign::Center);
    }
    let Some((kind, padding)) = value.split_once(':') else {
        return Err(error(format!("{line}행 align 형식 불일치")));
    };
    let padding = padding
        .parse::<usize>()
        .map_err(|_| error(format!("{line}행 align 형식 불일치")))?;
    match kind {
        "left" => Ok(HorizontalAlign::Left { padding }),
        "right" => Ok(HorizontalAlign::Right { padding }),
        _ => Err(error(format!("{line}행 align 형식 불일치"))),
    }
}

fn parse_effect(value: &str, line: usize) -> Result<BakedTextEffect, GeneralRasterManifestError> {
    if value == "none" {
        return Ok(BakedTextEffect::None);
    }
    let parts = value.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        ["outline", palette] => palette
            .parse::<u8>()
            .map(|palette_index| BakedTextEffect::Outline { palette_index })
            .map_err(|_| ()),
        ["shadow", x, y, palette] => {
            let offset_x = x.parse::<i32>();
            let offset_y = y.parse::<i32>();
            let palette_index = palette.parse::<u8>();
            match (offset_x, offset_y, palette_index) {
                (Ok(offset_x), Ok(offset_y), Ok(palette_index)) => Ok(BakedTextEffect::Shadow {
                    offset_x,
                    offset_y,
                    palette_index,
                }),
                _ => Err(()),
            }
        }
        _ => Err(()),
    }
    .map_err(|()| error(format!("{line}행 effect 형식 불일치")))
}

fn verify_manifests(
    translation_raw: &str,
    profile_raw: &str,
) -> Result<
    (
        Vec<GeneralRasterTranslationEntry>,
        GeneralRasterManifestReport,
    ),
    GeneralRasterManifestError,
> {
    let strategy = verify_box_btx_patch_strategy()?;
    let g_mess = verify_gocha_result_message_manifest()?;
    let result = verify_manifests_against(
        translation_raw,
        profile_raw,
        &strategy.profile_candidate_groups,
        &strategy.profile_candidate_members,
        g_mess,
        &["PCOMI01.BTX", "PCOMI02.BTX"],
    )?;
    if result.1.covered_raster_blocks != strategy.remaining_raster_blocks {
        return Err(error(format!(
            "patch strategy와 관리 raster 분모 불일치: strategy={}, managed={}",
            strategy.remaining_raster_blocks, result.1.covered_raster_blocks
        )));
    }
    Ok(result)
}

fn verify_manifests_against(
    translation_raw: &str,
    profile_raw: &str,
    profile_candidate_groups: &[BtxRasterProfileCandidateGroup],
    profile_candidate_members: &[BtxRasterProfileCandidateMember],
    g_mess: BtxRasterTranslationReport,
    scope_excluded_assets: &[&str],
) -> Result<
    (
        Vec<GeneralRasterTranslationEntry>,
        GeneralRasterManifestReport,
    ),
    GeneralRasterManifestError,
> {
    let mut entries = parse_translation_manifest(translation_raw)?;
    let profiles = parse_profile_manifest(profile_raw)?;
    let expected_members = profile_candidate_members
        .iter()
        .filter(|member| member.semantic_family_proposal != GOCHA_RESULT_MESSAGE_FAMILY_ID)
        .map(|member| (member.block_id, member))
        .collect::<BTreeMap<_, _>>();
    if entries.len() != expected_members.len() {
        return Err(error(format!(
            "일반 번역 분모 불일치: manifest={}, catalog={}",
            entries.len(),
            expected_members.len()
        )));
    }
    let mut ids = BTreeSet::new();
    let mut assets = BTreeSet::new();
    let mut families = BTreeSet::new();
    let mut pending_translations = 0usize;
    let mut draft_translations = 0usize;
    let mut approved_translations = 0usize;
    let mut scope_excluded_translations = 0usize;
    let mut previous_id = None;
    for entry in &entries {
        if previous_id.is_some_and(|previous| previous >= entry.block_id.as_str()) {
            return Err(error("번역 manifest가 block ID canonical 순서가 아님"));
        }
        previous_id = Some(entry.block_id.as_str());
        if !ids.insert(entry.block_id.as_str()) {
            return Err(error(format!("번역 block ID 중복: {}", entry.block_id)));
        }
        let member = expected_members
            .get(entry.block_id.as_str())
            .ok_or_else(|| {
                error(format!(
                    "일반 raster catalog에 없는 번역 block: {}",
                    entry.block_id
                ))
            })?;
        let expected_archives = member.archive_paths.join("\n");
        if entry.semantic_family != member.semantic_family_proposal
            || entry.profile_candidate_index != member.candidate_index
            || entry.archive_paths != expected_archives
            || entry.asset_name != member.asset_name
            || entry.asset_sha256 != member.asset_sha256
            || entry.block_pixels_sha256 != member.block_pixels_sha256
            || (
                entry.bbox_x,
                entry.bbox_y,
                entry.bbox_width,
                entry.bbox_height,
            ) != (
                member.bbox_x,
                member.bbox_y,
                member.bbox_width,
                member.bbox_height,
            )
            || entry.protected_source != member.protected_source
        {
            return Err(error(format!("{}의 보호 원본 필드 drift", entry.block_id)));
        }
        let is_scope_excluded_asset = scope_excluded_assets.contains(&entry.asset_name.as_str());
        if (entry.status == GeneralRasterTranslationStatus::ScopeExcludedProtectedNoWrite)
            != is_scope_excluded_asset
        {
            return Err(error(format!(
                "{}의 제품 범위 상태 불일치: asset={}",
                entry.block_id, entry.asset_name
            )));
        }
        assets.insert((entry.asset_name.clone(), entry.asset_sha256.clone()));
        families.insert(entry.semantic_family.clone());
        match entry.status {
            GeneralRasterTranslationStatus::PendingHumanTranslation => pending_translations += 1,
            GeneralRasterTranslationStatus::DraftHumanReviewRequired => draft_translations += 1,
            GeneralRasterTranslationStatus::Approved => approved_translations += 1,
            GeneralRasterTranslationStatus::ScopeExcludedProtectedNoWrite => {
                scope_excluded_translations += 1;
            }
        }
    }
    if ids != expected_members.keys().copied().collect() {
        return Err(error("일반 raster catalog와 번역 block 집합 불일치"));
    }

    let expected_profiles = expected_members
        .values()
        .map(|member| {
            let group = profile_candidate_groups
                .get(member.candidate_index - 1)
                .expect("검증된 profile candidate index");
            (
                (member.semantic_family_proposal, member.candidate_index),
                group,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut profile_keys = BTreeSet::new();
    let mut pending_profiles = 0usize;
    let mut development_profiles = 0usize;
    let mut approved_profiles = 0usize;
    let mut scope_excluded_profiles = 0usize;
    let profile_scope_exclusions = entries.iter().fold(
        BTreeMap::<(&str, usize), bool>::new(),
        |mut exclusions, entry| {
            let key = (
                entry.semantic_family.as_str(),
                entry.profile_candidate_index,
            );
            let excluded =
                entry.status == GeneralRasterTranslationStatus::ScopeExcludedProtectedNoWrite;
            exclusions
                .entry(key)
                .and_modify(|all_excluded| *all_excluded &= excluded)
                .or_insert(excluded);
            exclusions
        },
    );
    let mut previous_profile_key: Option<(&str, usize)> = None;
    for profile in &profiles {
        let key = (
            profile.semantic_family.as_str(),
            profile.profile_candidate_index,
        );
        if previous_profile_key.is_some_and(|previous| previous >= key) {
            return Err(error(
                "profile manifest가 family/index canonical 순서가 아님",
            ));
        }
        previous_profile_key = Some(key);
        if !profile_keys.insert(key) {
            return Err(error(format!("profile 연결 중복: {}:{}", key.0, key.1)));
        }
        let group = expected_profiles
            .get(&key)
            .ok_or_else(|| error(format!("catalog에 없는 profile 연결: {}:{}", key.0, key.1)))?;
        if profile.pixel_format != group.pixel_format
            || profile.bbox_height != group.bbox_height
            || profile.source_lines != group.source_lines
            || profile.palette_indices != group.palette_indices
        {
            return Err(error(format!(
                "profile 보호 제약 drift: {}:{}",
                key.0, key.1
            )));
        }
        let expected_scope_excluded = profile_scope_exclusions.get(&key) == Some(&true);
        if (profile.status == GeneralRasterProfileStatus::ScopeExcludedProtectedNoWrite)
            != expected_scope_excluded
        {
            return Err(error(format!(
                "profile 제품 범위 상태 불일치: {}:{}",
                key.0, key.1
            )));
        }
        if matches!(
            profile.status,
            GeneralRasterProfileStatus::DevelopmentDraftHumanReviewRequired
                | GeneralRasterProfileStatus::Approved
        ) {
            let line_count = parse_usize("line_count", &profile.line_count, 0)?;
            if entries
                .iter()
                .filter(|entry| {
                    entry.semantic_family == key.0 && entry.profile_candidate_index == key.1
                })
                .any(|entry| entry.ko.split('\n').count() != line_count)
            {
                return Err(error(format!(
                    "번역문과 profile 행 수 불일치: {}:{}",
                    key.0, key.1
                )));
            }
        }
        match profile.status {
            GeneralRasterProfileStatus::CandidateHumanApprovalRequired => pending_profiles += 1,
            GeneralRasterProfileStatus::DevelopmentDraftHumanReviewRequired => {
                pending_profiles += 1;
                development_profiles += 1;
            }
            GeneralRasterProfileStatus::Approved => approved_profiles += 1,
            GeneralRasterProfileStatus::ScopeExcludedProtectedNoWrite => {
                scope_excluded_profiles += 1;
            }
        }
    }
    if profile_keys != expected_profiles.keys().copied().collect() {
        return Err(error("일반 raster profile 연결 집합 불일치"));
    }
    let approved_profile_bindings = profiles
        .iter()
        .filter(|profile| profile.status == GeneralRasterProfileStatus::Approved)
        .map(|profile| {
            (
                (
                    profile.semantic_family.as_str(),
                    profile.profile_candidate_index,
                ),
                (profile.category_id.as_str(), profile.profile_id.as_str()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for entry in &mut entries {
        if let Some((category_id, profile_id)) = approved_profile_bindings.get(&(
            entry.semantic_family.as_str(),
            entry.profile_candidate_index,
        )) {
            entry.category_id = Some((*category_id).to_string());
            entry.profile_id = Some((*profile_id).to_string());
        }
    }

    let covered_raster_blocks = entries
        .len()
        .checked_add(g_mess.entries)
        .ok_or_else(|| error("raster block 합계 overflow"))?;
    let report = GeneralRasterManifestReport {
        translation_blocks: entries.len(),
        translation_assets: assets.len(),
        profile_assignments: profiles.len(),
        semantic_families: families.len(),
        pending_translations,
        draft_translations,
        approved_translations,
        scope_excluded_translations,
        pending_profiles,
        development_profiles,
        approved_profiles,
        scope_excluded_profiles,
        delegated_g_mess_blocks: g_mess.entries,
        covered_raster_blocks,
        all_in_scope_human_approved: approved_translations + scope_excluded_translations
            == entries.len()
            && approved_profiles + scope_excluded_profiles == profiles.len()
            && g_mess.product_eligible,
    };
    Ok((entries, report))
}

/// 보호 원문·geometry와 편집 가능한 번역 상태의 전수 연결을 검증한다.
pub fn verify_general_btx_raster_manifests(
) -> Result<GeneralRasterManifestReport, GeneralRasterManifestError> {
    verify_manifests(TRANSLATION_MANIFEST, PROFILE_MANIFEST).map(|(_, report)| report)
}

/// coverage와 검토 보고서가 사용할 검증 완료 일반 raster 번역 항목.
pub fn general_btx_raster_translation_entries(
) -> Result<Vec<GeneralRasterTranslationEntry>, GeneralRasterManifestError> {
    verify_manifests(TRANSLATION_MANIFEST, PROFILE_MANIFEST).map(|(entries, _)| entries)
}

/// 보호 candidate 제약을 통과한 모든 profile 행과 개발·승인 설정을 반환한다.
pub fn general_btx_raster_profile_assignments(
) -> Result<Vec<GeneralRasterProfileAssignment>, GeneralRasterManifestError> {
    verify_manifests(TRANSLATION_MANIFEST, PROFILE_MANIFEST)?;
    parse_profile_manifest(PROFILE_MANIFEST)?
        .into_iter()
        .enumerate()
        .map(|(index, entry)| public_profile_assignment(entry, index + 2))
        .collect()
}

#[cfg(test)]
#[path = "btx_raster_translation_manifest_tests.rs"]
mod tests;
