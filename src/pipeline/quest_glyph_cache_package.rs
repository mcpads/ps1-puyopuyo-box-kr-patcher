//! QUEST sparse glyph cache의 결정적 carrier package 직렬화.
//!
//! 저장 계획에서 만든 glyph catalog와 page assignment를 한 번만 직렬화하고,
//! 실제 hook이 읽는 offset layout으로 다시 해제해 원재료와
//! 비교한다. 마지막 scratch까지 모든 바이트의 소유자를 선언하지만 이
//! package 자체는 번역 승인이나 runtime 소비를 뜻하지 않는다.
//! 원본 atlas source catalog는 과거 runtime 대조용 side data로만 보존하며
//! carrier package에는 직렬화하지 않는다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::disc::VerifiedSourceImage;
use crate::script::QuestTranslationBatchAnalysis;

use super::quest_glyph_cache_plan::{
    build_quest_glyph_cache_materials, cell_row_index_record, decode_packbits, decode_page_indices,
    unpack_row_indices, verify_quest_glyph_cache_plan_structure, QuestEncodedPackBitsCatalog,
    QuestEncodedRowDictionaryCatalog, QuestGlyphCacheMaterials, QuestGlyphCachePlan,
    QUEST_GLYPH_CACHE_CELL_BYTES, QUEST_GLYPH_CACHE_KOREAN_DESCRIPTOR_BYTES,
    QUEST_GLYPH_CACHE_ROW_BYTES, QUEST_GLYPH_CACHE_SCRATCH_ALIGNMENT,
};
use super::quest_translation_routing::QuestTranslationRoutingAnalysis;

pub const QUEST_GLYPH_CACHE_PACKAGE_SCHEMA_VERSION: &str = "quest-glyph-cache-package-v3";
pub const QUEST_GLYPH_CACHE_VRAM_X: usize = 896;
pub const QUEST_GLYPH_CACHE_VRAM_Y: usize = 256;
pub const QUEST_GLYPH_CACHE_KOREAN_CELL_TABLE_RUNTIME: u32 = 0x8008_9D8C;
pub const QUEST_GLYPH_CACHE_KOREAN_PIXEL_FORMAT: u32 = 0;

/// plan의 section 크기에서 파생한 실제 package byte layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestGlyphCachePackageLayout {
    pub slot_rects_offset: usize,
    pub slot_rects_bytes: usize,
    pub korean_descriptor_offset: usize,
    pub korean_descriptor_bytes: usize,
    pub glyph_row_dictionary_offset: usize,
    pub glyph_row_dictionary_bytes: usize,
    pub glyph_cell_indices_offset: usize,
    pub glyph_cell_indices_bytes: usize,
    pub page_directory_offset: usize,
    pub page_directory_bytes: usize,
    pub page_assignments_offset: usize,
    pub page_assignments_bytes: usize,
    pub alignment_padding_offset: usize,
    pub alignment_padding_bytes: usize,
    pub scratch_offset: usize,
    pub scratch_bytes: usize,
    pub package_bytes: usize,
}

/// hook 입력으로 직접 쓸 수 있는 직렬화 package와 검산 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCachePackage {
    pub(super) catalog_glyphs: String,
    pub(super) menu_font_page: Option<u8>,
    /// 같은 source/조판에서 파생해 typed renderer에 넣는 page별 공백 slot.
    pub(super) space_slots: Vec<Option<u16>>,
    /// page별 영문 slot과 다음 cursor의 보정량. 글자 모양은 보존한다.
    pub(super) latin_advance_slots: Vec<Vec<(u16, i16)>>,
    /// 같은 glyph 순서에서 파생한 마지막 음절 code와 목적격 조사 code.
    pub(super) object_particle_codes: Vec<Vec<(u16, u16)>>,
    pub schema_version: &'static str,
    pub plan: QuestGlyphCachePlan,
    pub layout: QuestGlyphCachePackageLayout,
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub sections_exact: bool,
    pub catalog_readback_exact: bool,
    pub original_atlas_source_roundtrip_exact: bool,
    pub page_assignment_readback_exact: bool,
    pub slot_rect_readback_exact: bool,
    pub korean_descriptor_readback_exact: bool,
    pub scratch_zero: bool,
    pub runtime_verified: bool,
    pub build_eligible: bool,
    original_atlas_source_catalog: QuestEncodedPackBitsCatalog,
}

