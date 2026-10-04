//! QUEST sparse glyph cache 전환 중 SPU voice 연속성 검증.
//!
//! exact `62 → 62 → 50 → 0` 실행에서 각 OP21 hook 진입·복귀의 SPU
//! register와 SPU RAM을 결속한다. font upload가 main/CD volume, SPU control,
//! voice program을 바꾸지 않는지 확인하고, 장기 활성 voice가 Korean page
//! 교체와 원문 복구 동안 같은 program identity로 진행하는지 판정한다.
//! 이 결과는 특정 BGM track의 곡명·sequence identity까지 증명하지 않는다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::quest_glyph_cache_transition_poc::QuestGlyphCacheTransitionPocBuild;

pub const QUEST_GLYPH_CACHE_AUDIO_RUNTIME_SCHEMA_VERSION: &str =
    "quest-glyph-cache-audio-runtime-v1";

const PSX_SPU_RAM_BYTES: usize = 512 * 1024;
const QUEST_BODY_RETURN_RUNTIME: u32 = 0x8002_25E0;
const PAGE_STATES: [u8; 4] = [62, 62, 50, 0];
const LONG_LIVED_VOICE_CHECKPOINTS: [usize; 3] = [1, 2, 3];
const PAGE_CHANGE_VOICE_CHECKPOINTS: [usize; 2] = [2, 3];

const PRESERVED_SPU_FIELDS: [&str; 15] = [
    "g2.MainVolL",
    "g2.MainVolR",
    "g2.CDVolL",
    "g2.CDVolR",
    "g2.SPUCTRL",
    "g2.NoiseOn",
    "g2.FMOn",
    "g2.ReverbOn",
    "g2.RvbVolL",
    "g2.RvbVolR",
    "g2.VoiceOn",
    "g2.VoiceOff",
    "g3.SAddr",
    "g3.ADSRCTRL",
    "g3.Pitch",
];

/// 한 OP21 hook의 진입·복귀 SPU 증거.
#[derive(Debug, Clone, Copy)]
pub struct QuestGlyphCacheAudioRuntimeCheckpoint<'a> {
    pub entry_state: &'a [u8],
    pub return_state: &'a [u8],
    pub entry_spu: &'a [u8],
    pub return_spu: &'a [u8],
}

/// 두 Korean page와 다음 원문 복구까지의 SPU 증거.
#[derive(Debug, Clone, Copy)]
pub struct QuestGlyphCacheAudioRuntimeEvidence<'a> {
    pub first: QuestGlyphCacheAudioRuntimeCheckpoint<'a>,
    pub second: QuestGlyphCacheAudioRuntimeCheckpoint<'a>,
    pub third: QuestGlyphCacheAudioRuntimeCheckpoint<'a>,
    pub restored: QuestGlyphCacheAudioRuntimeCheckpoint<'a>,
}

/// sparse page 전환 중 SPU voice 연속성 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheAudioRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub entry_state_sha256: Vec<String>,
    pub return_state_sha256: Vec<String>,
    pub entry_spu_sha256: Vec<String>,
    pub return_spu_sha256: Vec<String>,
    pub page_states: Vec<u8>,
    pub load_image_calls: Vec<usize>,
    pub active_voice_at_entry: Vec<bool>,
    pub voice_progress_observed: Vec<bool>,
    pub preserved_spu_fields: usize,
    pub spu_control_preserved: bool,
    pub long_lived_voice_identity_preserved: bool,
    pub page_change_voice_progress_exact: bool,
    pub runtime_verified: bool,
    pub bgm_track_identity_verified: bool,
    pub release_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheAudioRuntimeError(String);

impl fmt::Display for QuestGlyphCacheAudioRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestGlyphCacheAudioRuntimeError {}

fn error(message: impl Into<String>) -> QuestGlyphCacheAudioRuntimeError {
    QuestGlyphCacheAudioRuntimeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn state_value(state: &Value, field: &str) -> Result<u32, QuestGlyphCacheAudioRuntimeError> {
    let raw = state
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("QUEST SPU state에 {field}가 없음")))?;
    u32::try_from(raw).map_err(|_| error(format!("QUEST SPU state {field} 범위 이탈")))
}

fn parse_state(raw: &[u8]) -> Result<Value, QuestGlyphCacheAudioRuntimeError> {
    serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST SPU state JSON 해석 실패: {source}")))
}

fn active_voice(state: &Value) -> Result<bool, QuestGlyphCacheAudioRuntimeError> {
    let level = state_value(state, "g3.ADSRLev")?;
    let volume_left = state_value(state, "g3.VolL")?;
    let volume_right = state_value(state, "g3.VolR")?;
    Ok(level != 0 && (volume_left != 0 || volume_right != 0))
}

