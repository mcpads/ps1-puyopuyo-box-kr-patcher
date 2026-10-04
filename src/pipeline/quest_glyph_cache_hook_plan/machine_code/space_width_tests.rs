use super::*;

// 분기와 load delay를 실행해 원본 page, bank bit, slot 충돌을 함께 검사한다.
fn scale(kinds: &[K], marker: u8, slot: u16) -> u32 {
    let mut regs = [0u32; 32];
    regs[Reg::T0.0 as usize] = u32::from(marker);
    regs[Reg::T1.0 as usize] = 0x1000;
    let mut pc = 0usize;
    let mut delayed = None;
    let mut load = None;
    let mut steps = 0;
    while pc < kinds.len() {
        steps += 1;
        assert!(steps <= kinds.len() * 2);
        let target = delayed.take();
        let pending_load = load.take();
        match kinds[pc] {
            K::Andi { rt, rs, imm } => regs[rt.0 as usize] = regs[rs.0 as usize] & u32::from(imm),
            K::Lhu {
                rt,
                base: Reg::A2,
                offset: 0x66,
            } => load = Some((rt, u32::from(slot))),
            K::Addiu { rt, rs, imm } => {
                regs[rt.0 as usize] = regs[rs.0 as usize].wrapping_add_signed(i32::from(imm))
            }
            K::Sltiu { rt, rs, imm } => {
                regs[rt.0 as usize] = u32::from(regs[rs.0 as usize] < imm as i32 as u32)
            }
            K::Beq { rs, rt, offset } => {
                if regs[rs.0 as usize] == regs[rt.0 as usize] {
                    delayed = Some((pc as isize + 1 + offset as isize) as usize);
                }
            }
            K::Bne { rs, rt, offset } => {
                if regs[rs.0 as usize] != regs[rt.0 as usize] {
                    delayed = Some((pc as isize + 1 + offset as isize) as usize);
                }
            }
            K::Sra { rd, rt, sa } => {
                regs[rd.0 as usize] = ((regs[rt.0 as usize] as i32) >> sa) as u32
            }
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            } => {}
            ref kind => panic!("지원하지 않는 공백 분기 명령: {kind:?}"),
        }
        if let Some((r, value)) = pending_load {
            regs[r.0 as usize] = value;
        }
        regs[0] = 0;
        pc = target.unwrap_or(pc + 1);
    }
    regs[Reg::T1.0 as usize]
}

#[test]
fn 공백_slot과_page가_모두_일치할_때만_반각으로_표시한다() {
    for slots in [
        vec![],
        vec![None; 127],
        vec![
            Some(292),
            Some(292),
            None,
            Some(199),
            Some(292),
            None,
            Some(199),
        ],
        (0..127).map(|n| Some(n + 128)).collect(),
    ] {
        let mut kinds = Vec::new();
        append_space_width(&mut kinds, &slots).unwrap();
        for marker in 0u8..=255 {
            for slot in 0..=325 {
                let page = usize::from(marker & 0x7f);
                let expected = if page > 0 && slots.get(page - 1) == Some(&Some(slot)) {
                    0x800
                } else {
                    0x1000
                };
                assert_eq!(
                    scale(&kinds, marker, slot),
                    expected,
                    "marker={marker} slot={slot}"
                );
            }
        }
    }
}