impl QuestGlyphCachePackage {
    fn decoded_original_atlas_source_cell(
        &self,
        index: usize,
    ) -> Result<Vec<u8>, QuestGlyphCachePackageError> {
        if index >= self.original_atlas_source_catalog.summary.cells {
            return Err(error(format!(
                "QUEST glyph cache original atlas source index 범위 이탈: index={index}, cells={}",
                self.original_atlas_source_catalog.summary.cells
            )));
        }
        if self.original_atlas_source_catalog.offsets.len()
            != self.original_atlas_source_catalog.summary.cells + 1
        {
            return Err(error(
                "QUEST glyph cache original atlas source offset 엔트리 수 불일치",
            ));
        }
        let start = usize::from(self.original_atlas_source_catalog.offsets[index]);
        let end = usize::from(self.original_atlas_source_catalog.offsets[index + 1]);
        if start > end {
            return Err(error(format!(
                "QUEST glyph cache original atlas source offset 역전: index={index}"
            )));
        }
        let encoded = self
            .original_atlas_source_catalog
            .payload
            .get(start..end)
            .ok_or_else(|| {
                error(format!(
                    "QUEST glyph cache original atlas source payload 범위 이탈: index={index}"
                ))
            })?;
        decode_packbits(encoded, QUEST_GLYPH_CACHE_CELL_BYTES)
            .map_err(|source| error(source.to_string()))
    }

    /// 직렬화된 glyph catalog의 한 cell을 hook과 같은 row dictionary 규칙으로 푼다.
    ///
    /// runtime verifier는 별도의 raster 재생성 결과가 아니라 실제 carrier
    /// package를 기준으로 VRAM cell을 대조해야 한다.
    pub fn glyph_cell(&self, catalog_index: u16) -> Result<Vec<u8>, QuestGlyphCachePackageError> {
        let index = usize::from(catalog_index);
        if index >= self.plan.glyphs {
            return Err(error(format!(
                "QUEST glyph cache glyph index 범위 이탈: index={index}, cells={}",
                self.plan.glyphs
            )));
        }
        let dictionary = section(
            &self.bytes,
            "glyph row dictionary",
            self.layout.glyph_row_dictionary_offset,
            self.layout.glyph_row_dictionary_bytes,
        )?;
        let indices = section(
            &self.bytes,
            "glyph cell row indices",
            self.layout.glyph_cell_indices_offset,
            self.layout.glyph_cell_indices_bytes,
        )?;
        let row_indices = cell_row_index_record(indices, self.plan.glyphs, index)
            .and_then(unpack_row_indices)
            .map_err(|source| error(source.to_string()))?;
        let mut cell = Vec::with_capacity(QUEST_GLYPH_CACHE_CELL_BYTES);
        for row_index in row_indices {
            let row_start = usize::from(row_index)
                .checked_mul(QUEST_GLYPH_CACHE_ROW_BYTES)
                .ok_or_else(|| error("QUEST glyph row dictionary offset overflow"))?;
            let row_end = row_start
                .checked_add(QUEST_GLYPH_CACHE_ROW_BYTES)
                .ok_or_else(|| error("QUEST glyph row dictionary 끝 overflow"))?;
            cell.extend_from_slice(
                dictionary
                    .get(row_start..row_end)
                    .ok_or_else(|| error("QUEST glyph row dictionary index 범위 이탈"))?,
            );
        }
        if cell.len() != QUEST_GLYPH_CACHE_CELL_BYTES {
            return Err(error("QUEST glyph row dictionary 해제 길이 불일치"));
        }
        Ok(cell)
    }

    /// package 밖에서 검산한 원본 atlas source의 정렬 slot 하나를 해제한다.
    pub fn original_atlas_source_cell(
        &self,
        slot_offset: usize,
    ) -> Result<Vec<u8>, QuestGlyphCachePackageError> {
        self.decoded_original_atlas_source_cell(slot_offset)
    }

