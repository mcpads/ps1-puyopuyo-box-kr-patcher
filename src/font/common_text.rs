//! `COM_FONT.BTX`에 구워진 일본어 pixel surface 후보 원장.
//!
//! 전수 atlas 대조에서 사람이 판독한 원문과 rectangle을 불변 원본 픽셀에
//! 결속한다. 대표 runtime cell 번호는 후속 draw 표본을 고르는 후보이며,
//! 실제 소비·화면 표시·번역 승인으로 승격하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;

use super::common::{verified_common_font_pixels, CommonFontError};
use super::{CommonFontSpec, BOX_COMMON_FONT_SPECS};

pub const COMMON_FONT_TEXT_SURFACE_SCHEMA_VERSION: &str = "common-font-text-surfaces-v1";

/// 원본 atlas의 일본어 pixel surface 하나와 대표 runtime cell 후보.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonFontTextSurfaceSpec {
    pub surface_id: &'static str,
    pub archive: &'static str,
    pub source_text: &'static str,
    pub semantic_category: &'static str,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub representative_runtime_cell_candidates: &'static [u16],
    pub nonzero_pixels: usize,
    pub pixel_sha256: &'static str,
}

const CELL_26: &[u16] = &[26];
const CELL_27: &[u16] = &[27];
const CELL_202: &[u16] = &[202];
const CELL_203: &[u16] = &[203];
const CELLS_202_203: &[u16] = &[202, 203];

