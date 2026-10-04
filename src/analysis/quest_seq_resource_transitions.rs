//! OP8A 정적 frontier의 resource ID를 `RPG_SEQ.VDD` entry에 결속한다.
//!
//! 공용 등록기가 증명한 archive descriptor 선행 행과 36바이트 stride를
//! 입력으로 받아 `resource ID -> VDD entry index -> SEQ 이름`을 계산한다.
//! row는 정적 candidate path의 중단점이며 정상 실행 도달을 주장하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::quest_seq_text_frontiers::{
    QuestSeqTextFrontierAnalysis, QuestSeqTextFrontierParameter, QuestSeqTextFrontierRequirement,
};

pub const QUEST_SEQ_RESOURCE_TRANSITION_SCHEMA_VERSION: &str = "quest-seq-resource-transitions-v1";
pub const BOX_QUEST_SEQ_RESOURCE_CATALOG_ENTRIES: usize = 113;
pub const BOX_QUEST_SEQ_RESOURCE_TRANSITION_ROWS: usize = 16;
pub const BOX_QUEST_SEQ_RESOURCE_TRANSITION_PATH_OCCURRENCES: usize = 16;
pub const BOX_QUEST_SEQ_RESOURCE_TRANSITION_UNIQUE_DIRECT_TARGETS: usize = 2;
pub const BOX_QUEST_SEQ_RESOURCE_TRANSITION_RUNTIME_DEFAULTS: usize = 1;
pub const BOX_QUEST_SEQ_RESOURCE_ID_EXPECTATIONS: [(u16, usize); 3] = [(0, 1), (1, 14), (9, 1)];
pub const BOX_QUEST_SEQ_RESOURCE_TRANSITION_REPORT_SHA256: &str =
    "f98ce38f29f07dab514f9b1cfd11516a097196dc0b3b4d209e443a0fef3ee18a";

/// OP8A resource ID를 descriptor와 VDD entry에 연결하는 좌표.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqResourceCatalogLayout {
    pub descriptor_root_runtime: u32,
    pub descriptor_stride: u32,
    pub prefix_descriptor_rows: usize,
}

/// 정적 path에서 찾은 OP8A resource 전환 후보 한 행.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqResourceTransition {
    pub source_sequence: String,
    pub source_sha256: [u8; 32],
    pub record_offset: usize,
    pub record_runtime: u32,
    pub base_evidence: &'static str,
    pub path_occurrences: usize,
    pub encoded_resource_id: u16,
    pub descriptor_runtime: Option<u32>,
    pub vdd_entry_index: Option<usize>,
    pub target_sequence: Option<String>,
}

/// OP8A 정적 전환 후보와 `RPG_SEQ.VDD` catalog 분모.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqResourceTransitionAnalysis {
    pub layout: QuestSeqResourceCatalogLayout,
    pub resource_catalog_entries: usize,
    pub transitions: Vec<QuestSeqResourceTransition>,
    pub path_occurrences: usize,
    pub unique_target_resources: usize,
    pub runtime_default_transitions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqResourceTransitionError(String);

impl fmt::Display for QuestSeqResourceTransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestSeqResourceTransitionError {}

