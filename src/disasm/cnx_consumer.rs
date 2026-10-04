//! 11개 EXE에 링크된 CNX 해제 소비 코드의 정확한 분모.
//!
//! `CNX\x02` magic을 조립하는 명령쌍, 해제 core를 부르는 정적 `jal`,
//! direct/streaming wrapper 선택 flag를 독립적으로 전수 열거해 고정 지원
//! 원본의 주소 계약에 대조한다. 이는 코드 존재와 shape를 증명하지만 정상
//! 게임 경로의 도달성까지 대신하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;

const CONSUMER_BUNDLE_WORDS: usize = 126;

const SHELL_MAGIC_CHECKS: [u32; 3] = [0x8012_2EB8, 0x8012_2EE0, 0x8012_2F80];
const GOCHA1_MAGIC_CHECKS: [u32; 3] = [0x8001_3D8C, 0x8001_3DB4, 0x8001_3E54];
const GOCHA2_MAGIC_CHECKS: [u32; 3] = [0x8001_C5F4, 0x8001_C61C, 0x8001_C6BC];
const HORI2_MAGIC_CHECKS: [u32; 3] = [0x8001_C620, 0x8001_C648, 0x8001_C6E8];
const HORI4_MAGIC_CHECKS: [u32; 3] = [0x8001_C600, 0x8001_C628, 0x8001_C6C8];
const KOUSIKI_MAGIC_CHECKS: [u32; 3] = [0x8001_49A0, 0x8001_49C8, 0x8001_4A68];
const MINPUYO_MAGIC_CHECKS: [u32; 3] = [0x8001_C5E4, 0x8001_C60C, 0x8001_C6AC];
const PCOMI_MAGIC_CHECKS: [u32; 3] = [0x8001_267C, 0x8001_26A0, 0x8001_2744];
const PUYO1_MAGIC_CHECKS: [u32; 3] = [0x8001_6E10, 0x8001_6E38, 0x8001_6ED8];
const PUYO2_MAGIC_CHECKS: [u32; 3] = [0x8001_56D0, 0x8001_56F8, 0x8001_5798];
const QUEST_MAGIC_CHECKS: [u32; 3] = [0x8001_66AC, 0x8001_66D4, 0x8001_6774];

const SHELL_CORE_CALLS: [u32; 2] = [0x8012_2F50, 0x8012_303C];
const GOCHA1_CORE_CALLS: [u32; 2] = [0x8001_3E24, 0x8001_3F10];
const GOCHA2_CORE_CALLS: [u32; 2] = [0x8001_C68C, 0x8001_C778];
const HORI2_CORE_CALLS: [u32; 2] = [0x8001_C6B8, 0x8001_C7A4];
const HORI4_CORE_CALLS: [u32; 2] = [0x8001_C698, 0x8001_C784];
const KOUSIKI_CORE_CALLS: [u32; 2] = [0x8001_4A38, 0x8001_4B24];
const MINPUYO_CORE_CALLS: [u32; 2] = [0x8001_C67C, 0x8001_C768];
const PCOMI_CORE_CALLS: [u32; 2] = [0x8001_2714, 0x8001_2800];
const PUYO1_CORE_CALLS: [u32; 2] = [0x8001_6EA8, 0x8001_6F94];
const PUYO2_CORE_CALLS: [u32; 2] = [0x8001_5768, 0x8001_5854];
const QUEST_CORE_CALLS: [u32; 2] = [0x8001_6744, 0x8001_6830];

/// 명령 shape가 같은 CNX 해제 core 계열.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CnxCoreFamily {
    /// 셸, QUEST와 PCOMI를 제외한 서브게임 8개가 공유하는 shape.
    Shared,
    /// register allocation과 상태 layout이 다른 PCOMI 전용 shape.
    Pcomi,
}

/// EXE 하나의 direct/streaming 선택 코드 계약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CnxPathSpec {
    pub direct_caller: u32,
    pub streaming_init_caller: u32,
    pub streaming_continue_callers: [u32; 2],
    pub selector_gp_offset: i16,
    pub selector_loads: [u32; 3],
    pub selector_enable_store: u32,
    pub selector_initializer_caller: u32,
}

/// EXE 하나의 CNX 소비 코드 계약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExeCnxConsumerSpec {
    pub exe: &'static str,
    pub core: u32,
    pub core_words: usize,
    pub magic_checks: &'static [u32; 3],
    pub core_calls: &'static [u32; 2],
    pub family: CnxCoreFamily,
    pub paths: CnxPathSpec,
}