    /// 직렬화된 package가 page 하나에 배정한 catalog index를 읽는다.
    ///
    /// hook과 같은 1-based page ID 및 delta ULEB128 표현을 사용한다. PoC
    /// stream은 이 결과의 순서와 slot code를 결속해야 하며 별도의 page
    /// 계산 결과를 신뢰하지 않는다.
    pub fn page_catalog_indices(
        &self,
        page_id: u8,
    ) -> Result<Vec<u16>, QuestGlyphCachePackageError> {
        let page_index = usize::from(page_id)
            .checked_sub(1)
            .filter(|index| *index < self.plan.pages)
            .ok_or_else(|| {
                error(format!(
                    "QUEST glyph cache page ID 범위 이탈: page={page_id}, pages={}",
                    self.plan.pages
                ))
            })?;
        let directory = decode_u16_table(section(
            &self.bytes,
            "page directory",
            self.layout.page_directory_offset,
            self.layout.page_directory_bytes,
        )?)?;
        if directory.len() != self.plan.pages + 1 {
            return Err(error("QUEST glyph cache page directory 엔트리 수 불일치"));
        }
        let start = usize::from(directory[page_index]);
        let end = usize::from(directory[page_index + 1]);
        if start > end {
            return Err(error(format!(
                "QUEST glyph cache page assignment offset 역전: page={page_id}"
            )));
        }
        let payload = section(
            &self.bytes,
            "page assignments",
            self.layout.page_assignments_offset,
            self.layout.page_assignments_bytes,
        )?;
        let encoded = payload.get(start..end).ok_or_else(|| {
            error(format!(
                "QUEST glyph cache page assignment 범위 이탈: page={page_id}"
            ))
        })?;
        decode_page_indices(encoded).map_err(|source| error(source.to_string()))
    }
}

/// package layout, 직렬화, readback 기준선 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCachePackageError(String);

impl fmt::Display for QuestGlyphCachePackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestGlyphCachePackageError {}

fn error(message: impl Into<String>) -> QuestGlyphCachePackageError {
    QuestGlyphCachePackageError(message.into())
}

impl QuestGlyphCachePackageLayout {
    pub(super) fn from_plan(
        plan: &QuestGlyphCachePlan,
    ) -> Result<Self, QuestGlyphCachePackageError> {
        let slot_rects_offset = 0;
        let slot_rects_bytes = plan.slot_rect_bytes;
        let korean_descriptor_offset = slot_rects_offset + slot_rects_bytes;
        let korean_descriptor_bytes = plan.korean_descriptor_bytes;
        let glyph_row_dictionary_offset = korean_descriptor_offset
            .checked_add(korean_descriptor_bytes)
            .ok_or_else(|| error("QUEST glyph package Korean descriptor 끝 overflow"))?;
        let glyph_row_dictionary_bytes = plan.catalog.dictionary_bytes;
        let glyph_cell_indices_offset = glyph_row_dictionary_offset
            .checked_add(glyph_row_dictionary_bytes)
            .ok_or_else(|| error("QUEST glyph package dictionary 끝 overflow"))?;
        let glyph_cell_indices_bytes = plan.catalog.cell_index_bytes;
        let page_directory_offset = glyph_cell_indices_offset
            .checked_add(glyph_cell_indices_bytes)
            .ok_or_else(|| error("QUEST glyph package cell index 끝 overflow"))?;
        let page_directory_bytes = plan.page_directory_bytes;
        let page_assignments_offset = page_directory_offset
            .checked_add(page_directory_bytes)
            .ok_or_else(|| error("QUEST glyph package page directory 끝 overflow"))?;
        let page_assignments_bytes = plan.page_assignment_bytes;
        let alignment_padding_offset = page_assignments_offset
            .checked_add(page_assignments_bytes)
            .ok_or_else(|| error("QUEST glyph package page assignment 끝 overflow"))?;
        let alignment_padding_bytes = plan.alignment_padding_bytes;
        let scratch_offset = alignment_padding_offset
            .checked_add(alignment_padding_bytes)
            .ok_or_else(|| error("QUEST glyph package padding 끝 overflow"))?;
        let scratch_bytes = plan.scratch_bytes;
        let package_bytes = scratch_offset
            .checked_add(scratch_bytes)
            .ok_or_else(|| error("QUEST glyph package scratch 끝 overflow"))?;
        if package_bytes != plan.package_bytes
            || !scratch_offset.is_multiple_of(QUEST_GLYPH_CACHE_SCRATCH_ALIGNMENT)
        {
            return Err(error("QUEST glyph package plan section 합계·정렬 불일치"));
        }
        Ok(Self {
            slot_rects_offset,
            slot_rects_bytes,
            korean_descriptor_offset,
            korean_descriptor_bytes,
            glyph_row_dictionary_offset,
            glyph_row_dictionary_bytes,
            glyph_cell_indices_offset,
            glyph_cell_indices_bytes,
            page_directory_offset,
            page_directory_bytes,
            page_assignments_offset,
            page_assignments_bytes,
            alignment_padding_offset,
            alignment_padding_bytes,
            scratch_offset,
            scratch_bytes,
            package_bytes,
        })
    }
}

