use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn write_instruction(map: &ExeMap, text: &mut [u8], runtime: u32, kind: K) {
    let offset = map.text_offset(runtime).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
}

fn write_required_instruction(map: &ExeMap, text: &mut [u8], runtime: u32, kind: K, _role: &str) {
    write_instruction(map, text, runtime, kind);
}

fn prerequisite_fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];

    for (runtime, kind, role) in [
        (
            0x8005_12C8,
            K::Lui {
                rt: Reg::S1,
                imm: 0x8010,
            },
            "state root lui",
        ),
        (
            0x8005_12CC,
            K::Addiu {
                rt: Reg::S0,
                rs: Reg::S1,
                imm: QUEST_BATTLE_STATE_RUNTIME as u16 as i16,
            },
            "state root combine",
        ),
        (
            0x8005_12D0,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
            "zero fill destination",
        ),
        (
            0x8005_12D4,
            K::Addu {
                rd: Reg::A1,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
            "zero fill value",
        ),
        (
            0x8005_12D8,
            K::Jal {
                target: QUEST_BATTLE_STATE_ZERO_FILL_RUNTIME,
            },
            "zero fill call",
        ),
        (
            0x8005_12DC,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: QUEST_BATTLE_STATE_BYTES as i16,
            },
            "zero fill length",
        ),
        (
            0x8005_1300,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: i16::from(QUEST_BATTLE_STATE_INITIAL_VALUE),
            },
            "initial value",
        ),
        (
            0x8005_1308,
            K::Sb {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_BATTLE_STATE_PREREQUISITE_OFFSET,
            },
            "initial store",
        ),
        (
            0x8005_2E10,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S2,
                offset: 0x00A2,
            },
            "object state load",
        ),
        (
            0x8005_2E18,
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 59,
            },
            "state 3 branch",
        ),
        (
            0x8005_2E38,
            K::Beq {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 19,
            },
            "state 0 branch",
        ),
        (
            0x8005_2E88,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8010,
            },
            "zero check lui",
        ),
        (
            0x8005_2E8C,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: QUEST_RESIDENT_DEFEAT_PREREQUISITE_RUNTIME as u16 as i16,
            },
            "zero check load",
        ),
        (
            0x8005_2E94,
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 3,
            },
            "nonzero branch",
        ),
        (
            0x8005_2E9C,
            K::J {
                target: 0x8005_30A8,
            },
            "zero ready return jump",
        ),
        (
            0x8005_2EA0,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 1,
            },
            "zero ready return value",
        ),
        (
            0x8005_2F40,
            K::Lui {
                rt: Reg::S1,
                imm: 0x8010,
            },
            "decrement state root lui",
        ),
        (
            0x8005_2F44,
            K::Addiu {
                rt: Reg::S1,
                rs: Reg::S1,
                imm: QUEST_BATTLE_STATE_RUNTIME as u16 as i16,
            },
            "decrement state root combine",
        ),
        (
            0x8005_2FC0,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::S1,
                offset: QUEST_BATTLE_STATE_PREREQUISITE_OFFSET,
            },
            "decrement read",
        ),
        (
            0x8005_2FC8,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -1,
            },
            "decrement value",
        ),
        (
            0x8005_2FCC,
            K::Sb {
                rt: Reg::V0,
                base: Reg::S1,
                offset: QUEST_BATTLE_STATE_PREREQUISITE_OFFSET,
            },
            "decrement store",
        ),
        (
            0x8005_2C14,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8006,
            },
            "increment callback lui",
        ),
        (
            0x8005_2C18,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: QUEST_BATTLE_STATE_INCREMENT_CALLBACK_RUNTIME as u16 as i16,
            },
            "increment callback combine",
        ),
        (
            0x8006_6FE0,
            K::Lui {
                rt: Reg::V1,
                imm: 0x8010,
            },
            "increment state root lui",
        ),
        (
            0x8006_6FE4,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: QUEST_BATTLE_STATE_RUNTIME as u16 as i16,
            },
            "increment state root combine",
        ),
        (
            0x8006_6FE8,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: QUEST_BATTLE_STATE_PREREQUISITE_OFFSET,
            },
            "increment read",
        ),
        (
            0x8006_6FF0,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
            "increment value",
        ),
        (
            0x8006_6FF4,
            K::Sb {
                rt: Reg::V0,
                base: Reg::V1,
                offset: QUEST_BATTLE_STATE_PREREQUISITE_OFFSET,
            },
            "increment store",
        ),
    ] {
        write_required_instruction(map, &mut text, runtime, kind, role);
    }

    for runtime in INITIALIZER_CALLERS {
        write_instruction(
            map,
            &mut text,
            runtime,
            K::Jal {
                target: QUEST_BATTLE_STATE_INITIALIZER_RUNTIME,
            },
        );
    }
    for spec in CAPTURE_LOOPS {
        for (runtime, kind) in [
            (
                spec.root_lui_runtime,
                K::Lui {
                    rt: Reg::V0,
                    imm: 0x8010,
                },
            ),
            (
                spec.root_combine_runtime,
                K::Addiu {
                    rt: Reg::A3,
                    rs: Reg::V0,
                    imm: QUEST_BATTLE_STATE_RUNTIME as u16 as i16,
                },
            ),
            (
                spec.index_zero_runtime,
                K::Addu {
                    rd: spec.index,
                    rs: Reg::ZERO,
                    rt: Reg::ZERO,
                },
            ),
            (
                spec.buffer_lui_runtime,
                K::Lui {
                    rt: spec.buffer_base,
                    imm: 0x8012,
                },
            ),
            (
                spec.buffer_pointer_runtime,
                K::Addu {
                    rd: Reg::V1,
                    rs: spec.cursor,
                    rt: spec.buffer_base,
                },
            ),
            (
                spec.state_pointer_runtime,
                K::Addu {
                    rd: Reg::V0,
                    rs: Reg::A3,
                    rt: spec.index,
                },
            ),
            (
                spec.load_runtime,
                K::Lbu {
                    rt: Reg::V0,
                    base: Reg::V0,
                    offset: 0,
                },
            ),
            (
                spec.increment_runtime,
                K::Addiu {
                    rt: spec.index,
                    rs: spec.index,
                    imm: 1,
                },
            ),
            (
                spec.store_runtime,
                K::Sb {
                    rt: Reg::V0,
                    base: Reg::V1,
                    offset: 0x0E00,
                },
            ),
            (
                spec.limit_runtime,
                K::Sltiu {
                    rt: Reg::V0,
                    rs: spec.index,
                    imm: QUEST_BATTLE_STATE_BYTES as i16,
                },
            ),
            (
                spec.branch_runtime,
                K::Bne {
                    rs: Reg::V0,
                    rt: Reg::ZERO,
                    offset: -7,
                },
            ),
            (
                spec.cursor_increment_runtime,
                K::Addiu {
                    rt: spec.cursor,
                    rs: spec.cursor,
                    imm: 1,
                },
            ),
        ] {
            write_instruction(map, &mut text, runtime, kind);
        }
    }
    for spec in RESTORE_LOOPS {
        for (runtime, kind) in [
            (
                spec.root_lui_runtime,
                K::Lui {
                    rt: Reg::V0,
                    imm: 0x8010,
                },
            ),
            (
                spec.root_combine_runtime,
                K::Addiu {
                    rt: Reg::A3,
                    rs: Reg::V0,
                    imm: QUEST_BATTLE_STATE_RUNTIME as u16 as i16,
                },
            ),
            (
                spec.index_zero_runtime,
                K::Addu {
                    rd: Reg::A0,
                    rs: Reg::ZERO,
                    rt: Reg::ZERO,
                },
            ),
            (
                spec.buffer_lui_runtime,
                K::Lui {
                    rt: Reg::A2,
                    imm: 0x8012,
                },
            ),
            (
                spec.buffer_pointer_runtime,
                K::Addu {
                    rd: Reg::V0,
                    rs: Reg::A1,
                    rt: Reg::A2,
                },
            ),
            (
                spec.state_pointer_runtime,
                K::Addu {
                    rd: Reg::V1,
                    rs: Reg::A3,
                    rt: Reg::A0,
                },
            ),
            (
                spec.load_runtime,
                K::Lbu {
                    rt: Reg::V0,
                    base: Reg::V0,
                    offset: 0x0E00,
                },
            ),
            (
                spec.increment_runtime,
                K::Addiu {
                    rt: Reg::A0,
                    rs: Reg::A0,
                    imm: 1,
                },
            ),
            (
                spec.store_runtime,
                K::Sb {
                    rt: Reg::V0,
                    base: Reg::V1,
                    offset: 0,
                },
            ),
            (
                spec.limit_runtime,
                K::Sltiu {
                    rt: Reg::V0,
                    rs: Reg::A0,
                    imm: QUEST_BATTLE_STATE_BYTES as i16,
                },
            ),
            (
                spec.branch_runtime,
                K::Bne {
                    rs: Reg::V0,
                    rt: Reg::ZERO,
                    offset: -7,
                },
            ),
            (
                spec.cursor_increment_runtime,
                K::Addiu {
                    rt: Reg::A1,
                    rs: Reg::A1,
                    imm: 1,
                },
            ),
        ] {
            write_instruction(map, &mut text, runtime, kind);
        }
    }

    (map, text)
}