/// 고정 지원 원본의 11개 EXE별 CNX 소비 코드 계약.
pub const BOX_CNX_CONSUMERS: [ExeCnxConsumerSpec; 11] = [
    ExeCnxConsumerSpec {
        exe: "SLPS_031.14",
        core: 0x8012_2D28,
        core_words: 110,
        magic_checks: &SHELL_MAGIC_CHECKS,
        core_calls: &SHELL_CORE_CALLS,
        family: CnxCoreFamily::Shared,
        paths: CnxPathSpec {
            direct_caller: 0x8012_3724,
            streaming_init_caller: 0x8012_36BC,
            streaming_continue_callers: [0x8012_36D8, 0x8012_5048],
            selector_gp_offset: 0x00EC,
            selector_loads: [0x8012_36A4, 0x8012_36D4, 0x8012_5040],
            selector_enable_store: 0x8012_4A44,
            selector_initializer_caller: 0x8012_B33C,
        },
    },
    ExeCnxConsumerSpec {
        exe: "GOCHA1.EXE",
        core: 0x8001_3BFC,
        core_words: 110,
        magic_checks: &GOCHA1_MAGIC_CHECKS,
        core_calls: &GOCHA1_CORE_CALLS,
        family: CnxCoreFamily::Shared,
        paths: CnxPathSpec {
            direct_caller: 0x8001_65EC,
            streaming_init_caller: 0x8001_6584,
            streaming_continue_callers: [0x8001_65A0, 0x8001_95E8],
            selector_gp_offset: 0x03B0,
            selector_loads: [0x8001_656C, 0x8001_659C, 0x8001_95E0],
            selector_enable_store: 0x8001_8964,
            selector_initializer_caller: 0x8002_CC48,
        },
    },
    ExeCnxConsumerSpec {
        exe: "GOCHA2.EXE",
        core: 0x8001_C464,
        core_words: 110,
        magic_checks: &GOCHA2_MAGIC_CHECKS,
        core_calls: &GOCHA2_CORE_CALLS,
        family: CnxCoreFamily::Shared,
        paths: CnxPathSpec {
            direct_caller: 0x8001_EB4C,
            streaming_init_caller: 0x8001_EAE4,
            streaming_continue_callers: [0x8001_EB00, 0x8002_1AF0],
            selector_gp_offset: 0x0418,
            selector_loads: [0x8001_EACC, 0x8001_EAFC, 0x8002_1AE8],
            selector_enable_store: 0x8002_0EC4,
            selector_initializer_caller: 0x8002_94E0,
        },
    },
    ExeCnxConsumerSpec {
        exe: "HORI2.EXE",
        core: 0x8001_C490,
        core_words: 110,
        magic_checks: &HORI2_MAGIC_CHECKS,
        core_calls: &HORI2_CORE_CALLS,
        family: CnxCoreFamily::Shared,
        paths: CnxPathSpec {
            direct_caller: 0x8001_EB78,
            streaming_init_caller: 0x8001_EB10,
            streaming_continue_callers: [0x8001_EB2C, 0x8002_1B10],
            selector_gp_offset: 0x0420,
            selector_loads: [0x8001_EAF8, 0x8001_EB28, 0x8002_1B08],
            selector_enable_store: 0x8002_0EE4,
            selector_initializer_caller: 0x8002_9500,
        },
    },
    ExeCnxConsumerSpec {
        exe: "HORI4.EXE",
        core: 0x8001_C470,
        core_words: 110,
        magic_checks: &HORI4_MAGIC_CHECKS,
        core_calls: &HORI4_CORE_CALLS,
        family: CnxCoreFamily::Shared,
        paths: CnxPathSpec {
            direct_caller: 0x8001_EB48,
            streaming_init_caller: 0x8001_EAE0,
            streaming_continue_callers: [0x8001_EAFC, 0x8002_1AE0],
            selector_gp_offset: 0x0414,
            selector_loads: [0x8001_EAC8, 0x8001_EAF8, 0x8002_1AD8],
            selector_enable_store: 0x8002_0EB4,
            selector_initializer_caller: 0x8002_97D8,
        },
    },
    ExeCnxConsumerSpec {
        exe: "KOUSIKI.EXE",
        core: 0x8001_4810,
        core_words: 110,
        magic_checks: &KOUSIKI_MAGIC_CHECKS,
        core_calls: &KOUSIKI_CORE_CALLS,
        family: CnxCoreFamily::Shared,
        paths: CnxPathSpec {
            direct_caller: 0x8001_6ED8,
            streaming_init_caller: 0x8001_6E70,
            streaming_continue_callers: [0x8001_6E8C, 0x8001_894C],
            selector_gp_offset: 0x03BC,
            selector_loads: [0x8001_6E58, 0x8001_6E88, 0x8001_8944],
            selector_enable_store: 0x8001_82F0,
            selector_initializer_caller: 0x8002_9684,
        },
    },
    ExeCnxConsumerSpec {
        exe: "MINPUYO.EXE",
        core: 0x8001_C454,
        core_words: 110,
        magic_checks: &MINPUYO_MAGIC_CHECKS,
        core_calls: &MINPUYO_CORE_CALLS,
        family: CnxCoreFamily::Shared,
        paths: CnxPathSpec {
            direct_caller: 0x8001_EB2C,
            streaming_init_caller: 0x8001_EAC4,
            streaming_continue_callers: [0x8001_EAE0, 0x8002_1AD4],
            selector_gp_offset: 0x0410,
            selector_loads: [0x8001_EAAC, 0x8001_EADC, 0x8002_1ACC],
            selector_enable_store: 0x8002_0EA8,
            selector_initializer_caller: 0x8002_9824,
        },
    },
    ExeCnxConsumerSpec {
        exe: "PCOMI.EXE",
        core: 0x8001_24EC,
        core_words: 100,
        magic_checks: &PCOMI_MAGIC_CHECKS,
        core_calls: &PCOMI_CORE_CALLS,
        family: CnxCoreFamily::Pcomi,
        paths: CnxPathSpec {
            direct_caller: 0x8001_2F00,
            streaming_init_caller: 0x8001_2E7C,
            streaming_continue_callers: [0x8001_2E98, 0x8001_4C4C],
            selector_gp_offset: 0x00A8,
            selector_loads: [0x8001_2E64, 0x8001_2E94, 0x8001_4C44],
            selector_enable_store: 0x8001_4350,
            selector_initializer_caller: 0x8001_DFB8,
        },
    },
    ExeCnxConsumerSpec {
        exe: "PUYO1.EXE",
        core: 0x8001_6C80,
        core_words: 110,
        magic_checks: &PUYO1_MAGIC_CHECKS,
        core_calls: &PUYO1_CORE_CALLS,
        family: CnxCoreFamily::Shared,
        paths: CnxPathSpec {
            direct_caller: 0x8001_B368,
            streaming_init_caller: 0x8001_B300,
            streaming_continue_callers: [0x8001_B31C, 0x8001_D06C],
            selector_gp_offset: 0x042C,
            selector_loads: [0x8001_B2E8, 0x8001_B318, 0x8001_D064],
            selector_enable_store: 0x8001_C770,
            selector_initializer_caller: 0x8002_A4A4,
        },
    },
    ExeCnxConsumerSpec {
        exe: "PUYO2.EXE",
        core: 0x8001_5540,
        core_words: 110,
        magic_checks: &PUYO2_MAGIC_CHECKS,
        core_calls: &PUYO2_CORE_CALLS,
        family: CnxCoreFamily::Shared,
        paths: CnxPathSpec {
            direct_caller: 0x8001_9DF4,
            streaming_init_caller: 0x8001_9D8C,
            streaming_continue_callers: [0x8001_9DA8, 0x8001_BAF8],
            selector_gp_offset: 0x0438,
            selector_loads: [0x8001_9D74, 0x8001_9DA4, 0x8001_BAF0],
            selector_enable_store: 0x8001_B1FC,
            selector_initializer_caller: 0x8002_89E4,
        },
    },
    ExeCnxConsumerSpec {
        exe: "QUEST.EXE",
        core: 0x8001_651C,
        core_words: 110,
        magic_checks: &QUEST_MAGIC_CHECKS,
        core_calls: &QUEST_CORE_CALLS,
        family: CnxCoreFamily::Shared,
        paths: CnxPathSpec {
            direct_caller: 0x8001_8DB4,
            streaming_init_caller: 0x8001_8D4C,
            streaming_continue_callers: [0x8001_8D68, 0x8001_BEC0],
            selector_gp_offset: 0x04E8,
            selector_loads: [0x8001_8D34, 0x8001_8D64, 0x8001_BEB8],
            selector_enable_store: 0x8001_B120,
            selector_initializer_caller: 0x8002_745C,
        },
    },
];

