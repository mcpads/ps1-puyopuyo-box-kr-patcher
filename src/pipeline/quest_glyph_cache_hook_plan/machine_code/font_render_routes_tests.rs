use super::*;
use std::collections::BTreeMap;

// delay slot과 load delay를 적용해 caller/stream 선택 결과를 검사한다.
fn selected_route(caller: u32, cursor: Option<u32>) -> usize {
    let mut kinds = Vec::new();
    let (selected, other_stream) = append_encounter_name_filter(&mut kinds).unwrap();
    let end = kinds.len();
    patch_branch(&mut kinds, selected, end + 1).unwrap();
    patch_branch(&mut kinds, other_stream, end + 2).unwrap();
    let mut regs = [0u32; 32];
    regs[Reg::RA.0 as usize] = caller;
    regs[Reg::S4.0 as usize] = 0x1000;
    // 이전 T2가 우연히 예상 cursor여도 다른 stream을 허용하면 안 된다.
    regs[Reg::T2.0 as usize] = crate::analysis::QUEST_ENCOUNTER_NAME_STREAM_CURSOR_RUNTIME;
    let mut pc = 0usize;
    let mut jump = None;
    let mut load = None;
    while pc < end {
        let next = jump.take();
        let pending = load.take();
        let r = |reg: Reg| regs[reg.0 as usize];
        let write = match kinds[pc] {
            K::Lui { rt, imm } => Some((rt, u32::from(imm) << 16)),
            K::Addiu { rt, rs, imm } => Some((rt, r(rs).wrapping_add_signed(i32::from(imm)))),
            K::Sll { rd, rt, sa } => Some((rd, r(rt) << sa)),
            K::Lw { rt, base, offset } => {
                assert_eq!(r(base).wrapping_add_signed(i32::from(offset)), 0x101C);
                load = Some((rt, cursor.expect("다른 caller의 S4는 읽으면 안 된다")));
                None
            }
            K::Beq { rs, rt, offset } | K::Bne { rs, rt, offset } => {
                let equal = r(rs) == r(rt);
                if equal == matches!(kinds[pc], K::Beq { .. }) {
                    jump = Some((pc as isize + 1 + isize::from(offset)) as usize);
                }
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
    pc - end
}

#[test]
fn encounter_name_requires_both_the_fixed_name_caller_and_its_stream() {
    let caller = crate::analysis::QUEST_ENCOUNTER_NAME_GLYPH_RETURN_RUNTIME;
    let cursor = crate::analysis::QUEST_ENCOUNTER_NAME_STREAM_CURSOR_RUNTIME;
    assert_eq!(selected_route(caller, Some(cursor)), 1);
    for other_cursor in [0, cursor - 2, cursor + 2] {
        assert_eq!(selected_route(caller, Some(other_cursor)), 2);
    }
    for other_caller in [
        0,
        caller - 4,
        caller + 4,
        QUEST_SEQ_TEXT_GLYPH_RENDERER_RETURN_RUNTIME,
    ] {
        assert_eq!(selected_route(other_caller, None), 0);
    }
}

// 실제 object marker 경로를 branch/load delay와 함께 실행한다.
// 확대·축소는 원본 object에서 이어 받고, 일치하는 공백만 상대 반폭이어야 한다.
fn marked_scale(marker: u8, slot: u16, initial: u32, spaces: &[Option<u16>]) -> u32 {
    let mut kinds = Vec::new();
    let entries =
        append_font_render_routes(&mut kinds, 0x8010_0000, 0x3e, 0x3f, false, spaces, &[]).unwrap();
    let mut regs = [0u32; 32];
    regs[Reg::A2.0 as usize] = 0x10000;
    regs[Reg::V1.0 as usize] = 0x20000;
    regs[Reg::S0.0 as usize] = 7;
    let mut memory = BTreeMap::from([
        (QUEST_GLYPH_CACHE_RENDERER_PAGE_RUNTIME, u32::from(marker)),
        (0x10058, initial),
        (0x1005c, 0x2345),
        (0x10066, u32::from(slot)),
        (0x2000e, 3),
    ]);
    let mut pc = entries.object_marker;
    let mut delayed = None;
    let mut load = None;
    let mut finished = false;
    for _ in 0..kinds.len() * 2 {
        let target = delayed.take();
        let pending_load = load.take();
        let address =
            |base: Reg, offset: i16| regs[base.0 as usize].wrapping_add_signed(i32::from(offset));
        match kinds[pc] {
            K::Lui { rt, imm } => regs[rt.0 as usize] = u32::from(imm) << 16,
            K::Ori { rt, rs, imm } => regs[rt.0 as usize] = regs[rs.0 as usize] | u32::from(imm),
            K::Andi { rt, rs, imm } => regs[rt.0 as usize] = regs[rs.0 as usize] & u32::from(imm),
            K::Addiu { rt, rs, imm } => {
                regs[rt.0 as usize] = regs[rs.0 as usize].wrapping_add_signed(i32::from(imm))
            }
            K::Sltiu { rt, rs, imm } => {
                regs[rt.0 as usize] = u32::from(regs[rs.0 as usize] < imm as i32 as u32)
            }
            K::Lbu { rt, base, offset }
            | K::Lhu { rt, base, offset }
            | K::Lw { rt, base, offset } => {
                load = Some((rt, memory[&address(base, offset)]));
            }
            K::Sb { rt, base, offset } | K::Sw { rt, base, offset } => {
                memory.insert(address(base, offset), regs[rt.0 as usize]);
            }
            K::Sll { rd, rt, sa } => regs[rd.0 as usize] = regs[rt.0 as usize] << sa,
            K::Sra { rd, rt, sa } => {
                regs[rd.0 as usize] = ((regs[rt.0 as usize] as i32) >> sa) as u32
            }
            K::Beq { rs, rt, offset } | K::Bne { rs, rt, offset } => {
                let equal = regs[rs.0 as usize] == regs[rt.0 as usize];
                if equal == matches!(kinds[pc], K::Beq { .. }) {
                    delayed = Some((pc as isize + 1 + offset as isize) as usize);
                }
            }
            K::J { target } => {
                assert_eq!(target, QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME + 8);
                delayed = Some(usize::MAX);
            }
            ref kind => panic!("object marker에서 지원하지 않는 명령: {kind:?}"),
        }
        if let Some((r, value)) = pending_load {
            regs[r.0 as usize] = value;
        }
        regs[0] = 0;
        if target == Some(usize::MAX) {
            finished = true;
            break;
        }
        pc = target.unwrap_or(pc + 1);
    }
    assert!(finished);
    assert_eq!(
        memory[&(0x10000 + QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET as u32)],
        u32::from(marker)
    );
    assert_eq!(
        memory[&(0x10000 + QUEST_FONT_OBJECT_STATE_INDEX_OFFSET as u32)],
        7
    );
    assert_eq!(memory[&0x1005c], 0x2345, "세로 효과를 변경하면 안 된다");
    memory[&0x10058]
}

#[test]
fn 한글_object의_확대와_축소를_보존하고_공백만_상대_반폭으로_그린다() {
    for initial in [
        0x400,
        0x800,
        0x1000,
        0x1800,
        0x2000,
        0x3000,
        (-0x1000i32) as u32,
    ] {
        for marker in [0, 1, 2, 0x81, 0x82] {
            for slot in [59, 292] {
                assert_eq!(marked_scale(marker, slot, initial, &[]), initial);
                let expected = if marker & 0x7f == 1 && slot == 292 {
                    ((initial as i32) >> 1) as u32
                } else {
                    initial
                };
                assert_eq!(
                    marked_scale(marker, slot, initial, &[Some(292), None]),
                    expected,
                    "marker={marker} slot={slot} initial={initial:#x}"
                );
            }
        }
    }
}
