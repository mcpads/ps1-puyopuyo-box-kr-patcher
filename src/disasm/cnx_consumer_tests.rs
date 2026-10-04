use crate::coords::LogicalFile;
use crate::isa::{encode_all, InstructionKind as K};

use super::*;

const SELECTOR_TEST_BASE: u32 = 0x8001_0000;
const SELECTOR_TEST_MAGIC: [u32; 3] = [
    SELECTOR_TEST_BASE,
    SELECTOR_TEST_BASE + 0x20,
    SELECTOR_TEST_BASE + 0x200,
];
const SELECTOR_TEST_CORE_CALLS: [u32; 2] = [SELECTOR_TEST_BASE + 0x98, SELECTOR_TEST_BASE + 0x2BC];

fn test_map(load_addr: u32, len: usize) -> ExeMap {
    ExeMap {
        name: "TEST.EXE",
        load_addr,
        text_len: len as u32,
        entry_pc: load_addr,
        file: LogicalFile::new(0, (len + 0x800) as u32),
    }
}

fn nop() -> K {
    K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    }
}

#[test]
fn magic은_사용_register와_무관하게_같은_register의_lui_ori를_센다() {
    let base = 0x8001_0000;
    let mut kinds = vec![nop(); 16];
    kinds[0] = K::Lui {
        rt: Reg::A1,
        imm: 0x0258,
    };
    kinds[2] = K::Ori {
        rt: Reg::A1,
        rs: Reg::A1,
        imm: 0x4E43,
    };
    kinds[4] = K::Lui {
        rt: Reg::V0,
        imm: 0x0258,
    };
    kinds[5] = K::Ori {
        rt: Reg::V0,
        rs: Reg::V0,
        imm: 0x4E43,
    };
    kinds[8] = K::Lui {
        rt: Reg::V1,
        imm: 0x0258,
    };
    kinds[9] = K::Ori {
        rt: Reg::A0,
        rs: Reg::A0,
        imm: 0x4E43,
    };
    let text = encode_all(&kinds);
    let found = scan_magic_checks("TEST.EXE", &test_map(base, text.len()), &text).unwrap();
    assert_eq!(found, vec![base, base + 0x10]);
}

#[test]
fn core_call은_전체_text에서_정확한_target만_센다() {
    let base = 0x8001_0000;
    let core = base + 0x100;
    let mut kinds = vec![nop(); 24];
    kinds[8] = K::Jal { target: core };
    kinds[12] = K::Jal {
        target: base + 0x200,
    };
    kinds[20] = K::Jal { target: core };
    let text = encode_all(&kinds);
    let found = scan_core_calls("TEST.EXE", &test_map(base, text.len()), &text, core).unwrap();
    assert_eq!(found, vec![base + 0x20, base + 0x50]);
}

#[test]
fn 공유_shape는_절대_jump와_gp_base_재배치를_정규화한다() {
    let first_base = 0x8001_0000;
    let second_base = 0x8002_0000;
    let first_kinds = [
        K::J {
            target: first_base + 0x10,
        },
        K::Lw {
            rt: Reg::V1,
            base: Reg::GP,
            offset: 0x0100,
        },
        K::Sw {
            rt: Reg::A0,
            base: Reg::GP,
            offset: 0x0104,
        },
    ];
    let second_kinds = [
        K::J {
            target: second_base + 0x10,
        },
        K::Lw {
            rt: Reg::V1,
            base: Reg::GP,
            offset: 0x0500,
        },
        K::Sw {
            rt: Reg::A0,
            base: Reg::GP,
            offset: 0x0504,
        },
    ];
    let first = crate::isa::decode_slice(first_base, &encode_all(&first_kinds)).unwrap();
    let second = crate::isa::decode_slice(second_base, &encode_all(&second_kinds)).unwrap();
    assert_eq!(
        normalized_core_shape(&first, first_base),
        normalized_core_shape(&second, second_base)
    );

    let changed = [
        second_kinds[0],
        second_kinds[1],
        K::Sw {
            rt: Reg::A1,
            base: Reg::GP,
            offset: 0x0504,
        },
    ];
    let changed = crate::isa::decode_slice(second_base, &encode_all(&changed)).unwrap();
    assert_ne!(
        normalized_core_shape(&first, first_base),
        normalized_core_shape(&changed, second_base)
    );
}

