use super::*;
use crate::compress::compress;
use crate::pipeline::quest_battle_name_compositions::QuestBattleNameDecisionStatus;
use crate::pipeline::quest_manual_stream_writer::QuestManualSequenceWrite;
use crate::script::{QuestItemDescriptionDraftStatus, QuestItemNameDraftStatus};

const VDD_SECTOR_LEN: usize = 2048;

fn fixture_vdd(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let table_bytes = 4 + entries.len() * 24;
    let first_sector = table_bytes.div_ceil(VDD_SECTOR_LEN);
    let mut sectors = first_sector;
    let compressed = entries
        .iter()
        .map(|(_, plain)| compress(plain, *b"bin").unwrap())
        .collect::<Vec<_>>();
    for raw in &compressed {
        sectors += raw.len().div_ceil(VDD_SECTOR_LEN).max(1);
    }
    let mut output = vec![0; (sectors + 2) * VDD_SECTOR_LEN];
    output[..4].copy_from_slice(&(entries.len() as u32).to_le_bytes());
    let mut sector = first_sector;
    for (index, ((name, _), raw)) in entries.iter().zip(&compressed).enumerate() {
        let base = 4 + index * 24;
        output[base..base + name.len()].copy_from_slice(name.as_bytes());
        output[base + 16..base + 20].copy_from_slice(&(sector as u32).to_le_bytes());
        output[base + 20..base + 24].copy_from_slice(&(raw.len() as u32).to_le_bytes());
        let start = sector * VDD_SECTOR_LEN;
        output[start..start + raw.len()].copy_from_slice(raw);
        sector += raw.len().div_ceil(VDD_SECTOR_LEN).max(1);
    }
    output
}

fn sequence(name: &str, source: &[u8], output: &[u8], selected: usize) -> QuestManualSequenceWrite {
    QuestManualSequenceWrite {
        sequence: name.to_string(),
        source_bytes: source.len(),
        output: output.to_vec(),
        source_sha256: sha256(source),
        output_sha256: sha256(output),
        selected_routes: selected,
        retained_source_routes: 0,
        inline_routes: selected,
        relocated_routes: 0,
        tail_bytes: 0,
        changed_bytes: source
            .iter()
            .zip(output)
            .filter(|(left, right)| left != right)
            .count(),
    }
}

fn writer(sequences: Vec<QuestManualSequenceWrite>) -> QuestManualStreamWriterAnalysis {
    QuestManualStreamWriterAnalysis {
        selected_routes: sequences.iter().map(|row| row.selected_routes).sum(),
        retained_source_routes: 0,
        inline_routes: sequences.iter().map(|row| row.inline_routes).sum(),
        relocated_routes: 0,
        return_records: 0,
        source_bytes: sequences.iter().map(|row| row.source_bytes).sum(),
        output_bytes: sequences.iter().map(|row| row.output.len()).sum(),
        changed_bytes: sequences.iter().map(|row| row.changed_bytes).sum(),
        retained_source_exact: true,
        opaque_exact: true,
        internal_addresses_exact: true,
        non_target_exact: true,
        control_order_exact: true,
        explicit_layout_routes: 0,
        translation_approved_routes: 0,
        layout_approved_routes: 0,
        runtime_verified: false,
        product_eligible: false,
        sequences,
    }
}

fn resident_writer() -> QuestResidentTextWriterAnalysis {
    let source = vec![0x21, 0, 0, 0, 0x82, 0xA0, 0xFF, 0x03];
    let output = vec![0x21, 7, 0, 0, 0x90, 0x01, 0xFF, 0x03];
    let write = QuestResidentTextWrite {
        unit_id: "resident-fixture".to_owned(),
        record_runtime: 0x8008_0000,
        end_runtime: 0x8008_0008,
        record_text_offset: 0x70000,
        end_text_offset: 0x70008,
        page_id: 7,
        source_sha256: sha256(&source),
        output_sha256: sha256(&output),
        source,
        output,
        glyph_cells: 1,
        status: QuestResidentTextCompositionStatus::ManualDraftNeedsGeometry,
        source_exact: true,
        controls_exact: true,
        terminal_exact: true,
        japanese_glyphs_remaining: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    };
    super::super::quest_resident_text_writer::build_quest_resident_text_writer_fixture(vec![write])
}

