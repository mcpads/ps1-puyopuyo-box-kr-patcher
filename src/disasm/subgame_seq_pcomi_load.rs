//! PCOMI의 master command에서 `PCOMI1.SEQ` overlay 진입까지의 정적 경로.
//!
//! `PCOMI1.SEQ`의 custom byte slot과 글리프 code 경로는 이미 각각 확인했지만,
//! 그 사이의 소비 경로를 찾으려면 먼저 이 overlay가 어떤 record로 적재되고
//! 어디서 실행을 시작하는지 고정해야 한다. 이 모듈은 EXE 안의 type/resource
//! 선택, CNX 완료 후 descriptor 목적지 기록, master command의 call record,
//! overlay 선두 jump를 하나의 지원 원본 경계로 검산한다.
//!
//! 여기서 확인하는 것은 overlay 진입까지다. custom byte slot의 reader와
//! 실제 BTX cell atlas는 아직 이 경계 밖이며, 따라서 slot 쓰기를 승인하지
//! 않는다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;

pub const PCOMI1_SEQUENCE_BYTES: usize = 0x72C8;
pub const PCOMI1_SEQUENCE_SHA256: [u8; 32] = [
    0xFB, 0x55, 0x56, 0xB9, 0x8A, 0x57, 0x61, 0x89, 0x28, 0x23, 0x2B, 0xF4, 0xCD, 0xE8, 0x3C, 0xFD,
    0x03, 0x43, 0x2E, 0x61, 0x00, 0x6F, 0xB8, 0x0B, 0xD4, 0x0F, 0x3C, 0xE8, 0x74, 0x3F, 0x9F, 0x8F,
];
pub const PCOMI1_LOAD_BASE: u32 = 0x801A_6270;
pub const PCOMI1_INITIALIZED_BYTES: u32 = 0x729C;
pub const PCOMI1_DECOMPRESSED_BYTES: u32 = 0x72C8;
pub const PCOMI1_LOAD_RECORD_RUNTIME: u32 = 0x8002_F320;
pub const PCOMI1_DESCRIPTOR_RUNTIME: u32 = 0x8002_F788;
pub const PCOMI1_DESCRIPTOR_DESTINATION_RUNTIME: u32 = 0x8002_F798;
pub const PCOMI1_MASTER_CONTINUATION_RUNTIME: u32 = 0x8002_F330;
pub const PCOMI1_MASTER_CALL_RUNTIME: u32 = 0x8002_F33C;
pub const PCOMI1_OVERLAY_ENTRY_OFFSET: usize = 0x48;
pub const PCOMI1_OVERLAY_ENTRY_RUNTIME: u32 = PCOMI1_LOAD_BASE + PCOMI1_OVERLAY_ENTRY_OFFSET as u32;
pub const PCOMI1_OVERLAY_START_OFFSET: usize = 0x58;
pub const PCOMI1_OVERLAY_START_RUNTIME: u32 = PCOMI1_LOAD_BASE + PCOMI1_OVERLAY_START_OFFSET as u32;
pub const PCOMI1_RESOURCE_TYPE: u8 = 1;
pub const PCOMI1_RESOURCE_ID: u16 = 1;
pub const PCOMI1_LOAD_FLAGS: u8 = 0x17;
pub const PCOMI1_COMPLETION_CALLBACK_RUNTIME: u32 = 0x8001_2DFC;

const MASTER_ROOT_FIELD_RUNTIME: u32 = 0x8002_F2A4;
const MASTER_ROOT_RUNTIME: u32 = 0x8002_F2AC;
const MASTER_INITIALIZER_RUNTIME: u32 = 0x8001_DFA0;
const MASTER_OBJECT_CONSTRUCTOR_RUNTIME: u32 = 0x8001_7FD0;
const MASTER_DISPATCHER_RUNTIME: u32 = 0x8001_8068;
const MASTER_DISPATCH_TABLE_RUNTIME: u32 = 0x8005_0258;
const NESTED_COMMAND_HANDLER_RUNTIME: u32 = 0x8001_8A6C;
const NESTED_DISPATCHER_RUNTIME: u32 = 0x8001_42D8;
const NESTED_DISPATCH_TABLE_RUNTIME: u32 = 0x8005_01D8;
const RESOURCE_SELECTOR_RUNTIME: u32 = 0x8001_2874;
const CONTINUATION_HANDLER_RUNTIME: u32 = 0x8001_61B8;
const DEFAULT_ADVANCE_HANDLER_RUNTIME: u32 = 0x8001_8108;
const JUMP_HANDLER_RUNTIME: u32 = 0x8001_81D0;
const CALL_HANDLER_RUNTIME: u32 = 0x8001_81E4;
const PCOMI1_NAME_RUNTIME: u32 = 0x8002_F800;
const PCOMI1_STORED_BYTES: u32 = 0x4491;

