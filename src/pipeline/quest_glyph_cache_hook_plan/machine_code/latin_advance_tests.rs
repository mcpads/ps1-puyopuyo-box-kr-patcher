use super::*;

fn cursor(marker: u8, slot: u16, effect: u8, initial: u16) -> u16 {
    let mut code = Vec::new();
    append_latin_advance(
        &mut code,
        &[vec![], vec![(200, -4), (201, -4), (203, -4), (204, -2)]],
    )
    .unwrap();
    let mut regs = [0u32; 32];
    regs[Reg::T0.0 as usize] = u32::from(marker);
    regs[Reg::T1.0 as usize] = 0x1000;
    regs[Reg::V0.0 as usize] = 123;
    regs[Reg::V1.0 as usize] = 0x100;
    regs[Reg::A2.0 as usize] = 0x200;
    let mut x = initial;
    let mut pc = 0;
    let mut branch = None;
    let mut delayed_load = None;
    let mut steps = 0;
    while pc < code.len() {
        steps += 1;
        assert!(steps <= code.len());
        let target = branch.take();
        let load = delayed_load.take();
        let get = |r: Reg| regs[r.0 as usize];
        let mut write = None;
        match code[pc] {
            K::Lbu {
                rt,
                base: Reg::V1,
                offset: 0x18,
            } => delayed_load = Some((rt, u32::from(effect))),
            K::Lhu {
                rt,
                base: Reg::A2,
                offset: 0x66,
            } => delayed_load = Some((rt, u32::from(slot))),
            K::Lhu {
                rt,
                base: Reg::V1,
                offset: 0x0A,
            } => delayed_load = Some((rt, u32::from(x))),
            K::Sh {
                rt,
                base: Reg::V1,
                offset: 0x0A,
            } => x = get(rt) as u16,
            K::Andi { rt, rs, imm } => write = Some((rt, get(rs) & u32::from(imm))),
            K::Addiu { rt, rs, imm } => {
                write = Some((rt, get(rs).wrapping_add_signed(i32::from(imm))))
            }
            K::Sltiu { rt, rs, imm } => {
                write = Some((rt, u32::from(get(rs) < i32::from(imm) as u32)))
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
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            } => {}
            other => panic!("unexpected {other:?}"),
        }
        if let Some((r, v)) = load {
            regs[r.0 as usize] = v;
        }
        if let Some((r, v)) = write {
            regs[r.0 as usize] = v;
        }
        regs[0] = 0;
        pc = target.unwrap_or(pc + 1);
    }
    assert_eq!(regs[Reg::T1.0 as usize], 0x1000, "object 배율 보존");
    assert_eq!(regs[Reg::V0.0 as usize], 123);
    assert_eq!(regs[Reg::V1.0 as usize], 0x100);
    assert_eq!(regs[Reg::A2.0 as usize], 0x200);
    x
}

#[test]
fn 선택한_page와_slot구간만_normal_효과에서_cursor를_당긴다() {
    for marker in 0..=255 {
        for slot in [0, 199, 200, 201, 202, 203, 204, 205, 325, u16::MAX] {
            for effect in [0, 1, 7, 255] {
                for initial in [0u16, 3, 120, 65535] {
                    let delta = if marker & 0x7f == 2 && effect == 0 {
                        match slot {
                            200 | 201 | 203 => -4,
                            204 => -2,
                            _ => 0,
                        }
                    } else {
                        0
                    };
                    assert_eq!(
                        cursor(marker, slot, effect, initial),
                        initial.wrapping_add_signed(delta)
                    );
                }
            }
        }
    }
}