struct PackageWriter {
    bytes: Vec<u8>,
    claimed: Vec<bool>,
}

impl PackageWriter {
    fn new(package_bytes: usize) -> Self {
        Self {
            bytes: vec![0; package_bytes],
            claimed: vec![false; package_bytes],
        }
    }

    fn write_section(
        &mut self,
        name: &str,
        offset: usize,
        expected_len: usize,
        raw: &[u8],
    ) -> Result<(), QuestGlyphCachePackageError> {
        if raw.len() != expected_len {
            return Err(error(format!(
                "QUEST glyph cache {name} 길이 불일치: expected={expected_len}, actual={}",
                raw.len()
            )));
        }
        let end = offset
            .checked_add(raw.len())
            .ok_or_else(|| error(format!("QUEST glyph cache {name} 범위 overflow")))?;
        let target = self.bytes.get_mut(offset..end).ok_or_else(|| {
            error(format!(
                "QUEST glyph cache {name} package 범위 초과: {offset}..{end}"
            ))
        })?;
        let claimed = self
            .claimed
            .get_mut(offset..end)
            .ok_or_else(|| error(format!("QUEST glyph cache {name} claim 범위 초과")))?;
        if claimed.iter().any(|byte| *byte) {
            return Err(error(format!("QUEST glyph cache {name} 쓰기 겹침")));
        }
        target.copy_from_slice(raw);
        claimed.fill(true);
        Ok(())
    }

    fn finish(self) -> Result<Vec<u8>, QuestGlyphCachePackageError> {
        if let Some(offset) = self.claimed.iter().position(|claimed| !claimed) {
            return Err(error(format!(
                "QUEST glyph cache package 미등록 바이트: offset={offset}"
            )));
        }
        Ok(self.bytes)
    }
}

fn encode_u16_table(values: &[u16]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn decode_u16_table(raw: &[u8]) -> Result<Vec<u16>, QuestGlyphCachePackageError> {
    if !raw.len().is_multiple_of(2) {
        return Err(error("QUEST glyph cache u16 table 길이가 홀수임"));
    }
    Ok(raw
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect())
}

fn section<'a>(
    package: &'a [u8],
    name: &str,
    offset: usize,
    len: usize,
) -> Result<&'a [u8], QuestGlyphCachePackageError> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| error(format!("QUEST glyph cache {name} readback 범위 overflow")))?;
    package
        .get(offset..end)
        .ok_or_else(|| error(format!("QUEST glyph cache {name} readback 범위 초과")))
}

