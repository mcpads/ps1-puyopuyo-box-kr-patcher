//! QUEST glyph cache 전환 표본의 SPU voice 원본 귀속.
//!
//! runtime SPU RAM에 완전한 `.VB` body가 상주하는 위치를 원본
//! `/QUEST/RPG_SND.VDD`와 exact 대조한다. 각 checkpoint의 `SAddr`를 상주
//! bank의 body offset, VAB sample, program/tone 참조로 역산한다. 같은 이름의
//! `.SEQ`가 Main RAM에 상주하는 사실도 별도로 기록하지만, 상주만으로 현재
//! 재생 sequence나 BGM consumer를 확정하지 않는다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::archive::{audit_sound_archives, VabBank, VddArchive};

use super::quest_glyph_cache_transition_poc::QuestGlyphCacheTransitionPocBuild;

pub const QUEST_SPU_VOICE_SOURCE_RUNTIME_SCHEMA_VERSION: &str = "quest-spu-voice-source-runtime-v1";

const RPG_SOUND_ARCHIVE_PATH: &str = "/QUEST/RPG_SND.VDD;1";
const PSX_MAIN_RAM_BYTES: usize = 2 * 1024 * 1024;
const PSX_SPU_RAM_BYTES: usize = 512 * 1024;
const EXPECTED_FIRST_RESIDENT_VAB_BODIES: [(&str, usize, usize); 4] = [
    ("P2SYST.VB", 0x1010, 21_344),
    ("P2HITO.VB", 0x6810, 157_696),
    ("Q_FIEL.VB", 0x3_3810, 107_344),
    ("P1_02.VB", 0x5_0010, 156_496),
];
const EXPECTED_LATER_RESIDENT_VAB_BODIES: [(&str, usize, usize); 4] = [
    ("P2SYST.VB", 0x1010, 21_344),
    ("P2HITO.VB", 0x6810, 157_696),
    ("Q_FIEL.VB", 0x3_3810, 107_344),
    ("_01WAKU.VB", 0x5_0010, 131_696),
];
const EXPECTED_WAKEUP_SEQUENCE_OFFSETS: [usize; 2] = [0x13_9200, 0x18_8E00];
const LONG_LIVED_CHECKPOINTS: [usize; 3] = [1, 2, 3];

/// 한 runtime checkpoint의 source 탐색 입력.
#[derive(Debug, Clone, Copy)]
pub struct QuestSpuVoiceSourceRuntimeCheckpoint<'a> {
    pub state: &'a [u8],
    pub ram: &'a [u8],
    pub spu: &'a [u8],
}

/// 첫 glyph cache 진입부터 원문 복구까지의 source 탐색 입력.
#[derive(Debug, Clone, Copy)]
pub struct QuestSpuVoiceSourceRuntimeEvidence<'a> {
    pub first: QuestSpuVoiceSourceRuntimeCheckpoint<'a>,
    pub second: QuestSpuVoiceSourceRuntimeCheckpoint<'a>,
    pub third: QuestSpuVoiceSourceRuntimeCheckpoint<'a>,
    pub restored: QuestSpuVoiceSourceRuntimeCheckpoint<'a>,
}

/// SPU RAM에 exact 상주하는 원본 VAB body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentVabBody {
    pub checkpoint: &'static str,
    pub name: String,
    pub spu_offset: usize,
    pub body_bytes: usize,
    pub body_sha256: String,
}

/// Main RAM에 exact 상주하는 원본 sequence occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSequence {
    pub checkpoint: &'static str,
    pub name: String,
    pub ram_offset: usize,
    pub sequence_bytes: usize,
    pub sequence_sha256: String,
}

/// 한 checkpoint의 SPU start address를 원본 VAB tone까지 역산한 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestVoiceSource {
    pub checkpoint: &'static str,
    pub start_address: usize,
    pub body_name: String,
    pub body_spu_offset: usize,
    pub body_offset: usize,
    pub sample_number: u16,
    pub sample_offset: usize,
    pub program_number: u8,
    pub tone_number: u8,
}

/// QUEST 전환 표본의 VAB/SEQ 상주와 voice source 귀속 보고서.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSpuVoiceSourceRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub state_sha256: Vec<String>,
    pub ram_sha256: Vec<String>,
    pub spu_sha256: Vec<String>,
    pub resident_vab_bodies: Vec<QuestResidentVabBody>,
    pub resident_sequences: Vec<QuestResidentSequence>,
    pub voice_sources: Vec<QuestVoiceSource>,
    pub long_lived_voice_source_exact: bool,
    pub wakeup_vab_resident: bool,
    pub wakeup_sequence_resident: bool,
    pub sequence_consumer_verified: bool,
    pub bgm_track_identity_verified: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSpuVoiceSourceRuntimeError(String);

