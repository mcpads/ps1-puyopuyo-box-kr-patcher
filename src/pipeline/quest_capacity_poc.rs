//! QUEST provisional 번역 코퍼스의 글리프 수용량과 재삽입 스트레스 PoC.
//!
//! 사람 승인 전 초벌 580개를 제품 입력으로 쓰지 않고 수요 위험 신호로만
//! 집계한다. 현재 256×256 atlas의 326개 wide slot을 실제 코퍼스에서
//! 획이 가장 많은 326자로 채운 뒤 CNX→VDD→Mode2/2352→readback을
//! 메모리 안에서 수행한다. 이 산출물은 원문 글리프를 보존하지 않으므로
//! 저장·컨테이너 스트레스 증거일 뿐 실행 가능한 한글 패치가 아니다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::btx::{decode, encode};
use crate::compress::{compress, decompress, parse_header};
use crate::disc::{apply_form1_write_plan, DiscImage, Iso9660, VerifiedSourceImage};
use crate::font::{
    bundled_maplestory_light, rasterize_wide_glyph, QuestGlyphSlot, QUEST_CODE_TABLE_COUNT,
    QUEST_FONT_CELL_HEIGHT, QUEST_FONT_LAYOUT, QUEST_FONT_SHA256, QUEST_FONT_WIDE_CELL_WIDTH,
    QUEST_FONT_WIDE_COLUMNS, QUEST_FONT_WIDE_SLOTS, QUEST_POC_RASTER_PROFILE,
    QUEST_POC_WIDE_CANDIDATE_COUNT,
};
use crate::patch::WritePlan;
use crate::script::{
    QuestTranslationBatchAnalysis, QuestTranslationBatchRole, QuestTranslationWorkStatus,
};

use super::vdd_plan::{add_vdd_entry_replacement, logical_file};

pub(super) const RPG_GRP_PATH: &str = "/QUEST/RPG_GRP.VDD;1";
pub(super) const RPG_FONT_NAME: &str = "RPG_FNT1.BTX";

pub const QUEST_CAPACITY_POC_SCHEMA_VERSION: &str = "quest-capacity-poc-v2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestBatchCapacityRow {
    pub batch_id: String,
    pub translate_entries: usize,
    pub unique_hangul: usize,
    pub hangul_occurrences: usize,
    pub source_glyphs: usize,
    pub draft_scalars: usize,
    pub fixed_slot_comparable_entries: usize,
    pub fixed_slot_overflow_entries: usize,
    pub unresolved_term_entries: usize,
    pub max_fixed_slot_overrun: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestCapacityPocAnalysis {
    pub batches: Vec<QuestBatchCapacityRow>,
    pub provisional_unique_hangul: usize,
    pub provisional_hangul_occurrences: usize,
    pub rasterized_glyphs: usize,
    pub code_table_entries: usize,
    pub current_wide_slots: usize,
    pub provisional_safe_wide_slots: usize,
    pub code_table_shortfall: usize,
    pub current_wide_shortfall: usize,
    pub provisional_safe_shortfall: usize,
    pub max_batch_unique_hangul: usize,
    pub max_batch_id: String,
    pub max_stream_unique_hangul: usize,
    pub max_stream_id: String,
    pub fixed_slot_comparable_entries: usize,
    pub fixed_slot_overflow_entries: usize,
    pub fixed_slot_max_overrun: usize,
    pub fixed_slot_max_overrun_stream_id: String,
    pub unresolved_term_entries: usize,
    pub theoretical_atlas_height: usize,
    pub theoretical_atlas_plain_bytes: usize,
    pub stress_assigned_glyphs: usize,
    pub stress_omitted_glyphs: usize,
    pub stress_nonzero_pixels: usize,
    pub font_plain_bytes: usize,
    pub font_cnx_before: usize,
    pub stress_font_cnx_bytes: usize,
    pub font_vdd_slot_capacity: usize,
    pub font_vdd_slot_remaining: usize,
    pub stress_in_place_fit: bool,
    pub stress_data_writes: usize,
    pub stress_touched_sectors: usize,
    pub stress_final_expected_writes: usize,
    pub stress_font_plain_sha256: String,
    pub stress_font_cnx_sha256: String,
    pub stress_final_image_sha256: String,
    pub build_eligible_entries: usize,
}

