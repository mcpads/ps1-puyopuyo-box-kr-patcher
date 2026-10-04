use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn message_row(
    raw: &[u8],
    map: RuntimeAssetMap,
    index: usize,
    command_offset: usize,
    pointer_field_offset: usize,
    record_offset: usize,
    record_len: usize,
) -> SubgameSeqSceneMessageRow {
    let record = raw[record_offset..record_offset + record_len].to_vec();
    SubgameSeqSceneMessageRow {
        scope: "PUYO1",
        exe: "PUYO1.EXE",
        archive: "/PUYO1/PP1_SEQ.VDD;1",
        sequence: "TEST_SEQ.SEQ",
        sequence_sha256: Sha256::digest(raw).into(),
        index,
        command_offset,
        pointer_field_offset,
        pointer_field_runtime: map.runtime_addr(pointer_field_offset).unwrap(),
        record_offset,
        record_runtime: map.runtime_addr(record_offset).unwrap(),
        record_end_offset: record_offset + record_len,
        variant: 0,
        header: record[..4].try_into().unwrap(),
        raw_sha256: Sha256::digest(&record).into(),
        raw: record,
        payload_copy_count: 1,
        counterpart_scope: None,
        counterpart_index: None,
        glyph_tokens: 1,
        control_tokens: 0,
        argument_controls: 0,
        unique_glyph_cells: 1,
        maximum_glyph_cell: 0,
        control_counts: BTreeMap::new(),
        source_text: String::new(),
    }
}

fn message_analysis(rows: Vec<SubgameSeqSceneMessageRow>) -> SubgameSeqSceneMessageAnalysis {
    SubgameSeqSceneMessageAnalysis {
        physical_messages: rows.len(),
        rows,
        families: 1,
        glyph_tokens: 2,
        control_tokens: 0,
        argument_controls: 0,
        terminators: 2,
        variant_records: 0,
        direct_pointers: 2,
        referenced_message_bytes: 16,
        unique_message_bytes: 16,
        unique_payloads: 2,
        cross_scope_pairs: 0,
        control_counts: BTreeMap::new(),
    }
}

fn glyph_analysis(sequence_sha256: [u8; 32]) -> SubgameSeqSceneGlyphAnalysis {
    SubgameSeqSceneGlyphAnalysis {
        rows: vec![crate::analysis::SubgameSeqSceneGlyphRow {
            scope: "PUYO1",
            exe: "PUYO1.EXE",
            seq_archive: "/PUYO1/PP1_SEQ.VDD;1",
            grp_archive: "/PUYO1/PP1_GRP.VDD;1",
            sequence: "TEST_SEQ.SEQ",
            sequence_sha256,
            e2_record_offset: 0,
            e1_record_offset: 0,
            repertoire_offset: 80,
            repertoire_runtime: 0,
            repertoire_bytes: 4,
            repertoire_slots: 2,
            strict_sjis_pairs: 2,
            opaque_pairs: 0,
            repertoire_unique_pairs: 2,
            repertoire_sha256: [0; 32],
            repertoire_copy_equal: true,
            canonical_glyph_slots: 2,
            protected_message_slots: 0,
            compound_message_records: 0,
            compound_glyph_tokens: 0,
            compound_control_tokens: 0,
            compound_variant_records: 0,
            embedded_message_record_offset: None,
            resource_id: 0,
            auxiliary_id: 0,
            auxiliary_asset: String::new(),
            cell_setup_record_offset: 0,
            cell_table_offset: 0,
            cell_table_runtime: 0,
            cell_base: 0,
            cell_last: 1,
            grid_slots: 2,
            selected_asset: String::new(),
            selected_vdd_index: 0,
            selected_stored_bytes: 0,
            selected_cnx_subtype: [0; 3],
            selected_plain_bytes: 0,
            selected_plain_sha256: [0; 32],
            selected_atlas_compatible: true,
            same_stem_asset: "TEST.BTX",
            same_stem_resource_id: 0,
            same_stem_vdd_index: 0,
            same_stem_stored_bytes: 0,
            same_stem_cnx_subtype: [0; 3],
            same_stem_plain_bytes: 0,
            same_stem_plain_sha256: [0; 32],
            selected_is_same_stem: true,
            selected_same_stem_equal_grid_cells: 2,
            same_stem_copy_equal: true,
            e1_handler_runtime: 0,
            e2_handler_runtime: 0,
            pair_lookup_runtime: 0,
            renderer_runtime: 0,
        }],
        families: 1,
        repertoire_slots: 2,
        strict_sjis_pairs: 2,
        opaque_pairs: 0,
        grid_slots: 2,
        repertoire_copy_matches: 1,
        same_stem_copy_matches: 1,
        puyo1_selected_same_stem_rows: 1,
        puyo2_selected_same_stem_rows: 0,
        puyo1_selected_atlas_rows: 1,
        puyo2_selected_atlas_rows: 0,
        puyo1_selected_same_stem_cells: 2,
        puyo2_selected_same_stem_cells: 0,
        canonical_glyph_slots: 2,
        protected_message_slots: 0,
        compound_rows: 0,
        compound_message_records: 0,
        compound_glyph_tokens: 0,
        compound_control_tokens: 0,
        compound_variant_records: 0,
    }
}

