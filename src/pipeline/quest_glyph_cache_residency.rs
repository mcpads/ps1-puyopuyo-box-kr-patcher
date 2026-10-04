//! QUEST sparse glyph cache의 save-state 상주 상태 검증.
//!
//! exact transition build를 실행해 얻은 Mednafen native state에서 package,
//! hook site/body, page state와 carrier 미사용 suffix를 다시 읽는다. package의
//! scratch는 마지막 upload cell로 바뀌므로 원본 직렬화 바이트와 단순 비교하지
//! 않는다. 화면 PNG는 소비 장면의 식별 자료로 묶되 번역·layout 승인을
//! 대신하지 않는다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::disasm::{
    QUEST_FONT_STATE_BASE_RUNTIME, QUEST_FONT_STATE_RECORD_COUNT, QUEST_FONT_STATE_RECORD_STRIDE,
    QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET, QUEST_PAGE_CARRIER_CAPACITY_BYTES,
    QUEST_PAGE_CARRIER_START_RUNTIME,
};
use crate::runtime::MednafenPsxState;

use super::quest_development_disc::{
    QuestDevelopmentDiscBuild, QUEST_DEVELOPMENT_DISC_SCHEMA_VERSION,
};
use super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_glyph_cache_transition_poc::QuestGlyphCacheTransitionPocBuild;
use super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;

pub const QUEST_GLYPH_CACHE_RESIDENCY_SCHEMA_VERSION: &str = "quest-glyph-cache-residency-v2";
pub const QUEST_DEVELOPMENT_DISC_RESIDENCY_SCHEMA_VERSION: &str =
    "quest-development-disc-residency-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestGlyphCacheScratchExpectation {
    InitialZero,
    UploadedPage(u8),
}

const PNG_WIDTH: u32 = 280;
const PNG_HEIGHT: u32 = 240;
const FONT_STATE_ACTIVE_OFFSET: u32 = QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET as u32 + 1;

/// 일반 입력 뒤 save-state에서 exact carrier가 유지된 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheResidencyReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub state_sha256: String,
    pub state_version: u32,
    pub state_sections: usize,
    pub screenshot_sha256: String,
    pub screenshot_width: u32,
    pub screenshot_height: u32,
    pub package_runtime: u32,
    pub package_bytes: usize,
    pub package_sha256: String,
    pub site_runtime: u32,
    pub site_bytes: usize,
    pub site_sha256: String,
    pub original_font_guard_site_runtime: u32,
    pub original_font_guard_site_bytes: usize,
    pub original_font_guard_site_sha256: String,
    pub body_runtime: u32,
    pub body_bytes: usize,
    pub body_sha256: String,
    pub carrier_suffix_runtime: u32,
    pub carrier_suffix_bytes: usize,
    pub page_state_runtime: u32,
    pub page_state: u8,
    pub active_font_state_indices: Vec<usize>,
    pub resident_package_exact: bool,
    pub site_exact: bool,
    pub original_font_guard_site_exact: bool,
    pub body_exact: bool,
    pub carrier_suffix_zero: bool,
    pub page_state_exact: bool,
    pub screenshot_shape_exact: bool,
    pub residency_verified: bool,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub release_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheResidencyError(String);

impl fmt::Display for QuestGlyphCacheResidencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestGlyphCacheResidencyError {}

