//! QUEST font 수용량·page 공급·dialogue plan report 명령.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::disasm::bios_font::verify_box_bios_font_routes;
use ps1_puyopuyo_box::disasm::quest_font::verify_box_quest_font_consumer;
use ps1_puyopuyo_box::disc::{verify_box_32m_reservation, Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::font::quest::{
    render_quest_poc_font_plan_tsv, QUEST_POC_WIDE_CANDIDATE_COUNT,
};
use ps1_puyopuyo_box::pipeline::quest_capacity_poc::{
    render_quest_capacity_poc_tsv, verify_box_quest_capacity_poc,
};
use ps1_puyopuyo_box::pipeline::quest_font_alternatives::{
    render_quest_font_alternatives_tsv, verify_box_quest_font_alternatives,
    BOX_QUEST_FONT_ALTERNATIVES_METHOD_ROWS,
};
use ps1_puyopuyo_box::pipeline::quest_page_carrier_plan::verify_quest_page_carrier_plan;
use ps1_puyopuyo_box::pipeline::quest_page_supply_plan::{
    render_quest_page_supply_plan_tsv, verify_box_quest_page_supply_plan,
    BOX_QUEST_PAGE_SUPPLY_PLAN_ROWS,
};
use ps1_puyopuyo_box::pipeline::quest_second_dialogue_plan::{
    render_quest_second_dialogue_plan_tsv, verify_box_quest_second_dialogue_plan,
    QuestSecondDialogueAssets, BOX_QUEST_SECOND_DIALOGUE_REPORT_ROWS,
};
use ps1_puyopuyo_box::pipeline::quest_source_repertoire::{
    render_quest_source_repertoire_tsv, verify_box_quest_source_repertoire,
};
use ps1_puyopuyo_box::script::quest_seq_text_extraction::verify_box_quest_seq_text_extraction;
use ps1_puyopuyo_box::script::quest_seq_translation_units::verify_box_quest_seq_translation_units;
use ps1_puyopuyo_box::script::quest_terminology::verify_box_quest_terminology_candidates;
use ps1_puyopuyo_box::script::quest_translation::{
    approved_quest_poc_translation, QUEST_DEFEAT_DIALOGUE_ID,
};
use ps1_puyopuyo_box::script::quest_translation_batches::verify_box_quest_translation_batches;
use ps1_puyopuyo_box::script::quest_translation_review::verify_box_quest_translation_reviews;

use super::analysis_inputs::read_quest_seq_sources;
use super::disc_commands::read_box_executables;
use super::quest_analysis_inputs::read_quest_seq_text_extraction_analysis;
use super::translation_reports::write_report_file;
use super::usage::usage;
use super::{
    quest_code_table_from_executable, read_quest_original_font, RPG_SEQ_PATH, TOWN00_NAME,
};

pub(super) fn write_quest_font_plan_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest_executable = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let code_table =
        quest_code_table_from_executable(quest_executable.map, &quest_executable.text)?;

    let vdd_raw = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|error| format!("{RPG_SEQ_PATH} VDD 파싱 실패: {error}"))?;
    let sources = read_quest_seq_sources(&archive)?;
    let town00 = sources
        .iter()
        .find(|(name, _)| name == TOWN00_NAME)
        .map(|(_, raw)| raw.as_slice())
        .ok_or_else(|| format!("{RPG_SEQ_PATH}:{TOWN00_NAME} 엔트리가 없음"))?;
    let observed_pairs = sources
        .iter()
        .flat_map(|(_, raw)| raw.windows(2).map(|pair| [pair[0], pair[1]]))
        .collect::<BTreeSet<_>>();
    let approved = approved_quest_poc_translation(town00, QUEST_DEFEAT_DIALOGUE_ID)
        .map_err(|error| error.to_string())?;
    let required_glyphs = approved
        .text
        .chars()
        .filter(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
        .collect::<BTreeSet<_>>();
    let report = render_quest_poc_font_plan_tsv(&code_table, &observed_pairs, &required_glyphs)
        .map_err(|error| error.to_string())?;
    write_report_file(
        output_path,
        "QUEST PoC font plan TSV",
        QUEST_POC_WIDE_CANDIDATE_COUNT,
        &report,
    )
}

