//! 디스어셈블리 표시.
//!
//! 의사명령 별칭은 **여기서만** 만든다. `nop`은 `sll $zero,$zero,0`의
//! 표시이고 실체가 아니다. [`super::encode`]는 별칭을 모르므로 재조립
//! 동일성이 표시 규칙에 영향받지 않는다.

use std::fmt;

use super::instruction::{Instruction, InstructionKind, Reg};

/// 명령 하나를 어셈블리 표기로 쓴다.
pub struct Asm(pub InstructionKind);

impl fmt::Display for Asm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use InstructionKind as K;
        match self.0 {
            // 별칭: 표시 전용
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            } => f.write_str("nop"),
            K::Addu {
                rd,
                rs,
                rt: Reg::ZERO,
            } => write!(f, "move    {rd}, {rs}"),
            K::Or {
                rd,
                rs,
                rt: Reg::ZERO,
            } => write!(f, "move    {rd}, {rs}"),

            K::Add { rd, rs, rt } => write!(f, "add     {rd}, {rs}, {rt}"),
            K::Addu { rd, rs, rt } => write!(f, "addu    {rd}, {rs}, {rt}"),
            K::Sub { rd, rs, rt } => write!(f, "sub     {rd}, {rs}, {rt}"),
            K::Subu { rd, rs, rt } => write!(f, "subu    {rd}, {rs}, {rt}"),
            K::And { rd, rs, rt } => write!(f, "and     {rd}, {rs}, {rt}"),
            K::Or { rd, rs, rt } => write!(f, "or      {rd}, {rs}, {rt}"),
            K::Xor { rd, rs, rt } => write!(f, "xor     {rd}, {rs}, {rt}"),
            K::Nor { rd, rs, rt } => write!(f, "nor     {rd}, {rs}, {rt}"),
            K::Slt { rd, rs, rt } => write!(f, "slt     {rd}, {rs}, {rt}"),
            K::Sltu { rd, rs, rt } => write!(f, "sltu    {rd}, {rs}, {rt}"),

            K::Addi { rt, rs, imm } => write!(f, "addi    {rt}, {rs}, {imm}"),
            K::Addiu { rt, rs, imm } => write!(f, "addiu   {rt}, {rs}, {imm}"),
            K::Slti { rt, rs, imm } => write!(f, "slti    {rt}, {rs}, {imm}"),
            K::Sltiu { rt, rs, imm } => write!(f, "sltiu   {rt}, {rs}, {imm}"),
            K::Andi { rt, rs, imm } => write!(f, "andi    {rt}, {rs}, 0x{imm:X}"),
            K::Ori { rt, rs, imm } => write!(f, "ori     {rt}, {rs}, 0x{imm:X}"),
            K::Xori { rt, rs, imm } => write!(f, "xori    {rt}, {rs}, 0x{imm:X}"),
            K::Lui { rt, imm } => write!(f, "lui     {rt}, 0x{imm:X}"),

            K::Sll { rd, rt, sa } => write!(f, "sll     {rd}, {rt}, {sa}"),
            K::Srl { rd, rt, sa } => write!(f, "srl     {rd}, {rt}, {sa}"),
            K::Sra { rd, rt, sa } => write!(f, "sra     {rd}, {rt}, {sa}"),
            K::Sllv { rd, rt, rs } => write!(f, "sllv    {rd}, {rt}, {rs}"),
            K::Srlv { rd, rt, rs } => write!(f, "srlv    {rd}, {rt}, {rs}"),
            K::Srav { rd, rt, rs } => write!(f, "srav    {rd}, {rt}, {rs}"),

            K::Mult { rs, rt } => write!(f, "mult    {rs}, {rt}"),
            K::Multu { rs, rt } => write!(f, "multu   {rs}, {rt}"),
            K::Div { rs, rt } => write!(f, "div     {rs}, {rt}"),
            K::Divu { rs, rt } => write!(f, "divu    {rs}, {rt}"),
            K::Mfhi { rd } => write!(f, "mfhi    {rd}"),
            K::Mthi { rs } => write!(f, "mthi    {rs}"),
            K::Mflo { rd } => write!(f, "mflo    {rd}"),
            K::Mtlo { rs } => write!(f, "mtlo    {rs}"),

            K::Lb { rt, base, offset } => write!(f, "lb      {rt}, {offset}({base})"),
            K::Lbu { rt, base, offset } => write!(f, "lbu     {rt}, {offset}({base})"),
            K::Lh { rt, base, offset } => write!(f, "lh      {rt}, {offset}({base})"),
            K::Lhu { rt, base, offset } => write!(f, "lhu     {rt}, {offset}({base})"),
            K::Lw { rt, base, offset } => write!(f, "lw      {rt}, {offset}({base})"),
            K::Lwl { rt, base, offset } => write!(f, "lwl     {rt}, {offset}({base})"),
            K::Lwr { rt, base, offset } => write!(f, "lwr     {rt}, {offset}({base})"),

            K::Sb { rt, base, offset } => write!(f, "sb      {rt}, {offset}({base})"),
            K::Sh { rt, base, offset } => write!(f, "sh      {rt}, {offset}({base})"),
            K::Sw { rt, base, offset } => write!(f, "sw      {rt}, {offset}({base})"),
            K::Swl { rt, base, offset } => write!(f, "swl     {rt}, {offset}({base})"),
            K::Swr { rt, base, offset } => write!(f, "swr     {rt}, {offset}({base})"),

            K::Beq { rs, rt, offset } => write!(f, "beq     {rs}, {rt}, {offset}"),
            K::Bne { rs, rt, offset } => write!(f, "bne     {rs}, {rt}, {offset}"),
            K::Blez { rs, offset } => write!(f, "blez    {rs}, {offset}"),
            K::Bgtz { rs, offset } => write!(f, "bgtz    {rs}, {offset}"),
            K::Bltz { rs, offset } => write!(f, "bltz    {rs}, {offset}"),
            K::Bgez { rs, offset } => write!(f, "bgez    {rs}, {offset}"),
            K::Bltzal { rs, offset } => write!(f, "bltzal  {rs}, {offset}"),
            K::Bgezal { rs, offset } => write!(f, "bgezal  {rs}, {offset}"),

            K::J { target } => write!(f, "j       0x{target:08X}"),
            K::Jal { target } => write!(f, "jal     0x{target:08X}"),
            K::Jr { rs } => write!(f, "jr      {rs}"),
            K::Jalr { rd: Reg::RA, rs } => write!(f, "jalr    {rs}"),
            K::Jalr { rd, rs } => write!(f, "jalr    {rd}, {rs}"),

            K::Syscall { code } => write!(f, "syscall 0x{code:X}"),
            K::Break { code } => write!(f, "break   0x{code:X}"),

            K::Mfc0 { rt, rd } => write!(f, "mfc0    {rt}, $c0_{rd}"),
            K::Mtc0 { rt, rd } => write!(f, "mtc0    {rt}, $c0_{rd}"),
            K::Rfe => f.write_str("rfe"),

            K::Mfc2 { rt, rd } => write!(f, "mfc2    {rt}, $c2_{rd}"),
            K::Mtc2 { rt, rd } => write!(f, "mtc2    {rt}, $c2_{rd}"),
            K::Cfc2 { rt, rd } => write!(f, "cfc2    {rt}, $c2c_{rd}"),
            K::Ctc2 { rt, rd } => write!(f, "ctc2    {rt}, $c2c_{rd}"),

            K::Lwc2 { rt, base, offset } => write!(f, "lwc2    $c2_{rt}, {offset}({base})"),
            K::Swc2 { rt, base, offset } => write!(f, "swc2    $c2_{rt}, {offset}({base})"),

            K::Gte { command, raw } => write!(f, "{:<8}; 0x{raw:08X}", command.name()),
            K::Unknown { word } => write!(f, ".word   0x{word:08X}"),
        }
    }
}

/// `주소  워드  어셈블리` 한 줄.
pub fn listing_line(ins: &Instruction) -> String {
    format!("{:08X}  {:08X}  {}", ins.addr, ins.word, Asm(ins.kind))
}

#[cfg(test)]
#[path = "display_tests.rs"]
mod display_tests;