fn same_voice_identity(
    left: &Value,
    right: &Value,
) -> Result<bool, QuestGlyphCacheAudioRuntimeError> {
    Ok(
        state_value(left, "g3.SAddr")? == state_value(right, "g3.SAddr")?
            && state_value(left, "g3.ADSRCTRL")? == state_value(right, "g3.ADSRCTRL")?,
    )
}

fn voice_progressed(
    entry: &Value,
    returned: &Value,
) -> Result<bool, QuestGlyphCacheAudioRuntimeError> {
    Ok(active_voice(entry)?
        && active_voice(returned)?
        && same_voice_identity(entry, returned)?
        && (state_value(entry, "g3.RAddr")? != state_value(returned, "g3.RAddr")?
            || state_value(entry, "g3.ADSRLev")? != state_value(returned, "g3.ADSRLev")?))
}

fn active_voices_progressed(checkpoints: &[VerifiedAudioCheckpoint], indices: &[usize]) -> bool {
    let mut active_voice_observed = false;
    for checkpoint in indices.iter().filter_map(|index| checkpoints.get(*index)) {
        if checkpoint.active_voice_at_entry {
            active_voice_observed = true;
            if !checkpoint.voice_progress_observed {
                return false;
            }
        }
    }
    active_voice_observed
}

#[derive(Debug)]
struct VerifiedAudioCheckpoint {
    entry_state_sha256: String,
    return_state_sha256: String,
    entry_spu_sha256: String,
    return_spu_sha256: String,
    entry_state: Value,
    active_voice_at_entry: bool,
    voice_progress_observed: bool,
}

fn verify_checkpoint(
    label: &str,
    checkpoint: QuestGlyphCacheAudioRuntimeCheckpoint<'_>,
    body_runtime: u32,
) -> Result<VerifiedAudioCheckpoint, QuestGlyphCacheAudioRuntimeError> {
    if checkpoint.entry_spu.len() != PSX_SPU_RAM_BYTES
        || checkpoint.return_spu.len() != PSX_SPU_RAM_BYTES
    {
        return Err(error(format!(
            "QUEST {label} SPU RAM 길이 불일치: entry={}, return={}, expected={PSX_SPU_RAM_BYTES}",
            checkpoint.entry_spu.len(),
            checkpoint.return_spu.len()
        )));
    }
    let entry_state = parse_state(checkpoint.entry_state)?;
    let return_state = parse_state(checkpoint.return_state)?;
    let entry_pc = state_value(&entry_state, "g0.PC")?;
    let return_pc = state_value(&return_state, "g0.PC")?;
    if entry_pc != body_runtime || return_pc != QUEST_BODY_RETURN_RUNTIME {
        return Err(error(format!(
            "QUEST {label} hook PC 불일치: entry=0x{entry_pc:08X}, return=0x{return_pc:08X}"
        )));
    }
    for field in PRESERVED_SPU_FIELDS {
        let entry = state_value(&entry_state, field)?;
        let returned = state_value(&return_state, field)?;
        if entry != returned {
            return Err(error(format!(
                "QUEST {label} SPU 보존 field 불일치: {field}={entry}→{returned}"
            )));
        }
    }
    Ok(VerifiedAudioCheckpoint {
        entry_state_sha256: digest(checkpoint.entry_state),
        return_state_sha256: digest(checkpoint.return_state),
        entry_spu_sha256: digest(checkpoint.entry_spu),
        return_spu_sha256: digest(checkpoint.return_spu),
        active_voice_at_entry: active_voice(&entry_state)?,
        voice_progress_observed: voice_progressed(&entry_state, &return_state)?,
        entry_state,
    })
}

