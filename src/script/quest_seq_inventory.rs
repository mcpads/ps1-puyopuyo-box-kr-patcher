//! `RPG_SEQ.VDD` 전체의 파일 내부주소 후보 분모.
//!
//! `TOWN00.SEQ`에서 runtime으로 확정한 active base를 다른 SEQ에도
//! 적용했을 때 자기 파일 범위를 가리키는 정렬 u32를 전수 열거한다. 다른
//! SEQ의 적재 base와 실제 소비는 아직 확정하지 않았으므로 후보로만
//! 분류한다. `QUEST.EXE`가 포인터로 소비하는 8/12/16/20바이트 record 형태와
//! 일치하는 field를 별도로 세지만, runtime 도달을 뜻하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

#[cfg(test)]
use super::quest_seq::TOWN00_SEQ_RUNTIME_BASE;
use super::quest_seq::{
    QUEST_SEQ_ACTIVE_RUNTIME_BASE, TOWN00_CONFIRMED_POINTER_FIELDS, TOWN00_SEQ_LEN,
    TOWN00_SEQ_SHA256,
};

/// record 안의 포인터가 소비되는 방식.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqPointerBehavior {
    /// 새 실행 객체의 stream pointer로 저장한다.
    SpawnStream,
    /// 기존 객체의 scheduler stream slot에 저장한다.
    ScheduledStream,
    /// 포인터를 u32 table base로 즉시 역참조한다.
    DereferenceTable,
    /// 후속 renderer가 색 데이터 주소로 역참조한다.
    DereferenceRenderData,
    /// child callback이 객체 데이터 주소로 역참조한다.
    DereferenceObjectData,
    /// child callback이 byte table 주소로 역참조한다.
    DereferenceByteTable,
    /// 비동기 loader callback이 복사 원본 주소로 역참조한다.
    CopySource,
    /// 전역에 보관했다가 다른 opcode가 stream pointer로 복원한다.
    SavedStream,
    /// 현재 stream pointer로 바꾼다.
    Branch,
    /// return stack을 보존한 뒤 현재 stream pointer로 바꾼다.
    StackBranch,
}

impl QuestSeqPointerBehavior {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SpawnStream => "spawn_stream",
            Self::ScheduledStream => "scheduled_stream",
            Self::DereferenceTable => "dereference_table",
            Self::DereferenceRenderData => "dereference_render_data",
            Self::DereferenceObjectData => "dereference_object_data",
            Self::DereferenceByteTable => "dereference_byte_table",
            Self::CopySource => "copy_source",
            Self::SavedStream => "saved_stream",
            Self::Branch => "branch",
            Self::StackBranch => "stack_branch",
        }
    }
}

/// 일부 opcode가 포인터를 읽는 source-side byte 조건.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqRecordByteCondition {
    Equal { offset: usize, value: u8 },
    NonZero { offset: usize },
    LessThan { offset: usize, exclusive_max: u8 },
}

impl QuestSeqRecordByteCondition {
    fn matches(self, raw: &[u8], record_start: usize) -> bool {
        match self {
            Self::Equal { offset, value } => {
                record_start
                    .checked_add(offset)
                    .and_then(|index| raw.get(index))
                    == Some(&value)
            }
            Self::NonZero { offset } => record_start
                .checked_add(offset)
                .and_then(|index| raw.get(index))
                .is_some_and(|byte| *byte != 0),
            Self::LessThan {
                offset,
                exclusive_max,
            } => record_start
                .checked_add(offset)
                .and_then(|index| raw.get(index))
                .is_some_and(|byte| *byte < exclusive_max),
        }
    }
}

/// `QUEST.EXE` source에서 검증할 포인터 record 사양.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqPointerRecordSpec {
    pub opcode: u8,
    pub handler_runtime: u32,
    pub pointer_consumer_runtime: u32,
    pub pointer_offset: usize,
    pub record_stride: usize,
    /// 일부 opcode가 포인터를 읽는 source-side 조건.
    pub required_byte: Option<QuestSeqRecordByteCondition>,
    pub behavior: QuestSeqPointerBehavior,
}

