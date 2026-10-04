//! 합성 OP21 record와 constructor·dispatch 명령 계약 검증.

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::trace_analysis::{high_low, read_word, require};
use super::{
    error, SubgameSeqTextBridgeError, SubgameSeqTextBridgeSpec, CONSTRUCTOR_STACK_ARGUMENT_OFFSET,
    SUBGAME_SEQ_TEXT_BRIDGE_MODE, SUBGAME_SEQ_TEXT_BRIDGE_OPCODE,
    SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES, TEXT_OBJECT_CURRENT_RECORD_OFFSET,
    TEXT_OBJECT_STREAM_OFFSET,
};

pub(super) fn verify_constructor(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    spec: SubgameSeqTextBridgeSpec,
) -> Result<(), SubgameSeqTextBridgeError> {
    require(
        instructions,
        spec,
        spec.fifth_argument_load_runtime,
        K::Lw {
            rt: Reg::S4,
            base: Reg::SP,
            offset: CONSTRUCTOR_STACK_ARGUMENT_OFFSET,
        },
    )?;
    require(
        instructions,
        spec,
        spec.synthetic_base_high_runtime,
        K::Lui {
            rt: Reg::S0,
            imm: spec.synthetic_base_high,
        },
    )?;
    require(
        instructions,
        spec,
        spec.synthetic_pointer_store_runtime,
        K::Sw {
            rt: Reg::S4,
            base: Reg::S0,
            offset: spec.synthetic_pointer_offset,
        },
    )?;
    require(
        instructions,
        spec,
        spec.synthetic_pointer_adjust_runtime,
        K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: spec.synthetic_pointer_offset,
        },
    )?;
    require(
        instructions,
        spec,
        spec.synthetic_record_adjust_runtime,
        K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: -4,
        },
    )?;
    let synthetic_pointer = ((u32::from(spec.synthetic_base_high)) << 16)
        .wrapping_add_signed(i32::from(spec.synthetic_pointer_offset));
    if synthetic_pointer != spec.synthetic_record_runtime + 4 {
        return Err(error(format!(
            "{} synthetic record +4 주소 계산 불일치",
            spec.exe
        )));
    }

    require(
        instructions,
        spec,
        spec.callback_high_runtime,
        K::Lui {
            rt: spec.callback_high_reg,
            imm: spec.callback_high,
        },
    )?;
    require(
        instructions,
        spec,
        spec.callback_low_runtime,
        K::Addiu {
            rt: Reg::A0,
            rs: spec.callback_high_reg,
            imm: spec.callback_low,
        },
    )?;
    let callback =
        ((u32::from(spec.callback_high)) << 16).wrapping_add_signed(i32::from(spec.callback_low));
    if callback != spec.dispatcher_runtime {
        return Err(error(format!(
            "{} constructor callback/dispatcher 주소 불일치",
            spec.exe
        )));
    }
    require(
        instructions,
        spec,
        spec.allocator_call_runtime,
        K::Jal {
            target: spec.allocator_runtime,
        },
    )?;
    if let Some(address) = spec.allocator_result_move_runtime {
        require(
            instructions,
            spec,
            address,
            K::Addu {
                rd: spec.object_reg,
                rs: Reg::V0,
                rt: Reg::ZERO,
            },
        )?;
    } else if spec.object_reg != Reg::V0 {
        return Err(error(format!(
            "{} allocator 결과 register 계약 누락",
            spec.exe
        )));
    }
    require(
        instructions,
        spec,
        spec.current_record_store_runtime,
        K::Sw {
            rt: Reg::S0,
            base: spec.object_reg,
            offset: TEXT_OBJECT_CURRENT_RECORD_OFFSET,
        },
    )?;
    require(
        instructions,
        spec,
        spec.object_stream_store_runtime,
        K::Sw {
            rt: Reg::S4,
            base: spec.object_reg,
            offset: TEXT_OBJECT_STREAM_OFFSET,
        },
    )?;

    let synthetic_offset = map
        .text_offset(spec.synthetic_record_runtime)
        .ok_or_else(|| error(format!("{} synthetic record가 EXE 밖임", spec.exe)))?;
    if text.get(
        synthetic_offset..synthetic_offset + SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES.len(),
    ) != Some(SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES.as_slice())
    {
        return Err(error(format!(
            "{} synthetic OP21/mode2 record 원본 불일치",
            spec.exe
        )));
    }
    Ok(())
}

pub(super) fn count_canonical_mode2_records(raw: &[u8]) -> usize {
    raw.chunks_exact(4)
        .enumerate()
        .filter(|(index, word)| {
            *word
                == [
                    SUBGAME_SEQ_TEXT_BRIDGE_OPCODE,
                    SUBGAME_SEQ_TEXT_BRIDGE_MODE,
                    0,
                    0,
                ]
                .as_slice()
                && raw.get(*index * 4 + 4..*index * 4 + 8).is_some()
        })
        .count()
}

