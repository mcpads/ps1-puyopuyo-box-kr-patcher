//! MIPS R3000A (PlayStation) 명령 표현.
//!
//! 선언 프로필은 [`super::profile`]가 소유한다. 이 모듈은 **실체 명령만**
//! 담는다. `nop`·`move`·`li`·`b` 같은 의사명령은 표시 계층의 별칭이며
//! 여기에 두지 않는다. 별칭을 실체로 만들면 재조립 동일성이 깨진다.

use std::fmt;

// ── 레지스터 ────────────────────────────────────────────────────

/// MIPS 범용 레지스터 (0–31).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reg(pub u8);

impl Reg {
    pub const ZERO: Reg = Reg(0);
    pub const AT: Reg = Reg(1);
    pub const V0: Reg = Reg(2);
    pub const V1: Reg = Reg(3);
    pub const A0: Reg = Reg(4);
    pub const A1: Reg = Reg(5);
    pub const A2: Reg = Reg(6);
    pub const A3: Reg = Reg(7);
    pub const T0: Reg = Reg(8);
    pub const T1: Reg = Reg(9);
    pub const T2: Reg = Reg(10);
    pub const T3: Reg = Reg(11);
    pub const T4: Reg = Reg(12);
    pub const T5: Reg = Reg(13);
    pub const T6: Reg = Reg(14);
    pub const T7: Reg = Reg(15);
    pub const S0: Reg = Reg(16);
    pub const S1: Reg = Reg(17);
    pub const S2: Reg = Reg(18);
    pub const S3: Reg = Reg(19);
    pub const S4: Reg = Reg(20);
    pub const S5: Reg = Reg(21);
    pub const S6: Reg = Reg(22);
    pub const S7: Reg = Reg(23);
    pub const T8: Reg = Reg(24);
    pub const T9: Reg = Reg(25);
    pub const K0: Reg = Reg(26);
    pub const K1: Reg = Reg(27);
    pub const GP: Reg = Reg(28);
    pub const SP: Reg = Reg(29);
    pub const FP: Reg = Reg(30);
    pub const RA: Reg = Reg(31);

    pub fn name(self) -> &'static str {
        const NAMES: [&str; 32] = [
            "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5",
            "t6", "t7", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1",
            "gp", "sp", "fp", "ra",
        ];
        NAMES[(self.0 & 0x1F) as usize]
    }
}

impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.name())
    }
}

// ── GTE (COP2) 커맨드 ───────────────────────────────────────────

/// GTE 커맨드. funct 비트(5:0)로 식별한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GteCommand {
    Rtps,
    Nclip,
    Op,
    Dpcs,
    Intpl,
    Mvmva,
    Ncds,
    Cdp,
    Ncdt,
    Nccs,
    Cc,
    Ncs,
    Nct,
    Sqr,
    Dcpl,
    Dpct,
    Avsz3,
    Avsz4,
    Rtpt,
    Gpf,
    Gpl,
    Ncct,
    /// 프로필에 없는 funct. 검산에서 거부 대상이다.
    Unknown(u8),
}

impl GteCommand {
    pub fn from_funct(funct: u8) -> Self {
        match funct & 0x3F {
            0x01 => Self::Rtps,
            0x06 => Self::Nclip,
            0x0C => Self::Op,
            0x10 => Self::Dpcs,
            0x11 => Self::Intpl,
            0x12 => Self::Mvmva,
            0x13 => Self::Ncds,
            0x14 => Self::Cdp,
            0x16 => Self::Ncdt,
            0x1B => Self::Nccs,
            0x1C => Self::Cc,
            0x1E => Self::Ncs,
            0x20 => Self::Nct,
            0x28 => Self::Sqr,
            0x29 => Self::Dcpl,
            0x2A => Self::Dpct,
            0x2D => Self::Avsz3,
            0x2E => Self::Avsz4,
            0x30 => Self::Rtpt,
            0x3D => Self::Gpf,
            0x3E => Self::Gpl,
            0x3F => Self::Ncct,
            other => Self::Unknown(other),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Rtps => "rtps",
            Self::Nclip => "nclip",
            Self::Op => "op",
            Self::Dpcs => "dpcs",
            Self::Intpl => "intpl",
            Self::Mvmva => "mvmva",
            Self::Ncds => "ncds",
            Self::Cdp => "cdp",
            Self::Ncdt => "ncdt",
            Self::Nccs => "nccs",
            Self::Cc => "cc",
            Self::Ncs => "ncs",
            Self::Nct => "nct",
            Self::Sqr => "sqr",
            Self::Dcpl => "dcpl",
            Self::Dpct => "dpct",
            Self::Avsz3 => "avsz3",
            Self::Avsz4 => "avsz4",
            Self::Rtpt => "rtpt",
            Self::Gpf => "gpf",
            Self::Gpl => "gpl",
            Self::Ncct => "ncct",
            Self::Unknown(_) => "cop2",
        }
    }

