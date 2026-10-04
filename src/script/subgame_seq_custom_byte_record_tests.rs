use sha2::{Digest, Sha256};

use super::*;

fn spec_for(raw: &[u8]) -> SubgameSeqCustomByteRecordSpec {
    let record_sha256 = Sha256::digest(&raw[48..64]).into();
    SubgameSeqCustomByteRecordSpec {
        scope: "PCOMI",
        archive: "/PCOMI/PC1_SEQ.VDD;1",
        sequence: "TEST.SEQ",
        sequence_bytes: raw.len(),
        sequence_sha256: Sha256::digest(raw).into(),
        load_base: 0x8010_0000,
        initialized_len: raw.len(),
        repertoire_root_field_offset: 0,
        repertoire_offset: 32,
        repertoire_bytes: 6,
        repertoire_sha256: Sha256::digest(&raw[32..38]).into(),
        repertoire_codes: 6,
        slot_table_root_field_offset: 4,
        slot_table_offset: 16,
        slot_count: 2,
        slot_stride: 16,
        slot_payload_bytes: 15,
        slot_array_start_offset: 48,
        slot_array_end_offset: 80,
        slot_array_sha256: Sha256::digest(&raw[48..80]).into(),
        used_codes: 6,
        target_slot_index: 0,
        pointer_field_offset: 16,
        next_pointer_field_offset: 20,
        record_start_offset: 48,
        record_end_offset: 64,
        record_sha256,
        candidate_offset: 52,
        candidate_raw: &[2, 3],
        candidate_heuristic_sjis: "ひ",
        embedded_ascii_offset: 56,
        embedded_ascii: b"USA",
        known_ascii_literals: &[(56, b"USA")],
    }
}

fn fixture() -> (Vec<u8>, SubgameSeqTextCandidate) {
    let mut raw = vec![0; 96];
    raw[0..4].copy_from_slice(&0x8010_0020u32.to_le_bytes());
    raw[4..8].copy_from_slice(&0x8010_0010u32.to_le_bytes());
    raw[16..20].copy_from_slice(&0x8010_0030u32.to_le_bytes());
    raw[20..24].copy_from_slice(&0x8010_0040u32.to_le_bytes());
    raw[32..38].copy_from_slice(&[1, 2, 3, b'U', b'S', b'A']);
    raw[48..63].fill(1);
    raw[64..79].fill(1);
    raw[52..54].copy_from_slice(&[2, 3]);
    raw[56..59].copy_from_slice(b"USA");
    let candidate = SubgameSeqTextCandidate {
        scope: "PCOMI",
        archive: "/PCOMI/PC1_SEQ.VDD;1".to_string(),
        sequence: "TEST.SEQ".to_string(),
        sequence_sha256: Sha256::digest(&raw).into(),
        offset: 52,
        raw: vec![2, 3],
        hiragana: 1,
        katakana: 0,
        decoded: Some("ひ".to_string()),
    };
    (raw, candidate)
}

#[test]
fn pointer_pair와_nul이_candidate를_custom_record에_귀속한다() {
    let (raw, candidate) = fixture();
    let row = audit_record(&raw, &candidate, spec_for(&raw)).unwrap();

    assert_eq!(row.candidate_delta, 4);
    assert_eq!(row.record_start_runtime, 0x8010_0030);
    assert_eq!(row.record_end_runtime, 0x8010_0040);
    assert_eq!(row.slot_count, 2);
    assert_eq!(row.repertoire_codes, 6);
    assert_eq!(row.owner_id(), "PCOMI:TEST.SEQ:CUSTOM-BYTE-RECORD@0x0030");
}

#[test]
fn 다음_pointer나_nul이_달라지면_부분_owner를_반환하지_않는다() {
    let (mut raw, mut candidate) = fixture();
    let mut spec = spec_for(&raw);
    raw[20..24].copy_from_slice(&0x8010_003Fu32.to_le_bytes());
    let changed_source_sha256 = Sha256::digest(&raw).into();
    spec.sequence_sha256 = changed_source_sha256;
    candidate.sequence_sha256 = changed_source_sha256;
    assert!(audit_record(&raw, &candidate, spec).is_err());

    let (mut raw, mut candidate) = fixture();
    let mut spec = spec_for(&raw);
    raw[63] = 1;
    let changed_source_sha256 = Sha256::digest(&raw).into();
    spec.sequence_sha256 = changed_source_sha256;
    spec.record_sha256 = Sha256::digest(&raw[48..64]).into();
    spec.slot_array_sha256 = Sha256::digest(&raw[48..80]).into();
    candidate.sequence_sha256 = changed_source_sha256;
    assert!(audit_record(&raw, &candidate, spec).is_err());
}

#[test]
fn slot에서_repertoire에_없는_code가_나오면_실패한다() {
    let (mut raw, mut candidate) = fixture();
    let mut spec = spec_for(&raw);
    raw[70] = 0xFE;
    let changed_source_sha256 = Sha256::digest(&raw).into();
    spec.sequence_sha256 = changed_source_sha256;
    spec.slot_array_sha256 = Sha256::digest(&raw[48..80]).into();
    candidate.sequence_sha256 = changed_source_sha256;

    assert!(audit_record(&raw, &candidate, spec).is_err());
}
