//! PUYO2 `MMENU.SEQ`의 alternate 상위 진입 후보 정적 경계.
//!
//! canonical `OP28/sub10/type1` record 밖에서 같은 type/resource table을
//! 소비할 수 있는 `OP28/sub12` wrapper와 type/resource 인자 진입 함수를
//! 검증한다. 정적 참조가 없다는 결과는 runtime 간접 호출이나 record/code
//! 변경까지 부정하지 않는다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::archive::VddArchive;
use crate::compress::{decompress, parse_header};
use crate::coords::ExeMap;
use crate::disasm::xref::{find_address_loads, DEFAULT_WINDOW};
use crate::isa::{decode, decode_slice, InstructionKind as K, Reg};
use crate::script::subgame_selector_records::{
    scan_subgame_selector_records, SubgameSelectorRecordForm,
};

const PP2_ARCHIVE: &str = "/PUYO2/PP2_SEQ.VDD;1";
const PP2_ARCHIVE_ENTRIES: usize = 201;
const PP2_SEQ_FILES: usize = 151;
const PP2_CEL_FILES: usize = 50;
const OP12_HANDLER_RUNTIME: u32 = 0x8001_9930;
const OP12_HANDLER_TABLE_RUNTIME: u32 = 0x8008_B5E4;

const RECORD_WRAPPER_WITH_DESTINATION_RUNTIME: u32 = 0x8001_B914;
const RECORD_WRAPPER_WITHOUT_DESTINATION_RUNTIME: u32 = 0x8001_B96C;
const RELEASE_SELECTED_DESCRIPTOR_RUNTIME: u32 = 0x8001_B474;
const QUERY_SELECTED_DESCRIPTOR_RUNTIME: u32 = 0x8001_B724;
const PROGRAMMATIC_LOADER_RUNTIME: u32 = 0x8001_B7D4;
const STAGE_SELECTED_DESCRIPTOR_RUNTIME: u32 = 0x8001_C554;
const MMENU_RESOURCE_ID: u16 = 62;
const MM_MES_RESOURCE_ID: u16 = 165;
const KNOWN_SELECTOR_RECORD_FORMS: usize = 7;
const ARGUMENT_ONLY_SELECTOR_ENTRIES: usize = 4;
const OPCODE_C0_TABLE_RUNTIME: u32 = 0x8008_BA40;
const OPCODE_C0_SOURCE_RUNTIME: u32 = 0x8008_D70C;
const OPCODE_C0_TABLE_ENTRIES: usize = 32;
const OPCODE_C0_DEFAULT_HANDLER_RUNTIME: u32 = 0x8002_28C4;
const OPCODE_C3_HANDLER_RUNTIME: u32 = 0x8005_1550;

const ENTRY_CANDIDATES: [MmenuEntryCandidateSpec; 6] = [
    MmenuEntryCandidateSpec {
        runtime: RECORD_WRAPPER_WITH_DESTINATION_RUNTIME,
        role: MmenuEntryCandidateRole::Op12RecordWrapperWithDestination,
    },
    MmenuEntryCandidateSpec {
        runtime: RECORD_WRAPPER_WITHOUT_DESTINATION_RUNTIME,
        role: MmenuEntryCandidateRole::Op12RecordWrapperWithoutDestination,
    },
    MmenuEntryCandidateSpec {
        runtime: RELEASE_SELECTED_DESCRIPTOR_RUNTIME,
        role: MmenuEntryCandidateRole::ReleaseSelectedDescriptor,
    },
    MmenuEntryCandidateSpec {
        runtime: QUERY_SELECTED_DESCRIPTOR_RUNTIME,
        role: MmenuEntryCandidateRole::QuerySelectedDescriptor,
    },
    MmenuEntryCandidateSpec {
        runtime: PROGRAMMATIC_LOADER_RUNTIME,
        role: MmenuEntryCandidateRole::ProgrammaticLoader,
    },
    MmenuEntryCandidateSpec {
        runtime: STAGE_SELECTED_DESCRIPTOR_RUNTIME,
        role: MmenuEntryCandidateRole::StageSelectedDescriptor,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MmenuEntryCandidateSpec {
    runtime: u32,
    role: MmenuEntryCandidateRole,
}

/// 정적으로 확인한 alternate 진입 함수의 역할.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmenuEntryCandidateRole {
    /// 목적지 word 둘을 포함한 임시 `OP12` record를 만드는 wrapper.
    Op12RecordWrapperWithDestination,
    /// 좌표 halfword를 포함한 임시 `OP12` record를 만드는 wrapper.
    Op12RecordWrapperWithoutDestination,
    /// type/resource로 고른 descriptor의 상주 object를 해제한다.
    ReleaseSelectedDescriptor,
    /// type/resource로 고른 descriptor의 상주 object 상태를 조회한다.
    QuerySelectedDescriptor,
    /// flags/type/resource/destination 인자로 descriptor를 직접 고르는 loader.
    ProgrammaticLoader,
    /// type/resource로 고른 descriptor의 payload pointer를 staging 상태에 둔다.
    StageSelectedDescriptor,
}

/// 한 corpus에서 센 정적 함수 참조 분모.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MmenuStaticReferenceCounts {
    pub direct_jal_words: usize,
    pub raw_pointer_bytes: usize,
    pub address_loads: usize,
}

