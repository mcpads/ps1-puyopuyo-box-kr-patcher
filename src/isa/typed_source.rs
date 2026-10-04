//! 제품용 typed source를 공통 PlayStation ISA 표현으로 내린다.
//!
//! 로컬 [`InstructionKind`]는 원본 조사에서 `Unknown`과 분기 immediate를
//! 보존한다. 제품 기계어는 이 adapter를 통과해 `psx-r3000a`의 배치 주소
//! 기반 의미 표현이 된 뒤 공통 checked assembler에서만 바이트가 된다.

use psx_r3000a::{
    GteOpcode as CheckedGteOpcode, Instruction as CheckedInstruction, Register as CheckedRegister,
};

use super::instruction::{GteCommand, InstructionKind, Reg};

fn register(register: Reg) -> Option<CheckedRegister> {
    CheckedRegister::new(register.0)
}

fn branch_target(address: u32, offset: i16) -> u32 {
    address
        .wrapping_add(4)
        .wrapping_add(((offset as i32) << 2) as u32)
}

pub(super) fn checked_gte_opcode(command: GteCommand) -> Option<CheckedGteOpcode> {
    Some(match command {
        GteCommand::Rtps => CheckedGteOpcode::Rtps,
        GteCommand::Nclip => CheckedGteOpcode::Nclip,
        GteCommand::Op => CheckedGteOpcode::Op,
        GteCommand::Dpcs => CheckedGteOpcode::Dpcs,
        GteCommand::Intpl => CheckedGteOpcode::Intpl,
        GteCommand::Mvmva => CheckedGteOpcode::Mvmva,
        GteCommand::Ncds => CheckedGteOpcode::Ncds,
        GteCommand::Cdp => CheckedGteOpcode::Cdp,
        GteCommand::Ncdt => CheckedGteOpcode::Ncdt,
        GteCommand::Nccs => CheckedGteOpcode::Nccs,
        GteCommand::Cc => CheckedGteOpcode::Cc,
        GteCommand::Ncs => CheckedGteOpcode::Ncs,
        GteCommand::Nct => CheckedGteOpcode::Nct,
        GteCommand::Sqr => CheckedGteOpcode::Sqr,
        GteCommand::Dcpl => CheckedGteOpcode::Dcpl,
        GteCommand::Dpct => CheckedGteOpcode::Dpct,
        GteCommand::Avsz3 => CheckedGteOpcode::Avsz3,
        GteCommand::Avsz4 => CheckedGteOpcode::Avsz4,
        GteCommand::Rtpt => CheckedGteOpcode::Rtpt,
        GteCommand::Gpf => CheckedGteOpcode::Gpf,
        GteCommand::Gpl => CheckedGteOpcode::Gpl,
        GteCommand::Ncct => CheckedGteOpcode::Ncct,
        GteCommand::Unknown(_) => return None,
    })
}

