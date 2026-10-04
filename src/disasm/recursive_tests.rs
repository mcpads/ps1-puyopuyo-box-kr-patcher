use super::*;
use crate::coords::LogicalFile;
use crate::isa::{encode_all, InstructionKind as K, Reg};

const BASE: u32 = 0x8001_0000;

fn nop() -> K {
    K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    }
}

fn map_for(text: &[u8]) -> ExeMap {
    ExeMap {
        name: "TEST.EXE",
        load_addr: BASE,
        text_len: text.len() as u32,
        entry_pc: BASE,
        file: LogicalFile::new(100, 0x800 + text.len() as u32),
    }
}

#[test]
fn follows_a_call_and_returns() {
    // 0x00 jal 0x80010010
    // 0x04 nop            (딜레이 슬롯)
    // 0x08 jr $ra
    // 0x0C nop
    // 0x10 jr $ra         (호출 대상)
    // 0x14 nop
    let text = encode_all(&[
        K::Jal {
            target: BASE + 0x10,
        },
        nop(),
        K::Jr { rs: Reg::RA },
        nop(),
        K::Jr { rs: Reg::RA },
        nop(),
    ]);
    let map = map_for(&text);
    let t = trace(&map, &text, &[]);

    assert!(t.call_targets.contains(&(BASE + 0x10)));
    assert!(t.instructions.contains_key(&(BASE + 0x10)));
    assert!(t.coverage_is_complete());
}

#[test]
fn delay_slot_after_unconditional_jump_is_still_decoded() {
    // j는 뒤로 이어지지 않지만 딜레이 슬롯은 실행된다.
    let text = encode_all(&[
        K::J {
            target: BASE + 0x0C,
        },
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 1,
        }, // 딜레이 슬롯
        K::Break { code: 0 }, // 도달하지 않아야 함
        K::Jr { rs: Reg::RA },
        nop(),
    ]);
    let map = map_for(&text);
    let t = trace(&map, &text, &[]);

    assert!(
        t.instructions.contains_key(&(BASE + 0x04)),
        "딜레이 슬롯 미해석"
    );
    assert!(
        !t.instructions.contains_key(&(BASE + 0x08)),
        "무조건 분기 뒤로 이어졌다"
    );
    assert!(t.instructions.contains_key(&(BASE + 0x0C)));
}

#[test]
fn conditional_branch_explores_both_paths() {
    // beq는 성립·미성립 양쪽으로 이어진다.
    let text = encode_all(&[
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 2, // → BASE+0x0C
        },
        nop(),
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 7,
        }, // 미성립 경로
        K::Jr { rs: Reg::RA },
        nop(),
    ]);
    let map = map_for(&text);
    let t = trace(&map, &text, &[]);

    assert!(
        t.instructions.contains_key(&(BASE + 0x08)),
        "미성립 경로 누락"
    );
    assert!(
        t.instructions.contains_key(&(BASE + 0x0C)),
        "성립 경로 누락"
    );
}

#[test]
fn indirect_branch_is_recorded_and_blocks_completeness_claims() {
    let text = encode_all(&[K::Jr { rs: Reg::T2 }, nop()]);
    let map = map_for(&text);
    let t = trace(&map, &text, &[]);

    assert!(t.indirect_sites.contains(&BASE));
    assert!(
        !t.coverage_is_complete(),
        "간접 분기가 남았는데 완전하다고 주장했다"
    );
}

#[test]
fn extra_seeds_reach_code_the_entry_cannot() {
    // 엔트리는 즉시 리턴하고, 0x08은 정적으로 도달 불가다.
    let text = encode_all(&[
        K::Jr { rs: Reg::RA },
        nop(),
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 9,
        },
        K::Jr { rs: Reg::RA },
        nop(),
    ]);
    let map = map_for(&text);

    let without = trace(&map, &text, &[]);
    assert!(!without.instructions.contains_key(&(BASE + 0x08)));

    let with = trace(&map, &text, &[BASE + 0x08]);
    assert!(with.instructions.contains_key(&(BASE + 0x08)));
}

#[test]
fn targets_outside_the_load_range_are_recorded_not_followed() {
    let text = encode_all(&[
        K::Jal {
            target: 0x8FFF_0000,
        },
        nop(),
        K::Jr { rs: Reg::RA },
        nop(),
    ]);
    let map = map_for(&text);
    let t = trace(&map, &text, &[]);

    assert!(t.out_of_range.iter().any(|&(_, to)| to == 0x8FFF_0000));
    assert!(t.call_targets.contains(&0x8FFF_0000));
}

#[test]
fn kseg1_seed_is_normalized_to_kseg0() {
    // 훅이 stale I-cache를 피해 KSEG1로 실행되면 시드도 KSEG1로 들어온다.
    let text = encode_all(&[
        K::Jr { rs: Reg::RA },
        nop(),
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 9,
        },
        K::Jr { rs: Reg::RA },
        nop(),
    ]);
    let map = map_for(&text);
    let t = trace(&map, &text, &[0xA001_0008]);

    assert!(
        t.instructions.contains_key(&(BASE + 0x08)),
        "KSEG1 시드가 정규화되지 않았다"
    );
    // 정규화된 주소로 기록해야 KSEG0 관측과 대조할 수 있다.
    assert!(!t.instructions.contains_key(&0xA001_0008));
}

#[test]
fn a_delay_slot_that_is_also_a_branch_target_keeps_its_flow() {
    // 0x00 beq → 0x04 (딜레이 슬롯을 타깃으로 삼는 형태)
    // 0x04 nop        (딜레이 슬롯이자 분기 타깃)
    // 0x08 addiu      (여기까지 이어져야 한다)
    let text = encode_all(&[
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 0, // → BASE+0x04
        },
        nop(),
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 5,
        },
        K::Jr { rs: Reg::RA },
        nop(),
    ]);
    let map = map_for(&text);
    let t = trace(&map, &text, &[]);

    assert!(
        t.instructions.contains_key(&(BASE + 0x08)),
        "합류 뒤 흐름 유실"
    );
    assert!(t.instructions.contains_key(&(BASE + 0x0C)));
}
