//! QUEST development glyph cache와 font/page 공급 경계를 검증한다.

use ps1_puyopuyo_box::coords::ExeMap;
use ps1_puyopuyo_box::disasm::bios_font::BiosFontAnalysis;
use ps1_puyopuyo_box::disasm::quest_font::QuestFontConsumerSummary;
use ps1_puyopuyo_box::disc::reserved::Box32mSummary;
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::font::quest::QuestCodeTable;
use ps1_puyopuyo_box::pipeline::quest_capacity_poc::verify_box_quest_capacity_poc;
use ps1_puyopuyo_box::pipeline::quest_font_alternatives::verify_box_quest_font_alternatives;
use ps1_puyopuyo_box::pipeline::quest_glyph_cache_hook_plan::verify_box_quest_glyph_cache_hook_plan;
use ps1_puyopuyo_box::pipeline::quest_glyph_cache_package::verify_box_quest_glyph_cache_package;
use ps1_puyopuyo_box::pipeline::quest_page_carrier_plan::verify_quest_page_carrier_plan;
use ps1_puyopuyo_box::pipeline::quest_page_supply_plan::verify_box_quest_page_supply_plan;
use ps1_puyopuyo_box::pipeline::quest_second_dialogue_plan::verify_box_quest_second_dialogue_plan;
use ps1_puyopuyo_box::pipeline::quest_translation_routing::QuestTranslationRoutingAnalysis;
use ps1_puyopuyo_box::script::quest_seq_text_extraction::QuestSeqTextExtractionAnalysis;
use ps1_puyopuyo_box::script::quest_translation_batches::QuestTranslationBatchAnalysis;
use ps1_puyopuyo_box::script::quest_translation_review::QuestTranslationReviewAnalysis;

use crate::cli::read_quest_original_font;

pub(super) struct QuestGlyphPageSupplyAuditInputs<'a, 'image> {
    pub source: &'a VerifiedSourceImage,
    pub iso: &'a Iso9660<'image>,
    pub reserved: &'a Box32mSummary,
    pub executable_map: &'a ExeMap,
    pub executable_text: &'a [u8],
    pub bios_font: &'a BiosFontAnalysis,
    pub font_consumer: &'a QuestFontConsumerSummary,
    pub translation_batches: QuestTranslationBatchAnalysis,
    pub translation_reviews: QuestTranslationReviewAnalysis,
    pub translation_routing: QuestTranslationRoutingAnalysis,
    pub text_extraction: &'a QuestSeqTextExtractionAnalysis,
    pub code_table: &'a QuestCodeTable,
}

