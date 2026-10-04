use super::*;

fn message_row(
    scope: &'static str,
    index: usize,
    command_offset: usize,
    record_offset: usize,
    raw: &[u8],
) -> SubgameSeqSceneMessageRow {
    SubgameSeqSceneMessageRow {
        scope,
        exe: if scope == "PUYO1" {
            "PUYO1.EXE"
        } else {
            "PUYO2.EXE"
        },
        archive: if scope == "PUYO1" {
            "/PUYO1/PP1_SEQ.VDD;1"
        } else {
            "/PUYO2/PP2_SEQ.VDD;1"
        },
        sequence: "TEST_SEQ.SEQ",
        sequence_sha256: [0; 32],
        index,
        command_offset,
        pointer_field_offset: command_offset + 4,
        pointer_field_runtime: 0,
        record_offset,
        record_runtime: 0,
        record_end_offset: record_offset + raw.len(),
        variant: 0,
        header: raw[..4].try_into().unwrap(),
        raw: raw.to_vec(),
        raw_sha256: Sha256::digest(raw).into(),
        payload_copy_count: 2,
        counterpart_scope: Some(if scope == "PUYO1" { "PUYO2" } else { "PUYO1" }),
        counterpart_index: Some(index),
        glyph_tokens: 1,
        control_tokens: 1,
        argument_controls: 0,
        unique_glyph_cells: 1,
        maximum_glyph_cell: 0,
        control_counts: [(0x72, 1)].into_iter().collect(),
        source_text: "あ<72><6565>".to_string(),
    }
}

fn message_analysis() -> SubgameSeqSceneMessageAnalysis {
    let raw = [1, 2, 3, 4, 0x82, 0xA0, 0x72, 0x65, 0x65];
    SubgameSeqSceneMessageAnalysis {
        rows: vec![
            message_row("PUYO1", 1, 0x10, 0x20, &raw),
            message_row("PUYO2", 1, 0x20, 0x28, &raw),
        ],
        families: 1,
        physical_messages: 2,
        glyph_tokens: 2,
        control_tokens: 2,
        argument_controls: 0,
        terminators: 2,
        variant_records: 0,
        direct_pointers: 2,
        referenced_message_bytes: raw.len() * 2,
        unique_message_bytes: raw.len() * 2,
        unique_payloads: 1,
        cross_scope_pairs: 1,
        control_counts: [(0x72, 2)].into_iter().collect(),
    }
}

fn manifest(status: &str, source: &str, ko: &str) -> String {
    let messages = message_analysis();
    let raw_sha256 = digest_array(&messages.rows[0].raw_sha256);
    format!(
        "{HEADER}\nscene-e0-translation-manifest-v1\t{status}\tSCENE-E0:TEST_SEQ.SEQ:L01\tTEST_SEQ.SEQ\t{raw_sha256}\t2\tPUYO1:TEST_SEQ.SEQ:0x0010\tPUYO2:TEST_SEQ.SEQ:0x0020\t0x0020\t0x0028\t{source}\t{ko}\tseries-reference-pending\tcontrol-placeholders-protected\n"
    )
}

#[test]
fn pending_manifest는_원문과_두_물리사본을_같은행에서_검증한다() {
    let messages = message_analysis();
    let raw = manifest("pending-human-translation", "あ<72><6565>", "");
    let analysis = audit_subgame_seq_scene_translation_manifest(&raw, &messages).unwrap();
    assert_eq!(analysis.logical_payloads, 1);
    assert_eq!(analysis.physical_messages, 2);
    assert_eq!(analysis.pending_human_translation, 1);
    assert!(!analysis.product_eligible);
    assert_eq!(
        render_box_subgame_seq_scene_translation_manifest_tsv(&analysis),
        raw
    );
}

#[test]
fn draft는_control_placeholder를_같은_순서로_보존한다() {
    let messages = message_analysis();
    let valid = manifest("draft-needs-human-review", "あ<72><6565>", "가<72><6565>");
    assert!(audit_subgame_seq_scene_translation_manifest(&valid, &messages).is_ok());

    let drift = manifest("draft-needs-human-review", "あ<72><6565>", "가<6565>");
    assert!(audit_subgame_seq_scene_translation_manifest(&drift, &messages).is_err());
}

#[test]
fn status와_번역내용과_보호원문_drift를_거부한다() {
    let messages = message_analysis();
    let pending_with_text = manifest("pending-human-translation", "あ<72><6565>", "가<72><6565>");
    assert!(audit_subgame_seq_scene_translation_manifest(&pending_with_text, &messages).is_err());

    let japanese = manifest("draft-needs-human-review", "あ<72><6565>", "あ<72><6565>");
    assert!(audit_subgame_seq_scene_translation_manifest(&japanese, &messages).is_err());

    let source_drift = manifest("pending-human-translation", "い<72><6565>", "");
    assert!(audit_subgame_seq_scene_translation_manifest(&source_drift, &messages).is_err());
}
