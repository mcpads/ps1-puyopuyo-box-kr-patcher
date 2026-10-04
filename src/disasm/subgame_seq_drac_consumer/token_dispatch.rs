//! message 본문 token cursor와 dispatch table을 검증한다.

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{
    error, high_low, require, SubgameSeqDracConsumerError, SubgameSeqDracConsumerSpec,
    BOX_SUBGAME_SEQ_DRAC_TOKEN_FIRST,
};

pub(super) fn verify_token_dispatch(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    spec: SubgameSeqDracConsumerSpec,
) -> Result<(), SubgameSeqDracConsumerError> {
    let start = spec.token_cursor_load_runtime;
    let (token_high, token_low) = high_low(spec.token_table_runtime);
    let (dispatch_high, dispatch_low) = high_low(spec.dispatch_table_runtime);
    let checks = [
        (
            start - 4,
            K::Lui {
                rt: Reg::V1,
                imm: token_high,
            },
        ),
        (
            start,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x58,
            },
        ),
        (
            start + 4,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: token_low,
            },
        ),
        (
            start + 0x14,
            K::Lbu {
                rt: Reg::S1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            start + 0x18,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x74,
            },
        ),
        (
            start + 0x20,
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 6,
            },
        ),
        (
            start + 0x24,
            K::Sw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x58,
            },
        ),
        (
            start + 0x28,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x77,
            },
        ),
        (
            start + 0x2C,
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 3,
            },
        ),
        (
            start + 0x30,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x71,
            },
        ),
        (
            start + 0x34,
            K::Bne {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 10,
            },
        ),
        (
            start + 0x40,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x58,
            },
        ),
        (
            start + 0x50,
            K::Lw {
                rt: Reg::A2,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            start + 0x54,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 1,
            },
        ),
        (
            start + 0x58,
            K::Sw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x58,
            },
        ),
        (
            start + 0x60,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V0,
                imm: -i16::from(BOX_SUBGAME_SEQ_DRAC_TOKEN_FIRST),
            },
        ),
        (
            start + 0x64,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: i16::try_from(spec.dispatch_targets.len())
                    .map_err(|_| error("DRAC dispatch entry 수가 i16을 넘음"))?,
            },
        ),
        (
            start + 0x6C,
            K::Lui {
                rt: Reg::V0,
                imm: dispatch_high,
            },
        ),
        (
            start + 0x70,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: dispatch_low,
            },
        ),
        (start + 0x84, K::Jr { rs: Reg::V0 }),
    ];
    for (address, expected) in checks {
        require(instructions, spec, address, expected)?;
    }

    for (index, &expected) in spec.dispatch_targets.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| error("DRAC dispatch index 초과"))?;
        let runtime = spec
            .dispatch_table_runtime
            .checked_add(index * 4)
            .ok_or_else(|| error("DRAC dispatch table runtime 초과"))?;
        let offset = map
            .text_offset(runtime)
            .ok_or_else(|| error("DRAC dispatch table이 EXE 밖임"))?;
        let actual = text
            .get(offset..offset + 4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
            .ok_or_else(|| error("DRAC dispatch table word 범위 이탈"))?;
        if actual != expected || !map.contains(actual) || !actual.is_multiple_of(4) {
            return Err(error(format!(
                "{} DRAC dispatch 0x{:02X} 불일치: expected=0x{expected:08X}, actual=0x{actual:08X}",
                spec.exe,
                0x63 + index
            )));
        }
    }
    Ok(())
}