pub(super) fn audit_quest_glyph_page_supply(
    inputs: QuestGlyphPageSupplyAuditInputs<'_, '_>,
) -> Result<(), String> {
    let QuestGlyphPageSupplyAuditInputs {
        source,
        iso,
        reserved,
        executable_map: quest_map,
        executable_text: quest_text,
        bios_font,
        font_consumer: quest_font_consumer,
        translation_batches: quest_translation_batches,
        translation_reviews: quest_translation_reviews,
        translation_routing: quest_translation_routing,
        text_extraction: quest_seq_text_extraction,
        code_table,
    } = inputs;

    let quest_glyph_cache_package = verify_box_quest_glyph_cache_package(
        source,
        &quest_translation_batches,
        &quest_translation_routing,
    )
    .map_err(|error| error.to_string())?;
    let quest_glyph_cache = &quest_glyph_cache_package.plan;
    println!(
        "PASS QUEST random-access glyph cache draft: glyphs {}, pages {}, max page {}, \
         assignments {}, catalog {}/{}B, original atlas source {}/{}B, metadata \
         directory/assignments plain->delta/slots/padding/scratch {}/{}->{}/{}/{}/{}, package {}/{}B \
         (remaining {}B), aligned {}, runtime {}, build-eligible {}",
        quest_glyph_cache.glyphs,
        quest_glyph_cache.pages,
        quest_glyph_cache.max_page_glyphs,
        quest_glyph_cache.page_assignments,
        quest_glyph_cache.catalog.plain_bytes,
        quest_glyph_cache.catalog.stored_bytes,
        quest_glyph_cache.original_atlas_source.plain_bytes,
        quest_glyph_cache.original_atlas_source.stored_bytes,
        quest_glyph_cache.page_directory_bytes,
        quest_glyph_cache.page_assignment_plain_bytes,
        quest_glyph_cache.page_assignment_bytes,
        quest_glyph_cache.slot_rect_bytes,
        quest_glyph_cache.alignment_padding_bytes,
        quest_glyph_cache.scratch_bytes,
        quest_glyph_cache.package_bytes,
        quest_glyph_cache.carrier_bytes,
        quest_glyph_cache.carrier_remaining_bytes,
        quest_glyph_cache.halfword_aligned_slots,
        quest_glyph_cache.runtime_verified,
        quest_glyph_cache.build_eligible,
    );
    println!(
        "PASS QUEST sparse glyph cache package: {}B, SHA-256 {}, \
         sections/catalog/original-source/pages/rect/scratch {}/{}/{}/{}/{}/{}, runtime {}, \
         build-eligible {}",
        quest_glyph_cache_package.bytes.len(),
        quest_glyph_cache_package.sha256,
        quest_glyph_cache_package.sections_exact,
        quest_glyph_cache_package.catalog_readback_exact,
        quest_glyph_cache_package.original_atlas_source_roundtrip_exact,
        quest_glyph_cache_package.page_assignment_readback_exact,
        quest_glyph_cache_package.slot_rect_readback_exact,
        quest_glyph_cache_package.scratch_zero,
        quest_glyph_cache_package.runtime_verified,
        quest_glyph_cache_package.build_eligible,
    );
    let quest_glyph_cache_hook = verify_box_quest_glyph_cache_hook_plan(
        quest_map,
        quest_text,
        &quest_glyph_cache_package,
        &quest_translation_routing,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST sparse glyph cache typed hook plan: site 0x{:08X}, body 0x{:08X} \
         / {} words / {}B, package+code {}/{}B (remaining {}B), max uploads {}, \
         relocated routes {}, route/page/marked-scan/decode/original-atlas/non-OP21-atlas/\
         marker/object-select/marked-quad/scratch-sync/commit-sync/unpinned-bank/all-pinned-yield/\
         decode-fail-closed/load-delay {}/{}/{}/{}/{}/{}/{}/{}/{}/{}/{}/{}/{}/{}/{}, \
         package materialized {}, batch-upload runtime {}, \
         atlas-bank policy {}, runtime {}, build-eligible {}",
        quest_glyph_cache_hook.site.base_addr(),
        quest_glyph_cache_hook.body_runtime,
        quest_glyph_cache_hook.body.len_words(),
        quest_glyph_cache_hook.body_bytes,
        quest_glyph_cache_hook.carrier_used_bytes,
        quest_glyph_cache.carrier_bytes,
        quest_glyph_cache_hook.carrier_remaining_bytes,
        quest_glyph_cache_hook.max_page_uploads,
        quest_glyph_cache_hook.relocated_routes,
        quest_glyph_cache_hook.redirects_relocated_streams,
        quest_glyph_cache_hook.validates_page_ids,
        quest_glyph_cache_hook.scans_marked_font_objects,
        quest_glyph_cache_hook.decodes_independent_cells,
        quest_glyph_cache_hook.keeps_original_atlas_unchanged,
        quest_glyph_cache_hook.keeps_non_op21_objects_on_original_atlas,
        quest_glyph_cache_hook.marks_op21_objects_with_route_page,
        quest_glyph_cache_hook.selects_atlas_per_object,
        quest_glyph_cache_hook.routes_marked_objects_through_per_object_texture_page,
        quest_glyph_cache_hook.waits_for_draw_sync_before_scratch_reuse,
        quest_glyph_cache_hook.waits_for_draw_sync_before_commit,
        quest_glyph_cache_hook.selects_unpinned_atlas_bank,
        quest_glyph_cache_hook.yields_only_when_all_atlas_banks_pinned,
        quest_glyph_cache_hook.runtime_decode_failure_fail_closed,
        quest_glyph_cache_hook.r3000a_load_delays_safe,
        quest_glyph_cache_hook.package_materialized,
        quest_glyph_cache_hook.batched_upload_runtime_verified,
        quest_glyph_cache_hook.atlas_bank_policy_runtime_verified,
        quest_glyph_cache_hook.runtime_verified,
        quest_glyph_cache_hook.build_eligible,
    );
    let quest_capacity = verify_box_quest_capacity_poc(source, &quest_translation_batches)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST capacity PoC: provisional glyphs {} / code table {} / wide slots {} \
         (shortfall {}), max batch {}:{} / stream {}:{}, fixed-slot overflow {}/{}, \
         dense font CNX {}→{}B / slot {}B, storage {}, build-eligible {}",
        quest_capacity.provisional_unique_hangul,
        quest_capacity.code_table_entries,
        quest_capacity.current_wide_slots,
        quest_capacity.current_wide_shortfall,
        quest_capacity.max_batch_id,
        quest_capacity.max_batch_unique_hangul,
        quest_capacity.max_stream_id,
        quest_capacity.max_stream_unique_hangul,
        quest_capacity.fixed_slot_overflow_entries,
        quest_capacity.fixed_slot_comparable_entries,
        quest_capacity.font_cnx_before,
        quest_capacity.stress_font_cnx_bytes,
        quest_capacity.font_vdd_slot_capacity,
        quest_capacity.storage_stress_verdict(),
        quest_capacity.build_eligible_entries,
    );
    let quest_bios_font_routes = bios_font
        .routes
        .iter()
        .find(|route| route.exe == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE BIOS font route summary가 없음".to_string())?
        .stubs
        .len();
    let quest_font_alternatives = verify_box_quest_font_alternatives(
        source,
        &quest_translation_batches,
        &quest_capacity,
        quest_font_consumer,
        quest_bios_font_routes,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST font alternatives: batch pages {} / max {} glyphs / in-place {}/{}, \
         page CNX {}..{}B / total {}B, catalog {}/{}B, expanded {}x{} \
         {}/{}B / in-place {} / VDD repack {} / disc readback {}, \
         compact profile fit {}/{}, jamo components {}, build-eligible {}",
        quest_font_alternatives.batch_pages,
        quest_font_alternatives.max_batch_glyphs,
        quest_font_alternatives.batch_pages_in_place_fit,
        quest_font_alternatives.batch_pages,
        quest_font_alternatives.batch_page_cnx_min,
        quest_font_alternatives.batch_page_cnx_max,
        quest_font_alternatives.batch_page_cnx_total,
        quest_font_alternatives.glyph_catalog_plain_bytes,
        quest_font_alternatives.glyph_catalog_cnx_bytes,
        quest_font_alternatives.extended_atlas_width,
        quest_font_alternatives.extended_atlas_height,
        quest_font_alternatives.extended_atlas_plain_bytes,
        quest_font_alternatives.extended_atlas_cnx_bytes,
        quest_font_alternatives.extended_atlas_in_place_fit,
        quest_font_alternatives.extended_atlas_vdd_repack_fit,
        quest_font_alternatives.extended_atlas_disc_readback,
        quest_font_alternatives.current_profile_compact_fit_glyphs,
        quest_font_alternatives.provisional_glyphs,
        quest_font_alternatives.jamo_components,
        quest_font_alternatives.build_eligible_entries,
    );
    let quest_page_carrier = verify_quest_page_carrier_plan(
        iso,
        reserved,
        quest_map,
        quest_text,
        &quest_font_alternatives,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST page carrier candidate: schema {}, RAM 0x{:08X}..0x{:08X} / {}B, \
         search end 0x{:08X} / {}B, suffix first/ref {:#010X}/{}/{}, \
         clear alignment gap {}B, EXE extension padding {}B, candidate refs {}/{}, \
         page CNX {}..{}B, file {}→{}B / {}→{} sectors (growth {}B/{} sectors), \
         next extent LBA {}, tail/reservation {}/{}, relocate {}, ISO control {}, \
         runtime ownership {}, build-eligible {}",
        quest_page_carrier.schema_version,
        quest_page_carrier.carrier.start_runtime,
        quest_page_carrier.carrier.end_runtime,
        quest_page_carrier.carrier.capacity_bytes,
        quest_page_carrier.carrier.search_end_runtime,
        quest_page_carrier.carrier.search_capacity_bytes,
        quest_page_carrier.carrier.first_suffix_reference_runtime,
        quest_page_carrier.carrier.suffix_address_loads,
        quest_page_carrier.carrier.suffix_aligned_pointer_words,
        quest_page_carrier.carrier.alignment_gap_bytes,
        quest_page_carrier.carrier.extension_padding_bytes,
        quest_page_carrier.carrier.address_loads,
        quest_page_carrier.carrier.aligned_pointer_words,
        quest_page_carrier.page_cnx_min_bytes,
        quest_page_carrier.page_cnx_max_bytes,
        quest_page_carrier.original_file_bytes,
        quest_page_carrier.required_file_bytes,
        quest_page_carrier.original_sectors,
        quest_page_carrier.required_sectors,
        quest_page_carrier.growth_bytes,
        quest_page_carrier.growth_sectors,
        quest_page_carrier.next_extent_lba,
        quest_page_carrier.volume_tail_sectors,
        quest_page_carrier.reservation_sectors,
        quest_page_carrier.requires_iso_relocation,
        quest_page_carrier.iso_relocation_implemented,
        quest_page_carrier.runtime_ownership_proven,
        quest_page_carrier.build_eligible,
    );
    let quest_page_supply = verify_box_quest_page_supply_plan(
        &quest_translation_reviews,
        &quest_capacity,
        &quest_font_alternatives,
        quest_page_carrier.carrier,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST next page supply: target {} / review {}, target/batch Hangul {}/{}, \
         active slots {}, two raw pages {}/{}B (remaining {}B), third raw shortfall {}B, \
         CNX catalog+page remaining {}B, translation approved {}, build-eligible {}",
        quest_page_supply.target_stream_id,
        quest_page_supply.target_review_status.label(),
        quest_page_supply.target_unique_hangul,
        quest_page_supply.draft_batch_unique_hangul,
        quest_page_supply.active_wide_slots,
        quest_page_supply.two_raw_pages_bytes,
        quest_page_supply.carrier_capacity_bytes,
        quest_page_supply.two_raw_pages_remaining,
        quest_page_supply.three_raw_pages_shortfall,
        quest_page_supply.cnx_catalog_active_page_remaining,
        quest_page_supply.target_translation_approved,
        quest_page_supply.build_eligible,
    );
    let quest_original_font = read_quest_original_font(iso)?;
    let quest_second_dialogue = verify_box_quest_second_dialogue_plan(
        quest_seq_text_extraction,
        &quest_translation_reviews,
        code_table,
        &quest_original_font,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST second dialogue fixed-slot plan: lines {:?}/{:?} + padding {:?}, \
         glyph mappings second/shared/batch {}/{}/{}, stream {}B / {}, page {}B / {}, approved {}/{}, \
         runtime {}, build-eligible {}",
        quest_second_dialogue
            .plan
            .lines
            .iter()
            .map(|line| line.used)
            .collect::<Vec<_>>(),
        quest_second_dialogue
            .plan
            .lines
            .iter()
            .map(|line| line.capacity)
            .collect::<Vec<_>>(),
        quest_second_dialogue
            .plan
            .lines
            .iter()
            .map(|line| line.padding)
            .collect::<Vec<_>>(),
        quest_second_dialogue.plan.mappings.len(),
        quest_second_dialogue.plan.shared_mappings,
        quest_second_dialogue.plan.batch_unique_mappings,
        quest_second_dialogue.plan.replacement_stream_bytes,
        quest_second_dialogue.plan.replaced_stream_sha256,
        quest_second_dialogue.plan.page_bytes,
        quest_second_dialogue.plan.page_sha256,
        quest_second_dialogue.plan.translation_approved,
        quest_second_dialogue.plan.layout_approved,
        quest_second_dialogue.plan.runtime_verified,
        quest_second_dialogue.plan.build_eligible,
    );
    Ok(())
}