const LOAD_RECORD: [u8; 16] = [
    0x28,
    0x10,
    PCOMI1_LOAD_FLAGS,
    PCOMI1_RESOURCE_TYPE,
    PCOMI1_RESOURCE_ID as u8,
    (PCOMI1_RESOURCE_ID >> 8) as u8,
    0,
    0,
    0x70,
    0x62,
    0x1A,
    0x80,
    0,
    0,
    0,
    0,
];
const MASTER_CONTINUATION: [u8; 12] = [0x11, 0x40, 0x70, 0x00, 0, 0, 0, 0, 0x62, 0, 0x2D, 0];
const MASTER_CALL: [u8; 8] = [0x04, 0, 0, 0, 0xB8, 0x62, 0x1A, 0x80];
const OVERLAY_ENTRY: [u8; 8] = [0x03, 0, 0, 0, 0xC8, 0x62, 0x1A, 0x80];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcomiSeqLoadRoute {
    pub load_record_runtime: u32,
    pub resource_type: u8,
    pub resource_id: u16,
    pub flags: u8,
    pub descriptor_runtime: u32,
    pub destination_runtime: u32,
    pub completion_callback_runtime: u32,
    pub master_call_runtime: u32,
    pub overlay_entry_runtime: u32,
    pub overlay_start_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcomiSeqLoadError(String);

impl fmt::Display for PcomiSeqLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for PcomiSeqLoadError {}

fn error(message: impl Into<String>) -> PcomiSeqLoadError {
    PcomiSeqLoadError(message.into())
}

fn read_word(raw: &[u8], offset: usize) -> Result<u32, PcomiSeqLoadError> {
    raw.get(offset..offset + 4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("4바이트 word")))
        .ok_or_else(|| error(format!("PCOMI load word 범위 이탈: +0x{offset:X}")))
}

fn text_bytes<'a>(
    map: &ExeMap,
    text: &'a [u8],
    runtime: u32,
    len: usize,
) -> Result<&'a [u8], PcomiSeqLoadError> {
    let offset = map
        .text_offset(runtime)
        .ok_or_else(|| error(format!("PCOMI load EXE 주소 범위 이탈: 0x{runtime:08X}")))?;
    text.get(offset..offset + len).ok_or_else(|| {
        error(format!(
            "PCOMI load EXE byte 범위 이탈: 0x{runtime:08X}+{len}"
        ))
    })
}

fn read_text_word(map: &ExeMap, text: &[u8], runtime: u32) -> Result<u32, PcomiSeqLoadError> {
    Ok(u32::from_le_bytes(
        text_bytes(map, text, runtime, 4)?
            .try_into()
            .expect("4바이트 word"),
    ))
}

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, PcomiSeqLoadError> {
    instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| error(format!("PCOMI load 명령 주소 없음: 0x{address:08X}")))
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: K,
) -> Result<(), PcomiSeqLoadError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "PCOMI load 명령 불일치: address=0x{address:08X}, \
             expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn verify_master_entry(instructions: &[Instruction]) -> Result<(), PcomiSeqLoadError> {
    let checks = [
        (
            MASTER_INITIALIZER_RUNTIME + 0x38,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8003,
            },
        ),
        (
            MASTER_INITIALIZER_RUNTIME + 0x3C,
            K::Lw {
                rt: Reg::A0,
                base: Reg::A0,
                offset: -0x0D5C,
            },
        ),
        (
            MASTER_INITIALIZER_RUNTIME + 0x40,
            K::Jal {
                target: MASTER_OBJECT_CONSTRUCTOR_RUNTIME,
            },
        ),
        (
            MASTER_OBJECT_CONSTRUCTOR_RUNTIME + 8,
            K::Addu {
                rd: Reg::S0,
                rs: Reg::A0,
                rt: Reg::ZERO,
            },
        ),
        (
            MASTER_OBJECT_CONSTRUCTOR_RUNTIME + 0x50,
            K::Sw {
                rt: Reg::S0,
                base: Reg::V0,
                offset: 0x1C,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, address, expected)?;
    }
    Ok(())
}

