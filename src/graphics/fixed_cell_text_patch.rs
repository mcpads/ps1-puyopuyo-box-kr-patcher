//! 고정 16×16 cell page의 Unicode 번역문 조립기.
//!
//! 원본 fixed-cell 계약을 먼저 검증하고, 번역문의 명시적 줄 경계를 행에
//! 대응시킨다. 사용하지 않는 cell은 원본 tail을 복제하지 않고 profile의
//! empty index로 지운다. 모든 cell을 메모리에서 합성·검증한 뒤에만 새 BTX를
//! 반환하므로 실패한 조립에서 부분 산출물은 없다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::font::{
    rasterize_info_page_glyph, InfoPageFontError, InfoPageRasterEffect, InfoPageRasterProfile,
    INFO_PAGE_GLYPH_CELL_HEIGHT, INFO_PAGE_GLYPH_CELL_WIDTH, P2_INFO_PAGE_DRAFT_RASTER_PROFILE,
};

use super::text_catalog::{
    is_japanese_translation_target, verify_text_asset, PaletteIndexSpec, TextAssetSpec,
    TextCatalogError, TextStorageSpec, P2_INFO4_TEXT_ASSET, P2_INFO5_TEXT_ASSET,
};
use super::text_profile::BakedFontBook;
use crate::btx::{decode, encode, BtxError};

pub const P2_INFO_PAGE_CAPACITY_STRESS_CELLS: usize = 256;

/// 현대 한글 전 구간에서 실제 합성 가능한 서로 다른 256자를 고른 비제품 입력.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoPageCapacityStressText {
    pub text: String,
    pub scanned_hangul: usize,
    pub skipped_hangul: usize,
    pub renderable_hangul: usize,
    pub selected_hangul: usize,
    pub first_hangul: char,
    pub last_hangul: char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedCellTextStressExpectation {
    pub asset_name: &'static str,
    pub changed_cells: usize,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub output_sha256: &'static str,
}

/// 지원 원본 두 INFO page의 전체 256-cell stress 결정값.
pub const P2_INFO_PAGE_CAPACITY_STRESS_EXPECTATIONS: [FixedCellTextStressExpectation; 2] = [
    FixedCellTextStressExpectation {
        asset_name: "P2_INFO4.BTX",
        changed_cells: 256,
        changed_pixels: 24_303,
        changed_bytes: 16_346,
        output_sha256: "dc42fd0c7615202a1e03f95e141f070406adc64108813218e0c628b93e1b1915",
    },
    FixedCellTextStressExpectation {
        asset_name: "P2_INFO5.BTX",
        changed_cells: 256,
        changed_pixels: 49_259,
        changed_bytes: 26_277,
        output_sha256: "dc42fd0c7615202a1e03f95e141f070406adc64108813218e0c628b93e1b1915",
    },
];

pub const BOX_P2_INFO_PAGE_PHYSICAL_ASSETS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxP2InfoPagePatchStrategyReport {
    pub assets: usize,
    pub physical_assets: usize,
    pub scanned_hangul: usize,
    pub skipped_hangul: usize,
    pub renderable_hangul: usize,
    pub distinct_hangul: usize,
    pub first_hangul: char,
    pub last_hangul: char,
    pub rows: usize,
    pub used_cells: usize,
    pub info4: FixedCellTextPatchReport,
    pub info5: FixedCellTextPatchReport,
}

