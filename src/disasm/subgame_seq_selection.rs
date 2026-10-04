//! 비-QUEST 장면 record에서 SEQ descriptor까지의 정적 선택 경계.
//!
//! `PUYO1/PUYO2`의 장면 SEQ에는 opcode `0x10`, type `1`, resource `46`인
//! record가 있다. 두 EXE의 공통 dispatcher는 현재 record의 `+1` opcode를
//! 32-word table에서 골라 generic descriptor selector를 간접 호출한다.
//! 이 모듈은 그 record와 dispatch, 이미 검증된 `DRAC_SEQ.SEQ` descriptor를
//! 한 경계로 묶는다. record의 runtime 실행, CNX 적재, 화면 표시는 주장하지
//! 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::parse_subgame_seq_overlay_footer;

use super::linear;
use super::subgame_seq_registration::SubgameSeqRegistrationAnalysis;

pub const SUBGAME_SEQ_SELECTION_SCHEMA_VERSION: &str = "subgame-seq-selections-v1";
pub const SUBGAME_SEQ_SELECTION_OPCODE: u8 = 0x10;
pub const SUBGAME_SEQ_SELECTION_TYPE: u8 = 1;
pub const SUBGAME_SEQ_SELECTION_RESOURCE: u16 = 46;
pub const SUBGAME_SEQ_OPCODE_TABLE_ENTRIES: usize = 32;
pub const BOX_SUBGAME_SEQ_SELECTION_ROWS: usize = 3;

/// 지원 원본에서 생성한 3행 TSV의 SHA-256.
pub const BOX_SUBGAME_SEQ_SELECTION_REPORT_SHA256: [u8; 32] = [
    0xC5, 0xB7, 0x2E, 0x7B, 0x55, 0x4C, 0x25, 0x6B, 0xCA, 0xF7, 0x44, 0xB4, 0x9B, 0xD4, 0x58, 0xFA,
    0xF1, 0x96, 0x2C, 0x95, 0x57, 0xA8, 0x62, 0x92, 0x1D, 0x81, 0x00, 0x6B, 0x53, 0x46, 0xBD, 0xD6,
];

/// EXE별 opcode dispatcher와 selector slot의 exact 사양.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqSelectionDispatcherSpec {
    pub exe: &'static str,
    pub dispatcher_runtime: u32,
    pub opcode_table_runtime: u32,
    pub selector_runtime: u32,
}

pub const BOX_SUBGAME_SEQ_SELECTION_DISPATCHERS: [SubgameSeqSelectionDispatcherSpec; 2] = [
    SubgameSeqSelectionDispatcherSpec {
        exe: "PUYO1.EXE",
        dispatcher_runtime: 0x8001_C6F8,
        opcode_table_runtime: 0x8008_954C,
        selector_runtime: 0x8001_ACF4,
    },
    SubgameSeqSelectionDispatcherSpec {
        exe: "PUYO2.EXE",
        dispatcher_runtime: 0x8001_B184,
        opcode_table_runtime: 0x8008_B59C,
        selector_runtime: 0x8001_9780,
    },
];

/// 장면 SEQ 안 resource 46 record의 exact 사양.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqSelectionSpec {
    pub exe: &'static str,
    pub archive: &'static str,
    pub scene: &'static str,
    pub load_base: u32,
    pub record_offset: usize,
    pub direct_pointer_field_offsets: &'static [usize],
}

const PUYO1_HITO_POINTERS: &[usize] = &[0x01A4];
const PUYO2_HITO_POINTERS: &[usize] = &[0x0138];

pub const BOX_SUBGAME_SEQ_SELECTION_SPECS: [SubgameSeqSelectionSpec; 3] = [
    SubgameSeqSelectionSpec {
        exe: "PUYO1.EXE",
        archive: "/PUYO1/PP1_SEQ.VDD;1",
        scene: "PP1_HITO.SEQ",
        load_base: 0x8019_E270,
        record_offset: 0x0A58,
        direct_pointer_field_offsets: PUYO1_HITO_POINTERS,
    },
    SubgameSeqSelectionSpec {
        exe: "PUYO2.EXE",
        archive: "/PUYO2/PP2_SEQ.VDD;1",
        scene: "PP1_HITO.SEQ",
        load_base: 0x8019_E270,
        record_offset: 0x09E8,
        direct_pointer_field_offsets: PUYO2_HITO_POINTERS,
    },
    SubgameSeqSelectionSpec {
        exe: "PUYO2.EXE",
        archive: "/PUYO2/PP2_SEQ.VDD;1",
        scene: "PP1_FUTA.SEQ",
        load_base: 0x8019_E270,
        record_offset: 0x08C8,
        direct_pointer_field_offsets: &[],
    },
];