fn battle_name_writer() -> QuestBattleNameWriterAnalysis {
    let source = vec![0x82, 0xA0, 0, 0, 0, 0];
    let output = vec![0x90, 0x01, 0, 0, 0, 0];
    let write = QuestBattleNameWrite {
        unit_id: "battle-name-fixture".to_owned(),
        index: 0,
        runtime: 0x8008_1000,
        end_runtime: 0x8008_1006,
        text_offset: 0x71000,
        page_id: 116,
        source_text: "あ".to_owned(),
        development_text: "가".to_owned(),
        source_sha256: sha256(&source),
        output_sha256: sha256(&output),
        source,
        output,
        glyph_cells: 1,
        slot_capacity_glyphs: 2,
        decision_status: QuestBattleNameDecisionStatus::DraftNeedsIndependentReview,
    };
    super::super::quest_battle_name_writer::build_quest_battle_name_writer_fixture(vec![write])
}

fn disc_report() -> QuestGlyphCacheDiscReport {
    QuestGlyphCacheDiscReport {
        schema_version: "fixture",
        package_sha256: "package".to_owned(),
        package_bytes: 1,
        site_runtime: 1,
        original_font_guard_site_runtime: 2,
        font_object_marker_site_runtime: 3,
        font_draw_descriptor_site_runtime: 4,
        font_draw_texture_page_site_runtime: 5,
        item_name_residency_site_runtime: None,
        item_text_mode_residency_site_runtime: None,
        item_description_residency_site_runtimes: Vec::new(),
        body_runtime: 2,
        body_bytes: 3,
        relocated_quest_lba: 4,
        relocated_quest_bytes: 5,
        relocated_quest_sectors: 6,
        relocated_32m_lba: 7,
        relocated_32m_bytes: 8,
        relocated_files: 0,
        relocated_file_bytes: 0,
        relocated_file_sectors: 0,
        archive_entry_patches: 0,
        archive_repacks: 0,
        machine_code_writes: 2,
        data_writes: 3,
        metadata_writes: 4,
        relocation_copy_writes: 5,
        touched_sectors: 6,
        final_expected_writes: 20,
        package_exact: true,
        site_exact: true,
        original_font_guard_site_exact: true,
        font_object_marker_site_exact: true,
        font_draw_descriptor_site_exact: true,
        font_draw_texture_page_site_exact: true,
        item_name_residency_site_exact: true,
        item_text_mode_residency_site_exact: true,
        item_description_residency_sites_exact: true,
        body_exact: true,
        supplemental_machine_code_sites: 0,
        supplemental_machine_code_exact: true,
        supplemental_data_patches: 0,
        supplemental_data_bytes: 0,
        supplemental_data_exact: true,
        resident_text_writes: 0,
        resident_text_bytes: 0,
        resident_text_exact: true,
        item_acquisition_text_writes: 0,
        item_acquisition_text_bytes: 0,
        item_acquisition_text_exact: true,
        original_text_except_site_exact: true,
        original_text_except_declared_writes_exact: true,
        extension_outside_payload_zero: true,
        original_quest_extent_unchanged: true,
        extent_overlaps: 0,
        runtime_verified: false,
        build_eligible: false,
        image_sha256: "image".to_owned(),
    }
}

fn development_report(
    resident_text: QuestDevelopmentDiscResidentTextReport,
) -> QuestDevelopmentDiscReport {
    QuestDevelopmentDiscReport {
        schema_version: "fixture",
        sequence_entries: 1,
        target_sequence_entries: 1,
        non_target_sequence_entries: 0,
        selected_routes: 1,
        retained_source_routes: 0,
        inline_routes: 1,
        relocated_routes: 0,
        source_plain_bytes: 1,
        output_plain_bytes: 1,
        source_target_cnx_bytes: 1,
        output_target_cnx_bytes: 1,
        seq_vdd_lba: 1,
        seq_vdd_bytes: 1,
        seq_vdd_sectors: 1,
        seq_vdd_sha256: "seq".to_owned(),
        seq_vdd_repacked: false,
        seq_vdd_relocated: false,
        seq_vdd_exact: true,
        target_plain_exact: true,
        target_cnx_roundtrip_exact: true,
        non_target_plain_exact: true,
        non_target_cnx_exact: true,
        original_font_archive_unchanged: true,
        explicit_layout_routes: 0,
        translation_approved_routes: 0,
        layout_approved_routes: 0,
        runtime_verified: false,
        product_eligible: false,
        resident_text,
        battle_names: battle_name_report(),
        item_names: item_name_report(),
        item_descriptions: item_description_report(),
        item_acquisition_text: item_acquisition_text_report(),
        ui_literals: ui_literal_report(),
        disc: disc_report(),
    }
}

