use super::*;
use std::collections::BTreeMap;

// 요구사항: 복귀는 객체 cursor만 쓰며 일반 page는 다음 stage로 흘러가야 한다.
#[test]
fn 복귀만_cursor를_쓰고_일반page는_상태변경없이_이어간다() {
    let mut code = Vec::new();
    append_compact_return(&mut code).unwrap();
    for marker in 0..=255u8 {
        for target_offset in [4u16, 0x808, 0x1fff] {
            let mut regs = [0xCAFE_0000u32; 32];
            regs[0] = 0;
            regs[Reg::A0.0 as usize] = 0x1000;
            regs[Reg::RA.0 as usize] = 0x8000_0000;
            let original_regs = regs;
            let mut memory = BTreeMap::new();
            for (addr, bytes) in [
                (0x101c, 0x2000u32.to_le_bytes()),
                (
                    0x2000,
                    [
                        0x21,
                        marker,
                        target_offset as u8,
                        (target_offset >> 8) as u8,
                    ],
                ),
                (0x1000, 0x12345678u32.to_le_bytes()),
                (0x1050, 0x87654321u32.to_le_bytes()),
            ] {
                for (i, b) in bytes.into_iter().enumerate() {
                    memory.insert(addr + i as u32, b);
                }
            }
            let before = memory.clone();
            let mut writes = Vec::new();
            let mut pc = 0usize;
            let mut delayed = None;
            for _ in 0..32 {
                if pc >= code.len() {
                    break;
                }
                let pending = delayed.take();
                let get = |r: Reg| regs[r.0 as usize];
                match code[pc] {
                    K::Lw { rt, base, offset } => {
                        let a = get(base).wrapping_add_signed(i32::from(offset));
                        regs[rt.0 as usize] =
                            u32::from_le_bytes(std::array::from_fn(|i| memory[&(a + i as u32)]));
                    }
                    K::Lbu { rt, base, offset } => {
                        regs[rt.0 as usize] =
                            u32::from(memory[&get(base).wrapping_add_signed(i32::from(offset))]);
                    }
                    K::Lhu { rt, base, offset } => {
                        let a = get(base).wrapping_add_signed(i32::from(offset));
                        regs[rt.0 as usize] =
                            u32::from(u16::from_le_bytes([memory[&a], memory[&(a + 1)]]));
                    }
                    K::Addiu { rt, rs, imm } => {
                        regs[rt.0 as usize] = get(rs).wrapping_add_signed(i32::from(imm))
                    }
                    K::Sll { rd, rt, sa } => regs[rd.0 as usize] = get(rt) << sa,
                    K::Lui { rt, imm } => regs[rt.0 as usize] = u32::from(imm) << 16,
                    K::Ori { rt, rs, imm } => regs[rt.0 as usize] = get(rs) | u32::from(imm),
                    K::Addu { rd, rs, rt } => regs[rd.0 as usize] = get(rs).wrapping_add(get(rt)),
                    K::Bne { rs, rt, offset } => {
                        if get(rs) != get(rt) {
                            delayed = Some((pc as isize + 1 + offset as isize) as usize);
                        }
                    }
                    K::Jr { rs } => {
                        assert_eq!(rs, Reg::RA);
                        delayed = Some(code.len() + 1);
                    }
                    K::Sw { rt, base, offset } => {
                        let a = get(base).wrapping_add_signed(i32::from(offset));
                        writes.push(a);
                        for (i, b) in get(rt).to_le_bytes().into_iter().enumerate() {
                            memory.insert(a + i as u32, b);
                        }
                    }
                    other => panic!("검증기에 없는 명령 {other:?}"),
                }
                regs[0] = 0;
                pc = pending.unwrap_or(pc + 1);
            }
            let mut expected = before;
            if marker == 255 {
                let target = QUEST_SEQ_ACTIVE_RUNTIME_BASE + u32::from(target_offset) * 4;
                for (i, b) in target.to_le_bytes().into_iter().enumerate() {
                    expected.insert(0x101c + i as u32, b);
                }
                assert_eq!(writes, [0x101c]);
                assert_eq!(pc, code.len() + 1);
            } else {
                assert!(writes.is_empty());
                assert_eq!(pc, code.len());
            }
            assert_eq!(memory, expected);
            for r in [
                Reg::A0,
                Reg::A1,
                Reg::A2,
                Reg::A3,
                Reg::SP,
                Reg::RA,
                Reg::S0,
                Reg::S1,
                Reg::S2,
                Reg::S3,
                Reg::S4,
                Reg::S5,
                Reg::S6,
                Reg::S7,
                Reg::GP,
            ] {
                assert_eq!(regs[r.0 as usize], original_regs[r.0 as usize]);
            }
        }
    }
}