#[test]
fn control_signature는_forbidden과_인자_control을_원형보존한다() {
    let repertoire = [[0x82, 0xA0]].into_iter().collect();
    let record = [1, 2, 3, 4, 0x82, 0xA0, 0x63, 0x71, 0x20, 0x78, 0x65, 0x65];
    assert_eq!(
        control_signature(&record, &repertoire, 0x78).unwrap(),
        vec![vec![0x63], vec![0x71, 0x20], vec![0x78]]
    );
}

#[test]
fn control_signature는_header_control_glyph_종결_drift를_거부한다() {
    let repertoire = [[0x82, 0xA0]].into_iter().collect();
    assert!(control_signature(&[1, 2, 3, 4, 0x82, 0xA2, 0x65, 0x65], &repertoire, 0x78).is_err());
    assert!(control_signature(&[1, 2, 3, 4, 0x62, 0x65, 0x65], &repertoire, 0x78).is_err());
    assert!(control_signature(&[1, 2, 3, 4, 0x65, 0x65, 0], &repertoire, 0x78).is_err());
}

#[test]
fn pointer_scan은_정렬되지_않은_block내부_참조도_숨기지_않는다() {
    let mut raw = vec![0; 64];
    raw[4..8].copy_from_slice(&0x801B_5280u32.to_le_bytes());
    raw[13..17].copy_from_slice(&0x801B_5285u32.to_le_bytes());
    let map = RuntimeAssetMap::new(0x801B_5270, raw.len() as u32).unwrap();
    let pointers = scan_block_pointers(&raw, raw.len(), map, 0x10, 0x20).unwrap();
    assert_eq!(
        pointers
            .iter()
            .map(|pointer| (pointer.field_offset, pointer.target_offset))
            .collect::<Vec<_>>(),
        vec![(4, 0x10), (13, 0x15)]
    );
}

#[test]
fn block은_message_사이의_zero_gap만_용량으로_인정한다() {
    let map = RuntimeAssetMap::new(0x8010_0000, 64).unwrap();
    let mut raw = vec![0; 64];
    raw[32..40].fill(1);
    raw[42..50].fill(2);
    raw[4..8].copy_from_slice(&map.runtime_addr(32).unwrap().to_le_bytes());
    raw[8..12].copy_from_slice(&map.runtime_addr(42).unwrap().to_le_bytes());
    let rows = [
        message_row(&raw, map, 1, 0x10, 4, 32, 8),
        message_row(&raw, map, 2, 0x20, 8, 42, 8),
    ];
    let references = rows.iter().collect::<Vec<_>>();
    let blocks = discover_blocks(&raw, raw.len(), map, &references).unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].capacity_bytes, 18);
    assert_eq!(blocks[0].original_message_bytes, 16);
    assert_eq!(blocks[0].slack_bytes, 2);

    raw[40] = 0x7F;
    let blocks = discover_blocks(&raw, raw.len(), map, &references).unwrap();
    assert_eq!(blocks.len(), 2);
    assert!(blocks.iter().all(|block| block.slack_bytes == 0));
}

