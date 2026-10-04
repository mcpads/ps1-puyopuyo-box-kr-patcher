//! `QUEST.EXE` resident 패배 selector의 선행 상태 전이.
//!
//! 게임상 의미를 추측하지 않고, 지원 원본에서 상태 블록 초기화, 공용
//! predicate의 감소와 0 판정, 증가 callback, snapshot 복원 경로를 검증한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{decode_slice, Instruction, InstructionKind as K, Reg};

/// resident 패배 selector writer가 0인지 검사하는 상태 byte.
pub const QUEST_RESIDENT_DEFEAT_PREREQUISITE_RUNTIME: u32 = 0x800F_BF4A;

const QUEST_BATTLE_STATE_RUNTIME: u32 = 0x800F_BF48;
const QUEST_BATTLE_STATE_BYTES: u16 = 104;
const QUEST_BATTLE_STATE_PREREQUISITE_OFFSET: i16 = 2;
const QUEST_BATTLE_STATE_INITIAL_VALUE: u8 = 3;
const QUEST_BATTLE_STATE_INITIALIZER_RUNTIME: u32 = 0x8005_10B8;
const QUEST_BATTLE_STATE_ZERO_FILL_RUNTIME: u32 = 0x8007_E6D8;
const QUEST_BATTLE_STATE_PREDICATE_RUNTIME: u32 = 0x8005_2DF8;
const QUEST_BATTLE_STATE_INCREMENT_CALLBACK_RUNTIME: u32 = 0x8006_6F4C;

