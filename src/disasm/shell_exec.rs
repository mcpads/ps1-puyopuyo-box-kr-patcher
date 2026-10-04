//! 셸의 서브게임 PS-X EXE 적재·실행 경로.
//!
//! selector jump table, 경로 문자열, ISO 검색, PS-X EXE header의
//! `t_addr`/`t_size` 소비와 BIOS A(0x43) `Exec` 호출을 typed MIPS
//! 명령으로 함께 검증한다. 런타임 도달성은 별도 실행 증거가 소유한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;

const PATH_TABLE_RUNTIME: u32 = 0x8012_0000;
const JUMP_TABLE_RUNTIME: u32 = 0x8012_0160;
const SELECTOR_FIRST: u8 = 10;
const SELECTOR_LAST: u8 = 80;
const DEFAULT_BLOCK: u32 = 0x8012_4770;
const OPEN_FUNCTION: u32 = 0x8012_4110;
const LOAD_STATE_FUNCTION: u32 = 0x8012_466C;
const LOAD_STATE_CALLER: u32 = 0x8013_3B40;
const EXEC_TRANSITION_FUNCTION: u32 = 0x8013_3B7C;
const EXEC_LOADER_FUNCTION: u32 = 0x8012_5AA4;
const EXEC_STUB: u32 = 0x8014_2E14;

/// selector와 shell-relative 경로·jump block의 정확한 대응.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellExecPathSpec {
    pub selector: u8,
    pub path_offset: u16,
    pub path: &'static str,
    pub block: u32,
}

pub const SHELL_EXEC_PATHS: [ShellExecPathSpec; 10] = [
    ShellExecPathSpec {
        selector: 10,
        path_offset: 0x00,
        path: r"PUYO1\PUYO1.EXE",
        block: 0x8012_46F0,
    },
    ShellExecPathSpec {
        selector: 11,
        path_offset: 0x10,
        path: r"PUYO2\PUYO2.EXE",
        block: 0x8012_46FC,
    },
    ShellExecPathSpec {
        selector: 12,
        path_offset: 0x20,
        path: r"KOUSIKI\KOUSIKI.EXE",
        block: 0x8012_4708,
    },
    ShellExecPathSpec {
        selector: 30,
        path_offset: 0x34,
        path: r"GOCHA1\GOCHA1.EXE",
        block: 0x8012_4714,
    },
    ShellExecPathSpec {
        selector: 31,
        path_offset: 0x48,
        path: r"GOCHA2\GOCHA2.EXE",
        block: 0x8012_4720,
    },
    ShellExecPathSpec {
        selector: 32,
        path_offset: 0x5C,
        path: r"MINPUYO\MINPUYO.EXE",
        block: 0x8012_472C,
    },
    ShellExecPathSpec {
        selector: 40,
        path_offset: 0x70,
        path: r"HORI2\HORI2.EXE",
        block: 0x8012_4738,
    },
    ShellExecPathSpec {
        selector: 41,
        path_offset: 0x80,
        path: r"HORI4\HORI4.EXE",
        block: 0x8012_4744,
    },
    ShellExecPathSpec {
        selector: 70,
        path_offset: 0x90,
        path: r"QUEST\QUEST.EXE",
        block: 0x8012_4750,
    },
    ShellExecPathSpec {
        selector: 80,
        path_offset: 0xA0,
        path: r"PCOMI\PCOMI.EXE",
        block: 0x8012_475C,
    },
];

/// 셸 EXE loader 검증 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellExecSummary {
    pub selectors: usize,
    pub default_selectors: usize,
    pub quest_selector: u8,
    pub quest_path_runtime: u32,
    pub open_function: u32,
    pub load_state_function: u32,
    pub exec_loader_function: u32,
    pub bios_exec_function: u8,
}

/// 셸 EXE loader 정적 계약 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellExecError {
    WrongExecutable(&'static str),
    UnalignedText(usize),
    RangeOutOfBounds {
        address: u32,
        len: usize,
    },
    PathMismatch {
        selector: u8,
        expected: String,
        actual: String,
    },
    JumpTableMismatch {
        selector: u8,
        expected: u32,
        actual: u32,
    },
    MissingInstruction(u32),
    InstructionMismatch {
        address: u32,
        expected: &'static str,
        actual: K,
    },
}

