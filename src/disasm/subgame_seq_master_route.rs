//! PUYO2의 resident master SEQ 선택 graph 검증.
//!
//! 지원 원본의 EXE resident record는 `PP2_ALL`을 고르고, 그 overlay가
//! `PP2_HITO/FUTA`를 고른다. `PP1_ALL` 아래에도 완전한 scene subtree가
//! 남아 있지만 canonical `OP10/type 1` incoming record와 정적 descriptor
//! 참조가 없다. 이 모듈은 그 source 경계를 검증할 뿐, 간접·runtime 생성
//! record까지 없다고 주장하지 않는다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::archive::VddArchive;
use crate::compress::decompress;
use crate::coords::ExeMap;

use super::{linear, xref};

pub const BOX_PUYO2_MASTER_ROUTE_ARCHIVE: &str = "/PUYO2/PP2_SEQ.VDD;1";
pub const BOX_PUYO2_MASTER_ROUTE_ARCHIVE_ENTRIES: usize = 201;

const PUYO2_EXE: &str = "PUYO2.EXE";
const DESCRIPTOR_BASE_RUNTIME: u32 = 0x8006_6824;
const DESCRIPTOR_STRIDE: u32 = 36;
const DESCRIPTOR_PREFIX_ROWS: u16 = 4;
const RESIDENT_ROOT_RUNTIME: u32 = 0x8006_3E0C;
const RESIDENT_ROOT_DESTINATION: u32 = 0x8019_9A70;
const PP1_ALL_DESCRIPTOR_RUNTIME: u32 = 0x8006_75A4;

const PP1_ALL_RESOURCE: u16 = 0x0060;
const PP1_HITO_RESOURCE: u16 = 0x0061;
const PP1_FUTA_RESOURCE: u16 = 0x0063;
const PP2_ALL_RESOURCE: u16 = 0x0087;
const PP2_HITO_RESOURCE: u16 = 0x0088;
const PP2_FUTA_RESOURCE: u16 = 0x008A;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MasterSpec {
    name: &'static str,
    vdd_index: usize,
    resource: u16,
}

