//! QUEST 글리프 공급·재삽입 대안의 저비용 비교 PoC.
//!
//! 승인 전 초벌 수요는 위험 신호로만 사용한다. 정적 확장, 장면별 page,
//! active cache, BIOS 경로 이식, 조합형, baked 대사를 같은 표에서
//! 비교하되 저장 성공을 runtime 소비 성공으로 승격하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::archive::{VddArchive, VddError};
use crate::btx::{decode, encode, Layout, PixelFormat};
use crate::compress::{compress, decompress, parse_header};
use crate::disasm::QuestFontConsumerSummary;
use crate::disc::{apply_form1_write_plan, DiscImage, Iso9660, VerifiedSourceImage};
use crate::font::{
    bundled_maplestory_light, rasterize_wide_glyph, QuestGlyphSlot, QUEST_CODE_TABLE_COUNT,
    QUEST_FONT_CELL_HEIGHT, QUEST_FONT_LAYOUT, QUEST_FONT_NARROW_CELL_WIDTH,
    QUEST_FONT_NARROW_SLOTS, QUEST_FONT_WIDE_CELL_WIDTH, QUEST_FONT_WIDE_COLUMNS,
    QUEST_FONT_WIDE_SLOTS, QUEST_POC_RASTER_PROFILE,
};
use crate::patch::WritePlan;
use crate::script::{QuestTranslationBatchAnalysis, QuestTranslationBatchRole};

use super::quest_capacity_poc::{QuestCapacityPocAnalysis, RPG_FONT_NAME, RPG_GRP_PATH};
use super::vdd_plan::{add_vdd_repack, logical_file};

pub const QUEST_FONT_ALTERNATIVES_SCHEMA_VERSION: &str = "quest-font-alternatives-v2";
pub const BOX_QUEST_FONT_ALTERNATIVES_METHOD_ROWS: usize = 12;
pub const BOX_QUEST_FONT_ALTERNATIVES_REPORT_SHA256: &str =
    "599e82ff151bbae964dc51695ccdfe5a972734c43de5da4d985cc5d1c72d7046";

