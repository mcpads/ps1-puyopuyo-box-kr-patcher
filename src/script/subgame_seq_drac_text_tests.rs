use super::*;

#[test]
fn sjis_span은_control_byte사이의_한자_only_fragment도_보존한다() {
    let raw = [
        0x06, 0x0C, 0x0A, 0x02, 0x8F, 0x9F, 0x95, 0x89, 0x81, 0x48, 0x73, 0x73, 0x77, 0x0C, 0x82,
        0xDC, 0x81, 0x41, 0x82, 0xDC, 0x82, 0xB3, 0x82, 0xA9,
    ];
    let spans = scan_sjis_spans(&raw, 0x1000);
    assert_eq!(
        spans
            .iter()
            .map(|span| (span.offset, span.decoded.as_str()))
            .collect::<Vec<_>>(),
        vec![(0x1004, "勝負？"), (0x100E, "ま、まさか")]
    );
}

#[test]
fn report상태는_repertoire와_message를_번역승인으로_승격하지_않는다() {
    let repertoire = SubgameSeqDracTextRow {
        scope: "TEST",
        archive: "/TEST.VDD;1",
        sequence: "TEST.SEQ",
        kind: SubgameSeqDracTextKind::Repertoire,
        index: 0,
        pointer_field_offset: 1,
        pointer_field_runtime: 2,
        record_offset: 3,
        record_runtime: 4,
        raw: vec![0x82, 0xA0],
        raw_sha256: Sha256::digest([0x82, 0xA0]).into(),
        spans: vec![SubgameSeqDracSjisSpan {
            offset: 3,
            raw: vec![0x82, 0xA0],
            decoded: "あ".to_string(),
        }],
        protected_bytes: 0,
        kana_candidate_overlaps: 1,
        missed_sjis_spans: 0,
    };
    let mut message = repertoire.clone();
    message.kind = SubgameSeqDracTextKind::Message;
    message.index = 1;
    let report = render_subgame_seq_drac_text_tsv(&SubgameSeqDracTextAnalysis {
        rows: vec![repertoire, message],
        message_records: 1,
        message_spans: 1,
        message_code_units: 1,
        message_protected_bytes: 0,
        repertoires: 1,
        repertoire_code_units: 1,
        kana_candidates_in_messages: 1,
        kana_candidates_in_repertoires: 1,
        missed_message_spans: 0,
    });
    assert!(report.contains("structural-repertoire-not-translation"));
    assert!(report.contains("pointer-rooted-message-consumer-pending"));
}