fn verify_row_dictionary_readback(
    package: &[u8],
    layout: QuestGlyphCachePackageLayout,
    catalog: &QuestEncodedRowDictionaryCatalog,
) -> Result<(), QuestGlyphCachePackageError> {
    let dictionary = section(
        package,
        "glyph row dictionary",
        layout.glyph_row_dictionary_offset,
        layout.glyph_row_dictionary_bytes,
    )?;
    let packed_indices = section(
        package,
        "glyph cell row indices",
        layout.glyph_cell_indices_offset,
        layout.glyph_cell_indices_bytes,
    )?;
    let expected_dictionary = catalog.rows.iter().flatten().copied().collect::<Vec<_>>();
    if dictionary != expected_dictionary || packed_indices != catalog.packed_cell_indices {
        return Err(error("QUEST glyph row dictionary 직렬화 readback 불일치"));
    }
    for (cell_index, expected) in catalog.cells.iter().enumerate() {
        let row_indices = cell_row_index_record(packed_indices, catalog.cells.len(), cell_index)
            .and_then(unpack_row_indices)
            .map_err(|source| error(source.to_string()))?;
        let mut actual = Vec::with_capacity(QUEST_GLYPH_CACHE_CELL_BYTES);
        for row_index in row_indices {
            let row_start = usize::from(row_index)
                .checked_mul(QUEST_GLYPH_CACHE_ROW_BYTES)
                .ok_or_else(|| error("QUEST glyph row readback dictionary offset overflow"))?;
            let row_end = row_start
                .checked_add(QUEST_GLYPH_CACHE_ROW_BYTES)
                .ok_or_else(|| error("QUEST glyph row readback dictionary 끝 overflow"))?;
            actual.extend_from_slice(
                dictionary
                    .get(row_start..row_end)
                    .ok_or_else(|| error("QUEST glyph row readback dictionary 범위 이탈"))?,
            );
        }
        if actual != *expected {
            return Err(error(format!(
                "QUEST glyph row dictionary cell readback 불일치: index={cell_index}"
            )));
        }
    }
    Ok(())
}

fn encode_slot_rects(
    materials: &QuestGlyphCacheMaterials,
) -> Result<Vec<u8>, QuestGlyphCachePackageError> {
    let mut raw = Vec::with_capacity(materials.plan.slot_rect_bytes);
    for slot in &materials.original_atlas_source_slots {
        if slot.x % 4 != 0 || slot.width != 12 || slot.height != 16 {
            return Err(error(format!(
                "QUEST glyph cache slot RECT 경계 불일치: {slot:?}"
            )));
        }
        let x = QUEST_GLYPH_CACHE_VRAM_X
            .checked_add(slot.x / 4)
            .and_then(|value| u16::try_from(value).ok())
            .ok_or_else(|| error("QUEST glyph cache slot RECT x 변환 실패"))?;
        let y = QUEST_GLYPH_CACHE_VRAM_Y
            .checked_add(slot.y)
            .and_then(|value| u16::try_from(value).ok())
            .ok_or_else(|| error("QUEST glyph cache slot RECT y 변환 실패"))?;
        raw.extend_from_slice(&x.to_le_bytes());
        raw.extend_from_slice(&y.to_le_bytes());
    }
    Ok(raw)
}

fn encode_korean_descriptor() -> Vec<u8> {
    let mut raw = vec![0; QUEST_GLYPH_CACHE_KOREAN_DESCRIPTOR_BYTES];
    raw[0x18..0x1A].copy_from_slice(&(QUEST_GLYPH_CACHE_VRAM_X as u16).to_le_bytes());
    raw[0x1A..0x1C].copy_from_slice(&(QUEST_GLYPH_CACHE_VRAM_Y as u16).to_le_bytes());
    raw[0x1C..0x20].copy_from_slice(&QUEST_GLYPH_CACHE_KOREAN_CELL_TABLE_RUNTIME.to_le_bytes());
    raw[0x20..0x24].copy_from_slice(&QUEST_GLYPH_CACHE_KOREAN_PIXEL_FORMAT.to_le_bytes());
    raw
}

fn verify_page_assignment_readback(
    package: &[u8],
    layout: QuestGlyphCachePackageLayout,
    materials: &QuestGlyphCacheMaterials,
) -> Result<(), QuestGlyphCachePackageError> {
    let directory = decode_u16_table(section(
        package,
        "page directory",
        layout.page_directory_offset,
        layout.page_directory_bytes,
    )?)?;
    let payload = section(
        package,
        "page assignments",
        layout.page_assignments_offset,
        layout.page_assignments_bytes,
    )?;
    if directory != materials.page_assignments.directory
        || payload != materials.page_assignments.payload
        || directory.len() != materials.page_assignments.pages.len() + 1
        || directory.first() != Some(&0)
        || directory.last().copied().map(usize::from) != Some(payload.len())
    {
        return Err(error(
            "QUEST glyph cache page assignment 직렬화 readback 불일치",
        ));
    }
    for (index, expected) in materials.page_assignments.pages.iter().enumerate() {
        let start = usize::from(directory[index]);
        let end = usize::from(directory[index + 1]);
        let encoded = payload
            .get(start..end)
            .ok_or_else(|| error("QUEST glyph cache page assignment 범위 초과"))?;
        let actual = decode_page_indices(encoded).map_err(|source| error(source.to_string()))?;
        if actual != *expected {
            return Err(error(format!(
                "QUEST glyph cache page assignment readback 불일치: page={}",
                index + 1
            )));
        }
    }
    Ok(())
}