impl fmt::Display for ShellExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(actual) => {
                write!(f, "셸 EXE loader에 다른 EXE 사상을 사용: {actual}")
            }
            Self::UnalignedText(actual) => {
                write!(f, "셸 text 길이가 4바이트 정렬이 아님: {actual}")
            }
            Self::RangeOutOfBounds { address, len } => write!(
                f,
                "셸 EXE loader 검증 범위 이탈: address=0x{address:08X}, len={len}"
            ),
            Self::PathMismatch {
                selector,
                expected,
                actual,
            } => write!(
                f,
                "셸 selector {selector} 경로 불일치: expected={expected:?}, actual={actual:?}"
            ),
            Self::JumpTableMismatch {
                selector,
                expected,
                actual,
            } => write!(
                f,
                "셸 selector {selector} jump block 불일치: \
                 expected=0x{expected:08X}, actual=0x{actual:08X}"
            ),
            Self::MissingInstruction(address) => {
                write!(f, "셸 EXE loader 명령이 없음: 0x{address:08X}")
            }
            Self::InstructionMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "셸 EXE loader 명령 불일치: 0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
        }
    }
}

impl Error for ShellExecError {}

/// 지원 원본 셸의 selector → ISO read → BIOS Exec 정적 경로를 검증한다.
pub fn verify_box_shell_exec_path(
    map: &ExeMap,
    text: &[u8],
) -> Result<ShellExecSummary, ShellExecError> {
    if map.name != "SLPS_031.14" {
        return Err(ShellExecError::WrongExecutable(map.name));
    }
    verify_paths(map, text)?;
    let default_selectors = verify_jump_table(map, text)?;
    let instructions = linear::sweep(map, text).ok_or(ShellExecError::UnalignedText(text.len()))?;

    require(
        &instructions,
        0x8012_46BC,
        "lw v0,0x98(s0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: 0x98,
        },
    )?;
    require(
        &instructions,
        0x8012_46C4,
        "addiu v1,v0,-10",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V0,
            imm: -10,
        },
    )?;
    require(
        &instructions,
        0x8012_46C8,
        "sltiu v0,v1,71",
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 71,
        },
    )?;
    require(
        &instructions,
        0x8012_46D4,
        "addiu v0,v0,0x160",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x160,
        },
    )?;
    require(&instructions, 0x8012_46E8, "jr v0", K::Jr { rs: Reg::V0 })?;

    let quest = SHELL_EXEC_PATHS
        .iter()
        .find(|spec| spec.selector == 70)
        .expect("고정 QUEST selector");
    require(
        &instructions,
        quest.block,
        "lui a0,0x8012",
        K::Lui {
            rt: Reg::A0,
            imm: 0x8012,
        },
    )?;
    require(
        &instructions,
        quest.block + 4,
        "j common open block",
        K::J {
            target: 0x8012_4764,
        },
    )?;
    require(
        &instructions,
        quest.block + 8,
        "addiu a0,a0,0x90",
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: quest.path_offset as i16,
        },
    )?;
    require(
        &instructions,
        0x8012_4764,
        "jal path open",
        K::Jal {
            target: OPEN_FUNCTION,
        },
    )?;

    // `CdSearchFile` 계열 함수에 조립한 path를 넘기는 call.
    require(
        &instructions,
        0x8012_41C4,
        "jal ISO path search",
        K::Jal {
            target: 0x8014_B994,
        },
    )?;

    // 읽은 PS-X EXE header에서 text destination과 size를 직접 소비한다.
    require(
        &instructions,
        0x8012_488C,
        "lw a2,0x18(v0) PS-X EXE t_addr",
        K::Lw {
            rt: Reg::A2,
            base: Reg::V0,
            offset: 0x18,
        },
    )?;
    require(
        &instructions,
        0x8012_4894,
        "lw a3,0x1C(v0) PS-X EXE t_size",
        K::Lw {
            rt: Reg::A3,
            base: Reg::V0,
            offset: 0x1C,
        },
    )?;
    require(
        &instructions,
        LOAD_STATE_CALLER,
        "jal EXE load state",
        K::Jal {
            target: LOAD_STATE_FUNCTION,
        },
    )?;
    require(
        &instructions,
        EXEC_TRANSITION_FUNCTION + 8,
        "jal loaded EXE transition",
        K::Jal {
            target: EXEC_LOADER_FUNCTION,
        },
    )?;

    // header의 stack을 셸 계약으로 덮고 EXEC 구조(+0x10)를 A(0x43)에 넘긴다.
    require(
        &instructions,
        0x8012_5AB8,
        "lw v0,0x110(gp) loaded PS-X EXE header",
        K::Lw {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 0x110,
        },
    )?;
    require(
        &instructions,
        0x8012_5AC0,
        "sw 0x801FFFF0,0x30(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x30,
        },
    )?;
    require(
        &instructions,
        0x8012_5AD8,
        "jal BIOS Exec stub",
        K::Jal { target: EXEC_STUB },
    )?;
    require(
        &instructions,
        0x8012_5ADC,
        "addiu a0,a0,0x10 EXEC structure",
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 0x10,
        },
    )?;
    require(
        &instructions,
        EXEC_STUB,
        "addiu t2,zero,0xA0",
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::ZERO,
            imm: 0xA0,
        },
    )?;
    require(&instructions, EXEC_STUB + 4, "jr t2", K::Jr { rs: Reg::T2 })?;
    require(
        &instructions,
        EXEC_STUB + 8,
        "addiu t1,zero,0x43",
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: 0x43,
        },
    )?;

    Ok(ShellExecSummary {
        selectors: SHELL_EXEC_PATHS.len(),
        default_selectors,
        quest_selector: quest.selector,
        quest_path_runtime: PATH_TABLE_RUNTIME + u32::from(quest.path_offset),
        open_function: OPEN_FUNCTION,
        load_state_function: LOAD_STATE_FUNCTION,
        exec_loader_function: EXEC_LOADER_FUNCTION,
        bios_exec_function: 0x43,
    })
}

