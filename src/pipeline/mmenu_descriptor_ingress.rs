//! PUYO2 type table literal ingress 전수 경계.
//!
//! `MMENU.SEQ`나 `MM_MES.BTX`를 고를 수 있는 공용 type/resource selector의
//! literal root 조립을 지원 EXE 전체에서 센다. 이 분모는 계산 alias, runtime
//! code 변경, 간접 함수 호출이나 실제 선택·표시를 부정하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::coords::{normalize_kseg, ExeMap, PSX_EXE_HEADER_LEN};
use crate::disasm::subgame_seq_descriptor_ingress::{
    descriptor_resource_x36_select, descriptor_root_literal_combines, DescriptorRootLiteral,
};
use crate::disasm::{linear, xref};
use crate::isa::{decode, Instruction, InstructionKind as K, Reg};

const PUYO2_EXE: &str = "PUYO2.EXE";
const TYPE_TABLE_ROOT: u32 = 0x800B_6710;
const TYPE_TABLE_LITERAL: DescriptorRootLiteral = DescriptorRootLiteral::new(0x800B, 0x6710);
const ROOT_LOOKBACK: usize = 8;
const SELECTOR_SCAN_INSTRUCTIONS: usize = 25;

const MMENU_SEQUENCE_DESCRIPTOR: u32 = 0x8006_70DC;
const MM_MES_GRAPHICS_DESCRIPTOR: u32 = 0x8006_55D0;

const LITERAL_COMBINES: [u32; 15] = [
    0x8001_97C0,
    0x8001_97C4,
    0x8001_9970,
    0x8001_9974,
    0x8001_A04C,
    0x8001_B47C,
    0x8001_B72C,
    0x8001_B7E4,
    0x8001_C058,
    0x8001_C128,
    0x8001_C18C,
    0x8001_C558,
    0x8002_339C,
    0x8002_3578,
    0x8005_1568,
];

const RESOURCE_X36_SELECTORS: [u32; 11] = [
    0x8001_97C4,
    0x8001_9974,
    0x8001_B47C,
    0x8001_B72C,
    0x8001_B7E4,
    0x8001_C058,
    0x8001_C128,
    0x8001_C18C,
    0x8001_C558,
    0x8002_3578,
    0x8005_1568,
];

const ALTERNATE_BRANCH_COMBINES: [u32; 2] = [0x8001_97C0, 0x8001_9970];
const REGISTRATION_COMBINES: [u32; 2] = [0x8001_A04C, 0x8002_339C];
const REGISTRATION_STORES: [u32; 2] = [0x8001_A058, 0x8002_33B0];

const NO_REFERENCES: &[u32] = &[];
const SELECTOR_10_POINTERS: &[u32] = &[0x8008_B5DC];
const SELECTOR_12_CALLS: &[u32] = &[0x8001_B954, 0x8001_B9A0];
const SELECTOR_12_POINTERS: &[u32] = &[0x8008_B5E4];
const SELECTOR_01_POINTERS: &[u32] = &[0x8008_B5A0];
const SELECTOR_03_POINTERS: &[u32] = &[0x8008_B5A8];
const SELECTOR_04_POINTERS: &[u32] = &[0x8008_B5AC];
const SELECTOR_39_POINTERS: &[u32] = &[0x8008_B824];
const SELECTOR_TABLE_3_POINTERS: &[u32] = &[0x8008_D718];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectorReferenceSpec {
    combine_runtime: u32,
    entry_runtime: u32,
    direct_jal_locations: &'static [u32],
    raw_pointer_locations: &'static [u32],
}

