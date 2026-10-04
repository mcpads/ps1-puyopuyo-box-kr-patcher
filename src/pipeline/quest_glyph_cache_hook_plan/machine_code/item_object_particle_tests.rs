use super::*;
use std::collections::BTreeMap;

// 생성 명령의 분기·load delay·callee clobber를 실행하여 관측 가능한 호출과
// frame 보존을 검사한다. 그래픽 renderer 자체는 실제 emulator 표본이 담당한다.
fn observe(caller: u32, name: &[u16]) -> Vec<u16> {
    observe_mode(caller, name, None)
}
fn observe_mode(caller: u32, name: &[u16], direct_finished: Option<bool>) -> Vec<u16> {
    let mut code = Vec::new();
    let builder = if direct_finished.is_some() {
        append_discard_object_particle
    } else {
        append_item_object_particle
    };
    builder(&mut code, &[(0x1234, 0x4567), (0x2345, 0x5678)]).unwrap();
    crate::isa::verify_typed_source(
        crate::isa::IsaProfile::DECLARED,
        0x80100000,
        "test::item_particle",
        &code,
    )
    .unwrap();
    let mut registers = [0u32; 32];
    registers[Reg::SP.0 as usize] = 0x2000;
    registers[Reg::V0.0 as usize] = 0xfeed;
    let mut memory = BTreeMap::from([(0x2000 + 60, caller), (0x2000 + 20, 0x3000)]);
    if let Some(finished) = direct_finished {
        for offset in (16..64).step_by(4) {
            memory.insert(0x2000 + offset, 0);
        }
        memory.insert(0x2000 + 60, caller);
        memory.insert(0x2000 + 20, u32::from(name[0]));
        memory.insert(0x2000 + 48, 8);
        memory.insert(0x2000 + 52, if finished { 8 } else { 2 });
    }
    for (i, &ch) in name.iter().enumerate() {
        memory.insert(0x3000 + i as u32 * 2, u32::from(ch));
    }
    let mut pc = 0usize;
    let mut jump = None;
    let mut delayed_load = None;
    let mut draws = Vec::new();
    let mut steps = 0;
    while pc < code.len() {
        steps += 1;
        assert!(steps < 2000, "bounded execution");
        let next = jump.take();
        let load = delayed_load.take();
        let get = |r: Reg| registers[r.0 as usize];
        let mut write = None;
        match code[pc] {
            K::Addiu { rt, rs, imm } => {
                write = Some((rt, get(rs).wrapping_add_signed(i32::from(imm))))
            }
            K::Addu { rd, rs, rt } => write = Some((rd, get(rs).wrapping_add(get(rt)))),
            K::Ori { rt, rs, imm } => write = Some((rt, get(rs) | u32::from(imm))),
            K::Lui { rt, imm } => write = Some((rt, u32::from(imm) << 16)),
            K::Lw { rt, base, offset } | K::Lhu { rt, base, offset } => {
                delayed_load = Some((
                    rt,
                    *memory
                        .get(&get(base).wrapping_add_signed(i32::from(offset)))
                        .expect("bounded mapped read"),
                ))
            }
            K::Sw { rt, base, offset }
            | K::Sh { rt, base, offset }
            | K::Sb { rt, base, offset } => {
                memory.insert(get(base).wrapping_add_signed(i32::from(offset)), get(rt));
            }
            K::Beq { rs, rt, offset } => {
                if get(rs) == get(rt) {
                    jump = Some((pc as isize + 1 + offset as isize) as usize)
                }
            }
            K::Bne { rs, rt, offset } => {
                if get(rs) != get(rt) {
                    jump = Some((pc as isize + 1 + offset as isize) as usize)
                }
            }
            K::Sltu { rd, rs, rt } => write = Some((rd, u32::from(get(rs) < get(rt)))),
            K::Jr { rs } => {
                assert_eq!(get(rs), caller);
                jump = Some(code.len());
            }
            K::Sll { rd, rt, sa } => write = Some((rd, get(rt) << sa)),
            K::Jal { target } => {
                assert_eq!(target, QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME);
                assert_eq!(get(Reg::A0), 0);
                let text = *memory.get(&get(Reg::A1)).unwrap();
                assert_eq!(text >> 16, 0, "NUL after particle");
                draws.push(text as u16);
                if direct_finished.is_some() {
                    draws.push(*memory.get(&(get(Reg::A1) + 2)).unwrap() as u16);
                    assert_eq!(memory.get(&(get(Reg::A1) + 4)), Some(&0));
                }
                // callee가 argument 영역과 caller-saved register를 사용해도 보존돼야 한다.
                for offset in [0, 4, 8, 12] {
                    memory.insert(get(Reg::SP) + offset, 0xdead);
                }
                write = Some((Reg::V0, 0xbad));
            }
            other => panic!("unexpected {other:?}"),
        }
        if let Some((r, v)) = load {
            registers[r.0 as usize] = v;
        }
        if let Some((r, v)) = write {
            registers[r.0 as usize] = v;
        }
        registers[0] = 0;
        pc = next.unwrap_or(pc + 1);
    }
    let returned = direct_finished.is_some() && !draws.is_empty();
    assert_eq!(
        registers[Reg::SP.0 as usize],
        if returned {
            0x2000
                + crate::pipeline::quest_glyph_cache_hook_plan::QUEST_GLYPH_CACHE_FRAME_BYTES as u32
        } else {
            0x2000
        }
    );
    if !returned {
        assert_eq!(registers[Reg::V0.0 as usize], 0xfeed);
    }
    draws
}

#[test]
fn 사용문은_끝음절별_조사를_한번_그리고_원래_frame과_반환값을_보존한다() {
    assert_eq!(observe(0x80053390, &[0x9999, 0x1234, 0]), vec![0x4567]);
    assert_eq!(observe(0x80053390, &[0x2345, 0]), vec![0x5678]);
    assert_eq!(observe(0x800618fc, &[0x2345, 0]), vec![0x5678]);
}
#[test]
fn 다른_이름_caller와_미등록_끝음절에는_붙이지_않고_nul_누락은_유한하게_끝난다() {
    assert!(observe(0x800614d8, &[]).is_empty());
    assert!(observe(0x80053390, &[0x9999, 0]).is_empty());
    assert!(observe(0x80053390, &[0]).is_empty());
    assert!(observe(0x80053390, &[0x1234; 64]).is_empty());
}

#[test]
fn 버리기_확인의_마지막_이름글자에만_조사를_붙인다() {
    assert_eq!(
        observe_mode(0x80037c60, &[0x1234], Some(true)),
        vec![0x1234, 0x4567]
    );
    assert_eq!(
        observe_mode(0x80037c60, &[0x2345], Some(true)),
        vec![0x2345, 0x5678]
    );
    assert!(observe_mode(0x80037c60, &[0x1234], Some(false)).is_empty());
    assert!(observe_mode(0x80037c61, &[0x1234], Some(true)).is_empty());
    assert!(observe_mode(0x80037c60, &[0x9999], Some(true)).is_empty());
}
