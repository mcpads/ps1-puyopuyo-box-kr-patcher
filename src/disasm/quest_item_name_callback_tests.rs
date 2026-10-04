use super::*;

fn retry_boundary(state_offset: i16) -> bool {
    let state_load = K::Lhu {
        rt: Reg::S0,
        base: Reg::S5,
        offset: state_offset,
    };
    let state_value = K::Addiu {
        rt: Reg::S4,
        rs: Reg::ZERO,
        imm: QUEST_ITEM_NAME_DRAW_STATE,
    };
    let state_branch = K::Beq {
        rs: Reg::S0,
        rt: Reg::S4,
        offset: 123,
    };
    let site_first = K::Addiu {
        rt: Reg::A0,
        rs: Reg::ZERO,
        imm: 1,
    };
    let site_second = K::Addu {
        rd: Reg::S2,
        rs: Reg::ZERO,
        rt: Reg::ZERO,
    };
    let advance_load = K::Lhu {
        rt: Reg::V0,
        base: Reg::S5,
        offset: QUEST_ITEM_NAME_INNER_STATE_OFFSET,
    };
    let advance_value = K::Addiu {
        rt: Reg::V0,
        rs: Reg::V0,
        imm: 1,
    };
    let advance_store = K::Sh {
        rt: Reg::V0,
        base: Reg::S5,
        offset: QUEST_ITEM_NAME_INNER_STATE_OFFSET,
    };

    is_retry_boundary(
        [&state_load, &state_value, &state_branch],
        [&site_first, &site_second],
        [&advance_load, &advance_value, &advance_store],
    )
}

#[test]
fn 이름page획득은_draw_state를_진행시키기전에_재시도할수있다() {
    assert!(retry_boundary(QUEST_ITEM_NAME_INNER_STATE_OFFSET));
}

#[test]
fn 다른state_field는_이름draw재시도경계가_아니다() {
    assert!(!retry_boundary(QUEST_ITEM_NAME_INNER_STATE_OFFSET + 2));
}
