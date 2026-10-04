//! 비-QUEST SEQ descriptor block 등록 경계.
//!
//! 세 EXE의 command `0x29` type 1 등록을 공용 VDD 등록 검증 위에서 묶고,
//! 대표 SEQ 자산의 exact resource ID를 검산한다. 실제 장면 record가 그 ID를
//! 골라 적재·표시한다는 주장은 아직 하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;

use super::subgame_seq_descriptor::SubgameSeqDescriptorAnalysis;
use super::vdd_registration::{
    verify_vdd_registration_source, VddRegistrationFamily, VddRegistrationSourceSpec,
    VDD_DESCRIPTOR_STRIDE, VDD_REGISTRATION_OPCODE,
};

pub const SUBGAME_SEQ_REGISTRATION_SCHEMA_VERSION: &str = "subgame-seq-registrations-v1";
pub const BOX_SUBGAME_SEQ_REGISTRATION_ROWS: usize = 3;
pub const SUBGAME_SEQ_REGISTRATION_OPCODE: u8 = VDD_REGISTRATION_OPCODE;
pub const SUBGAME_SEQ_TYPE: u8 = 1;
pub const SUBGAME_SEQ_DESCRIPTOR_STRIDE: u32 = VDD_DESCRIPTOR_STRIDE;

/// 지원 원본에서 생성한 3행 TSV의 SHA-256.
pub const BOX_SUBGAME_SEQ_REGISTRATION_REPORT_SHA256: [u8; 32] = [
    0xE4, 0x13, 0x29, 0x0D, 0xCA, 0x07, 0x94, 0x2E, 0x33, 0x11, 0x6B, 0x16, 0x04, 0x67, 0x72, 0x06,
    0x76, 0x20, 0x23, 0x77, 0x6F, 0x30, 0xBB, 0x8D, 0x23, 0x81, 0xFF, 0x4A, 0x78, 0xFD, 0x58, 0x20,
];

/// EXE별 SEQ type 등록과 대표 resource ID의 exact 사양.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqRegistrationSpec {
    pub exe: &'static str,
    pub archive: &'static str,
    pub asset: &'static str,
    pub vdd_index: usize,
    pub registration_record_runtime: u32,
    pub registration_handler_runtime: u32,
    pub loader_runtime: u32,
    pub table_root_runtime: u32,
    pub archive_descriptor_runtime: u32,
    pub archive_name_runtime: u32,
    pub asset_descriptor_runtime: u32,
    pub resource_id: u16,
    pub prefix_descriptor_rows: u16,
    family: VddRegistrationFamily,
}

impl SubgameSeqRegistrationSpec {
    fn source(self) -> VddRegistrationSourceSpec {
        VddRegistrationSourceSpec {
            exe: self.exe,
            resource_type: SUBGAME_SEQ_TYPE,
            archive: self.archive,
            registration_record_runtime: self.registration_record_runtime,
            registration_handler_runtime: self.registration_handler_runtime,
            loader_runtime: self.loader_runtime,
            table_root_runtime: self.table_root_runtime,
            archive_descriptor_runtime: self.archive_descriptor_runtime,
            archive_name_runtime: self.archive_name_runtime,
            family: self.family,
        }
    }
}

