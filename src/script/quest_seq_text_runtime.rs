//! OP21 정적 추출 stream과 표본 runtime 증거의 연결 원장.
//!
//! 화면에서 원문 일부만 확인한 표본과 OP21 handler에서 record pointer까지
//! 직접 잡은 표본을 구분한다. runtime 주소를 파일 offset으로 바꾸는 계산은
//! `coords::RuntimeAssetMap`만 사용하며, 정적 stream digest가 달라지면
//! source gate가 실패한다.
//!
//! 이 원장은 확인한 세 표본만 소유한다. 나머지 602개 stream의 실행 도달이나
//! section 소유권을 추정하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::RuntimeAssetMap;

use super::{
    QuestSeqTextExtractionAnalysis, TOWN00_SEQ_LEN, TOWN00_SEQ_RUNTIME_BASE, TOWN00_SEQ_SHA256,
};

pub const QUEST_SEQ_TEXT_RUNTIME_EVIDENCE_SCHEMA_VERSION: &str =
    "quest-seq-text-runtime-evidence-v1";
pub const QUEST_OP21_HANDLER_RUNTIME: u32 = 0x8002_2F0C;
pub const BOX_QUEST_SEQ_RUNTIME_TEXT_EVIDENCE: usize = 3;
pub const BOX_QUEST_SEQ_RUNTIME_OP21_HITS: usize = 2;
pub const BOX_QUEST_SEQ_RUNTIME_VISIBLE_ONLY: usize = 1;
pub const BOX_QUEST_SEQ_TEXT_RUNTIME_EVIDENCE_REPORT_SHA256: &str =
    "89a1ac69668d6dd2c91a6be307272331e77718f37e1a6df77dfe374e498e6ebd";

/// runtime에서 직접 확인한 강도.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqTextRuntimeEvidenceLevel {
    /// 화면 원문과 보호 source span은 연결했지만 OP21 record hit는 잡지 않았다.
    VisibleSource,
    /// OP21 handler entry의 object와 stream record pointer를 직접 잡았다.
    Op21Hit,
}

impl QuestSeqTextRuntimeEvidenceLevel {
    pub const fn label(self) -> &'static str {
        match self {
            Self::VisibleSource => "runtime-visible-source",
            Self::Op21Hit => "runtime-op21-hit",
        }
    }
}

/// 정적 OP21 stream 하나에 귀속한 runtime 표본.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqTextRuntimeEvidence {
    pub sequence: &'static str,
    pub record_offset: usize,
    pub expected_stream_sha256: &'static str,
    pub level: QuestSeqTextRuntimeEvidenceLevel,
    pub runtime_base: u32,
    pub observed_record_runtime: Option<u32>,
    pub op21_handler_runtime: Option<u32>,
    pub object_runtime: Option<u32>,
    pub object_stream_field_runtime: Option<u32>,
    pub screenshot_sha256: &'static str,
    pub state_sha256: &'static str,
    pub tracking_run_id: Option<&'static str>,
    /// 화면으로 확인한 범위만 적는다. 전체 stream 표시를 주장하지 않는다.
    pub visible_scope: &'static str,
}

impl QuestSeqTextRuntimeEvidence {
    pub fn stream_id(self) -> String {
        format!(
            "QUEST-SEQ-TEXT-STREAM:{}:0x{:04X}",
            self.sequence, self.record_offset
        )
    }
}

