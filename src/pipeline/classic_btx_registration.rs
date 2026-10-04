//! PUYO1 classic BTX의 type-0 VDD 등록 경계.
//!
//! `KANBAN.BTX`와 `RANK_B.BTX`는 알려진 selector record가 없지만
//! `PP1_GRP.VDD` 등록 loader가 이름으로 descriptor를 채운다. 공용 BTX
//! descriptor 선택·복사 gate와 같은 EXE/GRP 경계도 검산한다. 이 단계는
//! source CNX가 선택 가능한 descriptor에 결속된다는 뜻일 뿐, 두 target이
//! 실제로 선택되거나 texture 적재·VRAM upload·primitive draw까지 갔다는
//! 뜻은 아니다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::archive::VddArchive;
use crate::coords::ExeMap;
use crate::disasm::vdd_registration::{
    verify_vdd_registration_source, VddRegistrationFamily, VddRegistrationSourceSpec,
    VDD_DESCRIPTOR_STRIDE,
};
use crate::disasm::{
    BtxDescriptorSelection, BtxDescriptorSelectionAnalysis, BtxNameAnalysis, BtxNameLink,
};

const PUYO1_EXE: &str = "PUYO1.EXE";
const PP1_GRP_ARCHIVE: &str = "/PUYO1/PP1_GRP.VDD;1";
const PP1_GRP_DESCRIPTOR_NAME: &str = "PP1_GRP.VDD";
const PP1_GRP_PREFIX_DESCRIPTOR_ROWS: u16 = 38;
const PP1_GRP_REGISTRATION: VddRegistrationSourceSpec = VddRegistrationSourceSpec {
    exe: PUYO1_EXE,
    resource_type: 0,
    archive: PP1_GRP_DESCRIPTOR_NAME,
    registration_record_runtime: 0x8006_7B50,
    registration_handler_runtime: 0x8002_4334,
    loader_runtime: 0x8001_B3B4,
    table_root_runtime: 0x800B_4330,
    archive_descriptor_runtime: 0x8006_7E48,
    archive_name_runtime: 0x8006_984C,
    family: VddRegistrationFamily::Shared,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClassicBtxRegistrationTargetSpec {
    asset_name: &'static str,
    vdd_entry_index: usize,
    resource_id: u16,
    descriptor_runtime: u32,
}

const CLASSIC_BTX_REGISTRATION_TARGETS: [ClassicBtxRegistrationTargetSpec; 2] = [
    ClassicBtxRegistrationTargetSpec {
        asset_name: "KANBAN.BTX",
        vdd_entry_index: 137,
        resource_id: 175,
        descriptor_runtime: 0x8006_96E4,
    },
    ClassicBtxRegistrationTargetSpec {
        asset_name: "RANK_B.BTX",
        vdd_entry_index: 140,
        resource_id: 178,
        descriptor_runtime: 0x8006_9750,
    },
];

/// type-0 VDD 등록으로 source에 결속된 classic BTX 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicBtxRegistrationTarget {
    pub asset_name: &'static str,
    pub vdd_entry_index: usize,
    pub resource_id: u16,
    pub descriptor_runtime: u32,
}

/// PUYO1 `PP1_GRP.VDD` 등록과 현재 classic BTX 조사 대상.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicBtxRegistrationAnalysis {
    pub exe: &'static str,
    pub archive: &'static str,
    pub resource_type: u8,
    pub registration_record_runtime: u32,
    pub registration_handler_runtime: u32,
    pub loader_runtime: u32,
    pub table_root_runtime: u32,
    pub descriptor_table_runtime: u32,
    pub prefix_descriptor_rows: u16,
    pub descriptor_selection: BtxDescriptorSelection,
    pub targets: Vec<ClassicBtxRegistrationTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicBtxRegistrationError(String);

impl fmt::Display for ClassicBtxRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "classic BTX VDD 등록 감사 실패: {}", self.0)
    }
}

impl Error for ClassicBtxRegistrationError {}

fn error(message: impl Into<String>) -> ClassicBtxRegistrationError {
    ClassicBtxRegistrationError(message.into())
}

fn descriptor_resource_id(
    descriptor_table_runtime: u32,
    descriptor_runtime: u32,
) -> Result<u16, ClassicBtxRegistrationError> {
    let delta = descriptor_runtime
        .checked_sub(descriptor_table_runtime)
        .ok_or_else(|| error("target descriptor가 table 앞임"))?;
    if !delta.is_multiple_of(VDD_DESCRIPTOR_STRIDE) {
        return Err(error(format!(
            "target descriptor가 36바이트 격자 밖임: delta=0x{delta:X}"
        )));
    }
    u16::try_from(delta / VDD_DESCRIPTOR_STRIDE)
        .map_err(|_| error("target resource ID가 u16 범위를 넘음"))
}

