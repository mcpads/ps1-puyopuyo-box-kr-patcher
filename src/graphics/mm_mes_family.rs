//! PUYO1/PUYO2 `MM_MES.BTX` 두 baked-text surface의 원본 경계.
//!
//! 두 자산은 16개 가로 band에 규칙·option 문구가 연속 raster로 구워진
//! 거의 같은 512×256 surface다. 전체 픽셀과 검토용 점유 구간을 독립적으로
//! 검산하고, `NEXT` 상태 한 곳만 다른 사실을 source gate로 고정한다.
//! 16×16 grid는 저장 수용량을 재는 합성 stress partition일 뿐 실제 문구,
//! token, renderer cell 또는 화면 소비 단위로 승격하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::{decompress, parse_header};

use crate::btx::{decode, Layout, PixelFormat};

const PP1_ARCHIVE: &str = "/PUYO1/PP1_GRP.VDD;1";
const PP2_ARCHIVE: &str = "/PUYO2/PP2_GRP.VDD;1";
pub const MM_MES_ASSET_NAME: &str = "MM_MES.BTX";
const PP1_STORED_BYTES: u32 = 43_649;
const PP2_STORED_BYTES: u32 = 43_285;
const PP1_SHA256: &str = "b8c3fdabc55c3abe09938f53e6482e8d58ef972ef36c2c08cde8418ff9721f70";
const PP2_SHA256: &str = "8b8ef104858a692f2f9a1127e7d9ac792b1aaa577a28d02cced1c6e67525044c";

pub const MM_MES_LAYOUT: Layout = Layout {
    width: 512,
    height: 256,
    format: PixelFormat::Indexed4,
};
/// 제품 layout이 아닌 synthetic storage stress partition의 cell 폭.
pub const MM_MES_STORAGE_CELL_WIDTH: usize = 16;
/// 제품 layout이 아닌 synthetic storage stress partition의 cell 높이.
pub const MM_MES_STORAGE_CELL_HEIGHT: usize = 16;
pub const MM_MES_STORAGE_COLUMNS: usize = MM_MES_LAYOUT.width / MM_MES_STORAGE_CELL_WIDTH;
pub const MM_MES_STORAGE_ROWS: usize = MM_MES_LAYOUT.height / MM_MES_STORAGE_CELL_HEIGHT;
pub const MM_MES_STORAGE_CELLS: usize = MM_MES_STORAGE_COLUMNS * MM_MES_STORAGE_ROWS;
pub const MM_MES_BAKED_BAND_HEIGHT: usize = 16;
pub const MM_MES_BAKED_BANDS: usize = MM_MES_LAYOUT.height / MM_MES_BAKED_BAND_HEIGHT;
/// 검토용 점유 구간에서 하나의 문구 안쪽으로 볼 최대 공백은 5px다.
pub const MM_MES_REVIEW_RUN_MERGE_THRESHOLD: usize = 6;

/// 서로 다른 EXE·VDD 원본을 섞지 않기 위한 `MM_MES` 물리 사본 식별자.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MmMesVariant {
    Puyo1,
    Puyo2,
}

impl MmMesVariant {
    pub const fn scope(self) -> &'static str {
        match self {
            Self::Puyo1 => "PUYO1",
            Self::Puyo2 => "PUYO2",
        }
    }

    pub const fn archive_path(self) -> &'static str {
        match self {
            Self::Puyo1 => PP1_ARCHIVE,
            Self::Puyo2 => PP2_ARCHIVE,
        }
    }

    pub const fn stored_bytes(self) -> u32 {
        match self {
            Self::Puyo1 => PP1_STORED_BYTES,
            Self::Puyo2 => PP2_STORED_BYTES,
        }
    }

    pub const fn source_sha256(self) -> &'static str {
        match self {
            Self::Puyo1 => PP1_SHA256,
            Self::Puyo2 => PP2_SHA256,
        }
    }
}