pub const QUEST_SEQ_TEXT_RUNTIME_EVIDENCE: [QuestSeqTextRuntimeEvidence; 3] = [
    QuestSeqTextRuntimeEvidence {
        sequence: "TOWN00.SEQ",
        record_offset: 0x1B50,
        expected_stream_sha256: "effeb213e7a285be63c8a96d7d35f3dff4e4b0e88e5d1bdacdc68a3a449c0acf",
        level: QuestSeqTextRuntimeEvidenceLevel::VisibleSource,
        runtime_base: TOWN00_SEQ_RUNTIME_BASE,
        observed_record_runtime: None,
        op21_handler_runtime: None,
        object_runtime: None,
        object_stream_field_runtime: None,
        screenshot_sha256: "b1eb1c426c4f6fb1d1e7891c9269f79cf7819fa225052bc6553fbea0ec455549",
        state_sha256: "616c8107d294afb18e6721f3d7f6961701bdeefcdd8fb15d5c17f8130d72bc5c",
        tracking_run_id: None,
        visible_scope: "0x1B68+8,0x1B78+10,0x1B8A+18",
    },
    QuestSeqTextRuntimeEvidence {
        sequence: "TOWN00.SEQ",
        record_offset: 0x21CC,
        expected_stream_sha256: "c090a166d1a8beeddc01858675aab5d40e00d3b0c5473617ffabef72ac87b0d0",
        level: QuestSeqTextRuntimeEvidenceLevel::Op21Hit,
        runtime_base: TOWN00_SEQ_RUNTIME_BASE,
        observed_record_runtime: Some(0x8019_BC3C),
        op21_handler_runtime: Some(QUEST_OP21_HANDLER_RUNTIME),
        object_runtime: Some(0x800C_4578),
        object_stream_field_runtime: Some(0x800C_4594),
        screenshot_sha256: "7e5ac735fccd59ad0484bcfb20eb9c42a60746281c1bbbaa263df55d0e9091d2",
        state_sha256: "64e24b2ecb0e439e94e37978994c64fe6915f59ec4e15aebc8db21c226e2d7d8",
        tracking_run_id: Some("01KYMDRZR93E8ZKRSFVHQ5E2TG"),
        visible_scope: "0x21E4+8,0x21F4+10,0x2208+26",
    },
    QuestSeqTextRuntimeEvidence {
        sequence: "TOWN00.SEQ",
        record_offset: 0x22DC,
        expected_stream_sha256: "523f08d0b36f8200cae0aba808be2d0922ed2a1acd3aa52011bb1d70315c8fca",
        level: QuestSeqTextRuntimeEvidenceLevel::Op21Hit,
        runtime_base: TOWN00_SEQ_RUNTIME_BASE,
        observed_record_runtime: Some(0x8019_BD4C),
        op21_handler_runtime: Some(QUEST_OP21_HANDLER_RUNTIME),
        object_runtime: Some(0x800C_51B8),
        object_stream_field_runtime: Some(0x800C_51D4),
        screenshot_sha256: "f1b650c6699a98673e3dd83a34cb40608bb5960e4acfb77971b116aed2fad650",
        state_sha256: "934845140842ccbc597764034545e4d8d3758f8b3638ae2fcdfdfb1e8766cd45",
        tracking_run_id: Some("01KYMW9EJ0KE5F8HH65ZEKB65A"),
        visible_scope: "0x22E8..0x2327",
    },
];

/// 정적 source와 연결된 runtime 표본 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextRuntimeEvidenceAnalysis {
    pub entries: Vec<QuestSeqTextRuntimeEvidence>,
    pub op21_hits: usize,
    pub visible_only: usize,
}

/// runtime 증거의 좌표·digest·정적 소유 stream 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextRuntimeEvidenceError(String);

impl fmt::Display for QuestSeqTextRuntimeEvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSeqTextRuntimeEvidenceError {}

fn error(message: impl Into<String>) -> QuestSeqTextRuntimeEvidenceError {
    QuestSeqTextRuntimeEvidenceError(message.into())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_runtime_coordinates(
    evidence: QuestSeqTextRuntimeEvidence,
) -> Result<(), QuestSeqTextRuntimeEvidenceError> {
    if evidence.sequence != "TOWN00.SEQ"
        || evidence.runtime_base != TOWN00_SEQ_RUNTIME_BASE
        || evidence.visible_scope.is_empty()
        || !is_sha256(evidence.expected_stream_sha256)
        || !is_sha256(evidence.screenshot_sha256)
        || !is_sha256(evidence.state_sha256)
    {
        return Err(error(format!(
            "{}: runtime 증거 identity·digest 형식 불일치",
            evidence.stream_id()
        )));
    }
    let map = RuntimeAssetMap::new(
        evidence.runtime_base,
        u32::try_from(TOWN00_SEQ_LEN).map_err(|_| error("TOWN00 길이 변환 실패"))?,
    )
    .ok_or_else(|| error("TOWN00 runtime 사상 생성 실패"))?;

    match evidence.level {
        QuestSeqTextRuntimeEvidenceLevel::VisibleSource => {
            if evidence.observed_record_runtime.is_some()
                || evidence.op21_handler_runtime.is_some()
                || evidence.object_runtime.is_some()
                || evidence.object_stream_field_runtime.is_some()
            {
                return Err(error(format!(
                    "{}: visible-only 표본에 OP21 hit 필드가 있음",
                    evidence.stream_id()
                )));
            }
        }
        QuestSeqTextRuntimeEvidenceLevel::Op21Hit => {
            let expected_runtime = map
                .runtime_addr(evidence.record_offset)
                .ok_or_else(|| error(format!("{}: record 범위 이탈", evidence.stream_id())))?;
            let object = evidence
                .object_runtime
                .ok_or_else(|| error(format!("{}: object 주소 누락", evidence.stream_id())))?;
            if evidence.observed_record_runtime != Some(expected_runtime)
                || evidence.op21_handler_runtime != Some(QUEST_OP21_HANDLER_RUNTIME)
                || evidence.object_stream_field_runtime != object.checked_add(0x1C)
            {
                return Err(error(format!(
                    "{}: OP21 hit 좌표 불일치",
                    evidence.stream_id()
                )));
            }
        }
    }
    Ok(())
}

/// 지원 원본의 정적 OP21 추출 결과와 runtime 표본 원장을 교차한다.
pub fn audit_quest_seq_text_runtime_evidence(
    extraction: &QuestSeqTextExtractionAnalysis,
) -> Result<QuestSeqTextRuntimeEvidenceAnalysis, QuestSeqTextRuntimeEvidenceError> {
    let mut ids = BTreeSet::new();
    let mut entries = Vec::new();
    for evidence in QUEST_SEQ_TEXT_RUNTIME_EVIDENCE {
        validate_runtime_coordinates(evidence)?;
        if !ids.insert(evidence.stream_id()) {
            return Err(error(format!(
                "{}: runtime 증거 중복",
                evidence.stream_id()
            )));
        }
        let stream = extraction
            .streams
            .iter()
            .find(|stream| {
                stream.sequence == evidence.sequence
                    && stream.record_offset == evidence.record_offset
            })
            .ok_or_else(|| {
                error(format!(
                    "{}: 대응하는 정적 OP21 stream이 없음",
                    evidence.stream_id()
                ))
            })?;
        if hex(&stream.sequence_sha256) != TOWN00_SEQ_SHA256
            || hex(&stream.expected_raw_sha256) != evidence.expected_stream_sha256
        {
            return Err(error(format!(
                "{}: 정적 source digest 불일치",
                evidence.stream_id()
            )));
        }
        entries.push(evidence);
    }

    let op21_hits = entries
        .iter()
        .filter(|entry| entry.level == QuestSeqTextRuntimeEvidenceLevel::Op21Hit)
        .count();
    let visible_only = entries.len() - op21_hits;
    Ok(QuestSeqTextRuntimeEvidenceAnalysis {
        entries,
        op21_hits,
        visible_only,
    })
}

/// 고정 지원 원본의 runtime 표본 분모와 report digest를 검증한다.
pub fn verify_box_quest_seq_text_runtime_evidence(
    extraction: &QuestSeqTextExtractionAnalysis,
) -> Result<QuestSeqTextRuntimeEvidenceAnalysis, QuestSeqTextRuntimeEvidenceError> {
    let analysis = audit_quest_seq_text_runtime_evidence(extraction)?;
    if analysis.entries.len() != BOX_QUEST_SEQ_RUNTIME_TEXT_EVIDENCE
        || analysis.op21_hits != BOX_QUEST_SEQ_RUNTIME_OP21_HITS
        || analysis.visible_only != BOX_QUEST_SEQ_RUNTIME_VISIBLE_ONLY
    {
        return Err(error(format!(
            "QUEST runtime text 표본 분모 불일치: entries={}, op21={}, visible-only={}",
            analysis.entries.len(),
            analysis.op21_hits,
            analysis.visible_only
        )));
    }
    let report = render_quest_seq_text_runtime_evidence_tsv(&analysis);
    let actual_digest = format!("{:x}", Sha256::digest(report.as_bytes()));
    if actual_digest != BOX_QUEST_SEQ_TEXT_RUNTIME_EVIDENCE_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST runtime text evidence TSV SHA-256 불일치: \
             expected={BOX_QUEST_SEQ_TEXT_RUNTIME_EVIDENCE_REPORT_SHA256}, actual={actual_digest}"
        )));
    }
    Ok(analysis)
}

fn optional_runtime(value: Option<u32>) -> String {
    value
        .map(|address| format!("0x{address:08X}"))
        .unwrap_or_default()
}

/// 정적 stream ID와 runtime 증거 수준을 함께 보존하는 결정적 TSV.
pub fn render_quest_seq_text_runtime_evidence_tsv(
    analysis: &QuestSeqTextRuntimeEvidenceAnalysis,
) -> String {
    let mut output = String::from(
        "schema_version\tstream_id\tsequence_name\trecord_offset\t\
         expected_stream_sha256\tevidence_level\truntime_base\t\
         observed_record_runtime\top21_handler_runtime\tobject_runtime\t\
         object_stream_field_runtime\tscreenshot_sha256\tstate_sha256\t\
         tracking_run_id\tvisible_scope\n",
    );
    for entry in &analysis.entries {
        writeln!(
            output,
            "{}\t{}\t{}\t0x{:04X}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            QUEST_SEQ_TEXT_RUNTIME_EVIDENCE_SCHEMA_VERSION,
            entry.stream_id(),
            entry.sequence,
            entry.record_offset,
            entry.expected_stream_sha256,
            entry.level.label(),
            entry.runtime_base,
            optional_runtime(entry.observed_record_runtime),
            optional_runtime(entry.op21_handler_runtime),
            optional_runtime(entry.object_runtime),
            optional_runtime(entry.object_stream_field_runtime),
            entry.screenshot_sha256,
            entry.state_sha256,
            entry.tracking_run_id.unwrap_or_default(),
            entry.visible_scope,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_seq_text_runtime_tests.rs"]
mod tests;