pub const QUEST_SEQ_POINTER_RECORD_SPECS: [QuestSeqPointerRecordSpec; 19] = [
    QuestSeqPointerRecordSpec {
        opcode: 0x80,
        handler_runtime: 0x8007_80E4,
        pointer_consumer_runtime: 0x8007_8174,
        pointer_offset: 4,
        record_stride: 8,
        required_byte: Some(QuestSeqRecordByteCondition::Equal {
            offset: 1,
            value: 0,
        }),
        behavior: QuestSeqPointerBehavior::SpawnStream,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0x82,
        handler_runtime: 0x8007_2844,
        pointer_consumer_runtime: 0x8007_3358,
        pointer_offset: 8,
        record_stride: 12,
        required_byte: Some(QuestSeqRecordByteCondition::LessThan {
            offset: 1,
            exclusive_max: 27,
        }),
        behavior: QuestSeqPointerBehavior::SpawnStream,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0x83,
        handler_runtime: 0x8007_33A4,
        pointer_consumer_runtime: 0x8007_359C,
        pointer_offset: 8,
        record_stride: 12,
        required_byte: Some(QuestSeqRecordByteCondition::Equal {
            offset: 1,
            value: 5,
        }),
        behavior: QuestSeqPointerBehavior::ScheduledStream,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0x85,
        handler_runtime: 0x8007_8280,
        pointer_consumer_runtime: 0x8007_832C,
        pointer_offset: 12,
        record_stride: 20,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::DereferenceRenderData,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0x85,
        handler_runtime: 0x8007_8280,
        pointer_consumer_runtime: 0x8007_8340,
        pointer_offset: 16,
        record_stride: 20,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::DereferenceRenderData,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0x88,
        handler_runtime: 0x8007_83C0,
        pointer_consumer_runtime: 0x8007_83E8,
        pointer_offset: 4,
        record_stride: 8,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::DereferenceTable,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0x8F,
        handler_runtime: 0x8007_8540,
        pointer_consumer_runtime: 0x8007_8564,
        pointer_offset: 4,
        record_stride: 8,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::DereferenceTable,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0x89,
        handler_runtime: 0x8007_3960,
        pointer_consumer_runtime: 0x8007_397C,
        pointer_offset: 4,
        record_stride: 8,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::Branch,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0x93,
        handler_runtime: 0x8007_8650,
        pointer_consumer_runtime: 0x8007_8704,
        pointer_offset: 4,
        record_stride: 16,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::DereferenceObjectData,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0x93,
        handler_runtime: 0x8007_8650,
        pointer_consumer_runtime: 0x8007_8718,
        pointer_offset: 12,
        record_stride: 16,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::DereferenceRenderData,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0x95,
        handler_runtime: 0x8007_87CC,
        pointer_consumer_runtime: 0x8007_87D4,
        pointer_offset: 4,
        record_stride: 8,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::SavedStream,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0x9D,
        handler_runtime: 0x8007_88AC,
        pointer_consumer_runtime: 0x8007_88E8,
        pointer_offset: 4,
        record_stride: 8,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::SpawnStream,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0x9F,
        handler_runtime: 0x8007_89D0,
        pointer_consumer_runtime: 0x8007_8A00,
        pointer_offset: 4,
        record_stride: 8,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::DereferenceByteTable,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0xA1,
        handler_runtime: 0x8007_8A40,
        pointer_consumer_runtime: 0x8007_8A80,
        pointer_offset: 8,
        record_stride: 12,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::Branch,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0xA2,
        handler_runtime: 0x8007_8AA0,
        pointer_consumer_runtime: 0x8007_8B04,
        pointer_offset: 8,
        record_stride: 12,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::StackBranch,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0xA8,
        handler_runtime: 0x8007_8CDC,
        pointer_consumer_runtime: 0x8007_8CF8,
        pointer_offset: 4,
        record_stride: 8,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::Branch,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0xA9,
        handler_runtime: 0x8007_8D0C,
        pointer_consumer_runtime: 0x8007_8D4C,
        pointer_offset: 4,
        record_stride: 8,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::StackBranch,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0xC1,
        handler_runtime: 0x8007_9658,
        pointer_consumer_runtime: 0x8007_967C,
        pointer_offset: 4,
        record_stride: 8,
        required_byte: Some(QuestSeqRecordByteCondition::NonZero { offset: 1 }),
        behavior: QuestSeqPointerBehavior::SavedStream,
    },
    QuestSeqPointerRecordSpec {
        opcode: 0xE3,
        handler_runtime: 0x8006_FF34,
        pointer_consumer_runtime: 0x8006_FF70,
        pointer_offset: 8,
        record_stride: 12,
        required_byte: None,
        behavior: QuestSeqPointerBehavior::CopySource,
    },
];

/// 지원 원본에서 내부주소 후보가 존재하는 SEQ의 분모.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxQuestSeqAddressExpectation {
    pub sequence: &'static str,
    pub bytes: usize,
    pub fields: usize,
    pub targets: usize,
    pub pointer_records: [usize; QUEST_SEQ_POINTER_RECORD_SPECS.len()],
    pub relational_pointer_fields: usize,
    pub relational_pointer_edges: usize,
}

