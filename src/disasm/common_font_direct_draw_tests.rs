use super::*;

#[test]
fn 지원하지_않는_exe는_quest_direct_draw로_승격하지_않는다() {
    let map = Box::leak(Box::new(ExeMap {
        name: "PUYO2.EXE",
        file: crate::coords::LogicalFile::new(0, 4),
        entry_pc: 0x8001_0000,
        load_addr: 0x8001_0000,
        text_len: 4,
    }));
    let error = verify_quest_common_font_direct_draw(map, &[0; 4]).unwrap_err();
    assert!(error.to_string().contains("QUEST.EXE가 아님"));
}

#[test]
fn 지원_quest_text길이_drift는_source검증전에_거부한다() {
    let map = Box::leak(Box::new(ExeMap {
        name: "QUEST.EXE",
        file: crate::coords::LogicalFile::new(0, 8),
        entry_pc: 0x8001_0000,
        load_addr: 0x8001_0000,
        text_len: 8,
    }));
    let error = verify_quest_common_font_direct_draw(map, &[0; 4]).unwrap_err();
    assert!(error.to_string().contains("text 길이 불일치"));
}