fn verify_dispatcher(instructions: &[Instruction]) -> Result<(), PcomiSeqLoadError> {
    let checks = [
        (
            MASTER_DISPATCHER_RUNTIME + 0x0C,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8005,
            },
        ),
        (
            MASTER_DISPATCHER_RUNTIME + 0x14,
            K::Addiu {
                rt: Reg::S1,
                rs: Reg::V0,
                imm: 0x0258,
            },
        ),
        (
            MASTER_DISPATCHER_RUNTIME + 0x20,
            K::Lw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
        (
            MASTER_DISPATCHER_RUNTIME + 0x28,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            MASTER_DISPATCHER_RUNTIME + 0x30,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            MASTER_DISPATCHER_RUNTIME + 0x34,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::S1,
            },
        ),
        (
            MASTER_DISPATCHER_RUNTIME + 0x38,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            MASTER_DISPATCHER_RUNTIME + 0x40,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V1,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, address, expected)?;
    }
    Ok(())
}

fn verify_load_dispatch(instructions: &[Instruction]) -> Result<(), PcomiSeqLoadError> {
    let checks = [
        (
            NESTED_COMMAND_HANDLER_RUNTIME + 8,
            K::Jal {
                target: NESTED_DISPATCHER_RUNTIME,
            },
        ),
        (
            NESTED_DISPATCHER_RUNTIME + 8,
            K::Lw {
                rt: Reg::A1,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            NESTED_DISPATCHER_RUNTIME + 0x0C,
            K::Lui {
                rt: Reg::V1,
                imm: 0x8005,
            },
        ),
        (
            NESTED_DISPATCHER_RUNTIME + 0x10,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 1,
            },
        ),
        (
            NESTED_DISPATCHER_RUNTIME + 0x14,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x01D8,
            },
        ),
        (
            NESTED_DISPATCHER_RUNTIME + 0x18,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            NESTED_DISPATCHER_RUNTIME + 0x1C,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            NESTED_DISPATCHER_RUNTIME + 0x20,
            K::Lw {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            NESTED_DISPATCHER_RUNTIME + 0x28,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::A1,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, address, expected)?;
    }
    Ok(())
}

fn verify_selector_and_completion(instructions: &[Instruction]) -> Result<(), PcomiSeqLoadError> {
    let checks = [
        (
            0x8001_2890,
            K::Lw {
                rt: Reg::A1,
                base: Reg::S3,
                offset: 0x1C,
            },
        ),
        (
            0x8001_2898,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 4,
            },
        ),
        (
            0x8001_28B0,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A1,
                offset: 3,
            },
        ),
        (
            0x8001_28B8,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A1,
                sa: 3,
            },
        ),
        (
            0x8001_28BC,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A1,
            },
        ),
        (
            0x8001_28F0,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8001_28F4,
            K::Addu {
                rd: Reg::S2,
                rs: Reg::A0,
                rt: Reg::V0,
            },
        ),
        (
            0x8001_2908,
            K::Lbu {
                rt: Reg::S1,
                base: Reg::V1,
                offset: 2,
            },
        ),
        (
            0x8001_2990,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::S1,
                imm: 2,
            },
        ),
        (
            0x8001_299C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x2DFC,
            },
        ),
        (
            0x8001_29A8,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 8,
            },
        ),
        (
            0x8001_29B4,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S3,
                offset: 0x1C,
            },
        ),
        (
            0x8001_29BC,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 8,
            },
        ),
        (
            0x8001_29C4,
            K::Sw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0x18,
            },
        ),
        (
            0x8001_2A18,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 16,
            },
        ),
        (
            0x8001_2A1C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S3,
                offset: 0x1C,
            },
        ),
        (
            PCOMI1_COMPLETION_CALLBACK_RUNTIME + 0x18,
            K::Lw {
                rt: Reg::S1,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            PCOMI1_COMPLETION_CALLBACK_RUNTIME + 0xAC,
            K::Lw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0x18,
            },
        ),
        (
            PCOMI1_COMPLETION_CALLBACK_RUNTIME + 0xB4,
            K::Sw {
                rt: Reg::V1,
                base: Reg::S1,
                offset: 0x10,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, address, expected)?;
    }
    Ok(())
}