pub const BOX_QUEST_SEQ_ADDRESS_EXPECTATIONS: [BoxQuestSeqAddressExpectation; 13] = [
    BoxQuestSeqAddressExpectation {
        sequence: "ALLMAP.SEQ",
        bytes: 2_548,
        fields: 43,
        targets: 41,
        pointer_records: [0, 0, 0, 1, 1, 1, 0, 16, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0],
        relational_pointer_fields: 0,
        relational_pointer_edges: 0,
    },
    BoxQuestSeqAddressExpectation {
        sequence: "SUKE.SEQ",
        bytes: 5_628,
        fields: 56,
        targets: 38,
        pointer_records: [0, 1, 0, 4, 2, 3, 3, 9, 3, 3, 1, 1, 0, 4, 0, 0, 0, 0, 0],
        relational_pointer_fields: 4,
        relational_pointer_edges: 11,
    },
    BoxQuestSeqAddressExpectation {
        sequence: "TOWN00.SEQ",
        bytes: 22_372,
        fields: 167,
        targets: 122,
        pointer_records: [0, 15, 0, 4, 2, 5, 1, 7, 0, 0, 4, 2, 1, 21, 0, 5, 0, 0, 0],
        relational_pointer_fields: 6,
        relational_pointer_edges: 10,
    },
    BoxQuestSeqAddressExpectation {
        sequence: "TOWN10.SEQ",
        bytes: 25_680,
        fields: 219,
        targets: 143,
        pointer_records: [0, 17, 1, 5, 2, 5, 1, 10, 0, 0, 3, 2, 1, 25, 0, 6, 0, 0, 0],
        relational_pointer_fields: 7,
        relational_pointer_edges: 13,
    },
    BoxQuestSeqAddressExpectation {
        sequence: "TOWN20.SEQ",
        bytes: 15_940,
        fields: 134,
        targets: 101,
        pointer_records: [0, 11, 1, 4, 2, 4, 0, 7, 0, 0, 4, 2, 1, 17, 0, 6, 0, 0, 0],
        relational_pointer_fields: 6,
        relational_pointer_edges: 10,
    },
    BoxQuestSeqAddressExpectation {
        sequence: "TOWN30.SEQ",
        bytes: 7_680,
        fields: 75,
        targets: 57,
        pointer_records: [0, 3, 0, 3, 3, 3, 0, 3, 0, 0, 3, 3, 3, 3, 0, 4, 0, 0, 0],
        relational_pointer_fields: 0,
        relational_pointer_edges: 0,
    },
    BoxQuestSeqAddressExpectation {
        sequence: "HASI.SEQ",
        bytes: 5_508,
        fields: 21,
        targets: 17,
        pointer_records: [0, 1, 0, 1, 1, 1, 0, 2, 0, 0, 0, 1, 0, 3, 0, 1, 0, 0, 0],
        relational_pointer_fields: 0,
        relational_pointer_edges: 0,
    },
    BoxQuestSeqAddressExpectation {
        sequence: "MORI.SEQ",
        bytes: 9_612,
        fields: 68,
        targets: 48,
        pointer_records: [0, 2, 1, 3, 1, 3, 3, 5, 0, 0, 1, 1, 0, 12, 0, 0, 0, 0, 0],
        relational_pointer_fields: 4,
        relational_pointer_edges: 5,
    },
    BoxQuestSeqAddressExpectation {
        sequence: "KAWA.SEQ",
        bytes: 24_304,
        fields: 183,
        targets: 123,
        pointer_records: [0, 6, 0, 10, 3, 11, 6, 30, 4, 4, 2, 3, 1, 21, 0, 2, 0, 0, 0],
        relational_pointer_fields: 13,
        relational_pointer_edges: 33,
    },
    BoxQuestSeqAddressExpectation {
        sequence: "TOU.SEQ",
        bytes: 17_976,
        fields: 98,
        targets: 82,
        pointer_records: [0, 6, 0, 4, 2, 2, 0, 4, 1, 1, 0, 2, 0, 19, 0, 8, 0, 0, 0],
        relational_pointer_fields: 5,
        relational_pointer_edges: 5,
    },
    BoxQuestSeqAddressExpectation {
        sequence: "MITI.SEQ",
        bytes: 6_524,
        fields: 78,
        targets: 54,
        pointer_records: [0, 2, 0, 5, 2, 5, 3, 11, 0, 0, 2, 2, 0, 4, 0, 5, 0, 0, 0],
        relational_pointer_fields: 7,
        relational_pointer_edges: 10,
    },
    BoxQuestSeqAddressExpectation {
        sequence: "ISEKI.SEQ",
        bytes: 11_204,
        fields: 131,
        targets: 90,
        pointer_records: [0, 3, 0, 7, 2, 7, 6, 18, 0, 0, 1, 2, 0, 20, 0, 4, 0, 0, 0],
        relational_pointer_fields: 9,
        relational_pointer_edges: 19,
    },
    BoxQuestSeqAddressExpectation {
        sequence: "SHEZ.SEQ",
        bytes: 10_596,
        fields: 106,
        targets: 69,
        pointer_records: [0, 23, 9, 1, 1, 1, 1, 1, 0, 0, 0, 1, 0, 13, 1, 13, 0, 0, 0],
        relational_pointer_fields: 0,
        relational_pointer_edges: 0,
    },
];

pub const BOX_QUEST_SEQ_SEQUENCES: usize = 113;
pub const BOX_QUEST_SEQ_BYTES: usize = 243_546;
pub const BOX_QUEST_SEQ_ADDRESS_FILES: usize = 13;
pub const BOX_QUEST_SEQ_ADDRESS_FIELDS: usize = 1_379;
pub const BOX_QUEST_SEQ_ADDRESS_TARGETS: usize = 985;
pub const BOX_QUEST_SEQ_POINTER_RECORD_COUNTS: [usize; QUEST_SEQ_POINTER_RECORD_SPECS.len()] = [
    0, 90, 12, 52, 24, 51, 24, 123, 8, 8, 21, 23, 7, 163, 1, 54, 0, 0, 0,
];
pub const BOX_QUEST_SEQ_POINTER_RECORDS: usize = 661;
pub const BOX_QUEST_SEQ_STATIC_POINTER_FIELDS: usize = 655;
pub const BOX_QUEST_SEQ_RANGE_CANDIDATE_FIELDS: usize = 717;
pub const BOX_QUEST_SEQ_RUNTIME_READ_FIELDS: usize = 7;
pub const BOX_QUEST_SEQ_RELATIONAL_POINTER_FIELDS: usize = 61;
pub const BOX_QUEST_SEQ_RELATIONAL_POINTER_EDGES: usize = 116;

/// 후보 좌표·분류 전체를 고정한 지원 원본 digest.
pub const BOX_QUEST_SEQ_ADDRESS_ANALYSIS_SHA256: [u8; 32] = [
    0xA0, 0x70, 0x26, 0xDF, 0x16, 0x54, 0x67, 0xCB, 0x53, 0x26, 0x7C, 0xC7, 0x90, 0x15, 0xD0, 0xF7,
    0xE6, 0xDE, 0x29, 0xC5, 0xA4, 0x9D, 0x13, 0x8C, 0xD0, 0x8A, 0x89, 0x76, 0x67, 0x70, 0x79, 0xE0,
];