pub const BOX_MM_MES_VARIANTS: [MmMesVariant; 2] = [MmMesVariant::Puyo1, MmMesVariant::Puyo2];

/// 두 baked-text surface 사이에서 달라지는 최소 사각형.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmMesDifferenceRect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// 두 `MM_MES`를 합성 storage stress용 16×16 grid로 나눈 exact bitmap 관계.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmMesStorageGridAnalysis {
    pub columns: usize,
    pub rows: usize,
    pub cells: usize,
    pub pp1_active_cells: usize,
    pub pp1_blank_cells: usize,
    pub pp2_active_cells: usize,
    pub pp2_blank_cells: usize,
    pub pp1_unique_cells: usize,
    pub pp2_unique_cells: usize,
    pub shared_exact_cells: usize,
    pub shared_unique_active_cells: usize,
    pub variant_cells: [usize; 4],
    pub pp1_active_cells_per_row: [usize; MM_MES_STORAGE_ROWS],
    pub pp2_active_cells_per_row: [usize; MM_MES_STORAGE_ROWS],
}

/// 한 baked-text band에서 nonzero pixel을 포함하는 가로 구간. 끝은 exclusive다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmMesHorizontalRun {
    pub start_x: usize,
    pub end_x: usize,
}

/// 문구 전사 검토를 위한 16개 가로 band의 보수적 점유 구간.
///
/// 5px 이하의 내부 공백은 이어 붙인다. 이 구간은 문구 bbox 후보이지 실제
/// renderer block이나 번역 단위가 아니다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmMesBakedSurfaceAnalysis {
    pub band_height: usize,
    pub bands: usize,
    pub pp1_occupied_runs: Vec<Vec<MmMesHorizontalRun>>,
    pub pp2_occupied_runs: Vec<Vec<MmMesHorizontalRun>>,
}

/// 지원 원본의 두 `MM_MES` 사본 관계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmMesFamilyAnalysis {
    pub pp1_stored_bytes: u32,
    pub pp2_stored_bytes: u32,
    pub plain_bytes: usize,
    pub layout: Layout,
    pub changed_pixels: usize,
    pub difference_rect: MmMesDifferenceRect,
    pub pp1_palette_histogram: [usize; 16],
    pub pp2_palette_histogram: [usize; 16],
    pub storage_grid: MmMesStorageGridAnalysis,
    pub baked_surface: MmMesBakedSurfaceAnalysis,
}

/// `MM_MES` 두 사본 source gate 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmMesFamilyError(String);

impl fmt::Display for MmMesFamilyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for MmMesFamilyError {}

fn error(message: impl Into<String>) -> MmMesFamilyError {
    MmMesFamilyError(message.into())
}

fn extract(archive: &VddArchive, variant: MmMesVariant) -> Result<Vec<u8>, MmMesFamilyError> {
    let archive_path = variant.archive_path();
    let matches = archive
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.name().eq_ignore_ascii_case(MM_MES_ASSET_NAME))
        .collect::<Vec<_>>();
    let [(_, entry)] = matches.as_slice() else {
        return Err(error(format!(
            "{archive_path}:{MM_MES_ASSET_NAME} 엔트리 수가 1이 아님: {}",
            matches.len()
        )));
    };
    if entry.size != variant.stored_bytes() {
        return Err(error(format!(
            "{archive_path}:{MM_MES_ASSET_NAME} VDD stored 크기 불일치: {}",
            entry.size
        )));
    }

    let compressed = archive.read_file(entry);
    let header = parse_header(compressed)
        .map_err(|source| error(format!("{archive_path}:{MM_MES_ASSET_NAME}: {source}")))?;
    if header.subtype != *b"bi_" {
        return Err(error(format!(
            "{archive_path}:{MM_MES_ASSET_NAME} CNX subtype 불일치: {:?}",
            header.subtype
        )));
    }
    let plain = decompress(compressed)
        .map_err(|source| error(format!("{archive_path}:{MM_MES_ASSET_NAME}: {source}")))?;
    verify_mm_mes_plain(variant, &plain)?;
    Ok(plain)
}

