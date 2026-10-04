use std::collections::BTreeSet;

use super::*;
use crate::font::QUEST_CODE_TABLE_COUNT;

fn code_table_with(text: &str) -> QuestCodeTable {
    let required = text
        .chars()
        .map(|character| {
            let encoded = encode_sjis(&character.to_string()).unwrap();
            [encoded[0], encoded[1]]
        })
        .collect::<BTreeSet<_>>();
    let mut codes = required.into_iter().collect::<Vec<_>>();
    for high in 0u8..=u8::MAX {
        for low in 0u8..=u8::MAX {
            let pair = [high, low];
            if !codes.contains(&pair) {
                codes.push(pair);
            }
            if codes.len() == QUEST_CODE_TABLE_COUNT {
                return QuestCodeTable::parse(&codes.into_iter().flatten().collect::<Vec<_>>())
                    .unwrap();
            }
        }
    }
    unreachable!("345개 고유 code를 만들 수 있음")
}

#[test]
fn 입력_순서와_무관하게_모든_sequence를_요약한다() {
    let table = code_table_with("アルルドラコ");
    let first = encode_sjis("アルル").unwrap();
    let second = encode_sjis("ドラコ").unwrap();
    let analysis = audit_quest_seq_text_candidates(
        [
            ("B.SEQ", second.as_slice()),
            ("EMPTY.SEQ", &[0, 0][..]),
            ("A.SEQ", first.as_slice()),
        ],
        &table,
    )
    .unwrap();

    assert_eq!(
        analysis
            .summaries
            .iter()
            .map(|summary| summary.sequence.as_str())
            .collect::<Vec<_>>(),
        ["A.SEQ", "B.SEQ", "EMPTY.SEQ"]
    );
    assert_eq!(analysis.summaries.len(), 3);
    assert_eq!(analysis.sequences_with_candidates, 2);
    assert_eq!(analysis.candidates.len(), 2);
    assert_eq!(analysis.source_code_units, 6);
    assert_eq!(analysis.runtime_observed_spans, 0);
}

#[test]
fn 빈이름_중복과_빈입력을_거부한다() {
    let table = code_table_with("アルル");
    let raw = encode_sjis("アルル").unwrap();
    assert!(audit_quest_seq_text_candidates(std::iter::empty(), &table)
        .unwrap_err()
        .to_string()
        .contains("입력이 없음"));
    assert!(
        audit_quest_seq_text_candidates([("", raw.as_slice())], &table)
            .unwrap_err()
            .to_string()
            .contains("이름이 비어")
    );
    assert!(audit_quest_seq_text_candidates(
        [("A.SEQ", raw.as_slice()), ("A.SEQ", raw.as_slice())],
        &table
    )
    .unwrap_err()
    .to_string()
    .contains("입력 중복"));
}

#[test]
fn runtime_span은_sequence_offset_source_raw가_모두_같아야_한다() {
    let raw = encode_sjis("やった～！").unwrap();
    assert!(runtime_observed("TOWN00.SEQ", 0x1B78, "やった～！", &raw));

    let mut changed = raw;
    changed[0] ^= 1;
    assert!(!runtime_observed(
        "TOWN00.SEQ",
        0x1B78,
        "やった～！",
        &changed
    ));
    assert!(!runtime_observed(
        "TOWN10.SEQ",
        0x1B78,
        "やった～！",
        &encode_sjis("やった～！").unwrap()
    ));
}

#[test]
fn 후보_tsv는_원본_raw와_sequence_digest를_보존한다() {
    let table = code_table_with("アルル");
    let raw = encode_sjis("アルル").unwrap();
    let analysis = audit_quest_seq_text_candidates([("A.SEQ", raw.as_slice())], &table).unwrap();
    let report = render_quest_seq_text_candidates_tsv(&analysis);

    assert_eq!(report.lines().count(), 2);
    assert!(report.contains("\t1\tA.SEQ\t"));
    assert!(report.contains("\tQUEST-SEQ-TEXT:A.SEQ:0x0000\t0x0000\t\t"));
    assert!(report.contains(&format!(
        "\t{}\t6\t3\t3\tstatic-code-table-run-candidate\tアルル",
        hex(&raw)
    )));
    assert!(!report.contains('\0'));
}

#[test]
fn 요약_tsv는_후보가_없는_sequence도_오해없는_상태로_보존한다() {
    let table = code_table_with("アルル");
    let raw = encode_sjis("アルル").unwrap();
    let analysis = audit_quest_seq_text_candidates(
        [("TEXT.SEQ", raw.as_slice()), ("EMPTY.SEQ", &[0, 0][..])],
        &table,
    )
    .unwrap();
    let report = render_quest_seq_text_summary_tsv(&analysis);

    assert_eq!(report.lines().count(), 3);
    assert!(report.contains(
        "EMPTY.SEQ\t96a296d224f285c67bee93c30f8a309157f0daa35dc5b87e410b78630a09cfc7\t2\t0\t0\t\
         no-code-table-run-candidate"
    ));
    assert!(report.contains("\tTEXT.SEQ\t"));
    assert!(report.contains("\t1\t3\tstatic-candidates-present"));
}