pub(super) fn verify_dispatch(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    spec: SubgameSeqTextBridgeSpec,
) -> Result<(), SubgameSeqTextBridgeError> {
    let (table_high, table_low) = high_low(spec.dispatcher_table_runtime);
    require(
        instructions,
        spec,
        spec.dispatcher_runtime + 0x0C,
        K::Lui {
            rt: Reg::V0,
            imm: table_high,
        },
    )?;
    require(
        instructions,
        spec,
        spec.dispatcher_runtime + 0x14,
        K::Addiu {
            rt: Reg::S1,
            rs: Reg::V0,
            imm: table_low,
        },
    )?;
    require(
        instructions,
        spec,
        spec.dispatcher_runtime + 0x20,
        K::Lw {
            rt: spec.dispatcher_record_reg,
            base: Reg::S0,
            offset: TEXT_OBJECT_CURRENT_RECORD_OFFSET,
        },
    )?;
    require(
        instructions,
        spec,
        spec.dispatcher_runtime + 0x28,
        K::Lbu {
            rt: spec.dispatcher_opcode_reg,
            base: spec.dispatcher_record_reg,
            offset: 0,
        },
    )?;
    require(
        instructions,
        spec,
        spec.dispatcher_runtime + 0x30,
        K::Sll {
            rd: spec.dispatcher_opcode_reg,
            rt: spec.dispatcher_opcode_reg,
            sa: 2,
        },
    )?;
    require(
        instructions,
        spec,
        spec.dispatcher_runtime + 0x34,
        K::Addu {
            rd: spec.dispatcher_opcode_reg,
            rs: spec.dispatcher_opcode_reg,
            rt: Reg::S1,
        },
    )?;
    require(
        instructions,
        spec,
        spec.dispatcher_runtime + 0x38,
        K::Lw {
            rt: spec.dispatcher_handler_reg,
            base: spec.dispatcher_opcode_reg,
            offset: 0,
        },
    )?;
    require(
        instructions,
        spec,
        spec.dispatcher_runtime + 0x40,
        K::Jalr {
            rd: Reg::RA,
            rs: spec.dispatcher_handler_reg,
        },
    )?;

    let op21_slot = spec.dispatcher_table_runtime + u32::from(SUBGAME_SEQ_TEXT_BRIDGE_OPCODE) * 4;
    if read_word(map, text, op21_slot)? != spec.op21_handler_runtime {
        return Err(error(format!("{} dispatcher OP21 slot 불일치", spec.exe)));
    }

    let op21 = spec.op21_handler_runtime;
    require(
        instructions,
        spec,
        op21 + 0x10,
        K::Lw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: TEXT_OBJECT_CURRENT_RECORD_OFFSET,
        },
    )?;
    require(
        instructions,
        spec,
        op21 + 0x18,
        K::Lbu {
            rt: spec.op21_mode_reg,
            base: Reg::V0,
            offset: 1,
        },
    )?;
    let (mode_high, mode_low) = high_low(spec.op21_mode_table_runtime);
    require(
        instructions,
        spec,
        op21 + 0x28,
        K::Lui {
            rt: Reg::V0,
            imm: mode_high,
        },
    )?;
    require(
        instructions,
        spec,
        op21 + 0x2C,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: mode_low,
        },
    )?;
    require(
        instructions,
        spec,
        op21 + 0x30,
        K::Sll {
            rd: Reg::V1,
            rt: spec.op21_mode_reg,
            sa: 2,
        },
    )?;
    require(
        instructions,
        spec,
        op21 + 0x34,
        K::Addu {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::V0,
        },
    )?;
    require(
        instructions,
        spec,
        op21 + 0x38,
        K::Lw {
            rt: spec.op21_mode_handler_reg,
            base: Reg::V1,
            offset: 0,
        },
    )?;
    require(
        instructions,
        spec,
        op21 + 0x40,
        K::Jr {
            rs: spec.op21_mode_handler_reg,
        },
    )?;
    let mode2_slot = spec.op21_mode_table_runtime + u32::from(SUBGAME_SEQ_TEXT_BRIDGE_MODE) * 4;
    if read_word(map, text, mode2_slot)? != spec.op21_mode2_handler_runtime {
        return Err(error(format!("{} OP21 mode2 slot 불일치", spec.exe)));
    }
    let mode2 = spec.op21_mode2_handler_runtime;
    require(
        instructions,
        spec,
        mode2,
        K::Lbu {
            rt: Reg::V1,
            base: Reg::S0,
            offset: 0x77,
        },
    )?;
    require(
        instructions,
        spec,
        mode2 + 0x20,
        K::Lw {
            rt: Reg::A0,
            base: Reg::S0,
            offset: TEXT_OBJECT_CURRENT_RECORD_OFFSET,
        },
    )?;
    require(
        instructions,
        spec,
        mode2 + 0x2C,
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 2,
        },
    )?;
    require(
        instructions,
        spec,
        mode2 + 0x4C,
        K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 4,
        },
    )?;
    require(
        instructions,
        spec,
        mode2 + 0x50,
        K::J {
            target: mode2 + 0xBC,
        },
    )?;
    require(
        instructions,
        spec,
        mode2 + 0x54,
        K::Sw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: TEXT_OBJECT_STREAM_OFFSET,
        },
    )?;
    Ok(())
}
