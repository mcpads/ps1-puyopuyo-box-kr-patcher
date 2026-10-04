//! QUEST exact save-state의 TOWN00 interpreter와 번역 장면을 교차한다.
//!
//! callback pointer scan은 살아 있을 수 있는 interpreter 객체의 후보만 만든다.
//! 그 현재 stream이 canonical OP21 범위 안에 있고 검증된 번역 manifest에도
//! 있을 때만 `scene_id`와 `batch_id`를 붙인다. 이 식별만으로 font source의
//! 준비, window 무활성 또는 GPU 전송 완료를 주장하지 않는다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::coords::RuntimeAssetMap;
use crate::disasm::{QUEST_SEQ_INTERPRETER_RUNTIME, QUEST_SEQ_STREAM_POINTER_STATE_OFFSET};
use crate::script::{
    QuestSeqTextExtractionAnalysis, QuestTranslationBatchAnalysis, TOWN00_SEQ_LEN,
    TOWN00_SEQ_RUNTIME_BASE,
};

use super::{MednafenPsxState, MednafenStateError};

const QUEST_SEQ_INTERPRETER_CALLBACK_FIELD_OFFSET: u32 = 0x18;
const TOWN00_NAME: &str = "TOWN00.SEQ";

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuestFontSceneSpec {
    record_offset: usize,
    end_offset: usize,
    stream_id: String,
    scene_id: Option<String>,
    batch_id: Option<String>,
}

/// callback scan으로 찾은 TOWN00 interpreter 후보 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFontSceneCandidate {
    pub object_runtime: u32,
    pub stream_field_runtime: u32,
    pub stream_runtime: u32,
    pub stream_offset: usize,
    pub record_offset: Option<usize>,
    pub end_offset: Option<usize>,
    pub stream_id: Option<String>,
    pub scene_id: Option<String>,
    pub batch_id: Option<String>,
}

impl QuestFontSceneCandidate {
    /// canonical stream과 번역 batch가 모두 하나로 결합되었는지만 판정한다.
    pub const fn has_batch_binding(&self) -> bool {
        self.stream_id.is_some() && self.scene_id.is_some() && self.batch_id.is_some()
    }
}

/// exact state에서 찾은 TOWN00 interpreter 후보 전수.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFontSceneCensus {
    pub candidates: Vec<QuestFontSceneCandidate>,
    pub canonical_stream_candidates: usize,
    pub batch_bound_candidates: usize,
}

impl QuestFontSceneCensus {
    /// 모든 후보가 같은 번역 batch에 결합된 경우에만 그 ID를 반환한다.
    ///
    /// callback scan 후보가 0개이거나 하나라도 미결합이면 `None`이다.
    pub fn unique_bound_batch_id(&self) -> Option<&str> {
        let first = self.candidates.first()?.batch_id.as_deref()?;
        self.candidates
            .iter()
            .all(|candidate| {
                candidate.has_batch_binding() && candidate.batch_id.as_deref() == Some(first)
            })
            .then_some(first)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestFontSceneCensusError {
    CoordinateOverflow,
    InvalidStreamRange { stream_id: String },
    DuplicateBatchStream { stream_id: String },
    OverlappingStreams { first: String, second: String },
    State(MednafenStateError),
}

impl fmt::Display for QuestFontSceneCensusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinateOverflow => f.write_str("QUEST font scene 좌표 overflow"),
            Self::InvalidStreamRange { stream_id } => {
                write!(f, "{stream_id}: QUEST font scene stream 범위가 비정상")
            }
            Self::DuplicateBatchStream { stream_id } => {
                write!(f, "{stream_id}: QUEST font scene batch 결합이 중복")
            }
            Self::OverlappingStreams { first, second } => {
                write!(f, "QUEST font scene stream 범위가 겹침: {first} / {second}")
            }
            Self::State(source) => write!(f, "QUEST font scene state 읽기 실패: {source}"),
        }
    }
}

impl Error for QuestFontSceneCensusError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State(source) => Some(source),
            _ => None,
        }
    }
}

impl From<MednafenStateError> for QuestFontSceneCensusError {
    fn from(value: MednafenStateError) -> Self {
        Self::State(value)
    }
}

