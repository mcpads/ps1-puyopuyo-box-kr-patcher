//! QUEST ITEM_DAT 수동 조판과 source-bound writer 입력을 검증한다.

use ps1_puyopuyo_box::analysis::quest_resident_text_units::QuestResidentTextUnitAnalysis;
use ps1_puyopuyo_box::analysis::{
    verify_box_quest_item_acquisition_text_units, verify_box_quest_ui_literal_units,
    QuestBattleNameTableAnalysis, QuestResidentEncounterTextAnalysis,
};
use ps1_puyopuyo_box::coords::ExeMap;
use ps1_puyopuyo_box::disc::VerifiedSourceImage;
use ps1_puyopuyo_box::font::quest::QuestCodeTable;
use ps1_puyopuyo_box::pipeline::quest_item_text_glyph_cache_development::{
    build_quest_item_text_glyph_cache_development, QuestItemTextGlyphCacheDevelopmentInput,
};
use ps1_puyopuyo_box::pipeline::quest_item_text_writer::audit_quest_item_text_writer;
use ps1_puyopuyo_box::pipeline::quest_manual_stream_writer::{
    verify_box_quest_manual_stream_writer_with_code_supply_and_layouts,
    QuestManualStreamCodeSupply, QuestManualStreamLayout,
};
use ps1_puyopuyo_box::pipeline::quest_translation_selection_routing::QuestTranslationSelectionRoutingAnalysis;
use ps1_puyopuyo_box::pipeline::{
    audit_quest_item_acquisition_text_compositions, audit_quest_item_acquisition_text_writer,
    audit_quest_ui_literal_compositions, audit_quest_ui_literal_writer,
    build_quest_encounter_text_development, QuestEncounterTextDevelopmentInput,
};
use ps1_puyopuyo_box::script::quest_item_text_units::QuestItemTextUnitAnalysis;
use ps1_puyopuyo_box::script::quest_seq_inventory::QuestSeqAddressAnalysis;
use ps1_puyopuyo_box::script::quest_seq_text_document::QuestSeqTextDocumentAnalysis;
use ps1_puyopuyo_box::script::quest_translation_selection::QuestTranslationSelectionAnalysis;

pub(super) struct QuestDevelopmentAuditInputs<'a> {
    pub source: &'a VerifiedSourceImage,
    pub executable_map: &'a ExeMap,
    pub executable_text: &'a [u8],
    pub item_sequence: &'a [u8],
    pub item_units: &'a QuestItemTextUnitAnalysis,
    pub name_drafts: &'a ps1_puyopuyo_box::script::QuestItemNameDraftAnalysis,
    pub description_drafts: &'a ps1_puyopuyo_box::script::QuestItemDescriptionDraftAnalysis,
    pub item_acquisition: &'a ps1_puyopuyo_box::disasm::QuestItemAcquisitionSummary,
    pub resident_units: &'a QuestResidentTextUnitAnalysis,
    pub battle_names: &'a QuestBattleNameTableAnalysis,
    pub resident_encounter_text: &'a QuestResidentEncounterTextAnalysis,
    pub selection: &'a QuestTranslationSelectionAnalysis,
    pub routing: &'a QuestTranslationSelectionRoutingAnalysis,
    pub documents: &'a QuestSeqTextDocumentAnalysis,
    pub addresses: &'a QuestSeqAddressAnalysis,
    pub layouts: &'a [QuestManualStreamLayout],
    pub page_packing: &'a ps1_puyopuyo_box::pipeline::QuestDevelopmentPagePackingAnalysis,
    pub code_table: &'a QuestCodeTable,
}

