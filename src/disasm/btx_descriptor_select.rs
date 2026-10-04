//! pending 일본어 BTX를 고르는 descriptor 선택·복사 경계.
//!
//! 여덟 EXE는 resource ID에 36을 곱해 개별 descriptor를 고르고, 직전에
//! 사용한 descriptor와 다를 때만 36바이트 working descriptor로 복사한다.
//! descriptor block 초기화 참조는 개별 자산 소비 증거가 아니므로 이 경계와
//! 분리한다. 정확한 descriptor `+4` read PC는 runtime breakpoint의 PC
//! filter로 쓸 수 있지만, cache hit는 복사를 건너뛰므로 미관측을 미소비로
//! 판정할 수 없다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{linear, trace};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BtxDescriptorSelectionFamily {
    Shared,
    Pcomi,
}

/// pending BTX가 속한 EXE·GRP의 검토된 선택 경계.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxDescriptorSelectionSpec {
    pub exe: &'static str,
    pub archive: &'static str,
    pub resource_multiply_runtime: u32,
    pub cache_check_runtime: u32,
    pub descriptor_copy_runtime: u32,
    pub working_descriptor_runtime: u32,
    family: BtxDescriptorSelectionFamily,
}

/// 정적 검산을 통과한 개별 descriptor 선택 경계.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxDescriptorSelection {
    pub exe: &'static str,
    pub archive: &'static str,
    pub resource_multiply_runtime: u32,
    pub descriptor_select_runtime: u32,
    pub cache_check_runtime: u32,
    pub descriptor_copy_runtime: u32,
    pub descriptor_name_field_read_runtime: u32,
    pub descriptor_copy_end_runtime: u32,
    pub descriptor_copy_source_register: Reg,
    pub working_descriptor_runtime: u32,
    pub entry_reached: bool,
}

/// pending BTX를 덮는 EXE별 선택 경계 집합.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxDescriptorSelectionAnalysis {
    pub selections: Vec<BtxDescriptorSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxDescriptorSelectionError(String);

impl fmt::Display for BtxDescriptorSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for BtxDescriptorSelectionError {}

fn error(message: impl Into<String>) -> BtxDescriptorSelectionError {
    BtxDescriptorSelectionError(message.into())
}

const fn selection(
    exe: &'static str,
    archive: &'static str,
    resource_multiply_runtime: u32,
    cache_check_runtime: u32,
    descriptor_copy_runtime: u32,
    working_descriptor_runtime: u32,
    family: BtxDescriptorSelectionFamily,
) -> BtxDescriptorSelectionSpec {
    BtxDescriptorSelectionSpec {
        exe,
        archive,
        resource_multiply_runtime,
        cache_check_runtime,
        descriptor_copy_runtime,
        working_descriptor_runtime,
        family,
    }
}

