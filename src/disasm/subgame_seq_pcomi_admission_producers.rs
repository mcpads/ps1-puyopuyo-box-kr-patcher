//! PCOMI custom byte admission callback을 만드는 command 경로.
//!
//! `PCOMI.EXE`의 immutable main opcode table에서 `0x62`는 처음에는 단순
//! advance다. PCOMI 전용 초기화가 opcode `0x60..=0x7F`의 32개 handler를
//! 설치하면 `0x62`는 record `+2`의 callback index를 읽는 dispatcher가
//! 된다. 이 모듈은 그 설치 경로와 callback table의 `0x1C/0x1D` slot을
//! 검산하고, `MCRD1.SEQ`와 `PCOMI1.SEQ`에서 `0x1D` self-loop producer를
//! 전수 분류한다.
//!
//! 여기서 확인하는 것은 callback producer의 정체와 유한 분모다. 두 번째
//! producer가 실제로 source cell 0을 공급하는 입력·상태와 player-visible
//! UX 의미는 runtime 판정으로 남긴다. 타이틀 로컬 command 의미를 공용 ISA
//! 모델에 추가하지 않는다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;
use super::subgame_seq_pcomi_load::{
    PCOMI1_LOAD_BASE, PCOMI1_SEQUENCE_BYTES, PCOMI1_SEQUENCE_SHA256,
};

pub const PCOMI_MAIN_OPCODE_TABLE_RUNTIME: u32 = 0x8005_0258;
pub const PCOMI_LOCAL_CALLBACK_TABLE_RUNTIME: u32 = 0x8005_127C;
pub const PCOMI_LOCAL_HANDLER_SOURCE_RUNTIME: u32 = 0x8005_137C;
pub const PCOMI_LOCAL_HANDLER_INSTALLER_RUNTIME: u32 = 0x8002_94FC;
pub const PCOMI_LOCAL_HANDLER_REGISTER_RUNTIME: u32 = 0x8001_80D4;
pub const PCOMI_LOCAL_HANDLER_FIRST_OPCODE: u8 = 0x60;
pub const PCOMI_LOCAL_HANDLER_COUNT: usize = 32;
pub const PCOMI_CALLBACK_DISPATCH_OPCODE: u8 = 0x62;
pub const PCOMI_CALLBACK_DISPATCH_RUNTIME: u32 = 0x8002_97E0;
pub const PCOMI_UI_CALLBACK_INDEX: u16 = 0x001C;
pub const PCOMI_ADMISSION_CALLBACK_INDEX: u16 = 0x001D;
pub const PCOMI_UI_CALLBACK_RUNTIME: u32 = 0x8002_8D20;
pub const PCOMI_ADMISSION_CALLBACK_RUNTIME: u32 = 0x8002_1E8C;
pub const PCOMI1_FIRST_ADMISSION_OFFSET: usize = 0x05B8;
pub const PCOMI1_SECOND_ADMISSION_OFFSET: usize = 0x1620;
pub const PCOMI1_SECOND_UI_CALLBACK_OFFSET: usize = 0x160C;
pub const PCOMI1_REJECTED_HALFWORD_OFFSET: usize = 0x1928;

const PCOMI_DEFAULT_ADVANCE_RUNTIME: u32 = 0x8001_8108;
const PCOMI_WORD_COPY_RUNTIME: u32 = 0x8002_E4F8;
const MCRD1_LOAD_BASE: u32 = 0x801B_0270;
const MCRD1_SEQUENCE_BYTES: usize = 0x19E8;
const MCRD1_SEQUENCE_SHA256: [u8; 32] = [
    0x25, 0x68, 0x69, 0xD5, 0x67, 0xC0, 0x17, 0xA2, 0xC8, 0xD5, 0x4C, 0xC6, 0x15, 0xCE, 0x64, 0x18,
    0x9F, 0x7A, 0x6A, 0xC2, 0xF8, 0x30, 0x60, 0xC3, 0xAF, 0x6E, 0xF9, 0x94, 0x0F, 0x48, 0x51, 0xB9,
];