impl fmt::Display for BoxP2InfoPagePatchStrategyReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "assets {} / physical {}, scanned/skipped/renderable/used \
             {}/{}/{}/{}, range {}..{}, rows/cells {}/{}, \
             INFO4 changed cells/pixels/bytes {}/{}/{}, INFO5 {}/{}/{}, \
             output SHA-256 {}/{}; translation/runtime unapproved",
            self.assets,
            self.physical_assets,
            self.scanned_hangul,
            self.skipped_hangul,
            self.renderable_hangul,
            self.distinct_hangul,
            self.first_hangul,
            self.last_hangul,
            self.rows,
            self.used_cells,
            self.info4.changed_cells,
            self.info4.changed_pixels,
            self.info4.changed_bytes,
            self.info5.changed_cells,
            self.info5.changed_pixels,
            self.info5.changed_bytes,
            self.info4.output_sha256,
            self.info5.output_sha256,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedCellTextPatchReport {
    pub rows: usize,
    pub used_cells: usize,
    pub empty_cells: usize,
    pub changed_cells: usize,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedCellTextPatchError {
    Catalog(TextCatalogError),
    Btx(BtxError),
    Font(InfoPageFontError),
    FontBook(String),
    SourceCount {
        expected: usize,
        actual: usize,
    },
    UnexpectedSource {
        archive: String,
        name: String,
    },
    DuplicateSource {
        archive: String,
        name: String,
    },
    MissingSource {
        archive: String,
        name: String,
    },
    PhysicalCopyDrift(String),
    StressDecisionMismatch(String),
    InsufficientRenderableHangul {
        expected: usize,
        actual: usize,
    },
    DuplicateStressHangul(usize),
    StressRepertoireMismatch(String),
    NotFixedCellGrid(String),
    BlockCount {
        name: String,
        actual: usize,
    },
    UnsupportedCellSize {
        name: String,
        width: usize,
        height: usize,
    },
    GridDoesNotCoverAsset(String),
    ReferenceOnlyBlock(String),
    EmptyTranslation(String),
    CarriageReturn(String),
    TrailingLineBreak(String),
    MissingHangul(String),
    JapaneseRemaining(String),
    TooManyRows {
        id: String,
        maximum: usize,
        actual: usize,
    },
    RowTooWide {
        id: String,
        row: usize,
        maximum: usize,
        actual: usize,
    },
    EmptyProfileId(String),
    EmptyFontId(String),
    UnknownFont {
        id: String,
        font_id: String,
    },
    PaletteIndexNotDeclared {
        id: String,
        role: &'static str,
        value: u8,
    },
    PaletteRolesOverlap(String),
    Roundtrip,
    NoChangedPixels,
}

impl fmt::Display for FixedCellTextPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(source) => write!(f, "fixed-cell 원본 계약 실패: {source}"),
            Self::Btx(source) => write!(f, "fixed-cell BTX codec 실패: {source}"),
            Self::Font(source) => write!(f, "fixed-cell glyph 합성 실패: {source}"),
            Self::FontBook(source) => write!(f, "fixed-cell font book 구성 실패: {source}"),
            Self::SourceCount { expected, actual } => write!(
                f,
                "P2 INFO fixed-cell source 분모 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnexpectedSource { archive, name } => {
                write!(f, "P2 INFO fixed-cell 미등록 source: {archive}:{name}")
            }
            Self::DuplicateSource { archive, name } => {
                write!(f, "P2 INFO fixed-cell source 중복: {archive}:{name}")
            }
            Self::MissingSource { archive, name } => {
                write!(f, "P2 INFO fixed-cell source 누락: {archive}:{name}")
            }
            Self::PhysicalCopyDrift(name) => {
                write!(f, "{name}: fixed-cell stress 결과가 물리 사본마다 다름")
            }
            Self::StressDecisionMismatch(name) => {
                write!(f, "{name}: fixed-cell stress 결정값 불일치")
            }
            Self::InsufficientRenderableHangul { expected, actual } => write!(
                f,
                "P2 INFO stress용 renderable 한글 부족: expected={expected}, actual={actual}"
            ),
            Self::DuplicateStressHangul(actual) => {
                write!(f, "P2 INFO stress glyph 중복: {actual}")
            }
            Self::StressRepertoireMismatch(actual) => {
                write!(f, "P2 INFO stress renderable 한글 기준선 불일치: {actual}")
            }
            Self::NotFixedCellGrid(name) => {
                write!(f, "{name}: fixed-cell storage가 아닌 자산")
            }
            Self::BlockCount { name, actual } => {
                write!(
                    f,
                    "{name}: fixed-cell translation block 수 불일치: {actual}"
                )
            }
            Self::UnsupportedCellSize {
                name,
                width,
                height,
            } => write!(f, "{name}: 지원하지 않는 fixed-cell 크기: {width}x{height}"),
            Self::GridDoesNotCoverAsset(name) => {
                write!(f, "{name}: fixed-cell grid가 자산 전체를 정확히 덮지 않음")
            }
            Self::ReferenceOnlyBlock(id) => write!(f, "{id}: 일본어 번역 대상 block이 아님"),
            Self::EmptyTranslation(id) => write!(f, "{id}: 번역문이 비어 있음"),
            Self::CarriageReturn(id) => write!(f, "{id}: 번역문에 CR 문자가 있음"),
            Self::TrailingLineBreak(id) => write!(f, "{id}: 번역문 끝에 불필요한 줄바꿈이 있음"),
            Self::MissingHangul(id) => write!(f, "{id}: 번역문에 한글 음절이 없음"),
            Self::JapaneseRemaining(id) => write!(f, "{id}: 번역문에 일본어 문자가 남음"),
            Self::TooManyRows {
                id,
                maximum,
                actual,
            } => write!(
                f,
                "{id}: 번역문 행 초과: maximum={maximum}, actual={actual}"
            ),
            Self::RowTooWide {
                id,
                row,
                maximum,
                actual,
            } => write!(
                f,
                "{id}: 번역문 {row}행 cell 초과: maximum={maximum}, actual={actual}"
            ),
            Self::EmptyProfileId(id) => write!(f, "{id}: fixed-cell profile ID가 비어 있음"),
            Self::EmptyFontId(id) => write!(f, "{id}: fixed-cell font ID가 비어 있음"),
            Self::UnknownFont { id, font_id } => {
                write!(f, "{id}: font book에 없는 fixed-cell font ID: {font_id}")
            }
            Self::PaletteIndexNotDeclared { id, role, value } => {
                write!(f, "{id}: {role} palette index가 원본 계약에 없음: {value}")
            }
            Self::PaletteRolesOverlap(id) => {
                write!(f, "{id}: empty/background/foreground palette 역할이 겹침")
            }
            Self::Roundtrip => f.write_str("fixed-cell BTX encode/decode 왕복 불일치"),
            Self::NoChangedPixels => f.write_str("fixed-cell patch에서 변경된 픽셀이 없음"),
        }
    }
}