/// 양성 BTX 조사에서 descriptor 선택 구조를 확정한 여덟 실행 경계.
pub const BOX_PENDING_BTX_DESCRIPTOR_SELECTIONS: [BtxDescriptorSelectionSpec; 8] = [
    selection(
        "GOCHA1.EXE",
        "/GOCHA1/TUU_GRP.VDD;1",
        0x8001_6180,
        0x8001_61A0,
        0x8001_61B4,
        0x800B_D208,
        BtxDescriptorSelectionFamily::Shared,
    ),
    selection(
        "GOCHA2.EXE",
        "/GOCHA2/GOTY_GRP.VDD;1",
        0x8001_E6E0,
        0x8001_E700,
        0x8001_E714,
        0x800A_2CB0,
        BtxDescriptorSelectionFamily::Shared,
    ),
    selection(
        "HORI2.EXE",
        "/HORI2/HORI2GRP.VDD;1",
        0x8001_E70C,
        0x8001_E72C,
        0x8001_E740,
        0x800A_3360,
        BtxDescriptorSelectionFamily::Shared,
    ),
    selection(
        "HORI4.EXE",
        "/HORI4/HORI4GRP.VDD;1",
        0x8001_E6DC,
        0x8001_E6FC,
        0x8001_E710,
        0x800A_3608,
        BtxDescriptorSelectionFamily::Shared,
    ),
    selection(
        "MINPUYO.EXE",
        "/MINPUYO/MIN_GRP.VDD;1",
        0x8001_E6C0,
        0x8001_E6E0,
        0x8001_E6F4,
        0x800A_32D8,
        BtxDescriptorSelectionFamily::Shared,
    ),
    selection(
        "PCOMI.EXE",
        "/PCOMI/PC1_GRP.VDD;1",
        0x8001_2A98,
        0x8001_2ABC,
        0x8001_2AD4,
        0x8007_6D18,
        BtxDescriptorSelectionFamily::Pcomi,
    ),
    selection(
        "PUYO1.EXE",
        "/PUYO1/PP1_GRP.VDD;1",
        0x8001_AEFC,
        0x8001_AF1C,
        0x8001_AF30,
        0x800B_37C0,
        BtxDescriptorSelectionFamily::Shared,
    ),
    selection(
        "PUYO2.EXE",
        "/PUYO2/PP2_GRP.VDD;1",
        0x8001_9988,
        0x8001_99A8,
        0x8001_99BC,
        0x800B_5BA0,
        BtxDescriptorSelectionFamily::Shared,
    ),
];

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, BtxDescriptorSelectionError> {
    instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| {
            error(format!(
                "BTX descriptor 선택 명령 주소 없음: 0x{address:08X}"
            ))
        })
}

fn require(
    instructions: &[Instruction],
    exe: &str,
    address: u32,
    expected: K,
) -> Result<(), BtxDescriptorSelectionError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "{exe} BTX descriptor 선택 명령 불일치: address=0x{address:08X}, \
             expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn expected_selection_sequence(family: BtxDescriptorSelectionFamily) -> [K; 5] {
    match family {
        BtxDescriptorSelectionFamily::Shared => [
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A0,
                sa: 3,
            },
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
            K::Lw {
                rt: Reg::V1,
                base: Reg::V1,
                offset: 0,
            },
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
            K::Addu {
                rd: Reg::S1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ],
        BtxDescriptorSelectionFamily::Pcomi => [
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A1,
                sa: 3,
            },
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A1,
            },
            K::Lw {
                rt: Reg::A0,
                base: Reg::V1,
                offset: 0,
            },
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
            K::Addu {
                rd: Reg::S1,
                rs: Reg::A0,
                rt: Reg::V0,
            },
        ],
    }
}

fn expected_copy_sequence(family: BtxDescriptorSelectionFamily) -> [K; 14] {
    let (source, destination, stop, first) = match family {
        BtxDescriptorSelectionFamily::Shared => (Reg::V1, Reg::V0, Reg::A0, Reg::A2),
        BtxDescriptorSelectionFamily::Pcomi => (Reg::A0, Reg::A1, Reg::A2, Reg::V1),
    };
    [
        K::Lw {
            rt: first,
            base: source,
            offset: 0,
        },
        K::Lw {
            rt: Reg::A3,
            base: source,
            offset: 4,
        },
        K::Lw {
            rt: Reg::T0,
            base: source,
            offset: 8,
        },
        K::Lw {
            rt: Reg::T1,
            base: source,
            offset: 12,
        },
        K::Sw {
            rt: first,
            base: destination,
            offset: 0,
        },
        K::Sw {
            rt: Reg::A3,
            base: destination,
            offset: 4,
        },
        K::Sw {
            rt: Reg::T0,
            base: destination,
            offset: 8,
        },
        K::Sw {
            rt: Reg::T1,
            base: destination,
            offset: 12,
        },
        K::Addiu {
            rt: source,
            rs: source,
            imm: 16,
        },
        K::Bne {
            rs: source,
            rt: stop,
            offset: -10,
        },
        K::Addiu {
            rt: destination,
            rs: destination,
            imm: 16,
        },
        K::Lw {
            rt: first,
            base: source,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Sw {
            rt: first,
            base: destination,
            offset: 0,
        },
    ]
}

fn expected_copy_setup(family: BtxDescriptorSelectionFamily) -> [K; 2] {
    match family {
        BtxDescriptorSelectionFamily::Shared => [
            K::Addu {
                rd: Reg::V1,
                rs: Reg::S1,
                rt: Reg::ZERO,
            },
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::S1,
                imm: 32,
            },
        ],
        BtxDescriptorSelectionFamily::Pcomi => [
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S1,
                rt: Reg::ZERO,
            },
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::S1,
                imm: 32,
            },
        ],
    }
}