impl MmenuStaticReferenceCounts {
    fn is_empty(self) -> bool {
        self == Self::default()
    }

    fn add_assign(&mut self, other: Self) {
        self.direct_jal_words += other.direct_jal_words;
        self.raw_pointer_bytes += other.raw_pointer_bytes;
        self.address_loads += other.address_loads;
    }
}

/// alternate 진입 함수 하나의 EXE/SEQ 정적 참조 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmenuEntryCandidate {
    pub runtime: u32,
    pub role: MmenuEntryCandidateRole,
    pub executable_references: MmenuStaticReferenceCounts,
    pub sequence_references: MmenuStaticReferenceCounts,
    pub cell_references: MmenuStaticReferenceCounts,
}

/// 정적으로 복원한 공용 record selector 형식.
pub type MmenuKnownSelectorRecordForm = SubgameSelectorRecordForm;

/// 알려진 selector record가 고르는 조사 대상 descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmenuKnownSelectorTarget {
    MmenuSequence,
    MmMesGraphics,
}

/// SEQ/CEL/EXE에서 찾은 조사 대상 selector record 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmenuKnownSelectorTargetRecord {
    pub source: String,
    pub offset: usize,
    pub form: MmenuKnownSelectorRecordForm,
    pub target: MmenuKnownSelectorTarget,
}

/// 지원 원본에서 닫힌 `MMENU.SEQ` alternate 진입 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmenuEntryRouteAnalysis {
    pub pp2_archive_entries: usize,
    pub pp2_seq_files: usize,
    pub pp2_cel_files: usize,
    pub alternate_type1_sequence_records: usize,
    pub alternate_type1_cell_records: usize,
    pub alternate_type1_executable_records: usize,
    pub known_selector_record_forms: usize,
    pub argument_only_selector_entries: usize,
    pub known_target_sequence_records: Vec<MmenuKnownSelectorTargetRecord>,
    pub known_target_cell_records: Vec<MmenuKnownSelectorTargetRecord>,
    pub known_target_executable_records: Vec<MmenuKnownSelectorTargetRecord>,
    pub candidates: Vec<MmenuEntryCandidate>,
}

/// `MMENU.SEQ` alternate 진입 source gate 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmenuEntryRouteError(String);

impl fmt::Display for MmenuEntryRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for MmenuEntryRouteError {}

fn error(message: impl Into<String>) -> MmenuEntryRouteError {
    MmenuEntryRouteError(message.into())
}