pub const BOX_SUBGAME_SEQ_REGISTRATION_SPECS: [SubgameSeqRegistrationSpec; 3] = [
    SubgameSeqRegistrationSpec {
        exe: "PCOMI.EXE",
        archive: "PC1_SEQ.VDD",
        asset: "MCRD1.SEQ",
        vdd_index: 1,
        registration_record_runtime: 0x8002_F2C4,
        registration_handler_runtime: 0x8001_8A8C,
        loader_runtime: 0x8001_2F4C,
        table_root_runtime: 0x8007_7888,
        archive_descriptor_runtime: 0x8002_F764,
        archive_name_runtime: 0x8002_F7F4,
        asset_descriptor_runtime: 0x8002_F7AC,
        resource_id: 2,
        prefix_descriptor_rows: 1,
        family: VddRegistrationFamily::Pcomi,
    },
    SubgameSeqRegistrationSpec {
        exe: "PUYO1.EXE",
        archive: "PP1_SEQ.VDD",
        asset: "DRAC_SEQ.SEQ",
        vdd_index: 39,
        registration_record_runtime: 0x8006_7B60,
        registration_handler_runtime: 0x8002_4334,
        loader_runtime: 0x8001_B3B4,
        table_root_runtime: 0x800B_4330,
        archive_descriptor_runtime: 0x8006_9EEC,
        archive_name_runtime: 0x8006_B788,
        asset_descriptor_runtime: 0x8006_A564,
        resource_id: 46,
        prefix_descriptor_rows: 7,
        family: VddRegistrationFamily::Shared,
    },
    SubgameSeqRegistrationSpec {
        exe: "PUYO2.EXE",
        archive: "PP2_SEQ.VDD",
        asset: "DRAC_SEQ.SEQ",
        vdd_index: 42,
        registration_record_runtime: 0x8006_3DE0,
        registration_handler_runtime: 0x8002_3384,
        loader_runtime: 0x8001_9E40,
        table_root_runtime: 0x800B_6710,
        archive_descriptor_runtime: 0x8006_6824,
        archive_name_runtime: 0x8006_8564,
        asset_descriptor_runtime: 0x8006_6E9C,
        resource_id: 46,
        prefix_descriptor_rows: 4,
        family: VddRegistrationFamily::Shared,
    },
];

/// SEQ VDD 등록과 대표 resource 연결 한 행.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqRegistrationRow {
    pub exe: &'static str,
    pub archive: &'static str,
    pub asset: &'static str,
    pub vdd_index: usize,
    pub registration_record_runtime: u32,
    pub registration_handler_runtime: u32,
    pub loader_runtime: u32,
    pub table_root_runtime: u32,
    pub archive_descriptor_runtime: u32,
    pub archive_name_runtime: u32,
    pub asset_descriptor_runtime: u32,
    pub resource_id: u16,
    pub prefix_descriptor_rows: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqRegistrationAnalysis {
    pub rows: Vec<SubgameSeqRegistrationRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqRegistrationError(String);

impl fmt::Display for SubgameSeqRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for SubgameSeqRegistrationError {}

fn error(message: impl Into<String>) -> SubgameSeqRegistrationError {
    SubgameSeqRegistrationError(message.into())
}

fn audit_subgame_seq_registrations<'a>(
    specs: &[SubgameSeqRegistrationSpec],
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    descriptors: &SubgameSeqDescriptorAnalysis,
) -> Result<SubgameSeqRegistrationAnalysis, SubgameSeqRegistrationError> {
    if specs.is_empty() {
        return Err(error("SEQ registration spec이 없음"));
    }
    let executable_map = executables
        .into_iter()
        .map(|(map, text)| (map.name, (map, text)))
        .collect::<BTreeMap<_, _>>();
    let descriptor_map = descriptors
        .rows
        .iter()
        .map(|row| (row.exe, row))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut rows = Vec::with_capacity(specs.len());

    for &spec in specs {
        if !seen.insert(spec.exe) {
            return Err(error(format!(
                "SEQ registration spec EXE 중복: {}",
                spec.exe
            )));
        }
        let &(map, text) = executable_map
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} 검증 입력이 없음", spec.exe)))?;
        let descriptor = descriptor_map
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} descriptor 분석 행이 없음", spec.exe)))?;
        if descriptor.asset != spec.asset
            || descriptor.vdd_index != spec.vdd_index
            || descriptor.descriptor_runtime != spec.asset_descriptor_runtime
        {
            return Err(error(format!(
                "{} 대표 descriptor 분석과 registration spec 불일치",
                spec.exe
            )));
        }
        let expected_resource = spec
            .archive_descriptor_runtime
            .checked_add(u32::from(spec.resource_id) * SUBGAME_SEQ_DESCRIPTOR_STRIDE)
            .ok_or_else(|| error("SEQ resource 주소 overflow"))?;
        if expected_resource != spec.asset_descriptor_runtime
            || usize::from(spec.resource_id)
                != spec.vdd_index + usize::from(spec.prefix_descriptor_rows)
        {
            return Err(error(format!(
                "{} resource ID 환산 불일치: archive=0x{:08X}, resource={}, asset=0x{:08X}",
                spec.exe,
                spec.archive_descriptor_runtime,
                spec.resource_id,
                spec.asset_descriptor_runtime
            )));
        }

        verify_vdd_registration_source(map, text, spec.source())
            .map_err(|source| error(source.to_string()))?;
        rows.push(SubgameSeqRegistrationRow {
            exe: spec.exe,
            archive: spec.archive,
            asset: spec.asset,
            vdd_index: spec.vdd_index,
            registration_record_runtime: spec.registration_record_runtime,
            registration_handler_runtime: spec.registration_handler_runtime,
            loader_runtime: spec.loader_runtime,
            table_root_runtime: spec.table_root_runtime,
            archive_descriptor_runtime: spec.archive_descriptor_runtime,
            archive_name_runtime: spec.archive_name_runtime,
            asset_descriptor_runtime: spec.asset_descriptor_runtime,
            resource_id: spec.resource_id,
            prefix_descriptor_rows: spec.prefix_descriptor_rows,
        });
    }
    if executable_map.len() != specs.len() || descriptor_map.len() != specs.len() {
        return Err(error(format!(
            "SEQ registration 입력 집합 불일치: specs={}, EXE={}, descriptors={}",
            specs.len(),
            executable_map.len(),
            descriptor_map.len()
        )));
    }
    Ok(SubgameSeqRegistrationAnalysis { rows })
}

