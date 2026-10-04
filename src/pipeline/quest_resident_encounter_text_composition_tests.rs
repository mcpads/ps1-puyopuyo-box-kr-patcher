use crate::analysis::{
    QuestResidentTextRun, QuestResidentTextToken, QUEST_RESIDENT_ENCOUNTER_TEXT_ID,
};
use crate::script::QuestTextTerminal;

use super::*;

fn unit_for_rows(rows: &[CompositionRow]) -> QuestResidentTextUnit {
    let digest = rows[0].source_sha256;
    let runs = rows
        .iter()
        .map(|row| QuestResidentTextRun {
            index: row.run_index,
            text_offset: 4 + row.run_index * 8,
            runtime: 0x8001_0004 + row.run_index as u32 * 8,
            source: row.protected_source.clone(),
            raw: vec![1; row.protected_source.chars().count() * 2],
            glyph_occurrences: row.protected_source.chars().count(),
        })
        .collect::<Vec<_>>();
    QuestResidentTextUnit {
        id: QUEST_RESIDENT_ENCOUNTER_TEXT_ID.to_owned(),
        role: QuestResidentTextRole::EncounterMenu,
        record_runtime: 0x8001_0000,
        body_runtime: 0x8001_0004,
        end_runtime: 0x8001_0040,
        record_text_offset: 0,
        body_text_offset: 4,
        end_text_offset: 0x40,
        record_raw: [0x21, 0, 0, 0],
        protected_raw: vec![0; 0x40],
        expected_raw_sha256: digest,
        protected_source: rows
            .iter()
            .map(|row| row.protected_source.as_str())
            .collect(),
        glyph_occurrences: runs.iter().map(|run| run.glyph_occurrences).sum(),
        japanese_glyph_occurrences: 1,
        control_occurrences: 1,
        tokens: Vec::<QuestResidentTextToken>::new(),
        runs,
        terminal: QuestTextTerminal::DeactivateAndAlign,
        runtime_evidence: None,
    }
}

#[test]
fn 네_run을_명시pad와_세후보에_결속한다() {
    let rows = parse_rows(COMPOSITIONS).expect("embedded rows");
    let unit = unit_for_rows(&rows);

    let analysis = bind_rows(rows, &unit).expect("composition binding");

    assert_eq!(analysis.decisions.len(), 4);
    assert_eq!(analysis.human_choice_runs, 3);
    assert_eq!(analysis.composition.runs[0].text, "");
    assert_eq!(analysis.composition.runs[0].leading_pad_cells, 2);
    assert_eq!(analysis.composition.runs[1].text, "등장했다！！");
    assert_eq!(analysis.composition.runs[2].text, "싸운다");
    assert_eq!(analysis.composition.runs[3].text, "도망");
    assert!(analysis.used_glyphs.contains('\u{3000}'));
    assert!(!analysis.translation_approved);
    assert!(!analysis.layout_approved);
    assert!(!analysis.runtime_verified);
    assert!(!analysis.product_eligible);
}

#[test]
fn padding과_사람선택의_계약을_서로_바꾸지_않는다() {
    let missing_candidate = COMPOSITIONS.replacen(
        "등장했다!!\t1\t2\t등장했다!!\t나타났다!!\t출현했다!!",
        "등장했다!!\t1\t2\t등장했다!!\t\t출현했다!!",
        1,
    );
    assert!(parse_rows(&missing_candidate).is_err());

    let authored_padding = COMPOSITIONS.replacen(
        "\t\t2\t0\t\t\t\tmanual-padding",
        "\t공백\t2\t0\t\t\t\tmanual-padding",
        1,
    );
    assert!(parse_rows(&authored_padding).is_err());
}

#[test]
fn source_run_cell수와_개행_drift를_거부한다() {
    let rows = parse_rows(COMPOSITIONS).expect("embedded rows");
    let unit = unit_for_rows(&rows);

    let mut source_drift = rows.clone();
    source_drift[1].protected_source.push('X');
    assert!(bind_rows(source_drift, &unit).is_err());

    let mut cell_drift = rows;
    cell_drift[2].trailing_pad_cells += 1;
    assert!(bind_rows(cell_drift, &unit).is_err());

    assert!(parse_rows(&COMPOSITIONS.replacen("싸운다", "싸운\n다", 1)).is_err());
}
