use crate::disasm::{
    QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES, QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME,
};

use super::*;

#[test]
fn 공용_선택지의_여덟_call을_모두_direct_entry로_보낸다() {
    let sites = assemble_quest_ui_choice_direct_glyph_sites(0x8001_3000).expect("choice sites");
    assert_eq!(sites.len(), 8);
    assert!(sites
        .iter()
        .map(VerifiedMachineCode::base_addr)
        .eq(QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES));
    assert!(sites.iter().all(|site| site.bytes().len() == 8));
}

#[test]
fn world_map_지명_draw를_whole_string_entry로_보낸다() {
    let site = assemble_world_map_label_whole_string_site(0x8001_3400).expect("world map site");
    assert_eq!(
        site.base_addr(),
        QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME
    );
    assert_eq!(site.bytes().len(), 8);
}

#[test]
fn 메뉴_call은_종류별_entry와_원본_delay를_보존한다() {
    let direct = 0x8011_0000;
    let whole = 0x8011_0100;
    let sites = assemble_menu_literal_sites(direct, whole).unwrap();
    assert_eq!(sites.len(), crate::disasm::QUEST_MENU_GLYPH_CALLS.len());
    for (site, call) in sites.iter().zip(crate::disasm::QUEST_MENU_GLYPH_CALLS) {
        assert_eq!(site.base_addr(), call.runtime);
        let decoded = crate::isa::decode_slice(site.base_addr(), site.bytes()).unwrap();
        assert_eq!(
            decoded[0].kind,
            K::Jal {
                target: if call.whole_string { whole } else { direct }
            }
        );
        assert_eq!(decoded[1].kind, call.delay);
    }
}
