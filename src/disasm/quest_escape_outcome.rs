//! QUEST `OPC8`의 도주 결과 생산 경로.
//!
//! 조우 도주 stream은 `OPC8` 뒤 `OP14`에서 interpreter `+0x34`를
//! 비교한다. 이 모듈은 `OPC8`가 세 참가자 보정을 합친 값을 현재 비교값과
//! 대조하고, 강제 실패 상태 또는 세 난수 규칙을 거쳐 그 필드에 `0/1`을
//! 쓰는 원본 명령을 검증한다. 비교값과 강제 실패 상태를 누가 자연 실행에서
//! 만드는지는 이 경로만으로 단정하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::quest_seq_opcode_handler;

use super::linear;

pub const QUEST_ESCAPE_OUTCOME_OPCODE: u8 = 0xC8;
pub const QUEST_ESCAPE_OUTCOME_HANDLER_RUNTIME: u32 = 0x8007_6C28;
pub const QUEST_ESCAPE_OUTCOME_RECORD_BYTES: usize = 4;
pub const QUEST_ESCAPE_OUTCOME_FIELD_OFFSET: i16 = 0x34;
pub const QUEST_ESCAPE_STATE_BASE_RUNTIME: u32 = 0x800D_B6C0;
pub const QUEST_ESCAPE_COMPARISON_VALUE_RUNTIME: u32 = 0x800D_B790;
pub const QUEST_ESCAPE_AGGREGATE_VALUE_RUNTIME: u32 = 0x800D_B7B4;
pub const QUEST_ESCAPE_FAILURE_FLAGS_RUNTIME: u32 = 0x800D_B7B8;
pub const QUEST_ESCAPE_FORCED_FAILURE_STATE_RUNTIME: u32 = 0x800D_B940;
pub const QUEST_ESCAPE_PARTICIPANT_STATE_RUNTIME: u32 = 0x800F_BF48;
pub const QUEST_ESCAPE_PARTICIPANT_TABLE_RUNTIME: u32 = 0x801A_1A6C;
pub const QUEST_ESCAPE_RANDOM_RUNTIME: u32 = 0x8007_E6FC;
pub const QUEST_ESCAPE_FAILURE_FLAG_MASK: u16 = 0x1000;

const QUEST_ESCAPE_PARTICIPANT_BASE_VALUE_OFFSET: i16 = 0x08;
const QUEST_ESCAPE_PARTICIPANT_ID_OFFSET: i16 = 0x62;
const QUEST_ESCAPE_PARTICIPANT_VALUE_OFFSET: i16 = 0x03;
const QUEST_ESCAPE_PARTICIPANT_COUNT: u8 = 3;
const QUEST_ESCAPE_AGGREGATE_STATE_OFFSET: i16 = 0xF4;
const QUEST_ESCAPE_COMPARISON_STATE_OFFSET: i16 = 0xD0;
const QUEST_ESCAPE_FAILURE_FLAGS_STATE_OFFSET: i16 = 0xF8;
const QUEST_ESCAPE_FORCED_FAILURE_STATE_OFFSET: i16 = 0x280;

/// `OPC8`가 현재 입력값에 적용하는 결과 규칙.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestEscapeOutcomeRule {
    ForcedFailure,
    GuaranteedSuccess,
    RandomZeroSucceeds { modulus: u16 },
    RandomNonzeroSucceeds { modulus: u16 },
}

impl QuestEscapeOutcomeRule {
    /// 난수 함수 반환값을 interpreter `+0x34`에 쓰는 `0/1`로 바꾼다.
    pub const fn outcome_value(self, random_result: u16) -> u32 {
        match self {
            Self::ForcedFailure => 0,
            Self::GuaranteedSuccess => 1,
            Self::RandomZeroSucceeds { .. } => {
                if random_result == 0 {
                    1
                } else {
                    0
                }
            }
            Self::RandomNonzeroSucceeds { .. } => {
                if random_result != 0 {
                    1
                } else {
                    0
                }
            }
        }
    }
}