fn verify_paths(map: &ExeMap, text: &[u8]) -> Result<(), ShellExecError> {
    for spec in SHELL_EXEC_PATHS {
        let address = PATH_TABLE_RUNTIME + u32::from(spec.path_offset);
        let raw = runtime_slice(map, text, address, spec.path.len() + 1)?;
        let actual = raw
            .split(|byte| *byte == 0)
            .next()
            .and_then(|path| std::str::from_utf8(path).ok())
            .unwrap_or("<invalid>")
            .to_string();
        if raw.last() != Some(&0) || actual != spec.path {
            return Err(ShellExecError::PathMismatch {
                selector: spec.selector,
                expected: spec.path.to_string(),
                actual,
            });
        }
    }
    Ok(())
}

fn verify_jump_table(map: &ExeMap, text: &[u8]) -> Result<usize, ShellExecError> {
    let entries = usize::from(SELECTOR_LAST - SELECTOR_FIRST + 1);
    let raw = runtime_slice(map, text, JUMP_TABLE_RUNTIME, entries * 4)?;
    let mut default_selectors = 0usize;
    for (index, word) in raw.chunks_exact(4).enumerate() {
        let selector = SELECTOR_FIRST + index as u8;
        let actual = u32::from_le_bytes(word.try_into().unwrap());
        let expected = SHELL_EXEC_PATHS
            .iter()
            .find(|spec| spec.selector == selector)
            .map_or(DEFAULT_BLOCK, |spec| spec.block);
        if actual != expected {
            return Err(ShellExecError::JumpTableMismatch {
                selector,
                expected,
                actual,
            });
        }
        if actual == DEFAULT_BLOCK {
            default_selectors += 1;
        }
    }
    Ok(default_selectors)
}

fn runtime_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    address: u32,
    len: usize,
) -> Result<&'a [u8], ShellExecError> {
    let offset = map
        .text_offset(address)
        .ok_or(ShellExecError::RangeOutOfBounds { address, len })?;
    let end = offset
        .checked_add(len)
        .ok_or(ShellExecError::RangeOutOfBounds { address, len })?;
    text.get(offset..end)
        .ok_or(ShellExecError::RangeOutOfBounds { address, len })
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), ShellExecError> {
    let actual = instructions
        .get((address - PATH_TABLE_RUNTIME) as usize / 4)
        .filter(|instruction| instruction.addr == address)
        .ok_or(ShellExecError::MissingInstruction(address))?;
    if actual.kind != kind {
        return Err(ShellExecError::InstructionMismatch {
            address,
            expected,
            actual: actual.kind,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "shell_exec_tests.rs"]
mod shell_exec_tests;