/// 물리 사본별 원본 digest와 512×256 indexed4 길이를 검증한다.
pub fn verify_mm_mes_plain(variant: MmMesVariant, plain: &[u8]) -> Result<(), MmMesFamilyError> {
    if plain.len() != MM_MES_LAYOUT.byte_len().expect("상수 layout") {
        return Err(error(format!(
            "{}:{MM_MES_ASSET_NAME} 해제 크기 불일치: {}",
            variant.archive_path(),
            plain.len()
        )));
    }
    let actual_sha256 = format!("{:x}", Sha256::digest(plain));
    if actual_sha256 != variant.source_sha256() {
        return Err(error(format!(
            "{}:{MM_MES_ASSET_NAME} SHA-256 불일치: {actual_sha256}",
            variant.archive_path()
        )));
    }
    Ok(())
}

fn pixel_difference(
    first: &[u8],
    second: &[u8],
    layout: Layout,
) -> Result<(usize, MmMesDifferenceRect), MmMesFamilyError> {
    let first = decode(first, layout).map_err(|source| error(source.to_string()))?;
    let second = decode(second, layout).map_err(|source| error(source.to_string()))?;
    let mut changed_pixels = 0usize;
    let mut minimum_x = layout.width;
    let mut minimum_y = layout.height;
    let mut maximum_x = 0usize;
    let mut maximum_y = 0usize;

    for (offset, (left, right)) in first.iter().zip(&second).enumerate() {
        if left == right {
            continue;
        }
        changed_pixels += 1;
        let x = offset % layout.width;
        let y = offset / layout.width;
        minimum_x = minimum_x.min(x);
        minimum_y = minimum_y.min(y);
        maximum_x = maximum_x.max(x);
        maximum_y = maximum_y.max(y);
    }
    if changed_pixels == 0 {
        return Err(error("MM_MES 두 사본의 픽셀이 모두 같음"));
    }
    Ok((
        changed_pixels,
        MmMesDifferenceRect {
            x: minimum_x,
            y: minimum_y,
            width: maximum_x - minimum_x + 1,
            height: maximum_y - minimum_y + 1,
        },
    ))
}

fn cell_pixels(pixels: &[u8], cell: usize) -> Result<Vec<u8>, MmMesFamilyError> {
    if pixels.len() != MM_MES_LAYOUT.width * MM_MES_LAYOUT.height || cell >= MM_MES_STORAGE_CELLS {
        return Err(error(format!(
            "MM_MES cell 입력 범위 오류: pixels={}, cell={cell}",
            pixels.len()
        )));
    }
    let x = (cell % MM_MES_STORAGE_COLUMNS) * MM_MES_STORAGE_CELL_WIDTH;
    let y = (cell / MM_MES_STORAGE_COLUMNS) * MM_MES_STORAGE_CELL_HEIGHT;
    let mut output = Vec::with_capacity(MM_MES_STORAGE_CELL_WIDTH * MM_MES_STORAGE_CELL_HEIGHT);
    for row in 0..MM_MES_STORAGE_CELL_HEIGHT {
        let start = (y + row) * MM_MES_LAYOUT.width + x;
        output.extend_from_slice(&pixels[start..start + MM_MES_STORAGE_CELL_WIDTH]);
    }
    Ok(output)
}

