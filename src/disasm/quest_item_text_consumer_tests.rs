use super::*;

#[test]
fn description_route는_active_word와_stream을_같은_새_object에_쓴다() {
    let stream_load = K::Lw {
        rt: Reg::A1,
        base: Reg::S5,
        offset: QUEST_ITEM_DESCRIPTION_POINTER_FIELD_OFFSET,
    };
    let active_value = K::Addiu {
        rt: Reg::V1,
        rs: Reg::ZERO,
        imm: 1,
    };
    let active_store = K::Sw {
        rt: Reg::V1,
        base: Reg::V0,
        offset: 0,
    };
    let stream_store = K::Sw {
        rt: Reg::A1,
        base: Reg::V0,
        offset: QUEST_SEQ_STREAM_FIELD_OFFSET,
    };

    assert!(is_direct_text_install(
        &stream_load,
        &active_value,
        &active_store,
        &stream_store
    ));
}

#[test]
fn 다른_object나_field에는_direct_text_install을_인정하지_않는다() {
    let stream_load = K::Lw {
        rt: Reg::A1,
        base: Reg::S5,
        offset: QUEST_ITEM_DESCRIPTION_POINTER_FIELD_OFFSET,
    };
    let active_value = K::Addiu {
        rt: Reg::V1,
        rs: Reg::ZERO,
        imm: 1,
    };
    let active_store = K::Sw {
        rt: Reg::V1,
        base: Reg::V0,
        offset: 0,
    };
    let wrong_object = K::Sw {
        rt: Reg::A1,
        base: Reg::V1,
        offset: QUEST_SEQ_STREAM_FIELD_OFFSET,
    };
    let wrong_field = K::Sw {
        rt: Reg::A1,
        base: Reg::V0,
        offset: 0x20,
    };

    assert!(!is_direct_text_install(
        &stream_load,
        &active_value,
        &active_store,
        &wrong_object
    ));
    assert!(!is_direct_text_install(
        &stream_load,
        &active_value,
        &active_store,
        &wrong_field
    ));
}