/// 내부주소 후보의 증거 단계.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqAddressEvidence {
    /// `TOWN00` runtime breakpoint에서 해당 field의 정확한 read를 확인했다.
    RuntimeRead,
    /// source에서 포인터 소비를 검증한 record 형태와 일치한다.
    StaticPointerShape,
    /// 저장주소 범위 안의 정렬 u32라는 사실만 확인했다.
    RangeCandidate,
}

impl QuestSeqAddressEvidence {
    pub const fn label(self) -> &'static str {
        match self {
            Self::RuntimeRead => "runtime_read",
            Self::StaticPointerShape => "static_pointer_shape",
            Self::RangeCandidate => "range_candidate",
        }
    }
}

/// 파일 내부주소를 runtime 주소로 환산할 때 사용한 active base의 근거.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqRuntimeBaseEvidence {
    /// 지원 원본 `TOWN00.SEQ`의 적재와 소비에서 직접 확인했다.
    RuntimeObserved,
    /// 지원 원본의 공용 descriptor 선택·staging·active copy 경로로 확인했다.
    StaticCommonLoader,
}

impl QuestSeqRuntimeBaseEvidence {
    pub const fn label(self) -> &'static str {
        match self {
            Self::RuntimeObserved => "runtime-observed",
            Self::StaticCommonLoader => "static-common-loader",
        }
    }

    pub const fn runtime_observed(self) -> bool {
        matches!(self, Self::RuntimeObserved)
    }

    pub const fn base_binding_proven(self) -> bool {
        matches!(self, Self::RuntimeObserved | Self::StaticCommonLoader)
    }
}

/// 한 SEQ의 내부주소 후보 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqAddressCandidate {
    pub sequence: String,
    pub runtime_base_evidence: QuestSeqRuntimeBaseEvidence,
    pub field_offset: usize,
    pub field_runtime: u32,
    pub target_offset: usize,
    pub target_runtime: u32,
    pub target_references: usize,
    pub byte_minus_4: Option<u8>,
    pub byte_minus_8: Option<u8>,
    pub byte_minus_12: Option<u8>,
    pub byte_minus_16: Option<u8>,
    pub pointer_spec: Option<QuestSeqPointerRecordSpec>,
    pub pointer_relations: Vec<QuestSeqPointerRelation>,
    pub evidence: QuestSeqAddressEvidence,
    pub runtime_consumer: Option<u32>,
}

/// OP89 outer branch가 가리킨 inner 20바이트 record의 포인터 field 관계.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct QuestSeqPointerRelation {
    pub opcode: u8,
    pub outer_record_start: usize,
    pub outer_pointer_field: usize,
    pub inner_record_start: usize,
    pub inner_pointer_offset: usize,
    pub pointer_consumer_runtime: u32,
}

impl QuestSeqAddressCandidate {
    fn direction(&self) -> &'static str {
        match self.target_offset.cmp(&self.field_offset) {
            std::cmp::Ordering::Less => "backward",
            std::cmp::Ordering::Equal => "self",
            std::cmp::Ordering::Greater => "forward",
        }
    }
}

/// SEQ 하나의 후보 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqAddressSummary {
    pub sequence: String,
    pub runtime_base_evidence: QuestSeqRuntimeBaseEvidence,
    pub bytes: usize,
    pub fields: usize,
    pub targets: usize,
    pub pointer_records: [usize; QUEST_SEQ_POINTER_RECORD_SPECS.len()],
    pub relational_pointer_fields: usize,
    pub relational_pointer_edges: usize,
    pub runtime_read_fields: usize,
}

impl QuestSeqAddressSummary {
    pub fn pointer_records_for(&self, opcode: u8) -> Option<usize> {
        let mut found = false;
        let count = QUEST_SEQ_POINTER_RECORD_SPECS
            .iter()
            .enumerate()
            .filter(|(_, spec)| spec.opcode == opcode)
            .map(|(index, _)| {
                found = true;
                self.pointer_records[index]
            })
            .sum();
        found.then_some(count)
    }

    pub fn pointer_records_for_spec(&self, opcode: u8, pointer_offset: usize) -> Option<usize> {
        QUEST_SEQ_POINTER_RECORD_SPECS
            .iter()
            .position(|spec| spec.opcode == opcode && spec.pointer_offset == pointer_offset)
            .map(|index| self.pointer_records[index])
    }
}

/// `RPG_SEQ.VDD` 전체의 주소 후보 분석.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqAddressAnalysis {
    pub summaries: Vec<QuestSeqAddressSummary>,
    pub candidates: Vec<QuestSeqAddressCandidate>,
    pub analysis_sha256: [u8; 32],
}

