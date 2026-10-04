use super::*;
use crate::disasm::{SubgameSeqE6TextRow, SubgameSeqE6TextSpan};
use sha2::{Digest, Sha256};

fn fixture() -> SubgameSeqE6TextAnalysis {
    let raw = b"fixture";
    SubgameSeqE6TextAnalysis {
        rows: vec![SubgameSeqE6TextRow {
            scope: "PUYO2",
            archive: PUYO2_ARCHIVE.to_string(),
            sequence: "TEST.SEQ".to_string(),
            sequence_sha256: Sha256::digest(raw).into(),
            sequence_index: 1,
            command_offset: 0,
            command_runtime: 0,
            pointer_field_offset: 0,
            pointer_field_runtime: 0,
            stream_offset: 0x20,
            stream_runtime: 0,
            raw: raw.to_vec(),
            raw_sha256: Sha256::digest(raw).into(),
            spans: vec![SubgameSeqE6TextSpan {
                offset: 0x22,
                raw: vec![0; 2],
                decoded: "あ".to_string(),
                candidate_id: "TEST:1".to_string(),
            }],
            control_70: 0,
            control_72: 0,
            control_77: 0,
            emitted_tokens: 0,
            counterpart_raw_equal: false,
            glyph_resource: 0x88,
            glyph_asset: "DEMOFONT.BTX",
        }],
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

fn manifest(status: &str, source: &str, ko: &str) -> String {
    let text = fixture();
    let row = &text.rows[0];
    format!(
        "{HEADER}\n{}\t{status}\t{PUYO2_ARCHIVE}\tTEST.SEQ\t{}\t1\t0x0020\t{}\t1\t0x0022\tTEST:1\t{source}\t{ko}\t검산\n",
        PUYO2_E6_TRANSLATION_MANIFEST_SCHEMA_VERSION,
        digest_array(&row.sequence_sha256),
        digest_array(&row.raw_sha256),
    )
}

#[test]
fn 보호원문과초안을같은행에서검증한다() {
    let text = fixture();
    let raw = manifest("draft-needs-human-review", "あ", "가");
    let analysis = verify_manifest(&raw, &text).unwrap();
    assert_eq!(analysis.streams, 1);
    assert_eq!(analysis.spans, 1);
    assert_eq!(analysis.translations[0].spans, ["가"]);
    assert!(!analysis.product_eligible);
}

#[test]
fn 원문drift와일본어잔존을거부한다() {
    let text = fixture();
    assert!(
        verify_manifest(&manifest("draft-needs-human-review", "い", "가"), &text)
            .unwrap_err()
            .to_string()
            .contains("drift")
    );
    assert!(
        parse_manifest(&manifest("draft-needs-human-review", "あ", "あ"))
            .unwrap_err()
            .to_string()
            .contains("일본어")
    );
}

#[test]
fn 승인상태만제품입력을허용한다() {
    let text = fixture();
    let draft = verify_manifest(&manifest("draft-needs-human-review", "あ", "가"), &text).unwrap();
    assert!(!draft.product_eligible);
    let approved = verify_manifest(&manifest("approved", "あ", "가"), &text).unwrap();
    assert!(approved.product_eligible);
}