fn storage_grid_analysis(
    pp1_plain: &[u8],
    pp2_plain: &[u8],
) -> Result<MmMesStorageGridAnalysis, MmMesFamilyError> {
    let pp1 = decode(pp1_plain, MM_MES_LAYOUT).map_err(|source| error(source.to_string()))?;
    let pp2 = decode(pp2_plain, MM_MES_LAYOUT).map_err(|source| error(source.to_string()))?;
    let mut pp1_cells = Vec::with_capacity(MM_MES_STORAGE_CELLS);
    let mut pp2_cells = Vec::with_capacity(MM_MES_STORAGE_CELLS);
    let mut pp1_active_cells_per_row = [0usize; MM_MES_STORAGE_ROWS];
    let mut pp2_active_cells_per_row = [0usize; MM_MES_STORAGE_ROWS];

    for cell in 0..MM_MES_STORAGE_CELLS {
        let first = cell_pixels(&pp1, cell)?;
        let second = cell_pixels(&pp2, cell)?;
        pp1_active_cells_per_row[cell / MM_MES_STORAGE_COLUMNS] +=
            usize::from(first.iter().any(|&v| v != 0));
        pp2_active_cells_per_row[cell / MM_MES_STORAGE_COLUMNS] +=
            usize::from(second.iter().any(|&v| v != 0));
        pp1_cells.push(first);
        pp2_cells.push(second);
    }

    let pp1_active_cells = pp1_cells
        .iter()
        .filter(|cell| cell.iter().any(|&value| value != 0))
        .count();
    let pp2_active_cells = pp2_cells
        .iter()
        .filter(|cell| cell.iter().any(|&value| value != 0))
        .count();
    let variant_cells = pp1_cells
        .iter()
        .zip(&pp2_cells)
        .enumerate()
        .filter_map(|(index, (first, second))| (first != second).then_some(index))
        .collect::<Vec<_>>();
    let variant_cells: [usize; 4] = variant_cells.try_into().map_err(|cells: Vec<usize>| {
        error(format!(
            "MM_MES variant cell 수 불일치: expected=4, actual={}, cells={cells:?}",
            cells.len()
        ))
    })?;
    let shared_unique_active_cells = pp1_cells
        .iter()
        .zip(&pp2_cells)
        .filter(|(first, second)| first == second && first.iter().any(|&value| value != 0))
        .map(|(first, _)| first)
        .collect::<BTreeSet<_>>()
        .len();

    Ok(MmMesStorageGridAnalysis {
        columns: MM_MES_STORAGE_COLUMNS,
        rows: MM_MES_STORAGE_ROWS,
        cells: MM_MES_STORAGE_CELLS,
        pp1_active_cells,
        pp1_blank_cells: MM_MES_STORAGE_CELLS - pp1_active_cells,
        pp2_active_cells,
        pp2_blank_cells: MM_MES_STORAGE_CELLS - pp2_active_cells,
        pp1_unique_cells: pp1_cells.iter().collect::<BTreeSet<_>>().len(),
        pp2_unique_cells: pp2_cells.iter().collect::<BTreeSet<_>>().len(),
        shared_exact_cells: pp1_cells
            .iter()
            .zip(&pp2_cells)
            .filter(|(first, second)| first == second)
            .count(),
        shared_unique_active_cells,
        variant_cells,
        pp1_active_cells_per_row,
        pp2_active_cells_per_row,
    })
}

fn palette_histogram(plain: &[u8]) -> Result<[usize; 16], MmMesFamilyError> {
    let pixels = decode(plain, MM_MES_LAYOUT).map_err(|source| error(source.to_string()))?;
    let mut histogram = [0usize; 16];
    for pixel in pixels {
        let count = histogram
            .get_mut(usize::from(pixel))
            .ok_or_else(|| error(format!("MM_MES palette index 범위 이탈: {pixel}")))?;
        *count += 1;
    }
    Ok(histogram)
}