fn compile_specs(
    extraction: &QuestSeqTextExtractionAnalysis,
    batches: &QuestTranslationBatchAnalysis,
) -> Result<Vec<QuestFontSceneSpec>, QuestFontSceneCensusError> {
    let mut batch_by_stream = BTreeMap::new();
    for entry in &batches.entries {
        if batch_by_stream
            .insert(entry.stream_id.as_str(), entry)
            .is_some()
        {
            return Err(QuestFontSceneCensusError::DuplicateBatchStream {
                stream_id: entry.stream_id.clone(),
            });
        }
    }

    let mut specs = extraction
        .streams
        .iter()
        .filter(|stream| stream.sequence == TOWN00_NAME)
        .map(|stream| {
            let stream_id = stream.id();
            if stream.record_offset >= stream.end_offset {
                return Err(QuestFontSceneCensusError::InvalidStreamRange { stream_id });
            }
            let batch = batch_by_stream.get(stream_id.as_str());
            Ok(QuestFontSceneSpec {
                record_offset: stream.record_offset,
                end_offset: stream.end_offset,
                stream_id,
                scene_id: batch.map(|entry| entry.scene_id.clone()),
                batch_id: batch.map(|entry| entry.batch_id.clone()),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    specs.sort_by_key(|spec| spec.record_offset);
    for pair in specs.windows(2) {
        if pair[0].end_offset > pair[1].record_offset {
            return Err(QuestFontSceneCensusError::OverlappingStreams {
                first: pair[0].stream_id.clone(),
                second: pair[1].stream_id.clone(),
            });
        }
    }
    Ok(specs)
}

fn inspect_with_specs(
    state: &MednafenPsxState<'_>,
    specs: &[QuestFontSceneSpec],
) -> Result<QuestFontSceneCensus, QuestFontSceneCensusError> {
    let town00_map = RuntimeAssetMap::new(
        TOWN00_SEQ_RUNTIME_BASE,
        u32::try_from(TOWN00_SEQ_LEN).map_err(|_| QuestFontSceneCensusError::CoordinateOverflow)?,
    )
    .ok_or(QuestFontSceneCensusError::CoordinateOverflow)?;
    let mut candidates = Vec::new();
    for callback_field_runtime in state.find_aligned_u32_le(QUEST_SEQ_INTERPRETER_RUNTIME) {
        let Some(object_runtime) =
            callback_field_runtime.checked_sub(QUEST_SEQ_INTERPRETER_CALLBACK_FIELD_OFFSET)
        else {
            continue;
        };
        let stream_field_runtime = object_runtime
            .checked_add(QUEST_SEQ_STREAM_POINTER_STATE_OFFSET as u32)
            .ok_or(QuestFontSceneCensusError::CoordinateOverflow)?;
        let stream_runtime = state.read_u32_le(stream_field_runtime)?;
        let Some(stream_offset) = town00_map.file_offset(stream_runtime) else {
            continue;
        };
        let owner = specs
            .iter()
            .find(|spec| spec.record_offset <= stream_offset && stream_offset < spec.end_offset);
        candidates.push(QuestFontSceneCandidate {
            object_runtime,
            stream_field_runtime,
            stream_runtime,
            stream_offset,
            record_offset: owner.map(|spec| spec.record_offset),
            end_offset: owner.map(|spec| spec.end_offset),
            stream_id: owner.map(|spec| spec.stream_id.clone()),
            scene_id: owner.and_then(|spec| spec.scene_id.clone()),
            batch_id: owner.and_then(|spec| spec.batch_id.clone()),
        });
    }
    let canonical_stream_candidates = candidates
        .iter()
        .filter(|candidate| candidate.stream_id.is_some())
        .count();
    let batch_bound_candidates = candidates
        .iter()
        .filter(|candidate| candidate.has_batch_binding())
        .count();
    Ok(QuestFontSceneCensus {
        candidates,
        canonical_stream_candidates,
        batch_bound_candidates,
    })
}

/// exact state의 TOWN00 interpreter 후보를 검증된 OP21·번역 장면과 교차한다.
pub fn inspect_town00_quest_font_scenes(
    state: &MednafenPsxState<'_>,
    extraction: &QuestSeqTextExtractionAnalysis,
    batches: &QuestTranslationBatchAnalysis,
) -> Result<QuestFontSceneCensus, QuestFontSceneCensusError> {
    let specs = compile_specs(extraction, batches)?;
    inspect_with_specs(state, &specs)
}

#[cfg(test)]
#[path = "quest_font_scene_tests.rs"]
mod tests;