/// 전체 SEQ 주소 후보 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestSeqAddressError {
    EmptyInput,
    EmptyName,
    DuplicateSequence(String),
    AddressOverflow(String),
    BoxSummaryMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    MissingExpectedSequence(&'static str),
    SequenceSummaryMismatch {
        sequence: &'static str,
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    AnalysisDigestMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

impl fmt::Display for QuestSeqAddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "QUEST SEQ 주소 후보 입력이 없음"),
            Self::EmptyName => write!(f, "QUEST SEQ 주소 후보 입력 이름이 비어 있음"),
            Self::DuplicateSequence(sequence) => {
                write!(f, "QUEST SEQ 주소 후보 입력 중복: {sequence}")
            }
            Self::AddressOverflow(sequence) => {
                write!(f, "{sequence}: QUEST SEQ 주소 범위 overflow")
            }
            Self::BoxSummaryMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "QUEST SEQ 주소 후보 {field} 불일치: expected={expected}, actual={actual}"
            ),
            Self::MissingExpectedSequence(sequence) => {
                write!(f, "QUEST SEQ 주소 후보 기대 입력 누락: {sequence}")
            }
            Self::SequenceSummaryMismatch {
                sequence,
                field,
                expected,
                actual,
            } => write!(
                f,
                "{sequence}: QUEST SEQ 주소 후보 {field} 불일치: \
                 expected={expected}, actual={actual}"
            ),
            Self::AnalysisDigestMismatch { expected, actual } => write!(
                f,
                "QUEST SEQ 주소 후보 분석 digest 불일치: expected={}, actual={}",
                hex_digest(expected),
                hex_digest(actual)
            ),
        }
    }
}

impl Error for QuestSeqAddressError {}

/// 임의 SEQ 집합에서 저장주소 범위 후보와 포인터 record 형태를 계산한다.
pub fn audit_quest_seq_addresses<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Result<QuestSeqAddressAnalysis, QuestSeqAddressError> {
    let mut input = BTreeMap::<String, &'a [u8]>::new();
    for (name, raw) in sequences {
        if name.is_empty() {
            return Err(QuestSeqAddressError::EmptyName);
        }
        if input.insert(name.to_string(), raw).is_some() {
            return Err(QuestSeqAddressError::DuplicateSequence(name.to_string()));
        }
    }
    if input.is_empty() {
        return Err(QuestSeqAddressError::EmptyInput);
    }

    let mut summaries = Vec::with_capacity(input.len());
    let mut candidates = Vec::new();
    for (sequence, raw) in input {
        let raw_len = u32::try_from(raw.len())
            .map_err(|_| QuestSeqAddressError::AddressOverflow(sequence.clone()))?;
        let runtime_end = QUEST_SEQ_ACTIVE_RUNTIME_BASE
            .checked_add(raw_len)
            .ok_or_else(|| QuestSeqAddressError::AddressOverflow(sequence.clone()))?;
        let supported_town00 = sequence == "TOWN00.SEQ"
            && raw.len() == TOWN00_SEQ_LEN
            && hex_digest(&Sha256::digest(raw).into()) == TOWN00_SEQ_SHA256;
        let runtime_base_evidence = if supported_town00 {
            QuestSeqRuntimeBaseEvidence::RuntimeObserved
        } else {
            QuestSeqRuntimeBaseEvidence::StaticCommonLoader
        };
        let fields = raw
            .chunks_exact(4)
            .enumerate()
            .filter_map(|(index, word)| {
                let target_runtime = u32::from_le_bytes(word.try_into().ok()?);
                (QUEST_SEQ_ACTIVE_RUNTIME_BASE..runtime_end)
                    .contains(&target_runtime)
                    .then_some((index * 4, target_runtime))
            })
            .collect::<Vec<_>>();
        let mut pointer_relations = op89_pointer_relations(raw, &fields);
        let target_references =
            fields
                .iter()
                .fold(BTreeMap::<u32, usize>::new(), |mut counts, (_, target)| {
                    *counts.entry(*target).or_default() += 1;
                    counts
                });
        let start = candidates.len();

        for (field_offset, target_runtime) in fields {
            let target_offset = usize::try_from(target_runtime - QUEST_SEQ_ACTIVE_RUNTIME_BASE)
                .map_err(|_| QuestSeqAddressError::AddressOverflow(sequence.clone()))?;
            let byte_minus_4 = field_offset.checked_sub(4).map(|offset| raw[offset]);
            let byte_minus_8 = field_offset.checked_sub(8).map(|offset| raw[offset]);
            let byte_minus_12 = field_offset.checked_sub(12).map(|offset| raw[offset]);
            let byte_minus_16 = field_offset.checked_sub(16).map(|offset| raw[offset]);
            let pointer_spec = pointer_record_spec(raw, field_offset);
            let field_pointer_relations =
                pointer_relations.remove(&field_offset).unwrap_or_default();
            let runtime_consumer = supported_town00
                .then(|| {
                    TOWN00_CONFIRMED_POINTER_FIELDS.iter().find(|confirmed| {
                        confirmed.field_offset == field_offset
                            && confirmed.target_offset == target_offset
                    })
                })
                .flatten()
                .map(|confirmed| confirmed.consumer_runtime);
            let evidence = if runtime_consumer.is_some() {
                QuestSeqAddressEvidence::RuntimeRead
            } else if pointer_spec.is_some() || !field_pointer_relations.is_empty() {
                QuestSeqAddressEvidence::StaticPointerShape
            } else {
                QuestSeqAddressEvidence::RangeCandidate
            };
            candidates.push(QuestSeqAddressCandidate {
                sequence: sequence.clone(),
                runtime_base_evidence,
                field_offset,
                field_runtime: QUEST_SEQ_ACTIVE_RUNTIME_BASE
                    .checked_add(
                        u32::try_from(field_offset)
                            .map_err(|_| QuestSeqAddressError::AddressOverflow(sequence.clone()))?,
                    )
                    .ok_or_else(|| QuestSeqAddressError::AddressOverflow(sequence.clone()))?,
                target_offset,
                target_runtime,
                target_references: target_references[&target_runtime],
                byte_minus_4,
                byte_minus_8,
                byte_minus_12,
                byte_minus_16,
                pointer_spec,
                pointer_relations: field_pointer_relations,
                evidence,
                runtime_consumer,
            });
        }

        let sequence_candidates = &candidates[start..];
        let mut pointer_records = [0; QUEST_SEQ_POINTER_RECORD_SPECS.len()];
        for candidate in sequence_candidates {
            if let Some(spec) = candidate.pointer_spec {
                let index = QUEST_SEQ_POINTER_RECORD_SPECS
                    .iter()
                    .position(|candidate_spec| *candidate_spec == spec)
                    .expect("선택한 포인터 사양은 정적 사양 표에 존재함");
                pointer_records[index] += 1;
            }
        }
        summaries.push(QuestSeqAddressSummary {
            sequence,
            runtime_base_evidence,
            bytes: raw.len(),
            fields: sequence_candidates.len(),
            targets: sequence_candidates
                .iter()
                .map(|candidate| candidate.target_runtime)
                .collect::<BTreeSet<_>>()
                .len(),
            pointer_records,
            relational_pointer_fields: sequence_candidates
                .iter()
                .filter(|candidate| !candidate.pointer_relations.is_empty())
                .count(),
            relational_pointer_edges: sequence_candidates
                .iter()
                .map(|candidate| candidate.pointer_relations.len())
                .sum(),
            runtime_read_fields: sequence_candidates
                .iter()
                .filter(|candidate| candidate.runtime_consumer.is_some())
                .count(),
        });
    }

    let report = render_rows(&candidates);
    let analysis_sha256 = Sha256::digest(report.as_bytes()).into();
    Ok(QuestSeqAddressAnalysis {
        summaries,
        candidates,
        analysis_sha256,
    })
}