impl fmt::Display for QuestSpuVoiceSourceRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSpuVoiceSourceRuntimeError {}

fn error(message: impl Into<String>) -> QuestSpuVoiceSourceRuntimeError {
    QuestSpuVoiceSourceRuntimeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

#[derive(Debug)]
struct VabSource<'a> {
    body_name: String,
    body: &'a [u8],
    bank: VabBank,
}

fn sound_sources<'a>(
    archive: &'a VddArchive,
) -> Result<Vec<VabSource<'a>>, QuestSpuVoiceSourceRuntimeError> {
    audit_sound_archives([(RPG_SOUND_ARCHIVE_PATH, archive)])
        .map_err(|source| error(format!("QUEST Sound VDD 검증 실패: {source}")))?;
    let entries = archive
        .entries()
        .iter()
        .map(|entry| (entry.name().to_ascii_uppercase(), entry))
        .collect::<BTreeMap<_, _>>();
    entries
        .iter()
        .filter_map(|(name, header_entry)| {
            let stem = name.strip_suffix(".VH")?;
            let body_name = format!("{stem}.VB");
            Some((header_entry, body_name))
        })
        .map(|(header_entry, body_name)| {
            let body_entry = entries
                .get(&body_name)
                .ok_or_else(|| error(format!("QUEST Sound VDD에 {body_name}가 없음")))?;
            let header = archive.read_file(header_entry);
            let body = archive.read_file(body_entry);
            let bank = VabBank::parse(header, body)
                .map_err(|source| error(format!("{body_name} VAB 검증 실패: {source}")))?;
            Ok(VabSource {
                body_name,
                body,
                bank,
            })
        })
        .collect()
}

fn exact_offsets(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    let prefix_bytes = needle.len().min(64);
    haystack
        .windows(prefix_bytes)
        .enumerate()
        .filter_map(|(offset, window)| {
            (window == &needle[..prefix_bytes]
                && haystack.get(offset..offset + needle.len()) == Some(needle))
            .then_some(offset)
        })
        .collect()
}