    pub fn is_known(self) -> bool {
        !matches!(self, Self::Unknown(_))
    }
}

// ── 명령 종류 ───────────────────────────────────────────────────

/// R3000A 실체 명령.
///
/// GTE 커맨드는 인자 비트가 커맨드마다 다르므로 원본 워드를 함께 보존해
/// 재조립 동일성을 보장한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionKind {
    // 산술·논리 R-type
    Add {
        rd: Reg,
        rs: Reg,
        rt: Reg,
    },
    Addu {
        rd: Reg,
        rs: Reg,
        rt: Reg,
    },
    Sub {
        rd: Reg,
        rs: Reg,
        rt: Reg,
    },
    Subu {
        rd: Reg,
        rs: Reg,
        rt: Reg,
    },
    And {
        rd: Reg,
        rs: Reg,
        rt: Reg,
    },
    Or {
        rd: Reg,
        rs: Reg,
        rt: Reg,
    },
    Xor {
        rd: Reg,
        rs: Reg,
        rt: Reg,
    },
    Nor {
        rd: Reg,
        rs: Reg,
        rt: Reg,
    },
    Slt {
        rd: Reg,
        rs: Reg,
        rt: Reg,
    },
    Sltu {
        rd: Reg,
        rs: Reg,
        rt: Reg,
    },

    // 산술·논리 I-type
    Addi {
        rt: Reg,
        rs: Reg,
        imm: i16,
    },
    Addiu {
        rt: Reg,
        rs: Reg,
        imm: i16,
    },
    Slti {
        rt: Reg,
        rs: Reg,
        imm: i16,
    },
    Sltiu {
        rt: Reg,
        rs: Reg,
        imm: i16,
    },
    Andi {
        rt: Reg,
        rs: Reg,
        imm: u16,
    },
    Ori {
        rt: Reg,
        rs: Reg,
        imm: u16,
    },
    Xori {
        rt: Reg,
        rs: Reg,
        imm: u16,
    },
    Lui {
        rt: Reg,
        imm: u16,
    },

    // 시프트
    Sll {
        rd: Reg,
        rt: Reg,
        sa: u8,
    },
    Srl {
        rd: Reg,
        rt: Reg,
        sa: u8,
    },
    Sra {
        rd: Reg,
        rt: Reg,
        sa: u8,
    },
    Sllv {
        rd: Reg,
        rt: Reg,
        rs: Reg,
    },
    Srlv {
        rd: Reg,
        rt: Reg,
        rs: Reg,
    },
    Srav {
        rd: Reg,
        rt: Reg,
        rs: Reg,
    },

    // 곱셈·나눗셈 + HI/LO
    Mult {
        rs: Reg,
        rt: Reg,
    },
    Multu {
        rs: Reg,
        rt: Reg,
    },
    Div {
        rs: Reg,
        rt: Reg,
    },
    Divu {
        rs: Reg,
        rt: Reg,
    },
    Mfhi {
        rd: Reg,
    },
    Mthi {
        rs: Reg,
    },
    Mflo {
        rd: Reg,
    },
    Mtlo {
        rs: Reg,
    },

    // 로드
    Lb {
        rt: Reg,
        base: Reg,
        offset: i16,
    },
    Lbu {
        rt: Reg,
        base: Reg,
        offset: i16,
    },
    Lh {
        rt: Reg,
        base: Reg,
        offset: i16,
    },
    Lhu {
        rt: Reg,
        base: Reg,
        offset: i16,
    },
    Lw {
        rt: Reg,
        base: Reg,
        offset: i16,
    },
    Lwl {
        rt: Reg,
        base: Reg,
        offset: i16,
    },
    Lwr {
        rt: Reg,
        base: Reg,
        offset: i16,
    },

    // 스토어
    Sb {
        rt: Reg,
        base: Reg,
        offset: i16,
    },
    Sh {
        rt: Reg,
        base: Reg,
        offset: i16,
    },
    Sw {
        rt: Reg,
        base: Reg,
        offset: i16,
    },
    Swl {
        rt: Reg,
        base: Reg,
        offset: i16,
    },
    Swr {
        rt: Reg,
        base: Reg,
        offset: i16,
    },

    // 분기
    Beq {
        rs: Reg,
        rt: Reg,
        offset: i16,
    },
    Bne {
        rs: Reg,
        rt: Reg,
        offset: i16,
    },
    Blez {
        rs: Reg,
        offset: i16,
    },
    Bgtz {
        rs: Reg,
        offset: i16,
    },
    Bltz {
        rs: Reg,
        offset: i16,
    },
    Bgez {
        rs: Reg,
        offset: i16,
    },
    Bltzal {
        rs: Reg,
        offset: i16,
    },
    Bgezal {
        rs: Reg,
        offset: i16,
    },

    // 점프
    J {
        target: u32,
    },
    Jal {
        target: u32,
    },
    Jr {
        rs: Reg,
    },
    Jalr {
        rd: Reg,
        rs: Reg,
    },

    // 시스템
    Syscall {
        code: u32,
    },
    Break {
        code: u32,
    },

    // COP0
    Mfc0 {
        rt: Reg,
        rd: u8,
    },
    Mtc0 {
        rt: Reg,
        rd: u8,
    },
    Rfe,

    // COP2 레지스터 전송
    Mfc2 {
        rt: Reg,
        rd: u8,
    },
    Mtc2 {
        rt: Reg,
        rd: u8,
    },
    Cfc2 {
        rt: Reg,
        rd: u8,
    },
    Ctc2 {
        rt: Reg,
        rd: u8,
    },

    // COP2 메모리 접근
    Lwc2 {
        rt: u8,
        base: Reg,
        offset: i16,
    },
    Swc2 {
        rt: u8,
        base: Reg,
        offset: i16,
    },

    /// GTE 커맨드. `raw`는 인자 비트를 포함한 원본 워드.
    Gte {
        command: GteCommand,
        raw: u32,
    },

    /// 선언 프로필로 해석되지 않은 워드.
    ///
    /// 조사 경로에서는 허용하지만 기계어 검산에서는 **거부**한다.
    Unknown {
        word: u32,
    },
}

