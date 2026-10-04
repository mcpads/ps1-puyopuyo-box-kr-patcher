//! `QUEST.EXE`가 interpreter 객체에 설치하는 resident SEQ root.
//!
//! opcode처럼 보이는 EXE data byte를 훑지 않고, interpreter 객체 생성 뒤
//! `+0x1C` stream field를 실제로 쓰는 constructor만 모집단으로 삼는다.
//! literal root와 현재 record·객체에서 전달된 runtime pointer를 구분한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{decode_slice, Instruction, InstructionKind as K, Reg};
use crate::script::quest_seq_opcode_handler;

const QUEST_SEQ_INTERPRETER_RUNTIME: u32 = 0x8002_2570;
const QUEST_OBJECT_ALLOCATE_RUNTIME: u32 = 0x8001_4B14;
const QUEST_SEQ_STREAM_FIELD_OFFSET: i16 = 0x1C;
const CONSTRUCTOR_STORE_WINDOW: usize = 12;
const QUEST_RESIDENT_ROOT_STATE_BASE_RUNTIME: u32 = 0x800F_BDD0;
const QUEST_RESIDENT_ROOT_SELECTOR_RUNTIME: u32 = 0x800F_BDD4;
const QUEST_RESIDENT_ROOT_DISPATCH_TABLE_RUNTIME: u32 = 0x8001_3050;
const QUEST_RESIDENT_ROOT_SELECTOR_HANDLERS: [u32; 8] = [
    0x8007_3E9C,
    0x8007_3EA8,
    0x8007_3EE0,
    0x8007_3EF0,
    0x8007_3F08,
    0x8007_3FAC,
    0x8007_4060,
    0x8007_4130,
];
const QUEST_RESIDENT_FAREWELL_SELECTOR: u8 = 4;
const QUEST_RESIDENT_DEFEAT_SELECTOR: u8 = 5;
const QUEST_RESIDENT_CURRENT_RECORD_OPCODE: u8 = 0x1C;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuestResidentSeqRootSource {
    LiteralExeStream,
    CurrentRecordPointer,
    ForwardedObjectPointer,
}

impl QuestResidentSeqRootSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::LiteralExeStream => "literal-exe-stream",
            Self::CurrentRecordPointer => "current-record-pointer",
            Self::ForwardedObjectPointer => "forwarded-object-pointer",
        }
    }
}

/// interpreter constructor의 stream 설치 한 곳.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestResidentSeqRootInstall {
    pub allocator_call_runtime: u32,
    pub stream_store_runtime: u32,
    pub source: QuestResidentSeqRootSource,
    pub literal_root_runtime: Option<u32>,
}

/// resident state machine이 작별·패배 constructor를 고르는 selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqRootSelection {
    pub state_base_runtime: u32,
    pub selector_runtime: u32,
    pub dispatch_table_runtime: u32,
    pub handlers: [u32; 8],
    pub farewell_selector: u8,
    pub farewell_handler_runtime: u32,
    pub farewell_stream_store_runtime: u32,
    pub farewell_root_runtime: u32,
    pub defeat_selector: u8,
    pub defeat_handler_runtime: u32,
    pub defeat_stream_store_runtime: u32,
    pub defeat_root_runtime: u32,
}

/// 지원 원본에서 확인한 resident SEQ root 설치 분모.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqRootSummary {
    pub installs: Vec<QuestResidentSeqRootInstall>,
    pub selection: QuestResidentSeqRootSelection,
    pub literal_install_sites: usize,
    pub literal_roots: Vec<u32>,
    pub current_record_pointer_sites: usize,
    pub current_record_pointer_opcode: u8,
    pub current_record_pointer_handler_runtime: u32,
    pub forwarded_object_pointer_sites: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqRootError(String);

impl fmt::Display for QuestResidentSeqRootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentSeqRootError {}

fn error(message: impl Into<String>) -> QuestResidentSeqRootError {
    QuestResidentSeqRootError(message.into())
}

#[derive(Debug, Clone, Copy)]
enum RootSourceSpec {
    Literal {
        lui_runtime: u32,
        combine_runtime: u32,
        root_runtime: u32,
    },
    CurrentRecordPointer {
        record_load_runtime: u32,
        pointer_load_runtime: u32,
    },
    ForwardedObjectPointer {
        pointer_load_runtime: u32,
    },
}

#[derive(Debug, Clone, Copy)]
struct RootInstallSpec {
    callback_lui_runtime: u32,
    callback_combine_runtime: u32,
    allocator_call_runtime: u32,
    stream_store_runtime: u32,
    source: RootSourceSpec,
}