fn battle_name_report() -> QuestDevelopmentDiscBattleNameReport {
    let mut report = prepare_battle_name_report(&battle_name_writer()).unwrap();
    report.final_exact = true;
    report
}

fn item_name_report() -> QuestDevelopmentDiscItemNameReport {
    let source = vec![0x82, 0xA0, 0, 0];
    let output = vec![0x90, 0x01, 0, 0];
    QuestDevelopmentDiscItemNameReport {
        writes: vec![QuestItemNameWrite {
            unit_id: "item-name-fixture".to_owned(),
            record_offset: 0x1BC,
            pointer_field_offset: 0x1C0,
            source_text: "あ".to_owned(),
            draft_korean: "가".to_owned(),
            source_sha256: sha256(&source),
            output_sha256: sha256(&output),
            source,
            output,
            glyph_cells: 1,
            relocated_text_offset: 0x4200,
            relocated_pointer: 0x8010_4200,
            status: QuestItemNameDraftStatus::DraftNeedsIndependentReview,
        }],
        page_id: 57,
        units: 1,
        glyph_cells: 1,
        source_name_bytes: 4,
        output_name_bytes: 4,
        source_sequence_bytes: 16_776,
        output_sequence_bytes: 29_120,
        source_sequence_sha256: "source-sequence".to_owned(),
        output_sequence_sha256: "output-sequence".to_owned(),
        active_window_remaining: 3_648,
        draft_needs_independent_review: 1,
        draft_needs_human_choice: 0,
        source_exact: true,
        pointer_readback_exact: true,
        nul_terminal_exact: true,
        combined_relink_exact: true,
        final_exact: true,
        japanese_glyphs_remaining: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    }
}

fn item_description_report() -> QuestDevelopmentDiscItemDescriptionReport {
    let source = vec![0x82, 0xA0, 0x1A, 0xFF, 0, 0, 0, 0];
    let output = vec![30, 0, 0x1A, 0xFF, 0, 0, 0, 0];
    QuestDevelopmentDiscItemDescriptionReport {
        writes: vec![QuestItemDescriptionWrite {
            unit_id: "item-description-fixture".to_owned(),
            record_offset: 0x1BC,
            pointer_field_offset: 0x1C4,
            page_id: 58,
            source_text: "あ".to_owned(),
            draft_annotated: "가{FF1A:1A FF 00 00 00 00}".to_owned(),
            draft_korean: "가".to_owned(),
            source_sha256: sha256(&source),
            output_sha256: sha256(&output),
            source,
            output,
            glyph_cells: 1,
            control_occurrences: 1,
            relocated_record_offset: 0x4300,
            relocated_text_offset: 0x4304,
            relocated_pointer: 0x8010_4300,
            status: QuestItemDescriptionDraftStatus::DraftNeedsIndependentReview,
        }],
        page_ids: vec![57, 58],
        units: 1,
        glyph_cells: 1,
        controls: 1,
        source_description_bytes: 8,
        output_description_bytes: 8,
        active_window_remaining: 3_648,
        draft_needs_independent_review: 1,
        draft_needs_human_choice: 0,
        source_exact: true,
        pointer_readback_exact: true,
        controls_exact: true,
        terminal_exact: true,
        final_exact: true,
        japanese_glyphs_remaining: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    }
}

fn item_acquisition_text_report() -> QuestDevelopmentDiscItemAcquisitionTextReport {
    let source = vec![0x82, 0xA0, 0, 0];
    let output = vec![0x90, 0x01, 0, 0];
    QuestDevelopmentDiscItemAcquisitionTextReport {
        writes: vec![QuestItemAcquisitionTextWrite {
            unit_id: "item-acquisition-fixture".to_owned(),
            role: crate::disasm::QuestItemAcquisitionMessageRole::CurrencyAcquiredSuffix,
            runtime: 0x8001_2210,
            text_offset: 0x2210,
            page_id: 57,
            source_text: "を".to_owned(),
            selected_text: "가".to_owned(),
            source_sha256: sha256(&source),
            output_sha256: sha256(&output),
            source,
            output,
            glyph_cells: 1,
            status: QuestItemAcquisitionTextCompositionStatus::ManualDraftNeedsRuntimeGeometry,
        }],
        page_id: 57,
        units: 1,
        glyph_cells: 1,
        source_bytes: 4,
        output_bytes: 4,
        status: QuestItemAcquisitionTextCompositionStatus::ManualDraftNeedsRuntimeGeometry,
        source_exact: true,
        nul_terminal_exact: true,
        final_exact: true,
        japanese_glyphs_remaining: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    }
}

