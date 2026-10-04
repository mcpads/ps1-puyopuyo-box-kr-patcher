use super::*;

#[test]
fn 공백_분기_앞의_불변_명령이_다르면_설치하지_않는다() {
    let map = *crate::coords::exe_map("QUEST.EXE").unwrap();
    let mut text = vec![0; map.text_len as usize];
    let start = map.text_offset(SITE_RUNTIME).unwrap();
    text[start..start + 8].copy_from_slice(&[0x84, 0xe0, 0x43, 0x94, 0xff, 0xff, 0x22, 0x32]);
    let site = assemble(&map, &text, 0x80100000).unwrap();
    assert_eq!(site.bytes(), &[0, 0, 4, 8, 0, 0, 0, 0]);
    for byte in 0..8 {
        let mut changed = text.clone();
        changed[start + byte] ^= 1;
        assert!(assemble(&map, &changed, 0x80100000).is_err());
    }
}

// 원본 branch 직전까지 load delay와 branch delay를 적용해 비교 피연산자를 관측한다.
fn operands(page: u8, code: u16) -> (u32, u32) {
    let mut kinds = Vec::new();
    append(&mut kinds, &[Some(292), Some(31), None]).unwrap();
    let mut regs = [0u32; 32];
    regs[Reg::V0.0 as usize] = 0x800c0000;
    regs[Reg::S1.0 as usize] = u32::from(code);
    let mut pc = 0;
    let mut jump = None;
    let mut load = None;
    while pc < kinds.len() {
        let next = jump.take();
        let pending = load.take();
        let r = |reg: Reg| regs[reg.0 as usize];
        let write = match kinds[pc] {
            K::Lui { rt, imm } => Some((rt, u32::from(imm) << 16)),
            K::Addiu { rt, rs, imm } => Some((rt, r(rs).wrapping_add_signed(i32::from(imm)))),
            K::Andi { rt, rs, imm } => Some((rt, r(rs) & u32::from(imm))),
            K::Ori { rt, rs, imm } => Some((rt, r(rs) | u32::from(imm))),
            K::Sll { rd, rt, sa } => Some((rd, r(rt) << sa)),
            K::Lhu { rt, base, offset } => {
                assert_eq!(r(base).wrapping_add_signed(i32::from(offset)), 0x800be084);
                load = Some((rt, 0x4081));
                None
            }
            K::Lbu { rt, base, offset } => {
                assert_eq!(
                    r(base).wrapping_add_signed(i32::from(offset)),
                    QUEST_GLYPH_CACHE_RENDERER_PAGE_RUNTIME
                );
                load = Some((rt, u32::from(page)));
                None
            }
            K::Beq { rs, rt, offset } => {
                if r(rs) == r(rt) {
                    jump = Some((pc as isize + 1 + isize::from(offset)) as usize);
                }
                None
            }
            K::J { target } => {
                assert_eq!(target, SITE_RUNTIME + 8);
                jump = Some(kinds.len());
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
    (regs[Reg::V0.0 as usize], regs[Reg::V1.0 as usize])
}

#[test]
fn 원본_공백만_건너뛰고_두_번역_bank의_같은_코드는_그린다() {
    assert_eq!(operands(0, 0x4081), (0x4081, 0x4081));
    assert_eq!(operands(1, 0x4081), (0x4081, 0x4081));
    assert_eq!(operands(0x81, 0x4081), (0x4081, 0x4081));
    for page in [2, 3, 0x82, 0x83] {
        let (code, blank) = operands(page, 0x4081);
        assert_ne!(code, blank);
    }
    assert_eq!(operands(0, 0x5c82), (0x5c82, 0x4081));
}
