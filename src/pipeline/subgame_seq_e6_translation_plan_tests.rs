use super::*;
use crate::disasm::{SubgameSeqE6TextAnalysis, SubgameSeqE6TextRow, SubgameSeqE6TextSpan};
use sha2::{Digest, Sha256};

fn pair(glyph: char) -> [u8; 2] {
    encoding_rs::SHIFT_JIS
        .encode(&glyph.to_string())
        .0
        .as_ref()
        .try_into()
        .unwrap()
}

fn analysis(raw: &[u8]) -> SubgameSeqE6TextAnalysis {
    let sequence_sha256 = Sha256::digest(raw).into();
    let make_row =
        |index: usize, stream_offset: usize, stream: Vec<u8>, text: &str| SubgameSeqE6TextRow {
            scope: "PUYO2",
            archive: "/PUYO2/PP2_SEQ.VDD;1".to_string(),
            sequence: "TEST_SEQ.SEQ".to_string(),
            sequence_sha256,
            sequence_index: index,
            command_offset: 0x20 + (index - 1) * 8,
            command_runtime: 0x801B_5290 + u32::try_from((index - 1) * 8).unwrap(),
            pointer_field_offset: 0x24 + (index - 1) * 8,
            pointer_field_runtime: 0x801B_5294 + u32::try_from((index - 1) * 8).unwrap(),
            stream_offset,
            stream_runtime: 0x801B_5270 + u32::try_from(stream_offset).unwrap(),
            raw_sha256: Sha256::digest(&stream).into(),
            raw: stream.clone(),
            spans: vec![SubgameSeqE6TextSpan {
                offset: stream_offset,
                raw: stream[..2].to_vec(),
                decoded: text.to_string(),
                candidate_id: format!("TEST:{index}"),
            }],
            control_70: 0,
            control_72: 1,
            control_77: 0,
            emitted_tokens: 6,
            counterpart_raw_equal: true,
            glyph_resource: 0x0088,
            glyph_asset: "DEMOFONT.BTX",
        };
    let first = [pair('あ'), [0x72, 0x72], [0x65, 0x65]].concat();
    let second = [pair('い'), [0x72, 0x72], [0x65, 0x65]].concat();
    SubgameSeqE6TextAnalysis {
        rows: vec![
            make_row(1, 0x80, first, "あ"),
            make_row(2, 0x86, second, "い"),
        ],
        consumers: 0,
        physical_sequences: 0,
        logical_sequences: 0,
        spans: 0,
        code_units: 0,
        unique_glyph_pairs: 0,
        candidate_spans: 0,
        exact_e6_roots: 0,
        exact_e1_roots: 0,
        control_70: 0,
        control_72: 0,
        control_77: 0,
        terminators: 0,
        min_pairs: 0,
        max_pairs: 0,
        max_emitted_tokens: 0,
        counterpart_equal: 0,
        counterpart_different: 0,
        code_table_unique_pairs: 0,
        resource_links: 0,
    }
}

fn fixture() -> (Vec<u8>, SubgameSeqE6TextAnalysis, Vec<u8>) {
    let mut raw = vec![0u8; 0xC0 + SUBGAME_SEQ_OVERLAY_FOOTER_LEN];
    let first = [pair('あ'), [0x72, 0x72], [0x65, 0x65]].concat();
    let second = [pair('い'), [0x72, 0x72], [0x65, 0x65]].concat();
    raw[0x80..0x86].copy_from_slice(&first);
    raw[0x86..0x8C].copy_from_slice(&second);
    raw[0x24..0x28].copy_from_slice(&0x801B_52F0u32.to_le_bytes());
    raw[0x2C..0x30].copy_from_slice(&0x801B_52F6u32.to_le_bytes());
    let analysis = analysis(&raw);
    let code_table = [pair('あ'), pair('い'), pair('！'), pair('。')].concat();
    (raw, analysis, code_table)
}

fn translations(first: &str, second: &str) -> Vec<SubgameSeqE6Translation> {
    vec![
        SubgameSeqE6Translation {
            sequence: "TEST_SEQ.SEQ".to_string(),
            sequence_index: 1,
            spans: vec![first.to_string()],
        },
        SubgameSeqE6Translation {
            sequence: "TEST_SEQ.SEQ".to_string(),
            sequence_index: 2,
            spans: vec![second.to_string()],
        },
    ]
}

#[test]
fn 기존_pair와_한글을_분리해_first_match_cell에_배정한다() {
    let (raw, analysis, code_table) = fixture();
    let plan = plan_translation(
        &translations("！", "가"),
        &code_table,
        &analysis,
        4,
        None,
        false,
    )
    .unwrap();
    assert_eq!(plan.reachable_slots, 4);
    assert_eq!(plan.retained_source_characters, 1);
    assert_eq!(plan.glyph_mappings.len(), 1);
    assert_eq!(plan.glyph_mappings[0].glyph, '가');
    assert_eq!(plan.glyph_mappings[0].cell_index, 0);
    assert_eq!(plan.glyph_mappings[0].pair, pair('あ'));
    assert_eq!(plan.remaining_slots, 2);

    let repack = repack_box_puyo2_subgame_seq_e6_sequence("TEST_SEQ.SEQ", &raw, &plan).unwrap();
    assert_eq!(repack.pointer_relations, 2);
    assert_eq!(repack.stream_offsets, vec![0x80, 0x86]);
    assert_eq!(read_u32(&repack.sequence_bytes, 0x24), Some(0x801B_52F0));
    assert_eq!(read_u32(&repack.sequence_bytes, 0x2C), Some(0x801B_52F6));
    assert_eq!(
        &repack.sequence_bytes[raw.len() - SUBGAME_SEQ_OVERLAY_FOOTER_LEN..],
        &raw[raw.len() - SUBGAME_SEQ_OVERLAY_FOOTER_LEN..]
    );
}

#[test]
fn 일본어잔존과_전역slot초과를_거부한다() {
    let (_, analysis, code_table) = fixture();
    assert!(plan_translation(
        &translations("あ", "가"),
        &code_table,
        &analysis,
        4,
        None,
        false,
    )
    .unwrap_err()
    .to_string()
    .contains("일본어"));

    let mut too_many = String::new();
    too_many.extend(['가', '나', '다', '라', '마']);
    assert!(plan_translation(
        &translations(&too_many, "가"),
        &code_table,
        &analysis,
        4,
        None,
        false,
    )
    .unwrap_err()
    .to_string()
    .contains("glyph 용량 초과"));
}

#[test]
fn 연속block용량과_incoming_pointer분모를_실패폐쇄한다() {
    let (raw, analysis, code_table) = fixture();
    assert!(plan_translation(
        &translations("가나다", "가나다"),
        &code_table,
        &analysis,
        4,
        None,
        false,
    )
    .unwrap_err()
    .to_string()
    .contains("block 용량 초과"));

    let plan = plan_translation(
        &translations("가", "가"),
        &code_table,
        &analysis,
        4,
        None,
        false,
    )
    .unwrap();
    let mut extra_pointer = raw.clone();
    extra_pointer[0x34..0x38].copy_from_slice(&0x801B_52F2u32.to_le_bytes());
    let digest: [u8; 32] = Sha256::digest(&extra_pointer).into();
    let mut drifted_plan = plan.clone();
    for row in &mut drifted_plan.replacements {
        row.sequence_sha256 = digest;
    }
    assert!(repack_box_puyo2_subgame_seq_e6_sequence(
        "TEST_SEQ.SEQ",
        &extra_pointer,
        &drifted_plan,
    )
    .unwrap_err()
    .to_string()
    .contains("incoming pointer 분모"));
}
