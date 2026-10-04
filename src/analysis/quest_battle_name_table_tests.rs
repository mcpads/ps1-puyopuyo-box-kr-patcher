use super::*;
use crate::coords::LogicalFile;
use crate::font::QUEST_CODE_TABLE_COUNT;

const LOAD: u32 = 0x8001_0000;
const LEN: usize = 0x200;

fn map() -> ExeMap {
    ExeMap {
        name: "QUEST.EXE",
        load_addr: LOAD,
        text_len: LEN as u32,
        entry_pc: LOAD,
        file: LogicalFile::new(0, (LEN + 0x800) as u32),
    }
}

fn code_table() -> QuestCodeTable {
    let mut codes = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| u16::try_from(index).expect("fixture code").to_le_bytes())
        .collect::<Vec<_>>();
    codes[0] = [0x83, 0x68]; // ド
    codes[1] = [0x83, 0x89]; // ラ
    codes[2] = [0x83, 0x52]; // コ
    QuestCodeTable::parse(&codes.into_iter().flatten().collect::<Vec<_>>())
        .expect("fixture code table")
}

fn runtime(offset: usize) -> u32 {
    LOAD + offset as u32
}

fn assignment(index: u8, value: u32) -> [u8; 8] {
    let mut raw = [0x11, 0x40, index, 0, 0, 0, 0, 0];
    raw[4..8].copy_from_slice(&value.to_le_bytes());
    raw
}

fn fixture() -> (Vec<u8>, u32) {
    let selector = 0x20usize;
    let first_stream = 0x2Cusize;
    let second_stream = 0x60usize;
    let mut text = vec![0; LEN];
    text[selector..selector + 4].copy_from_slice(&[0x09, 0x34, 0, 0]);
    text[selector + 4..selector + 8].copy_from_slice(&runtime(first_stream).to_le_bytes());
    text[selector + 8..selector + 12].copy_from_slice(&runtime(second_stream).to_le_bytes());

    text[first_stream..first_stream + 8]
        .copy_from_slice(&assignment(QUEST_BATTLE_NAME_STATE_INDEX, runtime(0x40)));
    text[first_stream + 8..first_stream + 16].copy_from_slice(&assignment(
        QUEST_BATTLE_NAME_SLOT_END_STATE_INDEX,
        runtime(0x48),
    ));
    text[0x40..0x48].copy_from_slice(&[0x83, 0x68, 0x83, 0x89, 0x83, 0x52, 0, 0]);

    text[second_stream..second_stream + 8]
        .copy_from_slice(&assignment(QUEST_BATTLE_NAME_STATE_INDEX, runtime(0x80)));
    text[second_stream + 8..second_stream + 16].copy_from_slice(&assignment(
        QUEST_BATTLE_NAME_SLOT_END_STATE_INDEX,
        runtime(0x88),
    ));
    text[0x80..0x88].copy_from_slice(&[0x83, 0x68, 0x83, 0x89, 0x83, 0x52, 0, 0]);
    (text, runtime(selector))
}

#[test]
fn op09가_소유한_모든_branch에서_이름과_slot을_찾는다() {
    let (text, selector) = fixture();

    let analysis = audit_quest_battle_name_table(&map(), &text, &code_table(), selector).unwrap();

    assert_eq!(analysis.pointer_table_runtime, selector + 4);
    assert_eq!(analysis.pointer_table_end_runtime, runtime(0x2C));
    assert_eq!(analysis.fixed_text_state_runtime, 0x800D_B7A0);
    assert_eq!(analysis.entries.len(), 2);
    assert_eq!(analysis.unique_sources, 1);
    for (entry, stream) in analysis.entries.iter().zip([0x2C, 0x60]) {
        assert_eq!(entry.stream_runtime, runtime(stream));
        assert_eq!(entry.source, "ドラコ");
        assert_eq!(entry.source_raw, [0x83, 0x68, 0x83, 0x89, 0x83, 0x52]);
        assert_eq!(entry.protected_slot.len(), 8);
        assert_eq!(entry.slot_capacity_glyphs, 3);
    }
}

#[test]
fn slot_밖까지_이름으로_늘리거나_nul_뒤_자료를_padding으로_오인하지_않는다() {
    let (mut text, selector) = fixture();
    text[0x2C + 8..0x2C + 16].copy_from_slice(&assignment(
        QUEST_BATTLE_NAME_SLOT_END_STATE_INDEX,
        runtime(0x4C),
    ));
    text[0x48] = 1;

    let failure =
        audit_quest_battle_name_table(&map(), &text, &code_table(), selector).unwrap_err();

    assert!(failure.to_string().contains("NUL 뒤 slot padding"));
}

#[test]
fn branch_target에_이름_pointer_대입이_없으면_부분모집단을_반환하지_않는다() {
    let (mut text, selector) = fixture();
    text[0x2C + 2] = 0x37;

    let failure =
        audit_quest_battle_name_table(&map(), &text, &code_table(), selector).unwrap_err();

    assert!(failure.to_string().contains("index 0x38 대입이 없음"));
}

#[test]
fn 이름slot의_추가_직접pointer참조를_조우전용으로_오인하지_않는다() {
    let (mut text, selector) = fixture();
    text[0xB0..0xB4].copy_from_slice(&runtime(0x40).to_le_bytes());

    let failure =
        audit_quest_battle_name_table(&map(), &text, &code_table(), selector).unwrap_err();

    assert!(failure.to_string().contains("직접 pointer 참조 분모 drift"));
}

#[test]
fn 원문_순서와_물리slot_근거를_tsv로_보존한다() {
    let (text, selector) = fixture();
    let analysis = audit_quest_battle_name_table(&map(), &text, &code_table(), selector).unwrap();

    let report = render_quest_battle_name_table_tsv(&analysis);
    let rows = report.lines().collect::<Vec<_>>();

    assert_eq!(rows.len(), 3);
    assert!(rows[0].starts_with("unit_id\tindex\tsource\tsource_raw_hex\t"));
    assert!(rows[1].starts_with("QUEST-EXE-BATTLE-NAME:00:0x80010040\t0\tドラコ\t836883898352\t"));
    assert!(
        rows[1].contains("\t0x8001002C\t0x80010040\t0x80010034\t0x80010048\t8\t3\t3\t0x800DB7A0\t")
    );
    assert!(rows[2].starts_with("QUEST-EXE-BATTLE-NAME:01:0x80010080\t1\tドラコ\t836883898352\t"));
    assert!(rows[1].ends_with("source-consumer-bound;runtime-population-not-exhaustive"));
}
