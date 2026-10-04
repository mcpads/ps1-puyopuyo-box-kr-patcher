//! QUEST SEQ 원문·ITEM_DAT·OP21 token 원장을 검증한다.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::compress::{decompress, is_cnx};
use ps1_puyopuyo_box::coords::ExeMap;
use ps1_puyopuyo_box::font::quest::QuestCodeTable;
use ps1_puyopuyo_box::pipeline::quest_item_text_relink::verify_box_quest_item_text_relink_plan;
use ps1_puyopuyo_box::pipeline::quest_source_repertoire::verify_box_quest_source_repertoire;
use ps1_puyopuyo_box::script::quest_item_description_drafts::{
    audit_quest_item_description_drafts, QuestItemDescriptionDraftAnalysis,
};
use ps1_puyopuyo_box::script::quest_item_name_drafts::{
    audit_quest_item_name_drafts, QuestItemNameDraftAnalysis,
};
use ps1_puyopuyo_box::script::quest_item_text_units::{
    verify_box_quest_item_text_units, QuestItemTextUnitAnalysis,
};
use ps1_puyopuyo_box::script::quest_seq::verify_town00_seq_roundtrip;
use ps1_puyopuyo_box::script::quest_seq_inventory::{
    audit_quest_seq_addresses, verify_box_quest_seq_addresses, QuestSeqAddressAnalysis,
};
use ps1_puyopuyo_box::script::quest_seq_text_candidates::verify_box_quest_seq_text_candidates;
use ps1_puyopuyo_box::script::quest_seq_text_document::{
    verify_box_quest_seq_text_documents, QuestSeqTextDocumentAnalysis,
};
use ps1_puyopuyo_box::script::quest_seq_text_extraction::{
    verify_box_quest_seq_text_extraction, QuestSeqTextExtractionAnalysis,
};
use ps1_puyopuyo_box::script::quest_seq_text_streams::verify_box_quest_seq_text_streams;
use ps1_puyopuyo_box::script::quest_text_candidates::verify_box_town00_text_candidates;
use ps1_puyopuyo_box::script::quest_translation::{
    render_quest_dialogue_review_tsv, verify_quest_dialogue_translation_manifest,
};

use crate::cli::{RPG_SEQ_PATH, TOWN00_NAME};

pub(super) struct QuestTextSourceAuditInputs<'a> {
    pub archive: &'a VddArchive,
    pub sequences: &'a [(String, Vec<u8>)],
    pub executable_map: &'a ExeMap,
    pub executable_text: &'a [u8],
    pub code_table: &'a QuestCodeTable,
}

pub(super) struct QuestTextSourceAudit<'a> {
    pub town00: Vec<u8>,
    pub seq_addresses: QuestSeqAddressAnalysis,
    pub item_sequence: &'a [u8],
    pub item_units: QuestItemTextUnitAnalysis,
    pub item_name_drafts: QuestItemNameDraftAnalysis,
    pub item_description_drafts: QuestItemDescriptionDraftAnalysis,
    pub text_extraction: QuestSeqTextExtractionAnalysis,
    pub text_documents: QuestSeqTextDocumentAnalysis,
}