const FIRST_ADMISSION_PRELUDE_OFFSET: usize = 0x05A4;
const FIRST_ADMISSION_PRELUDE: [u8; 36] = [
    0x18, 0x96, 0x6A, 0x75, 0x10, 0x86, 0x6A, 0x00, 0x07, 0x00, 0x00, 0x00, 0x10, 0x81, 0x6A, 0x00,
    0x64, 0x00, 0x00, 0x00, 0x62, 0x00, 0x1D, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
    0x28, 0x68, 0x1A, 0x80,
];
const SECOND_ADMISSION_PRELUDE_OFFSET: usize = 0x15F0;
const SECOND_ADMISSION_PRELUDE: [u8; 64] = [
    0x10, 0x80, 0x6C, 0x00, 0x04, 0x00, 0x00, 0x00, 0x10, 0x80, 0x6E, 0x00, 0x80, 0x0D, 0x00, 0x00,
    0x10, 0x80, 0x6A, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0F, 0x80, 0x73, 0x0A, 0x62, 0x00, 0x1C, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x7C, 0x78, 0x1A, 0x80, 0x18, 0x96, 0x6A, 0x73,
    0x62, 0x00, 0x1D, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x90, 0x78, 0x1A, 0x80,
];
const REJECTED_HALFWORD_BYTES: [u8; 16] = [
    0x23, 0x04, 0x1D, 0x00, 0x23, 0x04, 0x1E, 0x00, 0x23, 0x04, 0x1F, 0x00, 0x23, 0x04, 0x20, 0x00,
];