fn ui_literal_report() -> QuestDevelopmentDiscUiLiteralReport {
    let roles = [
        crate::disasm::QuestUiLiteralRole::PlumberVillage,
        crate::disasm::QuestUiLiteralRole::SkeletonTCave,
        crate::disasm::QuestUiLiteralRole::NeighborTownBridge,
        crate::disasm::QuestUiLiteralRole::CherrybellTown,
        crate::disasm::QuestUiLiteralRole::HappinessTower,
        crate::disasm::QuestUiLiteralRole::HarpyForest,
        crate::disasm::QuestUiLiteralRole::MountainPassCave,
        crate::disasm::QuestUiLiteralRole::SevenleaseVillage,
        crate::disasm::QuestUiLiteralRole::ZohDaimaohRuins,
        crate::disasm::QuestUiLiteralRole::PakistaShop,
        crate::disasm::QuestUiLiteralRole::PararaShop,
        crate::disasm::QuestUiLiteralRole::SasorimanShop,
        crate::disasm::QuestUiLiteralRole::ChoiceYes,
        crate::disasm::QuestUiLiteralRole::ChoiceNo,
    ];
    let writes = roles
        .into_iter()
        .enumerate()
        .map(|(index, role)| {
            let source = vec![0x82, index as u8, 0, 0];
            let output = vec![0x90, index as u8, 0, 0];
            let runtime = 0x8001_3000 + index as u32 * 4;
            QuestUiLiteralWrite {
                unit_id: format!("ui-literal-{index}"),
                role,
                surface: role.surface(),
                runtime,
                end_runtime: runtime + 4,
                text_offset: 0x3000 + index * 4,
                page_id: 57,
                source_text: "原".to_owned(),
                selected_text: "가".to_owned(),
                source_sha256: sha256(&source),
                output_sha256: sha256(&output),
                source,
                output,
                glyph_cells: 1,
                status: QuestUiLiteralCompositionStatus::ManualDraftNeedsRuntimeGeometry,
            }
        })
        .collect::<Vec<_>>();
    QuestDevelopmentDiscUiLiteralReport {
        source_bytes: writes.iter().map(|write| write.source.len()).sum(),
        output_bytes: writes.iter().map(|write| write.output.len()).sum(),
        writes,
        page_id: 57,
        units: 14,
        world_map_labels: 12,
        binary_choices: 2,
        glyph_cells: 14,
        status: QuestUiLiteralCompositionStatus::ManualDraftNeedsRuntimeGeometry,
        source_exact: true,
        nul_terminal_exact: true,
        final_exact: true,
        japanese_glyphs_remaining: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    }
}

#[test]
fn 대상만_재압축하고_비대상_cnx는_byte_exact로_보존한다() {
    let source_a = b"original A";
    let source_b = b"original B";
    let original = fixture_vdd(&[("A.SEQ", source_a), ("B.SEQ", source_b)]);
    let writer = writer(vec![
        sequence("A.SEQ", source_a, b"manual Korean A", 1),
        sequence("B.SEQ", source_b, source_b, 0),
    ]);

    let prepared = prepare_development_seq_archive(&original, &writer, None).unwrap();
    assert_eq!(prepared.target_sequence_entries, 1);
    assert_eq!(prepared.non_target_sequence_entries, 1);
    assert!(!prepared.seq_vdd_repacked);
    assert!(!prepared.seq_vdd_relocated);
    assert!(prepared.target_plain_exact);
    assert!(prepared.target_cnx_roundtrip_exact);
    assert!(prepared.non_target_plain_exact);
    assert!(prepared.non_target_cnx_exact);
}

