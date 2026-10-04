use super::*;
use crate::font::QUEST_CODE_TABLE_BYTES;

fn code_table(codes: &[[u8; 2]]) -> QuestCodeTable {
    let mut raw = Vec::with_capacity(QUEST_CODE_TABLE_BYTES);
    raw.extend_from_slice(codes.as_flattened());
    for index in codes.len()..QUEST_CODE_TABLE_BYTES / 2 {
        raw.extend_from_slice(&[0x80 | ((index >> 8) as u8), index as u8]);
    }
    QuestCodeTable::parse(&raw).expect("고유 code table")
}

fn synthetic() -> (Vec<u8>, QuestCodeTable) {
    let base = 0x8010_0000u32;
    let name = [0x83, 0x4A, 0x83, 0x4D]; // カギ
    let description = [
        0x0B, 0xFF, 0, 0, // glyph delay
        0x82, 0xA0, // あ
        0x82, 0xA2, // い
        0x1A, 0xFF, 0, 0, // notify + release
    ];
    let mut raw = vec![0u8; 0x80];
    let record_offset = 8usize;
    raw[..4].copy_from_slice(&(base + record_offset as u32).to_le_bytes());
    raw[4..8].copy_from_slice(&(base + record_offset as u32).to_le_bytes());
    let name_offset = 0x30usize;
    let description_offset = 0x40usize;
    raw[record_offset + 12..record_offset + 16]
        .copy_from_slice(&(base + name_offset as u32).to_le_bytes());
    raw[record_offset + 16..record_offset + 20]
        .copy_from_slice(&(base + description_offset as u32).to_le_bytes());
    raw[name_offset..name_offset + name.len()].copy_from_slice(&name);
    raw[description_offset..description_offset + description.len()].copy_from_slice(&description);
    (
        raw,
        code_table(&[[0x83, 0x4A], [0x83, 0x4D], [0x82, 0xA0], [0x82, 0xA2]]),
    )
}

#[test]
fn 중복_table_entry는_한_record의_이름과_설명으로_접는다() {
    let (raw, code_table) = synthetic();
    let analysis = audit_quest_item_text_units(&raw, 0x8010_0000, 2, &code_table).unwrap();

    assert_eq!(analysis.table_entries, 2);
    assert_eq!(analysis.unique_records, 1);
    assert_eq!(analysis.units.len(), 2);
    assert!(analysis
        .units
        .iter()
        .all(|unit| unit.table_indices == [0, 1]));
    let name = analysis
        .units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Name)
        .unwrap();
    assert_eq!(name.protected_source, "カギ");
    assert_eq!(name.terminal, QuestItemTextTerminal::Null);
    let description = analysis
        .units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Description)
        .unwrap();
    assert_eq!(description.protected_source, "あい");
    assert_eq!(
        description.terminal,
        QuestItemTextTerminal::Text(QuestTextTerminal::NotifyParentAndRelease)
    );
}

#[test]
fn pointer와_glyph와_terminal_drift를_실패폐쇄한다() {
    let (raw, code_table) = synthetic();

    let mut outside = raw.clone();
    outside[20..24].copy_from_slice(&0x9000_0000u32.to_le_bytes());
    assert!(audit_quest_item_text_units(&outside, 0x8010_0000, 2, &code_table).is_err());

    let mut unknown_glyph = raw.clone();
    unknown_glyph[0x30..0x32].copy_from_slice(&[0x12, 0x34]);
    assert!(audit_quest_item_text_units(&unknown_glyph, 0x8010_0000, 2, &code_table).is_err());

    let mut no_terminal = raw;
    no_terminal[0x48..0x4C].fill(0);
    assert!(audit_quest_item_text_units(&no_terminal, 0x8010_0000, 2, &code_table).is_err());
}

#[test]
fn source_report는_중복_table과_control위치를_원문옆에_보존한다() {
    let (raw, code_table) = synthetic();
    let analysis = audit_quest_item_text_units(&raw, 0x8010_0000, 2, &code_table).unwrap();
    let report = render_quest_item_text_units_tsv(&analysis);
    let rows = report.lines().collect::<Vec<_>>();

    assert_eq!(rows.len(), 3);
    assert!(rows[0].contains("protected_source\tannotated_source\tcontrol_signature"));
    assert!(rows[1].contains("\tname\t0,1\t"));
    assert!(rows[1].contains("\tカギ\tカギ{NUL}\t\t"));
    assert!(rows[2].contains("\tdescription\t0,1\t"));
    assert!(
        rows[2].contains("\tあい\t{FF0B:0BFF0000}あい{FF1A:1AFF00000000}\t"),
        "{}",
        rows[2]
    );
    assert!(rows[2].contains("FF0B:0BFF0000|FF1A:1AFF00000000"));
}