const SELECTION_RECORD: [u8; 8] = [0x28, 0x10, 0x97, 0x01, 0x2E, 0, 0, 0];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSelectionRow {
    pub exe: &'static str,
    pub archive: &'static str,
    pub scene: &'static str,
    pub load_base: u32,
    pub record_offset: usize,
    pub record_runtime: u32,
    pub direct_pointer_field_runtimes: Vec<u32>,
    pub dispatcher_runtime: u32,
    pub opcode_table_runtime: u32,
    pub selector_slot_runtime: u32,
    pub selector_runtime: u32,
    pub selected_asset: &'static str,
    pub asset_descriptor_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSelectionAnalysis {
    pub rows: Vec<SubgameSeqSelectionRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSelectionError(String);

impl fmt::Display for SubgameSeqSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqSelectionError {}

fn error(message: impl Into<String>) -> SubgameSeqSelectionError {
    SubgameSeqSelectionError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, SubgameSeqSelectionError> {
    instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| error(format!("SEQ selection 명령 주소 없음: 0x{address:08X}")))
}

fn require(
    instructions: &[Instruction],
    spec: SubgameSeqSelectionDispatcherSpec,
    address: u32,
    expected: K,
) -> Result<(), SubgameSeqSelectionError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "{} SEQ selection 명령 불일치: address=0x{address:08X}, \
             expected={expected:?}, actual={:?}",
            spec.exe, actual.kind
        )));
    }
    Ok(())
}

fn high_low(address: u32) -> (u16, i16) {
    (
        ((address.wrapping_add(0x8000)) >> 16) as u16,
        address as u16 as i16,
    )
}

fn read_word(raw: &[u8], offset: usize) -> Result<u32, SubgameSeqSelectionError> {
    raw.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
        .ok_or_else(|| error(format!("SEQ selection word 범위 이탈: +0x{offset:X}")))
}

fn verify_dispatcher(
    map: &ExeMap,
    text: &[u8],
    spec: SubgameSeqSelectionDispatcherSpec,
) -> Result<(), SubgameSeqSelectionError> {
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error(format!("{} text 길이가 4바이트 정렬이 아님", spec.exe)))?;
    let start = spec.dispatcher_runtime;
    let (table_high, table_low) = high_low(spec.opcode_table_runtime);
    let checks = [
        (
            start + 8,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            start + 0x0C,
            K::Lui {
                rt: Reg::V1,
                imm: table_high,
            },
        ),
        (
            start + 0x10,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            start + 0x14,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: table_low,
            },
        ),
        (
            start + 0x18,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            start + 0x1C,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            start + 0x20,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            start + 0x28,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
    ];
    for (address, expected) in checks {
        require(&instructions, spec, address, expected)?;
    }

    let table_offset = map
        .text_offset(spec.opcode_table_runtime)
        .ok_or_else(|| error(format!("{} opcode table이 EXE text 밖임", spec.exe)))?;
    let table = text
        .get(table_offset..table_offset + SUBGAME_SEQ_OPCODE_TABLE_ENTRIES * 4)
        .ok_or_else(|| error(format!("{} opcode table 범위 이탈", spec.exe)))?;
    let (runtime_start, runtime_end) = map.runtime_range();
    for index in 0..SUBGAME_SEQ_OPCODE_TABLE_ENTRIES {
        let pointer = read_word(table, index * 4)?;
        if pointer < runtime_start || pointer >= runtime_end || !pointer.is_multiple_of(4) {
            return Err(error(format!(
                "{} opcode table code pointer 불일치: index={index}, value=0x{pointer:08X}",
                spec.exe
            )));
        }
    }
    let selector = read_word(table, usize::from(SUBGAME_SEQ_SELECTION_OPCODE) * 4)?;
    if selector != spec.selector_runtime {
        return Err(error(format!(
            "{} opcode 0x10 selector 불일치: expected=0x{:08X}, actual=0x{selector:08X}",
            spec.exe, spec.selector_runtime
        )));
    }
    Ok(())
}

fn find_selection_records(raw: &[u8]) -> Vec<usize> {
    raw.windows(SELECTION_RECORD.len())
        .enumerate()
        .filter_map(|(offset, bytes)| {
            (offset.is_multiple_of(4) && bytes == SELECTION_RECORD).then_some(offset)
        })
        .collect()
}

fn direct_pointer_field_offsets(raw: &[u8], initialized_len: usize, target: u32) -> Vec<usize> {
    raw[..initialized_len]
        .chunks_exact(4)
        .enumerate()
        .filter_map(|(index, bytes)| {
            (u32::from_le_bytes(bytes.try_into().expect("4바이트 word")) == target)
                .then_some(index * 4)
        })
        .collect()
}