/// 11개 EXE의 CNX 소비 코드 분모.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CnxConsumerSummary {
    pub executables: usize,
    pub magic_checks: usize,
    pub decode_paths: usize,
    pub wrapper_calls: usize,
    pub selector_initializer_calls: usize,
    pub selector_loads: usize,
    pub selector_enable_stores: usize,
    pub cores: usize,
    pub families: usize,
    pub shared_family_executables: usize,
    pub pcomi_family_executables: usize,
}

/// CNX 소비 코드 분모 검증 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CnxConsumerError {
    EmptyInput,
    DuplicateExecutable(String),
    UnexpectedExecutable(String),
    MissingExecutable(&'static str),
    UnalignedText(String),
    MagicCheckMismatch {
        exe: String,
        expected: Vec<u32>,
        actual: Vec<u32>,
    },
    CoreCallMismatch {
        exe: String,
        expected: Vec<u32>,
        actual: Vec<u32>,
    },
    WrapperCallMismatch {
        exe: String,
        path: &'static str,
        expected: Vec<u32>,
        actual: Vec<u32>,
    },
    SelectorLoadMismatch {
        exe: String,
        expected: Vec<u32>,
        actual: Vec<u32>,
    },
    SelectorStoreMismatch {
        exe: String,
        expected: Vec<u32>,
        actual: Vec<u32>,
    },
    SelectorReferenceMismatch {
        exe: String,
        expected: Vec<u32>,
        actual: Vec<u32>,
    },
    SelectorShapeMismatch {
        exe: String,
        detail: &'static str,
    },
    CoreRange {
        exe: String,
        core: u32,
        words: usize,
    },
    ConsumerRange {
        exe: String,
        start: u32,
        words: usize,
    },
    SharedCoreShapeMismatch {
        baseline: &'static str,
        actual: &'static str,
    },
    SharedConsumerShapeMismatch {
        baseline: &'static str,
        actual: &'static str,
    },
    SummaryMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for CnxConsumerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "CNX 소비 코드를 검증할 EXE가 없음"),
            Self::DuplicateExecutable(exe) => write!(f, "CNX 소비 코드 EXE 중복: {exe}"),
            Self::UnexpectedExecutable(exe) => write!(f, "CNX 소비 코드 미등록 EXE: {exe}"),
            Self::MissingExecutable(exe) => write!(f, "CNX 소비 코드 필수 EXE 누락: {exe}"),
            Self::UnalignedText(exe) => write!(f, "{exe}: EXE text가 4바이트 정렬이 아님"),
            Self::MagicCheckMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe}: CNX magic 검사 주소 불일치: expected={expected:?}, actual={actual:?}"
            ),
            Self::CoreCallMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe}: CNX core 호출 주소 불일치: expected={expected:?}, actual={actual:?}"
            ),
            Self::WrapperCallMismatch {
                exe,
                path,
                expected,
                actual,
            } => write!(
                f,
                "{exe}: CNX {path} wrapper 호출 주소 불일치: \
                 expected={expected:?}, actual={actual:?}"
            ),
            Self::SelectorLoadMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe}: CNX 경로 selector load 주소 불일치: \
                 expected={expected:?}, actual={actual:?}"
            ),
            Self::SelectorStoreMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe}: CNX 경로 selector store 주소 불일치: \
                 expected={expected:?}, actual={actual:?}"
            ),
            Self::SelectorReferenceMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe}: CNX 경로 selector GP-relative 참조 불일치: \
                 expected={expected:?}, actual={actual:?}"
            ),
            Self::SelectorShapeMismatch { exe, detail } => {
                write!(f, "{exe}: CNX 경로 selector shape 불일치: {detail}")
            }
            Self::CoreRange { exe, core, words } => {
                write!(f, "{exe}: CNX core 범위 밖: 0x{core:08X}, {words} words")
            }
            Self::ConsumerRange { exe, start, words } => write!(
                f,
                "{exe}: CNX 소비 bundle 범위 밖: 0x{start:08X}, {words} words"
            ),
            Self::SharedCoreShapeMismatch { baseline, actual } => write!(
                f,
                "공유 CNX core shape 불일치: baseline={baseline}, actual={actual}"
            ),
            Self::SharedConsumerShapeMismatch { baseline, actual } => write!(
                f,
                "공유 CNX 소비 bundle shape 불일치: baseline={baseline}, actual={actual}"
            ),
            Self::SummaryMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "BOX CNX 소비 코드 {field} 불일치: expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for CnxConsumerError {}

