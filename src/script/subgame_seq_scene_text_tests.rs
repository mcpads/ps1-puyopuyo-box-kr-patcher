use sha2::{Digest, Sha256};

use crate::script::{
    audit_subgame_seq_layouts, audit_subgame_seq_text_candidates, audit_subgame_seq_text_roots,
};
use crate::text::encode_sjis;

use super::*;

const ARCHIVE: &str = "/PUYO1/PP1_SEQ.VDD;1";

fn fixture() -> (Vec<u8>, SubgameSeqSceneTextSpec<'static>) {
    let base = 0x801B_5270u32;
    let repertoire = encode_sjis("アイウエ").unwrap();
    let message = encode_sjis("アイウ").unwrap();
    let mut raw = vec![0; 0x80];
    raw[0x10..0x14].copy_from_slice(&(base + 0x40).to_le_bytes());
    raw[0x1C..0x20].copy_from_slice(&[0xE0, 1, 0, 0]);
    raw[0x20..0x24].copy_from_slice(&(base + 0x4A).to_le_bytes());
    raw[0x40..0x48].copy_from_slice(&repertoire);
    raw[0x48..0x4A].copy_from_slice(&[0x65, 0x65]);
    raw[0x4A..0x4E].copy_from_slice(&[1, 2, 3, 1]);
    raw[0x4E..0x54].copy_from_slice(&message);
    raw[0x54..0x56].copy_from_slice(&[0x65, 0x65]);
    raw[0x56..0x5A].copy_from_slice(&encode_sjis("エア").unwrap());
    raw[0x5A..0x5C].copy_from_slice(&[0x65, 0x65]);
    let initialized_len = raw.len() as u32;
    let bss = base + initialized_len;
    for word in [
        0,
        0,
        0,
        base,
        initialized_len,
        bss,
        0x2C,
        bss + 0x30,
        0,
        0x8000,
        0x0080_0000,
    ] {
        raw.extend_from_slice(&word.to_le_bytes());
    }
    let digest: [u8; 32] = Sha256::digest(&raw).into();
    let spec = SubgameSeqSceneTextSpec {
        scope: "TEST",
        archive: ARCHIVE,
        sequence: "A.SEQ",
        expected_sequence_sha256: digest,
        expected_bytes: raw.len(),
        repertoire_pointer_field: 0x10,
        repertoire_offset: 0x40,
        repertoire_bytes: 8,
        message_pointer_fields: &[0x20],
        message_offsets: &[0x4A],
        message_end_offset: 0x5C,
        padding_end_offset: 0x5E,
    };
    (raw, spec)
}

fn analyses(
    raw: &[u8],
) -> (
    SubgameSeqTextCandidateAnalysis,
    SubgameSeqLayoutAnalysis,
    SubgameSeqTextRootAnalysis,
) {
    let text = audit_subgame_seq_text_candidates([(ARCHIVE, "A.SEQ", raw)]).unwrap();
    let layouts = audit_subgame_seq_layouts([(ARCHIVE, "A.SEQ", raw)], &text).unwrap();
    let roots = audit_subgame_seq_text_roots([(ARCHIVE, "A.SEQ", raw)], &text, &layouts).unwrap();
    (text, layouts, roots)
}

#[test]
fn 공용_scene_text가_repertoire와_e0_message를_분리한다() {
    let (raw, spec) = fixture();
    let (text, layouts, roots) = analyses(&raw);
    let analysis = audit_subgame_seq_scene_text(
        &[spec],
        [(ARCHIVE, "A.SEQ", raw.as_slice())],
        &text,
        &layouts,
        &roots,
    )
    .unwrap();

    assert_eq!(analysis.repertoires, 1);
    assert_eq!(analysis.message_records, 1);
    assert_eq!(analysis.message_code_units, 5);
    assert_eq!(analysis.message_terminators, 2);
    assert_eq!(analysis.direct_pointers, 2);
    assert_eq!(analysis.zero_reserve_bytes, 2);
    assert_eq!(analysis.rows[1].command_variant, Some(1));
    assert!(render_subgame_seq_scene_text_tsv(&analysis)
        .contains("preserve-no-write-until-control-layout-and-runtime-scene-contract"));
}

#[test]
fn source_digest와_e0_command와_pointer전수를_실패폐쇄한다() {
    let (raw, spec) = fixture();
    let (text, layouts, roots) = analyses(&raw);

    let mut wrong_spec = spec;
    wrong_spec.expected_bytes += 1;
    assert!(audit_subgame_seq_scene_text(
        &[wrong_spec],
        [(ARCHIVE, "A.SEQ", raw.as_slice())],
        &text,
        &layouts,
        &roots,
    )
    .is_err());

    let mut command_drift = raw.clone();
    command_drift[0x1C] = 0xE1;
    let (drift_text, drift_layouts, drift_roots) = analyses(&command_drift);
    let mut drift_spec = spec;
    drift_spec.expected_sequence_sha256 = Sha256::digest(&command_drift).into();
    assert!(audit_subgame_seq_scene_text(
        &[drift_spec],
        [(ARCHIVE, "A.SEQ", command_drift.as_slice())],
        &drift_text,
        &drift_layouts,
        &drift_roots,
    )
    .is_err());

    let mut extra_pointer = raw.clone();
    extra_pointer[0x28..0x2C].copy_from_slice(&(0x801B_5270u32 + 0x4A).to_le_bytes());
    let (extra_text, extra_layouts, extra_roots) = analyses(&extra_pointer);
    let mut extra_spec = spec;
    extra_spec.expected_sequence_sha256 = Sha256::digest(&extra_pointer).into();
    assert!(audit_subgame_seq_scene_text(
        &[extra_spec],
        [(ARCHIVE, "A.SEQ", extra_pointer.as_slice())],
        &extra_text,
        &extra_layouts,
        &extra_roots,
    )
    .is_err());
}
