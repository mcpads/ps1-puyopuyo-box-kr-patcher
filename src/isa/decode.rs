//! 공통 `psx-r3000a` 의미 모델을 프로젝트의 분석 표현으로 잇는다.
//!
//! 공통 crate의 엄격한 decoder는 제품 기계어 검산에서 미지원 word를
//! 거부한다. 원본 조사의 raw word·주소 보존과 `Unknown` 판정은 이 프로젝트의
//! 분석 adapter가 소유한다. 알려진 word만 공통 [`psx_r3000a::decode_word`]로
//! 의미화한 뒤 기존 분석기가 사용하는 상대 branch immediate 표현으로
//! 바꾼다. opcode 해석을 이 프로젝트에서 다시 구현하지 않는다.

use psx_r3000a::{
    GteOpcode as SharedGteOpcode, Instruction as SharedInstruction, Register as SharedRegister,
};

use super::instruction::{GteCommand, Instruction, InstructionKind, Reg};

fn local_register(register: SharedRegister) -> Reg {
    Reg(register.index())
}

fn branch_offset(address: u32, target: u32) -> i16 {
    let displacement = target.wrapping_sub(address.wrapping_add(4)) as i32;
    debug_assert_eq!(displacement & 3, 0);
    i16::try_from(displacement / 4).expect("공통 decoder의 branch target은 i16 범위다")
}

fn local_gte_opcode(opcode: SharedGteOpcode) -> GteCommand {
    match opcode {
        SharedGteOpcode::Rtps => GteCommand::Rtps,
        SharedGteOpcode::Nclip => GteCommand::Nclip,
        SharedGteOpcode::Op => GteCommand::Op,
        SharedGteOpcode::Dpcs => GteCommand::Dpcs,
        SharedGteOpcode::Intpl => GteCommand::Intpl,
        SharedGteOpcode::Mvmva => GteCommand::Mvmva,
        SharedGteOpcode::Ncds => GteCommand::Ncds,
        SharedGteOpcode::Cdp => GteCommand::Cdp,
        SharedGteOpcode::Ncdt => GteCommand::Ncdt,
        SharedGteOpcode::Nccs => GteCommand::Nccs,
        SharedGteOpcode::Cc => GteCommand::Cc,
        SharedGteOpcode::Ncs => GteCommand::Ncs,
        SharedGteOpcode::Nct => GteCommand::Nct,
        SharedGteOpcode::Sqr => GteCommand::Sqr,
        SharedGteOpcode::Dcpl => GteCommand::Dcpl,
        SharedGteOpcode::Dpct => GteCommand::Dpct,
        SharedGteOpcode::Avsz3 => GteCommand::Avsz3,
        SharedGteOpcode::Avsz4 => GteCommand::Avsz4,
        SharedGteOpcode::Rtpt => GteCommand::Rtpt,
        SharedGteOpcode::Gpf => GteCommand::Gpf,
        SharedGteOpcode::Gpl => GteCommand::Gpl,
        SharedGteOpcode::Ncct => GteCommand::Ncct,
    }
}

