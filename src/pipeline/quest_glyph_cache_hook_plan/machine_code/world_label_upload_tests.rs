use super::*;
use crate::isa::{verify_typed_source, IsaProfile};
use std::collections::BTreeMap;

// 실제 조립·재해석 명령에 load/branch delay를 적용한다. 공급하지 않은 메모리
// 읽기와 공용 frame의 부분 전송 flag 밖 쓰기는 즉시 실패시킨다.
fn run(caller: u32, pointer: u32, words: &[u16], code: u16) -> (bool, u32) {
    const SP: u32 = 0x1000;
    let mut kinds = Vec::new();
    append_selection(&mut kinds).unwrap();
    let skip = append_filter(&mut kinds).unwrap();
    kinds.push(K::Addiu {
        rt: Reg::V0,
        rs: Reg::ZERO,
        imm: 1,
    });
    let finish = kinds.len();
    patch_branch(&mut kinds, skip, finish).unwrap();
    append_partial_commit(&mut kinds);
    let verified = verify_typed_source(
        IsaProfile::DECLARED,
        0x8010_0000,
        "test::world_label_upload",
        &kinds,
    )
    .unwrap();
    let mut memory = BTreeMap::<u32, u8>::new();
    let mut put = |address: u32, bytes: &[u8]| {
        for (n, b) in bytes.iter().enumerate() {
            memory.insert(address + n as u32, *b);
        }
    };
    put(SP + 60, &caller.to_le_bytes());
    put(SP + 20, &pointer.to_le_bytes());
    put(QUEST_CODE_TABLE_RUNTIME + 60, &code.to_le_bytes());
    for (n, w) in words.iter().enumerate() {
        put(pointer + n as u32 * 2, &w.to_le_bytes());
    }
    let mut regs = [0u32; 32];
    regs[Reg::SP.0 as usize] = SP;
    regs[Reg::S1.0 as usize] = 58;
    let mut pc = 0usize;
    let mut branch = None;
    let mut delayed_load = None;
    let mut steps = 0;
    let instructions = verified.instructions();
    while pc < instructions.len() {
        steps += 1;
        assert!(steps < 400);
        let next = branch.take();
        let load = delayed_load.take();
        let get = |r: Reg| regs[r.0 as usize];
        let mut write = None;
        match instructions[pc].kind {
            K::Lui { rt, imm } => write = Some((rt, u32::from(imm) << 16)),
            K::Addiu { rt, rs, imm } => {
                write = Some((rt, get(rs).wrapping_add_signed(i32::from(imm))))
            }
            K::Addu { rd, rs, rt } => write = Some((rd, get(rs).wrapping_add(get(rt)))),
            K::Subu { rd, rs, rt } => write = Some((rd, get(rs).wrapping_sub(get(rt)))),
            K::Andi { rt, rs, imm } => write = Some((rt, get(rs) & u32::from(imm))),
            K::Ori { rt, rs, imm } => write = Some((rt, get(rs) | u32::from(imm))),
            K::Sltiu { rt, rs, imm } => {
                write = Some((rt, u32::from(get(rs) < i32::from(imm) as u32)))
            }
            K::Sll { rd, rt, sa } => write = Some((rd, get(rt) << sa)),
            K::Lw { rt, base, offset } | K::Lhu { rt, base, offset } => {
                let address = get(base).wrapping_add_signed(i32::from(offset));
                let size = if matches!(instructions[pc].kind, K::Lw { .. }) {
                    4
                } else {
                    2
                };
                assert_eq!(address % size, 0);
                let value = (0..size).fold(0, |v, n| {
                    v | (u32::from(*memory.get(&(address + n)).expect("읽기 범위")) << (n * 8))
                });
                delayed_load = Some((rt, value));
            }
            K::Sw { rt, base, offset } => {
                let address = get(base).wrapping_add_signed(i32::from(offset));
                assert_eq!(address, SP + PARTIAL as u32);
                for (n, b) in get(rt).to_le_bytes().iter().enumerate() {
                    memory.insert(address + n as u32, *b);
                }
            }
            K::Beq { rs, rt, offset } => {
                if get(rs) == get(rt) {
                    branch = Some((pc as isize + 1 + offset as isize) as usize);
                }
            }
            K::Bne { rs, rt, offset } => {
                if get(rs) != get(rt) {
                    branch = Some((pc as isize + 1 + offset as isize) as usize);
                }
            }
            other => panic!("미지원 명령 {other:?}"),
        }
        if let Some((r, v)) = load {
            regs[r.0 as usize] = v;
        }
        if let Some((r, v)) = write {
            regs[r.0 as usize] = v;
        }
        regs[0] = 0;
        pc = next.unwrap_or(pc + 1);
    }
    (regs[Reg::V0.0 as usize] != 0, regs[Reg::S1.0 as usize])
}

#[test]
fn 지명에_쓴_cell만_올리고_부분_page는_resident로_남기지_않는다() {
    let caller = QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME + 8;
    for pointer in [QUEST_WORLD_MAP_LABEL_TABLE_RUNTIME, 0x800B_E23C] {
        assert_eq!(
            run(caller, pointer, &[0x1234, 0x5678, 0], 0x5678),
            (true, 0xBA)
        );
        assert_eq!(
            run(caller, pointer, &[0x1234, 0x5678, 0], 0x9999),
            (false, 0xBA)
        );
        assert_eq!(run(caller, pointer, &[0], 0), (false, 0xBA));
    }
}

#[test]
fn 다른_caller와_미등록_pointer는_전체_page_공급을_유지한다() {
    let caller = QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME + 8;
    for (ra, ptr) in [
        (caller + 4, QUEST_WORLD_MAP_LABEL_TABLE_RUNTIME),
        (caller, 0x8000_0000),
        (caller, QUEST_WORLD_MAP_LABEL_TABLE_RUNTIME + 1),
        (caller, QUEST_WORLD_MAP_LABEL_TABLE_END_RUNTIME),
    ] {
        assert_eq!(run(ra, ptr, &[], 0x1234), (true, 58));
    }
}

#[test]
fn 손상된_nul도_유한_범위를_넘어_읽지_않는다() {
    assert_eq!(
        run(
            QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME + 8,
            QUEST_WORLD_MAP_LABEL_TABLE_RUNTIME,
            &[0x1234; 16],
            0x9999
        ),
        (false, 0xBA)
    );
}