fn magic_register(instruction: &Instruction) -> Option<Reg> {
    match instruction.kind {
        K::Lui { rt, imm: 0x0258 } => Some(rt),
        _ => None,
    }
}

fn is_magic_ori(instruction: &Instruction, register: Reg) -> bool {
    matches!(
        instruction.kind,
        K::Ori {
            rt,
            rs,
            imm: 0x4E43
        } if rt == register && rs == register
    )
}

fn scan_magic_checks(exe: &str, map: &ExeMap, text: &[u8]) -> Result<Vec<u32>, CnxConsumerError> {
    let instructions =
        linear::sweep(map, text).ok_or_else(|| CnxConsumerError::UnalignedText(exe.to_string()))?;
    let mut checks = Vec::new();
    for (index, instruction) in instructions.iter().enumerate() {
        let Some(register) = magic_register(instruction) else {
            continue;
        };
        if !(1..=4).any(|distance| {
            instructions
                .get(index + distance)
                .is_some_and(|candidate| is_magic_ori(candidate, register))
        }) {
            continue;
        }
        checks.push(instruction.addr);
    }
    Ok(checks)
}

fn scan_core_calls(
    exe: &str,
    map: &ExeMap,
    text: &[u8],
    core: u32,
) -> Result<Vec<u32>, CnxConsumerError> {
    let instructions =
        linear::sweep(map, text).ok_or_else(|| CnxConsumerError::UnalignedText(exe.to_string()))?;
    Ok(instructions
        .iter()
        .filter_map(|instruction| {
            matches!(instruction.kind, K::Jal { target } if target == core)
                .then_some(instruction.addr)
        })
        .collect())
}