fn op89_pointer_relations(
    raw: &[u8],
    fields: &[(usize, u32)],
) -> BTreeMap<usize, Vec<QuestSeqPointerRelation>> {
    let field_offsets = fields
        .iter()
        .map(|(field_offset, _)| *field_offset)
        .collect::<BTreeSet<_>>();
    let mut relations = BTreeMap::<usize, Vec<QuestSeqPointerRelation>>::new();
    for &(outer_pointer_field, target_runtime) in fields {
        let Some(spec) = pointer_record_spec(raw, outer_pointer_field) else {
            continue;
        };
        if spec.opcode != 0x89 || spec.pointer_offset != 4 {
            continue;
        }
        let inner_record_start =
            usize::try_from(target_runtime - QUEST_SEQ_ACTIVE_RUNTIME_BASE).expect("범위 안 u32");
        for (inner_pointer_offset, pointer_consumer_runtime) in
            [(12, 0x8007_3A34), (16, 0x8007_3A48)]
        {
            let Some(inner_pointer_field) = inner_record_start.checked_add(inner_pointer_offset)
            else {
                continue;
            };
            if !field_offsets.contains(&inner_pointer_field) {
                continue;
            }
            relations
                .entry(inner_pointer_field)
                .or_default()
                .push(QuestSeqPointerRelation {
                    opcode: 0x89,
                    outer_record_start: outer_pointer_field - spec.pointer_offset,
                    outer_pointer_field,
                    inner_record_start,
                    inner_pointer_offset,
                    pointer_consumer_runtime,
                });
        }
    }
    for sources in relations.values_mut() {
        sources.sort_unstable();
        sources.dedup();
    }
    relations
}

/// 후보 전 행을 결정적 TSV로 직렬화한다.
pub fn render_quest_seq_address_candidates_tsv(analysis: &QuestSeqAddressAnalysis) -> String {
    render_rows(&analysis.candidates)
}

