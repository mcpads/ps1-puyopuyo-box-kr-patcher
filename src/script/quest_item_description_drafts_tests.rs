use super::*;
use crate::script::{
    QuestItemTextTerminal, QuestItemTextToken, QuestItemTextUnit, QuestItemTextUnitAnalysis,
    QuestTextTerminal,
};

fn control(index: usize, raw: &[u8]) -> QuestItemTextToken {
    QuestItemTextToken {
        index,
        sequence_offset: index * 2,
        runtime: 0,
        raw: raw.to_vec(),
        kind: QuestItemTextTokenKind::Control,
        source: None,
        control_code: Some(raw[0]),
        control_meaning: None,
    }
}

fn source_unit(id: &str, source: &str, controls: &[&[u8]]) -> QuestItemTextUnit {
    QuestItemTextUnit {
        id: id.to_owned(),
        role: QuestItemTextRole::Description,
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
        tokens: controls
            .iter()
            .enumerate()
            .map(|(index, raw)| control(index, raw))
            .collect(),
        terminal: QuestItemTextTerminal::Text(QuestTextTerminal::NotifyParentAndRelease),
        glyph_occurrences: source.chars().count(),
        japanese_glyph_occurrences: source.chars().count(),
        control_occurrences: controls.len(),
        runtime_evidence: None,
    }
}

fn source_analysis(units: Vec<QuestItemTextUnit>) -> QuestItemTextUnitAnalysis {
    let descriptions = units.len();
    QuestItemTextUnitAnalysis {
        units,
        table_entries: descriptions,
        unique_records: descriptions,
        name_units: 0,
        description_units: descriptions,
        japanese_units: descriptions,
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
        .map(|(id, source, annotated, status)| {
            format!(
                "{QUEST_ITEM_DESCRIPTION_DRAFT_SCHEMA_VERSION}\t{id}\t{source}\t{annotated}\t{status}\t수동 작성"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{HEADER}\n{body}\n")
}

#[test]
fn 보호원문과설명unit전수및control순서를함께검증한다() {
    let controls: [&[u8]; 3] = [
        &[0x0B, 0xFF, 0, 0],
        &[0x04, 0xFF],
        &[0x1A, 0xFF, 0, 0, 0, 0],
    ];
    let source = source_analysis(vec![source_unit("item-a", "원문", &controls)]);
    let raw = draft_rows(&[(
        "item-a",
        "원문",
        "{FF0B:0BFF0000}첫째{FF04:04FF}둘째{FF1A:1AFF00000000}",
        "draft-needs-independent-review",
    )]);
    let analysis = audit_drafts(&raw, &source).unwrap();

    assert_eq!(analysis.entries.len(), 1);
    assert_eq!(analysis.entries[0].draft_korean, "첫째둘째");
    assert_eq!(
        analysis.entries[0].parts,
        vec![
            QuestItemDescriptionDraftPart::Control(controls[0].to_vec()),
            QuestItemDescriptionDraftPart::Literal("첫째".to_owned()),
            QuestItemDescriptionDraftPart::Control(controls[1].to_vec()),
            QuestItemDescriptionDraftPart::Literal("둘째".to_owned()),
            QuestItemDescriptionDraftPart::Control(controls[2].to_vec()),
        ]
    );
    assert_eq!(analysis.control_occurrences, 3);
    assert_eq!(analysis.manual_line_breaks, 1);
    assert!(!analysis.translation_approved);
    assert!(!analysis.layout_approved);
    assert!(!analysis.product_eligible);
}

#[test]
fn 한국어사이의수동개행이동은허용하지만control변조는거부한다() {
    let controls: [&[u8]; 3] = [
        &[0x0B, 0xFF, 0, 0],
        &[0x04, 0xFF],
        &[0x1A, 0xFF, 0, 0, 0, 0],
    ];
    let source = source_analysis(vec![source_unit("item-a", "원문", &controls)]);
    let moved = draft_rows(&[(
        "item-a",
        "원문",
        "{FF0B:0BFF0000}첫{FF04:04FF}째둘째{FF1A:1AFF00000000}",
        "draft-needs-independent-review",
    )]);
    audit_drafts(&moved, &source).unwrap();

    let drift = draft_rows(&[(
        "item-a",
        "원문",
        "{FF0B:0BFF0000}첫째{FF0C:0CFF0000}둘째{FF1A:1AFF00000000}",
        "draft-needs-independent-review",
    )]);
    assert!(audit_drafts(&drift, &source)
        .unwrap_err()
        .to_string()
        .contains("control raw 순서 drift"));
}

#[test]
fn 보호원문drift와분모누락을거부한다() {
    let controls: [&[u8]; 2] = [&[0x0B, 0xFF, 0, 0], &[0x1A, 0xFF, 0, 0, 0, 0]];
    let source = source_analysis(vec![
        source_unit("item-a", "원문A", &controls),
        source_unit("item-b", "원문B", &controls),
    ]);
    let drift = draft_rows(&[
        (
            "item-a",
            "원문X",
            "{FF0B:0BFF0000}하나{FF1A:1AFF00000000}",
            "draft-needs-independent-review",
        ),
        (
            "item-b",
            "원문B",
            "{FF0B:0BFF0000}둘{FF1A:1AFF00000000}",
            "draft-needs-independent-review",
        ),
    ]);
    assert!(audit_drafts(&drift, &source)
        .unwrap_err()
        .to_string()
        .contains("원문 drift"));

    let missing = draft_rows(&[(
        "item-a",
        "원문A",
        "{FF0B:0BFF0000}하나{FF1A:1AFF00000000}",
        "draft-needs-independent-review",
    )]);
    assert!(audit_drafts(&missing, &source)
        .unwrap_err()
        .to_string()
        .contains("분모 불일치"));
}

#[test]
fn 잘못된annotation과일본어잔존및중복id를거부한다() {
    let invalid_label = draft_rows(&[(
        "item-a",
        "原文",
        "{FF05:04FF}한국어",
        "draft-needs-independent-review",
    )]);
    assert!(parse_drafts(&invalid_label)
        .unwrap_err()
        .to_string()
        .contains("label/raw 불일치"));

    let japanese = draft_rows(&[(
        "item-a",
        "原文",
        "{FF04:04FF}한국어カナ",
        "draft-needs-independent-review",
    )]);
    assert!(parse_drafts(&japanese)
        .unwrap_err()
        .to_string()
        .contains("일본어"));

    let duplicate = draft_rows(&[
        (
            "item-a",
            "原文",
            "{FF04:04FF}하나",
            "draft-needs-independent-review",
        ),
        (
            "item-a",
            "原文",
            "{FF04:04FF}둘",
            "draft-needs-human-choice",
        ),
    ]);
    assert!(parse_drafts(&duplicate)
        .unwrap_err()
        .to_string()
        .contains("중복"));
}