fn expected_local_handler_runtime(index: usize) -> u32 {
    match index {
        1 => 0x8002_9698,
        2 => PCOMI_CALLBACK_DISPATCH_RUNTIME,
        3 => 0x8002_96A0,
        _ => 0x8002_9688,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelfRepeatingCallback {
    offset: usize,
    callback_index: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcomiAdmissionProducer {
    pub sequence_offset: usize,
    pub runtime: u32,
    pub callback_index: u16,
    pub preceding_ui_callback_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcomiAdmissionProducerAnalysis {
    pub installed_opcode: u8,
    pub installed_handler_runtime: u32,
    pub callback_table_runtime: u32,
    pub ui_callback_runtime: u32,
    pub admission_callback_runtime: u32,
    pub mcrd1_admission_producers: usize,
    pub pcomi1_aligned_halfword_candidates: usize,
    pub rejected_halfword_offset: usize,
    pub producers: Vec<PcomiAdmissionProducer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcomiAdmissionProducerError(String);

impl fmt::Display for PcomiAdmissionProducerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for PcomiAdmissionProducerError {}

fn error(message: impl Into<String>) -> PcomiAdmissionProducerError {
    PcomiAdmissionProducerError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, PcomiAdmissionProducerError> {
    instructions
        .binary_search_by_key(&address, |instruction| instruction.addr)
        .ok()
        .map(|index| &instructions[index])
        .ok_or_else(|| {
            error(format!(
                "PCOMI callback producer 명령 주소 없음: 0x{address:08X}"
            ))
        })
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: K,
) -> Result<(), PcomiAdmissionProducerError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "PCOMI callback producer 명령 불일치: address=0x{address:08X}, \
             expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn read_text_word(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
) -> Result<u32, PcomiAdmissionProducerError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "PCOMI callback producer EXE 주소 범위 이탈: 0x{runtime:08X}"
        ))
    })?;
    let bytes = text
        .get(offset..offset + 4)
        .ok_or_else(|| error("PCOMI callback producer EXE word 범위 이탈"))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

fn verify_handler_source(map: &ExeMap, text: &[u8]) -> Result<(), PcomiAdmissionProducerError> {
    let source_slot =
        PCOMI_MAIN_OPCODE_TABLE_RUNTIME + u32::from(PCOMI_CALLBACK_DISPATCH_OPCODE) * 4;
    if read_text_word(map, text, source_slot)? != PCOMI_DEFAULT_ADVANCE_RUNTIME {
        return Err(error("PCOMI immutable opcode 0x62 초기 handler 불일치"));
    }
    for index in 0..PCOMI_LOCAL_HANDLER_COUNT {
        let expected = expected_local_handler_runtime(index);
        let runtime =
            PCOMI_LOCAL_HANDLER_SOURCE_RUNTIME + u32::try_from(index).expect("32개 handler") * 4;
        if read_text_word(map, text, runtime)? != expected {
            return Err(error(format!(
                "PCOMI local handler source 불일치: index={index}"
            )));
        }
    }
    for (index, expected) in [
        (PCOMI_UI_CALLBACK_INDEX, PCOMI_UI_CALLBACK_RUNTIME),
        (
            PCOMI_ADMISSION_CALLBACK_INDEX,
            PCOMI_ADMISSION_CALLBACK_RUNTIME,
        ),
    ] {
        let runtime = PCOMI_LOCAL_CALLBACK_TABLE_RUNTIME + u32::from(index) * 4;
        if read_text_word(map, text, runtime)? != expected {
            return Err(error(format!(
                "PCOMI local callback table 불일치: index=0x{index:04X}"
            )));
        }
    }
    Ok(())
}

fn verify_handler_installation(
    instructions: &[Instruction],
) -> Result<(), PcomiAdmissionProducerError> {
    let checks = [
        (
            PCOMI_LOCAL_HANDLER_INSTALLER_RUNTIME,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -0x18,
            },
        ),
        (
            PCOMI_LOCAL_HANDLER_INSTALLER_RUNTIME + 4,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8005,
            },
        ),
        (
            PCOMI_LOCAL_HANDLER_INSTALLER_RUNTIME + 8,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: PCOMI_LOCAL_HANDLER_SOURCE_RUNTIME as u16 as i16,
            },
        ),
        (
            PCOMI_LOCAL_HANDLER_INSTALLER_RUNTIME + 0x0C,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: PCOMI_LOCAL_HANDLER_FIRST_OPCODE as i16,
            },
        ),
        (
            PCOMI_LOCAL_HANDLER_INSTALLER_RUNTIME + 0x14,
            K::Jal {
                target: PCOMI_LOCAL_HANDLER_REGISTER_RUNTIME,
            },
        ),
        (
            PCOMI_LOCAL_HANDLER_INSTALLER_RUNTIME + 0x18,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: PCOMI_LOCAL_HANDLER_COUNT as i16,
            },
        ),
        (
            PCOMI_LOCAL_HANDLER_REGISTER_RUNTIME + 4,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::A0,
                rt: Reg::ZERO,
            },
        ),
        (
            PCOMI_LOCAL_HANDLER_REGISTER_RUNTIME + 8,
            K::Sll {
                rd: Reg::A1,
                rt: Reg::A1,
                sa: 2,
            },
        ),
        (
            PCOMI_LOCAL_HANDLER_REGISTER_RUNTIME + 0x0C,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8005,
            },
        ),
        (
            PCOMI_LOCAL_HANDLER_REGISTER_RUNTIME + 0x10,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: PCOMI_MAIN_OPCODE_TABLE_RUNTIME as u16 as i16,
            },
        ),
        (
            PCOMI_LOCAL_HANDLER_REGISTER_RUNTIME + 0x14,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A1,
                rt: Reg::A0,
            },
        ),
        (
            PCOMI_LOCAL_HANDLER_REGISTER_RUNTIME + 0x1C,
            K::Jal {
                target: PCOMI_WORD_COPY_RUNTIME,
            },
        ),
        (
            PCOMI_LOCAL_HANDLER_REGISTER_RUNTIME + 0x20,
            K::Addu {
                rd: Reg::A1,
                rs: Reg::V0,
                rt: Reg::ZERO,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, address, expected)?;
    }
    Ok(())
}

fn verify_callback_dispatcher(
    instructions: &[Instruction],
) -> Result<(), PcomiAdmissionProducerError> {
    let checks = [
        (
            PCOMI_CALLBACK_DISPATCH_RUNTIME + 0x10,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
        (
            PCOMI_CALLBACK_DISPATCH_RUNTIME + 0x14,
            K::Lui {
                rt: Reg::V1,
                imm: 0x8005,
            },
        ),
        (
            PCOMI_CALLBACK_DISPATCH_RUNTIME + 0x18,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 2,
            },
        ),
        (
            PCOMI_CALLBACK_DISPATCH_RUNTIME + 0x1C,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: PCOMI_LOCAL_CALLBACK_TABLE_RUNTIME as u16 as i16,
            },
        ),
        (
            PCOMI_CALLBACK_DISPATCH_RUNTIME + 0x20,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            PCOMI_CALLBACK_DISPATCH_RUNTIME + 0x24,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            PCOMI_CALLBACK_DISPATCH_RUNTIME + 0x28,
            K::Lw {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            PCOMI_CALLBACK_DISPATCH_RUNTIME + 0x30,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::A1,
            },
        ),
        (
            PCOMI_CALLBACK_DISPATCH_RUNTIME + 0x38,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
        (
            PCOMI_CALLBACK_DISPATCH_RUNTIME + 0x40,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 4,
            },
        ),
        (
            PCOMI_CALLBACK_DISPATCH_RUNTIME + 0x44,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, address, expected)?;
    }
    Ok(())
}