#[test]
fn 초기값_감소_증가와_snapshot_복원을_보고한다() {
    let (map, text) = prerequisite_fixture();

    let summary = verify_box_quest_resident_seq_prerequisite(map, &text).expect("전이 검증");

    assert_eq!(summary.state_runtime, 0x800F_BF48);
    assert_eq!(summary.state_bytes, 104);
    assert_eq!(summary.prerequisite_runtime, 0x800F_BF4A);
    assert_eq!(summary.initial_value, 3);
    assert_eq!(summary.decrement_store_runtime, 0x8005_2FCC);
    assert_eq!(summary.increment_store_runtime, 0x8006_6FF4);
    assert_eq!(summary.snapshot_capture_loads, SNAPSHOT_CAPTURE_LOADS);
    assert_eq!(summary.snapshot_restore_stores, SNAPSHOT_RESTORE_STORES);
}

#[test]
fn 초기값이_3이_아니면_거부한다() {
    let (map, mut text) = prerequisite_fixture();
    write_instruction(
        map,
        &mut text,
        0x8005_1300,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 2,
        },
    );

    let failure =
        verify_box_quest_resident_seq_prerequisite(map, &text).expect_err("초기값 drift 거부");

    assert!(failure.to_string().contains("initial value drift"));
}

#[test]
fn 감소_store가_다른_field를_쓰면_거부한다() {
    let (map, mut text) = prerequisite_fixture();
    write_instruction(
        map,
        &mut text,
        0x8005_2FCC,
        K::Sb {
            rt: Reg::V0,
            base: Reg::S1,
            offset: 3,
        },
    );

    let failure =
        verify_box_quest_resident_seq_prerequisite(map, &text).expect_err("감소 field drift 거부");

    assert!(failure.to_string().contains("decrement store drift"));
}

#[test]
fn snapshot_restore가_104바이트를_넘으면_거부한다() {
    let (map, mut text) = prerequisite_fixture();
    write_instruction(
        map,
        &mut text,
        0x8007_40E0,
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::A0,
            imm: 105,
        },
    );

    let failure = verify_box_quest_resident_seq_prerequisite(map, &text)
        .expect_err("snapshot 길이 drift 거부");

    assert!(failure.to_string().contains("snapshot restore limit drift"));
}
