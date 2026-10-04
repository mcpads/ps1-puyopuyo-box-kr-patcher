use crate::analysis::QuestBattleNameEntry;
use crate::coords::LogicalFile;

use super::*;

fn code_table() -> QuestCodeTable {
    let mut codes = vec![[0x82, 0xA0]];
    for index in 0..344usize {
        codes.push([
            0x90 + u8::try_from(index / 200).unwrap(),
            u8::try_from(index % 200).unwrap(),
        ]);
    }
    let raw = codes.into_iter().flatten().collect::<Vec<_>>();
    QuestCodeTable::parse(&raw).unwrap()
}

struct BattleNameWriterFixture {
    map: ExeMap,
    text: Vec<u8>,
    table: QuestBattleNameTableAnalysis,
    compositions: QuestBattleNameCompositionAnalysis,
    page_codes: BTreeMap<u8, BTreeMap<char, [u8; 2]>>,
}

fn fixture() -> BattleNameWriterFixture {
    let map = ExeMap {
        name: "QUEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x100,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(10, 0x900),
    };
    let mut text = vec![0; map.text_len as usize];
    let protected_slot = vec![0x82, 0xA0, 0, 0, 0, 0, 0, 0];
    text[0x20..0x28].copy_from_slice(&protected_slot);
    let digest: [u8; 32] = Sha256::digest(&protected_slot).into();
    let entry = QuestBattleNameEntry {
        id: "QUEST-EXE-BATTLE-NAME:00:0x80010020".to_owned(),
        index: 0,
        branch_pointer_runtime: 0x8001_0010,
        stream_runtime: 0x8001_0014,
        name_assignment_runtime: 0x8001_0018,
        name_runtime: 0x8001_0020,
        slot_end_assignment_runtime: 0x8001_001C,
        slot_end_runtime: 0x8001_0028,
        name_text_offset: 0x20,
        slot_end_text_offset: 0x28,
        source: "あ".to_owned(),
        source_raw: vec![0x82, 0xA0],
        protected_slot: protected_slot.clone(),
        protected_slot_sha256: digest,
        glyph_occurrences: 1,
        slot_capacity_glyphs: 3,
    };
    let table = QuestBattleNameTableAnalysis {
        selector_runtime: 0x8001_0000,
        selector_field_offset: 0x34,
        pointer_table_runtime: 0x8001_0004,
        pointer_table_end_runtime: 0x8001_0008,
        fixed_text_state_index: 0x38,
        fixed_text_state_runtime: 0x800D_B7A0,
        entries: vec![entry],
        unique_sources: 1,
        glyph_occurrences: 1,
        slot_bytes: protected_slot.len(),
    };
    let composition = super::super::quest_battle_name_compositions::QuestBattleNameComposition {
        unit_id: table.entries[0].id.clone(),
        index: 0,
        protected_source: "あ".to_owned(),
        protected_slot_sha256: digest,
        slot_capacity_glyphs: 3,
        development_text: "한글".to_owned(),
        pronunciation_tail: '글',
        subject_particle: '이',
        candidates: vec!["한글".to_owned()],
        decision_status: QuestBattleNameDecisionStatus::DraftNeedsIndependentReview,
    };
    let compositions = QuestBattleNameCompositionAnalysis {
        compositions: vec![composition],
        used_glyphs: "글이한".to_owned(),
        selected_drafts: 1,
        needs_human_choice: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    };
    let mut dynamic = BTreeMap::new();
    dynamic.insert('글', [0x90, 0x12]);
    dynamic.insert('한', [0x90, 0x13]);
    dynamic.insert('이', [0x90, 0x14]);
    let page_codes = BTreeMap::from([(7, dynamic)]);
    BattleNameWriterFixture {
        map,
        text,
        table,
        compositions,
        page_codes,
    }
}

#[test]
fn 수동이름을_자기slot의_u16_nul과_zero_padding으로_쓴다() {
    let fixture = fixture();

    let writer = write_battle_name_slots(
        &fixture.map,
        &fixture.text,
        &fixture.table,
        &fixture.compositions,
        7,
        &fixture.page_codes,
        &code_table(),
        "0".repeat(64),
    )
    .unwrap();

    assert_eq!(writer.physical_names, 1);
    assert_eq!(writer.glyph_cells, 3);
    assert_eq!(writer.writes[0].development_text, "한글이");
    assert_eq!(
        writer.writes[0].output,
        [0x90, 0x13, 0x90, 0x12, 0x90, 0x14, 0, 0]
    );
    assert!(writer.source_exact);
    assert!(writer.nul_terminal_exact);
    assert!(!writer.translation_approved);
    assert!(!writer.layout_approved);
    assert!(!writer.runtime_verified);
    assert!(!writer.product_eligible);
}

#[test]
fn 원본byte_slot용량과_page공급_drift를_거부한다() {
    let fixture = fixture();
    let mut stale_text = fixture.text.clone();
    stale_text[0x20] ^= 1;
    assert!(write_battle_name_slots(
        &fixture.map,
        &stale_text,
        &fixture.table,
        &fixture.compositions,
        7,
        &fixture.page_codes,
        &code_table(),
        "0".repeat(64),
    )
    .is_err());

    let mut overflow = fixture.compositions.clone();
    overflow.compositions[0].development_text = "한글한글".to_owned();
    overflow.used_glyphs = "글한".to_owned();
    assert!(write_battle_name_slots(
        &fixture.map,
        &fixture.text,
        &fixture.table,
        &overflow,
        7,
        &fixture.page_codes,
        &code_table(),
        "0".repeat(64),
    )
    .is_err());

    let missing_page_codes = BTreeMap::from([(7, BTreeMap::from([('한', [0x90, 0x13])]))]);
    assert!(write_battle_name_slots(
        &fixture.map,
        &fixture.text,
        &fixture.table,
        &fixture.compositions,
        7,
        &missing_page_codes,
        &code_table(),
        "0".repeat(64),
    )
    .is_err());
}

#[test]
fn 수동발음끝음절과_주격조사의_결속_drift를_거부한다() {
    let fixture = fixture();
    let mut drift = fixture.compositions.clone();
    drift.compositions[0].subject_particle = '가';

    assert!(write_battle_name_slots(
        &fixture.map,
        &fixture.text,
        &fixture.table,
        &drift,
        7,
        &fixture.page_codes,
        &code_table(),
        "0".repeat(64),
    )
    .is_err());
}

#[test]
fn writer_provenance는_output변경을_거부한다() {
    let fixture = fixture();
    let mut writer = write_battle_name_slots(
        &fixture.map,
        &fixture.text,
        &fixture.table,
        &fixture.compositions,
        7,
        &fixture.page_codes,
        &code_table(),
        "0".repeat(64),
    )
    .unwrap();
    writer.writes[0].output[0] ^= 1;

    assert!(verify_quest_battle_name_writer_provenance(&writer).is_err());
}