fn occupied_runs(
    pixels: &[u8],
    layout: Layout,
    band_height: usize,
    merge_threshold: usize,
) -> Result<Vec<Vec<MmMesHorizontalRun>>, MmMesFamilyError> {
    if pixels.len() != layout.width * layout.height
        || band_height == 0
        || !layout.height.is_multiple_of(band_height)
        || merge_threshold == 0
    {
        return Err(error(format!(
            "MM_MES baked band 입력 오류: pixels={}, layout={}x{}, band={band_height}, merge={merge_threshold}",
            pixels.len(),
            layout.width,
            layout.height
        )));
    }

    let mut bands = Vec::with_capacity(layout.height / band_height);
    for band in 0..layout.height / band_height {
        let start_y = band * band_height;
        let mut occupied = vec![false; layout.width];
        for y in start_y..start_y + band_height {
            for x in 0..layout.width {
                occupied[x] |= pixels[y * layout.width + x] != 0;
            }
        }

        let mut raw_runs = Vec::new();
        let mut x = 0usize;
        while x < layout.width {
            if !occupied[x] {
                x += 1;
                continue;
            }
            let start_x = x;
            while x < layout.width && occupied[x] {
                x += 1;
            }
            raw_runs.push(MmMesHorizontalRun { start_x, end_x: x });
        }

        let mut merged_runs: Vec<MmMesHorizontalRun> = Vec::new();
        for run in raw_runs {
            if let Some(previous) = merged_runs.last_mut() {
                let gap = run.start_x.saturating_sub(previous.end_x);
                if gap < merge_threshold {
                    previous.end_x = run.end_x;
                    continue;
                }
            }
            merged_runs.push(run);
        }
        bands.push(merged_runs);
    }
    Ok(bands)
}

fn runs(runs: &[(usize, usize)]) -> Vec<MmMesHorizontalRun> {
    runs.iter()
        .copied()
        .map(|(start_x, end_x)| MmMesHorizontalRun { start_x, end_x })
        .collect()
}

fn expected_baked_surface() -> MmMesBakedSurfaceAnalysis {
    let pp1_occupied_runs = vec![
        runs(&[(1, 223), (257, 512)]),
        runs(&[(0, 511)]),
        runs(&[(0, 224), (256, 512)]),
        runs(&[(1, 512)]),
        runs(&[(2, 240), (256, 512)]),
        runs(&[(0, 208), (256, 443), (450, 503)]),
        runs(&[(1, 240), (256, 512)]),
        runs(&[(0, 240), (256, 512)]),
        runs(&[(1, 27), (34, 173), (179, 512)]),
        runs(&[(0, 512)]),
        runs(&[(0, 240), (256, 463)]),
        runs(&[(1, 496)]),
        runs(&[(0, 512)]),
        runs(&[(0, 143), (161, 512)]),
        runs(&[(0, 128)]),
        runs(&[(0, 96)]),
    ];
    let mut pp2_occupied_runs = pp1_occupied_runs.clone();
    pp2_occupied_runs[10] = runs(&[(0, 240), (256, 400)]);
    MmMesBakedSurfaceAnalysis {
        band_height: MM_MES_BAKED_BAND_HEIGHT,
        bands: MM_MES_BAKED_BANDS,
        pp1_occupied_runs,
        pp2_occupied_runs,
    }
}

