use crate::analysis::{QuestBattleNameEntry, QuestBattleNameTableAnalysis};

use super::*;

fn table_for_rows(rows: &[CompositionRow]) -> QuestBattleNameTableAnalysis {
    let entries = rows
        .iter()
        .map(|row| QuestBattleNameEntry {
            id: row.unit_id.clone(),
            index: row.index,
            branch_pointer_runtime: 0x8001_0000 + row.index as u32 * 4,
            stream_runtime: 0x8002_0000 + row.index as u32 * 0x20,
            name_assignment_runtime: 0x8002_0000 + row.index as u32 * 0x20,
            name_runtime: 0x8003_0000 + row.index as u32 * 0x20,
            slot_end_assignment_runtime: 0x8002_0008 + row.index as u32 * 0x20,
            slot_end_runtime: 0x8003_0000
                + row.index as u32 * 0x20
                + u32::try_from((row.slot_capacity_glyphs + 1) * 2).unwrap(),
            name_text_offset: row.index * 0x20,
            slot_end_text_offset: row.index * 0x20 + (row.slot_capacity_glyphs + 1) * 2,
            source: row.protected_source.clone(),
            source_raw: vec![1, 0],
            protected_slot: vec![0; (row.slot_capacity_glyphs + 1) * 2],
            protected_slot_sha256: row.protected_slot_sha256,
            glyph_occurrences: row.protected_source.chars().count(),
            slot_capacity_glyphs: row.slot_capacity_glyphs,
        })
        .collect::<Vec<_>>();
    QuestBattleNameTableAnalysis {
        selector_runtime: 0x8001_0000,
        selector_field_offset: 0x34,
        pointer_table_runtime: 0x8001_0004,
        pointer_table_end_runtime: 0x8002_0000,
        fixed_text_state_index: 0x38,
        fixed_text_state_runtime: 0x800D_B7A0,
        unique_sources: entries
            .iter()
            .map(|entry| entry.source.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        glyph_occurrences: entries.iter().map(|entry| entry.glyph_occurrences).sum(),
        slot_bytes: entries.iter().map(|entry| entry.protected_slot.len()).sum(),
        entries,
    }
}

#[test]
fn 내장composition은_59개_물리slot과_두_사람선택을_보존한다() {
    let rows = parse_rows(COMPOSITIONS).expect("embedded rows");
    let table = table_for_rows(&rows);

    let analysis = bind_rows(
        rows,
        parse_particle_pronunciations(PARTICLE_PRONUNCIATIONS).expect("particle pronunciations"),
        &table,
    )
    .expect("composition binding");

    assert_eq!(analysis.compositions.len(), 59);
    assert_eq!(analysis.selected_drafts, 57);
    assert_eq!(analysis.needs_human_choice, 2);
    assert!(analysis.used_glyphs.contains('밴'));
    assert!(analysis.used_glyphs.contains('T'));
    assert!(analysis.used_glyphs.contains('4'));
    assert!(analysis.used_glyphs.contains('가'));
    assert!(analysis.used_glyphs.contains('이'));
    assert_eq!(analysis.compositions[1].pronunciation_tail, '티');
    assert_eq!(analysis.compositions[1].subject_particle, '가');
    assert_eq!(analysis.compositions[13].development_text, "가지동자");
    assert_eq!(analysis.compositions[13].subject_particle, '가');
    assert_eq!(analysis.compositions[38].subject_particle, '이');
    assert_eq!(analysis.compositions[56].pronunciation_tail, '사');
    assert_eq!(analysis.compositions[56].subject_particle, '가');
    assert!(!analysis.translation_approved);
    assert!(!analysis.layout_approved);
    assert!(!analysis.runtime_verified);
    assert!(!analysis.product_eligible);
}

#[test]
fn 사람선택_행은_서로다른_세안과_명시적_개발안을_요구한다() {
    let input = COMPOSITIONS.replacen(
        "쓰지기리 몰\t쓰지기리 몰\t길목베기 몰\t노상검객 몰",
        "쓰지기리 몰\t쓰지기리 몰\t\t",
        1,
    );
    assert!(parse_rows(&input).is_err());

    let duplicate = COMPOSITIONS.replacen(
        "쓰지기리 몰\t쓰지기리 몰\t길목베기 몰\t노상검객 몰",
        "쓰지기리 몰\t쓰지기리 몰\t쓰지기리 몰\t노상검객 몰",
        1,
    );
    assert!(parse_rows(&duplicate).is_err());
}

#[test]
fn source_용량_일본어와_자동개행_drift를_거부한다() {
    let rows = parse_rows(COMPOSITIONS).expect("embedded rows");
    let table = table_for_rows(&rows);
    let pronunciations =
        parse_particle_pronunciations(PARTICLE_PRONUNCIATIONS).expect("particle pronunciations");

    let mut source_drift = rows.clone();
    source_drift[0].protected_source.push('X');
    assert!(bind_rows(source_drift, pronunciations.clone(), &table).is_err());

    let mut capacity_drift = rows.clone();
    capacity_drift[0].slot_capacity_glyphs = 1;
    assert!(bind_rows(capacity_drift, pronunciations, &table).is_err());

    assert!(parse_rows(&COMPOSITIONS.replacen("드라코", "ドラコ", 1)).is_err());
    assert!(parse_rows(&COMPOSITIONS.replacen("드라코", "드라\n코", 1)).is_err());
}

#[test]
fn 같은_원문의_물리사본은_같은_개발안을_요구한다() {
    let rows = parse_rows(COMPOSITIONS).expect("embedded rows");
    let table = table_for_rows(&rows);
    let mut drift = rows;
    let duplicate = drift
        .iter_mut()
        .find(|row| row.index == 37)
        .expect("second Draco");
    duplicate.development_text = "드래코".to_owned();
    duplicate.candidates = vec!["드래코".to_owned()];

    assert!(bind_rows(
        drift,
        parse_particle_pronunciations(PARTICLE_PRONUNCIATIONS).expect("particle pronunciations"),
        &table,
    )
    .is_err());
}

#[test]
fn 비한글_끝표기는_수동_발음끝음절과_정확히_결속한다() {
    let rows = parse_rows(COMPOSITIONS).expect("embedded rows");
    let table = table_for_rows(&rows);
    let pronunciations =
        parse_particle_pronunciations(PARTICLE_PRONUNCIATIONS).expect("particle pronunciations");

    let missing = pronunciations
        .iter()
        .filter(|row| !row.development_text.ends_with('T'))
        .cloned()
        .collect();
    assert!(bind_rows(rows.clone(), missing, &table).is_err());

    let mut stale = pronunciations;
    stale[0].development_text = "스켈톤티".to_owned();
    assert!(bind_rows(rows, stale, &table).is_err());
}