/// exact sparse 전환 build의 SPU voice 연속성을 검증한다.
pub fn verify_quest_glyph_cache_audio_runtime(
    build: &QuestGlyphCacheTransitionPocBuild,
    evidence: QuestGlyphCacheAudioRuntimeEvidence<'_>,
) -> Result<QuestGlyphCacheAudioRuntimeReport, QuestGlyphCacheAudioRuntimeError> {
    if build.report.disc.image_sha256 != digest(&build.image)
        || build.report.runtime_verified
        || build.report.release_eligible
    {
        return Err(error("QUEST SPU runtime 입력 계약 drift"));
    }
    let [first_stream, second_stream, third_stream] = build.report.streams.as_slice() else {
        return Err(error("QUEST SPU runtime stream 수 불일치"));
    };
    let build_page_states = [
        first_stream.page_id,
        second_stream.page_id,
        third_stream.page_id,
        0,
    ];
    let build_load_image_calls = [first_stream.page_glyphs, 0, third_stream.page_glyphs, 160];
    if build_page_states != PAGE_STATES {
        return Err(error("QUEST SPU runtime page 계약 drift"));
    }
    let checkpoints = [
        ("first", evidence.first),
        ("second", evidence.second),
        ("third", evidence.third),
        ("restored", evidence.restored),
    ];
    let verified = checkpoints
        .into_iter()
        .map(|(label, checkpoint)| {
            verify_checkpoint(label, checkpoint, build.report.disc.body_runtime)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut long_lived_voice_identity_preserved = LONG_LIVED_VOICE_CHECKPOINTS
        .iter()
        .all(|index| verified[*index].active_voice_at_entry);
    for indices in LONG_LIVED_VOICE_CHECKPOINTS.windows(2) {
        long_lived_voice_identity_preserved &= same_voice_identity(
            &verified[indices[0]].entry_state,
            &verified[indices[1]].entry_state,
        )?;
    }
    let page_change_voice_progress_exact =
        active_voices_progressed(&verified, &PAGE_CHANGE_VOICE_CHECKPOINTS);
    if !page_change_voice_progress_exact {
        return Err(error("QUEST SPU page 전환 중 활성 voice 진행 불일치"));
    }

    Ok(QuestGlyphCacheAudioRuntimeReport {
        schema_version: QUEST_GLYPH_CACHE_AUDIO_RUNTIME_SCHEMA_VERSION,
        image_sha256: build.report.disc.image_sha256.clone(),
        entry_state_sha256: verified
            .iter()
            .map(|checkpoint| checkpoint.entry_state_sha256.clone())
            .collect(),
        return_state_sha256: verified
            .iter()
            .map(|checkpoint| checkpoint.return_state_sha256.clone())
            .collect(),
        entry_spu_sha256: verified
            .iter()
            .map(|checkpoint| checkpoint.entry_spu_sha256.clone())
            .collect(),
        return_spu_sha256: verified
            .iter()
            .map(|checkpoint| checkpoint.return_spu_sha256.clone())
            .collect(),
        page_states: PAGE_STATES.to_vec(),
        load_image_calls: build_load_image_calls.to_vec(),
        active_voice_at_entry: verified
            .iter()
            .map(|checkpoint| checkpoint.active_voice_at_entry)
            .collect(),
        voice_progress_observed: verified
            .iter()
            .map(|checkpoint| checkpoint.voice_progress_observed)
            .collect(),
        preserved_spu_fields: PRESERVED_SPU_FIELDS.len(),
        spu_control_preserved: true,
        long_lived_voice_identity_preserved,
        page_change_voice_progress_exact,
        runtime_verified: true,
        bgm_track_identity_verified: false,
        release_eligible: false,
    })
}

fn join<T: fmt::Display>(values: &[T]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// sparse page 전환의 SPU voice 연속성 결과를 한 행 TSV로 만든다.
pub fn render_quest_glyph_cache_audio_runtime_tsv(
    report: &QuestGlyphCacheAudioRuntimeReport,
) -> String {
    let mut out = String::from(
        "schema_version\timage_sha256\tentry_state_sha256\treturn_state_sha256\t\
         entry_spu_sha256\treturn_spu_sha256\tpage_states\tload_image_calls\t\
         active_voice_at_entry\tvoice_progress_observed\tpreserved_spu_fields\t\
         spu_control_preserved\tlong_lived_voice_identity_preserved\t\
         page_change_voice_progress_exact\truntime_verified\t\
         bgm_track_identity_verified\trelease_eligible\n",
    );
    writeln!(
        out,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        report.schema_version,
        report.image_sha256,
        join(&report.entry_state_sha256),
        join(&report.return_state_sha256),
        join(&report.entry_spu_sha256),
        join(&report.return_spu_sha256),
        join(&report.page_states),
        join(&report.load_image_calls),
        join(&report.active_voice_at_entry),
        join(&report.voice_progress_observed),
        report.preserved_spu_fields,
        report.spu_control_preserved,
        report.long_lived_voice_identity_preserved,
        report.page_change_voice_progress_exact,
        report.runtime_verified,
        report.bgm_track_identity_verified,
        report.release_eligible,
    )
    .expect("String write는 실패하지 않음");
    out
}

#[cfg(test)]
#[path = "quest_glyph_cache_audio_runtime_tests.rs"]
mod tests;