fn error(message: impl Into<String>) -> QuestGlyphCacheResidencyError {
    QuestGlyphCacheResidencyError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

pub(super) fn png_dimensions(raw: &[u8]) -> Option<(u32, u32)> {
    if raw.get(..8) != Some(b"\x89PNG\r\n\x1a\n") || raw.get(12..16) != Some(b"IHDR") {
        return None;
    }
    let width = u32::from_be_bytes(raw.get(16..20)?.try_into().ok()?);
    let height = u32::from_be_bytes(raw.get(20..24)?.try_into().ok()?);
    Some((width, height))
}

fn expected_page_cells(
    package: &QuestGlyphCachePackage,
    page_id: u8,
) -> Result<Vec<Vec<u8>>, QuestGlyphCacheResidencyError> {
    if page_id == 0 {
        return (0..package.plan.max_page_glyphs)
            .map(|slot| {
                package
                    .original_atlas_source_cell(slot)
                    .map_err(|source| error(source.to_string()))
            })
            .collect();
    }
    package
        .page_catalog_indices(page_id)
        .map_err(|source| error(source.to_string()))?
        .iter()
        .map(|index| {
            package
                .glyph_cell(*index)
                .map_err(|source| error(source.to_string()))
        })
        .collect()
}

pub(super) fn expected_resident_package(
    package: &QuestGlyphCachePackage,
    scratch: QuestGlyphCacheScratchExpectation,
) -> Result<Vec<u8>, QuestGlyphCacheResidencyError> {
    let QuestGlyphCacheScratchExpectation::UploadedPage(page_id) = scratch else {
        if !package.scratch_zero {
            return Err(error("QUEST glyph cache package scratch 초기값 drift"));
        }
        return Ok(package.bytes.clone());
    };
    let cells = expected_page_cells(package, page_id)?;
    let final_cell = cells.last().ok_or_else(|| {
        error(format!(
            "QUEST glyph cache page cell이 비어 있음: page={page_id}"
        ))
    })?;
    let mut expected = package.bytes.clone();
    replace_resident_scratch(
        &mut expected,
        package.layout.scratch_offset,
        package.layout.scratch_bytes,
        final_cell,
    )?;
    Ok(expected)
}

fn replace_resident_scratch(
    package: &mut [u8],
    scratch_offset: usize,
    scratch_bytes: usize,
    final_cell: &[u8],
) -> Result<(), QuestGlyphCacheResidencyError> {
    if final_cell.len() != scratch_bytes {
        return Err(error(format!(
            "QUEST glyph cache scratch/cell 길이 불일치: scratch={}, cell={}",
            scratch_bytes,
            final_cell.len()
        )));
    }
    let end = scratch_offset
        .checked_add(final_cell.len())
        .ok_or_else(|| error("QUEST glyph cache scratch end overflow"))?;
    package
        .get_mut(scratch_offset..end)
        .ok_or_else(|| error("QUEST glyph cache scratch package 범위 이탈"))?
        .copy_from_slice(final_cell);
    Ok(())
}

fn exact_state_range(
    state: &MednafenPsxState<'_>,
    runtime: u32,
    expected: &[u8],
    label: &str,
) -> Result<String, QuestGlyphCacheResidencyError> {
    let actual = state
        .read_bytes(runtime, expected.len())
        .map_err(|source| error(format!("{label} state 읽기 실패: {source}")))?;
    if actual != expected {
        let first_offset = actual
            .iter()
            .zip(expected)
            .position(|(actual, expected)| actual != expected)
            .ok_or_else(|| error(format!("{label} state 비교 내부 불일치")))?;
        let mismatch_bytes = actual
            .iter()
            .zip(expected)
            .filter(|(actual, expected)| actual != expected)
            .count();
        return Err(error(format!(
            "{label} state readback 불일치: runtime=0x{runtime:08X}, bytes={}, \
             first_offset=0x{first_offset:X}, expected=0x{:02X}, actual=0x{:02X}, \
             mismatch_bytes={mismatch_bytes}",
            expected.len(),
            expected[first_offset],
            actual[first_offset]
        )));
    }
    Ok(digest(actual))
}

fn zero_state_range(
    state: &MednafenPsxState<'_>,
    runtime: u32,
    bytes: usize,
    label: &str,
) -> Result<(), QuestGlyphCacheResidencyError> {
    let actual = state
        .read_bytes(runtime, bytes)
        .map_err(|source| error(format!("{label} state 읽기 실패: {source}")))?;
    if let Some(offset) = actual.iter().position(|byte| *byte != 0) {
        return Err(error(format!(
            "{label} 0 경계 불일치: runtime=0x{runtime:08X}, offset=0x{offset:X}, value=0x{:02X}",
            actual[offset]
        )));
    }
    Ok(())
}

struct QuestGlyphCacheResidencyArtifact<'a> {
    schema_version: &'static str,
    image_sha256: &'a str,
    package: &'a QuestGlyphCachePackage,
    hook: &'a QuestGlyphCacheHookPlan,
    expected_page_state: u8,
    scratch: QuestGlyphCacheScratchExpectation,
}