fn self_repeating_callbacks(raw: &[u8], load_base: u32) -> Vec<SelfRepeatingCallback> {
    let mut records = Vec::new();
    for offset in 0..raw.len().saturating_sub(15) {
        if raw[offset] != PCOMI_CALLBACK_DISPATCH_OPCODE
            || raw.get(offset + 4..offset + 8) != Some([1, 0, 0, 0].as_slice())
            || raw.get(offset + 8..offset + 12) != Some([3, 0, 0, 0].as_slice())
        {
            continue;
        }
        let Some(target) = raw.get(offset + 12..offset + 16) else {
            continue;
        };
        let Some(runtime) = load_base.checked_add(u32::try_from(offset).unwrap_or(u32::MAX)) else {
            continue;
        };
        if u32::from_le_bytes(target.try_into().expect("4바이트 pointer")) != runtime {
            continue;
        }
        records.push(SelfRepeatingCallback {
            offset,
            callback_index: u16::from_le_bytes([raw[offset + 2], raw[offset + 3]]),
        });
    }
    records
}

fn verify_sequence_identity(
    name: &str,
    raw: &[u8],
    expected_len: usize,
    expected_sha256: [u8; 32],
) -> Result<(), PcomiAdmissionProducerError> {
    if raw.len() != expected_len {
        return Err(error(format!(
            "{name} 길이 불일치: expected={expected_len}, actual={}",
            raw.len()
        )));
    }
    let actual: [u8; 32] = Sha256::digest(raw).into();
    if actual != expected_sha256 {
        return Err(error(format!("{name} SHA-256 불일치")));
    }
    Ok(())
}

fn require_bytes(
    raw: &[u8],
    offset: usize,
    expected: &[u8],
    label: &str,
) -> Result<(), PcomiAdmissionProducerError> {
    if raw.get(offset..offset + expected.len()) != Some(expected) {
        return Err(error(format!("PCOMI1 {label} bytes 불일치: +0x{offset:X}")));
    }
    Ok(())
}

