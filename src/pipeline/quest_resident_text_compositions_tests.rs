use super::*;

fn digest(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn source(unit_id: &str, byte: u8, runs: usize) -> CompositionSource {
    CompositionSource {
        unit_id: unit_id.to_string(),
        source_sha256: digest(byte),
        runs,
    }
}

fn row(unit_id: &str, byte: u8, run_index: usize, text: &str) -> CompositionRow {
    CompositionRow {
        unit_id: unit_id.to_string(),
        source_sha256: digest(byte),
        run: QuestResidentTextRunComposition {
            run_index,
            text: text.to_string(),
            leading_pad_cells: 0,
            trailing_pad_cells: 0,
        },
    }
}

#[test]
fn source_unit_순서로_run을_정렬해_수동composition을_만든다() {
    let sources = [source("start", 1, 2), source("end", 2, 1)];
    let result = bind_rows(
        vec![
            row("end", 2, 0, "끝"),
            row("start", 1, 1, "둘"),
            row("start", 1, 0, "하나"),
        ],
        &sources,
    )
    .unwrap();

    assert_eq!(
        result
            .iter()
            .map(|composition| composition.unit_id.as_str())
            .collect::<Vec<_>>(),
        ["start", "end"]
    );
    assert_eq!(result[0].runs[0].text, "하나");
    assert_eq!(result[0].runs[1].text, "둘");
    assert_eq!(
        result[0].status.label(),
        QUEST_RESIDENT_TEXT_COMPOSITION_STATUS
    );
}

#[test]
fn source_sha와_unit_run_분모가_바뀌면_실패한다() {
    let sources = [source("start", 1, 2)];
    assert!(bind_rows(vec![row("start", 2, 0, "하나")], &sources).is_err());
    assert!(bind_rows(vec![row("missing", 1, 0, "하나")], &sources).is_err());
    assert!(bind_rows(vec![row("start", 1, 0, "하나")], &sources).is_err());
    assert!(bind_rows(
        vec![row("start", 1, 0, "하나"), row("start", 1, 0, "둘")],
        &sources,
    )
    .is_err());
}

#[test]
fn header_status_열수_형식을_엄격히_검사한다() {
    let sha = "01".repeat(32);
    let valid = format!(
        "{HEADER}\nstart\t{sha}\t0\t하나\t0\t1\t{QUEST_RESIDENT_TEXT_COMPOSITION_STATUS}\t수동 조판\n"
    );
    let rows = parse_rows(&valid).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].run.trailing_pad_cells, 1);

    assert!(parse_rows(&valid.replacen(HEADER, "wrong", 1)).is_err());
    assert!(
        parse_rows(&valid.replace(QUEST_RESIDENT_TEXT_COMPOSITION_STATUS, "approved")).is_err()
    );
    assert!(parse_rows(&valid.replace("\t수동 조판", "")).is_err());
}

#[test]
fn 내장composition은_다섯_unit_열한run을_보존한다() {
    let rows = parse_rows(COMPOSITIONS).unwrap();
    let counts = rows.iter().fold(BTreeMap::new(), |mut counts, row| {
        *counts.entry(row.unit_id.as_str()).or_insert(0usize) += 1;
        counts
    });

    assert_eq!(counts.len(), 5);
    assert_eq!(counts.values().sum::<usize>(), 11);
    assert!(rows.iter().all(|row| !row.run.text.is_empty()));
}

#[test]
fn 실제문자와_pad를_정규화한_중복없는_사용집합을_만든다() {
    let compositions = [QuestResidentTextComposition {
        unit_id: "start".to_string(),
        source_sha256: digest(1),
        status: QuestResidentTextCompositionStatus::ManualDraftNeedsGeometry,
        runs: vec![QuestResidentTextRunComposition {
            run_index: 0,
            text: "가, 가".to_string(),
            leading_pad_cells: 1,
            trailing_pad_cells: 0,
        }],
    }];

    let glyphs = quest_resident_text_composition_used_glyphs(&compositions).unwrap();
    assert_eq!(glyphs.chars().collect::<BTreeSet<_>>().len(), 3);
    assert!(glyphs.contains('가'));
    assert!(glyphs.contains('、'));
    assert!(glyphs.contains('\u{3000}'));
}
