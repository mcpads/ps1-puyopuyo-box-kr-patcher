//! QUEST 수동 composition 원장의 상태와 검증 결과형.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

pub const QUEST_MANUAL_COMPOSITION_SCHEMA_VERSION: &str = "quest-manual-composition";
pub const QUEST_MANUAL_COMPOSITION_SEGMENT_SCHEMA_VERSION: &str =
    "quest-manual-composition-segment";

/// 사람이 쓴 문장의 기원. 자동 변환 결과는 이 원장에 허용하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestCompositionCandidateBasis {
    ManualRevision,
}

impl QuestCompositionCandidateBasis {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ManualRevision => "manual-revision",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "manual-revision" => Some(Self::ManualRevision),
            _ => None,
        }
    }
}

/// 현재 조판 초안의 승격 가능 범위.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestManualCompositionStatus {
    ManualDraftNeedsGeometry,
}

impl QuestManualCompositionStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ManualDraftNeedsGeometry => "manual-draft-needs-geometry",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "manual-draft-needs-geometry" => Some(Self::ManualDraftNeedsGeometry),
            _ => None,
        }
    }
}

/// 선택 문장의 연속 text range 하나와 그 뒤의 원문 control anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestManualCompositionSegment {
    pub order: usize,
    /// `FF0F` 입력 대기로 나뉜 사람이 읽는 단계. visual page clear를 뜻하지 않는다.
    pub input_phase: usize,
    /// 같은 입력 단계에서 `FF04`로 나뉜 수동 조판 줄.
    pub phase_line: usize,
    pub selected_text_start: usize,
    pub selected_text_end: usize,
    pub text: String,
    pub following_control_token_index: usize,
    pub following_control_code: u8,
}

/// 한 번역 stream에 사람이 직접 작성한 표시 문장과 조판 초안.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestManualComposition {
    pub id: String,
    pub batch_id: String,
    pub stream_id: String,
    pub candidate_basis: QuestCompositionCandidateBasis,
    pub selected_korean: String,
    pub selected_text_sha256: String,
    pub normalized_text_sha256: String,
    pub display_korean: String,
    pub normalized_display_korean: String,
    pub source_stream_sha256: String,
    pub geometry_profile_id: Option<String>,
    pub geometry_sha256: Option<String>,
    pub status: QuestManualCompositionStatus,
    pub notes: String,
    pub segments: Vec<QuestManualCompositionSegment>,
    pub candidate_glyph_run_lengths: Vec<usize>,
    pub geometry_verified: bool,
    pub layout_approved: bool,
}

/// 현재 검토 batch와 결속된 수동 조판 초안 집합.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestManualCompositionAnalysis {
    pub compositions: Vec<QuestManualComposition>,
    pub manually_authored: usize,
    pub geometry_verified: usize,
    pub layout_approved: usize,
    pub product_eligible: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestManualCompositionError(String);

impl fmt::Display for QuestManualCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestManualCompositionError {}

pub(super) fn composition_error(message: impl Into<String>) -> QuestManualCompositionError {
    QuestManualCompositionError(message.into())
}

pub(super) fn sha256_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
