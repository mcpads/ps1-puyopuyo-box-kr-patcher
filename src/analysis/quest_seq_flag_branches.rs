//! QUEST 확장 SEQ `OPA1` flag 분기 record.
//!
//! 공용 handler가 조회하는 flag operand, 비교 기대값, branch와 fallthrough를
//! 한 record로 보존한다. 정적 분석은 두 다음 위치를 모두 제공할 뿐 record의
//! 정상 실행 도달이나 당시 flag 값을 주장하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::RuntimeAssetMap;
use crate::disasm::{
    resolve_quest_seq_flag_probe, QuestSeqFlagProbe, QUEST_SEQ_FLAG_BASE_OFFSET,
    QUEST_SEQ_FLAG_BRANCH_POINTER_OFFSET, QUEST_SEQ_FLAG_EXPECTED_OFFSET,
    QUEST_SEQ_FLAG_OFFSET_OFFSET, QUEST_SEQ_FLAG_QUERY_OPCODE, QUEST_SEQ_FLAG_QUERY_RECORD_BYTES,
};

use crate::script::quest_seq::QUEST_SEQ_ACTIVE_RUNTIME_BASE;
use crate::script::quest_seq_inventory::{
    QuestSeqAddressAnalysis, QuestSeqRuntimeBaseEvidence, BOX_QUEST_SEQ_POINTER_RECORD_COUNTS,
    QUEST_SEQ_POINTER_RECORD_SPECS,
};

pub const QUEST_SEQ_FLAG_BRANCH_REPORT_KIND: &str = "quest-seq-flag-branches";
pub const BOX_QUEST_SEQ_FLAG_BRANCH_REPORT_SHA256: &str =
    "ea4826179362cf849b8ee79704fc07658f7c6eacf7bef39ba86831b2d4e2b59d";
pub const BOX_QUEST_SUKE_CONFRONTATION_FLAG_RECORD_OFFSET: usize = 0x0764;
pub const BOX_QUEST_SUKE_CONFRONTATION_FALLTHROUGH_OFFSET: usize = 0x0770;
pub const BOX_QUEST_SUKE_CONFRONTATION_BRANCH_OFFSET: usize = 0x0E14;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqFlagBranchSelection {
    Branch,
    Fallthrough,
}

impl QuestSeqFlagBranchSelection {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Branch => "branch",
            Self::Fallthrough => "fallthrough",
        }
    }
}

/// `OPA1` 한 record와 두 가능한 다음 위치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqFlagBranch {
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub runtime_base_evidence: QuestSeqRuntimeBaseEvidence,
    pub record_offset: usize,
    pub record_runtime: u32,
    pub raw: [u8; QUEST_SEQ_FLAG_QUERY_RECORD_BYTES],
    pub expected_value: u8,
    pub flag_base: u16,
    pub flag_offset: u8,
    /// handler가 읽지 않는 record `+5..+7`.
    pub unconsumed_parameters: [u8; 3],
    pub probe: QuestSeqFlagProbe,
    pub branch_target_offset: usize,
    pub branch_target_runtime: u32,
    pub branch_target_opcode: u8,
    pub fallthrough_offset: usize,
    pub fallthrough_runtime: u32,
    pub fallthrough_opcode: u8,
}

impl QuestSeqFlagBranch {
    /// 관측한 probe word로 handler가 고를 다음 위치를 판정한다.
    pub const fn selection(&self, observed_word: u32) -> QuestSeqFlagBranchSelection {
        if self.probe.queried_value(observed_word) == self.expected_value {
            QuestSeqFlagBranchSelection::Fallthrough
        } else {
            QuestSeqFlagBranchSelection::Branch
        }
    }