fn is_gp_load_at(instruction: &Instruction, offset: i16) -> bool {
    matches!(
        instruction.kind,
        K::Lb {
            base: Reg::GP,
            offset: actual,
            ..
        } | K::Lbu {
            base: Reg::GP,
            offset: actual,
            ..
        } | K::Lh {
            base: Reg::GP,
            offset: actual,
            ..
        } | K::Lhu {
            base: Reg::GP,
            offset: actual,
            ..
        } | K::Lw {
            base: Reg::GP,
            offset: actual,
            ..
        } | K::Lwl {
            base: Reg::GP,
            offset: actual,
            ..
        } | K::Lwr {
            base: Reg::GP,
            offset: actual,
            ..
        } if actual == offset
    )
}

fn is_gp_store_at(instruction: &Instruction, offset: i16) -> bool {
    matches!(
        instruction.kind,
        K::Sb {
            base: Reg::GP,
            offset: actual,
            ..
        } | K::Sh {
            base: Reg::GP,
            offset: actual,
            ..
        } | K::Sw {
            base: Reg::GP,
            offset: actual,
            ..
        } | K::Swl {
            base: Reg::GP,
            offset: actual,
            ..
        } | K::Swr {
            base: Reg::GP,
            offset: actual,
            ..
        } if actual == offset
    )
}

fn selector_accesses(instructions: &[Instruction], offset: i16, load: bool) -> Vec<u32> {
    instructions
        .iter()
        .filter_map(|instruction| {
            let matches = if load {
                is_gp_load_at(instruction, offset)
            } else {
                is_gp_store_at(instruction, offset)
            };
            matches.then_some(instruction.addr)
        })
        .collect()
}

fn selector_gp_relative_references(instructions: &[Instruction], offset: i16) -> Vec<u32> {
    instructions
        .iter()
        .filter_map(|instruction| {
            let opcode = instruction.word >> 26;
            let base = (instruction.word >> 21) & 0x1F;
            let has_address_immediate = matches!(
                opcode,
                0x08..=0x0E
                    | 0x20
                    | 0x21
                    | 0x22
                    | 0x23
                    | 0x24
                    | 0x25
                    | 0x26
                    | 0x28
                    | 0x29
                    | 0x2A
                    | 0x2B
                    | 0x2E
                    | 0x32
                    | 0x3A
            );
            (has_address_immediate
                && base == u32::from(Reg::GP.0)
                && instruction.word as u16 == offset as u16)
                .then_some(instruction.addr)
        })
        .collect()
}

fn instruction_at(instructions: &[Instruction], addr: u32) -> Option<&Instruction> {
    instructions
        .binary_search_by_key(&addr, |instruction| instruction.addr)
        .ok()
        .and_then(|index| instructions.get(index))
}

fn wrapper_calls(
    exe: &str,
    map: &ExeMap,
    text: &[u8],
    target: u32,
) -> Result<Vec<u32>, CnxConsumerError> {
    scan_core_calls(exe, map, text, target)
}

fn verify_wrapper_calls(
    exe: &str,
    map: &ExeMap,
    text: &[u8],
    path: &'static str,
    target: u32,
    expected: &[u32],
) -> Result<(), CnxConsumerError> {
    let actual = wrapper_calls(exe, map, text, target)?;
    if actual != expected {
        return Err(CnxConsumerError::WrapperCallMismatch {
            exe: exe.to_string(),
            path,
            expected: expected.to_vec(),
            actual,
        });
    }
    Ok(())
}

fn selector_shape_error(exe: &str, detail: &'static str) -> CnxConsumerError {
    CnxConsumerError::SelectorShapeMismatch {
        exe: exe.to_string(),
        detail,
    }
}

