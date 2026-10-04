//! `QUEST.EXE` resident root selector의 direct producer와 consumer.
//!
//! selector 값을 장면 의미로 추측하지 않고, 지원 원본에서 직접 주소를
//! 읽고 쓰는 MIPS access와 값 5를 만드는 두 경로를 분리해 검증한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{decode_slice, Instruction, InstructionKind as K, Reg};

use super::quest_resident_seq_roots::QuestResidentSeqRootSelection;
use super::xref::{find_address_loads, LoadForm, DEFAULT_WINDOW};
use super::QUEST_RESIDENT_DEFEAT_PREREQUISITE_RUNTIME;

const QUEST_RESIDENT_SELECTOR_RUNTIME: u32 = 0x800F_BDD4;
const QUEST_RESIDENT_DEFEAT_SELECTOR: u8 = 5;
const QUEST_RESIDENT_SELECTOR_PREDICATE_RUNTIME: u32 = 0x8005_2DF8;
const QUEST_RESIDENT_STANDALONE_DEFEAT_CONSTRUCTOR_RUNTIME: u32 = 0x8007_7A90;
const EXPECTED_SELECTOR_READS: [u32; 2] = [0x8005_5274, 0x8005_8040];
const EXPECTED_SELECTOR_WRITES: [u32; 8] = [
    0x8005_35AC,
    0x8005_4478,
    0x8005_449C,
    0x8005_4DE4,
    0x8005_72BC,
    0x8007_3F0C,
    0x8007_3FB0,
    0x8007_408C,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestResidentSeqSelectorWriteValue {
    Constant(u8),
    RuntimeRegister,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestResidentSeqSelectorWrite {
    pub store_runtime: u32,
    pub value: QuestResidentSeqSelectorWriteValue,
}

/// 값 5 store 앞에서 확인한 두 direct 조건 경로.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestResidentSeqDefeatSelectorWriter {
    pub callback_runtime: u32,
    pub predicate_call_runtime: u32,
    pub prerequisite_load_runtime: u32,
    pub selector_store_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqSelectorSummary {
    pub selector_runtime: u32,
    pub reads: Vec<u32>,
    pub writes: Vec<QuestResidentSeqSelectorWrite>,
    pub defeat_writers: Vec<QuestResidentSeqDefeatSelectorWriter>,
    pub defeat_prerequisite_runtime: u32,
    pub standalone_defeat_constructor_runtime: u32,
    pub standalone_direct_control_refs: Vec<u32>,
    pub standalone_address_load_refs: Vec<u32>,
    pub standalone_aligned_pointer_refs: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqSelectorError(String);

impl fmt::Display for QuestResidentSeqSelectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentSeqSelectorError {}

fn error(message: impl Into<String>) -> QuestResidentSeqSelectorError {
    QuestResidentSeqSelectorError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    runtime: u32,
) -> Result<&Instruction, QuestResidentSeqSelectorError> {
    instructions
        .binary_search_by_key(&runtime, |instruction| instruction.addr)
        .ok()
        .and_then(|index| instructions.get(index))
        .ok_or_else(|| {
            error(format!(
                "QUEST resident selector 명령이 없음: 0x{runtime:08X}"
            ))
        })
}

fn require(
    instructions: &[Instruction],
    runtime: u32,
    expected: K,
    role: &str,
) -> Result<(), QuestResidentSeqSelectorError> {
    let actual = instruction_at(instructions, runtime)?;
    if actual.kind != expected {
        return Err(error(format!(
            "QUEST resident selector {role} drift: 0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct SelectorWriteSpec {
    store_runtime: u32,
    source_register: Reg,
    base_register: Reg,
    value: QuestResidentSeqSelectorWriteValue,
    constant_load_runtime: Option<u32>,
}

const SELECTOR_WRITE_SPECS: [SelectorWriteSpec; 8] = [
    SelectorWriteSpec {
        store_runtime: 0x8005_35AC,
        source_register: Reg::V1,
        base_register: Reg::A1,
        value: QuestResidentSeqSelectorWriteValue::Constant(3),
        constant_load_runtime: Some(0x8005_35A8),
    },
    SelectorWriteSpec {
        store_runtime: 0x8005_4478,
        source_register: Reg::V0,
        base_register: Reg::A1,
        value: QuestResidentSeqSelectorWriteValue::Constant(4),
        constant_load_runtime: Some(0x8005_4474),
    },
    SelectorWriteSpec {
        store_runtime: 0x8005_449C,
        source_register: Reg::S1,
        base_register: Reg::V0,
        value: QuestResidentSeqSelectorWriteValue::RuntimeRegister,
        constant_load_runtime: None,
    },
    SelectorWriteSpec {
        store_runtime: 0x8005_4DE4,
        source_register: Reg::V1,
        base_register: Reg::V0,
        value: QuestResidentSeqSelectorWriteValue::Constant(5),
        constant_load_runtime: Some(0x8005_4DE0),
    },
    SelectorWriteSpec {
        store_runtime: 0x8005_72BC,
        source_register: Reg::V1,
        base_register: Reg::V0,
        value: QuestResidentSeqSelectorWriteValue::Constant(5),
        constant_load_runtime: Some(0x8005_72B8),
    },
    SelectorWriteSpec {
        store_runtime: 0x8007_3F0C,
        source_register: Reg::ZERO,
        base_register: Reg::V0,
        value: QuestResidentSeqSelectorWriteValue::Constant(0),
        constant_load_runtime: None,
    },
    SelectorWriteSpec {
        store_runtime: 0x8007_3FB0,
        source_register: Reg::ZERO,
        base_register: Reg::V0,
        value: QuestResidentSeqSelectorWriteValue::Constant(0),
        constant_load_runtime: None,
    },
    SelectorWriteSpec {
        store_runtime: 0x8007_408C,
        source_register: Reg::ZERO,
        base_register: Reg::V0,
        value: QuestResidentSeqSelectorWriteValue::Constant(0),
        constant_load_runtime: None,
    },
];

#[derive(Debug, Clone, Copy)]
struct DefeatWriterSpec {
    callback_runtime: u32,
    predicate_call_runtime: u32,
    predicate_mask_runtime: u32,
    predicate_branch_runtime: u32,
    predicate_branch_offset: i16,
    prerequisite_lui_runtime: u32,
    prerequisite_load_runtime: u32,
    prerequisite_branch_runtime: u32,
    prerequisite_branch_offset: i16,
    selector_lui_runtime: u32,
    selector_store_runtime: u32,
}

const DEFEAT_WRITER_SPECS: [DefeatWriterSpec; 2] = [
    DefeatWriterSpec {
        callback_runtime: 0x8005_4744,
        predicate_call_runtime: 0x8005_4DBC,
        predicate_mask_runtime: 0x8005_4DC4,
        predicate_branch_runtime: 0x8005_4DC8,
        predicate_branch_offset: 235,
        prerequisite_lui_runtime: 0x8005_4DCC,
        prerequisite_load_runtime: 0x8005_4DD0,
        prerequisite_branch_runtime: 0x8005_4DD8,
        prerequisite_branch_offset: 230,
        selector_lui_runtime: 0x8005_4DDC,
        selector_store_runtime: 0x8005_4DE4,
    },
    DefeatWriterSpec {
        callback_runtime: 0x8005_5F44,
        predicate_call_runtime: 0x8005_7294,
        predicate_mask_runtime: 0x8005_729C,
        predicate_branch_runtime: 0x8005_72A0,
        predicate_branch_offset: 719,
        prerequisite_lui_runtime: 0x8005_72A4,
        prerequisite_load_runtime: 0x8005_72A8,
        prerequisite_branch_runtime: 0x8005_72B0,
        prerequisite_branch_offset: -1128,
        selector_lui_runtime: 0x8005_72B4,
        selector_store_runtime: 0x8005_72BC,
    },
];

fn selector_direct_accesses(
    instructions: &[Instruction],
) -> Result<(Vec<u32>, Vec<u32>), QuestResidentSeqSelectorError> {
    let access_sites = find_address_loads(instructions, DEFAULT_WINDOW)
        .into_iter()
        .filter(|load| {
            load.value == QUEST_RESIDENT_SELECTOR_RUNTIME && load.form == LoadForm::MemoryOffset
        })
        .map(|load| load.combine_addr)
        .collect::<BTreeSet<_>>();
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    for runtime in access_sites {
        match instruction_at(instructions, runtime)?.kind {
            K::Lbu { .. } => reads.push(runtime),
            K::Sb { .. } => writes.push(runtime),
            ref other => {
                return Err(error(format!(
                    "QUEST resident selector direct access 미분류: 0x{runtime:08X} {other:?}"
                )))
            }
        }
    }
    Ok((reads, writes))
}

fn standalone_constructor_refs(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let direct_control = instructions
        .iter()
        .filter_map(|instruction| match instruction.kind {
            K::J { target } | K::Jal { target }
                if target == QUEST_RESIDENT_STANDALONE_DEFEAT_CONSTRUCTOR_RUNTIME =>
            {
                Some(instruction.addr)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let address_loads = find_address_loads(instructions, DEFAULT_WINDOW)
        .into_iter()
        .filter(|load| load.value == QUEST_RESIDENT_STANDALONE_DEFEAT_CONSTRUCTOR_RUNTIME)
        .map(|load| load.lui_addr)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let aligned_pointers = text
        .chunks_exact(4)
        .enumerate()
        .filter_map(|(index, bytes)| {
            (u32::from_le_bytes(bytes.try_into().expect("4바이트 chunk"))
                == QUEST_RESIDENT_STANDALONE_DEFEAT_CONSTRUCTOR_RUNTIME)
                .then(|| map.text_runtime_addr(index * 4))
                .flatten()
        })
        .collect::<Vec<_>>();
    (direct_control, address_loads, aligned_pointers)
}

/// 지원 `QUEST.EXE`의 resident root selector direct 사용을 검증한다.
pub fn verify_box_quest_resident_seq_selector(
    map: &ExeMap,
    text: &[u8],
    selection: &QuestResidentSeqRootSelection,
) -> Result<QuestResidentSeqSelectorSummary, QuestResidentSeqSelectorError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error("QUEST resident selector source 입력 불일치"));
    }
    if selection.selector_runtime != QUEST_RESIDENT_SELECTOR_RUNTIME
        || selection.defeat_selector != QUEST_RESIDENT_DEFEAT_SELECTOR
    {
        return Err(error("QUEST resident selector root selection 결속 불일치"));
    }
    let instructions = decode_slice(map.load_addr, text)
        .ok_or_else(|| error("QUEST resident selector text가 4바이트 정렬이 아님"))?;
    let (reads, direct_writes) = selector_direct_accesses(&instructions)?;
    if reads != EXPECTED_SELECTOR_READS || direct_writes != EXPECTED_SELECTOR_WRITES {
        return Err(error(format!(
            "QUEST resident selector direct access 분모 불일치: reads={reads:08X?}, writes={direct_writes:08X?}"
        )));
    }

    let mut writes = Vec::with_capacity(SELECTOR_WRITE_SPECS.len());
    for spec in SELECTOR_WRITE_SPECS {
        require(
            &instructions,
            spec.store_runtime,
            K::Sb {
                rt: spec.source_register,
                base: spec.base_register,
                offset: QUEST_RESIDENT_SELECTOR_RUNTIME as u16 as i16,
            },
            "store",
        )?;
        if let (QuestResidentSeqSelectorWriteValue::Constant(value), Some(load_runtime)) =
            (spec.value, spec.constant_load_runtime)
        {
            require(
                &instructions,
                load_runtime,
                K::Addiu {
                    rt: spec.source_register,
                    rs: Reg::ZERO,
                    imm: i16::from(value),
                },
                "constant load",
            )?;
        }
        writes.push(QuestResidentSeqSelectorWrite {
            store_runtime: spec.store_runtime,
            value: spec.value,
        });
    }

    let mut defeat_writers = Vec::with_capacity(DEFEAT_WRITER_SPECS.len());
    for (runtime, expected, role) in [
        (
            0x8007_1CC8,
            K::Andi {
                rt: Reg::V1,
                rs: Reg::S3,
                imm: 0x00FF,
            },
            "callback selector mask",
        ),
        (
            0x8007_1CD0,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 1,
            },
            "callback selector value",
        ),
        (
            0x8007_1CD4,
            K::Bne {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 3,
            },
            "callback selector branch",
        ),
        (
            0x8007_1CD8,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8005,
            },
            "first callback lui",
        ),
        (
            0x8007_1CDC,
            K::J {
                target: 0x8007_1CEC,
            },
            "first callback join",
        ),
        (
            0x8007_1CE0,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x4744,
            },
            "first callback combine",
        ),
        (
            0x8007_1CE4,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8005,
            },
            "second callback lui",
        ),
        (
            0x8007_1CE8,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x5F44,
            },
            "second callback combine",
        ),
        (
            0x8007_1CEC,
            K::Jal {
                target: 0x8001_4B14,
            },
            "callback object allocator",
        ),
        (
            0x8007_1CF0,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 4,
            },
            "callback object type",
        ),
    ] {
        require(&instructions, runtime, expected, role)?;
    }
    for spec in DEFEAT_WRITER_SPECS {
        for (runtime, expected, role) in [
            (
                spec.predicate_call_runtime,
                K::Jal {
                    target: QUEST_RESIDENT_SELECTOR_PREDICATE_RUNTIME,
                },
                "defeat predicate call",
            ),
            (
                spec.predicate_mask_runtime,
                K::Andi {
                    rt: Reg::V0,
                    rs: Reg::V0,
                    imm: 0x00FF,
                },
                "defeat predicate mask",
            ),
            (
                spec.predicate_branch_runtime,
                K::Beq {
                    rs: Reg::V0,
                    rt: Reg::ZERO,
                    offset: spec.predicate_branch_offset,
                },
                "defeat predicate branch",
            ),
            (
                spec.prerequisite_lui_runtime,
                K::Lui {
                    rt: Reg::V0,
                    imm: 0x8010,
                },
                "defeat prerequisite lui",
            ),
            (
                spec.prerequisite_load_runtime,
                K::Lbu {
                    rt: Reg::V0,
                    base: Reg::V0,
                    offset: QUEST_RESIDENT_DEFEAT_PREREQUISITE_RUNTIME as u16 as i16,
                },
                "defeat prerequisite load",
            ),
            (
                spec.prerequisite_branch_runtime,
                K::Bne {
                    rs: Reg::V0,
                    rt: Reg::ZERO,
                    offset: spec.prerequisite_branch_offset,
                },
                "defeat prerequisite branch",
            ),
            (
                spec.selector_lui_runtime,
                K::Lui {
                    rt: Reg::V0,
                    imm: 0x8010,
                },
                "defeat selector lui",
            ),
        ] {
            require(&instructions, runtime, expected, role)?;
        }
        defeat_writers.push(QuestResidentSeqDefeatSelectorWriter {
            callback_runtime: spec.callback_runtime,
            predicate_call_runtime: spec.predicate_call_runtime,
            prerequisite_load_runtime: spec.prerequisite_load_runtime,
            selector_store_runtime: spec.selector_store_runtime,
        });
    }

    let (
        standalone_direct_control_refs,
        standalone_address_load_refs,
        standalone_aligned_pointer_refs,
    ) = standalone_constructor_refs(map, text, &instructions);
    if !standalone_direct_control_refs.is_empty()
        || !standalone_address_load_refs.is_empty()
        || !standalone_aligned_pointer_refs.is_empty()
    {
        return Err(error(format!(
            "QUEST standalone defeat constructor direct ref 분모 drift: control={standalone_direct_control_refs:08X?}, address={standalone_address_load_refs:08X?}, pointer={standalone_aligned_pointer_refs:08X?}"
        )));
    }

    Ok(QuestResidentSeqSelectorSummary {
        selector_runtime: QUEST_RESIDENT_SELECTOR_RUNTIME,
        reads,
        writes,
        defeat_writers,
        defeat_prerequisite_runtime: QUEST_RESIDENT_DEFEAT_PREREQUISITE_RUNTIME,
        standalone_defeat_constructor_runtime: QUEST_RESIDENT_STANDALONE_DEFEAT_CONSTRUCTOR_RUNTIME,
        standalone_direct_control_refs,
        standalone_address_load_refs,
        standalone_aligned_pointer_refs,
    })
}

#[cfg(test)]
#[path = "quest_resident_seq_selector_tests.rs"]
mod tests;