impl Error for FixedCellTextPatchError {}

impl From<TextCatalogError> for FixedCellTextPatchError {
    fn from(source: TextCatalogError) -> Self {
        Self::Catalog(source)
    }
}

impl From<BtxError> for FixedCellTextPatchError {
    fn from(source: BtxError) -> Self {
        Self::Btx(source)
    }
}

impl From<InfoPageFontError> for FixedCellTextPatchError {
    fn from(source: InfoPageFontError) -> Self {
        Self::Font(source)
    }
}

fn palette_contains(spec: PaletteIndexSpec<'_>, value: u8) -> bool {
    match spec {
        PaletteIndexSpec::Values(values) => values.contains(&value),
        PaletteIndexSpec::Bitset256(words) => {
            let word = usize::from(value / 64);
            let bit = u32::from(value % 64);
            words[word] & (1u64 << bit) != 0
        }
    }
}

fn validate_palette(
    id: &str,
    block_palette: PaletteIndexSpec<'_>,
    active_palette: PaletteIndexSpec<'_>,
    empty_palette: PaletteIndexSpec<'_>,
    profile: InfoPageRasterProfile,
) -> Result<(), FixedCellTextPatchError> {
    let InfoPageRasterEffect::Binary {
        background,
        foreground,
        ..
    } = profile.effect;
    let roles = [
        ("empty", profile.empty, empty_palette),
        ("background", background, active_palette),
        ("foreground", foreground, active_palette),
    ];
    if BTreeSet::from([profile.empty, background, foreground]).len() != 3 {
        return Err(FixedCellTextPatchError::PaletteRolesOverlap(id.to_string()));
    }
    for (role, value, role_palette) in roles {
        if !palette_contains(block_palette, value) || !palette_contains(role_palette, value) {
            return Err(FixedCellTextPatchError::PaletteIndexNotDeclared {
                id: id.to_string(),
                role,
                value,
            });
        }
    }
    Ok(())
}