const MASTER_SPECS: [MasterSpec; 6] = [
    MasterSpec {
        name: "PP1_ALL.SEQ",
        vdd_index: 92,
        resource: PP1_ALL_RESOURCE,
    },
    MasterSpec {
        name: "PP1_HITO.SEQ",
        vdd_index: 93,
        resource: PP1_HITO_RESOURCE,
    },
    MasterSpec {
        name: "PP1_FUTA.SEQ",
        vdd_index: 95,
        resource: PP1_FUTA_RESOURCE,
    },
    MasterSpec {
        name: "PP2_ALL.SEQ",
        vdd_index: 131,
        resource: PP2_ALL_RESOURCE,
    },
    MasterSpec {
        name: "PP2_HITO.SEQ",
        vdd_index: 132,
        resource: PP2_HITO_RESOURCE,
    },
    MasterSpec {
        name: "PP2_FUTA.SEQ",
        vdd_index: 134,
        resource: PP2_FUTA_RESOURCE,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectionRecord {
    offset: usize,
    flags: u8,
    resource: u16,
}

/// 지원 원본에서 닫힌 PUYO2 canonical master route 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2MasterRouteAnalysis {
    pub archive_entries: usize,
    pub master_descriptors: usize,
    pub resident_root_resource: u16,
    pub pp1_child_records: usize,
    pub pp2_child_records: usize,
    pub pp1_all_incoming_records: usize,
    pub pp1_all_raw_pointers: usize,
    pub pp1_all_address_loads: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2MasterRouteError(String);

impl fmt::Display for Puyo2MasterRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Puyo2MasterRouteError {}

fn error(message: impl Into<String>) -> Puyo2MasterRouteError {
    Puyo2MasterRouteError(message.into())
}

fn read_word(raw: &[u8], offset: usize) -> Result<u32, Puyo2MasterRouteError> {
    raw.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
        .ok_or_else(|| error(format!("PUYO2 master route word 범위 이탈: +0x{offset:X}")))
}

fn selection_records(raw: &[u8]) -> Vec<SelectionRecord> {
    if raw.len() < 8 {
        return Vec::new();
    }
    (0..=raw.len() - 8)
        .step_by(4)
        .filter_map(|offset| {
            let record = &raw[offset..offset + 8];
            (record[0] == 0x28
                && record[1] == 0x10
                && record[3] == 0x01
                && record[6] == 0
                && record[7] == 0)
                .then(|| SelectionRecord {
                    offset,
                    flags: record[2],
                    resource: u16::from_le_bytes([record[4], record[5]]),
                })
        })
        .collect()
}

fn require_record(
    sources: &BTreeMap<&str, &[u8]>,
    sequence: &str,
    offset: usize,
    flags: u8,
    resource: u16,
) -> Result<(), Puyo2MasterRouteError> {
    let raw = sources
        .get(sequence)
        .ok_or_else(|| error(format!("{sequence} source가 없음")))?;
    let expected = [
        0x28,
        0x10,
        flags,
        0x01,
        resource as u8,
        (resource >> 8) as u8,
        0,
        0,
    ];
    let actual = raw
        .get(offset..offset + expected.len())
        .ok_or_else(|| error(format!("{sequence} master record 범위 이탈: +0x{offset:X}")))?;
    if actual != expected {
        return Err(error(format!(
            "{sequence} master record 불일치: +0x{offset:X}, expected={expected:02X?}, actual={actual:02X?}"
        )));
    }
    Ok(())
}

fn incoming_count(exe_text: &[u8], sources: &BTreeMap<&str, &[u8]>, resource: u16) -> usize {
    selection_records(exe_text)
        .into_iter()
        .chain(
            sources
                .values()
                .flat_map(|raw| selection_records(raw).into_iter()),
        )
        .filter(|record| record.resource == resource)
        .count()
}

fn verify_route_records(
    map: &ExeMap,
    text: &[u8],
    sources: &BTreeMap<&str, &[u8]>,
) -> Result<(usize, usize), Puyo2MasterRouteError> {
    let root_offset = map
        .text_offset(RESIDENT_ROOT_RUNTIME)
        .ok_or_else(|| error("PUYO2 resident root가 EXE text 밖임"))?;
    let expected_root = [
        0x28, 0x10, 0x17, 0x01, 0x87, 0, 0, 0, 0x70, 0x9A, 0x19, 0x80,
    ];
    let actual_root = text
        .get(root_offset..root_offset + expected_root.len())
        .ok_or_else(|| error("PUYO2 resident root record 범위 이탈"))?;
    if actual_root != expected_root {
        return Err(error(format!(
            "PUYO2 resident root 불일치: runtime=0x{RESIDENT_ROOT_RUNTIME:08X}, expected={expected_root:02X?}, actual={actual_root:02X?}"
        )));
    }
    if read_word(actual_root, 8)? != RESIDENT_ROOT_DESTINATION {
        return Err(error("PUYO2 resident root destination 불일치"));
    }

    require_record(sources, "PP1_ALL.SEQ", 0x0120, 0x97, PP1_HITO_RESOURCE)?;
    require_record(sources, "PP1_ALL.SEQ", 0x0148, 0x97, PP1_FUTA_RESOURCE)?;
    require_record(sources, "PP2_ALL.SEQ", 0x0270, 0x97, PP2_HITO_RESOURCE)?;
    require_record(sources, "PP2_ALL.SEQ", 0x02B8, 0x97, PP2_HITO_RESOURCE)?;
    require_record(sources, "PP2_ALL.SEQ", 0x03C4, 0x97, PP2_FUTA_RESOURCE)?;

    let expected_incoming = [
        (PP1_ALL_RESOURCE, 0usize),
        (PP1_HITO_RESOURCE, 1),
        (PP1_FUTA_RESOURCE, 1),
        (PP2_ALL_RESOURCE, 1),
        (PP2_HITO_RESOURCE, 2),
        (PP2_FUTA_RESOURCE, 1),
    ];
    for (resource, expected) in expected_incoming {
        let actual = incoming_count(text, sources, resource);
        if actual != expected {
            return Err(error(format!(
                "PUYO2 master canonical incoming 수 불일치: resource=0x{resource:04X}, expected={expected}, actual={actual}"
            )));
        }
    }
    Ok((2, 3))
}

fn verify_master_descriptor(
    map: &ExeMap,
    text: &[u8],
    archive: &VddArchive,
    sources: &BTreeMap<&str, &[u8]>,
    spec: MasterSpec,
) -> Result<(), Puyo2MasterRouteError> {
    let expected_resource = u16::try_from(spec.vdd_index)
        .ok()
        .and_then(|index| index.checked_add(DESCRIPTOR_PREFIX_ROWS))
        .ok_or_else(|| error(format!("{} resource 계산 overflow", spec.name)))?;
    if spec.resource != expected_resource {
        return Err(error(format!(
            "{} VDD index/resource 계약 불일치: index={}, resource=0x{:04X}",
            spec.name, spec.vdd_index, spec.resource
        )));
    }
    let entry = archive.entries().get(spec.vdd_index).ok_or_else(|| {
        error(format!(
            "{} VDD index 범위 이탈: {}",
            BOX_PUYO2_MASTER_ROUTE_ARCHIVE, spec.vdd_index
        ))
    })?;
    if entry.name() != spec.name {
        return Err(error(format!(
            "PUYO2 master VDD 이름 불일치: index={}, expected={}, actual={}",
            spec.vdd_index,
            spec.name,
            entry.name()
        )));
    }
    let source = sources
        .get(spec.name)
        .ok_or_else(|| error(format!("{} source가 없음", spec.name)))?;

    let descriptor_runtime = DESCRIPTOR_BASE_RUNTIME + u32::from(spec.resource) * DESCRIPTOR_STRIDE;
    let descriptor_offset = map
        .text_offset(descriptor_runtime)
        .ok_or_else(|| error(format!("{} descriptor가 EXE text 밖임", spec.name)))?;
    let descriptor = text
        .get(descriptor_offset..descriptor_offset + DESCRIPTOR_STRIDE as usize)
        .ok_or_else(|| error(format!("{} descriptor 범위 이탈", spec.name)))?;
    let expected = [
        0x0001_0000,
        read_word(descriptor, 4)?,
        entry.size,
        u32::try_from(source.len())
            .map_err(|_| error(format!("{} source 길이 overflow", spec.name)))?,
        0,
        0,
        0,
        0,
        0,
    ];
    let mut actual = [0u32; 9];
    for (index, word) in actual.iter_mut().enumerate() {
        *word = read_word(descriptor, index * 4)?;
    }
    if actual != expected {
        return Err(error(format!(
            "{} descriptor 불일치: expected={expected:08X?}, actual={actual:08X?}",
            spec.name
        )));
    }
    let name_runtime = actual[1];
    let name_offset = map
        .text_offset(name_runtime)
        .ok_or_else(|| error(format!("{} descriptor 이름이 EXE text 밖임", spec.name)))?;
    let name_end = name_offset
        .checked_add(spec.name.len() + 1)
        .ok_or_else(|| error(format!("{} descriptor 이름 범위 overflow", spec.name)))?;
    let name = text
        .get(name_offset..name_end)
        .ok_or_else(|| error(format!("{} descriptor 이름 범위 이탈", spec.name)))?;
    if name[..spec.name.len()] != *spec.name.as_bytes() || name[spec.name.len()] != 0 {
        return Err(error(format!(
            "{} descriptor 이름 literal 불일치: runtime=0x{name_runtime:08X}",
            spec.name
        )));
    }
    Ok(())
}

fn verify_no_pp1_all_static_reference(
    map: &ExeMap,
    text: &[u8],
) -> Result<(usize, usize), Puyo2MasterRouteError> {
    let raw_pointers = text
        .chunks_exact(4)
        .filter(|word| {
            u32::from_le_bytes((*word).try_into().expect("4바이트 word"))
                == PP1_ALL_DESCRIPTOR_RUNTIME
        })
        .count();
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error("PUYO2.EXE text 길이가 4바이트 정렬이 아님"))?;
    let address_loads = xref::find_address_loads(&instructions, xref::DEFAULT_WINDOW)
        .into_iter()
        .filter(|load| load.value == PP1_ALL_DESCRIPTOR_RUNTIME)
        .count();
    if raw_pointers != 0 || address_loads != 0 {
        return Err(error(format!(
            "PP1_ALL descriptor 정적 참조가 생김: raw_pointer={raw_pointers}, address_load={address_loads}"
        )));
    }
    Ok((raw_pointers, address_loads))
}

/// 지원 원본의 PUYO2 resident root와 canonical master graph를 검증한다.
pub fn verify_box_puyo2_master_route(
    map: &ExeMap,
    text: &[u8],
    archive: &VddArchive,
) -> Result<Puyo2MasterRouteAnalysis, Puyo2MasterRouteError> {
    if map.name != PUYO2_EXE {
        return Err(error(format!(
            "PUYO2 master route에 다른 EXE 사상을 사용함: {}",
            map.name
        )));
    }
    if archive.entry_count() != BOX_PUYO2_MASTER_ROUTE_ARCHIVE_ENTRIES {
        return Err(error(format!(
            "{} 엔트리 수 불일치: expected={}, actual={}",
            BOX_PUYO2_MASTER_ROUTE_ARCHIVE,
            BOX_PUYO2_MASTER_ROUTE_ARCHIVE_ENTRIES,
            archive.entry_count()
        )));
    }
    let decompressed = archive
        .entries()
        .iter()
        .map(|entry| {
            decompress(archive.read_file(entry))
                .map(|raw| (entry.name(), raw))
                .map_err(|cause| error(format!("{} CNX 해제 실패: {cause}", entry.name())))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let sources = decompressed
        .iter()
        .map(|(name, raw)| (*name, raw.as_slice()))
        .collect::<BTreeMap<_, _>>();

    for spec in MASTER_SPECS {
        verify_master_descriptor(map, text, archive, &sources, spec)?;
    }
    let (pp1_child_records, pp2_child_records) = verify_route_records(map, text, &sources)?;
    let (pp1_all_raw_pointers, pp1_all_address_loads) =
        verify_no_pp1_all_static_reference(map, text)?;

    Ok(Puyo2MasterRouteAnalysis {
        archive_entries: sources.len(),
        master_descriptors: MASTER_SPECS.len(),
        resident_root_resource: PP2_ALL_RESOURCE,
        pp1_child_records,
        pp2_child_records,
        pp1_all_incoming_records: 0,
        pp1_all_raw_pointers,
        pp1_all_address_loads,
    })
}

#[cfg(test)]
#[path = "subgame_seq_master_route_tests.rs"]
mod subgame_seq_master_route_tests;