fn verify_selector_shape(
    exe: &str,
    instructions: &[Instruction],
    spec: ExeCnxConsumerSpec,
) -> Result<(), CnxConsumerError> {
    let paths = spec.paths;
    if spec.magic_checks[0].checked_add(0x98) != Some(spec.core_calls[0])
        || spec.magic_checks[2].checked_add(0xBC) != Some(spec.core_calls[1])
    {
        return Err(selector_shape_error(
            exe,
            "wrapper와 core call의 상대 위치가 다름",
        ));
    }
    if paths.selector_loads[0].checked_add(0x18) != Some(paths.streaming_init_caller)
        || paths.selector_loads[1].checked_add(4) != Some(paths.streaming_continue_callers[0])
        || paths.selector_loads[2].checked_add(8) != Some(paths.streaming_continue_callers[1])
    {
        return Err(selector_shape_error(
            exe,
            "selector load와 streaming caller의 상대 위치가 다름",
        ));
    }

    let first = instruction_at(instructions, paths.selector_loads[0])
        .ok_or_else(|| selector_shape_error(exe, "첫 selector load가 없음"))?;
    let selector_register = match first.kind {
        K::Lw {
            rt,
            base: Reg::GP,
            offset,
        } if offset == paths.selector_gp_offset => rt,
        _ => {
            return Err(selector_shape_error(
                exe,
                "첫 selector가 GP-relative lw가 아님",
            ))
        }
    };
    let branch = instruction_at(instructions, first.addr.wrapping_add(8))
        .ok_or_else(|| selector_shape_error(exe, "direct 선택 branch가 없음"))?;
    if !matches!(
        branch.kind,
        K::Beq {
            rs,
            rt: Reg::ZERO,
            ..
        } if rs == selector_register
    ) {
        return Err(selector_shape_error(
            exe,
            "selector 0을 direct로 보내는 beq가 아님",
        ));
    }
    let expected_direct_block = paths
        .direct_caller
        .checked_sub(8)
        .ok_or_else(|| selector_shape_error(exe, "direct block 주소 underflow"))?;
    if !matches!(
        branch.flow_kind(),
        crate::isa::FlowKind::Branch { taken } if taken == expected_direct_block
    ) {
        return Err(selector_shape_error(
            exe,
            "selector branch target이 direct wrapper block이 아님",
        ));
    }

    let store = instruction_at(instructions, paths.selector_enable_store)
        .ok_or_else(|| selector_shape_error(exe, "selector enable store가 없음"))?;
    let store_register = match store.kind {
        K::Sw {
            rt,
            base: Reg::GP,
            offset,
        } if offset == paths.selector_gp_offset => rt,
        _ => {
            return Err(selector_shape_error(
                exe,
                "selector enable이 GP-relative sw가 아님",
            ))
        }
    };
    let value = instruction_at(instructions, store.addr.wrapping_sub(4))
        .ok_or_else(|| selector_shape_error(exe, "selector enable 값 생성이 없음"))?;
    if !matches!(
        value.kind,
        K::Addiu {
            rt,
            rs: Reg::ZERO,
            imm: 1,
        } if rt == store_register
    ) {
        return Err(selector_shape_error(
            exe,
            "selector enable store가 상수 1을 쓰지 않음",
        ));
    }
    Ok(())
}

fn verify_path_contract(
    exe: &str,
    map: &ExeMap,
    text: &[u8],
    spec: ExeCnxConsumerSpec,
) -> Result<(), CnxConsumerError> {
    let direct_wrapper = spec.magic_checks[0].wrapping_add(0x20);
    let streaming_init_wrapper = spec.magic_checks[2];
    let streaming_continue_wrapper = spec.magic_checks[2].wrapping_add(0x6C);
    verify_wrapper_calls(
        exe,
        map,
        text,
        "direct",
        direct_wrapper,
        &[spec.paths.direct_caller],
    )?;
    verify_wrapper_calls(
        exe,
        map,
        text,
        "streaming init",
        streaming_init_wrapper,
        &[spec.paths.streaming_init_caller],
    )?;
    verify_wrapper_calls(
        exe,
        map,
        text,
        "streaming continue",
        streaming_continue_wrapper,
        &spec.paths.streaming_continue_callers,
    )?;
    let selector_initializer = spec
        .paths
        .selector_enable_store
        .checked_sub(0x38)
        .ok_or_else(|| selector_shape_error(exe, "selector initializer 주소 underflow"))?;
    verify_wrapper_calls(
        exe,
        map,
        text,
        "selector initializer",
        selector_initializer,
        &[spec.paths.selector_initializer_caller],
    )?;

    let instructions =
        linear::sweep(map, text).ok_or_else(|| CnxConsumerError::UnalignedText(exe.to_string()))?;
    let selector_loads = selector_accesses(&instructions, spec.paths.selector_gp_offset, true);
    if selector_loads != spec.paths.selector_loads {
        return Err(CnxConsumerError::SelectorLoadMismatch {
            exe: exe.to_string(),
            expected: spec.paths.selector_loads.to_vec(),
            actual: selector_loads,
        });
    }
    let selector_stores = selector_accesses(&instructions, spec.paths.selector_gp_offset, false);
    if selector_stores != [spec.paths.selector_enable_store] {
        return Err(CnxConsumerError::SelectorStoreMismatch {
            exe: exe.to_string(),
            expected: vec![spec.paths.selector_enable_store],
            actual: selector_stores,
        });
    }
    let selector_references =
        selector_gp_relative_references(&instructions, spec.paths.selector_gp_offset);
    let mut expected_references = spec.paths.selector_loads.to_vec();
    expected_references.push(spec.paths.selector_enable_store);
    expected_references.sort_unstable();
    if selector_references != expected_references {
        return Err(CnxConsumerError::SelectorReferenceMismatch {
            exe: exe.to_string(),
            expected: expected_references,
            actual: selector_references,
        });
    }
    verify_selector_shape(exe, &instructions, spec)
}