fn error(message: impl Into<String>) -> QuestSeqResourceTransitionError {
    QuestSeqResourceTransitionError(message.into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedQuestSeqResource {
    pub descriptor_runtime: u32,
    pub vdd_entry_index: usize,
    pub target_sequence: String,
}

pub(crate) fn collect_quest_seq_resource_catalog<'a>(
    resource_catalog: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<String>, QuestSeqResourceTransitionError> {
    let mut seen = BTreeSet::new();
    let mut catalog = Vec::new();
    for name in resource_catalog {
        if name.is_empty() {
            return Err(error("QUEST SEQ resource catalog 이름이 비어 있음"));
        }
        if !seen.insert(name.to_ascii_uppercase()) {
            return Err(error(format!(
                "QUEST SEQ resource catalog 이름 중복: {name}"
            )));
        }
        catalog.push(name.to_string());
    }
    if catalog.is_empty() {
        return Err(error("QUEST SEQ resource catalog가 비어 있음"));
    }
    Ok(catalog)
}

pub(crate) fn resolve_quest_seq_direct_resource(
    catalog: &[String],
    layout: QuestSeqResourceCatalogLayout,
    resource_id: u16,
    source: &str,
) -> Result<ResolvedQuestSeqResource, QuestSeqResourceTransitionError> {
    if layout.descriptor_stride == 0 {
        return Err(error("QUEST SEQ resource descriptor stride가 0임"));
    }
    if resource_id == 0 {
        return Err(error(format!(
            "QUEST SEQ resource ID 0은 runtime 기본값이 필요함: {source}"
        )));
    }
    let vdd_entry_index = usize::from(resource_id)
        .checked_sub(layout.prefix_descriptor_rows)
        .ok_or_else(|| error("QUEST SEQ OP8A resource index underflow"))?;
    let target_sequence = catalog.get(vdd_entry_index).ok_or_else(|| {
        error(format!(
            "QUEST SEQ OP8A resource ID가 VDD catalog 밖임: {source} ID {resource_id}, index {vdd_entry_index}"
        ))
    })?;
    let descriptor_runtime = layout
        .descriptor_stride
        .checked_mul(u32::from(resource_id))
        .and_then(|delta| layout.descriptor_root_runtime.checked_add(delta))
        .ok_or_else(|| error("QUEST SEQ OP8A descriptor 주소 overflow"))?;
    Ok(ResolvedQuestSeqResource {
        descriptor_runtime,
        vdd_entry_index,
        target_sequence: target_sequence.clone(),
    })
}

/// OP8A frontier의 ID를 명시적으로 순서가 보존된 VDD catalog에 결속한다.
pub fn audit_quest_seq_resource_transitions<'a>(
    resource_catalog: impl IntoIterator<Item = &'a str>,
    layout: QuestSeqResourceCatalogLayout,
    frontiers: &QuestSeqTextFrontierAnalysis,
) -> Result<QuestSeqResourceTransitionAnalysis, QuestSeqResourceTransitionError> {
    if layout.descriptor_stride == 0 {
        return Err(error("QUEST SEQ resource descriptor stride가 0임"));
    }
    let catalog = collect_quest_seq_resource_catalog(resource_catalog)?;

    let mut transitions = Vec::new();
    for frontier in frontiers.frontiers.iter().filter(|frontier| {
        frontier.requirement == QuestSeqTextFrontierRequirement::ResourceTransitionState
    }) {
        if frontier.opcode != 0x8A {
            return Err(error(format!(
                "QUEST SEQ resource frontier opcode 불일치: {} +0x{:04X} OP{:02X}",
                frontier.sequence, frontier.terminal_offset, frontier.opcode
            )));
        }
        let Some(QuestSeqTextFrontierParameter::ResourceId { id }) = frontier.parameter else {
            return Err(error(format!(
                "QUEST SEQ OP8A frontier resource ID 누락: {} +0x{:04X}",
                frontier.sequence, frontier.terminal_offset
            )));
        };
        let (descriptor_runtime, vdd_entry_index, target_sequence) = if id == 0 {
            // OP8A handler는 0을 그대로 descriptor에 적용하지 않고 runtime
            // 기본 resource ID로 치환한다. 해당 값은 동적 상태로 남긴다.
            (None, None, None)
        } else {
            let resolved = resolve_quest_seq_direct_resource(
                &catalog,
                layout,
                id,
                &format!("{} +0x{:04X}", frontier.sequence, frontier.terminal_offset),
            )?;
            (
                Some(resolved.descriptor_runtime),
                Some(resolved.vdd_entry_index),
                Some(resolved.target_sequence),
            )
        };
        transitions.push(QuestSeqResourceTransition {
            source_sequence: frontier.sequence.clone(),
            source_sha256: frontier.sequence_sha256,
            record_offset: frontier.terminal_offset,
            record_runtime: frontier.terminal_runtime,
            base_evidence: frontier.base_evidence.label(),
            path_occurrences: frontier.path_occurrences,
            encoded_resource_id: id,
            descriptor_runtime,
            vdd_entry_index,
            target_sequence,
        });
    }
    transitions.sort_by(|left, right| {
        (
            &left.source_sequence,
            left.record_offset,
            left.encoded_resource_id,
        )
            .cmp(&(
                &right.source_sequence,
                right.record_offset,
                right.encoded_resource_id,
            ))
    });
    let path_occurrences = transitions
        .iter()
        .map(|transition| transition.path_occurrences)
        .sum();
    let unique_target_resources = transitions
        .iter()
        .filter_map(|transition| transition.vdd_entry_index)
        .collect::<BTreeSet<_>>()
        .len();
    let runtime_default_transitions = transitions
        .iter()
        .filter(|transition| transition.encoded_resource_id == 0)
        .count();

    Ok(QuestSeqResourceTransitionAnalysis {
        layout,
        resource_catalog_entries: catalog.len(),
        transitions,
        path_occurrences,
        unique_target_resources,
        runtime_default_transitions,
    })
}

/// 지원 원본의 OP8A 정적 전환 분모와 결정적 report를 검증한다.
pub fn verify_box_quest_seq_resource_transitions<'a>(
    resource_catalog: impl IntoIterator<Item = &'a str>,
    layout: QuestSeqResourceCatalogLayout,
    frontiers: &QuestSeqTextFrontierAnalysis,
) -> Result<QuestSeqResourceTransitionAnalysis, QuestSeqResourceTransitionError> {
    let analysis = audit_quest_seq_resource_transitions(resource_catalog, layout, frontiers)?;
    if analysis.resource_catalog_entries != BOX_QUEST_SEQ_RESOURCE_CATALOG_ENTRIES
        || analysis.transitions.len() != BOX_QUEST_SEQ_RESOURCE_TRANSITION_ROWS
        || analysis.path_occurrences != BOX_QUEST_SEQ_RESOURCE_TRANSITION_PATH_OCCURRENCES
        || analysis.unique_target_resources
            != BOX_QUEST_SEQ_RESOURCE_TRANSITION_UNIQUE_DIRECT_TARGETS
        || analysis.runtime_default_transitions
            != BOX_QUEST_SEQ_RESOURCE_TRANSITION_RUNTIME_DEFAULTS
    {
        return Err(error(format!(
            "QUEST SEQ resource transition 분모 불일치: \
             expected={}/{}/{}/{}/{}, actual={}/{}/{}/{}/{}",
            BOX_QUEST_SEQ_RESOURCE_CATALOG_ENTRIES,
            BOX_QUEST_SEQ_RESOURCE_TRANSITION_ROWS,
            BOX_QUEST_SEQ_RESOURCE_TRANSITION_PATH_OCCURRENCES,
            BOX_QUEST_SEQ_RESOURCE_TRANSITION_UNIQUE_DIRECT_TARGETS,
            BOX_QUEST_SEQ_RESOURCE_TRANSITION_RUNTIME_DEFAULTS,
            analysis.resource_catalog_entries,
            analysis.transitions.len(),
            analysis.path_occurrences,
            analysis.unique_target_resources,
            analysis.runtime_default_transitions,
        )));
    }
    let resource_id_counts = BOX_QUEST_SEQ_RESOURCE_ID_EXPECTATIONS.map(|(id, _)| {
        (
            id,
            analysis
                .transitions
                .iter()
                .filter(|transition| transition.encoded_resource_id == id)
                .count(),
        )
    });
    if resource_id_counts != BOX_QUEST_SEQ_RESOURCE_ID_EXPECTATIONS {
        return Err(error(format!(
            "QUEST SEQ OP8A resource ID 분모 불일치: \
             expected={BOX_QUEST_SEQ_RESOURCE_ID_EXPECTATIONS:?}, actual={resource_id_counts:?}"
        )));
    }
    let report = render_quest_seq_resource_transitions_tsv(&analysis);
    let digest = format!("{:x}", Sha256::digest(report.as_bytes()));
    if digest != BOX_QUEST_SEQ_RESOURCE_TRANSITION_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST SEQ resource transition report SHA-256 불일치: \
             expected={BOX_QUEST_SEQ_RESOURCE_TRANSITION_REPORT_SHA256}, actual={digest}"
        )));
    }
    Ok(analysis)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 정적 선택 결속과 실행 미관측 경계를 보존하는 TSV.