pub(super) fn write_quest_capacity_poc_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let extraction = read_quest_seq_text_extraction_analysis(source_path)?;
    let translation_units =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let terminology = verify_box_quest_terminology_candidates(&translation_units)
        .map_err(|error| error.to_string())?;
    let batches =
        verify_box_quest_translation_batches(&extraction, &translation_units, &terminology)
            .map_err(|error| error.to_string())?;
    let analysis =
        verify_box_quest_capacity_poc(&source, &batches).map_err(|error| error.to_string())?;
    let report = render_quest_capacity_poc_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST glyph capacity and dense reinsertion stress TSV",
        analysis.batches.len() + 1,
        &report,
    )?;
    println!(
        "  provisional glyphs {} / code table {} / wide slots {} / safe slots {}, \
         shortfalls code {} / wide {} / safe {}, max batch {}:{} / stream {}:{}, \
         fixed-slot overflow {}/{}, dense font CNX {}→{}B / slot {}B, \
         storage {}, build-eligible {}",
        analysis.provisional_unique_hangul,
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
        analysis.fixed_slot_overflow_entries,
        analysis.fixed_slot_comparable_entries,
        analysis.font_cnx_before,
        analysis.stress_font_cnx_bytes,
        analysis.font_vdd_slot_capacity,
        analysis.storage_stress_verdict(),
        analysis.build_eligible_entries,
    );
    Ok(())
}

pub(super) fn write_quest_source_repertoire_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest_executable = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let code_table =
        quest_code_table_from_executable(quest_executable.map, &quest_executable.text)?;
    let vdd_raw = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|error| format!("{RPG_SEQ_PATH} VDD 파싱 실패: {error}"))?;
    let sources = read_quest_seq_sources(&archive)?;
    let extraction = verify_box_quest_seq_text_extraction(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &code_table,
    )
    .map_err(|error| error.to_string())?;
    let observed_pairs = sources
        .iter()
        .flat_map(|(_, raw)| raw.windows(2).map(|pair| [pair[0], pair[1]]))
        .collect::<BTreeSet<_>>();
    let analysis = verify_box_quest_source_repertoire(&code_table, &extraction, &observed_pairs)
        .map_err(|error| error.to_string())?;
    let report = render_quest_source_repertoire_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST source repertoire TSV",
        analysis.rows.len(),
        &report,
    )?;
    println!(
        "  source cells {} / canonical OP21 unique {} / occurrences {}, \
         selected kanji {} supplied / {} used, any-SEQ window observed {} / unobserved {}",
        analysis.rows.len(),
        analysis.canonical_unique_codes,
        analysis.canonical_glyph_occurrences,
        analysis.source_classes.selected_kanji,
        analysis.canonical_unique_classes.selected_kanji,
        analysis.sequence_window_observed_codes,
        analysis.sequence_window_unobserved_codes,
    );
    Ok(())
}

pub(super) fn write_quest_font_alternatives_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest_executable = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let consumer = verify_box_quest_font_consumer(quest_executable.map, &quest_executable.text)
        .map_err(|error| error.to_string())?;
    let bios =
        verify_box_bios_font_routes(executables.iter().map(|exe| (exe.map, exe.text.as_slice())))
            .map_err(|error| error.to_string())?;
    let quest_bios_routes = bios
        .routes
        .iter()
        .find(|route| route.exe == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE BIOS font route summary가 없음".to_string())?
        .stubs
        .len();
    let extraction = read_quest_seq_text_extraction_analysis(source_path)?;
    let translation_units =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let terminology = verify_box_quest_terminology_candidates(&translation_units)
        .map_err(|error| error.to_string())?;
    let batches =
        verify_box_quest_translation_batches(&extraction, &translation_units, &terminology)
            .map_err(|error| error.to_string())?;
    let capacity =
        verify_box_quest_capacity_poc(&source, &batches).map_err(|error| error.to_string())?;
    let analysis = verify_box_quest_font_alternatives(
        &source,
        &batches,
        &capacity,
        &consumer,
        quest_bios_routes,
    )
    .map_err(|error| error.to_string())?;
    let report = render_quest_font_alternatives_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST font and reinsertion alternatives TSV",
        BOX_QUEST_FONT_ALTERNATIVES_METHOD_ROWS,
        &report,
    )?;
    println!(
        "  batch pages {} / max {} / in-place {}/{}, CNX {}..{}B total {}B, \
         catalog {}/{}B, expanded {}/{}B in-place {} repack {} readback {}, \
         compact fit {}/{}, jamo {}, build-eligible {}",
        analysis.batch_pages,
        analysis.max_batch_glyphs,
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
        analysis.build_eligible_entries,
    );
    Ok(())
}

