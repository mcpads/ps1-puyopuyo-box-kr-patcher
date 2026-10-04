//! BTX 픽셀 텍스트 block의 버전 관리 계약.
//!
//! 사람은 원문과 bbox, style index를 판정한다. 이 모듈은 그 판정을 지원
//! 원본의 해제 픽셀과 대조하고, 범위·중복·겹침·digest drift를 실패시킨다.
//! 실제 번역 픽셀 생성과 BTX 교체는 아직 이 모듈의 범위가 아니다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::btx::{decode, BtxError, Layout, PixelFormat};

mod boxopt;
mod g_mess;
mod kanban;
mod puyo1_assets;
mod puyo2_assets;
mod puyo_endings;
mod shell_assets;

pub use boxopt::*;
pub use g_mess::*;
pub use kanban::*;
pub use puyo1_assets::*;
pub use puyo2_assets::*;
pub use puyo_endings::{
    P1_OVER3_TEXT_ASSET, P2_END10_TEXT_ASSET, P2_END15_TEXT_ASSET, P2_END16_TEXT_ASSET,
};
pub use shell_assets::*;

/// 행 우선 BTX 픽셀 좌표의 반열린 직사각형.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Rect {
    fn end(self) -> Option<(usize, usize)> {
        Some((
            self.x.checked_add(self.width)?,
            self.y.checked_add(self.height)?,
        ))
    }

    fn overlaps(self, other: Self) -> bool {
        let Some((right, bottom)) = self.end() else {
            return true;
        };
        let Some((other_right, other_bottom)) = other.end() else {
            return true;
        };
        self.x < other_right && other.x < right && self.y < other_bottom && other.y < bottom
    }
}

/// bbox에서 관측할 raw palette index의 exact 집합 표현.
///
/// 작은 집합은 사람이 읽을 수 있는 정렬 목록으로, indexed8의 큰 집합은
/// index 0부터 255까지를 나타내는 네 `u64` word로 선언한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteIndexSpec<'a> {
    Values(&'a [u8]),
    Bitset256([u64; 4]),
}

/// 사람이 승인한 한 텍스트 block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextBlockSpec<'a> {
    pub id: &'a str,
    pub source: &'a str,
    pub bbox: Rect,
    /// bbox 안에서 정확히 관측돼야 하는 raw palette index 집합.
    pub palette_indices: PaletteIndexSpec<'a>,
    /// bbox를 행 우선 1B/pixel로 편 결과의 SHA-256.
    pub expected_pixels_sha256: &'a str,
}

/// baked text의 물리 저장 형태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextStorageSpec<'a> {
    /// bbox 하나가 그대로 합성 가능한 독립 raster block이다.
    RasterBlocks,
    /// selector bbox가 같은 atlas의 픽셀을 일부 공유하는 sparse page다.
    SparseSelectorAtlas(SparseSelectorAtlasSpec<'a>),
    /// 문장 한 단위를 고정 크기 글리프 셀로 행 우선 조립한 page다.
    FixedCellGrid(FixedCellGridSpec<'a>),
}

/// 겹침을 포함하는 selector atlas의 원본 계약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SparseSelectorAtlasSpec<'a> {
    /// 사전식으로 정렬한 `(작은 block ID, 큰 block ID)` exact 겹침 쌍.
    pub allowed_overlap_pairs: &'a [(&'a str, &'a str)],
    /// 모든 selector bbox가 차지하는 물리 픽셀 union.
    pub expected_physical_union_pixels: usize,
}

/// 행 우선 고정 셀 page의 원본 계약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedCellGridSpec<'a> {
    pub cell_width: usize,
    pub cell_height: usize,
    pub expected_active_cells: usize,
    pub expected_empty_cells: usize,
    pub expected_unique_glyphs: usize,
    /// 원문이 있는 각 셀에서 정확히 관측할 palette index 집합.
    pub active_palette_indices: PaletteIndexSpec<'a>,
    /// 원문 뒤의 미사용 tail 셀에서 정확히 관측할 palette index 집합.
    pub empty_palette_indices: PaletteIndexSpec<'a>,
}

/// 같은 해제 픽셀 내용을 가진 물리 BTX들의 텍스트 계약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextAssetSpec<'a> {
    pub archive_paths: &'a [&'a str],
    pub name: &'a str,
    pub layout: Layout,
    pub expected_decompressed_sha256: &'a str,
    pub storage: TextStorageSpec<'a>,
    pub blocks: &'a [TextBlockSpec<'a>],
}