const COMPACT_CELL_WIDTH: usize = 6;
const GLYPH_BYTES_INDEXED4: usize = QUEST_FONT_WIDE_CELL_WIDTH * QUEST_FONT_CELL_HEIGHT / 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFontAlternativesAnalysis {
    pub provisional_glyphs: usize,
    pub code_table_entries: usize,
    pub current_wide_slots: usize,
    pub quest_bios_font_routes: usize,
    pub batch_pages: usize,
    pub max_batch_glyphs: usize,
    pub all_batches_fit_current_page: bool,
    pub batch_page_plain_total: usize,
    pub batch_page_cnx_min: usize,
    pub batch_page_cnx_max: usize,
    pub batch_page_cnx_total: usize,
    pub batch_pages_in_place_fit: usize,
    pub glyph_catalog_plain_bytes: usize,
    pub glyph_catalog_cnx_bytes: usize,
    pub extended_atlas_width: usize,
    pub extended_atlas_height: usize,
    pub extended_atlas_plain_bytes: usize,
    pub extended_atlas_cnx_bytes: usize,
    pub extended_atlas_in_place_fit: bool,
    pub extended_atlas_vdd_repack_fit: bool,
    pub extended_atlas_disc_readback: bool,
    pub extended_atlas_data_writes: usize,
    pub extended_atlas_touched_sectors: usize,
    pub extended_atlas_final_expected_writes: usize,
    pub extended_atlas_plain_sha256: String,
    pub extended_atlas_cnx_sha256: String,
    pub extended_atlas_final_image_sha256: String,
    pub compact_physical_slots: usize,
    pub current_profile_compact_fit_glyphs: usize,
    pub jamo_initials: usize,
    pub jamo_vowels: usize,
    pub jamo_finals: usize,
    pub jamo_components: usize,
    pub fixed_slot_comparable_entries: usize,
    pub fixed_slot_overflow_entries: usize,
    pub fixed_slot_max_overrun: usize,
    pub build_eligible_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFontAlternativesError(String);

impl fmt::Display for QuestFontAlternativesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestFontAlternativesError {}

fn error(message: impl Into<String>) -> QuestFontAlternativesError {
    QuestFontAlternativesError(message.into())
}

fn sha256(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn is_hangul(character: char) -> bool {
    matches!(character, '\u{ac00}'..='\u{d7a3}')
}

fn wide_slots() -> Result<Vec<QuestGlyphSlot>, QuestFontAlternativesError> {
    let slots = (0..QUEST_CODE_TABLE_COUNT)
        .filter_map(|index| {
            let slot = QuestGlyphSlot::for_index(index).ok()?;
            (slot.width == QUEST_FONT_WIDE_CELL_WIDTH).then_some(slot)
        })
        .collect::<Vec<_>>();
    if slots.len() != QUEST_FONT_WIDE_SLOTS {
        return Err(error(format!(
            "QUEST 대안 wide slot 분모 불일치: expected={}, actual={}",
            QUEST_FONT_WIDE_SLOTS,
            slots.len()
        )));
    }
    Ok(slots)
}

fn raster_map(repertoire: &str) -> Result<BTreeMap<char, Vec<u8>>, QuestFontAlternativesError> {
    let font = bundled_maplestory_light().map_err(|source| error(source.to_string()))?;
    repertoire
        .chars()
        .map(|glyph| {
            rasterize_wide_glyph(&font, glyph, QUEST_POC_RASTER_PROFILE)
                .map(|pixels| (glyph, pixels))
                .map_err(|source| error(source.to_string()))
        })
        .collect()
}

fn copy_glyph(
    atlas: &mut [u8],
    layout: Layout,
    x: usize,
    y: usize,
    glyph: &[u8],
) -> Result<(), QuestFontAlternativesError> {
    if glyph.len() != QUEST_FONT_WIDE_CELL_WIDTH * QUEST_FONT_CELL_HEIGHT {
        return Err(error(format!(
            "QUEST 대안 glyph 픽셀 길이 불일치: expected={}, actual={}",
            QUEST_FONT_WIDE_CELL_WIDTH * QUEST_FONT_CELL_HEIGHT,
            glyph.len()
        )));
    }
    if x + QUEST_FONT_WIDE_CELL_WIDTH > layout.width || y + QUEST_FONT_CELL_HEIGHT > layout.height {
        return Err(error(format!(
            "QUEST 대안 glyph atlas 범위 이탈: ({x},{y}) in {}x{}",
            layout.width, layout.height
        )));
    }
    for row in 0..QUEST_FONT_CELL_HEIGHT {
        let source_start = row * QUEST_FONT_WIDE_CELL_WIDTH;
        let target_start = (y + row) * layout.width + x;
        atlas[target_start..target_start + QUEST_FONT_WIDE_CELL_WIDTH]
            .copy_from_slice(&glyph[source_start..source_start + QUEST_FONT_WIDE_CELL_WIDTH]);
    }
    Ok(())
}

fn clean_current_page(
    original_plain: &[u8],
    slots: &[QuestGlyphSlot],
) -> Result<Vec<u8>, QuestFontAlternativesError> {
    let mut pixels =
        decode(original_plain, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;
    for slot in slots {
        for row in 0..slot.height {
            let start = (slot.y + row) * QUEST_FONT_LAYOUT.width + slot.x;
            pixels[start..start + slot.width].fill(0);
        }
    }
    Ok(pixels)
}

fn measure_batch_pages(
    batches: &QuestTranslationBatchAnalysis,
    rasters: &BTreeMap<char, Vec<u8>>,
    original_plain: &[u8],
    subtype: [u8; 3],
    slot_capacity: usize,
) -> Result<(usize, usize, usize, usize, usize, bool), QuestFontAlternativesError> {
    let mut grouped = BTreeMap::<&str, BTreeSet<char>>::new();
    for entry in &batches.entries {
        if entry.role != QuestTranslationBatchRole::Translate {
            continue;
        }
        let glyphs = grouped.entry(&entry.batch_id).or_default();
        glyphs.extend(entry.korean_draft.chars().filter(|glyph| is_hangul(*glyph)));
    }
    let slots = wide_slots()?;
    let clean = clean_current_page(original_plain, &slots)?;
    let mut cnx_sizes = Vec::with_capacity(grouped.len());
    let mut all_fit = true;
    for (batch_id, glyphs) in grouped {
        if glyphs.len() > slots.len() {
            all_fit = false;
            continue;
        }
        let mut pixels = clean.clone();
        for (slot, glyph) in slots.iter().zip(glyphs) {
            let glyph_pixels = rasters.get(&glyph).ok_or_else(|| {
                error(format!(
                    "{batch_id}: raster map에 batch glyph {glyph:?}가 없음"
                ))
            })?;
            copy_glyph(&mut pixels, QUEST_FONT_LAYOUT, slot.x, slot.y, glyph_pixels)?;
        }
        let plain =
            encode(&pixels, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;
        let cnx = compress(&plain, subtype).map_err(|source| error(source.to_string()))?;
        if decompress(&cnx).map_err(|source| error(source.to_string()))? != plain {
            return Err(error(format!("{batch_id}: page CNX 자기 readback 불일치")));
        }
        cnx_sizes.push(cnx.len());
    }
    if cnx_sizes.len() != batches.batches {
        return Err(error(format!(
            "QUEST 대안 batch page 수 불일치: expected={}, actual={}",
            batches.batches,
            cnx_sizes.len()
        )));
    }
    let min = *cnx_sizes
        .iter()
        .min()
        .ok_or_else(|| error("QUEST 대안 batch page가 비어 있음"))?;
    let max = *cnx_sizes
        .iter()
        .max()
        .ok_or_else(|| error("QUEST 대안 batch page가 비어 있음"))?;
    let total = cnx_sizes.iter().sum();
    let in_place = cnx_sizes
        .iter()
        .filter(|size| **size <= slot_capacity)
        .count();
    Ok((cnx_sizes.len(), min, max, total, in_place, all_fit))
}

fn build_glyph_catalog(
    rasters: &BTreeMap<char, Vec<u8>>,
) -> Result<(Vec<u8>, Vec<u8>), QuestFontAlternativesError> {
    let glyph_layout = Layout {
        width: QUEST_FONT_WIDE_CELL_WIDTH,
        height: QUEST_FONT_CELL_HEIGHT,
        format: PixelFormat::Indexed4,
    };
    let mut plain = Vec::with_capacity(rasters.len() * GLYPH_BYTES_INDEXED4);
    for pixels in rasters.values() {
        plain.extend(encode(pixels, glyph_layout).map_err(|source| error(source.to_string()))?);
    }
    let cnx = compress(&plain, *b"bin").map_err(|source| error(source.to_string()))?;
    if decompress(&cnx).map_err(|source| error(source.to_string()))? != plain {
        return Err(error("QUEST 대안 glyph catalog CNX 자기 readback 불일치"));
    }
    Ok((plain, cnx))
}

fn extended_atlas_height(glyphs: usize) -> usize {
    let rows_after_first = glyphs
        .saturating_sub(crate::font::QUEST_FONT_FIRST_ROW_WIDE_SLOTS)
        .div_ceil(QUEST_FONT_WIDE_COLUMNS);
    ((1 + rows_after_first) * QUEST_FONT_CELL_HEIGHT).max(QUEST_FONT_LAYOUT.height)
}

fn build_extended_atlas(
    original_plain: &[u8],
    rasters: &BTreeMap<char, Vec<u8>>,
) -> Result<Vec<u8>, QuestFontAlternativesError> {
    let layout = Layout {
        width: QUEST_FONT_LAYOUT.width,
        height: extended_atlas_height(rasters.len()),
        format: PixelFormat::Indexed4,
    };
    let original_pixels =
        decode(original_plain, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;
    let mut pixels = vec![0u8; layout.width * layout.height];
    let narrow_width = QUEST_FONT_NARROW_SLOTS * QUEST_FONT_NARROW_CELL_WIDTH;
    pixels[..narrow_width].copy_from_slice(&original_pixels[..narrow_width]);

    for (wide_index, glyph_pixels) in rasters.values().enumerate() {
        let (x, y) = if wide_index < crate::font::QUEST_FONT_FIRST_ROW_WIDE_SLOTS {
            (narrow_width + wide_index * QUEST_FONT_WIDE_CELL_WIDTH, 0)
        } else {
            let index = wide_index - crate::font::QUEST_FONT_FIRST_ROW_WIDE_SLOTS;
            (
                (index % QUEST_FONT_WIDE_COLUMNS) * QUEST_FONT_WIDE_CELL_WIDTH,
                (1 + index / QUEST_FONT_WIDE_COLUMNS) * QUEST_FONT_CELL_HEIGHT,
            )
        };
        copy_glyph(&mut pixels, layout, x, y, glyph_pixels)?;
    }
    encode(&pixels, layout).map_err(|source| error(source.to_string()))
}

fn compact_bbox_fit(rasters: &BTreeMap<char, Vec<u8>>) -> usize {
    rasters
        .values()
        .filter(|pixels| {
            let mut min_x = QUEST_FONT_WIDE_CELL_WIDTH;
            let mut max_x = 0usize;
            let mut any = false;
            for (offset, pixel) in pixels.iter().enumerate() {
                if *pixel == 0 {
                    continue;
                }
                any = true;
                let x = offset % QUEST_FONT_WIDE_CELL_WIDTH;
                min_x = min_x.min(x);
                max_x = max_x.max(x);
            }
            any && max_x - min_x < COMPACT_CELL_WIDTH
        })
        .count()
}

fn jamo_counts(repertoire: &str) -> (usize, usize, usize) {
    let mut initials = BTreeSet::new();
    let mut vowels = BTreeSet::new();
    let mut finals = BTreeSet::new();
    for glyph in repertoire.chars().filter(|glyph| is_hangul(*glyph)) {
        let index = glyph as u32 - 0xAC00;
        initials.insert(index / 588);
        vowels.insert((index % 588) / 28);
        let final_index = index % 28;
        if final_index != 0 {
            finals.insert(final_index);
        }
    }
    (initials.len(), vowels.len(), finals.len())
}

/// 승인 전 수요로 공급 대안의 저장·현재 소비 경계를 비교한다.
pub fn audit_quest_font_alternatives(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
    capacity: &QuestCapacityPocAnalysis,
    consumer: &QuestFontConsumerSummary,
    quest_bios_font_routes: usize,
) -> Result<QuestFontAlternativesAnalysis, QuestFontAlternativesError> {
    if capacity.provisional_unique_hangul != batches.draft_hangul_repertoire.chars().count()
        || consumer.code_table_entries != QUEST_CODE_TABLE_COUNT
    {
        return Err(error(
            "QUEST 대안 입력 분모가 capacity/consumer gate와 불일치",
        ));
    }
    if quest_bios_font_routes != 0 {
        return Err(error(format!(
            "QUEST BIOS font route 분모 drift: expected=0, actual={quest_bios_font_routes}"
        )));
    }

    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let grp_entry = iso
        .find(RPG_GRP_PATH)
        .map_err(|source| error(source.to_string()))?
        .clone();
    let file = logical_file(&grp_entry).map_err(error)?;
    let original_vdd = iso
        .read_file(RPG_GRP_PATH)
        .map_err(|source| error(source.to_string()))?;
    let archive = VddArchive::parse(&original_vdd).map_err(|source| error(source.to_string()))?;
    let font_entry = archive
        .find(RPG_FONT_NAME)
        .ok_or_else(|| error("QUEST 대안 RPG_FNT1.BTX가 없음"))?;
    let original_cnx = archive.read_file(font_entry);
    let header = parse_header(original_cnx).map_err(|source| error(source.to_string()))?;
    let original_plain = decompress(original_cnx).map_err(|source| error(source.to_string()))?;
    let current_page_bytes = QUEST_FONT_LAYOUT
        .byte_len()
        .map_err(|source| error(source.to_string()))?;
    if original_plain.len() != current_page_bytes {
        return Err(error("QUEST 대안 원본 font layout 길이 불일치"));
    }
    let slot_capacity = archive
        .entries()
        .iter()
        .filter(|entry| entry.byte_offset() > font_entry.byte_offset())
        .map(|entry| entry.byte_offset())
        .min()
        .unwrap_or_else(|| archive.serialize_lossless().len())
        .saturating_sub(font_entry.byte_offset());

    let rasters = raster_map(&batches.draft_hangul_repertoire)?;
    let (batch_pages, batch_cnx_min, batch_cnx_max, batch_cnx_total, pages_in_place, all_fit) =
        measure_batch_pages(
            batches,
            &rasters,
            &original_plain,
            header.subtype,
            slot_capacity,
        )?;
    let (catalog_plain, catalog_cnx) = build_glyph_catalog(&rasters)?;
    let extended_plain = build_extended_atlas(&original_plain, &rasters)?;
    let extended_cnx =
        compress(&extended_plain, header.subtype).map_err(|source| error(source.to_string()))?;
    if decompress(&extended_cnx).map_err(|source| error(source.to_string()))? != extended_plain {
        return Err(error("QUEST 대안 extended atlas CNX 자기 readback 불일치"));
    }

    let extended_atlas_in_place_fit = extended_cnx.len() <= slot_capacity;
    let mut extended_atlas_vdd_repack_fit = false;
    let mut extended_atlas_disc_readback = false;
    let mut extended_atlas_data_writes = 0usize;
    let mut extended_atlas_touched_sectors = 0usize;
    let mut extended_atlas_final_expected_writes = 0usize;
    let mut extended_atlas_final_image_sha256 = String::new();
    match archive.repack_in_container(&BTreeMap::from([(
        RPG_FONT_NAME.to_string(),
        extended_cnx.clone(),
    )])) {
        Ok(new_vdd) => {
            extended_atlas_vdd_repack_fit = true;
            let mut data_plan = WritePlan::new();
            add_vdd_repack(
                &mut data_plan,
                file,
                &original_vdd,
                &new_vdd,
                "quest-font-expanded-storage",
                "pipeline::quest_font_alternatives",
            )
            .map_err(error)?;
            extended_atlas_data_writes = data_plan.writes.len();
            extended_atlas_touched_sectors = data_plan.touched_sectors().len();
            extended_atlas_final_expected_writes =
                extended_atlas_data_writes + extended_atlas_touched_sectors;
            let output = apply_form1_write_plan(source.image(), &data_plan)
                .map_err(|source| error(source.to_string()))?;
            let final_image =
                DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
            let final_iso =
                Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
            let readback_vdd = final_iso
                .read_file(RPG_GRP_PATH)
                .map_err(|source| error(source.to_string()))?;
            if readback_vdd != new_vdd {
                return Err(error("QUEST extended atlas VDD 최종 readback 불일치"));
            }
            let readback_archive =
                VddArchive::parse(&readback_vdd).map_err(|source| error(source.to_string()))?;
            let readback_font = readback_archive
                .find(RPG_FONT_NAME)
                .ok_or_else(|| error("QUEST extended atlas font readback 없음"))?;
            if decompress(readback_archive.read_file(readback_font))
                .map_err(|source| error(source.to_string()))?
                != extended_plain
            {
                return Err(error("QUEST extended atlas font 최종 readback 불일치"));
            }
            extended_atlas_disc_readback = true;
            extended_atlas_final_image_sha256 = sha256(final_image.raw_bytes());
        }
        Err(VddError::RepackedArchiveDoesNotFit { .. }) => {}
        Err(source) => return Err(error(source.to_string())),
    }

    let (jamo_initials, jamo_vowels, jamo_finals) = jamo_counts(&batches.draft_hangul_repertoire);
    Ok(QuestFontAlternativesAnalysis {
        provisional_glyphs: rasters.len(),
        code_table_entries: consumer.code_table_entries,
        current_wide_slots: QUEST_FONT_WIDE_SLOTS,
        quest_bios_font_routes,
        batch_pages,
        max_batch_glyphs: capacity.max_batch_unique_hangul,
        all_batches_fit_current_page: all_fit,
        batch_page_plain_total: batch_pages * current_page_bytes,
        batch_page_cnx_min: batch_cnx_min,
        batch_page_cnx_max: batch_cnx_max,
        batch_page_cnx_total: batch_cnx_total,
        batch_pages_in_place_fit: pages_in_place,
        glyph_catalog_plain_bytes: catalog_plain.len(),
        glyph_catalog_cnx_bytes: catalog_cnx.len(),
        extended_atlas_width: QUEST_FONT_LAYOUT.width,
        extended_atlas_height: extended_atlas_height(rasters.len()),
        extended_atlas_plain_bytes: extended_plain.len(),
        extended_atlas_cnx_bytes: extended_cnx.len(),
        extended_atlas_in_place_fit,
        extended_atlas_vdd_repack_fit,
        extended_atlas_disc_readback,
        extended_atlas_data_writes,
        extended_atlas_touched_sectors,
        extended_atlas_final_expected_writes,
        extended_atlas_plain_sha256: sha256(&extended_plain),
        extended_atlas_cnx_sha256: sha256(&extended_cnx),
        extended_atlas_final_image_sha256,
        compact_physical_slots: (QUEST_FONT_LAYOUT.width / COMPACT_CELL_WIDTH)
            * (QUEST_FONT_LAYOUT.height / QUEST_FONT_CELL_HEIGHT),
        current_profile_compact_fit_glyphs: compact_bbox_fit(&rasters),
        jamo_initials,
        jamo_vowels,
        jamo_finals,
        jamo_components: jamo_initials + jamo_vowels + jamo_finals,
        fixed_slot_comparable_entries: capacity.fixed_slot_comparable_entries,
        fixed_slot_overflow_entries: capacity.fixed_slot_overflow_entries,
        fixed_slot_max_overrun: capacity.fixed_slot_max_overrun,
        build_eligible_entries: 0,
    })
}

macro_rules! row {
    (
        $out:expr,
        $analysis:expr,
        $method:expr,
        $layer:expr,
        $storage:expr,
        $consumer:expr,
        $lifecycle:expr,
        $verdict:expr,
        $metrics:expr,
        $notes:expr $(,)?
    ) => {
        writeln!(
            $out,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            QUEST_FONT_ALTERNATIVES_SCHEMA_VERSION,
            $method,
            $layer,
            $storage,
            $consumer,
            $lifecycle,
            $verdict,
            $metrics,
            $analysis.build_eligible_entries,
            $notes,
        )
        .expect("String write는 실패하지 않음");
    };
}

/// 대안 12개의 증거 경계와 0원칙 판정을 내보낸다.
pub fn render_quest_font_alternatives_tsv(analysis: &QuestFontAlternativesAnalysis) -> String {
    let mut out = String::from(
        "schema_version\tmethod\tlayer\tstorage_verdict\tconsumer_verdict\t\
         lifecycle_verdict\tzero_principle_verdict\tmetrics\tbuild_eligible\tnotes\n",
    );
    row!(
        &mut out,
        analysis,
        "current-static-atlas",
        "font",
        "current-shape-readback-pass",
        "insufficient-code-and-wide-slots",
        "static",
        "rejected",
        &format!(
            "demand={};code={};wide={}",
            analysis.provisional_glyphs, analysis.code_table_entries, analysis.current_wide_slots
        ),
        "전체 provisional 코퍼스를 현재 atlas에 1:1 상주시키지 못함",
    );
    row!(
        &mut out,
        analysis,
        "expanded-static-atlas",
        "font",
        if analysis.extended_atlas_disc_readback {
            "vdd-disc-readback-pass"
        } else if analysis.extended_atlas_vdd_repack_fit {
            "vdd-repack-pass"
        } else {
            "vdd-repack-overflow"
        },
        "current-256x256-consumer-open",
        "static-resource-growth-open",
        "not-admitted",
        &format!(
            "plain={};cnx={};in_place={};repack={};writes={};sectors={}",
            analysis.extended_atlas_plain_bytes,
            analysis.extended_atlas_cnx_bytes,
            analysis.extended_atlas_in_place_fit,
            analysis.extended_atlas_vdd_repack_fit,
            analysis.extended_atlas_data_writes,
            analysis.extended_atlas_touched_sectors,
        ),
        "현재 VDD 저장도 실패했고 cell 등록·VRAM 좌표·texture page 소비도 미증명",
    );
    row!(
        &mut out,
        analysis,
        "compact-6x16-static-atlas",
        "font",
        "physical-grid-only",
        "code-table-still-345",
        "static",
        "rejected-standalone",
        &format!(
            "physical_slots={};current_profile_bbox_fit={}/{}",
            analysis.compact_physical_slots,
            analysis.current_profile_compact_fit_glyphs,
            analysis.provisional_glyphs
        ),
        "물리 672칸만으로 코드표와 Maplestory Light 판독성을 해결하지 못함",
    );
    row!(
        &mut out,
        analysis,
        "scene-page-atlas",
        "font",
        "all-pages-compress-fit",
        "current-page-shape-compatible",
        "runtime-scene-binding-open",
        "candidate",
        &format!(
            "pages={};max_glyphs={};cnx_min={};cnx_max={};cnx_total={};fit={}/{}",
            analysis.batch_pages,
            analysis.max_batch_glyphs,
            analysis.batch_page_cnx_min,
            analysis.batch_page_cnx_max,
            analysis.batch_page_cnx_total,
            analysis.batch_pages_in_place_fit,
            analysis.batch_pages,
        ),
        "batch는 실제 동시 working set이 아니며 전환·대화 object 수명 증거가 필요",
    );
    row!(
        &mut out,
        analysis,
        "active-glyph-cache",
        "font",
        "catalog-storage-small",
        "lookup-and-upload-hook-required",
        "eviction-and-window-lifetime-open",
        "candidate",
        &format!(
            "catalog_plain={};catalog_cnx={};active_slots={};max_batch={}",
            analysis.glyph_catalog_plain_bytes,
            analysis.glyph_catalog_cnx_bytes,
            analysis.current_wide_slots,
            analysis.max_batch_glyphs,
        ),
        "기존 object renderer 뒤쪽 재사용 가능성이 있으나 miss upload와 생존 slot을 증명해야 함",
    );
    row!(
        &mut out,
        analysis,
        "bios-krom-port",
        "font",
        "external-font-asset-required",
        "quest-route-absent",
        "new-cache-and-upload-lifecycle",
        "rejected-direct-reuse",
        &format!("quest_bios_routes={}", analysis.quest_bios_font_routes),
        "와쿠뿌요 훅을 이식하면 QUEST에서는 새 공급·변환·upload 경로가 됨",
    );
    row!(
        &mut out,
        analysis,
        "compositional-jamo",
        "font",
        "component-count-small",
        "overlay-and-advance-hook-required",
        "clear-and-object-lifetime-open",
        "fallback-high-risk",
        &format!(
            "initials={};vowels={};finals={};components={}",
            analysis.jamo_initials,
            analysis.jamo_vowels,
            analysis.jamo_finals,
            analysis.jamo_components,
        ),
        "현재 1 code→1 cell object 경로에는 겹쳐 그리기·무진행 합성이 없음",
    );
    row!(
        &mut out,
        analysis,
        "baked-dialogue-images",
        "font",
        "per-line-assets-unmeasured",
        "text-control-path-bypassed",
        "state-and-dynamic-values-open",
        "rejected-general-path",
        &format!(
            "translation_streams=580;batch_pages={}",
            analysis.batch_pages
        ),
        "고정 그림은 control·페이지·동적 값과 전체 대사 소비를 대신하지 못함",
    );
    row!(
        &mut out,
        analysis,
        "fixed-length-text",
        "reinsertion",
        "no-growth",
        "current-consumer",
        "static",
        "insufficient-full-corpus",
        &format!(
            "overflow={}/{};max_overrun={}",
            analysis.fixed_slot_overflow_entries,
            analysis.fixed_slot_comparable_entries,
            analysis.fixed_slot_max_overrun,
        ),
        "placeholder 제외 provisional 압력이며 승인 번역의 최종 실패 수는 아님",
    );
    row!(
        &mut out,
        analysis,
        "seq-local-growth-and-relink",
        "reinsertion",
        "container-capacity-unmeasured",
        "text-consumer-compatible",
        "opaque-structure-and-internal-refs-open",
        "candidate",
        "documents=113;text_streams=605;opaque_chunks=711",
        "성장하는 모든 pointer·section·후속 위치 의존 구조를 먼저 닫아야 함",
    );
    row!(
        &mut out,
        analysis,
        "variable-width-layout",
        "reinsertion",
        "unchanged",
        "renderer-hook-required",
        "layout-and-clear-open",
        "not-byte-solution",
        &format!(
            "fixed_slot_overflow={}",
            analysis.fixed_slot_overflow_entries
        ),
        "픽셀 폭은 줄일 수 있어도 SEQ 저장 길이와 글리프 repertoire를 해결하지 않음",
    );
    row!(
        &mut out,
        analysis,
        "translation-shortening",
        "reinsertion",
        "unchanged",
        "current-consumer",
        "human-approval-required",
        "not-automatic-method",
        &format!(
            "provisional_glyphs={};overflow={}",
            analysis.provisional_glyphs, analysis.fixed_slot_overflow_entries
        ),
        "0원칙상 용어·힌트·말투·의미를 줄이는 자동 축약은 허용하지 않음",
    );
    out
}

/// 지원 원본의 대안 수치와 결정적 report를 검증한다.
pub fn verify_box_quest_font_alternatives(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
    capacity: &QuestCapacityPocAnalysis,
    consumer: &QuestFontConsumerSummary,
    quest_bios_font_routes: usize,
) -> Result<QuestFontAlternativesAnalysis, QuestFontAlternativesError> {
    let analysis =
        audit_quest_font_alternatives(source, batches, capacity, consumer, quest_bios_font_routes)?;
    let report = render_quest_font_alternatives_tsv(&analysis);
    if report.lines().count() != BOX_QUEST_FONT_ALTERNATIVES_METHOD_ROWS + 1 {
        return Err(error(format!(
            "QUEST font 대안 report 행 수 불일치: expected={}, actual={}",
            BOX_QUEST_FONT_ALTERNATIVES_METHOD_ROWS + 1,
            report.lines().count()
        )));
    }
    let columns = report
        .lines()
        .map(|line| line.split('\t').count())
        .collect::<BTreeSet<_>>();
    if columns.len() != 1 {
        return Err(error(format!(
            "QUEST font 대안 report 열 수 불일치: {columns:?}"
        )));
    }
    let actual = sha256(report.as_bytes());
    if actual != BOX_QUEST_FONT_ALTERNATIVES_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST font 대안 TSV SHA-256 불일치: \
             expected={BOX_QUEST_FONT_ALTERNATIVES_REPORT_SHA256}, actual={actual}; \
             pages={}/{}, page_cnx={}-{} total={}, catalog={}/{}, \
             expanded={}/{} in_place={} repack={} readback={}, compact_fit={}/{}, \
             jamo={}",
            analysis.batch_pages_in_place_fit,
            analysis.batch_pages,
            analysis.batch_page_cnx_min,
            analysis.batch_page_cnx_max,
            analysis.batch_page_cnx_total,
            analysis.glyph_catalog_plain_bytes,
            analysis.glyph_catalog_cnx_bytes,
            analysis.extended_atlas_plain_bytes,
            analysis.extended_atlas_cnx_bytes,
            analysis.extended_atlas_in_place_fit,
            analysis.extended_atlas_vdd_repack_fit,
            analysis.extended_atlas_disc_readback,
            analysis.current_profile_compact_fit_glyphs,
            analysis.provisional_glyphs,
            analysis.jamo_components,
        )));
    }
    Ok(analysis)
}

#[cfg(test)]
#[path = "quest_font_alternatives_tests.rs"]
mod tests;