impl InstructionKind {
    /// 선언 프로필로 해석된 실체 명령인가.
    pub fn is_decoded(&self) -> bool {
        match self {
            Self::Unknown { .. } => false,
            Self::Gte { command, .. } => command.is_known(),
            _ => true,
        }
    }
}

// ── 제어 흐름 ───────────────────────────────────────────────────

/// 명령의 제어 흐름 분류. 훅 이동 검산과 재귀 디스어셈블에 쓴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowKind {
    /// 다음 명령으로 진행.
    Sequential,
    /// 무조건 분기. 딜레이 슬롯 있음.
    Jump { target: u32 },
    /// 서브루틴 호출. 딜레이 슬롯 있음.
    Call { target: u32 },
    /// 조건 분기. 딜레이 슬롯 있음.
    Branch { taken: u32 },
    /// 레지스터 간접 분기. 타깃 미상.
    JumpRegister { rs: Reg },
    /// `jr $ra`.
    Return,
    /// 예외 유발.
    Trap,
}

impl FlowKind {
    /// 이 명령 뒤에 딜레이 슬롯이 따르는가.
    ///
    /// 훅으로 명령을 옮길 때 딜레이 슬롯을 같이 옮기지 않으면 실행 의미가
    /// 바뀐다.
    pub fn has_delay_slot(self) -> bool {
        !matches!(self, Self::Sequential | Self::Trap)
    }
}

