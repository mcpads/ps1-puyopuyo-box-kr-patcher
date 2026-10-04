//! QUEST 게임 저장으로 만든 PS1 memory card의 영속 형식 검증.
//!
//! 화면의 저장 완료 문구나 파일 시각만으로 성공을 판정하지 않는다. exact
//! QUEST sparse glyph-cache 이미지, 저장 전 빈 카드, 저장 뒤 카드의 directory
//! frame과 첫 data block을 함께 검사한다. 실제 cold load와 OP21 재진입은
//! Emucap 실행 증거가 별도로 소유한다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

pub const QUEST_SAVE_CARD_RUNTIME_SCHEMA_VERSION: &str = "quest-save-card-runtime-v1";

const PS1_MEMORY_CARD_BYTES: usize = 128 * 1024;
const PS1_MEMORY_CARD_FRAME_BYTES: usize = 128;
const PS1_MEMORY_CARD_DIRECTORY_FRAMES: std::ops::RangeInclusive<usize> = 1..=15;
const PS1_MEMORY_CARD_DATA_BLOCK_BYTES: usize = 8 * 1024;
const PS1_MEMORY_CARD_FIRST_DATA_BLOCK_OFFSET: usize = PS1_MEMORY_CARD_DATA_BLOCK_BYTES;
const PS1_MEMORY_CARD_FREE_STATUS: u8 = 0xA0;
const PS1_MEMORY_CARD_FIRST_BLOCK_STATUS: u8 = 0x51;
const PUYO_BOX_SAVE_BYTES: u32 = 0x2000;
const PUYO_BOX_SAVE_FILENAME: &str = "BISLPSP03114_PUYOBOX";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSaveCardRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub blank_card_sha256: String,
    pub saved_card_sha256: String,
    pub card_bytes: usize,
    pub directory_block: usize,
    pub directory_status: u8,
    pub declared_save_bytes: u32,
    pub next_block: u16,
    pub filename: String,
    pub data_block_offset: usize,
    pub blank_directory_free: bool,
    pub directory_checksums_valid: bool,
    pub save_entry_exact: bool,
    pub save_header_exact: bool,
    pub persistent_card_verified: bool,
    pub cold_load_verified: bool,
    pub release_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSaveCardRuntimeError(String);

impl fmt::Display for QuestSaveCardRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSaveCardRuntimeError {}

fn fail(message: impl Into<String>) -> QuestSaveCardRuntimeError {
    QuestSaveCardRuntimeError(message.into())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn frame(card: &[u8], index: usize) -> Result<&[u8], QuestSaveCardRuntimeError> {
    let start = index
        .checked_mul(PS1_MEMORY_CARD_FRAME_BYTES)
        .ok_or_else(|| fail("PS1 memory card frame offset overflow"))?;
    let end = start + PS1_MEMORY_CARD_FRAME_BYTES;
    card.get(start..end)
        .ok_or_else(|| fail(format!("PS1 memory card frame {index}가 범위 밖임")))
}

fn frame_checksum_valid(frame: &[u8]) -> bool {
    frame.iter().fold(0_u8, |checksum, byte| checksum ^ byte) == 0
}

fn directory_filename(frame: &[u8]) -> Result<&str, QuestSaveCardRuntimeError> {
    let bytes = &frame[10..30];
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..end])
        .map_err(|_| fail("PS1 memory card directory filename이 ASCII/UTF-8이 아님"))
}