fn audit_subgame_seq_selections<'a>(
    dispatcher_specs: &[SubgameSeqSelectionDispatcherSpec],
    selection_specs: &[SubgameSeqSelectionSpec],
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    registrations: &SubgameSeqRegistrationAnalysis,
) -> Result<SubgameSeqSelectionAnalysis, SubgameSeqSelectionError> {
    if dispatcher_specs.is_empty() || selection_specs.is_empty() {
        return Err(error("SEQ selection spec이 없음"));
    }
    let executable_map = executables
        .into_iter()
        .map(|(map, text)| (map.name, (map, text)))
        .collect::<BTreeMap<_, _>>();
    let source_map = sources
        .into_iter()
        .map(|(archive, scene, raw)| ((archive, scene), raw))
        .collect::<BTreeMap<_, _>>();
    let dispatcher_map = dispatcher_specs
        .iter()
        .map(|spec| (spec.exe, *spec))
        .collect::<BTreeMap<_, _>>();
    let registration_map = registrations
        .rows
        .iter()
        .filter(|row| row.exe == "PUYO1.EXE" || row.exe == "PUYO2.EXE")
        .map(|row| (row.exe, row))
        .collect::<BTreeMap<_, _>>();

    if executable_map.len() != dispatcher_specs.len()
        || dispatcher_map.len() != dispatcher_specs.len()
        || registration_map.len() != dispatcher_specs.len()
    {
        return Err(error(format!(
            "SEQ selection EXE/dispatcher/registration 집합 불일치: {}/{}/{}",
            executable_map.len(),
            dispatcher_map.len(),
            registration_map.len()
        )));
    }
    for &dispatcher in dispatcher_specs {
        let &(map, text) = executable_map
            .get(dispatcher.exe)
            .ok_or_else(|| error(format!("{} EXE 입력이 없음", dispatcher.exe)))?;
        if text.len() != map.text_len as usize {
            return Err(error(format!("{} text 길이 불일치", dispatcher.exe)));
        }
        verify_dispatcher(map, text, dispatcher)?;
    }

    let relevant_archives = selection_specs
        .iter()
        .map(|spec| spec.archive)
        .collect::<BTreeSet<_>>();
    let discovered = source_map
        .iter()
        .filter(|((archive, _), _)| relevant_archives.contains(archive))
        .flat_map(|(&(archive, scene), raw)| {
            find_selection_records(raw)
                .into_iter()
                .map(move |offset| (archive, scene, offset))
        })
        .collect::<BTreeSet<_>>();
    let expected = selection_specs
        .iter()
        .map(|spec| (spec.archive, spec.scene, spec.record_offset))
        .collect::<BTreeSet<_>>();
    if discovered != expected {
        return Err(error(format!(
            "SEQ resource 46 record 모집단 불일치: expected={expected:?}, actual={discovered:?}"
        )));
    }

    let mut rows = Vec::with_capacity(selection_specs.len());
    for &spec in selection_specs {
        let raw = source_map
            .get(&(spec.archive, spec.scene))
            .ok_or_else(|| error(format!("{}:{} 입력이 없음", spec.archive, spec.scene)))?;
        let footer = parse_subgame_seq_overlay_footer(raw)
            .map_err(|source| error(format!("{}:{}: {source}", spec.archive, spec.scene)))?
            .ok_or_else(|| error(format!("{}:{}가 overlay가 아님", spec.archive, spec.scene)))?;
        if footer.load_base != spec.load_base {
            return Err(error(format!(
                "{}:{} load base 불일치: expected=0x{:08X}, actual=0x{:08X}",
                spec.archive, spec.scene, spec.load_base, footer.load_base
            )));
        }
        let record = raw
            .get(spec.record_offset..spec.record_offset + SELECTION_RECORD.len())
            .ok_or_else(|| error(format!("{}:{} record 범위 이탈", spec.archive, spec.scene)))?;
        if record != SELECTION_RECORD {
            return Err(error(format!(
                "{}:{} resource 46 record 원본 불일치: +0x{:X}",
                spec.archive, spec.scene, spec.record_offset
            )));
        }
        let record_runtime = spec
            .load_base
            .checked_add(
                u32::try_from(spec.record_offset).map_err(|_| error("record offset overflow"))?,
            )
            .ok_or_else(|| error("record runtime overflow"))?;
        let actual_pointer_offsets =
            direct_pointer_field_offsets(raw, footer.footer_offset, record_runtime);
        if actual_pointer_offsets != spec.direct_pointer_field_offsets {
            return Err(error(format!(
                "{}:{} direct pointer field 불일치: expected={:X?}, actual={actual_pointer_offsets:X?}",
                spec.archive, spec.scene, spec.direct_pointer_field_offsets
            )));
        }

        let dispatcher = *dispatcher_map
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} dispatcher가 없음", spec.exe)))?;
        let registration = registration_map
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} registration 행이 없음", spec.exe)))?;
        if registration.asset != "DRAC_SEQ.SEQ"
            || registration.resource_id != SUBGAME_SEQ_SELECTION_RESOURCE
        {
            return Err(error(format!(
                "{} DRAC resource registration 불일치",
                spec.exe
            )));
        }
        let selector_slot_runtime = dispatcher
            .opcode_table_runtime
            .checked_add(u32::from(SUBGAME_SEQ_SELECTION_OPCODE) * 4)
            .ok_or_else(|| error("selector slot runtime overflow"))?;
        let direct_pointer_field_runtimes = spec
            .direct_pointer_field_offsets
            .iter()
            .map(|&offset| {
                spec.load_base
                    .checked_add(
                        u32::try_from(offset).map_err(|_| error("pointer offset overflow"))?,
                    )
                    .ok_or_else(|| error("pointer field runtime overflow"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        rows.push(SubgameSeqSelectionRow {
            exe: spec.exe,
            archive: spec.archive,
            scene: spec.scene,
            load_base: spec.load_base,
            record_offset: spec.record_offset,
            record_runtime,
            direct_pointer_field_runtimes,
            dispatcher_runtime: dispatcher.dispatcher_runtime,
            opcode_table_runtime: dispatcher.opcode_table_runtime,
            selector_slot_runtime,
            selector_runtime: dispatcher.selector_runtime,
            selected_asset: registration.asset,
            asset_descriptor_runtime: registration.asset_descriptor_runtime,
        });
    }
    Ok(SubgameSeqSelectionAnalysis { rows })
}

/// 지원 원본의 장면 record→opcode `0x10` selector→DRAC descriptor를 검증한다.
pub fn verify_box_subgame_seq_selections<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    registrations: &SubgameSeqRegistrationAnalysis,
) -> Result<SubgameSeqSelectionAnalysis, SubgameSeqSelectionError> {
    let analysis = audit_subgame_seq_selections(
        &BOX_SUBGAME_SEQ_SELECTION_DISPATCHERS,
        &BOX_SUBGAME_SEQ_SELECTION_SPECS,
        executables,
        sources,
        registrations,
    )?;
    if analysis.rows.len() != BOX_SUBGAME_SEQ_SELECTION_ROWS {
        return Err(error(format!(
            "SEQ selection 행 수 불일치: expected={}, actual={}",
            BOX_SUBGAME_SEQ_SELECTION_ROWS,
            analysis.rows.len()
        )));
    }
    let report = render_subgame_seq_selections_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_SELECTION_REPORT_SHA256 {
        return Err(error(format!(
            "SEQ selection report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_SELECTION_REPORT_SHA256),
            hex(&digest)
        )));
    }
    Ok(analysis)
}

/// 검증 결과를 결정적 3행 TSV로 직렬화한다.
pub fn render_subgame_seq_selections_tsv(analysis: &SubgameSeqSelectionAnalysis) -> String {
    let mut output = String::from(
        "schema_version\texe\tarchive\tscene\tload_base\trecord_offset\trecord_runtime\t\
         opcode\ttype\tresource_id\tdirect_pointer_field_runtimes\tdispatcher_runtime\t\
         opcode_table_runtime\tselector_slot_runtime\tselector_runtime\tselected_asset\t\
         asset_descriptor_runtime\tstatic_status\n",
    );
    for row in &analysis.rows {
        let pointer_fields = row
            .direct_pointer_field_runtimes
            .iter()
            .map(|address| format!("0x{address:08X}"))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t0x{:08X}\t0x{:04X}\t0x{:08X}\t0x{:02X}\t{}\t{}\t{}\t\
             0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t0x{:08X}\t\
             scene-record-to-drac-descriptor-selection-only",
            SUBGAME_SEQ_SELECTION_SCHEMA_VERSION,
            row.exe,
            row.archive,
            row.scene,
            row.load_base,
            row.record_offset,
            row.record_runtime,
            SUBGAME_SEQ_SELECTION_OPCODE,
            SUBGAME_SEQ_SELECTION_TYPE,
            SUBGAME_SEQ_SELECTION_RESOURCE,
            pointer_fields,
            row.dispatcher_runtime,
            row.opcode_table_runtime,
            row.selector_slot_runtime,
            row.selector_runtime,
            row.selected_asset,
            row.asset_descriptor_runtime,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_selection_tests.rs"]
mod subgame_seq_selection_tests;