// ── 디코딩된 명령 ───────────────────────────────────────────────

/// 주소·원본 워드·해석 결과를 함께 보존한 단일 명령.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    /// 이 명령이 놓인 런타임 주소.
    pub addr: u32,
    /// 원본 워드 (리틀엔디언 해석 결과).
    pub word: u32,
    pub kind: InstructionKind,
}

impl Instruction {
    pub fn new(addr: u32, word: u32, kind: InstructionKind) -> Self {
        Self { addr, word, kind }
    }

    /// 제어 흐름 분류.
    pub fn flow_kind(&self) -> FlowKind {
        let next = self.addr.wrapping_add(4);
        let rel = |offset: i16| next.wrapping_add((offset as i32 as u32) << 2);
        match self.kind {
            InstructionKind::J { target } => FlowKind::Jump { target },
            InstructionKind::Jal { target } => FlowKind::Call { target },
            InstructionKind::Jr { rs } => {
                if rs == Reg::RA {
                    FlowKind::Return
                } else {
                    FlowKind::JumpRegister { rs }
                }
            }
            InstructionKind::Jalr { rs, .. } => FlowKind::JumpRegister { rs },
            InstructionKind::Beq { offset, .. }
            | InstructionKind::Bne { offset, .. }
            | InstructionKind::Blez { offset, .. }
            | InstructionKind::Bgtz { offset, .. }
            | InstructionKind::Bltz { offset, .. }
            | InstructionKind::Bgez { offset, .. }
            | InstructionKind::Bltzal { offset, .. }
            | InstructionKind::Bgezal { offset, .. } => FlowKind::Branch { taken: rel(offset) },
            InstructionKind::Syscall { .. } | InstructionKind::Break { .. } => FlowKind::Trap,
            _ => FlowKind::Sequential,
        }
    }

    /// 이 명령이 쓰는 목적 레지스터. 로드 해저드 검사에 쓴다.
    pub fn dest_reg(&self) -> Option<Reg> {
        use InstructionKind as K;
        let rd = match self.kind {
            K::Add { rd, .. }
            | K::Addu { rd, .. }
            | K::Sub { rd, .. }
            | K::Subu { rd, .. }
            | K::And { rd, .. }
            | K::Or { rd, .. }
            | K::Xor { rd, .. }
            | K::Nor { rd, .. }
            | K::Slt { rd, .. }
            | K::Sltu { rd, .. }
            | K::Sll { rd, .. }
            | K::Srl { rd, .. }
            | K::Sra { rd, .. }
            | K::Sllv { rd, .. }
            | K::Srlv { rd, .. }
            | K::Srav { rd, .. }
            | K::Mfhi { rd }
            | K::Mflo { rd }
            | K::Jalr { rd, .. } => rd,

            K::Addi { rt, .. }
            | K::Addiu { rt, .. }
            | K::Slti { rt, .. }
            | K::Sltiu { rt, .. }
            | K::Andi { rt, .. }
            | K::Ori { rt, .. }
            | K::Xori { rt, .. }
            | K::Lui { rt, .. }
            | K::Lb { rt, .. }
            | K::Lbu { rt, .. }
            | K::Lh { rt, .. }
            | K::Lhu { rt, .. }
            | K::Lw { rt, .. }
            | K::Lwl { rt, .. }
            | K::Lwr { rt, .. }
            | K::Mfc0 { rt, .. }
            | K::Mfc2 { rt, .. }
            | K::Cfc2 { rt, .. } => rt,

            _ => return None,
        };
        (rd != Reg::ZERO).then_some(rd)
    }