impl QuestCapacityPocAnalysis {
    pub const fn static_atlas_verdict(&self) -> &'static str {
        if self.current_wide_shortfall == 0 && self.code_table_shortfall == 0 {
            "fits-current-static-atlas"
        } else {
            "current-static-atlas-insufficient"
        }
    }

    pub const fn storage_stress_verdict(&self) -> &'static str {
        if self.stress_in_place_fit {
            "dense-current-shape-readback-pass"
        } else {
            "dense-current-shape-in-place-overflow"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestCapacityPocError(String);

impl fmt::Display for QuestCapacityPocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestCapacityPocError {}

fn error(message: impl Into<String>) -> QuestCapacityPocError {
    QuestCapacityPocError(message.into())
}

fn sha256(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn is_hangul(character: char) -> bool {
    matches!(character, '\u{ac00}'..='\u{d7a3}')
}

struct BatchMeasurements {
    rows: Vec<QuestBatchCapacityRow>,
    hangul_occurrences: usize,
    max_batch_unique_hangul: usize,
    max_batch_id: String,
    max_stream_unique_hangul: usize,
    max_stream_id: String,
    fixed_slot_comparable_entries: usize,
    fixed_slot_overflow_entries: usize,
    fixed_slot_max_overrun: usize,
    fixed_slot_max_overrun_stream_id: String,
    unresolved_term_entries: usize,
}

fn measure_batches(
    batches: &QuestTranslationBatchAnalysis,
) -> Result<BatchMeasurements, QuestCapacityPocError> {
    let mut grouped = BTreeMap::<&str, Vec<_>>::new();
    for entry in &batches.entries {
        if entry.role == QuestTranslationBatchRole::Translate {
            grouped.entry(&entry.batch_id).or_default().push(entry);
        }
    }
    let mut rows = Vec::with_capacity(grouped.len());
    let mut total_hangul_occurrences = 0usize;
    let mut max_batch_unique_hangul = 0usize;
    let mut max_batch_id = String::new();
    let mut max_stream_unique_hangul = 0usize;
    let mut max_stream_id = String::new();
    let mut total_comparable = 0usize;
    let mut total_overflow = 0usize;
    let mut max_overrun = 0usize;
    let mut max_overrun_stream_id = String::new();
    let mut total_unresolved_terms = 0usize;

    for (batch_id, entries) in grouped {
        let mut unique_hangul = BTreeSet::new();
        let mut hangul_occurrences = 0usize;
        let mut source_glyphs = 0usize;
        let mut draft_scalars = 0usize;
        let mut comparable = 0usize;
        let mut overflow = 0usize;
        let mut unresolved_terms = 0usize;
        let mut batch_max_overrun = 0usize;
        for entry in &entries {
            let stream_hangul = entry
                .korean_draft
                .chars()
                .filter(|character| is_hangul(*character))
                .collect::<BTreeSet<_>>();
            if stream_hangul.len() > max_stream_unique_hangul {
                max_stream_unique_hangul = stream_hangul.len();
                max_stream_id = entry.stream_id.clone();
            }
            unique_hangul.extend(stream_hangul);
            hangul_occurrences += entry.korean_draft.chars().filter(|c| is_hangul(*c)).count();
            source_glyphs += entry.protected_source.chars().count();
            draft_scalars += entry.korean_draft.chars().count();

            if entry.work_status == QuestTranslationWorkStatus::NeedsHumanTermDecision
                || entry.korean_draft.contains('{')
                || entry.korean_draft.contains('}')
            {
                unresolved_terms += 1;
                continue;
            }
            comparable += 1;
            let source = entry.protected_source.chars().count();
            let draft = entry.korean_draft.chars().count();
            let overrun = draft.saturating_sub(source);
            if overrun > 0 {
                overflow += 1;
            }
            if overrun > batch_max_overrun {
                batch_max_overrun = overrun;
            }
            if overrun > max_overrun {
                max_overrun = overrun;
                max_overrun_stream_id = entry.stream_id.clone();
            }
        }
        if unique_hangul.len() > max_batch_unique_hangul {
            max_batch_unique_hangul = unique_hangul.len();
            max_batch_id = batch_id.to_owned();
        }
        total_hangul_occurrences += hangul_occurrences;
        total_comparable += comparable;
        total_overflow += overflow;
        total_unresolved_terms += unresolved_terms;
        rows.push(QuestBatchCapacityRow {
            batch_id: batch_id.to_owned(),
            translate_entries: entries.len(),
            unique_hangul: unique_hangul.len(),
            hangul_occurrences,
            source_glyphs,
            draft_scalars,
            fixed_slot_comparable_entries: comparable,
            fixed_slot_overflow_entries: overflow,
            unresolved_term_entries: unresolved_terms,
            max_fixed_slot_overrun: batch_max_overrun,
        });
    }
    if rows.is_empty() {
        return Err(error("QUEST capacity PoC batch가 비어 있음"));
    }
    Ok(BatchMeasurements {
        rows,
        hangul_occurrences: total_hangul_occurrences,
        max_batch_unique_hangul,
        max_batch_id,
        max_stream_unique_hangul,
        max_stream_id,
        fixed_slot_comparable_entries: total_comparable,
        fixed_slot_overflow_entries: total_overflow,
        fixed_slot_max_overrun: max_overrun,
        fixed_slot_max_overrun_stream_id: max_overrun_stream_id,
        unresolved_term_entries: total_unresolved_terms,
    })
}

fn vdd_slot_capacity(
    archive: &VddArchive,
    entry_name: &str,
) -> Result<usize, QuestCapacityPocError> {
    let entry = archive
        .find(entry_name)
        .ok_or_else(|| error(format!("{RPG_GRP_PATH}:{entry_name} 없음")))?;
    let start = entry.byte_offset();
    let next_start = archive
        .entries()
        .iter()
        .filter(|other| other.byte_offset() > start)
        .map(|other| other.byte_offset())
        .min()
        .unwrap_or_else(|| archive.serialize_lossless().len());
    Ok(next_start.saturating_sub(start))
}

fn dense_stress_font(
    original: &[u8],
    repertoire: &str,
) -> Result<(Vec<u8>, usize, usize), QuestCapacityPocError> {
    let font = bundled_maplestory_light().map_err(|source| error(source.to_string()))?;
    let mut rasters = repertoire
        .chars()
        .map(|glyph| {
            let pixels = rasterize_wide_glyph(&font, glyph, QUEST_POC_RASTER_PROFILE)
                .map_err(|source| error(source.to_string()))?;
            let nonzero = pixels.iter().filter(|pixel| **pixel != 0).count();
            Ok((glyph, pixels, nonzero))
        })
        .collect::<Result<Vec<_>, QuestCapacityPocError>>()?;
    rasters.sort_by(|left, right| right.2.cmp(&left.2).then_with(|| left.0.cmp(&right.0)));

    let wide_slots = (0..QUEST_CODE_TABLE_COUNT)
        .filter_map(|index| {
            let slot = QuestGlyphSlot::for_index(index).ok()?;
            (slot.width == QUEST_FONT_WIDE_CELL_WIDTH).then_some(slot)
        })
        .collect::<Vec<_>>();
    if wide_slots.len() != QUEST_FONT_WIDE_SLOTS {
        return Err(error(format!(
            "QUEST wide slot 분모 불일치: expected={}, actual={}",
            QUEST_FONT_WIDE_SLOTS,
            wide_slots.len()
        )));
    }
    if rasters.len() < wide_slots.len() {
        return Err(error(format!(
            "QUEST stress glyph 부족: required={}, actual={}",
            wide_slots.len(),
            rasters.len()
        )));
    }

    let mut atlas =
        decode(original, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;
    let mut nonzero_pixels = 0usize;
    for (slot, (_, glyph_pixels, nonzero)) in wide_slots.iter().zip(rasters.iter()) {
        nonzero_pixels += nonzero;
        for row in 0..slot.height {
            let source_start = row * slot.width;
            let target_start = (slot.y + row) * QUEST_FONT_LAYOUT.width + slot.x;
            atlas[target_start..target_start + slot.width]
                .copy_from_slice(&glyph_pixels[source_start..source_start + slot.width]);
        }
    }
    let encoded = encode(&atlas, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;
    if decode(&encoded, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))? != atlas {
        return Err(error("QUEST dense stress font encode readback 불일치"));
    }
    Ok((encoded, rasters.len(), nonzero_pixels))
}

/// 초벌 코퍼스 수요와 현재형 최대 밀도 폰트의 저장→disc readback을 분석한다.
pub fn audit_quest_capacity_poc(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
) -> Result<QuestCapacityPocAnalysis, QuestCapacityPocError> {
    let provisional_unique_hangul = batches.draft_hangul_repertoire.chars().count();
    let batch_measurements = measure_batches(batches)?;

    let required_wide_rows = provisional_unique_hangul.div_ceil(QUEST_FONT_WIDE_COLUMNS);
    let theoretical_atlas_height = required_wide_rows * QUEST_FONT_CELL_HEIGHT;
    let theoretical_atlas_plain_bytes = QUEST_FONT_LAYOUT.width * theoretical_atlas_height / 2;

    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let grp_entry = iso
        .find(RPG_GRP_PATH)
        .map_err(|source| error(source.to_string()))?
        .clone();
    let grp_file = logical_file(&grp_entry).map_err(error)?;
    let grp_vdd = iso
        .read_file(RPG_GRP_PATH)
        .map_err(|source| error(source.to_string()))?;
    let grp_archive = VddArchive::parse(&grp_vdd).map_err(|source| error(source.to_string()))?;
    let font_entry = grp_archive
        .find(RPG_FONT_NAME)
        .ok_or_else(|| error("RPG_GRP.VDD에 RPG_FNT1.BTX가 없음"))?;
    let font_cnx = grp_archive.read_file(font_entry);
    let font_header = parse_header(font_cnx).map_err(|source| error(source.to_string()))?;
    if font_header.subtype != *b"bi_" {
        return Err(error("RPG_FNT1.BTX CNX subtype이 bi_가 아님"));
    }
    let font_plain = decompress(font_cnx).map_err(|source| error(source.to_string()))?;
    let actual_font_sha256 = sha256(&font_plain);
    if actual_font_sha256 != QUEST_FONT_SHA256 {
        return Err(error(format!(
            "RPG_FNT1.BTX SHA-256 불일치: expected={QUEST_FONT_SHA256}, \
             actual={actual_font_sha256}"
        )));
    }
    let (stress_font_plain, rasterized_glyphs, stress_nonzero_pixels) =
        dense_stress_font(&font_plain, &batches.draft_hangul_repertoire)?;
    let stress_font_cnx = compress(&stress_font_plain, font_header.subtype)
        .map_err(|source| error(source.to_string()))?;
    if decompress(&stress_font_cnx).map_err(|source| error(source.to_string()))?
        != stress_font_plain
    {
        return Err(error("QUEST dense stress font CNX readback 불일치"));
    }
    let font_vdd_slot_capacity = vdd_slot_capacity(&grp_archive, RPG_FONT_NAME)?;
    let stress_in_place_fit = stress_font_cnx.len() <= font_vdd_slot_capacity;
    let mut stress_data_writes = 0usize;
    let mut stress_touched_sectors = 0usize;
    let mut stress_final_expected_writes = 0usize;
    let mut stress_final_image_sha256 = String::new();
    if stress_in_place_fit {
        let new_grp_vdd = grp_archive
            .replace_in_place(&BTreeMap::from([(
                RPG_FONT_NAME.to_string(),
                stress_font_cnx.clone(),
            )]))
            .map_err(|source| error(source.to_string()))?;
        let mut data_plan = WritePlan::new();
        add_vdd_entry_replacement(
            &mut data_plan,
            grp_file,
            &grp_archive,
            &grp_vdd,
            &new_grp_vdd,
            RPG_FONT_NAME,
            stress_font_cnx.len(),
            "quest-capacity-font-stress",
            "pipeline::quest_capacity_poc",
        )
        .map_err(error)?;
        stress_data_writes = data_plan.writes.len();
        stress_touched_sectors = data_plan.touched_sectors().len();
        stress_final_expected_writes = stress_data_writes + stress_touched_sectors;
        let output = apply_form1_write_plan(source.image(), &data_plan)
            .map_err(|source| error(source.to_string()))?;
        let final_image =
            DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
        let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
        let readback_vdd = final_iso
            .read_file(RPG_GRP_PATH)
            .map_err(|source| error(source.to_string()))?;
        if readback_vdd != new_grp_vdd {
            return Err(error("QUEST dense stress RPG_GRP.VDD readback 불일치"));
        }
        let readback_archive =
            VddArchive::parse(&readback_vdd).map_err(|source| error(source.to_string()))?;
        let readback_entry = readback_archive
            .find(RPG_FONT_NAME)
            .ok_or_else(|| error("QUEST dense stress font readback 없음"))?;
        let readback_plain = decompress(readback_archive.read_file(readback_entry))
            .map_err(|source| error(source.to_string()))?;
        if readback_plain != stress_font_plain {
            return Err(error("QUEST dense stress font 최종 해제 readback 불일치"));
        }
        stress_final_image_sha256 = sha256(final_image.raw_bytes());
    }

    Ok(QuestCapacityPocAnalysis {
        batches: batch_measurements.rows,
        provisional_unique_hangul,
        provisional_hangul_occurrences: batch_measurements.hangul_occurrences,
        rasterized_glyphs,
        code_table_entries: QUEST_CODE_TABLE_COUNT,
        current_wide_slots: QUEST_FONT_WIDE_SLOTS,
        provisional_safe_wide_slots: QUEST_POC_WIDE_CANDIDATE_COUNT,
        code_table_shortfall: provisional_unique_hangul.saturating_sub(QUEST_CODE_TABLE_COUNT),
        current_wide_shortfall: provisional_unique_hangul.saturating_sub(QUEST_FONT_WIDE_SLOTS),
        provisional_safe_shortfall: provisional_unique_hangul
            .saturating_sub(QUEST_POC_WIDE_CANDIDATE_COUNT),
        max_batch_unique_hangul: batch_measurements.max_batch_unique_hangul,
        max_batch_id: batch_measurements.max_batch_id,
        max_stream_unique_hangul: batch_measurements.max_stream_unique_hangul,
        max_stream_id: batch_measurements.max_stream_id,
        fixed_slot_comparable_entries: batch_measurements.fixed_slot_comparable_entries,
        fixed_slot_overflow_entries: batch_measurements.fixed_slot_overflow_entries,
        fixed_slot_max_overrun: batch_measurements.fixed_slot_max_overrun,
        fixed_slot_max_overrun_stream_id: batch_measurements.fixed_slot_max_overrun_stream_id,
        unresolved_term_entries: batch_measurements.unresolved_term_entries,
        theoretical_atlas_height,
        theoretical_atlas_plain_bytes,
        stress_assigned_glyphs: QUEST_FONT_WIDE_SLOTS,
        stress_omitted_glyphs: provisional_unique_hangul.saturating_sub(QUEST_FONT_WIDE_SLOTS),
        stress_nonzero_pixels,
        font_plain_bytes: font_plain.len(),
        font_cnx_before: font_cnx.len(),
        stress_font_cnx_bytes: stress_font_cnx.len(),
        font_vdd_slot_capacity,
        font_vdd_slot_remaining: font_vdd_slot_capacity.saturating_sub(stress_font_cnx.len()),
        stress_in_place_fit,
        stress_data_writes,
        stress_touched_sectors,
        stress_final_expected_writes,
        stress_font_plain_sha256: sha256(&stress_font_plain),
        stress_font_cnx_sha256: sha256(&stress_font_cnx),
        stress_final_image_sha256,
        build_eligible_entries: 0,
    })
}

/// 고정 지원 원본의 수요·stress report 분모와 digest를 검증한다.
pub fn verify_box_quest_capacity_poc(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
) -> Result<QuestCapacityPocAnalysis, QuestCapacityPocError> {
    audit_quest_capacity_poc(source, batches)
}

/// 전체 요약 한 행과 111개 batch별 provisional 압력을 내보낸다.
pub fn render_quest_capacity_poc_tsv(analysis: &QuestCapacityPocAnalysis) -> String {
    let mut output = String::from(
        "schema_version\trow_kind\tbatch_id\ttranslate_entries\tunique_hangul\t\
         hangul_occurrences\tsource_glyphs\tdraft_scalars\tfixed_slot_comparable_entries\t\
         fixed_slot_overflow_entries\tunresolved_term_entries\tmax_fixed_slot_overrun\t\
         provisional_unique_hangul\trasterized_glyphs\tcode_table_entries\tcurrent_wide_slots\t\
         provisional_safe_wide_slots\tcode_table_shortfall\tcurrent_wide_shortfall\t\
         provisional_safe_shortfall\tmax_batch_id\tmax_batch_unique_hangul\tmax_stream_id\t\
         max_stream_unique_hangul\tfixed_slot_max_overrun_stream_id\tfixed_slot_max_overrun\t\
         theoretical_atlas_height\ttheoretical_atlas_plain_bytes\tstress_assigned_glyphs\t\
         stress_omitted_glyphs\tstress_nonzero_pixels\tfont_plain_bytes\tfont_cnx_before\t\
         stress_font_cnx_bytes\tfont_vdd_slot_capacity\tfont_vdd_slot_remaining\t\
         stress_in_place_fit\tstress_data_writes\tstress_touched_sectors\t\
         stress_final_expected_writes\tstress_font_plain_sha256\tstress_font_cnx_sha256\t\
         stress_final_image_sha256\tstatic_atlas_verdict\tstorage_stress_verdict\t\
         build_eligible\tnotes\n",
    );
    writeln!(
        output,
        "{}\tsummary\t\t{}\t{}\t{}\t\t\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
         {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
         {}\t{}\t{}\t{}\t{}\tno\t현재형 atlas 전체를 덮는 비배포 in-memory stress이며 코드표·실행 소비·승인을 증명하지 않음",
        QUEST_CAPACITY_POC_SCHEMA_VERSION,
        analysis
            .batches
            .iter()
            .map(|row| row.translate_entries)
            .sum::<usize>(),
        analysis.provisional_unique_hangul,
        analysis.provisional_hangul_occurrences,
        analysis.fixed_slot_comparable_entries,
        analysis.fixed_slot_overflow_entries,
        analysis.unresolved_term_entries,
        analysis.fixed_slot_max_overrun,
        analysis.provisional_unique_hangul,
        analysis.rasterized_glyphs,
        analysis.code_table_entries,
        analysis.current_wide_slots,
        analysis.provisional_safe_wide_slots,
        analysis.code_table_shortfall,
        analysis.current_wide_shortfall,
        analysis.provisional_safe_shortfall,
        analysis.max_batch_id,
        analysis.max_batch_unique_hangul,
        analysis.max_stream_id,
        analysis.max_stream_unique_hangul,
        analysis.fixed_slot_max_overrun_stream_id,
        analysis.fixed_slot_max_overrun,
        analysis.theoretical_atlas_height,
        analysis.theoretical_atlas_plain_bytes,
        analysis.stress_assigned_glyphs,
        analysis.stress_omitted_glyphs,
        analysis.stress_nonzero_pixels,
        analysis.font_plain_bytes,
        analysis.font_cnx_before,
        analysis.stress_font_cnx_bytes,
        analysis.font_vdd_slot_capacity,
        analysis.font_vdd_slot_remaining,
        analysis.stress_in_place_fit,
        analysis.stress_data_writes,
        analysis.stress_touched_sectors,
        analysis.stress_final_expected_writes,
        analysis.stress_font_plain_sha256,
        analysis.stress_font_cnx_sha256,
        analysis.stress_final_image_sha256,
        analysis.static_atlas_verdict(),
        analysis.storage_stress_verdict(),
    )
    .expect("String write는 실패하지 않음");
    for row in &analysis.batches {
        writeln!(
            output,
            "{}\tbatch\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\t\t\t\t\t\t\t\t\t\t\
             \t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\tno\t{}",
            QUEST_CAPACITY_POC_SCHEMA_VERSION,
            row.batch_id,
            row.translate_entries,
            row.unique_hangul,
            row.hangul_occurrences,
            row.source_glyphs,
            row.draft_scalars,
            row.fixed_slot_comparable_entries,
            row.fixed_slot_overflow_entries,
            row.unresolved_term_entries,
            row.max_fixed_slot_overrun,
            escape_tsv("초벌 기준 장면 집합 상한이며 동시 runtime working set은 미확정"),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_capacity_poc_tests.rs"]
mod tests;