fn verify_original_atlas_source_roundtrip(
    catalog: &QuestEncodedPackBitsCatalog,
) -> Result<(), QuestGlyphCachePackageError> {
    if catalog.offsets.len() != catalog.cells.len() + 1
        || catalog.summary.cells != catalog.cells.len()
        || catalog.offsets.first() != Some(&0)
        || catalog.offsets.last().copied().map(usize::from) != Some(catalog.payload.len())
    {
        return Err(error(
            "QUEST glyph cache original atlas source catalog 구조 불일치",
        ));
    }
    for (index, expected) in catalog.cells.iter().enumerate() {
        let start = usize::from(catalog.offsets[index]);
        let end = usize::from(catalog.offsets[index + 1]);
        let encoded = catalog
            .payload
            .get(start..end)
            .ok_or_else(|| error("QUEST glyph cache original atlas source payload 범위 이탈"))?;
        let actual = decode_packbits(encoded, QUEST_GLYPH_CACHE_CELL_BYTES)
            .map_err(|source| error(source.to_string()))?;
        if actual != *expected {
            return Err(error(format!(
                "QUEST glyph cache original atlas source roundtrip 불일치: index={index}"
            )));
        }
    }
    Ok(())
}

pub(super) fn serialize_quest_glyph_cache_materials(
    materials: QuestGlyphCacheMaterials,
    schema_version: &'static str,
) -> Result<QuestGlyphCachePackage, QuestGlyphCachePackageError> {
    let plan = materials.plan.clone();
    let layout = QuestGlyphCachePackageLayout::from_plan(&plan)?;
    let slot_rects = encode_slot_rects(&materials)?;
    let korean_descriptor = encode_korean_descriptor();
    let glyph_row_dictionary = materials
        .glyph_catalog
        .rows
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let page_directory = encode_u16_table(&materials.page_assignments.directory);

    let mut writer = PackageWriter::new(layout.package_bytes);
    writer.write_section(
        "slot rects",
        layout.slot_rects_offset,
        layout.slot_rects_bytes,
        &slot_rects,
    )?;
    writer.write_section(
        "Korean descriptor",
        layout.korean_descriptor_offset,
        layout.korean_descriptor_bytes,
        &korean_descriptor,
    )?;
    writer.write_section(
        "glyph row dictionary",
        layout.glyph_row_dictionary_offset,
        layout.glyph_row_dictionary_bytes,
        &glyph_row_dictionary,
    )?;
    writer.write_section(
        "glyph cell row indices",
        layout.glyph_cell_indices_offset,
        layout.glyph_cell_indices_bytes,
        &materials.glyph_catalog.packed_cell_indices,
    )?;
    writer.write_section(
        "page directory",
        layout.page_directory_offset,
        layout.page_directory_bytes,
        &page_directory,
    )?;
    writer.write_section(
        "page assignments",
        layout.page_assignments_offset,
        layout.page_assignments_bytes,
        &materials.page_assignments.payload,
    )?;
    let alignment_padding = vec![0; layout.alignment_padding_bytes];
    writer.write_section(
        "alignment padding",
        layout.alignment_padding_offset,
        layout.alignment_padding_bytes,
        &alignment_padding,
    )?;
    let scratch = vec![0; layout.scratch_bytes];
    writer.write_section(
        "scratch",
        layout.scratch_offset,
        layout.scratch_bytes,
        &scratch,
    )?;
    let bytes = writer.finish()?;

    verify_row_dictionary_readback(&bytes, layout, &materials.glyph_catalog)?;
    verify_original_atlas_source_roundtrip(&materials.original_atlas_source_catalog)?;
    verify_page_assignment_readback(&bytes, layout, &materials)?;
    if section(
        &bytes,
        "slot rects",
        layout.slot_rects_offset,
        layout.slot_rects_bytes,
    )? != slot_rects
    {
        return Err(error("QUEST glyph cache slot RECT readback 불일치"));
    }
    if section(
        &bytes,
        "Korean descriptor",
        layout.korean_descriptor_offset,
        layout.korean_descriptor_bytes,
    )? != korean_descriptor
    {
        return Err(error("QUEST glyph cache Korean descriptor readback 불일치"));
    }
    let scratch_zero = section(
        &bytes,
        "scratch",
        layout.scratch_offset,
        layout.scratch_bytes,
    )?
    .iter()
    .all(|byte| *byte == 0);
    if !scratch_zero {
        return Err(error("QUEST glyph cache scratch 초기값이 0이 아님"));
    }

    Ok(QuestGlyphCachePackage {
        catalog_glyphs: materials.catalog_glyphs,
        menu_font_page: materials.menu_font_page,
        space_slots: materials.space_slots,
        latin_advance_slots: materials.latin_advance_slots,
        object_particle_codes: materials.object_particle_codes,
        schema_version,
        plan,
        layout,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        bytes,
        sections_exact: true,
        catalog_readback_exact: true,
        original_atlas_source_roundtrip_exact: true,
        page_assignment_readback_exact: true,
        slot_rect_readback_exact: true,
        korean_descriptor_readback_exact: true,
        scratch_zero,
        runtime_verified: false,
        build_eligible: false,
        original_atlas_source_catalog: materials.original_atlas_source_catalog,
    })
}