    pub const fn selected_offset(&self, observed_word: u32) -> usize {
        match self.selection(observed_word) {
            QuestSeqFlagBranchSelection::Branch => self.branch_target_offset,
            QuestSeqFlagBranchSelection::Fallthrough => self.fallthrough_offset,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqFlagBranchAnalysis {
    pub branches: Vec<QuestSeqFlagBranch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqFlagBranchError(String);

impl fmt::Display for QuestSeqFlagBranchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestSeqFlagBranchError {}

fn error(message: impl Into<String>) -> QuestSeqFlagBranchError {
    QuestSeqFlagBranchError(message.into())
}

/// 한 SEQ 안의 지정 위치를 `OPA1` record로 해석한다.
pub fn parse_quest_seq_flag_branch(
    sequence: &str,
    raw: &[u8],
    runtime_base_evidence: QuestSeqRuntimeBaseEvidence,
    record_offset: usize,
) -> Result<Option<QuestSeqFlagBranch>, QuestSeqFlagBranchError> {
    if raw.get(record_offset) != Some(&QUEST_SEQ_FLAG_QUERY_OPCODE) {
        return Ok(None);
    }
    let record_end = record_offset
        .checked_add(QUEST_SEQ_FLAG_QUERY_RECORD_BYTES)
        .ok_or_else(|| error("QUEST OPA1 record 끝 offset overflow"))?;
    let record: [u8; QUEST_SEQ_FLAG_QUERY_RECORD_BYTES] = raw
        .get(record_offset..record_end)
        .ok_or_else(|| {
            error(format!(
                "{sequence}: QUEST OPA1 record가 잘림: 0x{record_offset:04X}"
            ))
        })?
        .try_into()
        .expect("12바이트 범위를 확인함");
    let raw_len = u32::try_from(raw.len())
        .map_err(|_| error(format!("{sequence}: SEQ 길이가 u32 범위를 넘음")))?;
    let map = RuntimeAssetMap::new(QUEST_SEQ_ACTIVE_RUNTIME_BASE, raw_len)
        .ok_or_else(|| error(format!("{sequence}: SEQ runtime 사상 overflow")))?;
    let record_runtime = map.runtime_addr(record_offset).ok_or_else(|| {
        error(format!(
            "{sequence}: QUEST OPA1 record가 파일 밖임: 0x{record_offset:04X}"
        ))
    })?;
    let expected_value = record[QUEST_SEQ_FLAG_EXPECTED_OFFSET];
    let flag_base = u16::from_le_bytes(
        record[QUEST_SEQ_FLAG_BASE_OFFSET..QUEST_SEQ_FLAG_BASE_OFFSET + 2]
            .try_into()
            .expect("12바이트 record 안의 halfword"),
    );
    let flag_offset = record[QUEST_SEQ_FLAG_OFFSET_OFFSET];
    let probe = resolve_quest_seq_flag_probe(flag_base, flag_offset)
        .ok_or_else(|| error(format!("{sequence}: QUEST OPA1 flag probe 계산 overflow")))?;
    let branch_target_runtime = u32::from_le_bytes(
        record[QUEST_SEQ_FLAG_BRANCH_POINTER_OFFSET..QUEST_SEQ_FLAG_BRANCH_POINTER_OFFSET + 4]
            .try_into()
            .expect("12바이트 record 안의 pointer"),
    );
    let branch_target_offset = map.file_offset(branch_target_runtime).ok_or_else(|| {
        error(format!(
            "{sequence}: QUEST OPA1 branch target이 파일 밖임: \
             0x{record_offset:04X}->0x{branch_target_runtime:08X}"
        ))
    })?;
    let fallthrough_offset = record_end;
    let fallthrough_runtime = map.runtime_addr(fallthrough_offset).ok_or_else(|| {
        error(format!(
            "{sequence}: QUEST OPA1 fallthrough가 파일 밖임: \
             0x{record_offset:04X}->0x{fallthrough_offset:04X}"
        ))
    })?;
    let branch_target_opcode = *raw.get(branch_target_offset).ok_or_else(|| {
        error(format!(
            "{sequence}: QUEST OPA1 branch opcode가 파일 밖임: 0x{branch_target_offset:04X}"
        ))
    })?;
    let fallthrough_opcode = *raw.get(fallthrough_offset).ok_or_else(|| {
        error(format!(
            "{sequence}: QUEST OPA1 fallthrough opcode가 파일 밖임: 0x{fallthrough_offset:04X}"
        ))
    })?;

    Ok(Some(QuestSeqFlagBranch {
        sequence: sequence.to_string(),
        sequence_sha256: Sha256::digest(raw).into(),
        runtime_base_evidence,
        record_offset,
        record_runtime,
        raw: record,
        expected_value,
        flag_base,
        flag_offset,
        unconsumed_parameters: record[5..8].try_into().expect("12바이트 record의 +5..+7"),
        probe,
        branch_target_offset,
        branch_target_runtime,
        branch_target_opcode,
        fallthrough_offset,
        fallthrough_runtime,
        fallthrough_opcode,
    }))
}

/// 주소 원장에서 source-backed `OPA1` record를 빠짐없이 구조화한다.
pub fn audit_quest_seq_flag_branches<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    addresses: &QuestSeqAddressAnalysis,
) -> Result<QuestSeqFlagBranchAnalysis, QuestSeqFlagBranchError> {
    let mut input = BTreeMap::new();
    for (name, raw) in sequences {
        if name.is_empty() {
            return Err(error("QUEST OPA1 입력 이름이 비어 있음"));
        }
        if input.insert(name.to_string(), raw).is_some() {
            return Err(error(format!("QUEST OPA1 입력 이름 중복: {name}")));
        }
    }
    if input.is_empty() {
        return Err(error("QUEST OPA1 입력이 없음"));
    }

    let mut seen = BTreeSet::new();
    let mut branches = Vec::new();
    for candidate in &addresses.candidates {
        let Some(spec) = candidate
            .pointer_spec
            .filter(|spec| spec.opcode == QUEST_SEQ_FLAG_QUERY_OPCODE)
        else {
            continue;
        };
        let record_offset = candidate
            .field_offset
            .checked_sub(spec.pointer_offset)
            .ok_or_else(|| {
                error(format!(
                    "{}: QUEST OPA1 record 시작 offset underflow",
                    candidate.sequence
                ))
            })?;
        if !seen.insert((candidate.sequence.clone(), record_offset)) {
            return Err(error(format!(
                "{}: QUEST OPA1 record가 주소 원장에 중복됨: 0x{record_offset:04X}",
                candidate.sequence
            )));
        }
        let raw = input.get(&candidate.sequence).ok_or_else(|| {
            error(format!(
                "{}: QUEST OPA1 원본 SEQ가 없음",
                candidate.sequence
            ))
        })?;
        let branch = parse_quest_seq_flag_branch(
            &candidate.sequence,
            raw,
            candidate.runtime_base_evidence,
            record_offset,
        )?
        .ok_or_else(|| {
            error(format!(
                "{}: 주소 원장의 QUEST OPA1 opcode가 사라짐: 0x{record_offset:04X}",
                candidate.sequence
            ))
        })?;
        if branch.branch_target_offset != candidate.target_offset
            || branch.branch_target_runtime != candidate.target_runtime
            || branch.record_runtime
                != candidate
                    .field_runtime
                    .checked_sub(u32::try_from(spec.pointer_offset).expect("작은 pointer offset"))
                    .ok_or_else(|| error("QUEST OPA1 record runtime underflow"))?
        {
            return Err(error(format!(
                "{}: QUEST OPA1 주소 원장과 record 해석이 충돌함: 0x{record_offset:04X}",
                candidate.sequence
            )));
        }
        branches.push(branch);
    }
    branches.sort_unstable_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then(left.record_offset.cmp(&right.record_offset))
    });
    Ok(QuestSeqFlagBranchAnalysis { branches })
}

/// 지원 원본의 `OPA1` 분모와 첫 SUKE 대화 분기를 검증한다.
pub fn verify_box_quest_seq_flag_branches<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    addresses: &QuestSeqAddressAnalysis,
) -> Result<QuestSeqFlagBranchAnalysis, QuestSeqFlagBranchError> {
    let analysis = audit_quest_seq_flag_branches(sequences, addresses)?;
    let expected_branches = QUEST_SEQ_POINTER_RECORD_SPECS
        .iter()
        .position(|spec| spec.opcode == QUEST_SEQ_FLAG_QUERY_OPCODE)
        .map(|index| BOX_QUEST_SEQ_POINTER_RECORD_COUNTS[index])
        .ok_or_else(|| error("QUEST OPA1 pointer record 사양이 없음"))?;
    if analysis.branches.len() != expected_branches {
        return Err(error(format!(
            "QUEST OPA1 분모 불일치: expected={expected_branches}, actual={}",
            analysis.branches.len()
        )));
    }
    if analysis
        .branches
        .iter()
        .any(|branch| !matches!(branch.expected_value, 0 | 1))
    {
        return Err(error("QUEST OPA1 지원 원본에 0/1 밖의 기대값이 있음"));
    }

    let suke = analysis
        .branches
        .iter()
        .find(|branch| {
            branch.sequence == "SUKE.SEQ"
                && branch.record_offset == BOX_QUEST_SUKE_CONFRONTATION_FLAG_RECORD_OFFSET
        })
        .ok_or_else(|| error("SUKE 첫 대화 QUEST OPA1 record가 없음"))?;
    if suke.expected_value != 0
        || suke.flag_base != 0x8000
        || suke.flag_offset != 0x11
        || suke.probe.runtime_address != 0x800D_B8C0
        || suke.probe.mask != 0x0002_0000
        || suke.fallthrough_offset != BOX_QUEST_SUKE_CONFRONTATION_FALLTHROUGH_OFFSET
        || suke.fallthrough_opcode != 0x21
        || suke.branch_target_offset != BOX_QUEST_SUKE_CONFRONTATION_BRANCH_OFFSET
        || suke.branch_target_opcode != 0x21
    {
        return Err(error(format!(
            "SUKE 첫 대화 QUEST OPA1 의미 불일치: {suke:?}"
        )));
    }
    let report = render_quest_seq_flag_branches_tsv(&analysis);
    let digest = format!("{:x}", Sha256::digest(report.as_bytes()));
    if digest != BOX_QUEST_SEQ_FLAG_BRANCH_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST OPA1 flag branch report SHA-256 불일치: \
             expected={BOX_QUEST_SEQ_FLAG_BRANCH_REPORT_SHA256}, actual={digest}"
        )));
    }
    Ok(analysis)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// `OPA1`의 두 후보와 runtime probe를 보존하는 결정적 TSV.
