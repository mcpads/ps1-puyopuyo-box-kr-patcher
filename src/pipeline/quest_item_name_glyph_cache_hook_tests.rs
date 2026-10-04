use super::*;
use std::collections::BTreeMap;

fn requested_page(mode: i16, caller: u32, active: u32) -> u32 {
    requested_page_with_glyph(mode, caller, active, 0, 0)
}

fn requested_page_with_glyph(mode: i16, caller: u32, active: u32, slots: u16, glyph: u32) -> u32 {
    let mut code = Vec::new();
    append_item_name_entry_for_mode(&mut code, 10, 10, 12, mode, slots);
    let mut regs = [0u32; 32];
    regs[Reg::SP.0 as usize] = 0x2000;
    regs[Reg::RA.0 as usize] = caller;
    regs[Reg::A1.0 as usize] = glyph;
    let mut mem = BTreeMap::from([(
        super::super::quest_glyph_cache_hook_plan::QUEST_GLYPH_CACHE_ROUTE_PAGE_RUNTIME,
        active,
    )]);
    let first = crate::font::QUEST_CODE_TABLE_RUNTIME
        + (super::super::quest_glyph_cache_plan::QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX * 2)
            as u32;
    for index in 0..u32::from(slots) {
        mem.insert(first + index * 2, 0x8240 + index);
    }
    let mut pc = 0usize;
    let mut jump = None;
    let mut load = None;
    let mut steps = 0;
    while pc < code.len() {
        steps += 1;
        assert!(steps < 2000, "공통 prefix 탐색이 끝나지 않음");
        let next = jump.take();
        let pending = load.take();
        let r = |reg: Reg| regs[reg.0 as usize];
        let write = match code[pc] {
            K::Addiu { rt, rs, imm } => Some((rt, r(rs).wrapping_add_signed(i32::from(imm)))),
            K::Lui { rt, imm } => Some((rt, u32::from(imm) << 16)),
            K::Andi { rt, rs, imm } => Some((rt, r(rs) & u32::from(imm))),
            K::Sltiu { rt, rs, imm } => Some((rt, u32::from(r(rs) < imm as u32))),
            K::Xor { rd, rs, rt } => Some((rd, r(rs) ^ r(rt))),
            K::And { rd, rs, rt } => Some((rd, r(rs) & r(rt))),
            K::Subu { rd, rs, rt } => Some((rd, r(rs).wrapping_sub(r(rt)))),
            K::Addu { rd, rs, rt } => Some((rd, r(rs).wrapping_add(r(rt)))),
            K::Sll { rd, rt, sa } => Some((rd, r(rt) << sa)),
            K::Sw { rt, base, offset } => {
                mem.insert(r(base).wrapping_add_signed(i32::from(offset)), r(rt));
                None
            }
            K::Lw { rt, base, offset }
            | K::Lbu { rt, base, offset }
            | K::Lhu { rt, base, offset } => {
                load = Some((rt, mem[&r(base).wrapping_add_signed(i32::from(offset))]));
                None
            }
            K::Beq { rs, rt, offset } => {
                if r(rs) == r(rt) {
                    jump = Some((pc as isize + 1 + offset as isize) as usize);
                }
                None
            }
            K::Bne { rs, rt, offset } => {
                if r(rs) != r(rt) {
                    jump = Some((pc as isize + 1 + offset as isize) as usize);
                }
                None
            }
            K::J { .. } => {
                jump = Some(code.len());
                None
            }
            other => panic!("unexpected {other:?}"),
        };
        if let Some((reg, value)) = pending {
            regs[reg.0 as usize] = value;
        }
        if let Some((reg, value)) = write {
            regs[reg.0 as usize] = value;
        }
        regs[0] = 0;
        pc = next.unwrap_or(pc + 1);
    }
    regs[Reg::S1.0 as usize]
}

#[test]
fn 메뉴의_공통_cell만_설명_page를_재사용하고_전용_cell은_전환한다() {
    let mode = QUEST_ITEM_NAME_MODE_DIRECT_GLYPH;
    for call in crate::disasm::QUEST_MENU_GLYPH_CALLS
        .iter()
        .filter(|c| !c.whole_string)
    {
        for glyph in [0x8240, 0x8242] {
            assert_eq!(
                requested_page_with_glyph(mode, call.runtime + 8, 11, 3, glyph),
                11
            );
            assert_eq!(
                requested_page_with_glyph(mode, call.runtime + 8, 9, 3, glyph),
                10
            );
        }
        assert_eq!(
            requested_page_with_glyph(mode, call.runtime + 8, 11, 3, 0x8243),
            10
        );
    }
}

#[test]
fn 이름은_현재설명page를_재사용하고_ui는_첫page를_요청한다() {
    for (mode, callers) in [
        (
            QUEST_ITEM_NAME_MODE_DIRECT_GLYPH,
            QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES.as_slice(),
        ),
        (
            QUEST_ITEM_NAME_MODE_WHOLE_STRING,
            QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES.as_slice(),
        ),
    ] {
        for caller in callers {
            assert_eq!(requested_page(mode, caller + 8, 11), 11);
            assert_eq!(requested_page(mode, caller + 8, 9), 10);
        }
        for call in crate::disasm::QUEST_MENU_GLYPH_CALLS
            .iter()
            .filter(|c| c.whole_string == (mode == QUEST_ITEM_NAME_MODE_WHOLE_STRING))
        {
            assert_eq!(requested_page(mode, call.runtime + 8, 11), 10);
        }
    }
    assert_eq!(requested_page(QUEST_ITEM_NAME_MODE_CALLBACK, 0, 12), 12);
}