fn executable_word(map: &ExeMap, text: &[u8], runtime: u32) -> Result<u32, MmenuEntryRouteError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "PUYO2 MMENU entry runtime 주소가 EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    let bytes = text.get(offset..offset + 4).ok_or_else(|| {
        error(format!(
            "PUYO2 MMENU entry EXE text 범위 이탈: 0x{runtime:08X}"
        ))
    })?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), MmenuEntryRouteError> {
    let actual = decode(runtime, executable_word(map, text, runtime)?);
    if actual.kind != expected {
        return Err(error(format!(
            "PUYO2 MMENU entry typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn verify_entry_functions(map: &ExeMap, text: &[u8]) -> Result<(), MmenuEntryRouteError> {
    if executable_word(map, text, OP12_HANDLER_TABLE_RUNTIME)? != OP12_HANDLER_RUNTIME {
        return Err(error("PUYO2 OP12 handler table pointer 불일치"));
    }

    for (runtime, expected) in [
        (
            0x8001_B918,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800B,
            },
        ),
        (
            0x8001_B91C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x5F00,
            },
        ),
        (
            0x8001_B934,
            K::Lui {
                rt: Reg::A0,
                imm: 0x800B,
            },
        ),
        (
            0x8001_B938,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x5E10,
            },
        ),
        (
            0x8001_B93C,
            K::Sb {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 3,
            },
        ),
        (
            0x8001_B940,
            K::Sh {
                rt: Reg::A2,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8001_B948,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8001_B954,
            K::Jal {
                target: OP12_HANDLER_RUNTIME,
            },
        ),
        (
            0x8001_B970,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800B,
            },
        ),
        (
            0x8001_B974,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x5F00,
            },
        ),
        (
            0x8001_B984,
            K::Lui {
                rt: Reg::A0,
                imm: 0x800B,
            },
        ),
        (
            0x8001_B988,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x5E10,
            },
        ),
        (
            0x8001_B98C,
            K::Sb {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 3,
            },
        ),
        (
            0x8001_B990,
            K::Sh {
                rt: Reg::A2,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8001_B99C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8001_B9A0,
            K::Jal {
                target: OP12_HANDLER_RUNTIME,
            },
        ),
        (
            0x8001_B478,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800B,
            },
        ),
        (
            0x8001_B47C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x6710,
            },
        ),
        (
            0x8001_B480,
            K::Sll {
                rd: Reg::A0,
                rt: Reg::A0,
                sa: 2,
            },
        ),
        (
            0x8001_B488,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A1,
                sa: 3,
            },
        ),
        (
            0x8001_B49C,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8001_B4A0,
            K::Addu {
                rd: Reg::S0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8001_B4CC,
            K::Jal {
                target: 0x8001_AAE0,
            },
        ),
        (
            0x8001_B4E0,
            K::Jal {
                target: 0x8001_BEE0,
            },
        ),
        (
            0x8001_B4EC,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::S0,
                offset: 0x10,
            },
        ),
        (
            0x8001_B728,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800B,
            },
        ),
        (
            0x8001_B72C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x6710,
            },
        ),
        (
            0x8001_B730,
            K::Sll {
                rd: Reg::A0,
                rt: Reg::A0,
                sa: 2,
            },
        ),
        (
            0x8001_B738,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A1,
                sa: 3,
            },
        ),
        (
            0x8001_B74C,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8001_B750,
            K::Jal {
                target: 0x8001_A08C,
            },
        ),
        (
            0x8001_B7E0,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800B,
            },
        ),
        (
            0x8001_B7E4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x6710,
            },
        ),
        (
            0x8001_B7E8,
            K::Sll {
                rd: Reg::A1,
                rt: Reg::A1,
                sa: 2,
            },
        ),
        (
            0x8001_B7EC,
            K::Addu {
                rd: Reg::A1,
                rs: Reg::A1,
                rt: Reg::V0,
            },
        ),
        (
            0x8001_B7F0,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A2,
                sa: 3,
            },
        ),
        (
            0x8001_B7F4,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A2,
            },
        ),
        (
            0x8001_B7F8,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8001_B80C,
            K::Lw {
                rt: Reg::V1,
                base: Reg::A1,
                offset: 0,
            },
        ),
        (
            0x8001_B814,
            K::Addu {
                rd: Reg::S2,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8001_B818,
            K::Jal {
                target: 0x8001_BD58,
            },
        ),
        (
            0x8001_C554,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800B,
            },
        ),
        (
            0x8001_C558,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x6710,
            },
        ),
        (
            0x8001_C55C,
            K::Sll {
                rd: Reg::A0,
                rt: Reg::A0,
                sa: 2,
            },
        ),
        (
            0x8001_C574,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A1,
                sa: 3,
            },
        ),
        (
            0x8001_C57C,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8001_C584,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8001_C58C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 8,
            },
        ),
        (
            0x8005_17AC,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8009,
            },
        ),
        (
            0x8005_17B0,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: -0x28F4,
            },
        ),
        (
            0x8005_17B4,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 0x00C0,
            },
        ),
        (
            0x8005_17BC,
            K::Jal {
                target: 0x8002_2890,
            },
        ),
        (
            0x8005_17C0,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: 0x0020,
            },
        ),
        (
            0x8002_2898,
            K::Sll {
                rd: Reg::A1,
                rt: Reg::A1,
                sa: 2,
            },
        ),
        (
            0x8002_289C,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8009,
            },
        ),
        (
            0x8002_28A0,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: -0x48C0,
            },
        ),
        (
            0x8002_28A4,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A1,
                rt: Reg::A0,
            },
        ),
        (
            0x8002_28AC,
            K::Jal {
                target: 0x8006_0264,
            },
        ),
        (
            0x8002_28B0,
            K::Addu {
                rd: Reg::A1,
                rs: Reg::V0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8005_1564,
            K::Lw {
                rt: Reg::A2,
                base: Reg::S0,
                offset: 0x001C,
            },
        ),
        (
            0x8005_156C,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 3,
            },
        ),
        (
            0x8005_1570,
            K::Lhu {
                rt: Reg::A1,
                base: Reg::A2,
                offset: 4,
            },
        ),
        (
            0x8005_158C,
            K::Lw {
                rt: Reg::A1,
                base: Reg::A2,
                offset: 8,
            },
        ),
        (
            0x8005_1590,
            K::Lbu {
                rt: Reg::A2,
                base: Reg::A2,
                offset: 2,
            },
        ),
        (
            0x8005_1594,
            K::Jal {
                target: 0x8001_B208,
            },
        ),
        (
            0x8005_15A4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 12,
            },
        ),
    ] {
        require_instruction(map, text, runtime, expected)?;
    }

    for index in 0..OPCODE_C0_TABLE_ENTRIES {
        let runtime = OPCODE_C0_TABLE_RUNTIME + u32::try_from(index).expect("32 이하") * 4;
        if executable_word(map, text, runtime)? != OPCODE_C0_DEFAULT_HANDLER_RUNTIME {
            return Err(error(format!(
                "PUYO2 초기 opcode C0 table 기본 handler 불일치: index={index}"
            )));
        }
        let expected_source = match index {
            0 => 0x8005_09B8,
            1 => 0x8005_0CFC,
            2 => 0x8005_14EC,
            3 => OPCODE_C3_HANDLER_RUNTIME,
            _ => 0x8005_17D4,
        };
        let source_runtime = OPCODE_C0_SOURCE_RUNTIME + u32::try_from(index).expect("32 이하") * 4;
        if executable_word(map, text, source_runtime)? != expected_source {
            return Err(error(format!(
                "PUYO2 opcode C0 source table handler 불일치: index={index}"
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AlternateType1Record {
    offset: usize,
    resource_id: u16,
    reserved: u16,
}

fn alternate_type1_records(raw: &[u8]) -> Vec<AlternateType1Record> {
    raw.windows(8)
        .enumerate()
        .filter_map(|(offset, bytes)| {
            (bytes[0] == 0x28 && bytes[1] == 0x12 && bytes[3] == 1).then_some(
                AlternateType1Record {
                    offset,
                    resource_id: u16::from_le_bytes([bytes[4], bytes[5]]),
                    reserved: u16::from_le_bytes([bytes[6], bytes[7]]),
                },
            )
        })
        .collect()
}

fn known_selector_target(record_type: u8, resource_id: u16) -> Option<MmenuKnownSelectorTarget> {
    match (record_type, resource_id) {
        (1, MMENU_RESOURCE_ID) => Some(MmenuKnownSelectorTarget::MmenuSequence),
        (0, MM_MES_RESOURCE_ID) => Some(MmenuKnownSelectorTarget::MmMesGraphics),
        _ => None,
    }
}

fn known_selector_target_records(source: &str, raw: &[u8]) -> Vec<MmenuKnownSelectorTargetRecord> {
    scan_subgame_selector_records(raw)
        .into_iter()
        .filter_map(|record| {
            known_selector_target(record.record_type, record.resource_id).map(|target| {
                MmenuKnownSelectorTargetRecord {
                    source: source.to_string(),
                    offset: record.offset,
                    form: record.form,
                    target,
                }
            })
        })
        .collect()
}

fn count_static_references(raw: &[u8], target: u32) -> MmenuStaticReferenceCounts {
    let jal_word = 0x0C00_0000 | ((target >> 2) & 0x03FF_FFFF);
    let direct_jal_words = raw
        .chunks_exact(4)
        .filter(|bytes| u32::from_le_bytes((*bytes).try_into().expect("4바이트 word")) == jal_word)
        .count();
    let target_bytes = target.to_le_bytes();
    let raw_pointer_bytes = raw
        .windows(4)
        .filter(|bytes| *bytes == target_bytes)
        .count();
    let aligned_len = raw.len() / 4 * 4;
    let instructions = decode_slice(0x8000_0000, &raw[..aligned_len])
        .expect("4바이트 배수로 줄인 source는 연속 해석 가능");
    let address_loads = find_address_loads(&instructions, DEFAULT_WINDOW)
        .into_iter()
        .filter(|load| load.value == target)
        .count();
    MmenuStaticReferenceCounts {
        direct_jal_words,
        raw_pointer_bytes,
        address_loads,
    }
}

/// 지원 원본의 `OP12/type1` record와 alternate loader 정적 참조를 검증한다.
pub fn verify_box_mmenu_entry_routes<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<MmenuEntryRouteAnalysis, MmenuEntryRouteError> {
    let pp2_executables = executables
        .into_iter()
        .filter(|(map, _)| map.name == "PUYO2.EXE")
        .collect::<Vec<_>>();
    let [(map, text)] = pp2_executables.as_slice() else {
        return Err(error(format!(
            "PUYO2 EXE 입력 수가 1이 아님: {}",
            pp2_executables.len()
        )));
    };
    if text.len() != map.text_len as usize {
        return Err(error("PUYO2 EXE text 길이 불일치"));
    }
    verify_entry_functions(map, text)?;

    let pp2_sources = sources
        .into_iter()
        .filter(|(archive, _, _)| *archive == PP2_ARCHIVE)
        .collect::<Vec<_>>();
    if pp2_sources.len() != PP2_SEQ_FILES {
        return Err(error(format!(
            "PP2 SEQ 파일 수 불일치: expected={PP2_SEQ_FILES}, actual={}",
            pp2_sources.len()
        )));
    }

    let pp2_archives = archives
        .into_iter()
        .filter(|(archive, _)| *archive == PP2_ARCHIVE)
        .collect::<Vec<_>>();
    let [(_, pp2_archive)] = pp2_archives.as_slice() else {
        return Err(error(format!(
            "PP2 archive 입력 수가 1이 아님: {}",
            pp2_archives.len()
        )));
    };
    if pp2_archive.entries().len() != PP2_ARCHIVE_ENTRIES {
        return Err(error(format!(
            "PP2 archive entry 수 불일치: expected={PP2_ARCHIVE_ENTRIES}, actual={}",
            pp2_archive.entries().len()
        )));
    }

    let mut source_by_name = BTreeMap::new();
    for (_, sequence, raw) in &pp2_sources {
        if source_by_name.insert(*sequence, *raw).is_some() {
            return Err(error(format!("PP2 SEQ source 중복: {sequence}")));
        }
    }
    let mut sequence_entries = Vec::new();
    let mut cell_entries = Vec::new();
    for entry in pp2_archive.entries() {
        let name = entry.name();
        let stored = pp2_archive.read_file(entry);
        let header = parse_header(stored)
            .map_err(|source| error(format!("PP2 archive {name} CNX header: {source}")))?;
        let raw = decompress(stored)
            .map_err(|source| error(format!("PP2 archive {name} CNX decode: {source}")))?;
        match (
            name.ends_with(".SEQ"),
            name.ends_with(".CEL"),
            header.subtype,
        ) {
            (true, false, [b'b', b'i', b'n']) => {
                let source_raw = source_by_name.remove(name).ok_or_else(|| {
                    error(format!("PP2 archive SEQ가 source 입력에 없음: {name}"))
                })?;
                if source_raw != raw {
                    return Err(error(format!(
                        "PP2 archive/source SEQ 해제 결과 불일치: {name}"
                    )));
                }
                sequence_entries.push((name.to_string(), raw));
            }
            (false, true, [b'c', b'e', b'l']) => {
                cell_entries.push((name.to_string(), raw));
            }
            _ => {
                return Err(error(format!(
                    "PP2 archive entry 확장자/subtype 불일치: {name}, subtype={:?}",
                    header.subtype
                )));
            }
        }
    }
    if !source_by_name.is_empty()
        || sequence_entries.len() != PP2_SEQ_FILES
        || cell_entries.len() != PP2_CEL_FILES
    {
        return Err(error(format!(
            "PP2 archive 모집단 불일치: unmatched_sources={:?}, SEQ={}, CEL={}",
            source_by_name.keys().collect::<Vec<_>>(),
            sequence_entries.len(),
            cell_entries.len()
        )));
    }

    let alternate_sequence_records = sequence_entries
        .iter()
        .flat_map(|(sequence, raw)| {
            alternate_type1_records(raw)
                .into_iter()
                .map(move |record| (sequence.clone(), record))
        })
        .collect::<Vec<_>>();
    let alternate_cell_records = cell_entries
        .iter()
        .flat_map(|(cell, raw)| {
            alternate_type1_records(raw)
                .into_iter()
                .map(move |record| (cell.clone(), record))
        })
        .collect::<Vec<_>>();
    let alternate_executable_records = alternate_type1_records(text);
    if !alternate_sequence_records.is_empty()
        || !alternate_cell_records.is_empty()
        || !alternate_executable_records.is_empty()
    {
        return Err(error(format!(
            "PUYO2 alternate OP12/type1 record가 새로 발견됨: SEQ={alternate_sequence_records:?}, CEL={alternate_cell_records:?}, EXE={alternate_executable_records:?}"
        )));
    }

    let known_target_sequence_records = sequence_entries
        .iter()
        .flat_map(|(sequence, raw)| known_selector_target_records(sequence, raw))
        .collect::<Vec<_>>();
    let known_target_cell_records = cell_entries
        .iter()
        .flat_map(|(cell, raw)| known_selector_target_records(cell, raw))
        .collect::<Vec<_>>();
    let known_target_executable_records = known_selector_target_records("PUYO2.EXE", text);
    let expected_target_sequence_records =
        [0x222C, 0x2244].map(|offset| MmenuKnownSelectorTargetRecord {
            source: "MMENU.SEQ".to_string(),
            offset,
            form: MmenuKnownSelectorRecordForm::Op28Sub12,
            target: MmenuKnownSelectorTarget::MmMesGraphics,
        });
    if known_target_sequence_records != expected_target_sequence_records
        || !known_target_cell_records.is_empty()
        || !known_target_executable_records.is_empty()
    {
        return Err(error(format!(
            "PUYO2 알려진 selector record의 MMENU/MM_MES 대상 census 불일치: SEQ={known_target_sequence_records:?}, CEL={known_target_cell_records:?}, EXE={known_target_executable_records:?}"
        )));
    }

    let mut candidates = Vec::with_capacity(ENTRY_CANDIDATES.len());
    for spec in ENTRY_CANDIDATES {
        let executable_references = count_static_references(text, spec.runtime);
        let mut sequence_references = MmenuStaticReferenceCounts::default();
        for (_, raw) in &sequence_entries {
            sequence_references.add_assign(count_static_references(raw, spec.runtime));
        }
        let mut cell_references = MmenuStaticReferenceCounts::default();
        for (_, raw) in &cell_entries {
            cell_references.add_assign(count_static_references(raw, spec.runtime));
        }
        if !executable_references.is_empty()
            || !sequence_references.is_empty()
            || !cell_references.is_empty()
        {
            return Err(error(format!(
                "PUYO2 MMENU alternate entry 정적 참조가 새로 발견됨: runtime=0x{:08X}, EXE={executable_references:?}, SEQ={sequence_references:?}, CEL={cell_references:?}",
                spec.runtime
            )));
        }
        candidates.push(MmenuEntryCandidate {
            runtime: spec.runtime,
            role: spec.role,
            executable_references,
            sequence_references,
            cell_references,
        });
    }

    Ok(MmenuEntryRouteAnalysis {
        pp2_archive_entries: pp2_archive.entries().len(),
        pp2_seq_files: sequence_entries.len(),
        pp2_cel_files: cell_entries.len(),
        alternate_type1_sequence_records: alternate_sequence_records.len(),
        alternate_type1_cell_records: alternate_cell_records.len(),
        alternate_type1_executable_records: alternate_executable_records.len(),
        known_selector_record_forms: KNOWN_SELECTOR_RECORD_FORMS,
        argument_only_selector_entries: ARGUMENT_ONLY_SELECTOR_ENTRIES,
        known_target_sequence_records,
        known_target_cell_records,
        known_target_executable_records,
        candidates,
    })
}

#[cfg(test)]
#[path = "mmenu_entry_route_tests.rs"]
mod tests;