pub(super) fn write_quest_page_supply_plan_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let consumer = verify_box_quest_font_consumer(quest.map, &quest.text)
        .map_err(|error| error.to_string())?;
    let bios =
        verify_box_bios_font_routes(executables.iter().map(|exe| (exe.map, exe.text.as_slice())))
            .map_err(|error| error.to_string())?;
    let quest_bios_routes = bios
        .routes
        .iter()
        .find(|route| route.exe == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE BIOS font route summary가 없음".to_string())?
        .stubs
        .len();
    let extraction = read_quest_seq_text_extraction_analysis(source_path)?;
    let translation_units =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let terminology = verify_box_quest_terminology_candidates(&translation_units)
        .map_err(|error| error.to_string())?;
    let batches =
        verify_box_quest_translation_batches(&extraction, &translation_units, &terminology)
            .map_err(|error| error.to_string())?;
    let reviews =
        verify_box_quest_translation_reviews(&batches).map_err(|error| error.to_string())?;
    let capacity =
        verify_box_quest_capacity_poc(&source, &batches).map_err(|error| error.to_string())?;
    let alternatives = verify_box_quest_font_alternatives(
        &source,
        &batches,
        &capacity,
        &consumer,
        quest_bios_routes,
    )
    .map_err(|error| error.to_string())?;
    let reserved = verify_box_32m_reservation(
        &iso,
        source.image(),
        executables
            .iter()
            .map(|exe| (exe.map.name, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let carrier =
        verify_quest_page_carrier_plan(&iso, &reserved, quest.map, &quest.text, &alternatives)
            .map_err(|error| error.to_string())?;
    let analysis =
        verify_box_quest_page_supply_plan(&reviews, &capacity, &alternatives, carrier.carrier)
            .map_err(|error| error.to_string())?;
    let report = render_quest_page_supply_plan_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST next page supply plan TSV",
        BOX_QUEST_PAGE_SUPPLY_PLAN_ROWS,
        &report,
    )?;
    println!(
        "  target {} / review {}, target/batch Hangul {}/{}, two raw remaining {}B, \
         third raw shortfall {}B, CNX catalog+page remaining {}B, build-eligible {}",
        analysis.target_stream_id,
        analysis.target_review_status.label(),
        analysis.target_unique_hangul,
        analysis.draft_batch_unique_hangul,
        analysis.two_raw_pages_remaining,
        analysis.three_raw_pages_shortfall,
        analysis.cnx_catalog_active_page_remaining,
        analysis.build_eligible,
    );
    Ok(())
}

pub(super) fn read_verified_quest_second_dialogue_assets(
    source_path: &Path,
) -> Result<QuestSecondDialogueAssets, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let code_table = quest_code_table_from_executable(quest.map, &quest.text)?;
    let extraction = read_quest_seq_text_extraction_analysis(source_path)?;
    let translation_units =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let terminology = verify_box_quest_terminology_candidates(&translation_units)
        .map_err(|error| error.to_string())?;
    let batches =
        verify_box_quest_translation_batches(&extraction, &translation_units, &terminology)
            .map_err(|error| error.to_string())?;
    let reviews =
        verify_box_quest_translation_reviews(&batches).map_err(|error| error.to_string())?;
    let original_font = read_quest_original_font(&iso)?;
    verify_box_quest_second_dialogue_plan(&extraction, &reviews, &code_table, &original_font)
        .map_err(|error| error.to_string())
}

pub(super) fn write_quest_second_dialogue_plan_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let assets = read_verified_quest_second_dialogue_assets(source_path)?;
    let report = render_quest_second_dialogue_plan_tsv(&assets.plan);
    write_report_file(
        output_path,
        "QUEST second dialogue fixed-slot plan TSV",
        BOX_QUEST_SECOND_DIALOGUE_REPORT_ROWS,
        &report,
    )?;
    println!(
        "  lines used/capacity {:?}/{:?}, mappings second/shared/batch {}/{}/{}, stream {}, page {}, approved {}/{}, \
         runtime {}, build-eligible {}",
        assets
            .plan
            .lines
            .iter()
            .map(|line| line.used)
            .collect::<Vec<_>>(),
        assets
            .plan
            .lines
            .iter()
            .map(|line| line.capacity)
            .collect::<Vec<_>>(),
        assets.plan.mappings.len(),
        assets.plan.shared_mappings,
        assets.plan.batch_unique_mappings,
        assets.plan.replaced_stream_sha256,
        assets.plan.page_sha256,
        assets.plan.translation_approved,
        assets.plan.layout_approved,
        assets.plan.runtime_verified,
        assets.plan.build_eligible,
    );
    Ok(())
}