fn verify_residency_for_artifact(
    artifact: QuestGlyphCacheResidencyArtifact<'_>,
    native_state: &[u8],
    screenshot_png: &[u8],
) -> Result<QuestGlyphCacheResidencyReport, QuestGlyphCacheResidencyError> {
    let QuestGlyphCacheResidencyArtifact {
        schema_version,
        image_sha256,
        package,
        hook,
        expected_page_state,
        scratch,
    } = artifact;
    if image_sha256.len() != 64
        || hook.body_runtime
            != QUEST_PAGE_CARRIER_START_RUNTIME
                .checked_add(
                    u32::try_from(package.bytes.len())
                        .map_err(|_| error("QUEST glyph cache package 길이 범위 이탈"))?,
                )
                .ok_or_else(|| error("QUEST glyph cache body runtime overflow"))?
    {
        return Err(error("QUEST glyph cache residency 입력 계약 drift"));
    }

    let state = MednafenPsxState::parse(native_state)
        .map_err(|source| error(format!("Mednafen native state 검증 실패: {source}")))?;
    let expected_package = expected_resident_package(package, scratch)?;
    let package_sha256 = exact_state_range(
        &state,
        QUEST_PAGE_CARRIER_START_RUNTIME,
        &expected_package,
        "QUEST glyph cache resident package",
    )?;
    let site_sha256 = exact_state_range(
        &state,
        hook.site.base_addr(),
        hook.site.bytes(),
        "QUEST glyph cache hook site",
    )?;
    let original_font_guard_site_sha256 = exact_state_range(
        &state,
        hook.original_font_guard_site.base_addr(),
        hook.original_font_guard_site.bytes(),
        "QUEST original font guard site",
    )?;
    let body_sha256 = exact_state_range(
        &state,
        hook.body_runtime,
        hook.body.bytes(),
        "QUEST glyph cache hook body",
    )?;

    let carrier_end = QUEST_PAGE_CARRIER_START_RUNTIME
        .checked_add(
            u32::try_from(QUEST_PAGE_CARRIER_CAPACITY_BYTES)
                .map_err(|_| error("QUEST glyph cache carrier 길이 범위 이탈"))?,
        )
        .ok_or_else(|| error("QUEST glyph cache carrier end overflow"))?;
    let carrier_suffix_runtime = hook
        .body_runtime
        .checked_add(
            u32::try_from(hook.body.bytes().len())
                .map_err(|_| error("QUEST glyph cache body 길이 범위 이탈"))?,
        )
        .ok_or_else(|| error("QUEST glyph cache body end overflow"))?;
    let carrier_suffix_bytes = usize::try_from(
        carrier_end
            .checked_sub(carrier_suffix_runtime)
            .ok_or_else(|| error("QUEST glyph cache body가 carrier 끝을 넘음"))?,
    )
    .map_err(|_| error("QUEST glyph cache suffix 길이 범위 이탈"))?;
    zero_state_range(
        &state,
        carrier_suffix_runtime,
        carrier_suffix_bytes,
        "QUEST glyph cache carrier suffix",
    )?;

    let page_state = state
        .read_u8(QUEST_PAGE_HOOK_STATE_RUNTIME)
        .map_err(|source| error(format!("QUEST glyph cache page state 읽기 실패: {source}")))?;
    if page_state != expected_page_state {
        return Err(error(format!(
            "QUEST glyph cache page state 불일치: expected={expected_page_state}, actual={page_state}"
        )));
    }

    let mut active_font_state_indices = Vec::new();
    for index in 0..QUEST_FONT_STATE_RECORD_COUNT {
        let runtime = QUEST_FONT_STATE_BASE_RUNTIME
            .checked_add(
                u32::try_from(index * QUEST_FONT_STATE_RECORD_STRIDE)
                    .map_err(|_| error("QUEST font state offset 범위 이탈"))?,
            )
            .and_then(|runtime| runtime.checked_add(FONT_STATE_ACTIVE_OFFSET))
            .ok_or_else(|| error("QUEST font active state runtime overflow"))?;
        if state
            .read_u8(runtime)
            .map_err(|source| error(format!("QUEST font active state 읽기 실패: {source}")))?
            != 0
        {
            active_font_state_indices.push(index);
        }
    }

    let (screenshot_width, screenshot_height) = png_dimensions(screenshot_png)
        .ok_or_else(|| error("QUEST glyph cache screenshot PNG header 불일치"))?;
    let screenshot_shape_exact = screenshot_width == PNG_WIDTH && screenshot_height == PNG_HEIGHT;
    if !screenshot_shape_exact {
        return Err(error(format!(
            "QUEST glyph cache screenshot 크기 불일치: expected={PNG_WIDTH}x{PNG_HEIGHT}, actual={screenshot_width}x{screenshot_height}"
        )));
    }

    Ok(QuestGlyphCacheResidencyReport {
        schema_version,
        image_sha256: image_sha256.to_string(),
        state_sha256: digest(native_state),
        state_version: state.state_version(),
        state_sections: state.section_count(),
        screenshot_sha256: digest(screenshot_png),
        screenshot_width,
        screenshot_height,
        package_runtime: QUEST_PAGE_CARRIER_START_RUNTIME,
        package_bytes: expected_package.len(),
        package_sha256,
        site_runtime: hook.site.base_addr(),
        site_bytes: hook.site.bytes().len(),
        site_sha256,
        original_font_guard_site_runtime: hook.original_font_guard_site.base_addr(),
        original_font_guard_site_bytes: hook.original_font_guard_site.bytes().len(),
        original_font_guard_site_sha256,
        body_runtime: hook.body_runtime,
        body_bytes: hook.body.bytes().len(),
        body_sha256,
        carrier_suffix_runtime,
        carrier_suffix_bytes,
        page_state_runtime: QUEST_PAGE_HOOK_STATE_RUNTIME,
        page_state,
        active_font_state_indices,
        resident_package_exact: true,
        site_exact: true,
        original_font_guard_site_exact: true,
        body_exact: true,
        carrier_suffix_zero: true,
        page_state_exact: true,
        screenshot_shape_exact,
        residency_verified: true,
        translation_approved: false,
        layout_approved: false,
        release_eligible: false,
    })
}

