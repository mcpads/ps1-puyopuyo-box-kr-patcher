//! E0 command 등록 table과 message handler dispatch를 검증한다.

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{
    error, high_low, require, SubgameSeqDracConsumerError, SubgameSeqDracConsumerSpec,
    BOX_SUBGAME_SEQ_DRAC_COMMAND_OPCODE, BOX_SUBGAME_SEQ_DRAC_COMMAND_TABLE_ENTRIES,
};

pub(super) fn verify_command_dispatch(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    spec: SubgameSeqDracConsumerSpec,
) -> Result<(), SubgameSeqDracConsumerError> {
    let (source_high, source_low) = high_low(spec.command_source_table_runtime);
    let registration = spec.command_registration_runtime;
    let registration_checks = [
        (
            registration + 4,
            K::Lui {
                rt: Reg::A0,
                imm: source_high,
            },
        ),
        (
            registration + 8,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: source_low,
            },
        ),
        (
            registration + 0x0C,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: i16::from(BOX_SUBGAME_SEQ_DRAC_COMMAND_OPCODE),
            },
        ),
        (
            spec.command_registration_call_runtime,
            K::Jal {
                target: spec.command_registration_target_runtime,
            },
        ),
        (
            spec.command_registration_call_runtime + 4,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: i16::try_from(BOX_SUBGAME_SEQ_DRAC_COMMAND_TABLE_ENTRIES)
                    .map_err(|_| error("DRAC command table entry 수가 i16을 넘음"))?,
            },
        ),
    ];
    for (address, expected) in registration_checks {
        require(instructions, spec, address, expected)?;
    }

    let (dispatch_high, dispatch_low) = high_low(spec.command_dispatch_table_runtime);
    let register = spec.command_registration_target_runtime;
    let register_checks = [
        (
            register + 8,
            K::Sll {
                rd: Reg::A1,
                rt: Reg::A1,
                sa: 2,
            },
        ),
        (
            register + 0x0C,
            K::Lui {
                rt: Reg::A0,
                imm: dispatch_high,
            },
        ),
        (
            register + 0x10,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: dispatch_low,
            },
        ),
        (
            register + 0x14,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A1,
                rt: Reg::A0,
            },
        ),
        (
            register + 0x1C,
            K::Jal {
                target: spec.command_copy_words_runtime,
            },
        ),
        (
            register + 0x20,
            K::Addu {
                rd: Reg::A1,
                rs: Reg::V0,
                rt: Reg::ZERO,
            },
        ),
    ];
    for (address, expected) in register_checks {
        require(instructions, spec, address, expected)?;
    }

    let copy = spec.command_copy_words_runtime;
    let copy_checks = [
        (
            copy,
            K::Beq {
                rs: Reg::A2,
                rt: Reg::ZERO,
                offset: 7,
            },
        ),
        (
            copy + 8,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 0,
            },
        ),
        (
            copy + 0x0C,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 4,
            },
        ),
        (
            copy + 0x10,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::A2,
                imm: -1,
            },
        ),
        (
            copy + 0x14,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0,
            },
        ),
        (
            copy + 0x18,
            K::Bne {
                rs: Reg::A2,
                rt: Reg::ZERO,
                offset: -5,
            },
        ),
        (
            copy + 0x1C,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 4,
            },
        ),
    ];
    for (address, expected) in copy_checks {
        require(instructions, spec, address, expected)?;
    }

    let dispatcher = spec.command_dispatcher_runtime;
    let dispatcher_checks = [
        (
            dispatcher + 0x0C,
            K::Lui {
                rt: Reg::V0,
                imm: dispatch_high,
            },
        ),
        (
            dispatcher + 0x14,
            K::Addiu {
                rt: Reg::S1,
                rs: Reg::V0,
                imm: dispatch_low,
            },
        ),
        (
            dispatcher + 0x20,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
        (
            dispatcher + 0x28,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            dispatcher + 0x30,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            dispatcher + 0x34,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::S1,
            },
        ),
        (
            dispatcher + 0x38,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            dispatcher + 0x40,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
    ];
    for (address, expected) in dispatcher_checks {
        require(instructions, spec, address, expected)?;
    }

    let table_offset = map
        .text_offset(spec.command_source_table_runtime)
        .ok_or_else(|| error("DRAC command source table이 EXE 밖임"))?;
    let table = text
        .get(table_offset..table_offset + BOX_SUBGAME_SEQ_DRAC_COMMAND_TABLE_ENTRIES * 4)
        .ok_or_else(|| error("DRAC command source table 범위 이탈"))?;
    for (index, bytes) in table.chunks_exact(4).enumerate() {
        let pointer = u32::from_le_bytes(bytes.try_into().expect("4바이트 word"));
        if !map.contains(pointer) || !pointer.is_multiple_of(4) {
            return Err(error(format!(
                "{} DRAC command table pointer 불일치: index={index}, value=0x{pointer:08X}",
                spec.exe
            )));
        }
        if index == 0 && pointer != spec.message_handler_runtime {
            return Err(error(format!(
                "{} DRAC command E0 handler 불일치: expected=0x{:08X}, actual=0x{pointer:08X}",
                spec.exe, spec.message_handler_runtime
            )));
        }
    }
    if spec.current_record_load_runtime != spec.message_handler_runtime + 0x3C {
        return Err(error(format!(
            "{} DRAC message handler/current record 상대 위치 불일치",
            spec.exe
        )));
    }
    Ok(())
}