pub fn render_quest_seq_flag_branches_tsv(analysis: &QuestSeqFlagBranchAnalysis) -> String {
    let mut output = String::from(
        "report_kind\tsequence_name\tsequence_sha256\tsequence_runtime_base\t\
         runtime_base_evidence\trecord_offset\trecord_runtime\trecord_raw\t\
         expected_value\tflag_base\tflag_offset\tunconsumed_parameters\t\
         probe_word_index\tprobe_address\t\
         probe_bit_index\tprobe_mask\tbranch_target_offset\tbranch_target_runtime\t\
         branch_target_opcode\tfallthrough_offset\tfallthrough_runtime\t\
         fallthrough_opcode\n",
    );
    for branch in &analysis.branches {
        writeln!(
            output,
            "{}\t{}\t{}\t0x{:08X}\t{}\t0x{:04X}\t0x{:08X}\t{}\t{}\t\
             0x{:04X}\t0x{:02X}\t{}\t0x{:X}\t0x{:08X}\t{}\t0x{:08X}\t\
             0x{:04X}\t0x{:08X}\t0x{:02X}\t0x{:04X}\t0x{:08X}\t0x{:02X}",
            QUEST_SEQ_FLAG_BRANCH_REPORT_KIND,
            branch.sequence,
            hex(&branch.sequence_sha256),
            QUEST_SEQ_ACTIVE_RUNTIME_BASE,
            branch.runtime_base_evidence.label(),
            branch.record_offset,
            branch.record_runtime,
            hex(&branch.raw),
            branch.expected_value,
            branch.flag_base,
            branch.flag_offset,
            hex(&branch.unconsumed_parameters),
            branch.probe.word_index,
            branch.probe.runtime_address,
            branch.probe.bit_index,
            branch.probe.mask,
            branch.branch_target_offset,
            branch.branch_target_runtime,
            branch.branch_target_opcode,
            branch.fallthrough_offset,
            branch.fallthrough_runtime,
            branch.fallthrough_opcode,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_seq_flag_branches_tests.rs"]
mod tests;