const SELECTOR_REFERENCE_SPECS: [SelectorReferenceSpec; 11] = [
    SelectorReferenceSpec {
        combine_runtime: 0x8001_97C4,
        entry_runtime: 0x8001_9780,
        direct_jal_locations: NO_REFERENCES,
        raw_pointer_locations: SELECTOR_10_POINTERS,
    },
    SelectorReferenceSpec {
        combine_runtime: 0x8001_9974,
        entry_runtime: 0x8001_9930,
        direct_jal_locations: SELECTOR_12_CALLS,
        raw_pointer_locations: SELECTOR_12_POINTERS,
    },
    SelectorReferenceSpec {
        combine_runtime: 0x8001_B47C,
        entry_runtime: 0x8001_B474,
        direct_jal_locations: NO_REFERENCES,
        raw_pointer_locations: NO_REFERENCES,
    },
    SelectorReferenceSpec {
        combine_runtime: 0x8001_B72C,
        entry_runtime: 0x8001_B724,
        direct_jal_locations: NO_REFERENCES,
        raw_pointer_locations: NO_REFERENCES,
    },
    SelectorReferenceSpec {
        combine_runtime: 0x8001_B7E4,
        entry_runtime: 0x8001_B7D4,
        direct_jal_locations: NO_REFERENCES,
        raw_pointer_locations: NO_REFERENCES,
    },
    SelectorReferenceSpec {
        combine_runtime: 0x8001_C058,
        entry_runtime: 0x8001_C040,
        direct_jal_locations: NO_REFERENCES,
        raw_pointer_locations: SELECTOR_01_POINTERS,
    },
    SelectorReferenceSpec {
        combine_runtime: 0x8001_C128,
        entry_runtime: 0x8001_C110,
        direct_jal_locations: NO_REFERENCES,
        raw_pointer_locations: SELECTOR_03_POINTERS,
    },
    SelectorReferenceSpec {
        combine_runtime: 0x8001_C18C,
        entry_runtime: 0x8001_C174,
        direct_jal_locations: NO_REFERENCES,
        raw_pointer_locations: SELECTOR_04_POINTERS,
    },
    SelectorReferenceSpec {
        combine_runtime: 0x8001_C558,
        entry_runtime: 0x8001_C554,
        direct_jal_locations: NO_REFERENCES,
        raw_pointer_locations: NO_REFERENCES,
    },
    SelectorReferenceSpec {
        combine_runtime: 0x8002_3578,
        entry_runtime: 0x8002_354C,
        direct_jal_locations: NO_REFERENCES,
        raw_pointer_locations: SELECTOR_39_POINTERS,
    },
    SelectorReferenceSpec {
        combine_runtime: 0x8005_1568,
        entry_runtime: 0x8005_1550,
        direct_jal_locations: NO_REFERENCES,
        raw_pointer_locations: SELECTOR_TABLE_3_POINTERS,
    },
];

/// selector entry 하나의 초기 EXE 정적 참조.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmenuDescriptorSelectorReference {
    pub combine_runtime: u32,
    pub entry_runtime: u32,
    pub direct_jal_locations: Vec<u32>,
    pub raw_pointer_locations: Vec<u32>,
    pub address_load_locations: Vec<u32>,
}

impl MmenuDescriptorSelectorReference {
    fn is_statically_referenced(&self) -> bool {
        !self.direct_jal_locations.is_empty()
            || !self.raw_pointer_locations.is_empty()
            || !self.address_load_locations.is_empty()
    }
}

/// PUYO2 type table literal ingress의 지원 원본 census.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmenuDescriptorIngressAnalysis {
    pub type_table_root: u32,
    pub literal_combines: Vec<u32>,
    pub resource_x36_selectors: Vec<u32>,
    pub alternate_branch_combines: Vec<u32>,
    pub registration_stores: Vec<u32>,
    pub selector_references: Vec<MmenuDescriptorSelectorReference>,
    pub statically_referenced_selectors: usize,
    pub statically_unreferenced_selectors: usize,
    pub mmenu_descriptor_direct_address_loads: usize,
    pub mmenu_descriptor_raw_pointer_words: usize,
    pub mm_mes_descriptor_direct_address_loads: usize,
    pub mm_mes_descriptor_raw_pointer_words: usize,
}

/// PUYO2 type table literal ingress source gate 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmenuDescriptorIngressError(String);

impl fmt::Display for MmenuDescriptorIngressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for MmenuDescriptorIngressError {}

fn error(message: impl Into<String>) -> MmenuDescriptorIngressError {
    MmenuDescriptorIngressError(message.into())
}