/// 지원 원본의 type 1 SEQ 등록과 대표 resource ID를 검증한다.
pub fn verify_box_subgame_seq_registrations<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    descriptors: &SubgameSeqDescriptorAnalysis,
) -> Result<SubgameSeqRegistrationAnalysis, SubgameSeqRegistrationError> {
    let analysis = audit_subgame_seq_registrations(
        &BOX_SUBGAME_SEQ_REGISTRATION_SPECS,
        executables,
        descriptors,
    )?;
    if analysis.rows.len() != BOX_SUBGAME_SEQ_REGISTRATION_ROWS {
        return Err(error(format!(
            "SEQ registration 행 수 불일치: expected={}, actual={}",
            BOX_SUBGAME_SEQ_REGISTRATION_ROWS,
            analysis.rows.len()
        )));
    }
    let report = render_subgame_seq_registrations_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_REGISTRATION_REPORT_SHA256 {
        return Err(error(format!(
            "SEQ registration report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_REGISTRATION_REPORT_SHA256),
            hex(&digest)
        )));
    }
    Ok(analysis)
}

/// 검증 결과를 결정적 3행 TSV로 직렬화한다.
pub fn render_subgame_seq_registrations_tsv(analysis: &SubgameSeqRegistrationAnalysis) -> String {
    let mut output = String::from(
        "schema_version\texe\tarchive\tasset\tvdd_index\tregistration_record_runtime\t\
         registration_handler_runtime\tloader_runtime\ttable_root_runtime\t\
         archive_descriptor_runtime\tarchive_name_runtime\tasset_descriptor_runtime\t\
         resource_id\tprefix_descriptor_rows\tstatic_status\n",
    );
    for row in &analysis.rows {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t\
             0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\t\
             type1-registration-and-selectable-resource-only",
            SUBGAME_SEQ_REGISTRATION_SCHEMA_VERSION,
            row.exe,
            row.archive,
            row.asset,
            row.vdd_index,
            row.registration_record_runtime,
            row.registration_handler_runtime,
            row.loader_runtime,
            row.table_root_runtime,
            row.archive_descriptor_runtime,
            row.archive_name_runtime,
            row.asset_descriptor_runtime,
            row.resource_id,
            row.prefix_descriptor_rows,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_registration_tests.rs"]
mod tests;