fn verify_card_contract(
    image_sha256: &str,
    blank_card: &[u8],
    saved_card: &[u8],
) -> Result<QuestSaveCardRuntimeReport, QuestSaveCardRuntimeError> {
    if !is_sha256_hex(image_sha256) {
        return Err(fail(format!(
            "QUEST runtime image SHA-256 형식 불일치: got {image_sha256}"
        )));
    }
    for (role, card) in [("blank", blank_card), ("saved", saved_card)] {
        if card.len() != PS1_MEMORY_CARD_BYTES {
            return Err(fail(format!(
                "{role} PS1 memory card 크기 불일치: expected {PS1_MEMORY_CARD_BYTES}, got {}",
                card.len()
            )));
        }
        if card.get(..2) != Some(b"MC") {
            return Err(fail(format!("{role} PS1 memory card header가 MC가 아님")));
        }
    }
    if blank_card == saved_card {
        return Err(fail("저장 전후 PS1 memory card가 byte-identical임"));
    }

    let blank_directory_free = PS1_MEMORY_CARD_DIRECTORY_FRAMES.clone().all(|index| {
        frame(blank_card, index).is_ok_and(|entry| entry[0] == PS1_MEMORY_CARD_FREE_STATUS)
    });
    if !blank_directory_free {
        return Err(fail("저장 전 PS1 memory card directory가 비어 있지 않음"));
    }

    let directory_checksums_valid = [blank_card, saved_card]
        .into_iter()
        .all(|card| (0..=15).all(|index| frame(card, index).is_ok_and(frame_checksum_valid)));
    if !directory_checksums_valid {
        return Err(fail(
            "PS1 memory card system/directory frame checksum 불일치",
        ));
    }

    let first = frame(saved_card, 1)?;
    let directory_status = first[0];
    let declared_save_bytes = u32::from_le_bytes(first[4..8].try_into().unwrap());
    let next_block = u16::from_le_bytes(first[8..10].try_into().unwrap());
    let filename = directory_filename(first)?.to_owned();
    let remaining_directory_free = (2..=15).all(|index| {
        frame(saved_card, index).is_ok_and(|entry| entry[0] == PS1_MEMORY_CARD_FREE_STATUS)
    });
    let save_entry_exact = directory_status == PS1_MEMORY_CARD_FIRST_BLOCK_STATUS
        && declared_save_bytes == PUYO_BOX_SAVE_BYTES
        && next_block == u16::MAX
        && filename == PUYO_BOX_SAVE_FILENAME
        && remaining_directory_free;
    if !save_entry_exact {
        return Err(fail(format!(
            "Puyo Puyo BOX save directory 불일치: status=0x{directory_status:02X}, bytes={declared_save_bytes}, next=0x{next_block:04X}, filename={filename}, remaining_free={remaining_directory_free}"
        )));
    }

    let save_header_exact = saved_card
        .get(PS1_MEMORY_CARD_FIRST_DATA_BLOCK_OFFSET..PS1_MEMORY_CARD_FIRST_DATA_BLOCK_OFFSET + 2)
        == Some(b"SC");
    if !save_header_exact {
        return Err(fail("Puyo Puyo BOX 첫 save data block header가 SC가 아님"));
    }

    Ok(QuestSaveCardRuntimeReport {
        schema_version: QUEST_SAVE_CARD_RUNTIME_SCHEMA_VERSION,
        image_sha256: image_sha256.to_owned(),
        blank_card_sha256: sha256(blank_card),
        saved_card_sha256: sha256(saved_card),
        card_bytes: saved_card.len(),
        directory_block: 1,
        directory_status,
        declared_save_bytes,
        next_block,
        filename,
        data_block_offset: PS1_MEMORY_CARD_FIRST_DATA_BLOCK_OFFSET,
        blank_directory_free,
        directory_checksums_valid,
        save_entry_exact,
        save_header_exact,
        persistent_card_verified: true,
        cold_load_verified: false,
        release_eligible: false,
    })
}

/// exact current image와 저장 전후 memory card를 결속해 영속 카드 형식을 검증한다.
pub fn verify_quest_save_card_runtime(
    image: &[u8],
    blank_card: &[u8],
    saved_card: &[u8],
) -> Result<QuestSaveCardRuntimeReport, QuestSaveCardRuntimeError> {
    verify_card_contract(&sha256(image), blank_card, saved_card)
}

/// QUEST 게임 저장 memory card 검증 결과를 한 행 TSV로 만든다.
pub fn render_quest_save_card_runtime_tsv(report: &QuestSaveCardRuntimeReport) -> String {
    format!(
        "schema_version\timage_sha256\tblank_card_sha256\tsaved_card_sha256\tcard_bytes\t\
         directory_block\tdirectory_status\tdeclared_save_bytes\tnext_block\tfilename\t\
         data_block_offset\tblank_directory_free\tdirectory_checksums_valid\t\
         save_entry_exact\tsave_header_exact\tpersistent_card_verified\t\
         cold_load_verified\trelease_eligible\n\
         {}\t{}\t{}\t{}\t{}\t{}\t0x{:02X}\t{}\t0x{:04X}\t{}\t0x{:X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.image_sha256,
        report.blank_card_sha256,
        report.saved_card_sha256,
        report.card_bytes,
        report.directory_block,
        report.directory_status,
        report.declared_save_bytes,
        report.next_block,
        report.filename,
        report.data_block_offset,
        report.blank_directory_free,
        report.directory_checksums_valid,
        report.save_entry_exact,
        report.save_header_exact,
        report.persistent_card_verified,
        report.cold_load_verified,
        report.release_eligible,
    )
}

#[cfg(test)]
#[path = "quest_save_card_runtime_tests.rs"]
mod tests;
