use super::*;
use crate::script::{QuestItemTextTerminal, QuestItemTextUnit, QuestItemTextUnitAnalysis};

fn source_unit(id: &str, source: &str) -> QuestItemTextUnit {
    QuestItemTextUnit {
        id: id.to_owned(),
        role: QuestItemTextRole::Name,
        table_indices: vec![0],
        record_offset: 0,
        record_runtime: 0,
        pointer_field_offset: 0,
        pointer_field_runtime: 0,
        text_offset: 0,
        text_runtime: 0,
        end_offset: source.len(),
        end_runtime: 0,
        protected_raw: vec![1],
        protected_source: source.to_owned(),
        tokens: Vec::new(),
        terminal: QuestItemTextTerminal::Null,
        glyph_occurrences: source.chars().count(),
        japanese_glyph_occurrences: source.chars().count(),
        control_occurrences: 0,
        runtime_evidence: None,
    }
}

fn source_analysis(units: Vec<QuestItemTextUnit>) -> QuestItemTextUnitAnalysis {
    let names = units.len();
    QuestItemTextUnitAnalysis {
        units,
        table_entries: names,
        unique_records: names,
        name_units: names,
        description_units: 0,
        japanese_units: names,
        glyph_occurrences: 0,
        japanese_glyph_occurrences: 0,
        control_occurrences: 0,
        code_table_candidate_runs: 0,
        name_candidate_runs: 0,
        description_candidate_runs: 0,
        unowned_candidate_runs: 0,
        short_name_units: 0,
        runtime_observed_units: 0,
    }
}

fn draft_rows(rows: &[(&str, &str, &str, &str)]) -> String {
    let body = rows
        .iter()
        .map(|(id, source, korean, status)| {
            format!(
                "{QUEST_ITEM_NAME_DRAFT_SCHEMA_VERSION}\t{id}\t{source}\t{korean}\t{status}\t수동 작성"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{HEADER}\n{body}\n")
}

#[test]
fn 보호원문과이름unit전수를함께검증한다() {
    let source = source_analysis(vec![
        source_unit("item-a", "カギ"),
        source_unit("item-b", "ぷよまん"),
    ]);
    let raw = draft_rows(&[
        ("item-a", "カギ", "열쇠", "draft-needs-independent-review"),
        ("item-b", "ぷよまん", "뿌요만", "draft-needs-human-choice"),
    ]);
    let analysis = audit_drafts(&raw, &source).unwrap();

    assert_eq!(analysis.entries.len(), 2);
    assert_eq!(analysis.draft_needs_independent_review, 1);
    assert_eq!(analysis.draft_needs_human_choice, 1);
    assert!(!analysis.translation_approved);
    assert!(!analysis.layout_approved);
    assert!(!analysis.product_eligible);
}

#[test]
fn 원문drift와누락을거부한다() {
    let source = source_analysis(vec![
        source_unit("item-a", "カギ"),
        source_unit("item-b", "ぷよまん"),
    ]);
    let drift = draft_rows(&[
        ("item-a", "カキ", "열쇠", "draft-needs-independent-review"),
        (
            "item-b",
            "ぷよまん",
            "뿌요만",
            "draft-needs-independent-review",
        ),
    ]);
    assert!(audit_drafts(&drift, &source)
        .unwrap_err()
        .to_string()
        .contains("원문 drift"));

    let missing = draft_rows(&[("item-a", "カギ", "열쇠", "draft-needs-independent-review")]);
    assert!(audit_drafts(&missing, &source)
        .unwrap_err()
        .to_string()
        .contains("분모 불일치"));
}

#[test]
fn 중복id와한국어초안의일본어잔존을거부한다() {
    let duplicate = draft_rows(&[
        ("item-a", "カギ", "열쇠", "draft-needs-independent-review"),
        ("item-a", "カギ", "자물쇠", "draft-needs-independent-review"),
    ]);
    assert!(parse_drafts(&duplicate)
        .unwrap_err()
        .to_string()
        .contains("중복"));

    let japanese = draft_rows(&[(
        "item-a",
        "カギ",
        "열쇠カギ",
        "draft-needs-independent-review",
    )]);
    assert!(parse_drafts(&japanese)
        .unwrap_err()
        .to_string()
        .contains("일본어"));
}
