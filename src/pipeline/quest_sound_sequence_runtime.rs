//! QUEST glyph cache 전환 중 실제 sound sequence 소비와 진행 검증.
//!
//! 지원 원본의 typed 소비 경로를 전제로 `_01WAKU.SEQ`가 destination과
//! staging RAM에 exact 적재되고 application slot 0의 handle 0으로 열린 뒤,
//! low-level event cursor가 같은 staging sequence 안에서 진행하는지 검증한다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::archive::{audit_sound_archives, VddArchive};
use crate::coords::{psx_main_ram_offset, RuntimeAssetMap, PSX_MAIN_RAM_LEN};
use crate::disasm::{
    QuestSoundSequenceConsumerSummary, QUEST_SOUND_APPLICATION_SLOT_RUNTIME,
    QUEST_SOUND_LOADER_STATE_RUNTIME, QUEST_SOUND_LOAD_DESTINATION_RUNTIME,
    QUEST_SOUND_SEQUENCE_POINTER_TABLE_RUNTIME, QUEST_SOUND_SEQUENCE_STATE_RUNTIME,
    QUEST_SOUND_STAGING_RUNTIME, QUEST_WAKEUP_SEQUENCE_BYTES,
    QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME,
};

use super::quest_glyph_cache_transition_poc::QuestGlyphCacheTransitionPocBuild;

pub const QUEST_SOUND_SEQUENCE_RUNTIME_SCHEMA_VERSION: &str = "quest-sound-sequence-runtime-v1";

const RPG_SOUND_ARCHIVE_PATH: &str = "/QUEST/RPG_SND.VDD;1";
const WAKEUP_SEQUENCE_NAME: &str = "_01WAKU.SEQ";
const REQUEST_ID: u16 = 31;
const APPLICATION_SLOT: i16 = 0;
const SEQUENCE_HANDLE: i16 = 0;
const PLAYING_STATUS: u8 = 2;

/// 한 시점의 2 MiB Main RAM 증거.
#[derive(Debug, Clone, Copy)]
pub struct QuestSoundSequenceRuntimeCheckpoint<'a> {
    pub ram: &'a [u8],
}

/// loader 전이와 세 재생 진행 시점.
#[derive(Debug, Clone, Copy)]
pub struct QuestSoundSequenceRuntimeEvidence<'a> {
    pub first: QuestSoundSequenceRuntimeCheckpoint<'a>,
    pub second: QuestSoundSequenceRuntimeCheckpoint<'a>,
    pub third: QuestSoundSequenceRuntimeCheckpoint<'a>,
    pub restored: QuestSoundSequenceRuntimeCheckpoint<'a>,
}

/// 원본 sequence와 exact인 한 RAM 사본.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSoundSequenceRuntimeCopy {
    pub checkpoint: &'static str,
    pub runtime: u32,
    pub bytes: usize,
}

/// low-level sequence state에서 읽은 현재 event cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSoundSequenceRuntimeCursor {
    pub checkpoint: &'static str,
    pub cursor_runtime: u32,
    pub sequence_offset: usize,
}

/// `_01WAKU.SEQ` 적재, start, 진행을 결속한 보고서.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSoundSequenceRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub sequence_sha256: String,
    pub ram_sha256: Vec<String>,
    pub copies: Vec<QuestSoundSequenceRuntimeCopy>,
    pub cursors: Vec<QuestSoundSequenceRuntimeCursor>,
    pub request_id: u16,
    pub application_slot: i16,
    pub sequence_handle: i16,
    pub playing_status: u8,
    pub sequence_consumer_verified: bool,
    pub sequence_start_verified: bool,
    pub sequence_progress_verified: bool,
    pub bgm_track_identity_verified: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSoundSequenceRuntimeError(String);

impl fmt::Display for QuestSoundSequenceRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSoundSequenceRuntimeError {}

fn error(message: impl Into<String>) -> QuestSoundSequenceRuntimeError {
    QuestSoundSequenceRuntimeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn ram_range(
    ram: &[u8],
    runtime: u32,
    bytes: usize,
) -> Result<&[u8], QuestSoundSequenceRuntimeError> {
    let offset = psx_main_ram_offset(runtime, bytes).ok_or_else(|| {
        error(format!(
            "QUEST Main RAM 좌표 범위 이탈: 0x{runtime:08X} + {bytes}B"
        ))
    })?;
    ram.get(offset..offset + bytes).ok_or_else(|| {
        error(format!(
            "QUEST Main RAM dump 범위 이탈: 0x{runtime:08X} + {bytes}B"
        ))
    })
}

fn read_u16(ram: &[u8], runtime: u32) -> Result<u16, QuestSoundSequenceRuntimeError> {
    Ok(u16::from_le_bytes(
        ram_range(ram, runtime, 2)?.try_into().unwrap(),
    ))
}

fn read_i16(ram: &[u8], runtime: u32) -> Result<i16, QuestSoundSequenceRuntimeError> {
    Ok(i16::from_le_bytes(
        ram_range(ram, runtime, 2)?.try_into().unwrap(),
    ))
}

fn read_u32(ram: &[u8], runtime: u32) -> Result<u32, QuestSoundSequenceRuntimeError> {
    Ok(u32::from_le_bytes(
        ram_range(ram, runtime, 4)?.try_into().unwrap(),
    ))
}

fn expect_value<T: fmt::Debug + PartialEq>(
    checkpoint: &str,
    field: &str,
    actual: T,
    expected: T,
) -> Result<(), QuestSoundSequenceRuntimeError> {
    if actual != expected {
        return Err(error(format!(
            "QUEST {checkpoint} {field} drift: expected={expected:?}, actual={actual:?}"
        )));
    }
    Ok(())
}

fn verify_loader_transition(
    first: &[u8],
    later: &[(&'static str, &[u8])],
) -> Result<(), QuestSoundSequenceRuntimeError> {
    let base = QUEST_SOUND_LOADER_STATE_RUNTIME;
    expect_value("first", "loader stage", read_u16(first, base)?, 1)?;
    expect_value(
        "first",
        "request id",
        read_u16(first, base + 2)?,
        REQUEST_ID,
    )?;
    expect_value(
        "first",
        "application slot",
        read_i16(first, base + 4)?,
        APPLICATION_SLOT,
    )?;
    expect_value(
        "first",
        "sequence descriptor",
        read_u32(first, base + 8)?,
        QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME,
    )?;
    expect_value(
        "first",
        "staging pointer",
        read_u32(first, base + 12)?,
        QUEST_SOUND_STAGING_RUNTIME,
    )?;
    expect_value("first", "sequence handle", read_i16(first, base + 16)?, -1)?;
    expect_value("first", "loader status", read_u16(first, base + 18)?, 0)?;

    const CLEARED_LOADER: [u8; 20] = [
        0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xFF, 0xFF, 0x00, 0x00,
    ];
    for (checkpoint, ram) in later {
        expect_value(
            checkpoint,
            "cleared loader",
            ram_range(ram, base, CLEARED_LOADER.len())?,
            CLEARED_LOADER.as_slice(),
        )?;
    }
    Ok(())
}

fn verify_descriptor(
    checkpoint: &str,
    ram: &[u8],
    expected_destination: u32,
) -> Result<(), QuestSoundSequenceRuntimeError> {
    let base = QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME;
    expect_value(checkpoint, "descriptor active", read_u16(ram, base)?, 1)?;
    expect_value(checkpoint, "descriptor kind", read_u16(ram, base + 2)?, 1)?;
    expect_value(
        checkpoint,
        "descriptor size",
        read_u32(ram, base + 8)?,
        QUEST_WAKEUP_SEQUENCE_BYTES as u32,
    )?;
    expect_value(
        checkpoint,
        "descriptor remaining size",
        read_u32(ram, base + 12)?,
        QUEST_WAKEUP_SEQUENCE_BYTES as u32,
    )?;
    expect_value(
        checkpoint,
        "descriptor destination",
        read_u32(ram, base + 16)?,
        expected_destination,
    )
}

fn verify_application_slot(
    checkpoint: &str,
    ram: &[u8],
) -> Result<(), QuestSoundSequenceRuntimeError> {
    let base = QUEST_SOUND_APPLICATION_SLOT_RUNTIME;
    expect_value(checkpoint, "slot active", read_u16(ram, base)?, 1)?;
    expect_value(
        checkpoint,
        "sequence handle",
        read_i16(ram, base + 2)?,
        SEQUENCE_HANDLE,
    )?;
    expect_value(
        checkpoint,
        "left volume",
        ram_range(ram, base + 6, 1)?[0],
        127,
    )?;
    expect_value(
        checkpoint,
        "right volume",
        ram_range(ram, base + 7, 1)?[0],
        127,
    )?;
    expect_value(
        checkpoint,
        "slot descriptor",
        read_u32(ram, base + 12)?,
        QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME,
    )?;
    expect_value(
        checkpoint,
        "slot request id",
        read_u16(ram, base + 16)?,
        REQUEST_ID,
    )?;
    expect_value(
        checkpoint,
        "slot playing status",
        ram_range(ram, base + 18, 1)?[0],
        PLAYING_STATUS,
    )?;
    expect_value(
        checkpoint,
        "slot target volume",
        ram_range(ram, base + 19, 1)?[0],
        127,
    )?;
    expect_value(checkpoint, "slot tempo", read_u32(ram, base + 28)?, 0xAA)
}

fn verify_runtime_memory(
    sequence: &[u8],
    evidence: QuestSoundSequenceRuntimeEvidence<'_>,
) -> Result<
    (
        Vec<QuestSoundSequenceRuntimeCopy>,
        Vec<QuestSoundSequenceRuntimeCursor>,
    ),
    QuestSoundSequenceRuntimeError,
> {
    if sequence.len() != QUEST_WAKEUP_SEQUENCE_BYTES {
        return Err(error(format!(
            "QUEST _01WAKU.SEQ 길이 drift: expected={}, actual={}",
            QUEST_WAKEUP_SEQUENCE_BYTES,
            sequence.len()
        )));
    }
    let checkpoints = [
        ("first", evidence.first.ram),
        ("second", evidence.second.ram),
        ("third", evidence.third.ram),
        ("restored", evidence.restored.ram),
    ];
    for (checkpoint, ram) in checkpoints {
        if ram.len() != PSX_MAIN_RAM_LEN {
            return Err(error(format!(
                "QUEST {checkpoint} Main RAM 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
                ram.len()
            )));
        }
    }

    let later = [
        ("second", evidence.second.ram),
        ("third", evidence.third.ram),
        ("restored", evidence.restored.ram),
    ];
    verify_loader_transition(evidence.first.ram, &later)?;
    verify_descriptor("first", evidence.first.ram, 0)?;

    let sequence_map = RuntimeAssetMap::new(
        QUEST_SOUND_STAGING_RUNTIME,
        QUEST_WAKEUP_SEQUENCE_BYTES as u32,
    )
    .ok_or_else(|| error("QUEST staging sequence 사상 overflow"))?;
    let mut copies = Vec::new();
    let mut cursors = Vec::new();
    for (checkpoint, ram) in later {
        verify_descriptor(checkpoint, ram, QUEST_SOUND_LOAD_DESTINATION_RUNTIME)?;
        verify_application_slot(checkpoint, ram)?;
        expect_value(
            checkpoint,
            "sequence state pointer",
            read_u32(ram, QUEST_SOUND_SEQUENCE_POINTER_TABLE_RUNTIME)?,
            QUEST_SOUND_SEQUENCE_STATE_RUNTIME,
        )?;
        for runtime in [
            QUEST_SOUND_LOAD_DESTINATION_RUNTIME,
            QUEST_SOUND_STAGING_RUNTIME,
        ] {
            expect_value(
                checkpoint,
                "exact sequence copy",
                ram_range(ram, runtime, sequence.len())?,
                sequence,
            )?;
            copies.push(QuestSoundSequenceRuntimeCopy {
                checkpoint,
                runtime,
                bytes: sequence.len(),
            });
        }
        let cursor = read_u32(ram, QUEST_SOUND_SEQUENCE_STATE_RUNTIME)?;
        let sequence_offset = sequence_map.file_offset(cursor).ok_or_else(|| {
            error(format!(
                "QUEST {checkpoint} event cursor가 _01WAKU.SEQ staging 밖임: 0x{cursor:08X}"
            ))
        })?;
        cursors.push(QuestSoundSequenceRuntimeCursor {
            checkpoint,
            cursor_runtime: cursor,
            sequence_offset,
        });
    }
    if !cursors
        .windows(2)
        .all(|pair| pair[0].cursor_runtime < pair[1].cursor_runtime)
    {
        return Err(error(
            "QUEST _01WAKU.SEQ event cursor 진행이 단조 증가하지 않음",
        ));
    }
    Ok((copies, cursors))
}

/// `_01WAKU.SEQ`의 typed 소비 경로와 runtime 진행을 한 artifact에 결속한다.
pub fn verify_quest_sound_sequence_runtime(
    build: &QuestGlyphCacheTransitionPocBuild,
    consumer: &QuestSoundSequenceConsumerSummary,
    sound_vdd: &[u8],
    evidence: QuestSoundSequenceRuntimeEvidence<'_>,
) -> Result<QuestSoundSequenceRuntimeReport, QuestSoundSequenceRuntimeError> {
    if build.report.disc.image_sha256 != digest(&build.image)
        || build.report.runtime_verified
        || build.report.release_eligible
    {
        return Err(error("QUEST sound sequence runtime build 계약 drift"));
    }
    if consumer.sequence_descriptor_runtime != QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME
        || consumer.sequence_bytes != QUEST_WAKEUP_SEQUENCE_BYTES
        || consumer.load_destination_runtime != QUEST_SOUND_LOAD_DESTINATION_RUNTIME
        || consumer.application_slot_runtime != QUEST_SOUND_APPLICATION_SLOT_RUNTIME
        || consumer.application_slot_stride != 0x20
        || consumer.sequence_start_runtime != 0x8007_D5E4
        || consumer.low_level_start_core_runtime != 0x800A_0A18
    {
        return Err(error("QUEST sound sequence static consumer 계약 drift"));
    }

    let archive = VddArchive::parse(sound_vdd)
        .map_err(|source| error(format!("QUEST Sound VDD 파싱 실패: {source}")))?;
    audit_sound_archives([(RPG_SOUND_ARCHIVE_PATH, &archive)])
        .map_err(|source| error(format!("QUEST Sound VDD 검증 실패: {source}")))?;
    let entry = archive
        .find(WAKEUP_SEQUENCE_NAME)
        .ok_or_else(|| error("QUEST Sound VDD에 _01WAKU.SEQ가 없음"))?;
    let sequence = archive.read_file(entry);
    let (copies, cursors) = verify_runtime_memory(sequence, evidence)?;

    Ok(QuestSoundSequenceRuntimeReport {
        schema_version: QUEST_SOUND_SEQUENCE_RUNTIME_SCHEMA_VERSION,
        image_sha256: build.report.disc.image_sha256.clone(),
        sequence_sha256: digest(sequence),
        ram_sha256: [
            evidence.first.ram,
            evidence.second.ram,
            evidence.third.ram,
            evidence.restored.ram,
        ]
        .iter()
        .map(|ram| digest(ram))
        .collect(),
        copies,
        cursors,
        request_id: REQUEST_ID,
        application_slot: APPLICATION_SLOT,
        sequence_handle: SEQUENCE_HANDLE,
        playing_status: PLAYING_STATUS,
        sequence_consumer_verified: true,
        sequence_start_verified: true,
        sequence_progress_verified: true,
        bgm_track_identity_verified: true,
        runtime_verified: true,
        release_eligible: false,
    })
}

/// sound sequence runtime 보고서를 deterministic TSV로 직렬화한다.
pub fn render_quest_sound_sequence_runtime_tsv(report: &QuestSoundSequenceRuntimeReport) -> String {
    let mut output = String::from("kind\tcheckpoint\tlocation\tvalue\tdetail\tsha256\n");
    for copy in &report.copies {
        writeln!(
            output,
            "sequence_copy\t{}\t0x{:08X}\t{}\texact:_01WAKU.SEQ\t{}",
            copy.checkpoint, copy.runtime, copy.bytes, report.sequence_sha256
        )
        .unwrap();
    }
    for cursor in &report.cursors {
        let ram_index = match cursor.checkpoint {
            "second" => 1,
            "third" => 2,
            "restored" => 3,
            _ => 0,
        };
        writeln!(
            output,
            "event_cursor\t{}\t0x{:08X}\t0x{:X}\tinside_staging_sequence\t{}",
            cursor.checkpoint,
            cursor.cursor_runtime,
            cursor.sequence_offset,
            report.ram_sha256[ram_index]
        )
        .unwrap();
    }
    writeln!(
        output,
        "summary\tall\t0x{:08X}\t{}\trequest_id={};slot={};handle={};status={};sequence_consumer_verified={};sequence_start_verified={};sequence_progress_verified={};bgm_track_identity_verified={};runtime_verified={};release_eligible={}\t{}",
        QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME,
        QUEST_WAKEUP_SEQUENCE_BYTES,
        report.request_id,
        report.application_slot,
        report.sequence_handle,
        report.playing_status,
        report.sequence_consumer_verified,
        report.sequence_start_verified,
        report.sequence_progress_verified,
        report.bgm_track_identity_verified,
        report.runtime_verified,
        report.release_eligible,
        report.image_sha256,
    )
    .unwrap();
    output
}

#[cfg(test)]
#[path = "quest_sound_sequence_runtime_tests.rs"]
mod tests;