#[test]
fn item_dat은_manual_writer의_비대상출력대신_이름writer결과를_한번만쓴다() {
    let source_item = b"original ITEM_DAT";
    let original = fixture_vdd(&[
        (QUEST_ITEM_TEXT_SEQUENCE, source_item),
        ("OTHER.SEQ", b"other"),
    ]);
    let manual_writer = writer(vec![
        sequence(QUEST_ITEM_TEXT_SEQUENCE, source_item, source_item, 0),
        sequence("OTHER.SEQ", b"other", b"other", 0),
    ]);
    let item = super::super::quest_item_text_writer::build_quest_item_text_writer_archive_fixture(
        source_item,
        b"korean item names".to_vec(),
    );

    let prepared = prepare_development_seq_archive(&original, &manual_writer, Some(&item)).unwrap();
    assert_eq!(prepared.target_sequence_entries, 1);
    assert_eq!(prepared.non_target_sequence_entries, 1);
    let archive = VddArchive::parse(&prepared.replacement_vdd).unwrap();
    let entry = archive.find(QUEST_ITEM_TEXT_SEQUENCE).unwrap();
    assert_eq!(
        decompress(archive.read_file(entry)).unwrap(),
        b"korean item names"
    );
}

#[test]
fn item_dat이_manual_writer에서도바뀌면_두writer의충돌을거부한다() {
    let source_item = b"original ITEM_DAT";
    let original = fixture_vdd(&[(QUEST_ITEM_TEXT_SEQUENCE, source_item)]);
    let manual_writer = writer(vec![sequence(
        QUEST_ITEM_TEXT_SEQUENCE,
        source_item,
        b"other replacement",
        1,
    )]);
    let item = super::super::quest_item_text_writer::build_quest_item_text_writer_archive_fixture(
        source_item,
        b"korean item names".to_vec(),
    );

    assert!(prepare_development_seq_archive(&original, &manual_writer, Some(&item)).is_err());
}

#[test]
fn writer와_vdd_분모가_다르면_재패킹하지_않는다() {
    let original = fixture_vdd(&[("A.SEQ", b"A"), ("B.SEQ", b"B")]);
    let writer = writer(vec![sequence("A.SEQ", b"A", b"K", 1)]);
    assert!(prepare_development_seq_archive(&original, &writer, None).is_err());
}

#[test]
fn 고정_vdd를_넘으면_원본을_보존한_확장_container를_만든다() {
    let source = b"source";
    let original = fixture_vdd(&[("A.SEQ", source)]);
    let mut state = 0x1234_5678u32;
    let replacement = (0..20_000)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect::<Vec<_>>();
    let writer = writer(vec![sequence("A.SEQ", source, &replacement, 1)]);

    let prepared = prepare_development_seq_archive(&original, &writer, None).unwrap();
    assert!(prepared.seq_vdd_repacked);
    assert!(prepared.seq_vdd_relocated);
    assert!(prepared.replacement_vdd.len() > original.len());
    assert!(prepared.target_plain_exact);
}

#[test]
fn resident_report는_수동writer의_source와final_span을보존한다() {
    let writer = resident_writer();
    let report = prepare_resident_text_report(&writer).unwrap();
    assert_eq!(report.units, 1);
    assert_eq!(report.glyph_cells, 1);
    assert_eq!(report.source_bytes, 8);
    assert_eq!(report.output_bytes, 8);
    assert_eq!(report.writes[0].page_id, 7);
    assert_eq!(report.writes[0].source, writer.writes[0].source);
    assert_eq!(report.writes[0].output, writer.writes[0].output);
    assert!(!report.final_exact);
    assert!(!report.translation_approved);
    assert!(!report.layout_approved);
    assert!(!report.runtime_verified);
    assert!(!report.product_eligible);
}

#[test]
fn resident_report는_분모drift와자동승격을거부한다() {
    let mut writer = resident_writer();
    writer.units += 1;
    let error = prepare_resident_text_report(&writer).unwrap_err();
    assert!(error.to_string().contains("provenance 불일치"));

    let mut writer = resident_writer();
    writer.translation_approved = true;
    let error = prepare_resident_text_report(&writer).unwrap_err();
    assert!(error.to_string().contains("provenance 불일치"));
}

#[test]
fn battle_name_report는_물리slot과_사람판정경계를_분리한다() {
    let report = prepare_battle_name_report(&battle_name_writer()).unwrap();
    assert_eq!(report.page_id, 116);
    assert_eq!(report.physical_names, 1);
    assert_eq!(report.unique_sources, 1);
    assert_eq!(report.selected_drafts, 1);
    assert_eq!(report.needs_human_choice, 0);
    assert_eq!(report.source_bytes, report.output_bytes);
    assert!(!report.final_exact);
    assert!(!report.translation_approved);
    assert!(!report.layout_approved);
    assert!(!report.runtime_verified);
    assert!(!report.product_eligible);
}