fn local_instruction(instruction: SharedInstruction, address: u32, word: u32) -> InstructionKind {
    use InstructionKind as K;
    use SharedInstruction as C;

    match instruction {
        C::Add { rd, rs, rt } => K::Add {
            rd: local_register(rd),
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::Addu { rd, rs, rt } => K::Addu {
            rd: local_register(rd),
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::Sub { rd, rs, rt } => K::Sub {
            rd: local_register(rd),
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::Subu { rd, rs, rt } => K::Subu {
            rd: local_register(rd),
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::And { rd, rs, rt } => K::And {
            rd: local_register(rd),
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::Or { rd, rs, rt } => K::Or {
            rd: local_register(rd),
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::Xor { rd, rs, rt } => K::Xor {
            rd: local_register(rd),
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::Nor { rd, rs, rt } => K::Nor {
            rd: local_register(rd),
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::Slt { rd, rs, rt } => K::Slt {
            rd: local_register(rd),
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::Sltu { rd, rs, rt } => K::Sltu {
            rd: local_register(rd),
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::Addi { rt, rs, immediate } => K::Addi {
            rt: local_register(rt),
            rs: local_register(rs),
            imm: immediate,
        },
        C::Addiu { rt, rs, immediate } => K::Addiu {
            rt: local_register(rt),
            rs: local_register(rs),
            imm: immediate,
        },
        C::Slti { rt, rs, immediate } => K::Slti {
            rt: local_register(rt),
            rs: local_register(rs),
            imm: immediate,
        },
        C::Sltiu { rt, rs, immediate } => K::Sltiu {
            rt: local_register(rt),
            rs: local_register(rs),
            imm: immediate,
        },
        C::Andi { rt, rs, immediate } => K::Andi {
            rt: local_register(rt),
            rs: local_register(rs),
            imm: immediate,
        },
        C::Ori { rt, rs, immediate } => K::Ori {
            rt: local_register(rt),
            rs: local_register(rs),
            imm: immediate,
        },
        C::Xori { rt, rs, immediate } => K::Xori {
            rt: local_register(rt),
            rs: local_register(rs),
            imm: immediate,
        },
        C::Lui { rt, immediate } => K::Lui {
            rt: local_register(rt),
            imm: immediate,
        },
        C::Sll { rd, rt, shift } => K::Sll {
            rd: local_register(rd),
            rt: local_register(rt),
            sa: shift,
        },
        C::Srl { rd, rt, shift } => K::Srl {
            rd: local_register(rd),
            rt: local_register(rt),
            sa: shift,
        },
        C::Sra { rd, rt, shift } => K::Sra {
            rd: local_register(rd),
            rt: local_register(rt),
            sa: shift,
        },
        C::Sllv { rd, rt, rs } => K::Sllv {
            rd: local_register(rd),
            rt: local_register(rt),
            rs: local_register(rs),
        },
        C::Srlv { rd, rt, rs } => K::Srlv {
            rd: local_register(rd),
            rt: local_register(rt),
            rs: local_register(rs),
        },
        C::Srav { rd, rt, rs } => K::Srav {
            rd: local_register(rd),
            rt: local_register(rt),
            rs: local_register(rs),
        },
        C::Mult { rs, rt } => K::Mult {
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::Multu { rs, rt } => K::Multu {
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::Div { rs, rt } => K::Div {
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::Divu { rs, rt } => K::Divu {
            rs: local_register(rs),
            rt: local_register(rt),
        },
        C::Mfhi { rd } => K::Mfhi {
            rd: local_register(rd),
        },
        C::Mthi { rs } => K::Mthi {
            rs: local_register(rs),
        },
        C::Mflo { rd } => K::Mflo {
            rd: local_register(rd),
        },
        C::Mtlo { rs } => K::Mtlo {
            rs: local_register(rs),
        },
        C::Lb { rt, base, offset } => K::Lb {
            rt: local_register(rt),
            base: local_register(base),
            offset,
        },
        C::Lbu { rt, base, offset } => K::Lbu {
            rt: local_register(rt),
            base: local_register(base),
            offset,
        },
        C::Lh { rt, base, offset } => K::Lh {
            rt: local_register(rt),
            base: local_register(base),
            offset,
        },
        C::Lhu { rt, base, offset } => K::Lhu {
            rt: local_register(rt),
            base: local_register(base),
            offset,
        },
        C::Lw { rt, base, offset } => K::Lw {
            rt: local_register(rt),
            base: local_register(base),
            offset,
        },
        C::Lwl { rt, base, offset } => K::Lwl {
            rt: local_register(rt),
            base: local_register(base),
            offset,
        },
        C::Lwr { rt, base, offset } => K::Lwr {
            rt: local_register(rt),
            base: local_register(base),
            offset,
        },
        C::Sb { rt, base, offset } => K::Sb {
            rt: local_register(rt),
            base: local_register(base),
            offset,
        },
        C::Sh { rt, base, offset } => K::Sh {
            rt: local_register(rt),
            base: local_register(base),
            offset,
        },
        C::Sw { rt, base, offset } => K::Sw {
            rt: local_register(rt),
            base: local_register(base),
            offset,
        },
        C::Swl { rt, base, offset } => K::Swl {
            rt: local_register(rt),
            base: local_register(base),
            offset,
        },
        C::Swr { rt, base, offset } => K::Swr {
            rt: local_register(rt),
            base: local_register(base),
            offset,
        },
        C::Beq { rs, rt, target } => K::Beq {
            rs: local_register(rs),
            rt: local_register(rt),
            offset: branch_offset(address, target),
        },
        C::Bne { rs, rt, target } => K::Bne {
            rs: local_register(rs),
            rt: local_register(rt),
            offset: branch_offset(address, target),
        },
        C::Blez { rs, target } => K::Blez {
            rs: local_register(rs),
            offset: branch_offset(address, target),
        },
        C::Bgtz { rs, target } => K::Bgtz {
            rs: local_register(rs),
            offset: branch_offset(address, target),
        },
        C::Bltz { rs, target } => K::Bltz {
            rs: local_register(rs),
            offset: branch_offset(address, target),
        },
        C::Bgez { rs, target } => K::Bgez {
            rs: local_register(rs),
            offset: branch_offset(address, target),
        },
        C::Bltzal { rs, target } => K::Bltzal {
            rs: local_register(rs),
            offset: branch_offset(address, target),
        },
        C::Bgezal { rs, target } => K::Bgezal {
            rs: local_register(rs),
            offset: branch_offset(address, target),
        },
        C::J { target } => K::J { target },
        C::Jal { target } => K::Jal { target },
        C::Jr { rs } => K::Jr {
            rs: local_register(rs),
        },
        C::Jalr { rd, rs } => K::Jalr {
            rd: local_register(rd),
            rs: local_register(rs),
        },
        C::Syscall { code } => K::Syscall { code },
        C::Break { code } => K::Break { code },
        C::Mfc0 { rt, rd } => K::Mfc0 {
            rt: local_register(rt),
            rd,
        },
        C::Mtc0 { rt, rd } => K::Mtc0 {
            rt: local_register(rt),
            rd,
        },
        C::Rfe => K::Rfe,
        C::Mfc2 { rt, rd } => K::Mfc2 {
            rt: local_register(rt),
            rd,
        },
        C::Mtc2 { rt, rd } => K::Mtc2 {
            rt: local_register(rt),
            rd,
        },
        C::Cfc2 { rt, rd } => K::Cfc2 {
            rt: local_register(rt),
            rd,
        },
        C::Ctc2 { rt, rd } => K::Ctc2 {
            rt: local_register(rt),
            rd,
        },
        C::Lwc2 { rt, base, offset } => K::Lwc2 {
            rt,
            base: local_register(base),
            offset,
        },
        C::Swc2 { rt, base, offset } => K::Swc2 {
            rt,
            base: local_register(base),
            offset,
        },
        C::Gte(command) => K::Gte {
            command: local_gte_opcode(command.opcode),
            raw: word,
        },
    }
}

/// 워드 하나를 공통 profile로 분류한다.
///
/// 정렬되지 않은 단일 주소는 기존 단일-word 분석 API의 손실 없는 성격을
/// 유지해 `Unknown`으로 남긴다. 연속 stream은 [`decode_slice`]에서 정렬과
/// 주소 overflow를 실패시킨다.
pub fn decode_word(addr: u32, word: u32) -> InstructionKind {
    match psx_r3000a::decode_word(word, addr) {
        Ok(instruction) => local_instruction(instruction, addr, word),
        Err(_) => InstructionKind::Unknown { word },
    }
}

/// 주소·원본 word·분류 결과를 하나의 분석 단위로 묶는다.
pub fn decode(addr: u32, word: u32) -> Instruction {
    Instruction::new(addr, word, decode_word(addr, word))
}

/// 리틀엔디언 byte stream을 주소 overflow 없이 빈틈없이 분류한다.
///
/// 길이가 4의 배수가 아니거나 origin이 정렬되지 않았거나 주소가 넘치면
/// `None`이며 부분 결과를 반환하지 않는다.
pub fn decode_slice(base_addr: u32, bytes: &[u8]) -> Option<Vec<Instruction>> {
    if base_addr & 3 != 0 || !bytes.len().is_multiple_of(4) {
        return None;
    }
    bytes
        .chunks_exact(4)
        .enumerate()
        .map(|(index, chunk)| {
            let offset = u32::try_from(index).ok()?.checked_mul(4)?;
            let address = base_addr.checked_add(offset)?;
            let word = u32::from_le_bytes(chunk.try_into().expect("4바이트 word"));
            Some(Instruction::new(address, word, decode_word(address, word)))
        })
        .collect()
}

#[cfg(test)]
#[path = "decode_tests.rs"]
mod decode_tests;