/// exact transition build가 일반 입력 뒤에도 carrier에 상주하는지 검증한다.
pub fn verify_quest_glyph_cache_residency(
    build: &QuestGlyphCacheTransitionPocBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    expected_page_state: u8,
    native_state: &[u8],
    screenshot_png: &[u8],
) -> Result<QuestGlyphCacheResidencyReport, QuestGlyphCacheResidencyError> {
    if digest(&build.image) != build.report.disc.image_sha256
        || build.report.disc.package_sha256 != package.sha256
        || build.report.disc.package_bytes != package.bytes.len()
        || build.report.disc.site_runtime != hook.site.base_addr()
        || build.report.disc.original_font_guard_site_runtime
            != hook.original_font_guard_site.base_addr()
        || build.report.disc.body_runtime != hook.body_runtime
        || build.report.disc.body_bytes != hook.body.bytes().len()
        || build.report.runtime_verified
        || build.report.release_eligible
    {
        return Err(error(
            "QUEST glyph cache residency transition 입력 계약 drift",
        ));
    }
    verify_residency_for_artifact(
        QuestGlyphCacheResidencyArtifact {
            schema_version: QUEST_GLYPH_CACHE_RESIDENCY_SCHEMA_VERSION,
            image_sha256: &build.report.disc.image_sha256,
            package,
            hook,
            expected_page_state,
            scratch: QuestGlyphCacheScratchExpectation::UploadedPage(expected_page_state),
        },
        native_state,
        screenshot_png,
    )
}

struct QuestDevelopmentDiscResidencyInput<'a> {
    build: &'a QuestDevelopmentDiscBuild,
    package: &'a QuestGlyphCachePackage,
    hook: &'a QuestGlyphCacheHookPlan,
    expected_build_schema: &'static str,
    residency_schema: &'static str,
    expected_page_state: u8,
    scratch: QuestGlyphCacheScratchExpectation,
}

fn verify_quest_development_disc_residency_with_scratch(
    input: QuestDevelopmentDiscResidencyInput<'_>,
    native_state: &[u8],
    screenshot_png: &[u8],
) -> Result<QuestGlyphCacheResidencyReport, QuestGlyphCacheResidencyError> {
    let QuestDevelopmentDiscResidencyInput {
        build,
        package,
        hook,
        expected_build_schema,
        residency_schema,
        expected_page_state,
        scratch,
    } = input;
    let disc = &build.report.disc;
    if build.report.schema_version != expected_build_schema
        || digest(&build.image) != disc.image_sha256
        || disc.package_sha256 != package.sha256
        || disc.package_bytes != package.bytes.len()
        || disc.site_runtime != hook.site.base_addr()
        || disc.original_font_guard_site_runtime != hook.original_font_guard_site.base_addr()
        || disc.body_runtime != hook.body_runtime
        || disc.body_bytes != hook.body.bytes().len()
        || build.report.runtime_verified
        || build.report.product_eligible
        || disc.runtime_verified
        || disc.build_eligible
    {
        return Err(error(
            "QUEST development disc residency 정적 입력 계약 drift",
        ));
    }
    verify_residency_for_artifact(
        QuestGlyphCacheResidencyArtifact {
            schema_version: residency_schema,
            image_sha256: &disc.image_sha256,
            package,
            hook,
            expected_page_state,
            scratch,
        },
        native_state,
        screenshot_png,
    )
}