const INITIALIZER_CALLERS: [u32; 3] = [0x8007_667C, 0x8007_679C, 0x8007_80C4];
const SNAPSHOT_CAPTURE_LOADS: [u32; 2] = [0x8002_F6EC, 0x8003_D6C4];
const SNAPSHOT_RESTORE_STORES: [u32; 4] = [0x8007_40DC, 0x8007_6BB4, 0x8007_7744, 0x8007_7BCC];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqPrerequisiteSummary {
    pub state_runtime: u32,
    pub state_bytes: u16,
    pub prerequisite_runtime: u32,
    pub prerequisite_offset: i16,
    pub initializer_runtime: u32,
    pub initializer_callers: Vec<u32>,
    pub initial_value: u8,
    pub initial_store_runtime: u32,
    pub predicate_runtime: u32,
    pub zero_check_load_runtime: u32,
    pub zero_ready_return_runtime: u32,
    pub decrement_store_runtime: u32,
    pub increment_callback_runtime: u32,
    pub increment_store_runtime: u32,
    pub snapshot_capture_loads: Vec<u32>,
    pub snapshot_restore_stores: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqPrerequisiteError(String);

impl fmt::Display for QuestResidentSeqPrerequisiteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentSeqPrerequisiteError {}

fn error(message: impl Into<String>) -> QuestResidentSeqPrerequisiteError {
    QuestResidentSeqPrerequisiteError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    runtime: u32,
) -> Result<&Instruction, QuestResidentSeqPrerequisiteError> {
    instructions
        .binary_search_by_key(&runtime, |instruction| instruction.addr)
        .ok()
        .and_then(|index| instructions.get(index))
        .ok_or_else(|| {
            error(format!(
                "QUEST resident prerequisite 명령이 없음: 0x{runtime:08X}"
            ))
        })
}

fn require(
    instructions: &[Instruction],
    runtime: u32,
    expected: K,
    role: &str,
) -> Result<(), QuestResidentSeqPrerequisiteError> {
    let actual = instruction_at(instructions, runtime)?;
    if actual.kind != expected {
        return Err(error(format!(
            "QUEST resident prerequisite {role} drift: 0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn verify_initializer(
    instructions: &[Instruction],
) -> Result<(), QuestResidentSeqPrerequisiteError> {
    for (runtime, expected, role) in [
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
    ] {
        require(instructions, runtime, expected, role)?;
    }

    for runtime in INITIALIZER_CALLERS {
        require(
            instructions,
            runtime,
            K::Jal {
                target: QUEST_BATTLE_STATE_INITIALIZER_RUNTIME,
            },
            "initializer caller",
        )?;
    }
    Ok(())
}

fn verify_predicate_transition(
    instructions: &[Instruction],
) -> Result<(), QuestResidentSeqPrerequisiteError> {
    for (runtime, expected, role) in [
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
    ] {
        require(instructions, runtime, expected, role)?;
    }
    Ok(())
}

fn verify_increment_callback(
    instructions: &[Instruction],
) -> Result<(), QuestResidentSeqPrerequisiteError> {
    for (runtime, expected, role) in [
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
        require(instructions, runtime, expected, role)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct CaptureLoopSpec {
    root_lui_runtime: u32,
    root_combine_runtime: u32,
    index_zero_runtime: u32,
    buffer_lui_runtime: u32,
    buffer_pointer_runtime: u32,
    state_pointer_runtime: u32,
    load_runtime: u32,
    increment_runtime: u32,
    store_runtime: u32,
    limit_runtime: u32,
    branch_runtime: u32,
    cursor_increment_runtime: u32,
    index: Reg,
    cursor: Reg,
    buffer_base: Reg,
}

const CAPTURE_LOOPS: [CaptureLoopSpec; 2] = [
    CaptureLoopSpec {
        root_lui_runtime: 0x8002_F6D4,
        root_combine_runtime: 0x8002_F6D8,
        index_zero_runtime: 0x8002_F6DC,
        buffer_lui_runtime: 0x8002_F6E0,
        buffer_pointer_runtime: 0x8002_F6E4,
        state_pointer_runtime: 0x8002_F6E8,
        load_runtime: 0x8002_F6EC,
        increment_runtime: 0x8002_F6F0,
        store_runtime: 0x8002_F6F4,
        limit_runtime: 0x8002_F6F8,
        branch_runtime: 0x8002_F6FC,
        cursor_increment_runtime: 0x8002_F700,
        index: Reg::A0,
        cursor: Reg::A1,
        buffer_base: Reg::A2,
    },
    CaptureLoopSpec {
        root_lui_runtime: 0x8003_D6AC,
        root_combine_runtime: 0x8003_D6B0,
        index_zero_runtime: 0x8003_D6B4,
        buffer_lui_runtime: 0x8003_D6B8,
        buffer_pointer_runtime: 0x8003_D6BC,
        state_pointer_runtime: 0x8003_D6C0,
        load_runtime: 0x8003_D6C4,
        increment_runtime: 0x8003_D6C8,
        store_runtime: 0x8003_D6CC,
        limit_runtime: 0x8003_D6D0,
        branch_runtime: 0x8003_D6D4,
        cursor_increment_runtime: 0x8003_D6D8,
        index: Reg::A1,
        cursor: Reg::A2,
        buffer_base: Reg::A0,
    },
];

fn verify_capture_loops(
    instructions: &[Instruction],
) -> Result<(), QuestResidentSeqPrerequisiteError> {
    for spec in CAPTURE_LOOPS {
        for (runtime, expected, role) in [
            (
                spec.root_lui_runtime,
                K::Lui {
                    rt: Reg::V0,
                    imm: 0x8010,
                },
                "snapshot capture root lui",
            ),
            (
                spec.root_combine_runtime,
                K::Addiu {
                    rt: Reg::A3,
                    rs: Reg::V0,
                    imm: QUEST_BATTLE_STATE_RUNTIME as u16 as i16,
                },
                "snapshot capture root combine",
            ),
            (
                spec.index_zero_runtime,
                K::Addu {
                    rd: spec.index,
                    rs: Reg::ZERO,
                    rt: Reg::ZERO,
                },
                "snapshot capture index",
            ),
            (
                spec.buffer_lui_runtime,
                K::Lui {
                    rt: spec.buffer_base,
                    imm: 0x8012,
                },
                "snapshot capture buffer lui",
            ),
            (
                spec.buffer_pointer_runtime,
                K::Addu {
                    rd: Reg::V1,
                    rs: spec.cursor,
                    rt: spec.buffer_base,
                },
                "snapshot capture buffer pointer",
            ),
            (
                spec.state_pointer_runtime,
                K::Addu {
                    rd: Reg::V0,
                    rs: Reg::A3,
                    rt: spec.index,
                },
                "snapshot capture state pointer",
            ),
            (
                spec.load_runtime,
                K::Lbu {
                    rt: Reg::V0,
                    base: Reg::V0,
                    offset: 0,
                },
                "snapshot capture load",
            ),
            (
                spec.increment_runtime,
                K::Addiu {
                    rt: spec.index,
                    rs: spec.index,
                    imm: 1,
                },
                "snapshot capture index increment",
            ),
            (
                spec.store_runtime,
                K::Sb {
                    rt: Reg::V0,
                    base: Reg::V1,
                    offset: 0x0E00,
                },
                "snapshot capture store",
            ),
            (
                spec.limit_runtime,
                K::Sltiu {
                    rt: Reg::V0,
                    rs: spec.index,
                    imm: QUEST_BATTLE_STATE_BYTES as i16,
                },
                "snapshot capture limit",
            ),
            (
                spec.branch_runtime,
                K::Bne {
                    rs: Reg::V0,
                    rt: Reg::ZERO,
                    offset: -7,
                },
                "snapshot capture branch",
            ),
            (
                spec.cursor_increment_runtime,
                K::Addiu {
                    rt: spec.cursor,
                    rs: spec.cursor,
                    imm: 1,
                },
                "snapshot capture cursor increment",
            ),
        ] {
            require(instructions, runtime, expected, role)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct RestoreLoopSpec {
    root_lui_runtime: u32,
    root_combine_runtime: u32,
    index_zero_runtime: u32,
    buffer_lui_runtime: u32,
    buffer_pointer_runtime: u32,
    state_pointer_runtime: u32,
    load_runtime: u32,
    increment_runtime: u32,
    store_runtime: u32,
    limit_runtime: u32,
    branch_runtime: u32,
    cursor_increment_runtime: u32,
}

const RESTORE_LOOPS: [RestoreLoopSpec; 4] = [
    restore_loop(0x8007_40BC),
    restore_loop(0x8007_6B94),
    restore_loop(0x8007_7724),
    restore_loop(0x8007_7BAC),
];

const fn restore_loop(start: u32) -> RestoreLoopSpec {
    RestoreLoopSpec {
        root_lui_runtime: start,
        root_combine_runtime: start + 4,
        index_zero_runtime: start + 8,
        buffer_lui_runtime: start + 12,
        buffer_pointer_runtime: start + 16,
        state_pointer_runtime: start + 20,
        load_runtime: start + 24,
        increment_runtime: start + 28,
        store_runtime: start + 32,
        limit_runtime: start + 36,
        branch_runtime: start + 40,
        cursor_increment_runtime: start + 44,
    }
}

fn verify_restore_loops(
    instructions: &[Instruction],
) -> Result<(), QuestResidentSeqPrerequisiteError> {
    for spec in RESTORE_LOOPS {
        for (runtime, expected, role) in [
            (
                spec.root_lui_runtime,
                K::Lui {
                    rt: Reg::V0,
                    imm: 0x8010,
                },
                "snapshot restore root lui",
            ),
            (
                spec.root_combine_runtime,
                K::Addiu {
                    rt: Reg::A3,
                    rs: Reg::V0,
                    imm: QUEST_BATTLE_STATE_RUNTIME as u16 as i16,
                },
                "snapshot restore root combine",
            ),
            (
                spec.index_zero_runtime,
                K::Addu {
                    rd: Reg::A0,
                    rs: Reg::ZERO,
                    rt: Reg::ZERO,
                },
                "snapshot restore index",
            ),
            (
                spec.buffer_lui_runtime,
                K::Lui {
                    rt: Reg::A2,
                    imm: 0x8012,
                },
                "snapshot restore buffer lui",
            ),
            (
                spec.buffer_pointer_runtime,
                K::Addu {
                    rd: Reg::V0,
                    rs: Reg::A1,
                    rt: Reg::A2,
                },
                "snapshot restore buffer pointer",
            ),
            (
                spec.state_pointer_runtime,
                K::Addu {
                    rd: Reg::V1,
                    rs: Reg::A3,
                    rt: Reg::A0,
                },
                "snapshot restore state pointer",
            ),
            (
                spec.load_runtime,
                K::Lbu {
                    rt: Reg::V0,
                    base: Reg::V0,
                    offset: 0x0E00,
                },
                "snapshot restore load",
            ),
            (
                spec.increment_runtime,
                K::Addiu {
                    rt: Reg::A0,
                    rs: Reg::A0,
                    imm: 1,
                },
                "snapshot restore index increment",
            ),
            (
                spec.store_runtime,
                K::Sb {
                    rt: Reg::V0,
                    base: Reg::V1,
                    offset: 0,
                },
                "snapshot restore store",
            ),
            (
                spec.limit_runtime,
                K::Sltiu {
                    rt: Reg::V0,
                    rs: Reg::A0,
                    imm: QUEST_BATTLE_STATE_BYTES as i16,
                },
                "snapshot restore limit",
            ),
            (
                spec.branch_runtime,
                K::Bne {
                    rs: Reg::V0,
                    rt: Reg::ZERO,
                    offset: -7,
                },
                "snapshot restore branch",
            ),
            (
                spec.cursor_increment_runtime,
                K::Addiu {
                    rt: Reg::A1,
                    rs: Reg::A1,
                    imm: 1,
                },
                "snapshot restore cursor increment",
            ),
        ] {
            require(instructions, runtime, expected, role)?;
        }
    }
    Ok(())
}

/// 지원 `QUEST.EXE`에서 패배 selector 선행 byte의 알려진 전이를 검증한다.
pub fn verify_box_quest_resident_seq_prerequisite(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestResidentSeqPrerequisiteSummary, QuestResidentSeqPrerequisiteError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error("QUEST resident prerequisite source 입력 불일치"));
    }
    let instructions = decode_slice(map.load_addr, text)
        .ok_or_else(|| error("QUEST resident prerequisite text가 4바이트 정렬이 아님"))?;

    verify_initializer(&instructions)?;
    verify_predicate_transition(&instructions)?;
    verify_increment_callback(&instructions)?;
    verify_capture_loops(&instructions)?;
    verify_restore_loops(&instructions)?;

    Ok(QuestResidentSeqPrerequisiteSummary {
        state_runtime: QUEST_BATTLE_STATE_RUNTIME,
        state_bytes: QUEST_BATTLE_STATE_BYTES,
        prerequisite_runtime: QUEST_RESIDENT_DEFEAT_PREREQUISITE_RUNTIME,
        prerequisite_offset: QUEST_BATTLE_STATE_PREREQUISITE_OFFSET,
        initializer_runtime: QUEST_BATTLE_STATE_INITIALIZER_RUNTIME,
        initializer_callers: INITIALIZER_CALLERS.to_vec(),
        initial_value: QUEST_BATTLE_STATE_INITIAL_VALUE,
        initial_store_runtime: 0x8005_1308,
        predicate_runtime: QUEST_BATTLE_STATE_PREDICATE_RUNTIME,
        zero_check_load_runtime: 0x8005_2E8C,
        zero_ready_return_runtime: 0x8005_2E9C,
        decrement_store_runtime: 0x8005_2FCC,
        increment_callback_runtime: QUEST_BATTLE_STATE_INCREMENT_CALLBACK_RUNTIME,
        increment_store_runtime: 0x8006_6FF4,
        snapshot_capture_loads: SNAPSHOT_CAPTURE_LOADS.to_vec(),
        snapshot_restore_stores: SNAPSHOT_RESTORE_STORES.to_vec(),
    })
}

#[cfg(test)]
#[path = "quest_resident_seq_prerequisite_tests.rs"]
mod tests;