pub fn render_quest_seq_resource_transitions_tsv(
    analysis: &QuestSeqResourceTransitionAnalysis,
) -> String {
    let mut output = String::from(
        "schema_version\tarchive_path\tresource_catalog_entries\t\
         descriptor_root_runtime\tdescriptor_stride\tprefix_descriptor_rows\t\
         source_sequence\tsource_sha256\trecord_offset\trecord_runtime\tbase_evidence\t\
         path_occurrences\tencoded_resource_id\tresolution_requirement\t\
         descriptor_runtime\tvdd_entry_index\ttarget_sequence\tselection_evidence\t\
         execution_evidence\n",
    );
    for transition in &analysis.transitions {
        writeln!(
            output,
            "{}\t/QUEST/RPG_SEQ.VDD;1\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t\
             0x{:04X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
             static-path-frontier\tnot-observed",
            QUEST_SEQ_RESOURCE_TRANSITION_SCHEMA_VERSION,
            analysis.resource_catalog_entries,
            analysis.layout.descriptor_root_runtime,
            analysis.layout.descriptor_stride,
            analysis.layout.prefix_descriptor_rows,
            transition.source_sequence,
            hex(&transition.source_sha256),
            transition.record_offset,
            transition.record_runtime,
            transition.base_evidence,
            transition.path_occurrences,
            transition.encoded_resource_id,
            if transition.encoded_resource_id == 0 {
                "runtime-default-state-required"
            } else {
                "direct-resource-id"
            },
            transition
                .descriptor_runtime
                .map_or_else(String::new, |runtime| format!("0x{runtime:08X}")),
            transition
                .vdd_entry_index
                .map_or_else(String::new, |index| index.to_string()),
            transition.target_sequence.as_deref().unwrap_or_default(),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_seq_resource_transitions_tests.rs"]
mod tests;
