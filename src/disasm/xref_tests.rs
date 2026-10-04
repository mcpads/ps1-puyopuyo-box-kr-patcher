use super::*;
use crate::isa::{decode_slice, encode_all, InstructionKind as K, Reg};

const BASE: u32 = 0x8001_0000;

fn program(kinds: &[K]) -> Vec<crate::isa::Instruction> {
    decode_slice(BASE, &encode_all(kinds)).expect("정렬된 프로그램")
}

fn nop() -> K {
    K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    }
}

#[test]
fn lui_ori_pair_forms_an_unsigned_address() {
    let p = program(&[
        K::Lui {
            rt: Reg::T0,
            imm: 0x8009,
        },
        K::Ori {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 0xBD50,
        },
    ]);
    let loads = find_address_loads(&p, DEFAULT_WINDOW);
    assert_eq!(loads.len(), 1);
    assert_eq!(loads[0].value, 0x8009_BD50);
    assert_eq!(loads[0].form, LoadForm::Ori);
    assert_eq!(loads[0].lui_addr, BASE);
}

#[test]
fn lui_addiu_pair_sign_extends_the_low_half() {
    // 0x8002_0000 + (-0x10) = 0x8001_FFF0
    let p = program(&[
        K::Lui {
            rt: Reg::T0,
            imm: 0x8002,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: -0x10,
        },
    ]);
    let loads = find_address_loads(&p, DEFAULT_WINDOW);
    assert_eq!(loads[0].value, 0x8001_FFF0);
    assert_eq!(loads[0].form, LoadForm::Addiu);
}

#[test]
fn intervening_instructions_do_not_break_the_pair() {
    let p = program(&[
        K::Lui {
            rt: Reg::T0,
            imm: 0x8009,
        },
        nop(),
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::ZERO,
            imm: 1,
        },
        K::Ori {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 0x1234,
        },
    ]);
    let loads = find_address_loads(&p, DEFAULT_WINDOW);
    assert_eq!(loads.len(), 1);
    assert_eq!(loads[0].value, 0x8009_1234);
    assert_eq!(loads[0].combine_addr, BASE + 0x0C);
}

#[test]
fn clobbering_the_register_ends_the_search() {
    let p = program(&[
        K::Lui {
            rt: Reg::T0,
            imm: 0x8009,
        },
        // $t0를 다른 값으로 덮어쓰면 상위 비트는 죽는다.
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::ZERO,
            imm: 1,
        },
        K::Ori {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 0x1234,
        },
    ]);
    let loads = find_address_loads(&p, DEFAULT_WINDOW);
    // addiu가 $zero 기준이므로 lui와 짝이 아니다. 그 뒤로는 탐색이 멈춘다.
    assert!(loads.iter().all(|l| l.value != 0x8009_1234));
}

#[test]
fn memory_offset_form_is_detected() {
    // lui $t0, 0x800E ; lw $v0, 0x6948($t0)  → 0x800E6948 참조
    let p = program(&[
        K::Lui {
            rt: Reg::T0,
            imm: 0x800E,
        },
        K::Lw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: 0x6948,
        },
    ]);
    let loads = find_address_loads(&p, DEFAULT_WINDOW);
    assert_eq!(loads.len(), 1);
    assert_eq!(loads[0].value, 0x800E_6948);
    assert_eq!(loads[0].form, LoadForm::MemoryOffset);
}

#[test]
fn negative_memory_offset_sign_extends() {
    // lui $t0, 0x8010 ; lw $v0, -4($t0) → 0x800FFFFC
    let p = program(&[
        K::Lui {
            rt: Reg::T0,
            imm: 0x8010,
        },
        K::Lw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: -4,
        },
    ]);
    let loads = find_address_loads(&p, DEFAULT_WINDOW);
    assert_eq!(loads[0].value, 0x800F_FFFC);
}

#[test]
fn one_base_can_reach_several_fields() {
    let p = program(&[
        K::Lui {
            rt: Reg::T0,
            imm: 0x800E,
        },
        K::Lw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: 0x6940,
        },
        K::Lw {
            rt: Reg::V1,
            base: Reg::T0,
            offset: 0x6944,
        },
    ]);
    let loads = find_address_loads(&p, DEFAULT_WINDOW);
    let values: Vec<u32> = loads.iter().map(|l| l.value).collect();
    assert!(values.contains(&0x800E_6940));
    assert!(values.contains(&0x800E_6944));
}

#[test]
fn group_by_target_collects_reference_sites() {
    let p = program(&[
        K::Lui {
            rt: Reg::T0,
            imm: 0x8009,
        },
        K::Ori {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 0xBD50,
        },
        K::Lui {
            rt: Reg::T1,
            imm: 0x8009,
        },
        K::Ori {
            rt: Reg::T1,
            rs: Reg::T1,
            imm: 0xBD50,
        },
    ]);
    let grouped = group_by_target(&find_address_loads(&p, DEFAULT_WINDOW));
    assert_eq!(grouped[&0x8009_BD50].len(), 2);
}