pub const BOX_COMMON_FONT_TEXT_SURFACES: [CommonFontTextSurfaceSpec; 16] = [
    CommonFontTextSurfaceSpec {
        surface_id: "gocha1-chain-label",
        archive: "/GOCHA1/TUU_GRP.VDD;1",
        source_text: "れんさ",
        semantic_category: "baked-label",
        x: 176,
        y: 0,
        width: 24,
        height: 8,
        representative_runtime_cell_candidates: CELL_26,
        nonzero_pixels: 131,
        pixel_sha256: "2e1d7817a19c10816f62af1dc7e4f55bfaf058c7c0e81e412d3029c14777fa5b",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "gocha2-chain-label",
        archive: "/GOCHA2/GOTY_GRP.VDD;1",
        source_text: "れんさ",
        semantic_category: "baked-label",
        x: 176,
        y: 0,
        width: 24,
        height: 8,
        representative_runtime_cell_candidates: CELL_26,
        nonzero_pixels: 131,
        pixel_sha256: "2e1d7817a19c10816f62af1dc7e4f55bfaf058c7c0e81e412d3029c14777fa5b",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "hori2-chain-label",
        archive: "/HORI2/HORI2GRP.VDD;1",
        source_text: "れんさ",
        semantic_category: "baked-label",
        x: 176,
        y: 0,
        width: 24,
        height: 8,
        representative_runtime_cell_candidates: CELL_26,
        nonzero_pixels: 131,
        pixel_sha256: "2e1d7817a19c10816f62af1dc7e4f55bfaf058c7c0e81e412d3029c14777fa5b",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "hori4-chain-label",
        archive: "/HORI4/HORI4GRP.VDD;1",
        source_text: "れんさ",
        semantic_category: "baked-label",
        x: 152,
        y: 0,
        width: 16,
        height: 8,
        representative_runtime_cell_candidates: CELL_26,
        nonzero_pixels: 76,
        pixel_sha256: "436644feb964c47fcb7d14e5d7b9f06c5353fffc5557e0a6e052e4277add03dc",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "minpuyo-chain-label",
        archive: "/MINPUYO/MIN_GRP.VDD;1",
        source_text: "れんさ",
        semantic_category: "baked-label",
        x: 152,
        y: 0,
        width: 16,
        height: 8,
        representative_runtime_cell_candidates: CELL_26,
        nonzero_pixels: 76,
        pixel_sha256: "436644feb964c47fcb7d14e5d7b9f06c5353fffc5557e0a6e052e4277add03dc",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "kousiki-chain-label",
        archive: "/KOUSIKI/KOU_GRP.VDD;1",
        source_text: "れんさ",
        semantic_category: "baked-label",
        x: 216,
        y: 0,
        width: 24,
        height: 8,
        representative_runtime_cell_candidates: CELL_27,
        nonzero_pixels: 131,
        pixel_sha256: "2e1d7817a19c10816f62af1dc7e4f55bfaf058c7c0e81e412d3029c14777fa5b",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "puyo1-chain-label",
        archive: "/PUYO1/PP1_GRP.VDD;1",
        source_text: "れんさ",
        semantic_category: "baked-label",
        x: 216,
        y: 0,
        width: 24,
        height: 8,
        representative_runtime_cell_candidates: CELL_27,
        nonzero_pixels: 131,
        pixel_sha256: "2e1d7817a19c10816f62af1dc7e4f55bfaf058c7c0e81e412d3029c14777fa5b",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "puyo2-chain-label",
        archive: "/PUYO2/PP2_GRP.VDD;1",
        source_text: "れんさ",
        semantic_category: "baked-label",
        x: 216,
        y: 0,
        width: 24,
        height: 8,
        representative_runtime_cell_candidates: CELL_27,
        nonzero_pixels: 131,
        pixel_sha256: "2e1d7817a19c10816f62af1dc7e4f55bfaf058c7c0e81e412d3029c14777fa5b",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "quest-chain-label",
        archive: "/QUEST/RPG_GRP.VDD;1",
        source_text: "れんさ",
        semantic_category: "baked-label",
        x: 216,
        y: 0,
        width: 24,
        height: 8,
        representative_runtime_cell_candidates: CELL_27,
        nonzero_pixels: 131,
        pixel_sha256: "2e1d7817a19c10816f62af1dc7e4f55bfaf058c7c0e81e412d3029c14777fa5b",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "kousiki-record-label",
        archive: "/KOUSIKI/KOU_GRP.VDD;1",
        source_text: "きろく",
        semantic_category: "baked-label",
        x: 208,
        y: 64,
        width: 48,
        height: 16,
        representative_runtime_cell_candidates: CELL_202,
        nonzero_pixels: 612,
        pixel_sha256: "bd7ab1b552757ebfdb6a07d0d3c8aebbaa9713d16a0e956c3e94e370116f1ca0",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "puyo1-record-label",
        archive: "/PUYO1/PP1_GRP.VDD;1",
        source_text: "きろく",
        semantic_category: "baked-label",
        x: 208,
        y: 64,
        width: 48,
        height: 16,
        representative_runtime_cell_candidates: CELL_202,
        nonzero_pixels: 612,
        pixel_sha256: "bd7ab1b552757ebfdb6a07d0d3c8aebbaa9713d16a0e956c3e94e370116f1ca0",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "puyo2-record-label",
        archive: "/PUYO2/PP2_GRP.VDD;1",
        source_text: "きろく",
        semantic_category: "baked-label",
        x: 208,
        y: 64,
        width: 48,
        height: 16,
        representative_runtime_cell_candidates: CELL_202,
        nonzero_pixels: 612,
        pixel_sha256: "bd7ab1b552757ebfdb6a07d0d3c8aebbaa9713d16a0e956c3e94e370116f1ca0",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "kousiki-stage-hint",
        archive: "/KOUSIKI/KOU_GRP.VDD;1",
        source_text: "このステージでは\nヒントが出るよ。",
        semantic_category: "baked-message",
        x: 0,
        y: 64,
        width: 96,
        height: 32,
        representative_runtime_cell_candidates: CELL_203,
        nonzero_pixels: 1_227,
        pixel_sha256: "e01dc2da03784246ec61c894ef8921556fa5a2654053c4f47e20204722df52f5",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "puyo1-stage-hint",
        archive: "/PUYO1/PP1_GRP.VDD;1",
        source_text: "このステージでは\nヒントが出るよ。",
        semantic_category: "baked-message",
        x: 0,
        y: 64,
        width: 96,
        height: 32,
        representative_runtime_cell_candidates: CELL_203,
        nonzero_pixels: 1_227,
        pixel_sha256: "e01dc2da03784246ec61c894ef8921556fa5a2654053c4f47e20204722df52f5",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "puyo2-stage-hint",
        archive: "/PUYO2/PP2_GRP.VDD;1",
        source_text: "このステージでは\nヒントが出るよ。",
        semantic_category: "baked-message",
        x: 0,
        y: 64,
        width: 96,
        height: 32,
        representative_runtime_cell_candidates: CELL_203,
        nonzero_pixels: 1_227,
        pixel_sha256: "e01dc2da03784246ec61c894ef8921556fa5a2654053c4f47e20204722df52f5",
    },
    CommonFontTextSurfaceSpec {
        surface_id: "quest-break-label",
        archive: "/QUEST/RPG_GRP.VDD;1",
        source_text: "きゅうけい",
        semantic_category: "baked-label",
        x: 0,
        y: 64,
        width: 64,
        height: 24,
        representative_runtime_cell_candidates: CELLS_202_203,
        nonzero_pixels: 1_064,
        pixel_sha256: "0572c23e78d6dca2f6773b69cc132f4f097d4b15bbfe7211f19d4ec9108da0e2",
    },
];