fn exact_name_link<'a>(
    names: &'a BtxNameAnalysis,
    asset_name: &str,
) -> Result<&'a BtxNameLink, ClassicBtxRegistrationError> {
    let links = names
        .links
        .iter()
        .filter(|link| {
            link.exe == PUYO1_EXE
                && link.archive == PP1_GRP_ARCHIVE
                && link.asset_name == asset_name
        })
        .collect::<Vec<_>>();
    match links.as_slice() {
        [link] => Ok(*link),
        _ => Err(error(format!(
            "PUYO1 {asset_name} 이름표 연결 수가 1이 아님: {}",
            links.len()
        ))),
    }
}

fn exact_descriptor_selection(
    selections: &BtxDescriptorSelectionAnalysis,
) -> Result<BtxDescriptorSelection, ClassicBtxRegistrationError> {
    let matches = selections
        .selections
        .iter()
        .filter(|selection| selection.exe == PUYO1_EXE && selection.archive == PP1_GRP_ARCHIVE)
        .copied()
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [selection] => Ok(*selection),
        _ => Err(error(format!(
            "PUYO1 PP1_GRP descriptor 선택 경계 수가 1이 아님: {}",
            matches.len()
        ))),
    }
}

/// 지원 원본에서 classic BTX 두 자산의 type-0 VDD 등록을 검증한다.
pub fn verify_box_classic_btx_registration<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
    names: &BtxNameAnalysis,
    selections: &BtxDescriptorSelectionAnalysis,
) -> Result<ClassicBtxRegistrationAnalysis, ClassicBtxRegistrationError> {
    let executable_map = executables
        .into_iter()
        .map(|(map, text)| (map.name, (map, text)))
        .collect::<BTreeMap<_, _>>();
    let &(map, text) = executable_map
        .get(PUYO1_EXE)
        .ok_or_else(|| error("PUYO1.EXE 입력이 없음"))?;
    verify_vdd_registration_source(map, text, PP1_GRP_REGISTRATION)
        .map_err(|source| error(source.to_string()))?;
    let descriptor_selection = exact_descriptor_selection(selections)?;

    let archive_map = archives.into_iter().collect::<BTreeMap<_, _>>();
    let archive = archive_map
        .get(PP1_GRP_ARCHIVE)
        .copied()
        .ok_or_else(|| error("PP1_GRP.VDD 입력이 없음"))?;
    let mut targets = Vec::with_capacity(CLASSIC_BTX_REGISTRATION_TARGETS.len());
    for spec in CLASSIC_BTX_REGISTRATION_TARGETS {
        let entries = archive
            .entries()
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.name() == spec.asset_name)
            .collect::<Vec<_>>();
        let [(vdd_entry_index, _)] = entries.as_slice() else {
            return Err(error(format!(
                "PP1_GRP {} VDD entry 수가 1이 아님: {}",
                spec.asset_name,
                entries.len()
            )));
        };
        if *vdd_entry_index != spec.vdd_entry_index {
            return Err(error(format!(
                "{} VDD index 불일치: expected={}, actual={vdd_entry_index}",
                spec.asset_name, spec.vdd_entry_index
            )));
        }

        let link = exact_name_link(names, spec.asset_name)?;
        let descriptor_runtime = link
            .descriptor_pointer_runtime
            .checked_sub(4)
            .ok_or_else(|| error("descriptor name pointer 주소 underflow"))?;
        let resource_id = descriptor_resource_id(
            PP1_GRP_REGISTRATION.archive_descriptor_runtime,
            descriptor_runtime,
        )?;
        if link.descriptor_runtime_start != PP1_GRP_REGISTRATION.archive_descriptor_runtime
            || descriptor_runtime != spec.descriptor_runtime
            || resource_id != spec.resource_id
            || usize::from(resource_id)
                != spec.vdd_entry_index + usize::from(PP1_GRP_PREFIX_DESCRIPTOR_ROWS)
        {
            return Err(error(format!(
                "{} descriptor/resource 연결 불일치: table=0x{:08X}, descriptor=0x{descriptor_runtime:08X}, resource={resource_id}",
                spec.asset_name, link.descriptor_runtime_start
            )));
        }
        targets.push(ClassicBtxRegistrationTarget {
            asset_name: spec.asset_name,
            vdd_entry_index: spec.vdd_entry_index,
            resource_id,
            descriptor_runtime,
        });
    }

    Ok(ClassicBtxRegistrationAnalysis {
        exe: PUYO1_EXE,
        archive: PP1_GRP_ARCHIVE,
        resource_type: PP1_GRP_REGISTRATION.resource_type,
        registration_record_runtime: PP1_GRP_REGISTRATION.registration_record_runtime,
        registration_handler_runtime: PP1_GRP_REGISTRATION.registration_handler_runtime,
        loader_runtime: PP1_GRP_REGISTRATION.loader_runtime,
        table_root_runtime: PP1_GRP_REGISTRATION.table_root_runtime,
        descriptor_table_runtime: PP1_GRP_REGISTRATION.archive_descriptor_runtime,
        prefix_descriptor_rows: PP1_GRP_PREFIX_DESCRIPTOR_ROWS,
        descriptor_selection,
        targets,
    })
}

#[cfg(test)]
#[path = "classic_btx_registration_tests.rs"]
mod tests;