fn core_instructions(
    exe: &str,
    map: &ExeMap,
    text: &[u8],
    spec: ExeCnxConsumerSpec,
) -> Result<Vec<Instruction>, CnxConsumerError> {
    let start = spec
        .core
        .checked_sub(map.load_addr)
        .map(|offset| offset as usize)
        .ok_or_else(|| CnxConsumerError::CoreRange {
            exe: exe.to_string(),
            core: spec.core,
            words: spec.core_words,
        })?;
    let len = spec
        .core_words
        .checked_mul(4)
        .ok_or_else(|| CnxConsumerError::CoreRange {
            exe: exe.to_string(),
            core: spec.core,
            words: spec.core_words,
        })?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| CnxConsumerError::CoreRange {
            exe: exe.to_string(),
            core: spec.core,
            words: spec.core_words,
        })?;
    let raw = text
        .get(start..end)
        .ok_or_else(|| CnxConsumerError::CoreRange {
            exe: exe.to_string(),
            core: spec.core,
            words: spec.core_words,
        })?;
    crate::isa::decode_slice(spec.core, raw).ok_or_else(|| CnxConsumerError::CoreRange {
        exe: exe.to_string(),
        core: spec.core,
        words: spec.core_words,
    })
}

fn consumer_bundle_instructions(
    exe: &str,
    map: &ExeMap,
    text: &[u8],
    spec: ExeCnxConsumerSpec,
) -> Result<Vec<Instruction>, CnxConsumerError> {
    let start_addr = spec.magic_checks[0];
    let start = start_addr
        .checked_sub(map.load_addr)
        .map(|offset| offset as usize)
        .ok_or_else(|| CnxConsumerError::ConsumerRange {
            exe: exe.to_string(),
            start: start_addr,
            words: CONSUMER_BUNDLE_WORDS,
        })?;
    let len =
        CONSUMER_BUNDLE_WORDS
            .checked_mul(4)
            .ok_or_else(|| CnxConsumerError::ConsumerRange {
                exe: exe.to_string(),
                start: start_addr,
                words: CONSUMER_BUNDLE_WORDS,
            })?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| CnxConsumerError::ConsumerRange {
            exe: exe.to_string(),
            start: start_addr,
            words: CONSUMER_BUNDLE_WORDS,
        })?;
    let raw = text
        .get(start..end)
        .ok_or_else(|| CnxConsumerError::ConsumerRange {
            exe: exe.to_string(),
            start: start_addr,
            words: CONSUMER_BUNDLE_WORDS,
        })?;
    crate::isa::decode_slice(start_addr, raw).ok_or_else(|| CnxConsumerError::ConsumerRange {
        exe: exe.to_string(),
        start: start_addr,
        words: CONSUMER_BUNDLE_WORDS,
    })
}

fn gp_base(instructions: &[Instruction]) -> Option<u16> {
    instructions
        .iter()
        .filter_map(|instruction| {
            let op = instruction.word >> 26;
            let rs = (instruction.word >> 21) & 0x1F;
            (rs == u32::from(Reg::GP.0)
                && matches!(op, 0x20 | 0x21 | 0x23 | 0x24 | 0x25 | 0x28 | 0x29 | 0x2B))
            .then_some(instruction.word as u16)
        })
        .min()
}

fn normalized_core_shape(instructions: &[Instruction], core: u32) -> Option<Vec<u64>> {
    let gp_base = u32::from(gp_base(instructions)?);
    Some(
        instructions
            .iter()
            .map(|instruction| {
                let op = instruction.word >> 26;
                match instruction.kind {
                    K::J { target } | K::Jal { target } => {
                        (u64::from(op) << 32) | u64::from(target.wrapping_sub(core))
                    }
                    _ => {
                        let rs = (instruction.word >> 21) & 0x1F;
                        if rs == u32::from(Reg::GP.0)
                            && matches!(op, 0x20 | 0x21 | 0x23 | 0x24 | 0x25 | 0x28 | 0x29 | 0x2B)
                        {
                            let relative = u32::from(instruction.word as u16) - gp_base;
                            (u64::from(instruction.word & 0xFFFF_0000) << 32) | u64::from(relative)
                        } else {
                            u64::from(instruction.word)
                        }
                    }
                }
            })
            .collect(),
    )
}