/// 원본 픽셀에 결속된 surface 보고서 한 행.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontTextSurface {
    pub spec: CommonFontTextSurfaceSpec,
    pub pixel_sha256: String,
    pub pixel_surface_exact: bool,
    pub transcription_status: &'static str,
    pub runtime_draw_verified: bool,
    pub translation_approved: bool,
    pub release_eligible: bool,
}

/// 9개 물리 atlas의 일본어 surface 후보 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontTextSurfaceReport {
    pub schema_version: &'static str,
    pub physical_surfaces: usize,
    pub physical_archives: usize,
    pub unique_source_texts: usize,
    pub unique_pixel_surfaces: usize,
    pub representative_runtime_cell_bindings: usize,
    pub rows: Vec<CommonFontTextSurface>,
}

#[derive(Debug)]
pub enum CommonFontTextSurfaceError {
    DuplicateArchive(String),
    MissingArchive(&'static str),
    CommonFont(CommonFontError),
    RectangleOutsideAtlas(&'static str),
    PixelDigest {
        surface_id: &'static str,
        expected: &'static str,
        actual: String,
    },
    NonzeroPixels {
        surface_id: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for CommonFontTextSurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateArchive(archive) => write!(f, "COM_FONT text GRP 입력 중복: {archive}"),
            Self::MissingArchive(archive) => write!(f, "COM_FONT text GRP 입력 누락: {archive}"),
            Self::CommonFont(source) => write!(f, "COM_FONT 원본 gate 실패: {source}"),
            Self::RectangleOutsideAtlas(surface_id) => {
                write!(f, "COM_FONT text rectangle이 atlas 밖임: {surface_id}")
            }
            Self::PixelDigest {
                surface_id,
                expected,
                actual,
            } => write!(
                f,
                "COM_FONT text pixel digest 불일치: {surface_id}, expected={expected}, actual={actual}"
            ),
            Self::NonzeroPixels {
                surface_id,
                expected,
                actual,
            } => write!(
                f,
                "COM_FONT text nonzero pixel 수 불일치: {surface_id}, expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for CommonFontTextSurfaceError {}

impl From<CommonFontError> for CommonFontTextSurfaceError {
    fn from(source: CommonFontError) -> Self {
        Self::CommonFont(source)
    }
}

fn crop_pixels(
    pixels: &[u8],
    atlas_width: usize,
    atlas_height: usize,
    spec: CommonFontTextSurfaceSpec,
) -> Result<Vec<u8>, CommonFontTextSurfaceError> {
    let x_end =
        spec.x
            .checked_add(spec.width)
            .ok_or(CommonFontTextSurfaceError::RectangleOutsideAtlas(
                spec.surface_id,
            ))?;
    let y_end = spec.y.checked_add(spec.height).ok_or(
        CommonFontTextSurfaceError::RectangleOutsideAtlas(spec.surface_id),
    )?;
    if spec.width == 0
        || spec.height == 0
        || x_end > atlas_width
        || y_end > atlas_height
        || pixels.len() != atlas_width * atlas_height
    {
        return Err(CommonFontTextSurfaceError::RectangleOutsideAtlas(
            spec.surface_id,
        ));
    }
    let mut crop = Vec::with_capacity(spec.width * spec.height);
    for y in spec.y..y_end {
        let start = y * atlas_width + spec.x;
        crop.extend_from_slice(&pixels[start..start + spec.width]);
    }
    Ok(crop)
}

/// 지원 원본 9개 atlas에서 일본어 surface 후보의 픽셀을 다시 검증한다.
pub fn verify_box_common_font_text_surfaces<'a>(
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<CommonFontTextSurfaceReport, CommonFontTextSurfaceError> {
    let mut by_id = BTreeMap::new();
    for (archive_id, archive) in archives {
        if by_id.insert(archive_id, archive).is_some() {
            return Err(CommonFontTextSurfaceError::DuplicateArchive(
                archive_id.to_string(),
            ));
        }
    }

    let mut verified_pixels = BTreeMap::new();
    for source_spec in BOX_COMMON_FONT_SPECS {
        let archive =
            by_id
                .get(source_spec.archive)
                .ok_or(CommonFontTextSurfaceError::MissingArchive(
                    source_spec.archive,
                ))?;
        verified_pixels.insert(
            source_spec.archive,
            verified_common_font_pixels(source_spec, archive)?,
        );
    }

    let source_specs = BOX_COMMON_FONT_SPECS
        .iter()
        .map(|spec| (spec.archive, *spec))
        .collect::<BTreeMap<&str, CommonFontSpec>>();
    let mut rows = Vec::with_capacity(BOX_COMMON_FONT_TEXT_SURFACES.len());
    for surface_spec in BOX_COMMON_FONT_TEXT_SURFACES {
        let source_spec = source_specs.get(surface_spec.archive).ok_or(
            CommonFontTextSurfaceError::MissingArchive(surface_spec.archive),
        )?;
        let pixels = verified_pixels.get(surface_spec.archive).ok_or(
            CommonFontTextSurfaceError::MissingArchive(surface_spec.archive),
        )?;
        let crop = crop_pixels(pixels, source_spec.width, source_spec.height, surface_spec)?;
        let nonzero_pixels = crop.iter().filter(|pixel| **pixel != 0).count();
        if nonzero_pixels != surface_spec.nonzero_pixels {
            return Err(CommonFontTextSurfaceError::NonzeroPixels {
                surface_id: surface_spec.surface_id,
                expected: surface_spec.nonzero_pixels,
                actual: nonzero_pixels,
            });
        }
        let pixel_sha256 = format!("{:x}", Sha256::digest(&crop));
        if pixel_sha256 != surface_spec.pixel_sha256 {
            return Err(CommonFontTextSurfaceError::PixelDigest {
                surface_id: surface_spec.surface_id,
                expected: surface_spec.pixel_sha256,
                actual: pixel_sha256,
            });
        }
        rows.push(CommonFontTextSurface {
            spec: surface_spec,
            pixel_sha256,
            pixel_surface_exact: true,
            transcription_status: "visual-review-candidate",
            runtime_draw_verified: false,
            translation_approved: false,
            release_eligible: false,
        });
    }

    Ok(CommonFontTextSurfaceReport {
        schema_version: COMMON_FONT_TEXT_SURFACE_SCHEMA_VERSION,
        physical_surfaces: rows.len(),
        physical_archives: rows
            .iter()
            .map(|row| row.spec.archive)
            .collect::<BTreeSet<_>>()
            .len(),
        unique_source_texts: rows
            .iter()
            .map(|row| row.spec.source_text)
            .collect::<BTreeSet<_>>()
            .len(),
        unique_pixel_surfaces: rows
            .iter()
            .map(|row| row.spec.pixel_sha256)
            .collect::<BTreeSet<_>>()
            .len(),
        representative_runtime_cell_bindings: rows
            .iter()
            .map(|row| row.spec.representative_runtime_cell_candidates.len())
            .sum(),
        rows,
    })
}

/// 원본 대조용 일본어 surface 후보를 결정적 TSV로 만든다.
pub fn render_common_font_text_surfaces_tsv(report: &CommonFontTextSurfaceReport) -> String {
    let mut output = String::from(
        "schema_version\tsurface_id\tarchive\tsource_text\tsemantic_category\tx\ty\twidth\t\
         height\trepresentative_runtime_cell_candidates\tnonzero_pixels\tpixel_sha256\t\
         pixel_surface_exact\ttranscription_status\truntime_draw_verified\t\
         translation_approved\trelease_eligible\n",
    );
    for row in &report.rows {
        let cells = row
            .spec
            .representative_runtime_cell_candidates
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            report.schema_version,
            row.spec.surface_id,
            row.spec.archive,
            row.spec.source_text.replace('\n', "\\n"),
            row.spec.semantic_category,
            row.spec.x,
            row.spec.y,
            row.spec.width,
            row.spec.height,
            cells,
            row.spec.nonzero_pixels,
            row.pixel_sha256,
            row.pixel_surface_exact,
            row.transcription_status,
            row.runtime_draw_verified,
            row.translation_approved,
            row.release_eligible,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "common_text_tests.rs"]
mod common_text_tests;
