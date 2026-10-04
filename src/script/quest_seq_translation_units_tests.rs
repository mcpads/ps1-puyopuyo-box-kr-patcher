use super::*;
use crate::font::{
    QuestCodeTable, QUEST_CODE_TABLE_COUNT, QUEST_DOUBLE_EXCLAMATION_CODE,
    QUEST_DOUBLE_QUESTION_CODE,
};
use crate::script::audit_quest_seq_text_extraction;

fn code_table_with(codes: &[[u8; 2]]) -> QuestCodeTable {
    let mut table = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| {
            let value = u16::try_from(index).expect("test index");
            value.to_le_bytes()
        })
        .collect::<Vec<_>>();
    for (index, code) in codes.iter().copied().enumerate() {
        table[index] = code;
    }
    let raw = table.into_iter().flatten().collect::<Vec<_>>();
    QuestCodeTable::parse(&raw).expect("test code table")
}

#[test]
fn 일본어_stream만_번역후보이고_영문은_참고로_남긴다() {
    let code_table = code_table_with(&[[0x82, 0xA0], [0x82, 0x60]]);
    let japanese = [
        0x21, 0, 0, 0, 0x82, 0xA0, 0x04, 0xFF, 0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0,
    ];
    let latin = [
        0x21, 0, 0, 0, 0x82, 0x60, 0x82, 0x60, 0x03, 0xFF, 0, 0, 0, 0, 0, 0,
    ];
    let extraction = audit_quest_seq_text_extraction(
        [
            ("JP.SEQ", japanese.as_slice()),
            ("LATIN.SEQ", latin.as_slice()),
        ],
        &code_table,
    )
    .unwrap();
    let analysis = audit_quest_seq_translation_units(&extraction).unwrap();

    assert_eq!(analysis.japanese_translation_candidates, 1);
    assert_eq!(analysis.non_japanese_reference_units, 1);
    assert_eq!(
        analysis.units[0].policy,
        QuestSeqTranslationUnitPolicy::JapaneseTranslationCandidate
    );
    assert_eq!(analysis.units[0].protected_source, "ああ");
    assert!(analysis.units[0]
        .control_signature
        .contains("1:FF04:04ff:AdvanceTextLine"));
    assert_eq!(
        analysis.units[1].policy,
        QuestSeqTranslationUnitPolicy::NonJapaneseReference
    );
    assert_eq!(analysis.units[1].protected_source, "ＡＡ");
}

#[test]
fn control_raw는_원문과_분리하고_tsv에서_보호한다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let raw = [
        0x21, 0, 0, 0, 0x82, 0xA0, 0x04, 0xFF, 0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0,
    ];
    let extraction =
        audit_quest_seq_text_extraction([("TEST.SEQ", raw.as_slice())], &code_table).unwrap();
    let analysis = audit_quest_seq_translation_units(&extraction).unwrap();
    let report = render_quest_seq_translation_units_tsv(&analysis);

    assert!(report.contains("\tああ\t"));
    assert!(report
        .contains("1:FF04:04ff:AdvanceTextLine,3:FF03:03ff00000000:DeactivateInterpreterAndAlign"));
    assert!(!analysis.units[0].protected_source.contains("FF04"));
}

#[test]
fn stream_id_중복은_거부하고_미등록_glyph는_raw로_보호한다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let raw = [0x21, 0, 0, 0, 0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0];
    let extraction =
        audit_quest_seq_text_extraction([("TEST.SEQ", raw.as_slice())], &code_table).unwrap();

    let mut duplicate = extraction.clone();
    duplicate.streams.push(duplicate.streams[0].clone());
    assert!(audit_quest_seq_translation_units(&duplicate)
        .unwrap_err()
        .to_string()
        .contains("ID 중복"));

    let raw_code_table = code_table_with(&[[0x80, 0x80]]);
    let raw_stream = [0x21, 0, 0, 0, 0x80, 0x80, 0x03, 0xFF, 0, 0, 0, 0];
    let raw_extraction =
        audit_quest_seq_text_extraction([("RAW.SEQ", raw_stream.as_slice())], &raw_code_table)
            .unwrap();
    let raw_analysis = audit_quest_seq_translation_units(&raw_extraction).unwrap();
    assert_eq!(
        raw_analysis.units[0].policy,
        QuestSeqTranslationUnitPolicy::ProtectedRawReview
    );
    assert_eq!(raw_analysis.units[0].protected_source, "<RAW:8080>");
    assert_eq!(raw_analysis.units[0].protected_raw_glyph_occurrences, 1);
}

#[test]
fn 전용_두_문자_cell이_섞인_일본어_stream도_번역_후보다() {
    let code_table = code_table_with(&[
        [0x82, 0xA0],
        QUEST_DOUBLE_EXCLAMATION_CODE,
        QUEST_DOUBLE_QUESTION_CODE,
    ]);
    let raw = [
        0x21, 0, 0, 0, 0x82, 0xA0, 0x21, 0x21, 0x3F, 0x3F, 0x03, 0xFF, 0, 0, 0, 0,
    ];
    let extraction =
        audit_quest_seq_text_extraction([("TEST.SEQ", raw.as_slice())], &code_table).unwrap();
    let analysis = audit_quest_seq_translation_units(&extraction).unwrap();

    assert_eq!(analysis.units[0].protected_source, "あ‼⁇");
    assert_eq!(analysis.units[0].protected_raw_glyph_occurrences, 0);
    assert_eq!(
        analysis.units[0].policy,
        QuestSeqTranslationUnitPolicy::JapaneseTranslationCandidate
    );
}

#[test]
fn 입력_stream_순서와_무관하게_tsv가_결정적이다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let a = [0x21, 0, 0, 0, 0x82, 0xA0, 0x00, 0xFF, 0, 0, 0, 0];
    let b = [0x21, 0, 0, 0, 0x82, 0xA0, 0x1A, 0xFF, 0, 0, 0, 0];
    let extraction = audit_quest_seq_text_extraction(
        [("B.SEQ", b.as_slice()), ("A.SEQ", a.as_slice())],
        &code_table,
    )
    .unwrap();
    let mut reversed = extraction.clone();
    reversed.streams.reverse();
    let left = audit_quest_seq_translation_units(&extraction).unwrap();
    let right = audit_quest_seq_translation_units(&reversed).unwrap();
    assert_eq!(
        render_quest_seq_translation_units_tsv(&left),
        render_quest_seq_translation_units_tsv(&right)
    );
}