fn verify_transfer_handlers(instructions: &[Instruction]) -> Result<(), PcomiSeqLoadError> {
    let checks = [
        (
            CONTINUATION_HANDLER_RUNTIME + 0x28,
            K::Lbu {
                rt: Reg::S3,
                base: Reg::A0,
                offset: 1,
            },
        ),
        (
            CONTINUATION_HANDLER_RUNTIME + 0x2D4,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::S3,
                imm: 0x30,
            },
        ),
        (
            CONTINUATION_HANDLER_RUNTIME + 0x2D8,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 4,
            },
        ),
        (
            CONTINUATION_HANDLER_RUNTIME + 0x2EC,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S2,
                offset: 0x1C,
            },
        ),
        (
            CONTINUATION_HANDLER_RUNTIME + 0x2F4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
        (
            CONTINUATION_HANDLER_RUNTIME + 0x2F8,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S2,
                offset: 0x1C,
            },
        ),
        (
            DEFAULT_ADVANCE_HANDLER_RUNTIME,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            DEFAULT_ADVANCE_HANDLER_RUNTIME + 8,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 4,
            },
        ),
        (
            DEFAULT_ADVANCE_HANDLER_RUNTIME + 0x10,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            CALL_HANDLER_RUNTIME + 4,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            CALL_HANDLER_RUNTIME + 0x10,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
        (
            CALL_HANDLER_RUNTIME + 0x28,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 4,
            },
        ),
        (
            CALL_HANDLER_RUNTIME + 0x30,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            JUMP_HANDLER_RUNTIME,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            JUMP_HANDLER_RUNTIME + 8,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            JUMP_HANDLER_RUNTIME + 0x10,
            K::Sw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, address, expected)?;
    }
    Ok(())
}

fn verify_source_records(
    map: &ExeMap,
    text: &[u8],
    sequence: &[u8],
) -> Result<(), PcomiSeqLoadError> {
    if text_bytes(map, text, PCOMI1_LOAD_RECORD_RUNTIME, LOAD_RECORD.len())? != LOAD_RECORD {
        return Err(error("PCOMI1 16바이트 load record 불일치"));
    }
    if text_bytes(
        map,
        text,
        PCOMI1_MASTER_CONTINUATION_RUNTIME,
        MASTER_CONTINUATION.len(),
    )? != MASTER_CONTINUATION
    {
        return Err(error("PCOMI1 master continuation record 불일치"));
    }
    if text_bytes(map, text, PCOMI1_MASTER_CALL_RUNTIME, MASTER_CALL.len())? != MASTER_CALL {
        return Err(error("PCOMI1 master call record 불일치"));
    }
    if sequence.get(PCOMI1_OVERLAY_ENTRY_OFFSET..PCOMI1_OVERLAY_ENTRY_OFFSET + OVERLAY_ENTRY.len())
        != Some(OVERLAY_ENTRY.as_slice())
    {
        return Err(error("PCOMI1 overlay entry jump 불일치"));
    }
    if read_text_word(map, text, MASTER_ROOT_FIELD_RUNTIME)? != MASTER_ROOT_RUNTIME {
        return Err(error("PCOMI master root pointer 불일치"));
    }
    let descriptor = text_bytes(map, text, PCOMI1_DESCRIPTOR_RUNTIME, 36)?;
    let expected_descriptor = [
        0x0001_0000,
        PCOMI1_NAME_RUNTIME,
        PCOMI1_STORED_BYTES,
        PCOMI1_DECOMPRESSED_BYTES,
        0,
        0,
        0,
        0,
        0,
    ];
    for (index, expected) in expected_descriptor.into_iter().enumerate() {
        if read_word(descriptor, index * 4)? != expected {
            return Err(error(format!(
                "PCOMI1 descriptor word 불일치: index={index}"
            )));
        }
    }
    if text_bytes(map, text, PCOMI1_NAME_RUNTIME, 11)? != b"PCOMI1.SEQ\0" {
        return Err(error("PCOMI1 descriptor 이름표 불일치"));
    }
    Ok(())
}