/// 지원 원본과 provisional batch로 실제 carrier package를 만든다.
pub fn audit_quest_glyph_cache_package(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
    routing: &QuestTranslationRoutingAnalysis,
) -> Result<QuestGlyphCachePackage, QuestGlyphCachePackageError> {
    let materials = build_quest_glyph_cache_materials(source, batches, routing)
        .map_err(|source| error(source.to_string()))?;
    verify_quest_glyph_cache_plan_structure(materials.plan.clone())
        .map_err(|source| error(source.to_string()))?;
    serialize_quest_glyph_cache_materials(materials, QUEST_GLYPH_CACHE_PACKAGE_SCHEMA_VERSION)
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

/// 현재 입력에서 파생한 package layout과 모든 역검산을 요구한다.
pub fn verify_box_quest_glyph_cache_package(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
    routing: &QuestTranslationRoutingAnalysis,
) -> Result<QuestGlyphCachePackage, QuestGlyphCachePackageError> {
    let package = audit_quest_glyph_cache_package(source, batches, routing)?;
    let assigned = (1..=package.plan.pages)
        .map(|page| {
            let page_id =
                u8::try_from(page).map_err(|_| error("QUEST glyph package page ID가 u8을 넘음"))?;
            package
                .page_catalog_indices(page_id)
                .map(|indices| indices.len())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_layout = QuestGlyphCachePackageLayout::from_plan(&package.plan)?;
    if package.schema_version != QUEST_GLYPH_CACHE_PACKAGE_SCHEMA_VERSION
        || package.layout != expected_layout
        || package.layout.package_bytes != package.plan.package_bytes
        || package.bytes.len() != package.layout.package_bytes
        || assigned.iter().sum::<usize>() != package.plan.page_assignments
        || assigned.iter().copied().max().unwrap_or(0) != package.plan.max_page_glyphs
        || !is_sha256_hex(&package.sha256)
        || package.sha256 != format!("{:x}", Sha256::digest(&package.bytes))
        || !package.sections_exact
        || !package.catalog_readback_exact
        || !package.original_atlas_source_roundtrip_exact
        || !package.page_assignment_readback_exact
        || !package.slot_rect_readback_exact
        || !package.korean_descriptor_readback_exact
        || !package.scratch_zero
        || package.runtime_verified
        || package.build_eligible
    {
        return Err(error("QUEST glyph cache package 판정 drift"));
    }
    Ok(package)
}

#[cfg(test)]
#[path = "quest_glyph_cache_package_tests.rs"]
mod tests;
