use crate::text::encode_sjis;

use super::*;

const TEST_ARCHIVE: &str = "/PUYO1/PP1_SEQ.VDD;1";

#[test]
fn 입력_순서와_무관하게_후보없는_sequence도_보존한다() {
    let first = encode_sjis("アルル").unwrap();
    let second = encode_sjis("ドラコ").unwrap();
    let analysis = audit_subgame_seq_text_candidates([
        (TEST_ARCHIVE, "B.SEQ", second.as_slice()),
        (TEST_ARCHIVE, "EMPTY.SEQ", &[0, 0][..]),
        (TEST_ARCHIVE, "A.SEQ", first.as_slice()),
    ])
    .unwrap();

    assert_eq!(
        analysis
            .summaries
            .iter()
            .map(|summary| summary.sequence.as_str())
            .collect::<Vec<_>>(),
        ["A.SEQ", "B.SEQ", "EMPTY.SEQ"]
    );
    assert_eq!(analysis.candidates.len(), 2);
    assert_eq!(analysis.files_with_candidates, 2);
    assert_eq!(analysis.code_units, 6);
    assert_eq!(analysis.strict_candidates, 2);
}

#[test]
fn 미등록_archive와_중복과_빈입력을_거부한다() {
    let raw = encode_sjis("アルル").unwrap();
    assert!(audit_subgame_seq_text_candidates(std::iter::empty()).is_err());
    assert!(
        audit_subgame_seq_text_candidates([("/UNKNOWN.VDD;1", "A.SEQ", raw.as_slice())]).is_err()
    );
    assert!(audit_subgame_seq_text_candidates([
        (TEST_ARCHIVE, "A.SEQ", raw.as_slice()),
        (TEST_ARCHIVE, "A.SEQ", raw.as_slice()),
    ])
    .is_err());
}

#[test]
fn heuristic_only를_엄격한_decode와_분리한다() {
    let raw = [0x81, 0xAD, 0x82, 0xA0, 0x82, 0xA2];
    let analysis = audit_subgame_seq_text_candidates([(TEST_ARCHIVE, "A.SEQ", &raw[..])])
        .expect("휴리스틱 후보 자체는 보존한다");

    assert_eq!(analysis.candidates.len(), 1);
    assert_eq!(analysis.strict_candidates, 0);
    assert_eq!(analysis.heuristic_only_candidates, 1);
    assert!(analysis.candidates[0].decoded.is_none());
}

#[test]
fn 두_report는_runtime과_patch경계를_명시한다() {
    let raw = encode_sjis("アルル").unwrap();
    let analysis =
        audit_subgame_seq_text_candidates([(TEST_ARCHIVE, "A.SEQ", raw.as_slice())]).unwrap();
    let candidates = render_subgame_seq_text_candidates_tsv(&analysis);
    let summaries = render_subgame_seq_text_summary_tsv(&analysis);

    assert!(candidates.contains("SUBGAME-SEQ-TEXT:PUYO1:A.SEQ:0x0000"));
    assert!(candidates.contains("\tunverified\tunknown-subgame-seq-text-consumer\t"));
    assert!(candidates.contains("preserve-no-write-until-consumer-and-control-contract"));
    assert!(summaries.contains("\tstatic-candidates-present\n"));
}