const ROOT_INSTALL_SPECS: [RootInstallSpec; 8] = [
    RootInstallSpec {
        callback_lui_runtime: 0x8002_2C90,
        callback_combine_runtime: 0x8002_2C9C,
        allocator_call_runtime: 0x8002_2C98,
        stream_store_runtime: 0x8002_2CB0,
        source: RootSourceSpec::CurrentRecordPointer {
            record_load_runtime: 0x8002_2CA0,
            pointer_load_runtime: 0x8002_2CA8,
        },
    },
    RootInstallSpec {
        callback_lui_runtime: 0x8003_A36C,
        callback_combine_runtime: 0x8003_A370,
        allocator_call_runtime: 0x8003_A3A4,
        stream_store_runtime: 0x8003_A3C8,
        source: RootSourceSpec::ForwardedObjectPointer {
            pointer_load_runtime: 0x8003_A3B0,
        },
    },
    RootInstallSpec {
        callback_lui_runtime: 0x8007_2060,
        callback_combine_runtime: 0x8007_2064,
        allocator_call_runtime: 0x8007_208C,
        stream_store_runtime: 0x8007_20B4,
        source: RootSourceSpec::Literal {
            lui_runtime: 0x8007_20AC,
            combine_runtime: 0x8007_20B0,
            root_runtime: 0x8008_C590,
        },
    },
    RootInstallSpec {
        callback_lui_runtime: 0x8007_3F8C,
        callback_combine_runtime: 0x8007_3F90,
        allocator_call_runtime: 0x8007_3F94,
        stream_store_runtime: 0x8007_3FA8,
        source: RootSourceSpec::Literal {
            lui_runtime: 0x8007_3F9C,
            combine_runtime: 0x8007_3FA0,
            root_runtime: 0x8008_97DC,
        },
    },
    RootInstallSpec {
        callback_lui_runtime: 0x8007_4030,
        callback_combine_runtime: 0x8007_4034,
        allocator_call_runtime: 0x8007_4038,
        stream_store_runtime: 0x8007_4048,
        source: RootSourceSpec::Literal {
            lui_runtime: 0x8007_4040,
            combine_runtime: 0x8007_4044,
            root_runtime: 0x8008_C498,
        },
    },
    RootInstallSpec {
        callback_lui_runtime: 0x8007_67A4,
        callback_combine_runtime: 0x8007_67A8,
        allocator_call_runtime: 0x8007_67AC,
        stream_store_runtime: 0x8007_67BC,
        source: RootSourceSpec::Literal {
            lui_runtime: 0x8007_67B4,
            combine_runtime: 0x8007_67B8,
            root_runtime: 0x8008_9888,
        },
    },
    RootInstallSpec {
        callback_lui_runtime: 0x8007_7A64,
        callback_combine_runtime: 0x8007_7A68,
        allocator_call_runtime: 0x8007_7A6C,
        stream_store_runtime: 0x8007_7A7C,
        source: RootSourceSpec::Literal {
            lui_runtime: 0x8007_7A74,
            combine_runtime: 0x8007_7A78,
            root_runtime: 0x8008_97DC,
        },
    },
    RootInstallSpec {
        callback_lui_runtime: 0x8007_7B18,
        callback_combine_runtime: 0x8007_7B1C,
        allocator_call_runtime: 0x8007_7B20,
        stream_store_runtime: 0x8007_7B30,
        source: RootSourceSpec::Literal {
            lui_runtime: 0x8007_7B28,
            combine_runtime: 0x8007_7B2C,
            root_runtime: 0x8008_C498,
        },
    },
];

fn instruction_at(
    instructions: &[Instruction],
    runtime: u32,
) -> Result<&Instruction, QuestResidentSeqRootError> {
    instructions
        .binary_search_by_key(&runtime, |instruction| instruction.addr)
        .ok()
        .and_then(|index| instructions.get(index))
        .ok_or_else(|| {
            error(format!(
                "QUEST resident SEQ root 명령이 없음: 0x{runtime:08X}"
            ))
        })
}