fn verify_one(
    spec: BtxDescriptorSelectionSpec,
    map: &ExeMap,
    text: &[u8],
) -> Result<BtxDescriptorSelection, BtxDescriptorSelectionError> {
    if text.len() != map.text_len as usize {
        return Err(error(format!(
            "{} text 길이 불일치: expected={}, actual={}",
            spec.exe,
            map.text_len,
            text.len()
        )));
    }
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error(format!("{} text 길이가 4바이트 정렬이 아님", spec.exe)))?;

    for (index, expected) in expected_selection_sequence(spec.family)
        .into_iter()
        .enumerate()
    {
        require(
            &instructions,
            spec.exe,
            spec.resource_multiply_runtime + index as u32 * 4,
            expected,
        )?;
    }
    require(
        &instructions,
        spec.exe,
        spec.cache_check_runtime,
        K::Beq {
            rs: match spec.family {
                BtxDescriptorSelectionFamily::Shared => Reg::V0,
                BtxDescriptorSelectionFamily::Pcomi => Reg::A0,
            },
            rt: Reg::S1,
            offset: 29,
        },
    )?;
    for (index, expected) in expected_copy_setup(spec.family).into_iter().enumerate() {
        require(
            &instructions,
            spec.exe,
            spec.descriptor_copy_runtime - 8 + index as u32 * 4,
            expected,
        )?;
    }
    let copy_sequence = expected_copy_sequence(spec.family);
    for (index, expected) in copy_sequence.into_iter().enumerate() {
        require(
            &instructions,
            spec.exe,
            spec.descriptor_copy_runtime + index as u32 * 4,
            expected,
        )?;
    }

    let reached = trace(map, text, &[]).instructions;
    let required_addresses = (0..5)
        .map(|index| spec.resource_multiply_runtime + index * 4)
        .chain([spec.cache_check_runtime])
        .chain((0..2).map(|index| spec.descriptor_copy_runtime - 8 + index * 4))
        .chain(
            (0..copy_sequence.len() as u32).map(|index| spec.descriptor_copy_runtime + index * 4),
        )
        .collect::<BTreeSet<_>>();
    let entry_reached = required_addresses
        .iter()
        .all(|address| reached.contains_key(address));

    let working_load_matches = match spec.family {
        BtxDescriptorSelectionFamily::Shared => {
            let high = ((spec.working_descriptor_runtime.wrapping_add(0x8000)) >> 16) as u16;
            let low = spec.working_descriptor_runtime as u16 as i16;
            instruction_at(&instructions, spec.cache_check_runtime + 4)?.kind
                == K::Lui {
                    rt: Reg::V0,
                    imm: high,
                }
                && instruction_at(&instructions, spec.cache_check_runtime + 8)?.kind
                    == K::Addiu {
                        rt: Reg::V0,
                        rs: Reg::V0,
                        imm: low,
                    }
        }
        BtxDescriptorSelectionFamily::Pcomi => {
            let high = ((spec.working_descriptor_runtime.wrapping_add(0x8000)) >> 16) as u16;
            let low = spec.working_descriptor_runtime as u16 as i16;
            instruction_at(&instructions, spec.descriptor_copy_runtime - 16)?.kind
                == K::Lui {
                    rt: Reg::V0,
                    imm: high,
                }
                && instruction_at(&instructions, spec.descriptor_copy_runtime - 12)?.kind
                    == K::Addiu {
                        rt: Reg::A1,
                        rs: Reg::V0,
                        imm: low,
                    }
        }
    };
    if !working_load_matches {
        return Err(error(format!(
            "{} working descriptor 주소 load 불일치: expected=0x{:08X}",
            spec.exe, spec.working_descriptor_runtime
        )));
    }

    Ok(BtxDescriptorSelection {
        exe: spec.exe,
        archive: spec.archive,
        resource_multiply_runtime: spec.resource_multiply_runtime,
        descriptor_select_runtime: spec.resource_multiply_runtime + 16,
        cache_check_runtime: spec.cache_check_runtime,
        descriptor_copy_runtime: spec.descriptor_copy_runtime,
        descriptor_name_field_read_runtime: spec.descriptor_copy_runtime + 4,
        descriptor_copy_end_runtime: spec.descriptor_copy_runtime + 0x34,
        descriptor_copy_source_register: match spec.family {
            BtxDescriptorSelectionFamily::Shared => Reg::V1,
            BtxDescriptorSelectionFamily::Pcomi => Reg::A0,
        },
        working_descriptor_runtime: spec.working_descriptor_runtime,
        entry_reached,
    })
}

