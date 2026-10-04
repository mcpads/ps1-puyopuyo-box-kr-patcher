//! `MM_MES` 512-grid-cell synthetic glyph·CNX·VDD capacity stress.
//!
//! Maplestory Light에서 실제 합성 가능한 서로 다른 한글 512자를 전 partition에
//! 채운다. 실제 자산은 baked-text surface다. 이 입력은 storage 상한을 보는
//! 비제품 PoC이며 승인 번역, 실제 block 소비, runtime 표시는 증명하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::archive::{VddArchive, VddError};
use crate::compress::{compress, decompress, parse_header};
use crate::font::{
    bundled_maplestory_light, rasterize_mm_mes_storage_stress_glyph, MmMesStorageStressFontError,
    MM_MES_STORAGE_STRESS_RASTER_PROFILE, MM_MES_STRESS_CELL_HEIGHT, MM_MES_STRESS_CELL_WIDTH,
};
use crate::graphics::mm_mes_family::{
    MmMesVariant, BOX_MM_MES_VARIANTS, MM_MES_ASSET_NAME, MM_MES_STORAGE_CELLS,
};
use crate::graphics::mm_mes_grid_stress::{stress_patch_mm_mes_grid, MmMesGridStressMapping};

pub const MM_MES_STORAGE_STRESS_SCHEMA_VERSION: &str = "mm-mes-storage-stress-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmMesVddRewriteMethod {
    InPlace,
    RepackInContainer,
}