fn executable_word(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
) -> Result<u32, MmenuDescriptorIngressError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "PUYO2 descriptor ingress runtime 주소가 EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    let bytes = text.get(offset..offset + 4).ok_or_else(|| {
        error(format!(
            "PUYO2 descriptor ingress EXE text 범위 이탈: 0x{runtime:08X}"
        ))
    })?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), MmenuDescriptorIngressError> {
    let actual = decode(runtime, executable_word(map, text, runtime)?);
    if actual.kind != expected {
        return Err(error(format!(
            "PUYO2 descriptor ingress typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn verify_alternate_branches(map: &ExeMap, text: &[u8]) -> Result<(), MmenuDescriptorIngressError> {
    for runtime in ALTERNATE_BRANCH_COMBINES {
        require_instruction(
            map,
            text,
            runtime - 4,
            K::J {
                target: runtime + 0x10,
            },
        )?;
    }
    Ok(())
}

fn verify_registration_stores(
    map: &ExeMap,
    text: &[u8],
) -> Result<(), MmenuDescriptorIngressError> {
    for (runtime, expected) in [
        (
            0x8001_A044,
            K::Lui {
                rt: Reg::V1,
                imm: 0x800B,
            },
        ),
        (
            0x8001_A04C,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x6710,
            },
        ),
        (
            0x8001_A050,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A3,
                sa: 2,
            },
        ),
        (
            0x8001_A054,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8001_A058,
            K::Sw {
                rt: Reg::FP,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8002_3390,
            K::Lui {
                rt: Reg::V1,
                imm: 0x800B,
            },
        ),
        (
            0x8002_339C,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x6710,
            },
        ),
        (
            0x8002_33A8,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8002_33AC,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8002_33B0,
            K::Sw {
                rt: Reg::A0,
                base: Reg::V0,
                offset: 0,
            },
        ),
    ] {
        require_instruction(map, text, runtime, expected)?;
    }
    Ok(())
}

fn verify_partition(
    literal_combines: &[u32],
    resource_x36_selectors: &[u32],
) -> Result<(), MmenuDescriptorIngressError> {
    let classified = resource_x36_selectors
        .iter()
        .chain(&ALTERNATE_BRANCH_COMBINES)
        .chain(&REGISTRATION_COMBINES)
        .copied()
        .collect::<BTreeSet<_>>();
    let literal = literal_combines.iter().copied().collect::<BTreeSet<_>>();
    if classified != literal || classified.len() != literal_combines.len() {
        return Err(error(format!(
            "PUYO2 descriptor root literal combine 분류 불완전: literal={literal:?}, classified={classified:?}"
        )));
    }
    Ok(())
}

fn direct_reference_counts(
    instructions: &[Instruction],
    text: &[u8],
    target: u32,
) -> (usize, usize) {
    let address_loads = xref::find_address_loads(instructions, xref::DEFAULT_WINDOW)
        .into_iter()
        .filter(|load| normalize_kseg(load.value) == target)
        .count();
    let raw_pointer_words = text
        .chunks_exact(4)
        .filter(|word| {
            normalize_kseg(u32::from_le_bytes(
                (*word).try_into().expect("4바이트 word"),
            )) == target
        })
        .count();
    (address_loads, raw_pointer_words)
}

fn text_runtime(map: &ExeMap, text_offset: usize) -> Result<u32, MmenuDescriptorIngressError> {
    map.runtime_addr(
        PSX_EXE_HEADER_LEN
            .checked_add(
                u64::try_from(text_offset).map_err(|_| error("PUYO2 text offset이 u64를 넘음"))?,
            )
            .ok_or_else(|| error("PUYO2 text file offset overflow"))?,
    )
    .ok_or_else(|| error(format!("PUYO2 text offset이 EXE 밖임: +0x{text_offset:X}")))
}

fn selector_reference(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    spec: SelectorReferenceSpec,
) -> Result<MmenuDescriptorSelectorReference, MmenuDescriptorIngressError> {
    let jal_word = 0x0C00_0000 | ((spec.entry_runtime >> 2) & 0x03FF_FFFF);
    let mut direct_jal_locations = Vec::new();
    let mut raw_pointer_locations = Vec::new();
    for (index, bytes) in text.chunks_exact(4).enumerate() {
        let word = u32::from_le_bytes(bytes.try_into().expect("4바이트 word"));
        let runtime = text_runtime(map, index * 4)?;
        if word == jal_word {
            direct_jal_locations.push(runtime);
        }
        if normalize_kseg(word) == spec.entry_runtime {
            raw_pointer_locations.push(runtime);
        }
    }
    let address_load_locations = xref::find_address_loads(instructions, xref::DEFAULT_WINDOW)
        .into_iter()
        .filter_map(|load| {
            (normalize_kseg(load.value) == spec.entry_runtime).then_some(load.combine_addr)
        })
        .collect::<Vec<_>>();
    if direct_jal_locations != spec.direct_jal_locations
        || raw_pointer_locations != spec.raw_pointer_locations
        || !address_load_locations.is_empty()
    {
        return Err(error(format!(
            "PUYO2 descriptor selector entry 정적 참조 분모 불일치: entry=0x{:08X}, JAL={direct_jal_locations:?}, raw={raw_pointer_locations:?}, address={address_load_locations:?}",
            spec.entry_runtime
        )));
    }
    Ok(MmenuDescriptorSelectorReference {
        combine_runtime: spec.combine_runtime,
        entry_runtime: spec.entry_runtime,
        direct_jal_locations,
        raw_pointer_locations,
        address_load_locations,
    })
}

/// 지원 원본 PUYO2 EXE의 type table literal ingress를 전수 검증한다.
pub fn verify_box_mmenu_descriptor_ingress<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
) -> Result<MmenuDescriptorIngressAnalysis, MmenuDescriptorIngressError> {
    let puyo2 = executables
        .into_iter()
        .filter(|(map, _)| map.name == PUYO2_EXE)
        .collect::<Vec<_>>();
    let [(map, text)] = puyo2.as_slice() else {
        return Err(error(format!(
            "PUYO2 EXE 입력 수가 1이 아님: {}",
            puyo2.len()
        )));
    };
    if text.len() != map.text_len as usize {
        return Err(error("PUYO2 EXE text 길이 불일치"));
    }
    let instructions =
        linear::sweep(map, text).ok_or_else(|| error("PUYO2 EXE text가 4바이트 배수가 아님"))?;

    let literal_combines =
        descriptor_root_literal_combines(&instructions, TYPE_TABLE_LITERAL, ROOT_LOOKBACK);
    if literal_combines != LITERAL_COMBINES {
        return Err(error(format!(
            "PUYO2 descriptor root literal combine 분모 불일치: {literal_combines:?}"
        )));
    }
    let resource_x36_selectors = literal_combines
        .iter()
        .copied()
        .filter(|runtime| {
            descriptor_resource_x36_select(
                &instructions,
                *runtime,
                TYPE_TABLE_LITERAL,
                SELECTOR_SCAN_INSTRUCTIONS,
            )
            .is_some()
        })
        .collect::<Vec<_>>();
    if resource_x36_selectors != RESOURCE_X36_SELECTORS {
        return Err(error(format!(
            "PUYO2 descriptor root resource*36 selector 분모 불일치: {resource_x36_selectors:?}"
        )));
    }

    verify_alternate_branches(map, text)?;
    verify_registration_stores(map, text)?;
    verify_partition(&literal_combines, &resource_x36_selectors)?;

    let selector_references = SELECTOR_REFERENCE_SPECS
        .into_iter()
        .map(|spec| selector_reference(map, text, &instructions, spec))
        .collect::<Result<Vec<_>, _>>()?;
    let statically_referenced_selectors = selector_references
        .iter()
        .filter(|reference| reference.is_statically_referenced())
        .count();
    let statically_unreferenced_selectors =
        selector_references.len() - statically_referenced_selectors;
    if statically_referenced_selectors != 7 || statically_unreferenced_selectors != 4 {
        return Err(error(format!(
            "PUYO2 descriptor selector 정적 진입 분류 불일치: referenced={statically_referenced_selectors}, unreferenced={statically_unreferenced_selectors}"
        )));
    }

    let (mmenu_descriptor_direct_address_loads, mmenu_descriptor_raw_pointer_words) =
        direct_reference_counts(&instructions, text, MMENU_SEQUENCE_DESCRIPTOR);
    let (mm_mes_descriptor_direct_address_loads, mm_mes_descriptor_raw_pointer_words) =
        direct_reference_counts(&instructions, text, MM_MES_GRAPHICS_DESCRIPTOR);
    if mmenu_descriptor_direct_address_loads != 0
        || mmenu_descriptor_raw_pointer_words != 0
        || mm_mes_descriptor_direct_address_loads != 0
        || mm_mes_descriptor_raw_pointer_words != 0
    {
        return Err(error(format!(
            "PUYO2 MMENU/MM_MES descriptor direct 참조가 생김: MMENU address/raw={mmenu_descriptor_direct_address_loads}/{mmenu_descriptor_raw_pointer_words}, MM_MES address/raw={mm_mes_descriptor_direct_address_loads}/{mm_mes_descriptor_raw_pointer_words}"
        )));
    }

    Ok(MmenuDescriptorIngressAnalysis {
        type_table_root: TYPE_TABLE_ROOT,
        literal_combines,
        resource_x36_selectors,
        alternate_branch_combines: ALTERNATE_BRANCH_COMBINES.to_vec(),
        registration_stores: REGISTRATION_STORES.to_vec(),
        selector_references,
        statically_referenced_selectors,
        statically_unreferenced_selectors,
        mmenu_descriptor_direct_address_loads,
        mmenu_descriptor_raw_pointer_words,
        mm_mes_descriptor_direct_address_loads,
        mm_mes_descriptor_raw_pointer_words,
    })
}

#[cfg(test)]
#[path = "mmenu_descriptor_ingress_tests.rs"]
mod tests;