fn render_rows(candidates: &[QuestSeqAddressCandidate]) -> String {
    let mut output = String::from(
        "sequence\truntime_base_evidence\tfield_offset\tfield_runtime\ttarget_offset\ttarget_runtime\t\
         target_references\tdirection\tbyte_minus_4\tbyte_minus_8\tbyte_minus_12\t\
         byte_minus_16\t\
         record_opcode\trecord_start\tpointer_offset\trecord_stride\tpointer_behavior\t\
         static_pointer_consumer\trelation_opcodes\trelation_outer_record_starts\t\
         relation_outer_pointer_fields\trelation_inner_record_starts\t\
         relation_inner_pointer_offsets\trelation_consumers\trelation_edges\t\
         evidence\truntime_consumer\n",
    );
    for candidate in candidates {
        let byte_minus_4 = candidate
            .byte_minus_4
            .map_or_else(String::new, |opcode| format!("0x{opcode:02X}"));
        let byte_minus_8 = candidate
            .byte_minus_8
            .map_or_else(String::new, |opcode| format!("0x{opcode:02X}"));
        let byte_minus_12 = candidate
            .byte_minus_12
            .map_or_else(String::new, |opcode| format!("0x{opcode:02X}"));
        let byte_minus_16 = candidate
            .byte_minus_16
            .map_or_else(String::new, |opcode| format!("0x{opcode:02X}"));
        let record_opcode = candidate
            .pointer_spec
            .map_or_else(String::new, |spec| format!("0x{:02X}", spec.opcode));
        let record_start = candidate.pointer_spec.map_or_else(String::new, |spec| {
            format!("0x{:04X}", candidate.field_offset - spec.pointer_offset)
        });
        let pointer_offset = candidate
            .pointer_spec
            .map_or_else(String::new, |spec| spec.pointer_offset.to_string());
        let record_stride = candidate
            .pointer_spec
            .map_or_else(String::new, |spec| spec.record_stride.to_string());
        let pointer_behavior = candidate
            .pointer_spec
            .map_or_else(String::new, |spec| spec.behavior.label().to_string());
        let static_consumer = candidate.pointer_spec.map_or_else(String::new, |spec| {
            format!("0x{:08X}", spec.pointer_consumer_runtime)
        });
        let relation_opcodes =
            join_relations(candidate, |relation| format!("0x{:02X}", relation.opcode));
        let relation_outer_record_starts = join_relations(candidate, |relation| {
            format!("0x{:04X}", relation.outer_record_start)
        });
        let relation_outer_pointer_fields = join_relations(candidate, |relation| {
            format!("0x{:04X}", relation.outer_pointer_field)
        });
        let relation_inner_record_starts = join_relations(candidate, |relation| {
            format!("0x{:04X}", relation.inner_record_start)
        });
        let relation_inner_pointer_offsets = join_relations(candidate, |relation| {
            relation.inner_pointer_offset.to_string()
        });
        let relation_consumers = join_relations(candidate, |relation| {
            format!("0x{:08X}", relation.pointer_consumer_runtime)
        });
        let relation_edges = if candidate.pointer_relations.is_empty() {
            String::new()
        } else {
            candidate.pointer_relations.len().to_string()
        };
        let runtime_consumer = candidate
            .runtime_consumer
            .map_or_else(String::new, |address| format!("0x{address:08X}"));
        writeln!(
            output,
            "{}\t{}\t0x{:04X}\t0x{:08X}\t0x{:04X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            candidate.sequence,
            candidate.runtime_base_evidence.label(),
            candidate.field_offset,
            candidate.field_runtime,
            candidate.target_offset,
            candidate.target_runtime,
            candidate.target_references,
            candidate.direction(),
            byte_minus_4,
            byte_minus_8,
            byte_minus_12,
            byte_minus_16,
            record_opcode,
            record_start,
            pointer_offset,
            record_stride,
            pointer_behavior,
            static_consumer,
            relation_opcodes,
            relation_outer_record_starts,
            relation_outer_pointer_fields,
            relation_inner_record_starts,
            relation_inner_pointer_offsets,
            relation_consumers,
            relation_edges,
            candidate.evidence.label(),
            runtime_consumer,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

fn join_relations(
    candidate: &QuestSeqAddressCandidate,
    render: impl Fn(&QuestSeqPointerRelation) -> String,
) -> String {
    candidate
        .pointer_relations
        .iter()
        .map(render)
        .collect::<Vec<_>>()
        .join(",")
}

/// 지원 원본의 113개 SEQ 분모와 후보 ledger를 검증한다.
pub fn verify_box_quest_seq_addresses(
    analysis: &QuestSeqAddressAnalysis,
) -> Result<(), QuestSeqAddressError> {
    require_total("SEQ 수", BOX_QUEST_SEQ_SEQUENCES, analysis.summaries.len())?;
    require_total(
        "해제 바이트",
        BOX_QUEST_SEQ_BYTES,
        analysis.summaries.iter().map(|summary| summary.bytes).sum(),
    )?;
    require_total(
        "주소 후보가 있는 SEQ 수",
        BOX_QUEST_SEQ_ADDRESS_FILES,
        analysis
            .summaries
            .iter()
            .filter(|summary| summary.fields > 0)
            .count(),
    )?;
    require_total(
        "runtime-observed active base SEQ 수",
        1,
        analysis
            .summaries
            .iter()
            .filter(|summary| summary.runtime_base_evidence.runtime_observed())
            .count(),
    )?;
    require_total(
        "static common-loader active base SEQ 수",
        BOX_QUEST_SEQ_SEQUENCES - 1,
        analysis
            .summaries
            .iter()
            .filter(|summary| {
                summary.runtime_base_evidence == QuestSeqRuntimeBaseEvidence::StaticCommonLoader
            })
            .count(),
    )?;
    require_total(
        "active base 결합이 증명된 SEQ 수",
        BOX_QUEST_SEQ_SEQUENCES,
        analysis
            .summaries
            .iter()
            .filter(|summary| summary.runtime_base_evidence.base_binding_proven())
            .count(),
    )?;
    require_total(
        "후보 field",
        BOX_QUEST_SEQ_ADDRESS_FIELDS,
        analysis.candidates.len(),
    )?;
    require_total(
        "후보 target",
        BOX_QUEST_SEQ_ADDRESS_TARGETS,
        analysis
            .summaries
            .iter()
            .map(|summary| summary.targets)
            .sum(),
    )?;
    let actual_pointer_counts = analysis.summaries.iter().fold(
        [0; QUEST_SEQ_POINTER_RECORD_SPECS.len()],
        |mut sum, summary| {
            for (index, count) in summary.pointer_records.iter().enumerate() {
                sum[index] += count;
            }
            sum
        },
    );
    for (index, spec) in QUEST_SEQ_POINTER_RECORD_SPECS.iter().enumerate() {
        require_total(
            pointer_record_field(spec),
            BOX_QUEST_SEQ_POINTER_RECORD_COUNTS[index],
            actual_pointer_counts[index],
        )?;
    }
    require_total(
        "pointer record 형태",
        BOX_QUEST_SEQ_POINTER_RECORDS,
        actual_pointer_counts.iter().sum(),
    )?;
    require_total(
        "static pointer field",
        BOX_QUEST_SEQ_STATIC_POINTER_FIELDS,
        analysis
            .candidates
            .iter()
            .filter(|candidate| candidate.evidence == QuestSeqAddressEvidence::StaticPointerShape)
            .count(),
    )?;
    require_total(
        "range candidate field",
        BOX_QUEST_SEQ_RANGE_CANDIDATE_FIELDS,
        analysis
            .candidates
            .iter()
            .filter(|candidate| candidate.evidence == QuestSeqAddressEvidence::RangeCandidate)
            .count(),
    )?;
    require_total(
        "runtime read field",
        BOX_QUEST_SEQ_RUNTIME_READ_FIELDS,
        analysis
            .summaries
            .iter()
            .map(|summary| summary.runtime_read_fields)
            .sum(),
    )?;
    require_total(
        "OP89 inner 관계 field",
        BOX_QUEST_SEQ_RELATIONAL_POINTER_FIELDS,
        analysis
            .summaries
            .iter()
            .map(|summary| summary.relational_pointer_fields)
            .sum(),
    )?;
    require_total(
        "OP89 inner 관계 edge",
        BOX_QUEST_SEQ_RELATIONAL_POINTER_EDGES,
        analysis
            .summaries
            .iter()
            .map(|summary| summary.relational_pointer_edges)
            .sum(),
    )?;

    for expected in BOX_QUEST_SEQ_ADDRESS_EXPECTATIONS {
        let actual = analysis
            .summaries
            .iter()
            .find(|summary| summary.sequence == expected.sequence)
            .ok_or(QuestSeqAddressError::MissingExpectedSequence(
                expected.sequence,
            ))?;
        require_sequence(expected.sequence, "bytes", expected.bytes, actual.bytes)?;
        require_sequence(expected.sequence, "fields", expected.fields, actual.fields)?;
        require_sequence(
            expected.sequence,
            "targets",
            expected.targets,
            actual.targets,
        )?;
        for (index, spec) in QUEST_SEQ_POINTER_RECORD_SPECS.iter().enumerate() {
            require_sequence(
                expected.sequence,
                pointer_record_field(spec),
                expected.pointer_records[index],
                actual.pointer_records[index],
            )?;
        }
        require_sequence(
            expected.sequence,
            "OP89 inner 관계 field",
            expected.relational_pointer_fields,
            actual.relational_pointer_fields,
        )?;
        require_sequence(
            expected.sequence,
            "OP89 inner 관계 edge",
            expected.relational_pointer_edges,
            actual.relational_pointer_edges,
        )?;
    }

    if analysis.analysis_sha256 != BOX_QUEST_SEQ_ADDRESS_ANALYSIS_SHA256 {
        return Err(QuestSeqAddressError::AnalysisDigestMismatch {
            expected: BOX_QUEST_SEQ_ADDRESS_ANALYSIS_SHA256,
            actual: analysis.analysis_sha256,
        });
    }
    Ok(())
}

fn pointer_record_spec(raw: &[u8], field_offset: usize) -> Option<QuestSeqPointerRecordSpec> {
    QUEST_SEQ_POINTER_RECORD_SPECS
        .iter()
        .filter(|spec| {
            let Some(record_start) = field_offset.checked_sub(spec.pointer_offset) else {
                return false;
            };
            if raw.get(record_start) != Some(&spec.opcode)
                || record_start
                    .checked_add(spec.record_stride)
                    .is_none_or(|record_end| record_end > raw.len())
            {
                return false;
            }
            spec.required_byte
                .is_none_or(|condition| condition.matches(raw, record_start))
        })
        // 짧은 record처럼 보이는 byte가 긴 record의 필드 안에 중첩되면
        // 더 큰 pointer offset을 가진 바깥 record를 우선한다.
        .max_by_key(|spec| (spec.pointer_offset, spec.record_stride))
        .copied()
}

const fn pointer_record_field(spec: &QuestSeqPointerRecordSpec) -> &'static str {
    match (spec.opcode, spec.pointer_offset) {
        (0x80, _) => "OP80 pointer records",
        (0x82, _) => "OP82 pointer records",
        (0x83, _) => "OP83 pointer records",
        (0x85, 12) => "OP85 +12 pointer records",
        (0x85, 16) => "OP85 +16 pointer records",
        (0x88, _) => "OP88 pointer records",
        (0x8F, _) => "OP8F pointer records",
        (0x89, _) => "OP89 pointer records",
        (0x93, 4) => "OP93 +4 pointer records",
        (0x93, 12) => "OP93 +12 pointer records",
        (0x95, _) => "OP95 pointer records",
        (0x9D, _) => "OP9D pointer records",
        (0x9F, _) => "OP9F pointer records",
        (0xA1, _) => "OPA1 pointer records",
        (0xA2, _) => "OPA2 pointer records",
        (0xA8, _) => "OPA8 pointer records",
        (0xA9, _) => "OPA9 pointer records",
        (0xC1, _) => "OPC1 pointer records",
        (0xE3, _) => "OPE3 pointer records",
        _ => "unknown pointer records",
    }
}

fn require_total(
    field: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), QuestSeqAddressError> {
    if expected != actual {
        return Err(QuestSeqAddressError::BoxSummaryMismatch {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

fn require_sequence(
    sequence: &'static str,
    field: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), QuestSeqAddressError> {
    if expected != actual {
        return Err(QuestSeqAddressError::SequenceSummaryMismatch {
            sequence,
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "quest_seq_inventory_tests.rs"]
mod quest_seq_inventory_tests;