fn require_summary(
    field: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), CnxConsumerError> {
    if expected != actual {
        return Err(CnxConsumerError::SummaryMismatch {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

/// 11개 EXE의 magic 검사·decode 경로·core와 두 code family 분모를 검증한다.
pub fn verify_box_cnx_consumers<'a>(
    executables: impl IntoIterator<Item = (&'a str, &'a ExeMap, &'a [u8])>,
) -> Result<CnxConsumerSummary, CnxConsumerError> {
    let mut provided = BTreeMap::new();
    for (exe, map, text) in executables {
        if provided.insert(exe, (map, text)).is_some() {
            return Err(CnxConsumerError::DuplicateExecutable(exe.to_string()));
        }
    }
    if provided.is_empty() {
        return Err(CnxConsumerError::EmptyInput);
    }
    for exe in provided.keys() {
        if !BOX_CNX_CONSUMERS.iter().any(|spec| spec.exe == *exe) {
            return Err(CnxConsumerError::UnexpectedExecutable((*exe).to_string()));
        }
    }

    let mut magic_checks_total = 0usize;
    let mut decode_paths_total = 0usize;
    let mut wrapper_calls_total = 0usize;
    let mut selector_initializer_calls_total = 0usize;
    let mut selector_loads_total = 0usize;
    let mut selector_enable_stores_total = 0usize;
    let mut families = BTreeSet::new();
    let mut shared_shape: Option<(&'static str, Vec<u64>)> = None;
    let mut shared_consumer_shape: Option<(&'static str, Vec<u64>)> = None;
    let mut shared_family_executables = 0usize;
    let mut pcomi_family_executables = 0usize;
    for spec in BOX_CNX_CONSUMERS {
        let (map, text) = provided
            .get(spec.exe)
            .ok_or(CnxConsumerError::MissingExecutable(spec.exe))?;
        let magic_checks = scan_magic_checks(spec.exe, map, text)?;
        if magic_checks != spec.magic_checks {
            return Err(CnxConsumerError::MagicCheckMismatch {
                exe: spec.exe.to_string(),
                expected: spec.magic_checks.to_vec(),
                actual: magic_checks,
            });
        }
        let core_calls = scan_core_calls(spec.exe, map, text, spec.core)?;
        if core_calls != spec.core_calls {
            return Err(CnxConsumerError::CoreCallMismatch {
                exe: spec.exe.to_string(),
                expected: spec.core_calls.to_vec(),
                actual: core_calls,
            });
        }
        verify_path_contract(spec.exe, map, text, spec)?;
        magic_checks_total += spec.magic_checks.len();
        decode_paths_total += spec.core_calls.len();
        wrapper_calls_total += 4;
        selector_initializer_calls_total += 1;
        selector_loads_total += spec.paths.selector_loads.len();
        selector_enable_stores_total += 1;
        families.insert(spec.family);

        let core = core_instructions(spec.exe, map, text, spec)?;
        let consumer = consumer_bundle_instructions(spec.exe, map, text, spec)?;
        match spec.family {
            CnxCoreFamily::Shared => {
                shared_family_executables += 1;
                let shape = normalized_core_shape(&core, spec.core).ok_or_else(|| {
                    CnxConsumerError::CoreRange {
                        exe: spec.exe.to_string(),
                        core: spec.core,
                        words: spec.core_words,
                    }
                })?;
                if let Some((baseline_name, baseline)) = &shared_shape {
                    if baseline != &shape {
                        return Err(CnxConsumerError::SharedCoreShapeMismatch {
                            baseline: baseline_name,
                            actual: spec.exe,
                        });
                    }
                } else {
                    shared_shape = Some((spec.exe, shape));
                }
                let consumer_shape = normalized_core_shape(&consumer, spec.magic_checks[0])
                    .ok_or_else(|| CnxConsumerError::ConsumerRange {
                        exe: spec.exe.to_string(),
                        start: spec.magic_checks[0],
                        words: CONSUMER_BUNDLE_WORDS,
                    })?;
                if let Some((baseline_name, baseline)) = &shared_consumer_shape {
                    if baseline != &consumer_shape {
                        return Err(CnxConsumerError::SharedConsumerShapeMismatch {
                            baseline: baseline_name,
                            actual: spec.exe,
                        });
                    }
                } else {
                    shared_consumer_shape = Some((spec.exe, consumer_shape));
                }
            }
            CnxCoreFamily::Pcomi => pcomi_family_executables += 1,
        }
    }

    let summary = CnxConsumerSummary {
        executables: provided.len(),
        magic_checks: magic_checks_total,
        decode_paths: decode_paths_total,
        wrapper_calls: wrapper_calls_total,
        selector_initializer_calls: selector_initializer_calls_total,
        selector_loads: selector_loads_total,
        selector_enable_stores: selector_enable_stores_total,
        cores: BOX_CNX_CONSUMERS.len(),
        families: families.len(),
        shared_family_executables,
        pcomi_family_executables,
    };
    for (field, expected, actual) in [
        ("EXE 수", 11, summary.executables),
        ("magic 검사 수", 33, summary.magic_checks),
        ("decode 경로 수", 22, summary.decode_paths),
        ("wrapper 호출 수", 44, summary.wrapper_calls),
        (
            "selector initializer 호출 수",
            11,
            summary.selector_initializer_calls,
        ),
        ("selector load 수", 33, summary.selector_loads),
        (
            "selector enable store 수",
            11,
            summary.selector_enable_stores,
        ),
        ("core 수", 11, summary.cores),
        ("core family 수", 2, summary.families),
        ("공유 family EXE 수", 10, summary.shared_family_executables),
        ("PCOMI family EXE 수", 1, summary.pcomi_family_executables),
    ] {
        require_summary(field, expected, actual)?;
    }
    Ok(summary)
}

#[cfg(test)]
#[path = "cnx_consumer_tests.rs"]
mod tests;