    /// 명령이 지정한 GPR의 현재 값을 읽는가.
    ///
    /// `lwl`/`lwr`는 메모리 값과 기존 `rt`를 합치므로 `rt`도 읽는 것으로
    /// 판정한다. 선언 프로필 밖의 word는 어떤 register든 읽을 수 있는 것으로
    /// 보아 provenance 분석이 조용히 통과하지 못하게 한다.
    pub fn reads_reg(&self, register: Reg) -> bool {
        use InstructionKind as K;
        match self.kind {
            K::Add { rs, rt, .. }
            | K::Addu { rs, rt, .. }
            | K::Sub { rs, rt, .. }
            | K::Subu { rs, rt, .. }
            | K::And { rs, rt, .. }
            | K::Or { rs, rt, .. }
            | K::Xor { rs, rt, .. }
            | K::Nor { rs, rt, .. }
            | K::Slt { rs, rt, .. }
            | K::Sltu { rs, rt, .. }
            | K::Sllv { rs, rt, .. }
            | K::Srlv { rs, rt, .. }
            | K::Srav { rs, rt, .. }
            | K::Mult { rs, rt }
            | K::Multu { rs, rt }
            | K::Div { rs, rt }
            | K::Divu { rs, rt }
            | K::Beq { rs, rt, .. }
            | K::Bne { rs, rt, .. } => rs == register || rt == register,
            K::Addi { rs, .. }
            | K::Addiu { rs, .. }
            | K::Slti { rs, .. }
            | K::Sltiu { rs, .. }
            | K::Andi { rs, .. }
            | K::Ori { rs, .. }
            | K::Xori { rs, .. }
            | K::Blez { rs, .. }
            | K::Bgtz { rs, .. }
            | K::Bltz { rs, .. }
            | K::Bgez { rs, .. }
            | K::Bltzal { rs, .. }
            | K::Bgezal { rs, .. }
            | K::Jr { rs }
            | K::Jalr { rs, .. }
            | K::Mthi { rs }
            | K::Mtlo { rs } => rs == register,
            K::Sll { rt, .. } | K::Srl { rt, .. } | K::Sra { rt, .. } => rt == register,
            K::Lb { base, .. }
            | K::Lbu { base, .. }
            | K::Lh { base, .. }
            | K::Lhu { base, .. }
            | K::Lw { base, .. }
            | K::Lwc2 { base, .. } => base == register,
            K::Lwl { rt, base, .. } | K::Lwr { rt, base, .. } => base == register || rt == register,
            K::Sb { rt, base, .. }
            | K::Sh { rt, base, .. }
            | K::Sw { rt, base, .. }
            | K::Swl { rt, base, .. }
            | K::Swr { rt, base, .. } => base == register || rt == register,
            K::Mtc0 { rt, .. } | K::Mtc2 { rt, .. } | K::Ctc2 { rt, .. } => rt == register,
            K::Swc2 { base, .. } => base == register,
            K::Unknown { .. } => true,
            K::Lui { .. }
            | K::Mfhi { .. }
            | K::Mflo { .. }
            | K::J { .. }
            | K::Jal { .. }
            | K::Syscall { .. }
            | K::Break { .. }
            | K::Mfc0 { .. }
            | K::Rfe
            | K::Mfc2 { .. }
            | K::Cfc2 { .. }
            | K::Gte { .. } => false,
        }
    }

    /// 로드 명령인가. R3000A의 로드 딜레이 판정에 쓴다.
    pub fn is_load(&self) -> bool {
        use InstructionKind as K;
        matches!(
            self.kind,
            K::Lb { .. }
                | K::Lbu { .. }
                | K::Lh { .. }
                | K::Lhu { .. }
                | K::Lw { .. }
                | K::Lwl { .. }
                | K::Lwr { .. }
                | K::Lwc2 { .. }
        )
    }
}

#[cfg(test)]
#[path = "instruction_tests.rs"]
mod instruction_tests;