fn audit_btx_descriptor_selections<'a>(
    specs: &[BtxDescriptorSelectionSpec],
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
) -> Result<BtxDescriptorSelectionAnalysis, BtxDescriptorSelectionError> {
    if specs.is_empty() {
        return Err(error("BTX descriptor 선택 spec이 없음"));
    }
    let mut spec_executables = BTreeSet::new();
    let mut spec_archives = BTreeSet::new();
    for spec in specs {
        if !spec_executables.insert(spec.exe) {
            return Err(error(format!(
                "BTX descriptor 선택 EXE spec 중복: {}",
                spec.exe
            )));
        }
        if !spec_archives.insert(spec.archive) {
            return Err(error(format!(
                "BTX descriptor 선택 archive spec 중복: {}",
                spec.archive
            )));
        }
    }
    let mut inputs = BTreeMap::new();
    for (map, text) in executables {
        if inputs.insert(map.name, (map, text)).is_some() {
            return Err(error(format!(
                "BTX descriptor 선택 EXE 입력 중복: {}",
                map.name
            )));
        }
    }

    let mut selections = Vec::with_capacity(specs.len());
    for &spec in specs {
        let &(map, text) = inputs
            .get(spec.exe)
            .ok_or_else(|| error(format!("BTX descriptor 선택 EXE 입력 누락: {}", spec.exe)))?;
        selections.push(verify_one(spec, map, text)?);
    }
    selections.sort_by_key(|selection| selection.exe);
    Ok(BtxDescriptorSelectionAnalysis { selections })
}

/// 지원 원본의 pending BTX 여덟 EXE별 선택·복사 경계를 검산한다.
pub fn verify_box_btx_descriptor_selections<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
) -> Result<BtxDescriptorSelectionAnalysis, BtxDescriptorSelectionError> {
    let analysis =
        audit_btx_descriptor_selections(&BOX_PENDING_BTX_DESCRIPTOR_SELECTIONS, executables)?;
    if analysis.selections.len() != BOX_PENDING_BTX_DESCRIPTOR_SELECTIONS.len() {
        return Err(error(format!(
            "지원 원본 BTX descriptor 선택 경계 수 불일치: expected={}, actual={}",
            BOX_PENDING_BTX_DESCRIPTOR_SELECTIONS.len(),
            analysis.selections.len()
        )));
    }
    Ok(analysis)
}

#[cfg(test)]
#[path = "btx_descriptor_select_tests.rs"]
mod btx_descriptor_select_tests;