/// 로컬 조사 표현 하나를 공통 checked assembler의 의미 표현으로 바꾼다.
///
/// `None`은 잘라 낼 수 있는 명령이라는 뜻이 아니라 제품 프로필 밖이라는
/// 뜻이다. 호출자는 해당 typed source 전체를 실패시켜야 한다.
pub(super) fn lower_instruction(
    instruction: InstructionKind,
    address: u32,
) -> Option<CheckedInstruction> {
    use CheckedInstruction as C;
    use InstructionKind as K;

    Some(match instruction {
        K::Add { rd, rs, rt } => C::Add {
            rd: register(rd)?,
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::Addu { rd, rs, rt } => C::Addu {
            rd: register(rd)?,
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::Sub { rd, rs, rt } => C::Sub {
            rd: register(rd)?,
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::Subu { rd, rs, rt } => C::Subu {
            rd: register(rd)?,
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::And { rd, rs, rt } => C::And {
            rd: register(rd)?,
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::Or { rd, rs, rt } => C::Or {
            rd: register(rd)?,
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::Xor { rd, rs, rt } => C::Xor {
            rd: register(rd)?,
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::Nor { rd, rs, rt } => C::Nor {
            rd: register(rd)?,
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::Slt { rd, rs, rt } => C::Slt {
            rd: register(rd)?,
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::Sltu { rd, rs, rt } => C::Sltu {
            rd: register(rd)?,
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::Addi { rt, rs, imm } => C::Addi {
            rt: register(rt)?,
            rs: register(rs)?,
            immediate: imm,
        },
        K::Addiu { rt, rs, imm } => C::Addiu {
            rt: register(rt)?,
            rs: register(rs)?,
            immediate: imm,
        },
        K::Slti { rt, rs, imm } => C::Slti {
            rt: register(rt)?,
            rs: register(rs)?,
            immediate: imm,
        },
        K::Sltiu { rt, rs, imm } => C::Sltiu {
            rt: register(rt)?,
            rs: register(rs)?,
            immediate: imm,
        },
        K::Andi { rt, rs, imm } => C::Andi {
            rt: register(rt)?,
            rs: register(rs)?,
            immediate: imm,
        },
        K::Ori { rt, rs, imm } => C::Ori {
            rt: register(rt)?,
            rs: register(rs)?,
            immediate: imm,
        },
        K::Xori { rt, rs, imm } => C::Xori {
            rt: register(rt)?,
            rs: register(rs)?,
            immediate: imm,
        },
        K::Lui { rt, imm } => C::Lui {
            rt: register(rt)?,
            immediate: imm,
        },
        K::Sll { rd, rt, sa } => C::Sll {
            rd: register(rd)?,
            rt: register(rt)?,
            shift: sa,
        },
        K::Srl { rd, rt, sa } => C::Srl {
            rd: register(rd)?,
            rt: register(rt)?,
            shift: sa,
        },
        K::Sra { rd, rt, sa } => C::Sra {
            rd: register(rd)?,
            rt: register(rt)?,
            shift: sa,
        },
        K::Sllv { rd, rt, rs } => C::Sllv {
            rd: register(rd)?,
            rt: register(rt)?,
            rs: register(rs)?,
        },
        K::Srlv { rd, rt, rs } => C::Srlv {
            rd: register(rd)?,
            rt: register(rt)?,
            rs: register(rs)?,
        },
        K::Srav { rd, rt, rs } => C::Srav {
            rd: register(rd)?,
            rt: register(rt)?,
            rs: register(rs)?,
        },
        K::Mult { rs, rt } => C::Mult {
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::Multu { rs, rt } => C::Multu {
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::Div { rs, rt } => C::Div {
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::Divu { rs, rt } => C::Divu {
            rs: register(rs)?,
            rt: register(rt)?,
        },
        K::Mfhi { rd } => C::Mfhi { rd: register(rd)? },
        K::Mthi { rs } => C::Mthi { rs: register(rs)? },
        K::Mflo { rd } => C::Mflo { rd: register(rd)? },
        K::Mtlo { rs } => C::Mtlo { rs: register(rs)? },
        K::Lb { rt, base, offset } => C::Lb {
            rt: register(rt)?,
            base: register(base)?,
            offset,
        },
        K::Lbu { rt, base, offset } => C::Lbu {
            rt: register(rt)?,
            base: register(base)?,
            offset,
        },
        K::Lh { rt, base, offset } => C::Lh {
            rt: register(rt)?,
            base: register(base)?,
            offset,
        },
        K::Lhu { rt, base, offset } => C::Lhu {
            rt: register(rt)?,
            base: register(base)?,
            offset,
        },
        K::Lw { rt, base, offset } => C::Lw {
            rt: register(rt)?,
            base: register(base)?,
            offset,
        },
        K::Lwl { rt, base, offset } => C::Lwl {
            rt: register(rt)?,
            base: register(base)?,
            offset,
        },
        K::Lwr { rt, base, offset } => C::Lwr {
            rt: register(rt)?,
            base: register(base)?,
            offset,
        },
        K::Sb { rt, base, offset } => C::Sb {
            rt: register(rt)?,
            base: register(base)?,
            offset,
        },
        K::Sh { rt, base, offset } => C::Sh {
            rt: register(rt)?,
            base: register(base)?,
            offset,
        },
        K::Sw { rt, base, offset } => C::Sw {
            rt: register(rt)?,
            base: register(base)?,
            offset,
        },
        K::Swl { rt, base, offset } => C::Swl {
            rt: register(rt)?,
            base: register(base)?,
            offset,
        },
        K::Swr { rt, base, offset } => C::Swr {
            rt: register(rt)?,
            base: register(base)?,
            offset,
        },
        K::Beq { rs, rt, offset } => C::Beq {
            rs: register(rs)?,
            rt: register(rt)?,
            target: branch_target(address, offset),
        },
        K::Bne { rs, rt, offset } => C::Bne {
            rs: register(rs)?,
            rt: register(rt)?,
            target: branch_target(address, offset),
        },
        K::Blez { rs, offset } => C::Blez {
            rs: register(rs)?,
            target: branch_target(address, offset),
        },
        K::Bgtz { rs, offset } => C::Bgtz {
            rs: register(rs)?,
            target: branch_target(address, offset),
        },
        K::Bltz { rs, offset } => C::Bltz {
            rs: register(rs)?,
            target: branch_target(address, offset),
        },
        K::Bgez { rs, offset } => C::Bgez {
            rs: register(rs)?,
            target: branch_target(address, offset),
        },
        K::Bltzal { rs, offset } => C::Bltzal {
            rs: register(rs)?,
            target: branch_target(address, offset),
        },
        K::Bgezal { rs, offset } => C::Bgezal {
            rs: register(rs)?,
            target: branch_target(address, offset),
        },
        K::J { target } => C::J { target },
        K::Jal { target } => C::Jal { target },
        K::Jr { rs } => C::Jr { rs: register(rs)? },
        K::Jalr { rd, rs } => C::Jalr {
            rd: register(rd)?,
            rs: register(rs)?,
        },
        K::Syscall { code } => C::Syscall { code },
        K::Break { code } => C::Break { code },
        K::Mfc0 { rt, rd } => C::Mfc0 {
            rt: register(rt)?,
            rd,
        },
        K::Mtc0 { rt, rd } => C::Mtc0 {
            rt: register(rt)?,
            rd,
        },
        K::Rfe => C::Rfe,
        K::Mfc2 { rt, rd } => C::Mfc2 {
            rt: register(rt)?,
            rd,
        },
        K::Mtc2 { rt, rd } => C::Mtc2 {
            rt: register(rt)?,
            rd,
        },
        K::Cfc2 { rt, rd } => C::Cfc2 {
            rt: register(rt)?,
            rd,
        },
        K::Ctc2 { rt, rd } => C::Ctc2 {
            rt: register(rt)?,
            rd,
        },
        K::Lwc2 { rt, base, offset } => C::Lwc2 {
            rt,
            base: register(base)?,
            offset,
        },
        K::Swc2 { rt, base, offset } => C::Swc2 {
            rt,
            base: register(base)?,
            offset,
        },
        K::Gte { command, raw } => {
            let expected_opcode = checked_gte_opcode(command)?;
            match psx_r3000a::decode_word(raw, address).ok()? {
                C::Gte(decoded) if decoded.opcode == expected_opcode => C::Gte(decoded),
                _ => return None,
            }
        }
        K::Unknown { .. } => return None,
    })
}