fn require(
    instructions: &[Instruction],
    runtime: u32,
    expected: K,
    role: &str,
) -> Result<(), QuestResidentSeqRootError> {
    let actual = instruction_at(instructions, runtime)?;
    if actual.kind != expected {
        return Err(error(format!(
            "QUEST resident SEQ root {role} drift: 0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn read_u32_at(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    role: &str,
) -> Result<u32, QuestResidentSeqRootError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "QUEST resident SEQ root {role}가 EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    let bytes = text.get(offset..offset + 4).ok_or_else(|| {
        error(format!(
            "QUEST resident SEQ root {role}가 잘림: 0x{runtime:08X}"
        ))
    })?;
    Ok(u32::from_le_bytes(
        bytes.try_into().expect("길이를 4바이트로 검사함"),
    ))
}

fn verify_root_selection(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
) -> Result<QuestResidentSeqRootSelection, QuestResidentSeqRootError> {
    for (runtime, expected, role) in [
        (
            0x8007_3DF4,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8010,
            },
            "state base lui",
        ),
        (
            0x8007_3DF8,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_RESIDENT_ROOT_STATE_BASE_RUNTIME as u16 as i16,
            },
            "state gate load",
        ),
        (
            0x8007_3DFC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_RESIDENT_ROOT_STATE_BASE_RUNTIME as u16 as i16,
            },
            "state base combine",
        ),
        (
            0x8007_3E04,
            K::Beq {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 25,
            },
            "state gate branch",
        ),
        (
            0x8007_3E6C,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: (QUEST_RESIDENT_ROOT_SELECTOR_RUNTIME
                    - QUEST_RESIDENT_ROOT_STATE_BASE_RUNTIME) as i16,
            },
            "selector load",
        ),
        (
            0x8007_3E74,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: QUEST_RESIDENT_ROOT_SELECTOR_HANDLERS.len() as i16,
            },
            "selector range",
        ),
        (
            0x8007_3E78,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 197,
            },
            "selector range branch",
        ),
        (
            0x8007_3E7C,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8001,
            },
            "dispatch table lui",
        ),
        (
            0x8007_3E80,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_RESIDENT_ROOT_DISPATCH_TABLE_RUNTIME as u16 as i16,
            },
            "dispatch table combine",
        ),
        (
            0x8007_3E84,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
            "selector word index",
        ),
        (
            0x8007_3E88,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
            "dispatch entry address",
        ),
        (
            0x8007_3E8C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0,
            },
            "dispatch target load",
        ),
        (0x8007_3E94, K::Jr { rs: Reg::V0 }, "dispatch target jump"),
        (
            0x8007_3F0C,
            K::Sb {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: QUEST_RESIDENT_ROOT_SELECTOR_RUNTIME as u16 as i16,
            },
            "farewell selector clear",
        ),
        (
            0x8007_3FB0,
            K::Sb {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: QUEST_RESIDENT_ROOT_SELECTOR_RUNTIME as u16 as i16,
            },
            "defeat selector clear",
        ),
        (
            0x8007_3FA4,
            K::J {
                target: 0x8007_404C,
            },
            "farewell constructor exit",
        ),
    ] {
        require(instructions, runtime, expected, role)?;
    }

    let mut handlers = [0; QUEST_RESIDENT_ROOT_SELECTOR_HANDLERS.len()];
    for (index, handler) in handlers.iter_mut().enumerate() {
        let runtime = QUEST_RESIDENT_ROOT_DISPATCH_TABLE_RUNTIME
            + u32::try_from(index).expect("selector 수는 u32 범위") * 4;
        *handler = read_u32_at(map, text, runtime, "dispatch table entry")?;
    }
    if handlers != QUEST_RESIDENT_ROOT_SELECTOR_HANDLERS {
        return Err(error(format!(
            "QUEST resident SEQ root selector table drift: expected={QUEST_RESIDENT_ROOT_SELECTOR_HANDLERS:08X?}, actual={handlers:08X?}"
        )));
    }
    if !handlers.iter().all(|handler| map.contains(*handler)) {
        return Err(error(
            "QUEST resident SEQ root selector handler가 EXE text 밖임",
        ));
    }

    Ok(QuestResidentSeqRootSelection {
        state_base_runtime: QUEST_RESIDENT_ROOT_STATE_BASE_RUNTIME,
        selector_runtime: QUEST_RESIDENT_ROOT_SELECTOR_RUNTIME,
        dispatch_table_runtime: QUEST_RESIDENT_ROOT_DISPATCH_TABLE_RUNTIME,
        handlers,
        farewell_selector: QUEST_RESIDENT_FAREWELL_SELECTOR,
        farewell_handler_runtime: handlers[usize::from(QUEST_RESIDENT_FAREWELL_SELECTOR)],
        farewell_stream_store_runtime: 0x8007_3FA8,
        farewell_root_runtime: 0x8008_97DC,
        defeat_selector: QUEST_RESIDENT_DEFEAT_SELECTOR,
        defeat_handler_runtime: handlers[usize::from(QUEST_RESIDENT_DEFEAT_SELECTOR)],
        defeat_stream_store_runtime: 0x8007_4048,
        defeat_root_runtime: 0x8008_C498,
    })
}