/// 전 현대 한글에서 실제 profile에 들어가는 서로 다른 256자를 균등 표본한다.
pub fn build_p2_info_page_capacity_stress_text(
    fonts: &BakedFontBook,
) -> Result<InfoPageCapacityStressText, FixedCellTextPatchError> {
    let profile = P2_INFO_PAGE_DRAFT_RASTER_PROFILE;
    let font = fonts
        .get(profile.font_id)
        .ok_or_else(|| FixedCellTextPatchError::UnknownFont {
            id: "P2_INFO4/5-capacity-stress".to_string(),
            font_id: profile.font_id.to_string(),
        })?;
    let mut renderable = Vec::new();
    let mut scanned_hangul = 0usize;
    let mut skipped_hangul = 0usize;
    for scalar in 0xAC00..=0xD7A3 {
        let glyph = char::from_u32(scalar).expect("현대 한글 음절 범위");
        scanned_hangul += 1;
        match rasterize_info_page_glyph(font, glyph, profile) {
            Ok(_) => renderable.push(glyph),
            Err(InfoPageFontError::EmptyGlyph(_) | InfoPageFontError::GlyphDoesNotFit { .. }) => {
                skipped_hangul += 1;
            }
            Err(source) => return Err(FixedCellTextPatchError::Font(source)),
        }
    }
    if renderable.len() < P2_INFO_PAGE_CAPACITY_STRESS_CELLS {
        return Err(FixedCellTextPatchError::InsufficientRenderableHangul {
            expected: P2_INFO_PAGE_CAPACITY_STRESS_CELLS,
            actual: renderable.len(),
        });
    }
    let selected = (0..P2_INFO_PAGE_CAPACITY_STRESS_CELLS)
        .map(|cell| {
            let index = cell * (renderable.len() - 1) / (P2_INFO_PAGE_CAPACITY_STRESS_CELLS - 1);
            renderable[index]
        })
        .collect::<Vec<_>>();
    let distinct = selected.iter().copied().collect::<BTreeSet<_>>();
    if distinct.len() != P2_INFO_PAGE_CAPACITY_STRESS_CELLS {
        return Err(FixedCellTextPatchError::DuplicateStressHangul(
            distinct.len(),
        ));
    }
    let text = selected
        .chunks(16)
        .map(|row| row.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    let boundary = (
        scanned_hangul,
        skipped_hangul,
        renderable.len(),
        selected.len(),
        selected[0],
        selected[selected.len() - 1],
    );
    const EXPECTED_BOUNDARY: (usize, usize, usize, usize, char, char) =
        (11_172, 8_698, 2_474, 256, '가', '힝');
    if boundary != EXPECTED_BOUNDARY {
        return Err(FixedCellTextPatchError::StressRepertoireMismatch(format!(
            "{boundary:?}"
        )));
    }
    Ok(InfoPageCapacityStressText {
        text,
        scanned_hangul,
        skipped_hangul,
        renderable_hangul: renderable.len(),
        selected_hangul: selected.len(),
        first_hangul: selected[0],
        last_hangul: selected[selected.len() - 1],
    })
}

/// 원본 fixed-cell BTX를 검증하고 명시적 행/열의 Unicode 번역문으로 재조립한다.
pub fn patch_fixed_cell_text_page(
    original: &[u8],
    spec: TextAssetSpec<'_>,
    fonts: &BakedFontBook,
    text: &str,
    profile: InfoPageRasterProfile,
) -> Result<(Vec<u8>, FixedCellTextPatchReport), FixedCellTextPatchError> {
    verify_text_asset(original, spec)?;
    let TextStorageSpec::FixedCellGrid(grid) = spec.storage else {
        return Err(FixedCellTextPatchError::NotFixedCellGrid(
            spec.name.to_string(),
        ));
    };
    let [block] = spec.blocks else {
        return Err(FixedCellTextPatchError::BlockCount {
            name: spec.name.to_string(),
            actual: spec.blocks.len(),
        });
    };
    if grid.cell_width != INFO_PAGE_GLYPH_CELL_WIDTH
        || grid.cell_height != INFO_PAGE_GLYPH_CELL_HEIGHT
    {
        return Err(FixedCellTextPatchError::UnsupportedCellSize {
            name: spec.name.to_string(),
            width: grid.cell_width,
            height: grid.cell_height,
        });
    }
    if !spec.layout.width.is_multiple_of(grid.cell_width)
        || !spec.layout.height.is_multiple_of(grid.cell_height)
        || block.bbox.x != 0
        || block.bbox.y != 0
        || block.bbox.width != spec.layout.width
        || block.bbox.height != spec.layout.height
    {
        return Err(FixedCellTextPatchError::GridDoesNotCoverAsset(
            spec.name.to_string(),
        ));
    }
    if !is_japanese_translation_target(block.source) {
        return Err(FixedCellTextPatchError::ReferenceOnlyBlock(
            block.id.to_string(),
        ));
    }
    if text.is_empty() {
        return Err(FixedCellTextPatchError::EmptyTranslation(
            block.id.to_string(),
        ));
    }
    if text.contains('\r') {
        return Err(FixedCellTextPatchError::CarriageReturn(
            block.id.to_string(),
        ));
    }
    if text.ends_with('\n') {
        return Err(FixedCellTextPatchError::TrailingLineBreak(
            block.id.to_string(),
        ));
    }
    if !text
        .chars()
        .any(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
    {
        return Err(FixedCellTextPatchError::MissingHangul(block.id.to_string()));
    }
    if is_japanese_translation_target(text) {
        return Err(FixedCellTextPatchError::JapaneseRemaining(
            block.id.to_string(),
        ));
    }
    if profile.id.is_empty() {
        return Err(FixedCellTextPatchError::EmptyProfileId(
            block.id.to_string(),
        ));
    }
    if profile.font_id.is_empty() {
        return Err(FixedCellTextPatchError::EmptyFontId(block.id.to_string()));
    }
    validate_palette(
        block.id,
        block.palette_indices,
        grid.active_palette_indices,
        grid.empty_palette_indices,
        profile,
    )?;
    let font = fonts
        .get(profile.font_id)
        .ok_or_else(|| FixedCellTextPatchError::UnknownFont {
            id: block.id.to_string(),
            font_id: profile.font_id.to_string(),
        })?;

    let columns = spec.layout.width / grid.cell_width;
    let maximum_rows = spec.layout.height / grid.cell_height;
    let lines = text.split('\n').collect::<Vec<_>>();
    if lines.len() > maximum_rows {
        return Err(FixedCellTextPatchError::TooManyRows {
            id: block.id.to_string(),
            maximum: maximum_rows,
            actual: lines.len(),
        });
    }

    let total_cells = columns * maximum_rows;
    let mut patched_pixels = vec![profile.empty; spec.layout.width * spec.layout.height];
    let mut used_cells = 0usize;
    for (row, line) in lines.iter().copied().enumerate() {
        let glyphs = line.chars().collect::<Vec<_>>();
        if glyphs.len() > columns {
            return Err(FixedCellTextPatchError::RowTooWide {
                id: block.id.to_string(),
                row: row + 1,
                maximum: columns,
                actual: glyphs.len(),
            });
        }
        for (column, glyph) in glyphs.into_iter().enumerate() {
            let cell = rasterize_info_page_glyph(font, glyph, profile)?;
            for cell_row in 0..grid.cell_height {
                let source = cell_row * grid.cell_width;
                let target = (row * grid.cell_height + cell_row) * spec.layout.width
                    + column * grid.cell_width;
                patched_pixels[target..target + grid.cell_width]
                    .copy_from_slice(&cell[source..source + grid.cell_width]);
            }
            used_cells += 1;
        }
    }

    let original_pixels = decode(original, spec.layout)?;
    let changed_pixels = original_pixels
        .iter()
        .zip(&patched_pixels)
        .filter(|(before, after)| before != after)
        .count();
    if changed_pixels == 0 {
        return Err(FixedCellTextPatchError::NoChangedPixels);
    }
    let changed_cells = (0..total_cells)
        .filter(|index| {
            let cell_x = (index % columns) * grid.cell_width;
            let cell_y = (index / columns) * grid.cell_height;
            (0..grid.cell_height).any(|row| {
                let start = (cell_y + row) * spec.layout.width + cell_x;
                original_pixels[start..start + grid.cell_width]
                    != patched_pixels[start..start + grid.cell_width]
            })
        })
        .count();
    let patched = encode(&patched_pixels, spec.layout)?;
    if decode(&patched, spec.layout)? != patched_pixels {
        return Err(FixedCellTextPatchError::Roundtrip);
    }
    let changed_bytes = original
        .iter()
        .zip(&patched)
        .filter(|(before, after)| before != after)
        .count();
    let output_sha256 = format!("{:x}", Sha256::digest(&patched));
    Ok((
        patched,
        FixedCellTextPatchReport {
            rows: lines.len(),
            used_cells,
            empty_cells: total_cells - used_cells,
            changed_cells,
            changed_pixels,
            changed_bytes,
            output_sha256,
        },
    ))
}

/// 지원 원본의 INFO4/5 네 물리 사본과 전체 256-cell patch 전략을 한 번에 검증한다.
pub fn verify_box_p2_info_page_patch_strategy<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
) -> Result<BoxP2InfoPagePatchStrategyReport, FixedCellTextPatchError> {
    let sources = sources.into_iter().collect::<Vec<_>>();
    if sources.len() != BOX_P2_INFO_PAGE_PHYSICAL_ASSETS {
        return Err(FixedCellTextPatchError::SourceCount {
            expected: BOX_P2_INFO_PAGE_PHYSICAL_ASSETS,
            actual: sources.len(),
        });
    }
    let expected_sources = [P2_INFO4_TEXT_ASSET, P2_INFO5_TEXT_ASSET]
        .into_iter()
        .flat_map(|spec| {
            spec.archive_paths
                .iter()
                .map(move |archive| (*archive, spec.name))
        })
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for (archive, name, _) in &sources {
        if !expected_sources.contains(&(*archive, *name)) {
            return Err(FixedCellTextPatchError::UnexpectedSource {
                archive: (*archive).to_string(),
                name: (*name).to_string(),
            });
        }
        if !seen.insert((*archive, *name)) {
            return Err(FixedCellTextPatchError::DuplicateSource {
                archive: (*archive).to_string(),
                name: (*name).to_string(),
            });
        }
    }
    if seen != expected_sources {
        let (archive, name) = expected_sources
            .difference(&seen)
            .next()
            .copied()
            .expect("분모가 같고 집합이 다르면 누락 source가 있음");
        return Err(FixedCellTextPatchError::MissingSource {
            archive: archive.to_string(),
            name: name.to_string(),
        });
    }

    let fonts = BakedFontBook::with_bundled_maplestory_light()
        .map_err(|error| FixedCellTextPatchError::FontBook(error.to_string()))?;
    let stress = build_p2_info_page_capacity_stress_text(&fonts)?;
    let mut reports = Vec::new();
    for (spec, expectation) in [P2_INFO4_TEXT_ASSET, P2_INFO5_TEXT_ASSET]
        .into_iter()
        .zip(P2_INFO_PAGE_CAPACITY_STRESS_EXPECTATIONS)
    {
        let mut canonical = None;
        for (_, _, plain) in sources.iter().filter(|(_, name, _)| *name == spec.name) {
            let (_, report) = patch_fixed_cell_text_page(
                plain,
                spec,
                &fonts,
                &stress.text,
                P2_INFO_PAGE_DRAFT_RASTER_PROFILE,
            )?;
            if canonical
                .as_ref()
                .is_some_and(|previous| previous != &report)
            {
                return Err(FixedCellTextPatchError::PhysicalCopyDrift(
                    spec.name.to_string(),
                ));
            }
            canonical = Some(report);
        }
        let report = canonical.ok_or_else(|| FixedCellTextPatchError::MissingSource {
            archive: spec.archive_paths[0].to_string(),
            name: spec.name.to_string(),
        })?;
        if spec.name != expectation.asset_name
            || report.rows != 16
            || report.used_cells != 256
            || report.empty_cells != 0
            || report.changed_cells != expectation.changed_cells
            || report.changed_pixels != expectation.changed_pixels
            || report.changed_bytes != expectation.changed_bytes
            || report.output_sha256 != expectation.output_sha256
        {
            return Err(FixedCellTextPatchError::StressDecisionMismatch(format!(
                "{} actual={report:?}",
                spec.name
            )));
        }
        reports.push(report);
    }
    let [info4, info5] = reports.as_slice() else {
        unreachable!("두 INFO spec을 정확히 순회함")
    };
    Ok(BoxP2InfoPagePatchStrategyReport {
        assets: reports.len(),
        physical_assets: sources.len(),
        scanned_hangul: stress.scanned_hangul,
        skipped_hangul: stress.skipped_hangul,
        renderable_hangul: stress.renderable_hangul,
        distinct_hangul: stress.selected_hangul,
        first_hangul: stress.first_hangul,
        last_hangul: stress.last_hangul,
        rows: info4.rows,
        used_cells: info4.used_cells,
        info4: info4.clone(),
        info5: info5.clone(),
    })
}

#[cfg(test)]
#[path = "fixed_cell_text_patch_tests.rs"]
mod tests;