/// 원본 unsigned 산술과 같은 경계로 `OPC8`의 결과 규칙을 고른다.
pub const fn classify_quest_escape_outcome(
    forced_failure_state: bool,
    aggregate_value: u32,
    comparison_value: u32,
) -> QuestEscapeOutcomeRule {
    if forced_failure_state {
        return QuestEscapeOutcomeRule::ForcedFailure;
    }
    if aggregate_value >= comparison_value {
        return QuestEscapeOutcomeRule::GuaranteedSuccess;
    }
    if aggregate_value.wrapping_mul(2) < comparison_value {
        return QuestEscapeOutcomeRule::RandomZeroSucceeds { modulus: 8 };
    }
    if aggregate_value.wrapping_mul(15) < comparison_value.wrapping_mul(10) {
        return QuestEscapeOutcomeRule::RandomZeroSucceeds { modulus: 2 };
    }
    QuestEscapeOutcomeRule::RandomNonzeroSucceeds { modulus: 3 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestEscapeOutcomeSummary {
    pub opcode: u8,
    pub handler_runtime: u32,
    pub record_bytes: usize,
    pub outcome_field_offset: i16,
    pub state_base_runtime: u32,
    pub comparison_value_runtime: u32,
    pub aggregate_value_runtime: u32,
    pub failure_flags_runtime: u32,
    pub forced_failure_state_runtime: u32,
    pub participant_state_runtime: u32,
    pub participant_table_runtime: u32,
    pub participant_count: u8,
    pub random_runtime: u32,
    pub random_moduli: [u16; 3],
    pub failure_flag_mask: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestEscapeOutcomeError(String);

impl fmt::Display for QuestEscapeOutcomeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestEscapeOutcomeError {}

fn error(message: impl Into<String>) -> QuestEscapeOutcomeError {
    QuestEscapeOutcomeError(message.into())
}

const QUEST_ESCAPE_OUTCOME_EVIDENCE: &[(u32, &str, K)] = &[
    (
        0x8007_6C34,
        "initialize participant index",
        K::Addu {
            rd: Reg::A3,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
    ),
    (
        0x8007_6C38,
        "participant state high",
        K::Lui {
            rt: Reg::V0,
            imm: 0x8010,
        },
    ),
    (
        0x8007_6C3C,
        "participant state low",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: -0x40B8,
        },
    ),
    (
        0x8007_6C40,
        "retain participant state",
        K::Addu {
            rd: Reg::T0,
            rs: Reg::V0,
            rt: Reg::ZERO,
        },
    ),
    (
        0x8007_6C44,
        "participant table high",
        K::Lui {
            rt: Reg::T1,
            imm: 0x801A,
        },
    ),
    (
        0x8007_6C4C,
        "escape state high",
        K::Lui {
            rt: Reg::V0,
            imm: 0x800E,
        },
    ),
    (
        0x8007_6C50,
        "escape state low",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: -0x4940,
        },
    ),
    (
        0x8007_6C48,
        "participant table low",
        K::Ori {
            rt: Reg::T1,
            rs: Reg::T1,
            imm: 0x1A6C,
        },
    ),
    (
        0x8007_6C60,
        "load participant base value",
        K::Lhu {
            rt: Reg::A1,
            base: Reg::T0,
            offset: QUEST_ESCAPE_PARTICIPANT_BASE_VALUE_OFFSET,
        },
    ),
    (
        0x8007_6C64,
        "retain escape state",
        K::Addu {
            rd: Reg::A2,
            rs: Reg::V0,
            rt: Reg::ZERO,
        },
    ),
    (
        0x8007_6C74,
        "store aggregate base value",
        K::Sw {
            rt: Reg::A1,
            base: Reg::A2,
            offset: QUEST_ESCAPE_AGGREGATE_STATE_OFFSET,
        },
    ),
    (
        0x8007_6C80,
        "load participant ID",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V1,
            offset: QUEST_ESCAPE_PARTICIPANT_ID_OFFSET,
        },
    ),
    (
        0x8007_6C8C,
        "scale participant ID",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ),
    (
        0x8007_6C90,
        "select participant table slot",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::T1,
        },
    ),
    (
        0x8007_6C94,
        "load participant record",
        K::Lw {
            rt: Reg::A0,
            base: Reg::V0,
            offset: 0,
        },
    ),
    (
        0x8007_6CC0,
        "load signed participant value",
        K::Lb {
            rt: Reg::V1,
            base: Reg::A0,
            offset: QUEST_ESCAPE_PARTICIPANT_VALUE_OFFSET,
        },
    ),
    (
        0x8007_6CCC,
        "add participant value",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::V1,
        },
    ),
    (
        0x8007_6CD0,
        "store adjusted aggregate value",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A2,
            offset: QUEST_ESCAPE_AGGREGATE_STATE_OFFSET,
        },
    ),
    (
        0x8007_6CE8,
        "advance participant index",
        K::Addiu {
            rt: Reg::A3,
            rs: Reg::A3,
            imm: 1,
        },
    ),
    (
        0x8007_6CEC,
        "bound participant index to byte",
        K::Andi {
            rt: Reg::V0,
            rs: Reg::A3,
            imm: 0x00FF,
        },
    ),
    (
        0x8007_6CF0,
        "bound participant loop",
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: QUEST_ESCAPE_PARTICIPANT_COUNT as i16,
        },
    ),
    (
        0x8007_6CF4,
        "repeat participant loop",
        K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: -31,
        },
    ),
    (
        0x8007_6CFC,
        "decision state high",
        K::Lui {
            rt: Reg::V0,
            imm: 0x800E,
        },
    ),
    (
        0x8007_6D00,
        "decision state low",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V0,
            imm: -0x4940,
        },
    ),
    (
        0x8007_6D04,
        "load forced failure state",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: QUEST_ESCAPE_FORCED_FAILURE_STATE_OFFSET,
        },
    ),
    (
        0x8007_6D0C,
        "branch on forced failure state",
        K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 36,
        },
    ),
    (
        0x8007_6D14,
        "load aggregate value",
        K::Lw {
            rt: Reg::A0,
            base: Reg::V1,
            offset: QUEST_ESCAPE_AGGREGATE_STATE_OFFSET,
        },
    ),
    (
        0x8007_6D18,
        "load comparison value",
        K::Lw {
            rt: Reg::A1,
            base: Reg::V1,
            offset: QUEST_ESCAPE_COMPARISON_STATE_OFFSET,
        },
    ),
    (
        0x8007_6D20,
        "compare aggregate below comparison",
        K::Sltu {
            rd: Reg::V0,
            rs: Reg::A0,
            rt: Reg::A1,
        },
    ),
    (
        0x8007_6D24,
        "guaranteed success branch",
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 32,
        },
    ),
    (
        0x8007_6D28,
        "double aggregate value",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::A0,
            sa: 1,
        },
    ),
    (
        0x8007_6D2C,
        "compare double aggregate",
        K::Sltu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::A1,
        },
    ),
    (
        0x8007_6D30,
        "select lowest ratio rule",
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 8,
        },
    ),
    (
        0x8007_6D34,
        "start aggregate times fifteen",
        K::Sll {
            rd: Reg::V1,
            rt: Reg::A0,
            sa: 4,
        },
    ),
    (
        0x8007_6D38,
        "call random for lowest ratio",
        K::Jal {
            target: QUEST_ESCAPE_RANDOM_RUNTIME,
        },
    ),
    (
        0x8007_6D3C,
        "lowest ratio modulus",
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::ZERO,
            imm: 8,
        },
    ),
    (
        0x8007_6D40,
        "mask lowest ratio random result",
        K::Andi {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0xFFFF,
        },
    ),
    (
        0x8007_6D44,
        "lowest ratio zero succeeds",
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 25,
        },
    ),
    (
        0x8007_6D4C,
        "leave lowest ratio failure path",
        K::J {
            target: 0x8007_6DB0,
        },
    ),
    (
        0x8007_6D50,
        "lowest ratio nonzero fails",
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::S0,
            offset: QUEST_ESCAPE_OUTCOME_FIELD_OFFSET,
        },
    ),
    (
        0x8007_6D54,
        "multiply aggregate by fifteen",
        K::Subu {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::A0,
        },
    ),
    (
        0x8007_6D58,
        "start comparison times ten",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::A1,
            sa: 2,
        },
    ),
    (
        0x8007_6D5C,
        "add comparison for times five",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::A1,
        },
    ),
    (
        0x8007_6D60,
        "multiply comparison by ten",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 1,
        },
    ),
    (
        0x8007_6D64,
        "compare middle ratio",
        K::Sltu {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::V0,
        },
    ),
    (
        0x8007_6D68,
        "select middle or highest ratio rule",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::ZERO,
            offset: 8,
        },
    ),
    (
        0x8007_6D70,
        "call random for middle ratio",
        K::Jal {
            target: QUEST_ESCAPE_RANDOM_RUNTIME,
        },
    ),
    (
        0x8007_6D74,
        "middle ratio modulus",
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::ZERO,
            imm: 2,
        },
    ),
    (
        0x8007_6D78,
        "mask middle ratio random result",
        K::Andi {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0xFFFF,
        },
    ),
    (
        0x8007_6D7C,
        "middle ratio zero succeeds",
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 11,
        },
    ),
    (
        0x8007_6D80,
        "select middle ratio success value",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 1,
        },
    ),
    (
        0x8007_6D84,
        "leave middle ratio failure path",
        K::J {
            target: 0x8007_6DB0,
        },
    ),
    (
        0x8007_6D88,
        "middle ratio nonzero fails",
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::S0,
            offset: QUEST_ESCAPE_OUTCOME_FIELD_OFFSET,
        },
    ),
    (
        0x8007_6D8C,
        "call random for highest ratio",
        K::Jal {
            target: QUEST_ESCAPE_RANDOM_RUNTIME,
        },
    ),
    (
        0x8007_6D90,
        "highest ratio modulus",
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::ZERO,
            imm: 3,
        },
    ),
    (
        0x8007_6D94,
        "mask highest ratio random result",
        K::Andi {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0xFFFF,
        },
    ),
    (
        0x8007_6D98,
        "highest ratio nonzero succeeds",
        K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 4,
        },
    ),
    (
        0x8007_6D9C,
        "select highest ratio success value",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 1,
        },
    ),
    (
        0x8007_6DA0,
        "leave forced or highest ratio failure path",
        K::J {
            target: 0x8007_6DB0,
        },
    ),
    (
        0x8007_6DA4,
        "forced or highest ratio zero fails",
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::S0,
            offset: QUEST_ESCAPE_OUTCOME_FIELD_OFFSET,
        },
    ),
    (
        0x8007_6DA8,
        "select success value",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 1,
        },
    ),
    (
        0x8007_6DAC,
        "store selected outcome",
        K::Sw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: QUEST_ESCAPE_OUTCOME_FIELD_OFFSET,
        },
    ),
    (
        0x8007_6DB0,
        "post-decision state high",
        K::Lui {
            rt: Reg::V0,
            imm: 0x800E,
        },
    ),
    (
        0x8007_6DB4,
        "post-decision state low",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V0,
            imm: -0x4940,
        },
    ),
    (
        0x8007_6DB8,
        "clear forced failure state",
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::V1,
            offset: QUEST_ESCAPE_FORCED_FAILURE_STATE_OFFSET,
        },
    ),
    (
        0x8007_6DBC,
        "reload selected outcome",
        K::Lw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: QUEST_ESCAPE_OUTCOME_FIELD_OFFSET,
        },
    ),
    (
        0x8007_6DC4,
        "skip failure flag on success",
        K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 5,
        },
    ),
    (
        0x8007_6DCC,
        "load failure flags",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: QUEST_ESCAPE_FAILURE_FLAGS_STATE_OFFSET,
        },
    ),
    (
        0x8007_6DD4,
        "set escape failure flag",
        K::Ori {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: QUEST_ESCAPE_FAILURE_FLAG_MASK,
        },
    ),
    (
        0x8007_6DD8,
        "store failure flags",
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: QUEST_ESCAPE_FAILURE_FLAGS_STATE_OFFSET,
        },
    ),
    (
        0x8007_6DE4,
        "advance OPC8 record",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: QUEST_ESCAPE_OUTCOME_RECORD_BYTES as i16,
        },
    ),
    (
        0x8007_6DE8,
        "store advanced stream",
        K::Sw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: 0x1C,
        },
    ),
];