fn selector_test_spec() -> ExeCnxConsumerSpec {
    ExeCnxConsumerSpec {
        exe: "TEST.EXE",
        core: SELECTOR_TEST_BASE + 0x300,
        core_words: 1,
        magic_checks: &SELECTOR_TEST_MAGIC,
        core_calls: &SELECTOR_TEST_CORE_CALLS,
        family: CnxCoreFamily::Shared,
        paths: CnxPathSpec {
            direct_caller: SELECTOR_TEST_BASE + 0x80,
            streaming_init_caller: SELECTOR_TEST_BASE + 0x38,
            streaming_continue_callers: [SELECTOR_TEST_BASE + 0x50, SELECTOR_TEST_BASE + 0x90],
            selector_gp_offset: 0x0120,
            selector_loads: [
                SELECTOR_TEST_BASE + 0x20,
                SELECTOR_TEST_BASE + 0x4C,
                SELECTOR_TEST_BASE + 0x88,
            ],
            selector_enable_store: SELECTOR_TEST_BASE + 0xB0,
            selector_initializer_caller: SELECTOR_TEST_BASE + 0xC0,
        },
    }
}

fn selector_test_kinds(enable_value: i16) -> Vec<K> {
    let mut kinds = vec![nop(); 64];
    kinds[8] = K::Lw {
        rt: Reg::V0,
        base: Reg::GP,
        offset: 0x0120,
    };
    kinds[10] = K::Beq {
        rs: Reg::V0,
        rt: Reg::ZERO,
        offset: 0x0013,
    };
    kinds[14] = K::Jal {
        target: SELECTOR_TEST_BASE + 0x200,
    };
    kinds[19] = K::Lw {
        rt: Reg::A3,
        base: Reg::GP,
        offset: 0x0120,
    };
    kinds[20] = K::Jal {
        target: SELECTOR_TEST_BASE + 0x26C,
    };
    kinds[32] = K::Jal {
        target: SELECTOR_TEST_BASE + 0x20,
    };
    kinds[34] = K::Lw {
        rt: Reg::A3,
        base: Reg::GP,
        offset: 0x0120,
    };
    kinds[36] = K::Jal {
        target: SELECTOR_TEST_BASE + 0x26C,
    };
    kinds[43] = K::Addiu {
        rt: Reg::V1,
        rs: Reg::ZERO,
        imm: enable_value,
    };
    kinds[44] = K::Sw {
        rt: Reg::V1,
        base: Reg::GP,
        offset: 0x0120,
    };
    kinds
}

#[test]
fn selector_shape는_0을_direct로_보내고_1만_enable해야_한다() {
    let kinds = selector_test_kinds(1);
    let instructions = crate::isa::decode_slice(SELECTOR_TEST_BASE, &encode_all(&kinds)).unwrap();
    verify_selector_shape("TEST.EXE", &instructions, selector_test_spec()).unwrap();

    let changed = selector_test_kinds(0);
    let changed = crate::isa::decode_slice(SELECTOR_TEST_BASE, &encode_all(&changed)).unwrap();
    assert!(matches!(
        verify_selector_shape("TEST.EXE", &changed, selector_test_spec()),
        Err(CnxConsumerError::SelectorShapeMismatch { .. })
    ));
}

#[test]
fn selector_access는_같은_gp_offset의_부분_width도_숨기지_않는다() {
    let mut kinds = vec![nop(); 8];
    kinds[0] = K::Lw {
        rt: Reg::V0,
        base: Reg::GP,
        offset: 0x0120,
    };
    kinds[1] = K::Lbu {
        rt: Reg::V1,
        base: Reg::GP,
        offset: 0x0120,
    };
    kinds[2] = K::Sw {
        rt: Reg::V0,
        base: Reg::GP,
        offset: 0x0120,
    };
    kinds[3] = K::Sh {
        rt: Reg::V1,
        base: Reg::GP,
        offset: 0x0120,
    };
    kinds[4] = K::Lw {
        rt: Reg::V0,
        base: Reg::GP,
        offset: 0x0124,
    };
    let instructions = crate::isa::decode_slice(SELECTOR_TEST_BASE, &encode_all(&kinds)).unwrap();
    assert_eq!(
        selector_accesses(&instructions, 0x0120, true),
        vec![SELECTOR_TEST_BASE, SELECTOR_TEST_BASE + 4]
    );
    assert_eq!(
        selector_accesses(&instructions, 0x0120, false),
        vec![SELECTOR_TEST_BASE + 8, SELECTOR_TEST_BASE + 12]
    );
}

#[test]
fn selector_gp_relative_참조는_주소_escape도_숨기지_않는다() {
    let kinds = [
        K::Lw {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 0x0120,
        },
        K::Sw {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 0x0120,
        },
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::GP,
            imm: 0x0120,
        },
        K::Lw {
            rt: Reg::V1,
            base: Reg::GP,
            offset: 0x0124,
        },
    ];
    let instructions = crate::isa::decode_slice(SELECTOR_TEST_BASE, &encode_all(&kinds)).unwrap();
    assert_eq!(
        selector_gp_relative_references(&instructions, 0x0120),
        vec![
            SELECTOR_TEST_BASE,
            SELECTOR_TEST_BASE + 4,
            SELECTOR_TEST_BASE + 8
        ]
    );
}
