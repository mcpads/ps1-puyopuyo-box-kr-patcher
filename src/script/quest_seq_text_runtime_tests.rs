use super::*;
use crate::script::{
    QuestSeqExtractedTextStream, QuestSeqTextExtractionAnalysis, QuestTextTerminal,
    QUEST_SEQ_TEXT_MODE_RECORD_BYTES,
};

fn decode_sha256(value: &str) -> [u8; 32] {
    let bytes = value.as_bytes();
    let mut decoded = [0u8; 32];
    for (index, pair) in bytes.chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(pair).expect("ASCII hex");
        decoded[index] = u8::from_str_radix(text, 16).expect("hex byte");
    }
    decoded
}

fn fixture() -> QuestSeqTextExtractionAnalysis {
    let sequence_sha256 = decode_sha256(TOWN00_SEQ_SHA256);
    let streams = QUEST_SEQ_TEXT_RUNTIME_EVIDENCE
        .iter()
        .map(|entry| QuestSeqExtractedTextStream {
            sequence: entry.sequence.to_string(),
            sequence_sha256,
            record_offset: entry.record_offset,
            record_raw: [0x21, 0, 0, 0],
            stream_offset: entry.record_offset + QUEST_SEQ_TEXT_MODE_RECORD_BYTES,
            end_offset: entry.record_offset + 8,
            terminal: QuestTextTerminal::DeactivateAndAlign,
            expected_raw_sha256: decode_sha256(entry.expected_stream_sha256),
            tokens: Vec::new(),
        })
        .collect();
    QuestSeqTextExtractionAnalysis {
        sequences: 113,
        source_bytes: 243_546,
        streams,
        token_occurrences: 0,
        glyph_occurrences: 0,
        control_occurrences: 0,
        roundtrip_bytes: 0,
    }
}

#[test]
fn visible표본과_op21_hit를_서로_다른_증거로_보존한다() {
    let analysis = audit_quest_seq_text_runtime_evidence(&fixture()).unwrap();
    assert_eq!(
        (
            analysis.entries.len(),
            analysis.op21_hits,
            analysis.visible_only
        ),
        (3, 2, 1)
    );
    assert_eq!(
        analysis.entries[2].stream_id(),
        "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x22DC"
    );
}

#[test]
fn 정적_stream_누락과_digest_drift를_거부한다() {
    let mut missing = fixture();
    missing.streams.pop();
    assert!(audit_quest_seq_text_runtime_evidence(&missing).is_err());

    let mut changed = fixture();
    changed.streams[0].expected_raw_sha256[0] ^= 1;
    assert!(audit_quest_seq_text_runtime_evidence(&changed).is_err());
}

#[test]
fn runtime_record주소는_coords_사상과_object_field를_따른다() {
    let entry = QUEST_SEQ_TEXT_RUNTIME_EVIDENCE[2];
    validate_runtime_coordinates(entry).unwrap();
    assert_eq!(entry.observed_record_runtime, Some(0x8019_BD4C));
    assert_eq!(entry.object_stream_field_runtime, Some(0x800C_51D4));
}

#[test]
fn runtime_evidence_tsv는_정적_stream_id와_증거수준을_보존한다() {
    let analysis = audit_quest_seq_text_runtime_evidence(&fixture()).unwrap();
    let report = render_quest_seq_text_runtime_evidence_tsv(&analysis);
    assert_eq!(report.lines().count(), 4);
    assert!(report.contains("runtime-visible-source"));
    assert!(report.contains("runtime-op21-hit"));
    assert!(report.contains("01KYMW9EJ0KE5F8HH65ZEKB65A"));
}