/// 지원 `QUEST.EXE`에서 `OPC8` 입력 집계·결과 선택·후속 상태를 검증한다.
pub fn verify_box_quest_escape_outcome(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestEscapeOutcomeSummary, QuestEscapeOutcomeError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST escape outcome 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len,
        )));
    }
    if quest_seq_opcode_handler(QUEST_ESCAPE_OUTCOME_OPCODE)
        != Some(QUEST_ESCAPE_OUTCOME_HANDLER_RUNTIME)
    {
        return Err(error("QUEST OPC8 dispatch handler 불일치"));
    }
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error("QUEST.EXE text 길이가 4바이트 정렬이 아님"))?;
    for &(address, role, expected) in QUEST_ESCAPE_OUTCOME_EVIDENCE {
        require(&instructions, address, role, expected)?;
    }

    Ok(QuestEscapeOutcomeSummary {
        opcode: QUEST_ESCAPE_OUTCOME_OPCODE,
        handler_runtime: QUEST_ESCAPE_OUTCOME_HANDLER_RUNTIME,
        record_bytes: QUEST_ESCAPE_OUTCOME_RECORD_BYTES,
        outcome_field_offset: QUEST_ESCAPE_OUTCOME_FIELD_OFFSET,
        state_base_runtime: QUEST_ESCAPE_STATE_BASE_RUNTIME,
        comparison_value_runtime: QUEST_ESCAPE_COMPARISON_VALUE_RUNTIME,
        aggregate_value_runtime: QUEST_ESCAPE_AGGREGATE_VALUE_RUNTIME,
        failure_flags_runtime: QUEST_ESCAPE_FAILURE_FLAGS_RUNTIME,
        forced_failure_state_runtime: QUEST_ESCAPE_FORCED_FAILURE_STATE_RUNTIME,
        participant_state_runtime: QUEST_ESCAPE_PARTICIPANT_STATE_RUNTIME,
        participant_table_runtime: QUEST_ESCAPE_PARTICIPANT_TABLE_RUNTIME,
        participant_count: QUEST_ESCAPE_PARTICIPANT_COUNT,
        random_runtime: QUEST_ESCAPE_RANDOM_RUNTIME,
        random_moduli: [8, 2, 3],
        failure_flag_mask: QUEST_ESCAPE_FAILURE_FLAG_MASK,
    })
}

fn require(
    instructions: &[Instruction],
    address: u32,
    role: &str,
    expected: K,
) -> Result<(), QuestEscapeOutcomeError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| {
            error(format!(
                "QUEST escape outcome {role}가 text 밖임: 0x{address:08X}"
            ))
        })?;
    if actual.kind != expected {
        return Err(error(format!(
            "QUEST escape outcome {role} 명령 불일치: 0x{address:08X}, expected={expected:?}, actual={:?}",
            actual.kind,
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_escape_outcome_tests.rs"]
mod tests;