fn verify_sequence_producers(
    mcrd1: &[u8],
    pcomi1: &[u8],
) -> Result<Vec<PcomiAdmissionProducer>, PcomiAdmissionProducerError> {
    verify_sequence_identity(
        "MCRD1.SEQ",
        mcrd1,
        MCRD1_SEQUENCE_BYTES,
        MCRD1_SEQUENCE_SHA256,
    )?;
    verify_sequence_identity(
        "PCOMI1.SEQ",
        pcomi1,
        PCOMI1_SEQUENCE_BYTES,
        PCOMI1_SEQUENCE_SHA256,
    )?;

    let mcrd_admission = self_repeating_callbacks(mcrd1, MCRD1_LOAD_BASE)
        .into_iter()
        .filter(|record| record.callback_index == PCOMI_ADMISSION_CALLBACK_INDEX)
        .collect::<Vec<_>>();
    if !mcrd_admission.is_empty() {
        return Err(error(format!(
            "MCRD1 admission callback producer가 0개가 아님: {}",
            mcrd_admission.len()
        )));
    }

    let aligned_halfword_candidates = (0..pcomi1.len().saturating_sub(3))
        .step_by(2)
        .filter(|&offset| {
            u16::from_le_bytes([pcomi1[offset + 2], pcomi1[offset + 3]])
                == PCOMI_ADMISSION_CALLBACK_INDEX
        })
        .collect::<Vec<_>>();
    if aligned_halfword_candidates
        != [
            PCOMI1_FIRST_ADMISSION_OFFSET,
            PCOMI1_SECOND_ADMISSION_OFFSET,
            PCOMI1_REJECTED_HALFWORD_OFFSET,
        ]
    {
        return Err(error(format!(
            "PCOMI1 aligned callback-index 후보 불일치: {aligned_halfword_candidates:X?}"
        )));
    }

    let admission_records = self_repeating_callbacks(pcomi1, PCOMI1_LOAD_BASE)
        .into_iter()
        .filter(|record| record.callback_index == PCOMI_ADMISSION_CALLBACK_INDEX)
        .collect::<Vec<_>>();
    let expected_offsets = [
        PCOMI1_FIRST_ADMISSION_OFFSET,
        PCOMI1_SECOND_ADMISSION_OFFSET,
    ];
    if admission_records
        .iter()
        .map(|record| record.offset)
        .collect::<Vec<_>>()
        != expected_offsets
    {
        return Err(error(format!(
            "PCOMI1 admission callback producer 불일치: {admission_records:X?}"
        )));
    }

    require_bytes(
        pcomi1,
        FIRST_ADMISSION_PRELUDE_OFFSET,
        &FIRST_ADMISSION_PRELUDE,
        "첫 admission prelude",
    )?;
    require_bytes(
        pcomi1,
        SECOND_ADMISSION_PRELUDE_OFFSET,
        &SECOND_ADMISSION_PRELUDE,
        "둘째 admission prelude",
    )?;
    require_bytes(
        pcomi1,
        PCOMI1_REJECTED_HALFWORD_OFFSET,
        &REJECTED_HALFWORD_BYTES,
        "기각 halfword 후보",
    )?;

    let callbacks = self_repeating_callbacks(pcomi1, PCOMI1_LOAD_BASE);
    if !callbacks.iter().any(|record| {
        record.offset == PCOMI1_SECOND_UI_CALLBACK_OFFSET
            && record.callback_index == PCOMI_UI_CALLBACK_INDEX
    }) {
        return Err(error(
            "PCOMI1 둘째 admission 앞 UI callback producer가 없음",
        ));
    }

    Ok(vec![
        PcomiAdmissionProducer {
            sequence_offset: PCOMI1_FIRST_ADMISSION_OFFSET,
            runtime: PCOMI1_LOAD_BASE + PCOMI1_FIRST_ADMISSION_OFFSET as u32,
            callback_index: PCOMI_ADMISSION_CALLBACK_INDEX,
            preceding_ui_callback_offset: None,
        },
        PcomiAdmissionProducer {
            sequence_offset: PCOMI1_SECOND_ADMISSION_OFFSET,
            runtime: PCOMI1_LOAD_BASE + PCOMI1_SECOND_ADMISSION_OFFSET as u32,
            callback_index: PCOMI_ADMISSION_CALLBACK_INDEX,
            preceding_ui_callback_offset: Some(PCOMI1_SECOND_UI_CALLBACK_OFFSET),
        },
    ])
}

/// 지원 원본의 PCOMI callback handler 설치와 admission producer 분모를 검산한다.
pub fn verify_box_pcomi_custom_byte_admission_producers(
    map: &ExeMap,
    text: &[u8],
    mcrd1: &[u8],
    pcomi1: &[u8],
) -> Result<PcomiAdmissionProducerAnalysis, PcomiAdmissionProducerError> {
    if map.name != "PCOMI.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "PCOMI callback producer EXE 사상 불일치: name={}, expected_len={}, actual_len={}",
            map.name,
            map.text_len,
            text.len()
        )));
    }
    verify_handler_source(map, text)?;
    let instructions =
        linear::sweep(map, text).ok_or_else(|| error("PCOMI.EXE text 정렬 불일치"))?;
    verify_handler_installation(&instructions)?;
    verify_callback_dispatcher(&instructions)?;
    let producers = verify_sequence_producers(mcrd1, pcomi1)?;

    Ok(PcomiAdmissionProducerAnalysis {
        installed_opcode: PCOMI_CALLBACK_DISPATCH_OPCODE,
        installed_handler_runtime: PCOMI_CALLBACK_DISPATCH_RUNTIME,
        callback_table_runtime: PCOMI_LOCAL_CALLBACK_TABLE_RUNTIME,
        ui_callback_runtime: PCOMI_UI_CALLBACK_RUNTIME,
        admission_callback_runtime: PCOMI_ADMISSION_CALLBACK_RUNTIME,
        mcrd1_admission_producers: 0,
        pcomi1_aligned_halfword_candidates: 3,
        rejected_halfword_offset: PCOMI1_REJECTED_HALFWORD_OFFSET,
        producers,
    })
}

#[cfg(test)]
#[path = "subgame_seq_pcomi_admission_producers_tests.rs"]
mod tests;
