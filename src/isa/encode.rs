//! 프로젝트 분석 표현을 공통 `psx-r3000a` encoder로 내린다.
//!
//! known 명령의 bit-level 조립은 공통 crate만 소유한다. `Unknown`의 raw word
//! 반환은 원본 조사 왕복을 위한 경계이며 제품 기계어 생성 승인이 아니다.
//! 제품 쓰기는 계속 [`super::verify::verify_typed_source`]를 거쳐야 한다.

use super::instruction::{GteCommand, InstructionKind};
use super::typed_source::{checked_gte_opcode, lower_instruction};

fn placement_for_encoding(kind: InstructionKind) -> u32 {
    match kind {
        InstructionKind::J { target } | InstructionKind::Jal { target } => target & 0xF000_0000,
        _ => 0,
    }
}

/// 분석·테스트용 명령 하나를 공통 encoder로 조립한다.
///
/// `Unknown`과 미지 GTE command는 원본 raw word만 보존한다. 새 제품 code를
/// 만들 때 이 함수로 raw word를 승인해서는 안 된다.
pub fn encode(kind: InstructionKind) -> u32 {
    match kind {
        InstructionKind::Unknown { word } => word,
        InstructionKind::Gte {
            command: GteCommand::Unknown(_),
            raw,
        } => raw,
        _ => {
            let address = placement_for_encoding(kind);
            let instruction = lower_instruction(kind, address)
                .expect("known 로컬 명령은 공통 PSX profile로 내려가야 한다");
            psx_r3000a::encode_word(&instruction, address)
                .expect("공통 profile에서 온 명령은 같은 주소에 조립돼야 한다")
        }
    }
}

/// 분석·테스트용 명령 목록을 리틀엔디언 byte stream으로 조립한다.
pub fn encode_all(kinds: &[InstructionKind]) -> Vec<u8> {
    let mut out = Vec::with_capacity(kinds.len() * 4);
    for &kind in kinds {
        out.extend_from_slice(&encode(kind).to_le_bytes());
    }
    out
}

/// GTE command의 의미 필드를 공통 encoder로 조립한다.
///
/// `arg_bits`는 원래 word의 24:6 필드다. 공통 profile이 reserved로 선언한
/// bit가 서 있거나 command가 profile 밖이면 `None`이다.
pub fn encode_gte(command: GteCommand, arg_bits: u32) -> Option<u32> {
    if arg_bits & 0x0000_1BC0 != 0 {
        return None;
    }
    let command = psx_r3000a::GteCommand {
        opcode: checked_gte_opcode(command)?,
        fake: ((arg_bits >> 20) & 0x1F) as u8,
        shift_fraction: arg_bits & (1 << 19) != 0,
        matrix: ((arg_bits >> 17) & 0x03) as u8,
        vector: ((arg_bits >> 15) & 0x03) as u8,
        control_vector: ((arg_bits >> 13) & 0x03) as u8,
        limit_unsigned: arg_bits & (1 << 10) != 0,
    };
    psx_r3000a::encode_word(&psx_r3000a::Instruction::Gte(command), 0).ok()
}

#[cfg(test)]
#[path = "encode_tests.rs"]
mod encode_tests;