/// 원본과 대조한 텍스트 block 요약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextCatalogReport {
    pub declared_physical_assets: usize,
    pub blocks: usize,
    pub block_pixels: usize,
    pub japanese_target_blocks: usize,
    pub japanese_target_pixels: usize,
}

/// typed catalog 전체의 선언 분모.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextCatalogSpecReport {
    pub assets: usize,
    pub declared_physical_assets: usize,
    pub blocks: usize,
    pub japanese_target_blocks: usize,
}

/// BTX 텍스트 계약 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextCatalogError {
    Btx(BtxError),
    EmptyCatalog,
    EmptyArchivePaths,
    EmptyArchivePath,
    DuplicateArchivePath(String),
    DuplicatePhysicalAsset {
        archive_path: String,
        name: String,
    },
    EmptyAssetName,
    EmptyBlockCatalog,
    DecompressedDigestMismatch {
        expected: String,
        actual: String,
    },
    EmptyBlockId,
    EmptySource {
        id: String,
    },
    DuplicateBlockId(String),
    EmptyBbox {
        id: String,
    },
    BboxOverflow {
        id: String,
    },
    BboxOutOfBounds {
        id: String,
        bbox: Rect,
        layout: Layout,
    },
    OverlappingBlocks {
        first: String,
        second: String,
    },
    SparseOverlapPairNotCanonical {
        first: String,
        second: String,
    },
    SparseUnknownOverlapBlock(String),
    SparseDuplicateOverlapPair {
        first: String,
        second: String,
    },
    SparseOverlapSetMismatch {
        expected: Vec<(String, String)>,
        actual: Vec<(String, String)>,
    },
    SparsePhysicalUnionMismatch {
        expected: usize,
        actual: usize,
    },
    EmptyPaletteIndices {
        id: String,
    },
    UnsortedPaletteIndices {
        id: String,
    },
    PaletteIndexOutOfRange {
        id: String,
        value: u8,
        format: PixelFormat,
    },
    PaletteSetMismatch {
        id: String,
        expected: Vec<u8>,
        actual: Vec<u8>,
    },
    PixelDigestMismatch {
        id: String,
        expected: String,
        actual: String,
    },
    FixedGridSingleBlockRequired {
        name: String,
        actual: usize,
    },
    FixedGridCellSizeInvalid {
        name: String,
        cell_width: usize,
        cell_height: usize,
    },
    FixedGridBlockMustCoverAsset {
        id: String,
        bbox: Rect,
        layout: Layout,
    },
    FixedGridCarriageReturn {
        id: String,
    },
    FixedGridRowCountOutOfRange {
        id: String,
        rows: usize,
        maximum: usize,
    },
    FixedGridRowLengthMismatch {
        id: String,
        row: usize,
        expected: usize,
        actual: usize,
    },
    FixedGridCellCountMismatch {
        id: String,
        expected_active: usize,
        actual_active: usize,
        expected_empty: usize,
        actual_empty: usize,
    },
    FixedGridPaletteSetMismatch {
        id: String,
        cell: usize,
        active: bool,
        expected: Vec<u8>,
        actual: Vec<u8>,
    },
    FixedGridCharacterBitmapMismatch {
        id: String,
        character: char,
        first_cell: usize,
        second_cell: usize,
    },
    FixedGridBitmapCharacterMismatch {
        id: String,
        first_character: char,
        second_character: char,
        first_cell: usize,
        second_cell: usize,
    },
    FixedGridUniqueGlyphCountMismatch {
        id: String,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for TextCatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Btx(source) => write!(f, "BTX decode 실패: {source}"),
            Self::EmptyCatalog => f.write_str("BTX 텍스트 catalog가 비어 있음"),
            Self::EmptyArchivePaths => f.write_str("BTX 텍스트 자산의 물리 archive 경로가 없음"),
            Self::EmptyArchivePath => f.write_str("BTX 텍스트 자산의 archive 경로가 비어 있음"),
            Self::DuplicateArchivePath(path) => {
                write!(f, "BTX 텍스트 자산의 archive 경로 중복: {path}")
            }
            Self::DuplicatePhysicalAsset { archive_path, name } => {
                write!(f, "BTX 텍스트 물리 자산 중복: {archive_path}:{name}")
            }
            Self::EmptyAssetName => f.write_str("BTX 텍스트 자산 이름이 비어 있음"),
            Self::EmptyBlockCatalog => f.write_str("BTX 텍스트 block 목록이 비어 있음"),
            Self::DecompressedDigestMismatch { expected, actual } => write!(
                f,
                "BTX 해제 원문 SHA-256 불일치: expected={expected}, actual={actual}"
            ),
            Self::EmptyBlockId => f.write_str("BTX 텍스트 block ID가 비어 있음"),
            Self::EmptySource { id } => write!(f, "{id}: 원문이 비어 있음"),
            Self::DuplicateBlockId(id) => write!(f, "BTX 텍스트 block ID 중복: {id}"),
            Self::EmptyBbox { id } => write!(f, "{id}: bbox 크기가 0"),
            Self::BboxOverflow { id } => write!(f, "{id}: bbox 좌표 overflow"),
            Self::BboxOutOfBounds { id, bbox, layout } => write!(
                f,
                "{id}: bbox ({},{},{},{})가 layout {}x{} 밖임",
                bbox.x, bbox.y, bbox.width, bbox.height, layout.width, layout.height
            ),
            Self::OverlappingBlocks { first, second } => {
                write!(f, "BTX 텍스트 block bbox 겹침: {first} / {second}")
            }
            Self::SparseOverlapPairNotCanonical { first, second } => write!(
                f,
                "sparse selector 겹침 쌍은 서로 다른 ID를 사전식으로 선언해야 함: {first} / {second}"
            ),
            Self::SparseUnknownOverlapBlock(id) => {
                write!(f, "sparse selector 겹침 쌍에 catalog에 없는 block이 있음: {id}")
            }
            Self::SparseDuplicateOverlapPair { first, second } => {
                write!(f, "sparse selector 겹침 쌍 중복: {first} / {second}")
            }
            Self::SparseOverlapSetMismatch { expected, actual } => write!(
                f,
                "sparse selector 실제 겹침 쌍 불일치: expected={expected:?}, actual={actual:?}"
            ),
            Self::SparsePhysicalUnionMismatch { expected, actual } => write!(
                f,
                "sparse selector 물리 union 불일치: expected={expected}, actual={actual}"
            ),
            Self::EmptyPaletteIndices { id } => write!(f, "{id}: palette index 집합이 비어 있음"),
            Self::UnsortedPaletteIndices { id } => {
                write!(f, "{id}: palette index는 오름차순·중복 없음이어야 함")
            }
            Self::PaletteIndexOutOfRange { id, value, format } => {
                write!(f, "{id}: {:?} palette index 범위 초과: {value}", format)
            }
            Self::PaletteSetMismatch {
                id,
                expected,
                actual,
            } => write!(
                f,
                "{id}: palette index 집합 불일치: expected={expected:?}, actual={actual:?}"
            ),
            Self::PixelDigestMismatch {
                id,
                expected,
                actual,
            } => write!(
                f,
                "{id}: bbox 픽셀 SHA-256 불일치: expected={expected}, actual={actual}"
            ),
            Self::FixedGridSingleBlockRequired { name, actual } => write!(
                f,
                "{name}: fixed cell grid는 문장 translation block 하나여야 함: actual={actual}"
            ),
            Self::FixedGridCellSizeInvalid {
                name,
                cell_width,
                cell_height,
            } => write!(
                f,
                "{name}: fixed cell grid 크기가 layout을 정확히 나누지 못함: \
                 cell={cell_width}x{cell_height}"
            ),
            Self::FixedGridBlockMustCoverAsset { id, bbox, layout } => write!(
                f,
                "{id}: fixed cell grid block은 전체 자산을 덮어야 함: \
                 bbox=({},{},{},{}), layout={}x{}",
                bbox.x, bbox.y, bbox.width, bbox.height, layout.width, layout.height
            ),
            Self::FixedGridCarriageReturn { id } => {
                write!(f, "{id}: fixed cell grid 원문에 CR 문자가 있음")
            }
            Self::FixedGridRowCountOutOfRange { id, rows, maximum } => write!(
                f,
                "{id}: fixed cell grid 원문 행 수 범위 이탈: rows={rows}, maximum={maximum}"
            ),
            Self::FixedGridRowLengthMismatch {
                id,
                row,
                expected,
                actual,
            } => write!(
                f,
                "{id}: fixed cell grid 원문 행 길이 불일치: \
                 row={row}, expected={expected}, actual={actual}"
            ),
            Self::FixedGridCellCountMismatch {
                id,
                expected_active,
                actual_active,
                expected_empty,
                actual_empty,
            } => write!(
                f,
                "{id}: fixed cell grid active/empty 셀 수 불일치: \
                 expected={expected_active}/{expected_empty}, \
                 actual={actual_active}/{actual_empty}"
            ),
            Self::FixedGridPaletteSetMismatch {
                id,
                cell,
                active,
                expected,
                actual,
            } => write!(
                f,
                "{id}: fixed cell grid {} cell {cell} palette 불일치: \
                 expected={expected:?}, actual={actual:?}",
                if *active { "active" } else { "empty" }
            ),
            Self::FixedGridCharacterBitmapMismatch {
                id,
                character,
                first_cell,
                second_cell,
            } => write!(
                f,
                "{id}: 같은 문자 {character:?}의 셀 픽셀이 다름: \
                 first={first_cell}, second={second_cell}"
            ),
            Self::FixedGridBitmapCharacterMismatch {
                id,
                first_character,
                second_character,
                first_cell,
                second_cell,
            } => write!(
                f,
                "{id}: 같은 셀 픽셀에 서로 다른 문자를 연결함: \
                 {first_character:?}@{first_cell}, {second_character:?}@{second_cell}"
            ),
            Self::FixedGridUniqueGlyphCountMismatch {
                id,
                expected,
                actual,
            } => write!(
                f,
                "{id}: fixed cell grid 고유 글리프 수 불일치: \
                 expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for TextCatalogError {}

impl From<BtxError> for TextCatalogError {
    fn from(source: BtxError) -> Self {
        Self::Btx(source)
    }
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn palette_limit(format: PixelFormat) -> u8 {
    match format {
        PixelFormat::Indexed4 => 0x0F,
        PixelFormat::Indexed8 => 0xFF,
    }
}

fn palette_values(
    id: &str,
    spec: PaletteIndexSpec<'_>,
    format: PixelFormat,
) -> Result<Vec<u8>, TextCatalogError> {
    let values = match spec {
        PaletteIndexSpec::Values(values) => {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(TextCatalogError::UnsortedPaletteIndices { id: id.to_string() });
            }
            values.to_vec()
        }
        PaletteIndexSpec::Bitset256(words) => (0u16..=u16::from(u8::MAX))
            .filter_map(|index| {
                let word = usize::from(index / 64);
                let bit = u32::from(index % 64);
                (words[word] & (1u64 << bit) != 0).then_some(index as u8)
            })
            .collect(),
    };
    if values.is_empty() {
        return Err(TextCatalogError::EmptyPaletteIndices { id: id.to_string() });
    }
    let limit = palette_limit(format);
    if let Some(&value) = values.iter().find(|&&value| value > limit) {
        return Err(TextCatalogError::PaletteIndexOutOfRange {
            id: id.to_string(),
            value,
            format,
        });
    }
    Ok(values)
}

fn block_pixels(
    pixels: &[u8],
    layout: Layout,
    block: TextBlockSpec<'_>,
) -> Result<Vec<u8>, TextCatalogError> {
    if block.bbox.width == 0 || block.bbox.height == 0 {
        return Err(TextCatalogError::EmptyBbox {
            id: block.id.to_string(),
        });
    }
    let Some((right, bottom)) = block.bbox.end() else {
        return Err(TextCatalogError::BboxOverflow {
            id: block.id.to_string(),
        });
    };
    if right > layout.width || bottom > layout.height {
        return Err(TextCatalogError::BboxOutOfBounds {
            id: block.id.to_string(),
            bbox: block.bbox,
            layout,
        });
    }

    let capacity = block
        .bbox
        .width
        .checked_mul(block.bbox.height)
        .ok_or_else(|| TextCatalogError::BboxOverflow {
            id: block.id.to_string(),
        })?;
    let mut region = Vec::with_capacity(capacity);
    for y in block.bbox.y..bottom {
        let start = y * layout.width + block.bbox.x;
        region.extend_from_slice(&pixels[start..start + block.bbox.width]);
    }
    Ok(region)
}

fn rect_union_pixels(
    layout: Layout,
    blocks: impl IntoIterator<Item = Rect>,
) -> Result<usize, TextCatalogError> {
    let capacity =
        layout
            .width
            .checked_mul(layout.height)
            .ok_or_else(|| TextCatalogError::BboxOverflow {
                id: "sparse-selector-atlas".to_string(),
            })?;
    let mut occupied = vec![false; capacity];
    for bbox in blocks {
        let Some((right, bottom)) = bbox.end() else {
            return Err(TextCatalogError::BboxOverflow {
                id: "sparse-selector-atlas".to_string(),
            });
        };
        if bbox.width == 0 || bbox.height == 0 || right > layout.width || bottom > layout.height {
            return Err(TextCatalogError::BboxOutOfBounds {
                id: "sparse-selector-atlas".to_string(),
                bbox,
                layout,
            });
        }
        for y in bbox.y..bottom {
            for x in bbox.x..right {
                occupied[y * layout.width + x] = true;
            }
        }
    }
    Ok(occupied.into_iter().filter(|value| *value).count())
}

fn verify_sparse_selector_atlas(
    spec: TextAssetSpec<'_>,
    sparse: SparseSelectorAtlasSpec<'_>,
) -> Result<(), TextCatalogError> {
    let ids = spec
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let mut expected = BTreeSet::new();
    for &(first, second) in sparse.allowed_overlap_pairs {
        if first >= second {
            return Err(TextCatalogError::SparseOverlapPairNotCanonical {
                first: first.to_string(),
                second: second.to_string(),
            });
        }
        for id in [first, second] {
            if !ids.contains(id) {
                return Err(TextCatalogError::SparseUnknownOverlapBlock(id.to_string()));
            }
        }
        if !expected.insert((first, second)) {
            return Err(TextCatalogError::SparseDuplicateOverlapPair {
                first: first.to_string(),
                second: second.to_string(),
            });
        }
    }

    let mut actual = BTreeSet::new();
    for (offset, left) in spec.blocks.iter().enumerate() {
        for right in spec.blocks.iter().skip(offset + 1) {
            if left.bbox.overlaps(right.bbox) {
                let pair = if left.id < right.id {
                    (left.id, right.id)
                } else {
                    (right.id, left.id)
                };
                actual.insert(pair);
            }
        }
    }
    if actual != expected {
        return Err(TextCatalogError::SparseOverlapSetMismatch {
            expected: expected
                .into_iter()
                .map(|(first, second)| (first.to_string(), second.to_string()))
                .collect(),
            actual: actual
                .into_iter()
                .map(|(first, second)| (first.to_string(), second.to_string()))
                .collect(),
        });
    }

    let actual_union = rect_union_pixels(spec.layout, spec.blocks.iter().map(|block| block.bbox))?;
    if actual_union != sparse.expected_physical_union_pixels {
        return Err(TextCatalogError::SparsePhysicalUnionMismatch {
            expected: sparse.expected_physical_union_pixels,
            actual: actual_union,
        });
    }
    Ok(())
}

fn fixed_grid_source_chars(
    name: &str,
    layout: Layout,
    block: TextBlockSpec<'_>,
    grid: FixedCellGridSpec<'_>,
) -> Result<Vec<char>, TextCatalogError> {
    if grid.cell_width == 0
        || grid.cell_height == 0
        || !layout.width.is_multiple_of(grid.cell_width)
        || !layout.height.is_multiple_of(grid.cell_height)
    {
        return Err(TextCatalogError::FixedGridCellSizeInvalid {
            name: name.to_string(),
            cell_width: grid.cell_width,
            cell_height: grid.cell_height,
        });
    }
    if block.bbox
        != (Rect {
            x: 0,
            y: 0,
            width: layout.width,
            height: layout.height,
        })
    {
        return Err(TextCatalogError::FixedGridBlockMustCoverAsset {
            id: block.id.to_string(),
            bbox: block.bbox,
            layout,
        });
    }
    if block.source.contains('\r') {
        return Err(TextCatalogError::FixedGridCarriageReturn {
            id: block.id.to_string(),
        });
    }

    let columns = layout.width / grid.cell_width;
    let maximum_rows = layout.height / grid.cell_height;
    let rows = block.source.split('\n').collect::<Vec<_>>();
    if rows.is_empty() || rows.len() > maximum_rows {
        return Err(TextCatalogError::FixedGridRowCountOutOfRange {
            id: block.id.to_string(),
            rows: rows.len(),
            maximum: maximum_rows,
        });
    }
    let mut characters = Vec::new();
    for (row, source) in rows.iter().enumerate() {
        let actual = source.chars().count();
        let is_last = row + 1 == rows.len();
        if actual == 0 || actual > columns || (!is_last && actual != columns) {
            return Err(TextCatalogError::FixedGridRowLengthMismatch {
                id: block.id.to_string(),
                row,
                expected: columns,
                actual,
            });
        }
        characters.extend(source.chars());
    }

    let total_cells = columns * maximum_rows;
    let actual_active = characters.len();
    let actual_empty = total_cells.saturating_sub(actual_active);
    if actual_active != grid.expected_active_cells
        || actual_empty != grid.expected_empty_cells
        || actual_active > total_cells
    {
        return Err(TextCatalogError::FixedGridCellCountMismatch {
            id: block.id.to_string(),
            expected_active: grid.expected_active_cells,
            actual_active,
            expected_empty: grid.expected_empty_cells,
            actual_empty,
        });
    }
    Ok(characters)
}

fn fixed_grid_cell_pixels(
    pixels: &[u8],
    layout: Layout,
    grid: FixedCellGridSpec<'_>,
    cell: usize,
) -> Vec<u8> {
    let columns = layout.width / grid.cell_width;
    let x = (cell % columns) * grid.cell_width;
    let y = (cell / columns) * grid.cell_height;
    let mut region = Vec::with_capacity(grid.cell_width * grid.cell_height);
    for row in 0..grid.cell_height {
        let start = (y + row) * layout.width + x;
        region.extend_from_slice(&pixels[start..start + grid.cell_width]);
    }
    region
}

fn verify_fixed_cell_grid(
    pixels: &[u8],
    spec: TextAssetSpec<'_>,
    grid: FixedCellGridSpec<'_>,
) -> Result<(), TextCatalogError> {
    if spec.blocks.len() != 1 {
        return Err(TextCatalogError::FixedGridSingleBlockRequired {
            name: spec.name.to_string(),
            actual: spec.blocks.len(),
        });
    }
    let block = spec.blocks[0];
    let characters = fixed_grid_source_chars(spec.name, spec.layout, block, grid)?;
    let active_palette = palette_values(block.id, grid.active_palette_indices, spec.layout.format)?;
    let empty_palette = palette_values(block.id, grid.empty_palette_indices, spec.layout.format)?;
    let total_cells = grid.expected_active_cells + grid.expected_empty_cells;

    let mut character_cells = BTreeMap::<char, (String, usize)>::new();
    let mut bitmap_characters = BTreeMap::<String, (char, usize)>::new();
    let mut unique_glyphs = BTreeSet::new();
    for cell in 0..total_cells {
        let region = fixed_grid_cell_pixels(pixels, spec.layout, grid, cell);
        let actual_palette = region
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let active = cell < characters.len();
        let expected_palette = if active {
            &active_palette
        } else {
            &empty_palette
        };
        if &actual_palette != expected_palette {
            return Err(TextCatalogError::FixedGridPaletteSetMismatch {
                id: block.id.to_string(),
                cell,
                active,
                expected: expected_palette.clone(),
                actual: actual_palette,
            });
        }
        if !active {
            continue;
        }
        let character = characters[cell];
        let cell_digest = digest(&region);
        if let Some((first_digest, first_cell)) = character_cells.get(&character) {
            if first_digest != &cell_digest {
                return Err(TextCatalogError::FixedGridCharacterBitmapMismatch {
                    id: block.id.to_string(),
                    character,
                    first_cell: *first_cell,
                    second_cell: cell,
                });
            }
        } else {
            character_cells.insert(character, (cell_digest.clone(), cell));
        }
        if let Some((first_character, first_cell)) = bitmap_characters.get(&cell_digest) {
            if *first_character != character {
                return Err(TextCatalogError::FixedGridBitmapCharacterMismatch {
                    id: block.id.to_string(),
                    first_character: *first_character,
                    second_character: character,
                    first_cell: *first_cell,
                    second_cell: cell,
                });
            }
        } else {
            bitmap_characters.insert(cell_digest.clone(), (character, cell));
        }
        unique_glyphs.insert(cell_digest);
    }
    if unique_glyphs.len() != grid.expected_unique_glyphs {
        return Err(TextCatalogError::FixedGridUniqueGlyphCountMismatch {
            id: block.id.to_string(),
            expected: grid.expected_unique_glyphs,
            actual: unique_glyphs.len(),
        });
    }
    Ok(())
}

/// 원문에 일본어 문자 하나 이상이 있으면 번역 대상으로 분류한다.
///
/// 영문·숫자만 있는 baked label은 원본 보존과 layout 검증 대상이지만
/// 번역 모집단에는 넣지 않는다.
pub fn is_japanese_translation_target(source: &str) -> bool {
    source.chars().any(|character| {
        matches!(
            character,
            '\u{3040}'..='\u{30ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{f900}'..='\u{faff}'
                | '\u{ff66}'..='\u{ff9f}'
        )
    })
}

/// typed catalog 전체에서 물리 자산과 block ID가 유일한지 검증한다.
pub fn verify_text_catalog_specs(
    specs: &[TextAssetSpec<'_>],
) -> Result<TextCatalogSpecReport, TextCatalogError> {
    if specs.is_empty() {
        return Err(TextCatalogError::EmptyCatalog);
    }

    let mut physical_assets = BTreeSet::new();
    let mut block_ids = BTreeSet::new();
    let mut declared_physical_assets = 0usize;
    let mut blocks = 0usize;
    let mut japanese_target_blocks = 0usize;
    for spec in specs {
        if spec.archive_paths.is_empty() {
            return Err(TextCatalogError::EmptyArchivePaths);
        }
        if spec.name.is_empty() {
            return Err(TextCatalogError::EmptyAssetName);
        }
        if spec.blocks.is_empty() {
            return Err(TextCatalogError::EmptyBlockCatalog);
        }
        if let TextStorageSpec::FixedCellGrid(grid) = spec.storage {
            if spec.blocks.len() != 1 {
                return Err(TextCatalogError::FixedGridSingleBlockRequired {
                    name: spec.name.to_string(),
                    actual: spec.blocks.len(),
                });
            }
            fixed_grid_source_chars(spec.name, spec.layout, spec.blocks[0], grid)?;
        }
        for &archive_path in spec.archive_paths {
            if archive_path.is_empty() {
                return Err(TextCatalogError::EmptyArchivePath);
            }
            if !physical_assets.insert((archive_path, spec.name)) {
                return Err(TextCatalogError::DuplicatePhysicalAsset {
                    archive_path: archive_path.to_string(),
                    name: spec.name.to_string(),
                });
            }
            declared_physical_assets += 1;
        }
        for block in spec.blocks {
            if block.id.is_empty() {
                return Err(TextCatalogError::EmptyBlockId);
            }
            if !block_ids.insert(block.id) {
                return Err(TextCatalogError::DuplicateBlockId(block.id.to_string()));
            }
            blocks += 1;
            japanese_target_blocks += usize::from(is_japanese_translation_target(block.source));
        }
        if let TextStorageSpec::SparseSelectorAtlas(sparse) = spec.storage {
            verify_sparse_selector_atlas(*spec, sparse)?;
        }
    }

    Ok(TextCatalogSpecReport {
        assets: specs.len(),
        declared_physical_assets,
        blocks,
        japanese_target_blocks,
    })
}

/// 해제 BTX 하나가 사람이 승인한 텍스트 계약과 정확히 일치하는지 검증한다.
pub fn verify_text_asset(
    data: &[u8],
    spec: TextAssetSpec<'_>,
) -> Result<TextCatalogReport, TextCatalogError> {
    if spec.archive_paths.is_empty() {
        return Err(TextCatalogError::EmptyArchivePaths);
    }
    let mut archive_paths = BTreeSet::new();
    for &path in spec.archive_paths {
        if path.is_empty() {
            return Err(TextCatalogError::EmptyArchivePath);
        }
        if !archive_paths.insert(path) {
            return Err(TextCatalogError::DuplicateArchivePath(path.to_string()));
        }
    }
    if spec.name.is_empty() {
        return Err(TextCatalogError::EmptyAssetName);
    }
    if spec.blocks.is_empty() {
        return Err(TextCatalogError::EmptyBlockCatalog);
    }

    let actual_digest = digest(data);
    if actual_digest != spec.expected_decompressed_sha256 {
        return Err(TextCatalogError::DecompressedDigestMismatch {
            expected: spec.expected_decompressed_sha256.to_string(),
            actual: actual_digest,
        });
    }
    let pixels = decode(data, spec.layout)?;

    if let TextStorageSpec::FixedCellGrid(grid) = spec.storage {
        verify_fixed_cell_grid(&pixels, spec, grid)?;
    }
    if let TextStorageSpec::SparseSelectorAtlas(sparse) = spec.storage {
        verify_sparse_selector_atlas(spec, sparse)?;
    }

    let mut ids = BTreeSet::new();
    let mut prior_blocks: Vec<TextBlockSpec<'_>> = Vec::new();
    let mut total_pixels = 0usize;
    let mut japanese_target_blocks = 0usize;
    let mut japanese_target_pixels = 0usize;
    for block in spec.blocks {
        if block.id.is_empty() {
            return Err(TextCatalogError::EmptyBlockId);
        }
        if block.source.is_empty() {
            return Err(TextCatalogError::EmptySource {
                id: block.id.to_string(),
            });
        }
        if !ids.insert(block.id) {
            return Err(TextCatalogError::DuplicateBlockId(block.id.to_string()));
        }
        let region = block_pixels(&pixels, spec.layout, *block)?;
        if !matches!(spec.storage, TextStorageSpec::SparseSelectorAtlas(_)) {
            for prior in &prior_blocks {
                if block.bbox.overlaps(prior.bbox) {
                    return Err(TextCatalogError::OverlappingBlocks {
                        first: prior.id.to_string(),
                        second: block.id.to_string(),
                    });
                }
            }
        }
        prior_blocks.push(*block);

        let expected_indices = palette_values(block.id, block.palette_indices, spec.layout.format)?;
        let actual_indices = region.iter().copied().collect::<BTreeSet<_>>();
        let actual_indices = actual_indices.into_iter().collect::<Vec<_>>();
        if actual_indices != expected_indices {
            return Err(TextCatalogError::PaletteSetMismatch {
                id: block.id.to_string(),
                expected: expected_indices,
                actual: actual_indices,
            });
        }
        let actual_region_digest = digest(&region);
        if actual_region_digest != block.expected_pixels_sha256 {
            return Err(TextCatalogError::PixelDigestMismatch {
                id: block.id.to_string(),
                expected: block.expected_pixels_sha256.to_string(),
                actual: actual_region_digest,
            });
        }
        let physical_text_pixels = match spec.storage {
            TextStorageSpec::RasterBlocks => region.len(),
            TextStorageSpec::SparseSelectorAtlas(_) => 0,
            TextStorageSpec::FixedCellGrid(grid) => grid
                .expected_active_cells
                .checked_mul(grid.cell_width)
                .and_then(|value| value.checked_mul(grid.cell_height))
                .ok_or_else(|| TextCatalogError::BboxOverflow {
                    id: block.id.to_string(),
                })?,
        };
        total_pixels += physical_text_pixels;
        if is_japanese_translation_target(block.source) {
            japanese_target_blocks += 1;
            japanese_target_pixels += physical_text_pixels;
        }
    }

    if let TextStorageSpec::SparseSelectorAtlas(sparse) = spec.storage {
        total_pixels = sparse.expected_physical_union_pixels;
        japanese_target_pixels = rect_union_pixels(
            spec.layout,
            spec.blocks
                .iter()
                .filter(|block| is_japanese_translation_target(block.source))
                .map(|block| block.bbox),
        )?;
    }

    Ok(TextCatalogReport {
        declared_physical_assets: spec.archive_paths.len(),
        blocks: spec.blocks.len(),
        block_pixels: total_pixels,
        japanese_target_blocks,
        japanese_target_pixels,
    })
}

/// 지원 원본에서 현재 block 분할까지 승인한 BTX 텍스트 자산.
pub const BOX_TEXT_ASSETS: &[TextAssetSpec<'static>] = &[
    NOWLOAD_TEXT_ASSET,
    MOPT_TEXT_ASSET,
    MCRD01_TEXT_ASSET,
    RANK_S_TEXT_ASSET,
    MENU02_TEXT_ASSET,
    MM_SPR_TEXT_ASSET,
    TITLEB_TEXT_ASSET,
    BOXCUBE_TEXT_ASSET,
    BOXPARTS_TEXT_ASSET,
    TITLEA_TEXT_ASSET,
    MMENU_TEXT_ASSET,
    RPG_SYS1_TEXT_ASSET,
    P2_OP1_TEXT_ASSET,
    BATAN2_TEXT_ASSET,
    P2_OP13_TEXT_ASSET,
    PCOMI01_TEXT_ASSET,
    PCOMI02_TEXT_ASSET,
    P2RULED1_TEXT_ASSET,
    P2_INFO3_TEXT_ASSET,
    P2_INFO1_TEXT_ASSET,
    P2_INFO4_TEXT_ASSET,
    P2_INFO5_TEXT_ASSET,
    MENU01_TEXT_ASSET,
    MENU203_TEXT_ASSET,
    MENU202_TEXT_ASSET,
    MENU201_TEXT_ASSET,
    G_MESS_TEXT_ASSET,
    P1_OVER3_TEXT_ASSET,
    P2_END15_TEXT_ASSET,
    P2_END10_TEXT_ASSET,
    P2_END16_TEXT_ASSET,
    BOXOPT_TEXT_ASSET,
    KANBAN_TEXT_ASSET,
];

#[cfg(test)]
#[path = "text_catalog_tests.rs"]
mod tests;