#[test]
fn development_tsv는_수동resident와_item_text_결속을_항상_기록한다() {
    let mut resident = prepare_resident_text_report(&resident_writer()).unwrap();
    resident.final_exact = true;
    let report = development_report(resident);
    let rendered = render_quest_development_disc_tsv(&report);
    let mut lines = rendered.lines();
    let header = lines.next().unwrap();
    let row = lines.next().unwrap();
    assert!(header.contains("resident_page_ids"));
    assert!(header.contains("resident_source_sha256"));
    assert!(header.contains("resident_output_sha256"));
    assert!(header.contains("battle_name_physical_names"));
    assert!(header.contains("battle_name_output_sha256"));
    assert!(header.contains("item_name_page_id"));
    assert!(header.contains("item_acquisition_runtimes"));
    assert!(header.contains("item_acquisition_output_sha256"));
    assert!(header.contains("ui_literal_world_map_labels"));
    assert!(header.contains("ui_literal_output_sha256"));
    assert!(row.contains("manual-draft-needs-geometry"));
    assert!(row.contains("\t7\t0x80080000\t"));
    assert!(row.contains(&resident_writer().writes[0].source_sha256));
    assert!(row.contains(&resident_writer().writes[0].output_sha256));
    assert!(row.contains(&battle_name_writer().writes[0].output_sha256));
    assert!(lines.next().is_none());
}

#[test]
fn item_text_development_tsv는_이름설명writer와_승인경계를_노출한다() {
    let mut resident = prepare_resident_text_report(&resident_writer()).unwrap();
    resident.final_exact = true;
    let report = development_report(resident);

    let rendered = render_quest_development_disc_tsv(&report);
    let mut lines = rendered.lines();
    let header = lines.next().unwrap();
    let row = lines.next().unwrap();
    assert!(header.contains("item_name_page_id"));
    assert!(header.contains("battle_name_needs_human_choice"));
    assert!(header.contains("battle_name_final_exact"));
    assert!(header.contains("item_name_pointer_readback_exact"));
    assert!(header.contains("item_name_runtime_verified"));
    assert!(header.contains("item_description_page_ids"));
    assert!(header.contains("item_description_controls_exact"));
    assert!(header.contains("item_description_runtime_verified"));
    assert!(header.contains("item_acquisition_nul_terminal_exact"));
    assert!(header.contains("item_acquisition_runtime_verified"));
    assert!(header.contains("ui_literal_nul_terminal_exact"));
    assert!(header.contains("ui_literal_runtime_verified"));
    assert!(row.contains("\t57\t1\t1\t4\t4\t16776\t29120\t"));
    assert!(row.contains("\t57,58\t1\t1\t1\t8\t8\t3648\t1\t0\ttrue\ttrue\ttrue\ttrue\ttrue\t0\tfalse\tfalse\tfalse\tfalse"));
    assert!(row.contains("\t57\t1\t1\t4\t4\tmanual-draft-needs-runtime-geometry\titem-acquisition-fixture\t0x80012210\t"));
    let fields = header
        .split('\t')
        .zip(row.split('\t'))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(header.split('\t').count(), row.split('\t').count());
    assert_eq!(fields.get("battle_name_page_id"), Some(&"116"));
    assert_eq!(fields.get("battle_name_physical_names"), Some(&"1"));
    assert_eq!(fields.get("battle_name_selected_drafts"), Some(&"1"));
    assert_eq!(fields.get("battle_name_final_exact"), Some(&"true"));
    assert_eq!(fields.get("battle_name_runtime_verified"), Some(&"false"));
    assert_eq!(fields.get("ui_literal_units"), Some(&"14"));
    assert_eq!(fields.get("ui_literal_world_map_labels"), Some(&"12"));
    assert_eq!(fields.get("ui_literal_binary_choices"), Some(&"2"));
    assert_eq!(
        fields.get("ui_literal_status"),
        Some(&"manual-draft-needs-runtime-geometry")
    );
    assert_eq!(fields.get("ui_literal_source_exact"), Some(&"true"));
    assert_eq!(fields.get("ui_literal_final_exact"), Some(&"true"));
    assert_eq!(fields.get("ui_literal_runtime_verified"), Some(&"false"));
    assert!(lines.next().is_none());
}