pub(super) fn audit_quest_development(
    inputs: QuestDevelopmentAuditInputs<'_>,
) -> Result<(), String> {
    let QuestDevelopmentAuditInputs {
        source,
        executable_map: quest_map,
        executable_text: quest_text,
        item_sequence,
        item_units: quest_item_text,
        name_drafts: quest_item_name_drafts,
        description_drafts: quest_item_description_drafts,
        item_acquisition,
        resident_units: quest_resident_text_units,
        battle_names: quest_battle_names,
        resident_encounter_text: quest_resident_encounter_text,
        selection: quest_development_selection,
        routing: quest_development_routing,
        documents: quest_seq_text_documents,
        addresses: quest_seq_addresses,
        layouts: quest_development_layouts,
        page_packing: quest_development_page_packing,
        code_table,
    } = inputs;
    let item_acquisition_units = verify_box_quest_item_acquisition_text_units(item_acquisition)
        .map_err(|error| error.to_string())?;
    let item_acquisition_compositions =
        audit_quest_item_acquisition_text_compositions(&item_acquisition_units)
            .map_err(|error| error.to_string())?;
    let ui_literals = ps1_puyopuyo_box::disasm::verify_box_quest_ui_literals(quest_map, quest_text)
        .map_err(|error| error.to_string())?;
    let ui_literal_units =
        verify_box_quest_ui_literal_units(&ui_literals).map_err(|error| error.to_string())?;
    let ui_literal_compositions = audit_quest_ui_literal_compositions(&ui_literal_units)
        .map_err(|error| error.to_string())?;
    let quest_item_text_development =
        build_quest_item_text_glyph_cache_development(QuestItemTextGlyphCacheDevelopmentInput {
            source,
            executable_map: quest_map,
            executable_text: quest_text,
            item_units: quest_item_text,
            drafts: quest_item_name_drafts,
            description_drafts: quest_item_description_drafts,
            item_acquisition_used_glyphs: &item_acquisition_compositions.used_glyphs,
            ui_literal_used_glyphs: &ui_literal_compositions.used_glyphs,
            resident_units: quest_resident_text_units,
            selection: quest_development_selection,
            routing: quest_development_routing,
            code_table,
        })
        .map_err(|error| error.to_string())?;
    let quest_encounter_text_development =
        build_quest_encounter_text_development(QuestEncounterTextDevelopmentInput {
            source,
            executable_map: quest_map,
            executable_text: quest_text,
            resident_units: quest_resident_text_units,
            encounter: quest_resident_encounter_text,
            battle_names: quest_battle_names,
            item_text: &quest_item_text_development,
            selection: quest_development_selection,
            routing: quest_development_routing,
            code_table,
        })
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST encounter common text page and writers: page {}, battle/encounter/shared/used/storage glyphs {}/{}/{}/{}/{}, capacity {}, \
         resident units/writes {}/{}, battle physical/unique/writes {}/{}/{}, name/encounter cells {}/{}, package/body/remaining {}/{}/{}B, \
         previous codes/source/NUL/control/terminal exact {}/{}/{}/{}/{}, translation/layout/runtime/product false/false/false/false",
        quest_encounter_text_development.page.page_id,
        quest_encounter_text_development
            .page
            .battle_name_used_glyphs
            .chars()
            .count(),
        quest_encounter_text_development
            .page
            .encounter_used_glyphs
            .chars()
            .count(),
        quest_encounter_text_development
            .page
            .shared_glyphs
            .chars()
            .count(),
        quest_encounter_text_development.page.used_glyphs.chars().count(),
        quest_encounter_text_development
            .page
            .storage_glyphs
            .chars()
            .count(),
        quest_encounter_text_development.page.dynamic_slot_capacity,
        quest_encounter_text_development.resident_text_writer.units,
        quest_encounter_text_development
            .resident_text_writer
            .writes
            .len(),
        quest_encounter_text_development
            .battle_name_writer
            .physical_names,
        quest_encounter_text_development
            .battle_name_writer
            .unique_sources,
        quest_encounter_text_development
            .battle_name_writer
            .writes
            .len(),
        quest_encounter_text_development
            .battle_name_writer
            .glyph_cells,
        quest_encounter_text_development
            .resident_text_writer
            .writes
            .last()
            .ok_or_else(|| "QUEST encounter resident write가 비었음".to_owned())?
            .glyph_cells,
        quest_encounter_text_development.package.bytes.len(),
        quest_encounter_text_development.hook.body_bytes,
        quest_encounter_text_development.hook.carrier_remaining_bytes,
        quest_encounter_text_development.previous_page_codes_exact,
        quest_encounter_text_development
            .battle_name_writer
            .source_exact,
        quest_encounter_text_development
            .battle_name_writer
            .nul_terminal_exact,
        quest_encounter_text_development
            .resident_text_writer
            .controls_exact,
        quest_encounter_text_development
            .resident_text_writer
            .terminal_exact,
    );
    println!(
            "PASS QUEST ITEM_DAT glyph cache development: name/description units {}/{}, \
             Korean name used/shared page glyphs {}/{}, combined used/storage/capacity \
             {}/{}/{}, single-page {}, item page used/storage glyphs {}/{}, \
             resident/item pages {}/{:?}, package/body/remaining {}/{}/{}B, item site/entry \
             0x{:08X}/0x{:08X}, translation/layout approved {}/{}, writer/runtime/product {}/{}/{}, \
             typed retry/marker/primary-only/direct-sites/name-whole-sites/name-scoped/\
             acquisition-whole-sites/acquisition-scoped/residency-sites \
            {}/{}/{}/{}/{}/{}/{}/{}/{}",
            quest_item_text_development.name_units,
            quest_item_text_development.description_units,
            quest_item_text_development.name_used_glyphs.chars().count(),
            quest_item_text_development
                .shared_item_page_glyphs
                .chars()
                .count(),
            quest_item_text_development
                .combined_item_text_used_glyphs
                .chars()
                .count(),
            quest_item_text_development
                .combined_item_text_storage_glyphs
                .chars()
                .count(),
            quest_item_text_development.single_page_dynamic_slot_capacity,
            quest_item_text_development.single_page_fits,
            quest_item_text_development
                .item_text_page_used_glyphs
                .iter()
                .map(|glyphs| glyphs.chars().count().to_string())
                .collect::<Vec<_>>()
                .join(","),
            quest_item_text_development
                .item_text_page_glyphs
                .iter()
                .map(|glyphs| glyphs.chars().count().to_string())
                .collect::<Vec<_>>()
                .join(","),
            quest_item_text_development.resident.page_id,
            quest_item_text_development.item_text_page_ids,
            quest_item_text_development.resident.package.bytes.len(),
            quest_item_text_development.hook.body_bytes,
            quest_item_text_development.hook.carrier_remaining_bytes,
            quest_item_text_development
                .hook
                .item_name_residency_site
                .as_ref()
                .ok_or_else(|| "QUEST item name residency site가 없음".to_string())?
                .base_addr(),
            quest_item_text_development
                .hook
                .item_name_entry_runtime
                .ok_or_else(|| "QUEST item name entry가 없음".to_string())?,
            quest_item_text_development.translation_approved,
            quest_item_text_development.layout_approved,
            quest_item_text_development.item_writer_linked,
            quest_item_text_development.runtime_verified,
            quest_item_text_development.product_eligible,
            quest_item_text_development
                .hook
                .retries_item_name_before_state_advance,
            quest_item_text_development
                .hook
                .marks_item_name_objects_with_page,
            quest_item_text_development
                .hook
                .routes_item_text_pages_only_to_primary_bank,
            quest_item_text_development
                .hook
                .item_name_direct_glyph_sites
                .len(),
            quest_item_text_development
                .hook
                .item_name_whole_string_sites
                .len(),
            quest_item_text_development
                .hook
                .scopes_item_name_whole_string_renderer_route,
            quest_item_text_development
                .hook
                .item_acquisition_whole_string_sites
                .len(),
            quest_item_text_development
                .hook
                .scopes_item_acquisition_fixed_message_renderer_route,
            quest_item_text_development
                .hook
                .item_description_residency_sites
                .len(),
        );
    let quest_item_name_streams =
        verify_box_quest_manual_stream_writer_with_code_supply_and_layouts(
            quest_seq_text_documents,
            quest_seq_addresses,
            quest_development_selection,
            quest_development_routing,
            QuestManualStreamCodeSupply {
                code_table,
                package: &quest_encounter_text_development.package,
                encoding: &quest_encounter_text_development.encoding,
            },
            quest_development_layouts,
        )
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST manual stream source-bound writer: sequences {}, selected/retained \
         {}/{}, explicit/inline/relocated {}/{}/{}, bytes {}→{} (changed {}), \
         retained/opaque/address/non-target/control exact {}/{}/{}/{}/{}, \
         translation/layout approved {}/{}; final disc readback belongs to build dev quest",
        quest_item_name_streams.sequences.len(),
        quest_item_name_streams.selected_routes,
        quest_item_name_streams.retained_source_routes,
        quest_item_name_streams.explicit_layout_routes,
        quest_item_name_streams.inline_routes,
        quest_item_name_streams.relocated_routes,
        quest_item_name_streams.source_bytes,
        quest_item_name_streams.output_bytes,
        quest_item_name_streams.changed_bytes,
        quest_item_name_streams.retained_source_exact,
        quest_item_name_streams.opaque_exact,
        quest_item_name_streams.internal_addresses_exact,
        quest_item_name_streams.non_target_exact,
        quest_item_name_streams.control_order_exact,
        quest_item_name_streams.translation_approved_routes,
        quest_item_name_streams.layout_approved_routes,
    );
    let default_item_text_page_id = quest_item_text_development
        .item_text_page_ids
        .first()
        .copied()
        .ok_or_else(|| "QUEST item text page가 비었음".to_owned())?;
    let quest_item_text_writer = audit_quest_item_text_writer(
        quest_map,
        quest_text,
        item_sequence,
        quest_item_text,
        quest_item_name_drafts,
        quest_item_description_drafts,
        default_item_text_page_id,
        &quest_item_text_development.description_page_assignments,
        &quest_encounter_text_development.encoding,
        &quest_encounter_text_development.package,
        code_table,
    )
    .map_err(|error| error.to_string())?;
    let quest_item_acquisition_text_writer = audit_quest_item_acquisition_text_writer(
        quest_map,
        quest_text,
        &item_acquisition_units,
        &item_acquisition_compositions,
        default_item_text_page_id,
        &quest_encounter_text_development.encoding,
        &quest_encounter_text_development.package,
        code_table,
    )
    .map_err(|error| error.to_string())?;
    let quest_ui_literal_writer = audit_quest_ui_literal_writer(
        quest_map,
        quest_text,
        &ui_literal_units,
        &ui_literal_compositions,
        default_item_text_page_id,
        &quest_encounter_text_development.encoding,
        &quest_encounter_text_development.package,
        code_table,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST ITEM_DAT manual text writer: name/description pages {}/{:?}, units {}/{}, \
             name/description cells {}/{}, name bytes {}→{}, description bytes {}→{}, \
             sequence {}→{}B, pointer writes {}, remaining {}B, \
             exact source/pointer/NUL/control/terminal/outside-relink {}/{}/{}/{}/{}/{}, \
             translation/layout/runtime/product {}/{}/{}/{}",
        quest_item_text_writer.name.page_id,
        quest_item_text_writer.description_page_ids,
        quest_item_text_writer.name_units,
        quest_item_text_writer.description_units,
        quest_item_text_writer.name.glyph_cells,
        quest_item_text_writer.description_glyph_cells,
        quest_item_text_writer.name.source_name_bytes,
        quest_item_text_writer.name.output_name_bytes,
        quest_item_text_writer.source_description_bytes,
        quest_item_text_writer.output_description_bytes,
        quest_item_text_writer.relink.control.source_bytes,
        quest_item_text_writer.relink.control.output_bytes,
        quest_item_text_writer.pointer_writes,
        quest_item_text_writer.active_window_remaining,
        quest_item_text_writer.source_exact,
        quest_item_text_writer.pointer_readback_exact,
        quest_item_text_writer.name.nul_terminal_exact,
        quest_item_text_writer.controls_exact,
        quest_item_text_writer.terminal_exact,
        quest_item_text_writer.source_exact_outside_relinked_fields,
        quest_item_text_writer.translation_approved,
        quest_item_text_writer.layout_approved,
        quest_item_text_writer.runtime_verified,
        quest_item_text_writer.product_eligible,
    );
    let quest_resident_writer = &quest_encounter_text_development.resident_text_writer;
    println!(
        "PASS QUEST resident source-bound writer: units/cells/writes {}/{}/{}, \
         source/control/terminal exact {}/{}/{}, status {}, \
         translation/layout/runtime/product {}/{}/{}/{}",
        quest_resident_writer.units,
        quest_resident_writer.glyph_cells,
        quest_resident_writer.writes.len(),
        quest_resident_writer.source_exact,
        quest_resident_writer.controls_exact,
        quest_resident_writer.terminal_exact,
        quest_resident_writer.status.label(),
        quest_resident_writer.translation_approved,
        quest_resident_writer.layout_approved,
        quest_resident_writer.runtime_verified,
        quest_resident_writer.product_eligible,
    );
    let quest_battle_name_writer = &quest_encounter_text_development.battle_name_writer;
    println!(
        "PASS QUEST battle name source-bound writer: page/physical/unique/cells/bytes \
         {}/{}/{}/{}/{}→{}, source/NUL exact {}/{}, review/choice {}/{}, \
         translation/layout/runtime/product {}/{}/{}/{}",
        quest_battle_name_writer.page_id,
        quest_battle_name_writer.physical_names,
        quest_battle_name_writer.unique_sources,
        quest_battle_name_writer.glyph_cells,
        quest_battle_name_writer.source_bytes,
        quest_battle_name_writer.output_bytes,
        quest_battle_name_writer.source_exact,
        quest_battle_name_writer.nul_terminal_exact,
        quest_battle_name_writer.selected_drafts,
        quest_battle_name_writer.needs_human_choice,
        quest_battle_name_writer.translation_approved,
        quest_battle_name_writer.layout_approved,
        quest_battle_name_writer.runtime_verified,
        quest_battle_name_writer.product_eligible,
    );
    println!(
        "PASS QUEST item acquisition source-bound writer: page/units/cells/bytes \
         {}/{}/{}/{}→{}, calls {}, source/NUL exact {}/{}, \
         translation/layout/runtime/product {}/{}/{}/{}",
        quest_item_acquisition_text_writer.page_id,
        quest_item_acquisition_text_writer.units,
        quest_item_acquisition_text_writer.glyph_cells,
        quest_item_acquisition_text_writer.source_bytes,
        quest_item_acquisition_text_writer.output_bytes,
        quest_item_acquisition_text_writer.writes.len(),
        quest_item_acquisition_text_writer.source_exact,
        quest_item_acquisition_text_writer.nul_terminal_exact,
        quest_item_acquisition_text_writer.translation_approved,
        quest_item_acquisition_text_writer.layout_approved,
        quest_item_acquisition_text_writer.runtime_verified,
        quest_item_acquisition_text_writer.product_eligible,
    );
    println!(
        "PASS QUEST world-map/shared-choice source-bound writer: page/units/map/choice/cells/bytes \
         {}/{}/{}/{}/{}/{}→{}, calls {}, source/NUL exact {}/{}, \
         translation/layout/runtime/product {}/{}/{}/{}",
        quest_ui_literal_writer.page_id,
        quest_ui_literal_writer.units,
        quest_ui_literal_writer.world_map_labels,
        quest_ui_literal_writer.binary_choices,
        quest_ui_literal_writer.glyph_cells,
        quest_ui_literal_writer.source_bytes,
        quest_ui_literal_writer.output_bytes,
        quest_ui_literal_writer.writes.len(),
        quest_ui_literal_writer.source_exact,
        quest_ui_literal_writer.nul_terminal_exact,
        quest_ui_literal_writer.translation_approved,
        quest_ui_literal_writer.layout_approved,
        quest_ui_literal_writer.runtime_verified,
        quest_ui_literal_writer.product_eligible,
    );
    println!(
        "PASS QUEST development page packing input: source/packed pages {}/{}, \
         max glyphs stream/source/packed {}/{}/{}, assignments source/packed {}/{}, \
         resident/item page glyphs {}/{:?}; final disc Expected Write/readback belongs to build dev quest",
        quest_development_page_packing.source_pages,
        quest_development_page_packing.packed_pages,
        quest_development_page_packing.max_stream_dynamic_glyphs,
        quest_development_page_packing.source_max_page_dynamic_glyphs,
        quest_development_page_packing.packed_max_page_dynamic_glyphs,
        quest_development_page_packing.source_page_assignments,
        quest_development_page_packing.packed_page_assignments,
        quest_item_text_development
            .resident
            .used_glyphs
            .chars()
            .count(),
        quest_item_text_development
            .item_text_page_glyphs
            .iter()
            .map(|glyphs| glyphs.chars().count())
            .collect::<Vec<_>>(),
    );

    Ok(())
}