fn verify_dispatch_slots(map: &ExeMap, text: &[u8]) -> Result<(), PcomiSeqLoadError> {
    for (opcode, expected) in [
        (0x03u8, JUMP_HANDLER_RUNTIME),
        (0x04, CALL_HANDLER_RUNTIME),
        (0x11, CONTINUATION_HANDLER_RUNTIME),
        (0x28, NESTED_COMMAND_HANDLER_RUNTIME),
        (0x62, DEFAULT_ADVANCE_HANDLER_RUNTIME),
    ] {
        let slot = MASTER_DISPATCH_TABLE_RUNTIME
            .checked_add(u32::from(opcode) * 4)
            .ok_or_else(|| error("PCOMI dispatcher slot 주소 overflow"))?;
        if read_text_word(map, text, slot)? != expected {
            return Err(error(format!(
                "PCOMI dispatcher slot 불일치: opcode=0x{opcode:02X}"
            )));
        }
    }
    let nested_slot = NESTED_DISPATCH_TABLE_RUNTIME + 0x10 * 4;
    if read_text_word(map, text, nested_slot)? != RESOURCE_SELECTOR_RUNTIME {
        return Err(error("PCOMI nested dispatcher slot 불일치: opcode=0x10"));
    }
    Ok(())
}

/// 지원 원본에서 PCOMI master command의 resource 1 적재와 overlay 진입을 검산한다.
pub fn verify_box_pcomi_seq_load_route(
    map: &ExeMap,
    text: &[u8],
    sequence: &[u8],
) -> Result<PcomiSeqLoadRoute, PcomiSeqLoadError> {
    if map.name != "PCOMI.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "PCOMI load EXE 사상 불일치: name={}, expected_len={}, actual_len={}",
            map.name,
            map.text_len,
            text.len()
        )));
    }
    if sequence.len() != PCOMI1_SEQUENCE_BYTES
        || Sha256::digest(sequence).as_slice() != PCOMI1_SEQUENCE_SHA256
    {
        return Err(error("PCOMI1.SEQ 원본 길이 또는 SHA-256 불일치"));
    }
    let instructions =
        linear::sweep(map, text).ok_or_else(|| error("PCOMI.EXE text 정렬 불일치"))?;
    verify_master_entry(&instructions)?;
    verify_dispatcher(&instructions)?;
    verify_load_dispatch(&instructions)?;
    verify_selector_and_completion(&instructions)?;
    verify_transfer_handlers(&instructions)?;
    verify_source_records(map, text, sequence)?;
    verify_dispatch_slots(map, text)?;

    Ok(PcomiSeqLoadRoute {
        load_record_runtime: PCOMI1_LOAD_RECORD_RUNTIME,
        resource_type: PCOMI1_RESOURCE_TYPE,
        resource_id: PCOMI1_RESOURCE_ID,
        flags: PCOMI1_LOAD_FLAGS,
        descriptor_runtime: PCOMI1_DESCRIPTOR_RUNTIME,
        destination_runtime: PCOMI1_LOAD_BASE,
        completion_callback_runtime: PCOMI1_COMPLETION_CALLBACK_RUNTIME,
        master_call_runtime: PCOMI1_MASTER_CALL_RUNTIME,
        overlay_entry_runtime: PCOMI1_OVERLAY_ENTRY_RUNTIME,
        overlay_start_runtime: PCOMI1_OVERLAY_START_RUNTIME,
    })
}

#[cfg(test)]
#[path = "subgame_seq_pcomi_load_tests.rs"]
mod tests;