fn resident_vab_bodies(
    checkpoint: &'static str,
    sources: &[VabSource<'_>],
    spu: &[u8],
) -> Vec<QuestResidentVabBody> {
    let mut rows = sources
        .iter()
        .flat_map(|source| {
            exact_offsets(spu, source.body)
                .into_iter()
                .map(|spu_offset| QuestResidentVabBody {
                    checkpoint,
                    name: source.body_name.clone(),
                    spu_offset,
                    body_bytes: source.body.len(),
                    body_sha256: digest(source.body),
                })
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| (row.spu_offset, row.name.clone()));
    rows
}

fn resident_sequences(
    checkpoint: &'static str,
    archive: &VddArchive,
    ram: &[u8],
) -> Vec<QuestResidentSequence> {
    let mut rows = archive
        .entries()
        .iter()
        .filter(|entry| entry.name().to_ascii_uppercase().ends_with(".SEQ"))
        .flat_map(|entry| {
            let sequence = archive.read_file(entry);
            exact_offsets(ram, sequence)
                .into_iter()
                .map(|ram_offset| QuestResidentSequence {
                    checkpoint,
                    name: entry.name().to_ascii_uppercase(),
                    ram_offset,
                    sequence_bytes: sequence.len(),
                    sequence_sha256: digest(sequence),
                })
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| (row.ram_offset, row.name.clone()));
    rows
}

fn state_value(state: &Value, field: &str) -> Result<usize, QuestSpuVoiceSourceRuntimeError> {
    let value = state
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("QUEST voice source state에 {field}가 없음")))?;
    usize::try_from(value).map_err(|_| error(format!("QUEST voice source {field} 범위 이탈")))
}

fn locate_voice_source(
    checkpoint: &'static str,
    start_address: usize,
    resident: &[QuestResidentVabBody],
    sources: &[VabSource<'_>],
) -> Result<QuestVoiceSource, QuestSpuVoiceSourceRuntimeError> {
    let containing = resident
        .iter()
        .filter(|row| {
            row.spu_offset <= start_address
                && start_address < row.spu_offset.saturating_add(row.body_bytes)
        })
        .collect::<Vec<_>>();
    let [resident_body] = containing.as_slice() else {
        return Err(error(format!(
            "QUEST {checkpoint} SAddr 0x{start_address:X}의 exact 상주 VAB가 하나가 아님: {}",
            containing.len()
        )));
    };
    let source = sources
        .iter()
        .find(|source| source.body_name == resident_body.name)
        .ok_or_else(|| error(format!("{} 원본 VAB source 누락", resident_body.name)))?;
    let body_offset = start_address - resident_body.spu_offset;
    let sample = source.bank.sample_containing(body_offset).ok_or_else(|| {
        error(format!(
            "{} +0x{body_offset:X} sample 누락",
            source.body_name
        ))
    })?;
    let tones = source.bank.tones_for_sample(sample.number);
    let [tone] = tones.as_slice() else {
        return Err(error(format!(
            "{} sample {}의 tone 참조가 하나가 아님: {}",
            source.body_name,
            sample.number,
            tones.len()
        )));
    };
    Ok(QuestVoiceSource {
        checkpoint,
        start_address,
        body_name: source.body_name.clone(),
        body_spu_offset: resident_body.spu_offset,
        body_offset,
        sample_number: sample.number,
        sample_offset: body_offset - sample.body_range.start,
        program_number: tone.program_number,
        tone_number: tone.number,
    })
}

fn verify_expected_resident_bodies(
    actual: &[QuestResidentVabBody],
    expected: &[(&str, usize, usize)],
) -> Result<(), QuestSpuVoiceSourceRuntimeError> {
    let actual = actual
        .iter()
        .map(|row| (row.name.as_str(), row.spu_offset, row.body_bytes))
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(error(format!(
            "QUEST exact 상주 VAB 분모 drift: {actual:?}"
        )));
    }
    Ok(())
}

/// glyph cache 전환의 voice start를 원본 VAB sample과 tone까지 귀속한다.
pub fn verify_quest_spu_voice_source_runtime(
    build: &QuestGlyphCacheTransitionPocBuild,
    sound_vdd: &[u8],
    evidence: QuestSpuVoiceSourceRuntimeEvidence<'_>,
) -> Result<QuestSpuVoiceSourceRuntimeReport, QuestSpuVoiceSourceRuntimeError> {
    if build.report.disc.image_sha256 != digest(&build.image)
        || build.report.runtime_verified
        || build.report.release_eligible
    {
        return Err(error("QUEST voice source runtime build 계약 drift"));
    }
    let archive = VddArchive::parse(sound_vdd)
        .map_err(|source| error(format!("QUEST Sound VDD 파싱 실패: {source}")))?;
    let sources = sound_sources(&archive)?;
    let checkpoints = [
        ("first", evidence.first),
        ("second", evidence.second),
        ("third", evidence.third),
        ("restored", evidence.restored),
    ];
    let mut states = Vec::with_capacity(checkpoints.len());
    let mut resident_per_checkpoint = Vec::with_capacity(checkpoints.len());
    for (checkpoint_index, (label, checkpoint)) in checkpoints.into_iter().enumerate() {
        if checkpoint.ram.len() != PSX_MAIN_RAM_BYTES || checkpoint.spu.len() != PSX_SPU_RAM_BYTES {
            return Err(error(format!(
                "QUEST {label} memory 길이 불일치: RAM={}, SPU={}",
                checkpoint.ram.len(),
                checkpoint.spu.len()
            )));
        }
        let state: Value = serde_json::from_slice(checkpoint.state)
            .map_err(|source| error(format!("QUEST {label} state JSON 해석 실패: {source}")))?;
        let pc = state_value(&state, "g0.PC")?;
        if pc != build.report.disc.body_runtime as usize {
            return Err(error(format!("QUEST {label} hook PC 불일치: 0x{pc:X}")));
        }
        let start_address = state_value(&state, "g3.SAddr")?;
        let resident = resident_vab_bodies(label, &sources, checkpoint.spu);
        let expected_resident = if checkpoint_index == 0 {
            EXPECTED_FIRST_RESIDENT_VAB_BODIES.as_slice()
        } else {
            EXPECTED_LATER_RESIDENT_VAB_BODIES.as_slice()
        };
        verify_expected_resident_bodies(&resident, expected_resident)?;
        states.push((label, checkpoint, start_address));
        resident_per_checkpoint.push(resident);
    }

    let voice_sources = states
        .iter()
        .zip(&resident_per_checkpoint)
        .map(|((label, _, start_address), resident)| {
            locate_voice_source(label, *start_address, resident, &sources)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let long_lived_voice_source_exact = LONG_LIVED_CHECKPOINTS.windows(2).all(|indices| {
        let left = &voice_sources[indices[0]];
        let right = &voice_sources[indices[1]];
        left.start_address == right.start_address
            && left.body_name == right.body_name
            && left.body_spu_offset == right.body_spu_offset
            && left.body_offset == right.body_offset
            && left.sample_number == right.sample_number
            && left.sample_offset == right.sample_offset
            && left.program_number == right.program_number
            && left.tone_number == right.tone_number
    });
    let resident_sequences = states
        .iter()
        .flat_map(|(label, checkpoint, _)| resident_sequences(label, &archive, checkpoint.ram))
        .collect::<Vec<_>>();
    let wakeup_offsets = resident_sequences
        .iter()
        .filter(|row| row.checkpoint == "second" && row.name == "_01WAKU.SEQ")
        .map(|row| row.ram_offset)
        .collect::<Vec<_>>();
    if wakeup_offsets != EXPECTED_WAKEUP_SEQUENCE_OFFSETS {
        return Err(error(format!(
            "QUEST _01WAKU.SEQ 상주 offset drift: {wakeup_offsets:X?}"
        )));
    }
    let resident_vab_bodies = resident_per_checkpoint
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let wakeup_vab_resident = resident_vab_bodies.iter().any(|row| {
        row.checkpoint == "second" && row.name == "_01WAKU.VB" && row.spu_offset == 0x5_0010
    });
    let wakeup_sequence_resident = wakeup_offsets.len() == 2;

    Ok(QuestSpuVoiceSourceRuntimeReport {
        schema_version: QUEST_SPU_VOICE_SOURCE_RUNTIME_SCHEMA_VERSION,
        image_sha256: build.report.disc.image_sha256.clone(),
        state_sha256: states
            .iter()
            .map(|(_, checkpoint, _)| digest(checkpoint.state))
            .collect(),
        ram_sha256: states
            .iter()
            .map(|(_, checkpoint, _)| digest(checkpoint.ram))
            .collect(),
        spu_sha256: states
            .iter()
            .map(|(_, checkpoint, _)| digest(checkpoint.spu))
            .collect(),
        resident_vab_bodies,
        resident_sequences,
        voice_sources,
        long_lived_voice_source_exact,
        wakeup_vab_resident,
        wakeup_sequence_resident,
        sequence_consumer_verified: false,
        bgm_track_identity_verified: false,
        runtime_verified: true,
        release_eligible: false,
    })
}

/// voice source runtime 보고서를 deterministic TSV로 직렬화한다.
pub fn render_quest_spu_voice_source_runtime_tsv(
    report: &QuestSpuVoiceSourceRuntimeReport,
) -> String {
    let mut output = String::from("kind\tname\tlocation\tbytes\tdetail\tsha256\n");
    for row in &report.resident_vab_bodies {
        writeln!(
            output,
            "vab_body\t{}:{}\t0x{:X}\t{}\texact_spu_resident\t{}",
            row.checkpoint, row.name, row.spu_offset, row.body_bytes, row.body_sha256
        )
        .unwrap();
    }
    for row in &report.resident_sequences {
        writeln!(
            output,
            "sequence\t{}:{}\t0x{:X}\t{}\texact_main_ram_resident\t{}",
            row.checkpoint, row.name, row.ram_offset, row.sequence_bytes, row.sequence_sha256
        )
        .unwrap();
    }
    for row in &report.voice_sources {
        writeln!(
            output,
            "voice\t{}\t0x{:X}\t0\t{}+0x{:X};sample={};sample_offset=0x{:X};program={};tone={}\t",
            row.checkpoint,
            row.start_address,
            row.body_name,
            row.body_offset,
            row.sample_number,
            row.sample_offset,
            row.program_number,
            row.tone_number,
        )
        .unwrap();
    }
    writeln!(
        output,
        "summary\t{}\t\t0\tlong_lived_source_exact={};wakeup_vab_resident={};wakeup_sequence_resident={};sequence_consumer_verified={};bgm_track_identity_verified={};runtime_verified={};release_eligible={}\t{}",
        report.schema_version,
        report.long_lived_voice_source_exact,
        report.wakeup_vab_resident,
        report.wakeup_sequence_resident,
        report.sequence_consumer_verified,
        report.bgm_track_identity_verified,
        report.runtime_verified,
        report.release_eligible,
        report.image_sha256,
    )
    .unwrap();
    output
}

#[cfg(test)]
#[path = "quest_spu_voice_source_runtime_tests.rs"]
mod quest_spu_voice_source_runtime_tests;