/// 지원 원본 VDD에서 두 baked-text surface와 한 변형이 정확히 닫히는지 검증한다.
pub fn verify_box_mm_mes_family<'a>(
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<MmMesFamilyAnalysis, MmMesFamilyError> {
    let mut pp1 = None;
    let mut pp2 = None;
    for (archive_path, archive) in archives {
        let target = match archive_path {
            PP1_ARCHIVE => &mut pp1,
            PP2_ARCHIVE => &mut pp2,
            _ => continue,
        };
        if target.replace(archive).is_some() {
            return Err(error(format!("{archive_path} archive가 중복됨")));
        }
    }
    let pp1 = pp1.ok_or_else(|| error(format!("{PP1_ARCHIVE} archive가 없음")))?;
    let pp2 = pp2.ok_or_else(|| error(format!("{PP2_ARCHIVE} archive가 없음")))?;
    let pp1_plain = extract(pp1, MmMesVariant::Puyo1)?;
    let pp2_plain = extract(pp2, MmMesVariant::Puyo2)?;
    let (changed_pixels, difference_rect) =
        pixel_difference(&pp1_plain, &pp2_plain, MM_MES_LAYOUT)?;
    let expected_rect = MmMesDifferenceRect {
        x: 401,
        y: 160,
        width: 62,
        height: 16,
    };
    if changed_pixels != 618 || difference_rect != expected_rect {
        return Err(error(format!(
            "MM_MES variant 경계 불일치: changed={changed_pixels}, rect={difference_rect:?}"
        )));
    }
    let pp1_palette_histogram = palette_histogram(&pp1_plain)?;
    let pp2_palette_histogram = palette_histogram(&pp2_plain)?;
    let expected_palette_histograms = (
        [
            57_319, 24_055, 4_148, 3_053, 3_436, 2_538, 2_925, 4_071, 2_613, 3_278, 2_897, 2_661,
            4_184, 13_892, 0, 2,
        ],
        [
            57_937, 23_902, 4_120, 3_023, 3_412, 2_527, 2_902, 4_028, 2_600, 3_257, 2_882, 2_652,
            4_160, 13_668, 0, 2,
        ],
    );
    if (pp1_palette_histogram, pp2_palette_histogram) != expected_palette_histograms {
        return Err(error(format!(
            "MM_MES palette histogram 계약 불일치: PP1={pp1_palette_histogram:?}, \
             PP2={pp2_palette_histogram:?}"
        )));
    }
    let storage_grid = storage_grid_analysis(&pp1_plain, &pp2_plain)?;
    let expected_storage_grid = MmMesStorageGridAnalysis {
        columns: 32,
        rows: 16,
        cells: 512,
        pp1_active_cells: 446,
        pp1_blank_cells: 66,
        pp2_active_cells: 442,
        pp2_blank_cells: 70,
        pp1_unique_cells: 178,
        pp2_unique_cells: 178,
        shared_exact_cells: 508,
        shared_unique_active_cells: 177,
        variant_cells: [345, 346, 347, 348],
        pp1_active_cells_per_row: [30, 32, 30, 32, 31, 29, 31, 31, 32, 32, 28, 31, 32, 31, 8, 6],
        pp2_active_cells_per_row: [30, 32, 30, 32, 31, 29, 31, 31, 32, 32, 24, 31, 32, 31, 8, 6],
    };
    if storage_grid != expected_storage_grid {
        return Err(error(format!(
            "MM_MES synthetic 16x16 storage grid 계약 불일치: {storage_grid:?}"
        )));
    }
    let pp1_pixels =
        decode(&pp1_plain, MM_MES_LAYOUT).map_err(|source| error(source.to_string()))?;
    let pp2_pixels =
        decode(&pp2_plain, MM_MES_LAYOUT).map_err(|source| error(source.to_string()))?;
    let baked_surface = MmMesBakedSurfaceAnalysis {
        band_height: MM_MES_BAKED_BAND_HEIGHT,
        bands: MM_MES_BAKED_BANDS,
        pp1_occupied_runs: occupied_runs(
            &pp1_pixels,
            MM_MES_LAYOUT,
            MM_MES_BAKED_BAND_HEIGHT,
            MM_MES_REVIEW_RUN_MERGE_THRESHOLD,
        )?,
        pp2_occupied_runs: occupied_runs(
            &pp2_pixels,
            MM_MES_LAYOUT,
            MM_MES_BAKED_BAND_HEIGHT,
            MM_MES_REVIEW_RUN_MERGE_THRESHOLD,
        )?,
    };
    let expected_baked_surface = expected_baked_surface();
    if baked_surface != expected_baked_surface {
        return Err(error(format!(
            "MM_MES baked-text band 점유 구간 계약 불일치: {baked_surface:?}"
        )));
    }

    Ok(MmMesFamilyAnalysis {
        pp1_stored_bytes: PP1_STORED_BYTES,
        pp2_stored_bytes: PP2_STORED_BYTES,
        plain_bytes: pp1_plain.len(),
        layout: MM_MES_LAYOUT,
        changed_pixels,
        difference_rect,
        pp1_palette_histogram,
        pp2_palette_histogram,
        storage_grid,
        baked_surface,
    })
}

#[cfg(test)]
#[path = "mm_mes_family_tests.rs"]
mod tests;