fn constructor_stream_stores(instructions: &[Instruction]) -> Vec<(u32, u32)> {
    let mut stores = Vec::new();
    for (index, instruction) in instructions.iter().enumerate() {
        if instruction.kind
            != (K::Jal {
                target: QUEST_OBJECT_ALLOCATE_RUNTIME,
            })
        {
            continue;
        }
        for probe in instructions
            .iter()
            .skip(index + 1)
            .take(CONSTRUCTOR_STORE_WINDOW)
        {
            if matches!(probe.kind, K::Jal { .. }) {
                break;
            }
            if matches!(
                probe.kind,
                K::Sw {
                    base: Reg::V0,
                    offset: QUEST_SEQ_STREAM_FIELD_OFFSET,
                    ..
                }
            ) {
                stores.push((instruction.addr, probe.addr));
            }
        }
    }
    stores.sort_unstable();
    stores.dedup();
    stores
}

/// 지원 `QUEST.EXE`의 interpreter constructor stream root를 검증한다.
pub fn verify_box_quest_resident_seq_roots(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestResidentSeqRootSummary, QuestResidentSeqRootError> {
    if map.name != "QUEST.EXE" {
        return Err(error(format!(
            "QUEST resident SEQ root 사상 이름 불일치: {}",
            map.name
        )));
    }
    if text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST resident SEQ root text 길이 불일치: map={}, actual={}",
            map.text_len,
            text.len()
        )));
    }
    let instructions = decode_slice(map.load_addr, text)
        .ok_or_else(|| error("QUEST resident SEQ root text가 4바이트 정렬이 아님"))?;
    let selection = verify_root_selection(map, text, &instructions)?;
    let actual_stores = constructor_stream_stores(&instructions);
    let expected_stores = ROOT_INSTALL_SPECS
        .iter()
        .map(|spec| (spec.allocator_call_runtime, spec.stream_store_runtime))
        .collect::<Vec<_>>();
    if actual_stores != expected_stores {
        return Err(error(format!(
            "QUEST resident SEQ root constructor 분모 불일치: expected={expected_stores:08X?}, actual={actual_stores:08X?}"
        )));
    }

    let mut installs = Vec::with_capacity(ROOT_INSTALL_SPECS.len());
    for spec in ROOT_INSTALL_SPECS {
        require(
            &instructions,
            spec.callback_lui_runtime,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8002,
            },
            "interpreter callback lui",
        )?;
        require(
            &instructions,
            spec.callback_combine_runtime,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: (QUEST_SEQ_INTERPRETER_RUNTIME as u16) as i16,
            },
            "interpreter callback combine",
        )?;
        require(
            &instructions,
            spec.allocator_call_runtime,
            K::Jal {
                target: QUEST_OBJECT_ALLOCATE_RUNTIME,
            },
            "object allocator call",
        )?;

        let (source, literal_root_runtime, store_register) = match spec.source {
            RootSourceSpec::Literal {
                lui_runtime,
                combine_runtime,
                root_runtime,
            } => {
                require(
                    &instructions,
                    lui_runtime,
                    K::Lui {
                        rt: Reg::V1,
                        imm: ((root_runtime.wrapping_add(0x8000)) >> 16) as u16,
                    },
                    "literal root lui",
                )?;
                require(
                    &instructions,
                    combine_runtime,
                    K::Addiu {
                        rt: Reg::V1,
                        rs: Reg::V1,
                        imm: root_runtime as u16 as i16,
                    },
                    "literal root combine",
                )?;
                if !map.contains(root_runtime) {
                    return Err(error(format!(
                        "QUEST resident literal SEQ root가 EXE text 밖임: 0x{root_runtime:08X}"
                    )));
                }
                (
                    QuestResidentSeqRootSource::LiteralExeStream,
                    Some(root_runtime),
                    Reg::V1,
                )
            }
            RootSourceSpec::CurrentRecordPointer {
                record_load_runtime,
                pointer_load_runtime,
            } => {
                require(
                    &instructions,
                    record_load_runtime,
                    K::Lw {
                        rt: Reg::V1,
                        base: Reg::S0,
                        offset: QUEST_SEQ_STREAM_FIELD_OFFSET,
                    },
                    "current record load",
                )?;
                require(
                    &instructions,
                    pointer_load_runtime,
                    K::Lw {
                        rt: Reg::V1,
                        base: Reg::V1,
                        offset: 4,
                    },
                    "current record pointer load",
                )?;
                (
                    QuestResidentSeqRootSource::CurrentRecordPointer,
                    None,
                    Reg::V1,
                )
            }
            RootSourceSpec::ForwardedObjectPointer {
                pointer_load_runtime,
            } => {
                require(
                    &instructions,
                    pointer_load_runtime,
                    K::Lw {
                        rt: Reg::A1,
                        base: Reg::S5,
                        offset: 0x10,
                    },
                    "forwarded object pointer load",
                )?;
                (
                    QuestResidentSeqRootSource::ForwardedObjectPointer,
                    None,
                    Reg::A1,
                )
            }
        };
        require(
            &instructions,
            spec.stream_store_runtime,
            K::Sw {
                rt: store_register,
                base: Reg::V0,
                offset: QUEST_SEQ_STREAM_FIELD_OFFSET,
            },
            "interpreter stream store",
        )?;
        installs.push(QuestResidentSeqRootInstall {
            allocator_call_runtime: spec.allocator_call_runtime,
            stream_store_runtime: spec.stream_store_runtime,
            source,
            literal_root_runtime,
        });
    }

    let literal_roots = installs
        .iter()
        .filter_map(|install| install.literal_root_runtime)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let literal_install_sites = installs
        .iter()
        .filter(|install| install.source == QuestResidentSeqRootSource::LiteralExeStream)
        .count();
    let current_record_pointer_sites = installs
        .iter()
        .filter(|install| install.source == QuestResidentSeqRootSource::CurrentRecordPointer)
        .count();
    let current_record_pointer_handler_runtime =
        quest_seq_opcode_handler(QUEST_RESIDENT_CURRENT_RECORD_OPCODE)
            .ok_or_else(|| error("QUEST current-record constructor의 OP1C handler가 없음"))?;
    let next_handler_runtime =
        quest_seq_opcode_handler(QUEST_RESIDENT_CURRENT_RECORD_OPCODE + 1)
            .ok_or_else(|| error("QUEST current-record constructor 다음 handler가 없음"))?;
    let current_record_install = installs
        .iter()
        .find(|install| install.source == QuestResidentSeqRootSource::CurrentRecordPointer)
        .ok_or_else(|| error("QUEST current-record constructor가 없음"))?;
    if current_record_pointer_handler_runtime != 0x8002_2C7C
        || current_record_install.allocator_call_runtime < current_record_pointer_handler_runtime
        || current_record_install.stream_store_runtime >= next_handler_runtime
    {
        return Err(error(format!(
            "QUEST current-record constructor의 OP1C 소유 경계 불일치: handler=0x{current_record_pointer_handler_runtime:08X}, allocator=0x{:08X}, store=0x{:08X}, next=0x{next_handler_runtime:08X}",
            current_record_install.allocator_call_runtime,
            current_record_install.stream_store_runtime,
        )));
    }
    let forwarded_object_pointer_sites = installs
        .iter()
        .filter(|install| install.source == QuestResidentSeqRootSource::ForwardedObjectPointer)
        .count();
    if literal_install_sites != 6
        || literal_roots.len() != 4
        || current_record_pointer_sites != 1
        || forwarded_object_pointer_sites != 1
    {
        return Err(error(format!(
            "QUEST resident SEQ root 분류 불일치: literal={literal_install_sites}/{}, record={current_record_pointer_sites}, forwarded={forwarded_object_pointer_sites}",
            literal_roots.len()
        )));
    }
    for (role, stream_store_runtime, root_runtime) in [
        (
            "farewell",
            selection.farewell_stream_store_runtime,
            selection.farewell_root_runtime,
        ),
        (
            "defeat",
            selection.defeat_stream_store_runtime,
            selection.defeat_root_runtime,
        ),
    ] {
        if !installs.iter().any(|install| {
            install.stream_store_runtime == stream_store_runtime
                && install.literal_root_runtime == Some(root_runtime)
        }) {
            return Err(error(format!(
                "QUEST resident SEQ root {role} selector와 constructor 결속 불일치"
            )));
        }
    }

    Ok(QuestResidentSeqRootSummary {
        installs,
        selection,
        literal_install_sites,
        literal_roots,
        current_record_pointer_sites,
        current_record_pointer_opcode: QUEST_RESIDENT_CURRENT_RECORD_OPCODE,
        current_record_pointer_handler_runtime,
        forwarded_object_pointer_sites,
    })
}

#[cfg(test)]
#[path = "quest_resident_seq_roots_tests.rs"]
mod tests;