/// 사람이 작성한 조판을 포함한 development disc의 일반 입력 뒤 상주 상태를 검증한다.
pub fn verify_quest_development_disc_residency(
    build: &QuestDevelopmentDiscBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    expected_page_state: u8,
    native_state: &[u8],
    screenshot_png: &[u8],
) -> Result<QuestGlyphCacheResidencyReport, QuestGlyphCacheResidencyError> {
    verify_quest_development_disc_residency_with_scratch(
        QuestDevelopmentDiscResidencyInput {
            build,
            package,
            hook,
            expected_build_schema: QUEST_DEVELOPMENT_DISC_SCHEMA_VERSION,
            residency_schema: QUEST_DEVELOPMENT_DISC_RESIDENCY_SCHEMA_VERSION,
            expected_page_state,
            scratch: QuestGlyphCacheScratchExpectation::UploadedPage(expected_page_state),
        },
        native_state,
        screenshot_png,
    )
}

/// development disc의 같은-page skip에서 초기 scratch가 유지됐는지 검증한다.
pub fn verify_quest_development_disc_initial_residency(
    build: &QuestDevelopmentDiscBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    expected_page_state: u8,
    native_state: &[u8],
    screenshot_png: &[u8],
) -> Result<QuestGlyphCacheResidencyReport, QuestGlyphCacheResidencyError> {
    verify_quest_development_disc_residency_with_scratch(
        QuestDevelopmentDiscResidencyInput {
            build,
            package,
            hook,
            expected_build_schema: QUEST_DEVELOPMENT_DISC_SCHEMA_VERSION,
            residency_schema: QUEST_DEVELOPMENT_DISC_RESIDENCY_SCHEMA_VERSION,
            expected_page_state,
            scratch: QuestGlyphCacheScratchExpectation::InitialZero,
        },
        native_state,
        screenshot_png,
    )
}

/// 상주 상태 결과를 한 행 TSV로 만든다.
pub fn render_quest_glyph_cache_residency_tsv(report: &QuestGlyphCacheResidencyReport) -> String {
    let active = report
        .active_font_state_indices
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut out = String::from(
        "schema_version\timage_sha256\tstate_sha256\tstate_version\tstate_sections\t\
         screenshot_sha256\tscreenshot_width\tscreenshot_height\tpackage_runtime\t\
         package_bytes\tpackage_sha256\tsite_runtime\tsite_bytes\tsite_sha256\t\
         original_font_guard_site_runtime\toriginal_font_guard_site_bytes\t\
         original_font_guard_site_sha256\tbody_runtime\tbody_bytes\tbody_sha256\t\
         carrier_suffix_runtime\t\
         carrier_suffix_bytes\tpage_state_runtime\tpage_state\tactive_font_state_indices\t\
         resident_package_exact\tsite_exact\toriginal_font_guard_site_exact\tbody_exact\t\
         carrier_suffix_zero\t\
         page_state_exact\tscreenshot_shape_exact\tresidency_verified\ttranslation_approved\t\
         layout_approved\trelease_eligible\n",
    );
    writeln!(
        out,
        "{}\t{}\t{}\t0x{:X}\t{}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        report.schema_version,
        report.image_sha256,
        report.state_sha256,
        report.state_version,
        report.state_sections,
        report.screenshot_sha256,
        report.screenshot_width,
        report.screenshot_height,
        report.package_runtime,
        report.package_bytes,
        report.package_sha256,
        report.site_runtime,
        report.site_bytes,
        report.site_sha256,
        report.original_font_guard_site_runtime,
        report.original_font_guard_site_bytes,
        report.original_font_guard_site_sha256,
        report.body_runtime,
        report.body_bytes,
        report.body_sha256,
        report.carrier_suffix_runtime,
        report.carrier_suffix_bytes,
        report.page_state_runtime,
        report.page_state,
        active,
        report.resident_package_exact,
        report.site_exact,
        report.original_font_guard_site_exact,
        report.body_exact,
        report.carrier_suffix_zero,
        report.page_state_exact,
        report.screenshot_shape_exact,
        report.residency_verified,
        report.translation_approved,
        report.layout_approved,
        report.release_eligible,
    )
    .expect("String write는 실패하지 않음");
    out
}

#[cfg(test)]
#[path = "quest_glyph_cache_residency_tests.rs"]
mod tests;