pub(super) fn audit_quest_text_sources(
    inputs: QuestTextSourceAuditInputs<'_>,
) -> Result<QuestTextSourceAudit<'_>, String> {
    let QuestTextSourceAuditInputs {
        archive: rpg_seq,
        sequences: quest_seq_sources,
        executable_map: quest_map,
        executable_text: quest_text,
        code_table,
    } = inputs;

    let quest_seq_addresses = audit_quest_seq_addresses(
        quest_seq_sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    verify_box_quest_seq_addresses(&quest_seq_addresses).map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ address inventory: sequences {}, bytes {}, \
         address-bearing {}, fields {}, targets {}, pointer records {}, OP89 inner \
         relations {} fields / {} edges, runtime reads {}, active base observed/static \
         {}/{}, analysis SHA-256 {}",
        quest_seq_addresses.summaries.len(),
        quest_seq_addresses
            .summaries
            .iter()
            .map(|summary| summary.bytes)
            .sum::<usize>(),
        quest_seq_addresses
            .summaries
            .iter()
            .filter(|summary| summary.fields > 0)
            .count(),
        quest_seq_addresses.candidates.len(),
        quest_seq_addresses
            .summaries
            .iter()
            .map(|summary| summary.targets)
            .sum::<usize>(),
        quest_seq_addresses
            .summaries
            .iter()
            .map(|summary| summary.pointer_records.iter().sum::<usize>())
            .sum::<usize>(),
        quest_seq_addresses
            .summaries
            .iter()
            .map(|summary| summary.relational_pointer_fields)
            .sum::<usize>(),
        quest_seq_addresses
            .summaries
            .iter()
            .map(|summary| summary.relational_pointer_edges)
            .sum::<usize>(),
        quest_seq_addresses
            .summaries
            .iter()
            .map(|summary| summary.runtime_read_fields)
            .sum::<usize>(),
        quest_seq_addresses
            .summaries
            .iter()
            .filter(|summary| summary.runtime_base_evidence.runtime_observed())
            .count(),
        quest_seq_addresses
            .summaries
            .iter()
            .filter(|summary| !summary.runtime_base_evidence.runtime_observed())
            .count(),
        quest_seq_addresses
            .analysis_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    let town00_entry = rpg_seq
        .find(TOWN00_NAME)
        .ok_or_else(|| format!("{RPG_SEQ_PATH}:{TOWN00_NAME} 엔트리가 없음"))?;
    let town00_cnx = rpg_seq.read_file(town00_entry);
    if !is_cnx(town00_cnx) {
        return Err(format!("{RPG_SEQ_PATH}:{TOWN00_NAME}가 CNX v2가 아님"));
    }
    let town00 =
        decompress(town00_cnx).map_err(|error| format!("{RPG_SEQ_PATH}:{TOWN00_NAME}: {error}"))?;
    let quest_seq = verify_town00_seq_roundtrip(&town00).map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ source roundtrip: TOWN00 prefix {} records / {}B, \
         root OP03 0x{:X}->0x{:X}, OP0D frontier state word 0x{:02X}, opaque body {}B, \
         internal-address candidates {} fields / {} targets, \
         dialogue sample {} glyphs / {} controls",
        quest_seq.prefix_records,
        quest_seq.prefix_bytes,
        quest_seq.root_stream_offset,
        quest_seq.root_target_offset,
        quest_seq.root_frontier_state_word_index,
        quest_seq.opaque_body_bytes,
        quest_seq.internal_address_candidate_fields,
        quest_seq.internal_address_candidate_targets,
        quest_seq.sample_glyphs,
        quest_seq.sample_controls
    );
    let quest_dialogues =
        verify_quest_dialogue_translation_manifest(&town00).map_err(|error| error.to_string())?;
    let quest_dialogue_review =
        render_quest_dialogue_review_tsv(&town00).map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST dialogue translation ledger: observed Japanese {}, \
         PoC-approved {}, pending human translation {}, review SHA-256 {:x}",
        quest_dialogues.observed_japanese_samples,
        quest_dialogues.poc_approved,
        quest_dialogues.pending_human_translation,
        Sha256::digest(quest_dialogue_review.as_bytes()),
    );
    let town00_text = verify_box_town00_text_candidates(&town00, code_table)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS TOWN00 text candidates: candidates {}, runtime-observed spans {}, \
         static-only {}, source code units {}, unique source characters {}",
        town00_text.candidates.len(),
        town00_text.runtime_observed_spans,
        town00_text.static_only_candidates,
        town00_text.source_code_units,
        town00_text.unique_source_characters,
    );
    let quest_seq_text = verify_box_quest_seq_text_candidates(
        quest_seq_sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        code_table,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ text candidates: sequences {} / with candidates {}, \
         candidates {}, runtime-observed spans {}, static-only {}, source code units {}, \
         unique source characters {}",
        quest_seq_text.summaries.len(),
        quest_seq_text.sequences_with_candidates,
        quest_seq_text.candidates.len(),
        quest_seq_text.runtime_observed_spans,
        quest_seq_text.static_only_candidates,
        quest_seq_text.source_code_units,
        quest_seq_text.unique_source_characters,
    );
    let item_data = quest_seq_sources
        .iter()
        .find(|(name, _)| name == "ITEM_DAT.SEQ")
        .ok_or_else(|| "QUEST ITEM_DAT.SEQ 입력이 없음".to_string())?;
    let quest_item_text = verify_box_quest_item_text_units(&item_data.1, code_table)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST ITEM_DAT text units: table entries {} / records {}, units {} \
         (name {} / description {}), glyphs {} / Japanese {}, controls {}, \
         candidate runs {} = name {} + description {}, heuristic short names {}, \
         runtime-observed {}",
        quest_item_text.table_entries,
        quest_item_text.unique_records,
        quest_item_text.units.len(),
        quest_item_text.name_units,
        quest_item_text.description_units,
        quest_item_text.glyph_occurrences,
        quest_item_text.japanese_glyph_occurrences,
        quest_item_text.control_occurrences,
        quest_item_text.code_table_candidate_runs,
        quest_item_text.name_candidate_runs,
        quest_item_text.description_candidate_runs,
        quest_item_text.short_name_units,
        quest_item_text.runtime_observed_units,
    );
    let quest_item_name_drafts =
        audit_quest_item_name_drafts(&quest_item_text).map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST ITEM_DAT name manual drafts: entries {}, glyphs {}, independent-review {}, \
         human-choice {}, translation/layout/product approved {}/{}/{}, asset SHA-256 {}",
        quest_item_name_drafts.entries.len(),
        quest_item_name_drafts.used_glyphs.chars().count(),
        quest_item_name_drafts.draft_needs_independent_review,
        quest_item_name_drafts.draft_needs_human_choice,
        quest_item_name_drafts.translation_approved,
        quest_item_name_drafts.layout_approved,
        quest_item_name_drafts.product_eligible,
        quest_item_name_drafts.asset_sha256,
    );
    let quest_item_description_drafts =
        audit_quest_item_description_drafts(&quest_item_text).map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST ITEM_DAT description manual drafts: entries {}, glyphs {} / unique {}, \
         controls {}, manual FF04 {}, independent-review {}, human-choice {}, \
         translation/layout/product approved {}/{}/{}, asset SHA-256 {}",
        quest_item_description_drafts.entries.len(),
        quest_item_description_drafts.glyph_occurrences,
        quest_item_description_drafts.used_glyphs.chars().count(),
        quest_item_description_drafts.control_occurrences,
        quest_item_description_drafts.manual_line_breaks,
        quest_item_description_drafts.draft_needs_independent_review,
        quest_item_description_drafts.draft_needs_human_choice,
        quest_item_description_drafts.translation_approved,
        quest_item_description_drafts.layout_approved,
        quest_item_description_drafts.product_eligible,
        quest_item_description_drafts.asset_sha256,
    );
    let item_relink = verify_box_quest_item_text_relink_plan(
        quest_map,
        quest_text,
        &item_data.1,
        &quest_item_text,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST ITEM_DAT page-0 relink control: source/output {}→{}B, appended {}B, \
         active-window remaining {}B, pointer writes {}, names/descriptions {}/{}, \
         OP21 records {}, outside relink exact {}, constructor active store unchanged {} typed word",
        item_relink.control.source_bytes,
        item_relink.control.output_bytes,
        item_relink.control.appended_bytes,
        item_relink.control.active_window_remaining,
        item_relink.control.pointer_writes,
        item_relink.control.name_units,
        item_relink.control.description_units,
        item_relink.control.description_op21_records,
        item_relink.control.source_exact_outside_relinked_fields,
        item_relink.verified_active_store.len_words(),
    );
    let quest_seq_text_streams = verify_box_quest_seq_text_streams(
        quest_seq_sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        code_table,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ OP21 text streams: shapes {}, parseable {}, rejected {}, \
         exact rooted runs {} / code-table candidates {}, rooted code units {}, \
         unique source characters {}",
        quest_seq_text_streams.markers.len(),
        quest_seq_text_streams
            .markers
            .iter()
            .filter(|marker| marker.is_parseable())
            .count(),
        quest_seq_text_streams
            .markers
            .iter()
            .filter(|marker| !marker.is_parseable())
            .count(),
        quest_seq_text_streams.exact_code_table_candidate_matches,
        quest_seq_text_streams.code_table_run_candidates,
        quest_seq_text_streams.rooted_code_units,
        quest_seq_text_streams.unique_source_characters,
    );
    let quest_seq_text_extraction = verify_box_quest_seq_text_extraction(
        quest_seq_sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        code_table,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ token extraction: streams {}, tokens {} \
         (glyph {}, control {}), roundtrip bytes {}",
        quest_seq_text_extraction.streams.len(),
        quest_seq_text_extraction.token_occurrences,
        quest_seq_text_extraction.glyph_occurrences,
        quest_seq_text_extraction.control_occurrences,
        quest_seq_text_extraction.roundtrip_bytes,
    );
    let observed_quest_seq_pairs = quest_seq_sources
        .iter()
        .flat_map(|(_, raw)| raw.windows(2).map(|pair| [pair[0], pair[1]]))
        .collect::<BTreeSet<_>>();
    let quest_source_repertoire = verify_box_quest_source_repertoire(
        code_table,
        &quest_seq_text_extraction,
        &observed_quest_seq_pairs,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST source repertoire: code table {} cells \
         (digits {}, hiragana {}, katakana/length {}, selected kanji {}, \
         symbols/space {}, fullwidth Latin {}, custom pairs {}), canonical OP21 \
         unique {} / occurrences {}, any-SEQ windows observed {} / unobserved {}",
        quest_source_repertoire.rows.len(),
        quest_source_repertoire.source_classes.digits,
        quest_source_repertoire.source_classes.hiragana,
        quest_source_repertoire
            .source_classes
            .katakana_and_length_marks,
        quest_source_repertoire.source_classes.selected_kanji,
        quest_source_repertoire.source_classes.symbols_or_space,
        quest_source_repertoire.source_classes.fullwidth_latin,
        quest_source_repertoire.source_classes.custom_ascii_pairs,
        quest_source_repertoire.canonical_unique_codes,
        quest_source_repertoire.canonical_glyph_occurrences,
        quest_source_repertoire.sequence_window_observed_codes,
        quest_source_repertoire.sequence_window_unobserved_codes,
    );
    let quest_seq_text_documents = verify_box_quest_seq_text_documents(
        quest_seq_sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &quest_seq_text_extraction,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ text documents: sequences {}, chunks {} \
         (text {} / {}B, opaque {} / {}B), full roundtrip {}B",
        quest_seq_text_documents.documents.len(),
        quest_seq_text_documents.chunks,
        quest_seq_text_documents.text_stream_chunks,
        quest_seq_text_documents.text_stream_bytes,
        quest_seq_text_documents.opaque_chunks,
        quest_seq_text_documents.opaque_bytes,
        quest_seq_text_documents.source_bytes,
    );
    Ok(QuestTextSourceAudit {
        town00,
        seq_addresses: quest_seq_addresses,
        item_sequence: item_data.1.as_slice(),
        item_units: quest_item_text,
        item_name_drafts: quest_item_name_drafts,
        item_description_drafts: quest_item_description_drafts,
        text_extraction: quest_seq_text_extraction,
        text_documents: quest_seq_text_documents,
    })
}