impl MmMesVddRewriteMethod {
    pub const fn label(self) -> &'static str {
        match self {
            Self::InPlace => "in-place",
            Self::RepackInContainer => "repack-in-container",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmMesStorageStressPatchRow {
    pub variant: MmMesVariant,
    pub mapped_cells: usize,
    pub changed_cells: usize,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmMesStorageStressVddRow {
    pub variant: MmMesVariant,
    pub archive: &'static str,
    pub source_cnx_bytes: usize,
    pub stress_cnx_bytes: usize,
    pub in_place_capacity: usize,
    pub method: MmMesVddRewriteMethod,
    pub changed_vdd_bytes: usize,
    pub output_vdd_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmMesStorageStressAnalysis {
    pub schema_version: &'static str,
    pub scanned_hangul: usize,
    pub skipped_hangul: usize,
    pub renderable_hangul: usize,
    pub distinct_hangul: usize,
    pub first_hangul: char,
    pub last_hangul: char,
    pub patches: Vec<MmMesStorageStressPatchRow>,
    pub vdds: Vec<MmMesStorageStressVddRow>,
    pub in_place: usize,
    pub repacked: usize,
    pub product_eligible: bool,
    pub runtime_verified: bool,
}

impl fmt::Display for MmMesStorageStressAnalysis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "scanned/skipped/renderable/used {}/{}/{}/{}, range {}..{}, \
             patch cells {:?}, pixels {:?}, bytes {:?}, output SHA-256 {:?}; \
             CNX before/after {:?}, in-place capacity {:?}, methods {:?}, \
             VDD changed bytes {:?}, VDD SHA-256 {:?}, product eligible {}, \
             runtime verified {}",
            self.scanned_hangul,
            self.skipped_hangul,
            self.renderable_hangul,
            self.distinct_hangul,
            self.first_hangul,
            self.last_hangul,
            self.patches
                .iter()
                .map(|row| (row.variant.scope(), row.mapped_cells, row.changed_cells))
                .collect::<Vec<_>>(),
            self.patches
                .iter()
                .map(|row| (row.variant.scope(), row.changed_pixels))
                .collect::<Vec<_>>(),
            self.patches
                .iter()
                .map(|row| (row.variant.scope(), row.changed_bytes))
                .collect::<Vec<_>>(),
            self.patches
                .iter()
                .map(|row| (row.variant.scope(), row.output_sha256.as_str()))
                .collect::<Vec<_>>(),
            self.vdds
                .iter()
                .map(|row| (
                    row.variant.scope(),
                    row.source_cnx_bytes,
                    row.stress_cnx_bytes
                ))
                .collect::<Vec<_>>(),
            self.vdds
                .iter()
                .map(|row| (row.variant.scope(), row.in_place_capacity))
                .collect::<Vec<_>>(),
            self.vdds
                .iter()
                .map(|row| (row.variant.scope(), row.method.label()))
                .collect::<Vec<_>>(),
            self.vdds
                .iter()
                .map(|row| (row.variant.scope(), row.changed_vdd_bytes))
                .collect::<Vec<_>>(),
            self.vdds
                .iter()
                .map(|row| (row.variant.scope(), row.output_vdd_sha256.as_str()))
                .collect::<Vec<_>>(),
            self.product_eligible,
            self.runtime_verified,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmMesStorageStressError(String);

impl fmt::Display for MmMesStorageStressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for MmMesStorageStressError {}

fn error(message: impl Into<String>) -> MmMesStorageStressError {
    MmMesStorageStressError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn ink_dimensions(pixels: &[u8]) -> Option<(usize, usize)> {
    let mut minimum_x = MM_MES_STRESS_CELL_WIDTH;
    let mut minimum_y = MM_MES_STRESS_CELL_HEIGHT;
    let mut maximum_x = 0usize;
    let mut maximum_y = 0usize;
    let mut found = false;
    for (offset, &pixel) in pixels.iter().enumerate() {
        if pixel == 0 {
            continue;
        }
        found = true;
        let x = offset % MM_MES_STRESS_CELL_WIDTH;
        let y = offset / MM_MES_STRESS_CELL_WIDTH;
        minimum_x = minimum_x.min(x);
        minimum_y = minimum_y.min(y);
        maximum_x = maximum_x.max(x);
        maximum_y = maximum_y.max(y);
    }
    found.then_some((maximum_x - minimum_x + 1, maximum_y - minimum_y + 1))
}

fn stress_mappings(
) -> Result<(Vec<MmMesGridStressMapping>, usize, usize, usize), MmMesStorageStressError> {
    let font = bundled_maplestory_light().map_err(|source| error(source.to_string()))?;
    let mut renderable = Vec::new();
    let mut scanned = 0usize;
    let mut skipped = 0usize;
    for scalar in 0xAC00..=0xD7A3 {
        let glyph = char::from_u32(scalar).expect("현대 한글 음절 범위");
        scanned += 1;
        match rasterize_mm_mes_storage_stress_glyph(
            &font,
            glyph,
            MM_MES_STORAGE_STRESS_RASTER_PROFILE,
        ) {
            Ok(pixels) => {
                let (width, height) = ink_dimensions(&pixels)
                    .ok_or_else(|| error(format!("MM_MES 빈 stress glyph: {glyph}")))?;
                if width > height {
                    return Err(error(format!(
                        "MM_MES 세로 압축 glyph가 normalization 뒤 남음: \
                         {glyph} {width}x{height}"
                    )));
                }
                renderable.push(glyph);
            }
            Err(
                MmMesStorageStressFontError::EmptyGlyph(_)
                | MmMesStorageStressFontError::GlyphDoesNotFit { .. },
            ) => {
                skipped += 1;
            }
            Err(source) => return Err(error(source.to_string())),
        }
    }
    if renderable.len() < MM_MES_STORAGE_CELLS {
        return Err(error(format!(
            "MM_MES stress용 renderable 한글 부족: expected={MM_MES_STORAGE_CELLS}, actual={}",
            renderable.len()
        )));
    }
    let mappings = (0..MM_MES_STORAGE_CELLS)
        .map(|cell| {
            let repertoire_index = cell * (renderable.len() - 1) / (MM_MES_STORAGE_CELLS - 1);
            MmMesGridStressMapping {
                cell: u16::try_from(cell).expect("512 cell은 u16 범위"),
                glyph: Some(renderable[repertoire_index]),
            }
        })
        .collect::<Vec<_>>();
    let repertoire_boundary = (
        scanned,
        skipped,
        renderable.len(),
        mappings[0].glyph.expect("stress mapping은 모두 glyph"),
        mappings[MM_MES_STORAGE_CELLS - 1]
            .glyph
            .expect("stress mapping은 모두 glyph"),
    );
    const EXPECTED_REPERTOIRE_BOUNDARY: (usize, usize, usize, char, char) =
        (11_172, 8_698, 2_474, '가', '힝');
    if repertoire_boundary != EXPECTED_REPERTOIRE_BOUNDARY {
        return Err(error(format!(
            "MM_MES stress renderable 한글 기준선 불일치: {repertoire_boundary:?}"
        )));
    }
    Ok((mappings, scanned, skipped, renderable.len()))
}

fn slot_capacity(archive: &VddArchive, target: &str) -> Result<usize, MmMesStorageStressError> {
    let entry = archive
        .find(target)
        .ok_or_else(|| error(format!("VDD target 누락: {target}")))?;
    let start = entry.byte_offset();
    let next_start = archive
        .entries()
        .iter()
        .filter(|other| !other.name().eq_ignore_ascii_case(target) && other.byte_offset() >= start)
        .map(|other| other.byte_offset())
        .min()
        .unwrap_or_else(|| archive.serialize_lossless().len());
    Ok(next_start.saturating_sub(start))
}

fn verify_rebuilt_vdd(
    variant: MmMesVariant,
    original: &VddArchive,
    rebuilt: &[u8],
    replacement_cnx: &[u8],
    replacement_plain: &[u8],
) -> Result<(), MmMesStorageStressError> {
    if rebuilt.len() != original.serialize_lossless().len() {
        return Err(error(format!(
            "{} MM_MES stress VDD 전체 길이 변경",
            variant.scope()
        )));
    }
    let reparsed = VddArchive::parse(rebuilt)
        .map_err(|source| error(format!("{} VDD readback 실패: {source}", variant.scope())))?;
    if reparsed.entry_count() != original.entry_count() {
        return Err(error(format!(
            "{} MM_MES stress VDD 엔트리 수 변경",
            variant.scope()
        )));
    }
    for source_entry in original.entries() {
        let readback_entry = reparsed.find(source_entry.name()).ok_or_else(|| {
            error(format!(
                "{}:{} VDD readback 엔트리 누락",
                variant.scope(),
                source_entry.name()
            ))
        })?;
        if source_entry.name().eq_ignore_ascii_case(MM_MES_ASSET_NAME) {
            let readback_cnx = reparsed.read_file(readback_entry);
            if readback_cnx != replacement_cnx {
                return Err(error(format!(
                    "{} MM_MES stress CNX readback 불일치",
                    variant.scope()
                )));
            }
            let readback_plain = decompress(readback_cnx).map_err(|source| {
                error(format!(
                    "{} MM_MES stress plain readback 실패: {source}",
                    variant.scope()
                ))
            })?;
            if readback_plain != replacement_plain {
                return Err(error(format!(
                    "{} MM_MES stress plain readback 불일치",
                    variant.scope()
                )));
            }
        } else {
            let original_entry = original
                .find(source_entry.name())
                .ok_or_else(|| error("MM_MES source VDD 엔트리 재탐색 실패"))?;
            if reparsed.read_file(readback_entry) != original.read_file(original_entry) {
                return Err(error(format!(
                    "{}:{} MM_MES stress 비대상 엔트리 변경",
                    variant.scope(),
                    source_entry.name()
                )));
            }
        }
    }
    Ok(())
}

fn prepare_variant(
    variant: MmMesVariant,
    archive: &VddArchive,
    mappings: &[MmMesGridStressMapping],
) -> Result<(MmMesStorageStressPatchRow, MmMesStorageStressVddRow), MmMesStorageStressError> {
    let entry = archive.find(MM_MES_ASSET_NAME).ok_or_else(|| {
        error(format!(
            "{}:{MM_MES_ASSET_NAME} 누락",
            variant.archive_path()
        ))
    })?;
    if entry.size != variant.stored_bytes() {
        return Err(error(format!(
            "{}:{MM_MES_ASSET_NAME} stored 크기 불일치: {}",
            variant.archive_path(),
            entry.size
        )));
    }
    let source_cnx = archive.read_file(entry);
    let header = parse_header(source_cnx).map_err(|source| {
        error(format!(
            "{}:{MM_MES_ASSET_NAME}: {source}",
            variant.archive_path()
        ))
    })?;
    if header.subtype != *b"bi_" {
        return Err(error(format!(
            "{}:{MM_MES_ASSET_NAME} subtype 불일치: {:?}",
            variant.archive_path(),
            header.subtype
        )));
    }
    let source_plain = decompress(source_cnx).map_err(|source| {
        error(format!(
            "{}:{MM_MES_ASSET_NAME}: {source}",
            variant.archive_path()
        ))
    })?;
    let patch = stress_patch_mm_mes_grid(
        variant,
        &source_plain,
        mappings,
        MM_MES_STORAGE_STRESS_RASTER_PROFILE,
    )
    .map_err(|source| error(source.to_string()))?;
    let replacement_cnx =
        compress(&patch.plain, header.subtype).map_err(|source| error(source.to_string()))?;
    if decompress(&replacement_cnx).map_err(|source| error(source.to_string()))? != patch.plain {
        return Err(error(format!(
            "{} MM_MES stress CNX roundtrip 불일치",
            variant.scope()
        )));
    }

    let replacements = BTreeMap::from([(MM_MES_ASSET_NAME.to_string(), replacement_cnx.clone())]);
    let capacity = slot_capacity(archive, MM_MES_ASSET_NAME)?;
    let (rebuilt, method) = match archive.replace_in_place(&replacements) {
        Ok(rebuilt) => (rebuilt, MmMesVddRewriteMethod::InPlace),
        Err(VddError::ReplacementDoesNotFit { .. }) => (
            archive
                .repack_in_container(&replacements)
                .map_err(|source| {
                    error(format!(
                        "{} MM_MES stress VDD 고정 컨테이너 수용 실패: {source}",
                        variant.scope()
                    ))
                })?,
            MmMesVddRewriteMethod::RepackInContainer,
        ),
        Err(source) => {
            return Err(error(format!(
                "{} MM_MES stress VDD 제자리 교체 실패: {source}",
                variant.scope()
            )))
        }
    };
    verify_rebuilt_vdd(variant, archive, &rebuilt, &replacement_cnx, &patch.plain)?;
    let source_vdd = archive.serialize_lossless();
    let patch_row = MmMesStorageStressPatchRow {
        variant,
        mapped_cells: patch.mapped_cells,
        changed_cells: patch.changed_cells,
        changed_pixels: patch.changed_pixels,
        changed_bytes: patch.changed_bytes,
        output_sha256: patch.output_sha256,
    };
    let vdd_row = MmMesStorageStressVddRow {
        variant,
        archive: variant.archive_path(),
        source_cnx_bytes: source_cnx.len(),
        stress_cnx_bytes: replacement_cnx.len(),
        in_place_capacity: capacity,
        method,
        changed_vdd_bytes: source_vdd
            .iter()
            .zip(&rebuilt)
            .filter(|(before, after)| before != after)
            .count(),
        output_vdd_sha256: digest(&rebuilt),
    };
    Ok((patch_row, vdd_row))
}

const EXPECTED_PATCH_ROWS: [(MmMesVariant, usize, usize, usize, usize, &str); 2] = [
    (
        MmMesVariant::Puyo1,
        512,
        512,
        89_108,
        50_481,
        "2bba83cedec0cbe050d25a280a9fb4bec8c2897a015fefe7877a4cd89cd81405",
    ),
    (
        MmMesVariant::Puyo2,
        512,
        512,
        88_840,
        50_323,
        "2bba83cedec0cbe050d25a280a9fb4bec8c2897a015fefe7877a4cd89cd81405",
    ),
];

const EXPECTED_VDD_ROWS: [(
    MmMesVariant,
    usize,
    usize,
    usize,
    MmMesVddRewriteMethod,
    usize,
    &str,
); 2] = [
    (
        MmMesVariant::Puyo1,
        43_649,
        41_458,
        45_056,
        MmMesVddRewriteMethod::InPlace,
        40_797,
        "e2662cf3653c46bef3f1e24e0fee0e1dae79bc91fc92614712241efd8fcba0c3",
    ),
    (
        MmMesVariant::Puyo2,
        43_285,
        41_458,
        45_056,
        MmMesVddRewriteMethod::InPlace,
        40_804,
        "6b68f3a4d7408602fb02160c19657fcf9a18c4d6d339e692b9e313737d377d9a",
    ),
];

/// 두 exact GRP에서 512개 서로 다른 renderable 한글의 합성·재삽입을 검산한다.
pub fn verify_box_mm_mes_storage_stress<'a>(
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<MmMesStorageStressAnalysis, MmMesStorageStressError> {
    let mut inputs = BTreeMap::new();
    for (archive_path, archive) in archives {
        if !BOX_MM_MES_VARIANTS
            .iter()
            .any(|variant| variant.archive_path() == archive_path)
        {
            continue;
        }
        if inputs.insert(archive_path, archive).is_some() {
            return Err(error(format!("{archive_path} MM_MES stress archive 중복")));
        }
    }
    let (mappings, scanned_hangul, skipped_hangul, renderable_hangul) = stress_mappings()?;
    let distinct_glyphs = mappings
        .iter()
        .filter_map(|mapping| mapping.glyph)
        .collect::<BTreeSet<_>>();
    if distinct_glyphs.len() != MM_MES_STORAGE_CELLS {
        return Err(error(format!(
            "MM_MES stress glyph 중복: {}",
            distinct_glyphs.len()
        )));
    }

    let mut patches = Vec::new();
    let mut vdds = Vec::new();
    for variant in BOX_MM_MES_VARIANTS {
        let archive = inputs.get(variant.archive_path()).copied().ok_or_else(|| {
            error(format!(
                "{} MM_MES stress archive 누락",
                variant.archive_path()
            ))
        })?;
        let (patch, vdd) = prepare_variant(variant, archive, &mappings)?;
        patches.push(patch);
        vdds.push(vdd);
    }

    let actual_patches = patches
        .iter()
        .map(|row| {
            (
                row.variant,
                row.mapped_cells,
                row.changed_cells,
                row.changed_pixels,
                row.changed_bytes,
                row.output_sha256.as_str(),
            )
        })
        .collect::<Vec<_>>();
    if actual_patches != EXPECTED_PATCH_ROWS {
        return Err(error(format!(
            "MM_MES storage stress patch 기준선 불일치: {actual_patches:?}"
        )));
    }
    let actual_vdds = vdds
        .iter()
        .map(|row| {
            (
                row.variant,
                row.source_cnx_bytes,
                row.stress_cnx_bytes,
                row.in_place_capacity,
                row.method,
                row.changed_vdd_bytes,
                row.output_vdd_sha256.as_str(),
            )
        })
        .collect::<Vec<_>>();
    if actual_vdds != EXPECTED_VDD_ROWS {
        return Err(error(format!(
            "MM_MES storage stress VDD 기준선 불일치: {actual_vdds:?}"
        )));
    }
    let in_place = vdds
        .iter()
        .filter(|row| row.method == MmMesVddRewriteMethod::InPlace)
        .count();
    let repacked = vdds.len() - in_place;
    Ok(MmMesStorageStressAnalysis {
        schema_version: MM_MES_STORAGE_STRESS_SCHEMA_VERSION,
        scanned_hangul,
        skipped_hangul,
        renderable_hangul,
        distinct_hangul: distinct_glyphs.len(),
        first_hangul: mappings[0].glyph.expect("stress mapping은 모두 glyph"),
        last_hangul: mappings[MM_MES_STORAGE_CELLS - 1]
            .glyph
            .expect("stress mapping은 모두 glyph"),
        patches,
        vdds,
        in_place,
        repacked,
        product_eligible: false,
        runtime_verified: false,
    })
}

#[cfg(test)]
#[path = "mm_mes_storage_stress_tests.rs"]
mod tests;