#[test]
fn block은_e0_외부의_incoming_pointer를_거부한다() {
    let map = RuntimeAssetMap::new(0x8010_0000, 64).unwrap();
    let mut raw = vec![0; 64];
    raw[32..40].fill(1);
    raw[42..50].fill(2);
    raw[4..8].copy_from_slice(&map.runtime_addr(32).unwrap().to_le_bytes());
    raw[8..12].copy_from_slice(&map.runtime_addr(42).unwrap().to_le_bytes());
    raw[13..17].copy_from_slice(&map.runtime_addr(35).unwrap().to_le_bytes());
    let rows = [
        message_row(&raw, map, 1, 0x10, 4, 32, 8),
        message_row(&raw, map, 2, 0x20, 8, 42, 8),
    ];
    let references = rows.iter().collect::<Vec<_>>();
    assert!(discover_blocks(&raw, raw.len(), map, &references).is_err());
}

#[test]
fn repack은_block안에서_record를_옮기고_pointer를_재검산한다() {
    let map = RuntimeAssetMap::new(0x8010_0000, 128).unwrap();
    let mut raw = vec![0; 128];
    raw[32..40].copy_from_slice(&[1, 2, 3, 4, 0x82, 0xA0, 0x65, 0x65]);
    raw[42..50].copy_from_slice(&[5, 6, 7, 8, 0x82, 0xA2, 0x65, 0x65]);
    raw[80..84].copy_from_slice(&[0x82, 0xA0, 0x82, 0xA2]);
    raw[4..8].copy_from_slice(&map.runtime_addr(32).unwrap().to_le_bytes());
    raw[8..12].copy_from_slice(&map.runtime_addr(42).unwrap().to_le_bytes());
    let rows = vec![
        message_row(&raw, map, 1, 0x10, 4, 32, 8),
        message_row(&raw, map, 2, 0x20, 8, 42, 8),
    ];
    let references = rows.iter().collect::<Vec<_>>();
    let blocks = discover_blocks(&raw, raw.len(), map, &references).unwrap();
    let sequence_sha256: [u8; 32] = Sha256::digest(&raw).into();
    let relinks = SubgameSeqSceneRelinkAnalysis {
        rows: vec![SubgameSeqSceneRelinkRow {
            scope: "PUYO1",
            exe: "PUYO1.EXE",
            archive: "/PUYO1/PP1_SEQ.VDD;1",
            sequence: "TEST_SEQ.SEQ",
            sequence_sha256,
            load_base: map.base,
            initialized_bytes: raw.len(),
            messages: 2,
            blocks,
        }],
        blocks: 1,
        messages: 2,
        capacity_bytes: 18,
        original_message_bytes: 16,
        slack_bytes: 2,
        zero_slack_blocks: 0,
    };
    let messages = message_analysis(rows);
    let glyphs = glyph_analysis(sequence_sha256);
    let replacements = vec![
        SubgameSeqSceneReplacement {
            message_id: "PUYO1:TEST_SEQ.SEQ:0x0010".to_string(),
            raw: vec![1, 2, 3, 4, 0x65, 0x65],
        },
        SubgameSeqSceneReplacement {
            message_id: "PUYO1:TEST_SEQ.SEQ:0x0020".to_string(),
            raw: vec![5, 6, 7, 8, 0x82, 0xA2, 0x65, 0x65],
        },
    ];
    let repack = repack_box_subgame_seq_scene_messages(
        "PUYO1",
        "TEST_SEQ.SEQ",
        &raw,
        &replacements,
        &messages,
        &glyphs,
        &relinks,
    )
    .unwrap();
    assert_eq!(repack.record_offsets["PUYO1:TEST_SEQ.SEQ:0x0010"], 32);
    assert_eq!(repack.record_offsets["PUYO1:TEST_SEQ.SEQ:0x0020"], 38);
    assert_eq!(
        read_u32(&repack.sequence, 8),
        Some(map.runtime_addr(38).unwrap())
    );
    assert_eq!(repack.used_bytes, 14);
    assert_eq!(repack.remaining_bytes, 4);
    assert!(repack.sequence[46..50].iter().all(|&byte| byte == 0));
}
