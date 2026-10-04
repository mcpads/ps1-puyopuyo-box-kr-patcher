use sha2::{Digest, Sha256};

use super::*;
use crate::script::SubgameSeqDracSjisSpan;
use crate::text::encode_sjis;

const REPERTOIRE: &str =
    "あいうおかきけこはふまもよりれをテトン「」、！？地獄・　　　　　さしたてとなにのんっゃじばぷコスー前通勝負美少女";

fn message_record(index: usize, text: &str) -> Vec<u8> {
    let columns = u8::try_from(text.chars().count()).unwrap();
    let index = u8::try_from(index).unwrap();
    let mut raw = vec![index, 1, columns, 1];
    raw.extend_from_slice(&encode_sjis(text).unwrap());
    raw.extend_from_slice(&TERMINATOR);
    raw
}

fn row(
    scope: &'static str,
    kind: SubgameSeqDracTextKind,
    index: usize,
    raw: Vec<u8>,
) -> SubgameSeqDracTextRow {
    let spans = if kind == SubgameSeqDracTextKind::Message {
        vec![SubgameSeqDracSjisSpan {
            offset: 4,
            raw: raw[4..raw.len() - 2].to_vec(),
            decoded: decode_sjis(&raw[4..raw.len() - 2]).unwrap(),
        }]
    } else {
        vec![SubgameSeqDracSjisSpan {
            offset: 0,
            raw: raw.clone(),
            decoded: decode_sjis(&raw).unwrap(),
        }]
    };
    SubgameSeqDracTextRow {
        scope,
        archive: if scope == "PUYO1" { "PP1" } else { "PP2" },
        sequence: "DRAC_SEQ.SEQ",
        kind,
        index,
        pointer_field_offset: 0,
        pointer_field_runtime: 0,
        record_offset: 0,
        record_runtime: 0,
        raw_sha256: Sha256::digest(&raw).into(),
        raw,
        spans,
        protected_bytes: 0,
        kana_candidate_overlaps: 0,
        missed_sjis_spans: 0,
    }
}

fn test_analysis() -> SubgameSeqDracTextAnalysis {
    let repertoire = encode_sjis(REPERTOIRE).unwrap();
    assert_eq!(repertoire.len(), BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_BYTES);
    let records = (1..=5)
        .map(|index| message_record(index, "お前っ！"))
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for scope in ["PUYO1", "PUYO2"] {
        rows.push(row(
            scope,
            SubgameSeqDracTextKind::Repertoire,
            0,
            repertoire.clone(),
        ));
        for (index, record) in records.iter().enumerate() {
            rows.push(row(
                scope,
                SubgameSeqDracTextKind::Message,
                index + 1,
                record.clone(),
            ));
        }
    }
    SubgameSeqDracTextAnalysis {
        rows,
        message_records: 10,
        message_spans: 10,
        message_code_units: 40,
        message_protected_bytes: 0,
        repertoires: 2,
        repertoire_code_units: 112,
        kana_candidates_in_messages: 0,
        kana_candidates_in_repertoires: 0,
        missed_message_spans: 0,
    }
}

#[test]
fn 한글은_고유pair에_배정하고_기존_문장부호는_보존한다() {
    let analysis = test_analysis();
    let translations = [
        SubgameSeqDracTranslation {
            index: 1,
            spans: &["가！"],
        },
        SubgameSeqDracTranslation {
            index: 2,
            spans: &["나？"],
        },
        SubgameSeqDracTranslation {
            index: 3,
            spans: &["다！"],
        },
        SubgameSeqDracTranslation {
            index: 4,
            spans: &["가나"],
        },
        SubgameSeqDracTranslation {
            index: 5,
            spans: &["다가"],
        },
    ];
    let plan = plan_box_subgame_seq_drac_translation(&translations, &analysis).unwrap();
    assert_eq!(plan.source_unique_pairs, 52);
    assert_eq!(plan.source_duplicate_slots, 4);
    assert_eq!(plan.retained_source_characters, 2);
    assert_eq!(
        plan.glyph_mappings
            .iter()
            .map(|mapping| (
                mapping.glyph,
                mapping.repertoire_index,
                mapping.cell_index()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("가".chars().next().unwrap(), 0, 30),
            ('나', 1, 31),
            ('다', 2, 32)
        ]
    );
    assert_eq!(&plan.replacement_records[0][..4], &[1, 1, 4, 1]);
    assert_eq!(plan.replacement_records[0].len(), 10);
    assert!(plan.used_bytes <= BOX_SUBGAME_SEQ_DRAC_MESSAGE_CAPACITY_BYTES);
}

#[test]
fn 일본어잔존과_미등록문자와_폭초과를_거부한다() {
    let analysis = test_analysis();
    let base = [
        SubgameSeqDracTranslation {
            index: 1,
            spans: &["가"],
        },
        SubgameSeqDracTranslation {
            index: 2,
            spans: &["나"],
        },
        SubgameSeqDracTranslation {
            index: 3,
            spans: &["다"],
        },
        SubgameSeqDracTranslation {
            index: 4,
            spans: &["라"],
        },
        SubgameSeqDracTranslation {
            index: 5,
            spans: &["마"],
        },
    ];

    let mut japanese = base;
    japanese[0].spans = &["가あ"];
    assert!(plan_box_subgame_seq_drac_translation(&japanese, &analysis)
        .unwrap_err()
        .to_string()
        .contains("일본어 문자가 남음"));

    let mut unknown = base;
    unknown[0].spans = &["가🙂"];
    assert!(plan_box_subgame_seq_drac_translation(&unknown, &analysis)
        .unwrap_err()
        .to_string()
        .contains("mapping이 없는 문자"));

    let mut too_wide = base;
    too_wide[0].spans = &["가나다라마"];
    assert!(plan_box_subgame_seq_drac_translation(&too_wide, &analysis)
        .unwrap_err()
        .to_string()
        .contains("header 열 상한 초과"));
}

#[test]
fn 두_사본의_원문_drift와_span분모를_거부한다() {
    let mut analysis = test_analysis();
    analysis
        .rows
        .iter_mut()
        .find(|row| row.scope == "PUYO2" && row.kind == SubgameSeqDracTextKind::Message)
        .unwrap()
        .raw[4] ^= 1;
    let translations = [
        SubgameSeqDracTranslation {
            index: 1,
            spans: &["가"],
        },
        SubgameSeqDracTranslation {
            index: 2,
            spans: &["나"],
        },
        SubgameSeqDracTranslation {
            index: 3,
            spans: &["다"],
        },
        SubgameSeqDracTranslation {
            index: 4,
            spans: &["라"],
        },
        SubgameSeqDracTranslation {
            index: 5,
            spans: &["마"],
        },
    ];
    assert!(
        plan_box_subgame_seq_drac_translation(&translations, &analysis)
            .unwrap_err()
            .to_string()
            .contains("두 DRAC 사본")
    );

    let analysis = test_analysis();
    let mut wrong_spans = translations;
    wrong_spans[0].spans = &["가", "나"];
    assert!(
        plan_box_subgame_seq_drac_translation(&wrong_spans, &analysis)
            .unwrap_err()
            .to_string()
            .contains("span 수 불일치")
    );
}
