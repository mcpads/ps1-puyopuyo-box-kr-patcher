use crate::text::encode_sjis;

use super::*;
use crate::script::{audit_subgame_seq_layouts, audit_subgame_seq_text_candidates};

const ARCHIVE: &str = "/PUYO1/PP1_SEQ.VDD;1";

fn overlay(mut payload: Vec<u8>, base: u32) -> Vec<u8> {
    while !payload.len().is_multiple_of(4) {
        payload.push(0);
    }
    let initialized_len = payload.len() as u32;
    let bss_addr = base + initialized_len;
    for word in [
        0,
        0,
        0,
        base,
        initialized_len,
        bss_addr,
        0x2C,
        bss_addr + 0x30,
        0,
        0x8000,
        0x0080_0000,
    ] {
        payload.extend_from_slice(&word.to_le_bytes());
    }
    payload
}

#[test]
fn exact와_선행_header_pointer를_후보별로_분리한다() {
    let base = 0x801B_5270u32;
    let first = encode_sjis("アルル").unwrap();
    let second = encode_sjis("ドラコ").unwrap();
    let mut payload = vec![0; 64];
    payload[0..4].copy_from_slice(&(base + 16).to_le_bytes());
    payload[4..8].copy_from_slice(&(base + 28).to_le_bytes());
    payload[16..16 + first.len()].copy_from_slice(&first);
    payload[32..32 + second.len()].copy_from_slice(&second);
    let raw = overlay(payload, base);
    let text = audit_subgame_seq_text_candidates([(ARCHIVE, "A.SEQ", raw.as_slice())]).unwrap();
    let layouts = audit_subgame_seq_layouts([(ARCHIVE, "A.SEQ", raw.as_slice())], &text).unwrap();
    let analysis =
        audit_subgame_seq_text_roots([(ARCHIVE, "A.SEQ", raw.as_slice())], &text, &layouts)
            .unwrap();

    assert_eq!(analysis.rows.len(), 2);
    assert_eq!(analysis.exact_candidates, 1);
    assert_eq!(analysis.header_candidates, 1);
    assert_eq!(analysis.none_candidates, 0);
    assert_eq!(analysis.rows[0].roots[0].candidate_delta, 0);
    assert_eq!(analysis.rows[1].roots[0].candidate_delta, 4);
}

#[test]
fn 가까운_pointer가_없으면_쓰기금지_상태를_보고한다() {
    let raw = overlay(encode_sjis("アルル").unwrap(), 0x801B_5270);
    let text = audit_subgame_seq_text_candidates([(ARCHIVE, "A.SEQ", raw.as_slice())]).unwrap();
    let layouts = audit_subgame_seq_layouts([(ARCHIVE, "A.SEQ", raw.as_slice())], &text).unwrap();
    let analysis =
        audit_subgame_seq_text_roots([(ARCHIVE, "A.SEQ", raw.as_slice())], &text, &layouts)
            .unwrap();
    let report = render_subgame_seq_text_roots_tsv(&analysis);

    assert_eq!(analysis.none_candidates, 1);
    assert!(report.contains("no-nearby-internal-pointer"));
    assert!(report.contains("preserve-no-write-until-record-consumer-and-control-contract"));
}

#[test]
fn 중복입력과_text_layout_drift를_거부한다() {
    let raw = overlay(encode_sjis("アルル").unwrap(), 0x801B_5270);
    let text = audit_subgame_seq_text_candidates([(ARCHIVE, "A.SEQ", raw.as_slice())]).unwrap();
    let layouts = audit_subgame_seq_layouts([(ARCHIVE, "A.SEQ", raw.as_slice())], &text).unwrap();

    assert!(audit_subgame_seq_text_roots(
        [
            (ARCHIVE, "A.SEQ", raw.as_slice()),
            (ARCHIVE, "A.SEQ", raw.as_slice()),
        ],
        &text,
        &layouts,
    )
    .is_err());
    assert!(audit_subgame_seq_text_roots(std::iter::empty(), &text, &layouts).is_err());
}
