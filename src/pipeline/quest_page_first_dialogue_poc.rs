//! 첫 승인 QUEST 대사를 위한 scene-bound font page hook PoC.
//!
//! 원본 boot atlas는 그대로 두고, 한글 11자가 들어간 `RPG_FNT1.BTX`
//! CNX를 재배치된 `QUEST.EXE`의 resident carrier에 넣는다. OP21 record가
//! `TOWN00:0x21CC`일 때만 10개 window의 active byte를 전수 확인하고,
//! resident raw page를 VRAM에 upload하고 `DrawSync(0)`을 끝낸 뒤 page
//! state를 commit한다. 두 번째 PoC는 이 record만 한글 page를 고르고 나머지
//! 모든 OP21 record에는 원본 page를 골라 stale page를 막는다. 다음 실제
//! 장면은 원본 page 복귀 checkpoint로 쓴다. 두 exact 표본 모두 release
//! 전체의 carrier 수명을 증명하지 않으므로 명시적으로 PoC로만 만든다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::disasm::{
    QUEST_FONT_SOURCE_BYTES, QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET,
    QUEST_PAGE_CARRIER_CAPACITY_BYTES, QUEST_PAGE_CARRIER_END_RUNTIME,
    QUEST_PAGE_CARRIER_START_RUNTIME,
};
use crate::disc::VerifiedSourceImage;
use crate::isa::{VerifiedMachineCode, VerifyError};
use crate::script::TOWN00_SEQ_RUNTIME_BASE;

use super::quest_hook_poc::QUEST_HOOK_BODY_RUNTIME;
use super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;
use super::quest_poc::{
    prepare_quest_poc_assets, prepare_quest_two_dialogue_assets, PreparedQuestPocAssets,
    QuestPocMode,
};
use super::QuestSecondDialogueAssets;

#[cfg(test)]
use super::quest_hook_poc::QUEST_HOOK_RESERVED_BYTES;
#[cfg(test)]
use crate::disasm::{QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME, QUEST_FONT_UPLOAD_TARGET_RUNTIME};
#[cfg(test)]
use crate::isa::{InstructionKind as K, Reg};

mod artifact_readback;
mod expected_writes;
mod machine_code;
mod report;
mod runtime_verification;

use artifact_readback::apply_and_verify_quest_resident_pages;
use expected_writes::plan_quest_resident_pages;
#[cfg(test)]
use machine_code::{assemble_body, assemble_transition_body};
pub use report::{
    render_quest_page_first_dialogue_runtime_tsv, render_quest_page_first_dialogue_tsv,
    render_quest_page_transition_runtime_tsv, render_quest_page_transition_tsv,
    render_quest_page_two_dialogues_tsv,
};
pub use runtime_verification::{
    verify_quest_page_first_dialogue_runtime, verify_quest_page_transition_runtime,
};

pub(super) fn assemble_two_dialogue_transition_body(
) -> Result<VerifiedMachineCode, QuestPageFirstDialogueError> {
    machine_code::assemble_two_dialogue_transition_body()
}

pub(super) fn uploaded_font_page(gpu: &[u8]) -> Result<Vec<u8>, QuestPageFirstDialogueError> {
    runtime_verification::uploaded_font_page(gpu)
}

pub const QUEST_PAGE_FIRST_DIALOGUE_SCHEMA_VERSION: &str = "quest-page-first-dialogue-poc-v2";
pub const QUEST_PAGE_FIRST_DIALOGUE_RUNTIME_SCHEMA_VERSION: &str =
    "quest-page-first-dialogue-runtime-v2";
pub const QUEST_PAGE_FIRST_DIALOGUE_SOURCE_ID: &str =
    "pipeline::quest_page_first_dialogue_poc::scene_bound_body";
pub const QUEST_PAGE_FIRST_DIALOGUE_RECORD_OFFSET: u32 = 0x21CC;
pub const QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME: u32 =
    TOWN00_SEQ_RUNTIME_BASE + QUEST_PAGE_FIRST_DIALOGUE_RECORD_OFFSET;
pub const QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID: u8 = 1;
pub const QUEST_PAGE_FIRST_DIALOGUE_BODY_WORDS: usize = 45;
pub const QUEST_PAGE_FIRST_DIALOGUE_BODY_BYTES: usize = QUEST_PAGE_FIRST_DIALOGUE_BODY_WORDS * 4;

pub const QUEST_PAGE_TRANSITION_SCHEMA_VERSION: &str = "quest-page-transition-poc-v2";
pub const QUEST_PAGE_TRANSITION_RUNTIME_SCHEMA_VERSION: &str = "quest-page-transition-runtime-v2";
pub const QUEST_PAGE_TWO_DIALOGUES_SCHEMA_VERSION: &str = "quest-page-two-dialogues-poc-v1";
pub const BOX_QUEST_PAGE_TWO_DIALOGUES_IMAGE_SHA256: &str =
    "22ed402a2251a43129876bd16c23e1ed38d93eade8589d5cbf891ce4fec4bca6";
pub const QUEST_PAGE_TRANSITION_SOURCE_ID: &str =
    "pipeline::quest_page_first_dialogue_poc::two_page_transition_body";
pub const QUEST_PAGE_TWO_DIALOGUES_SOURCE_ID: &str =
    "pipeline::quest_page_first_dialogue_poc::two_dialogue_transition_body";
pub const QUEST_PAGE_SECOND_DIALOGUE_RECORD_OFFSET: u32 = 0x22DC;
pub const QUEST_PAGE_SECOND_DIALOGUE_RECORD_RUNTIME: u32 =
    TOWN00_SEQ_RUNTIME_BASE + QUEST_PAGE_SECOND_DIALOGUE_RECORD_OFFSET;
pub const QUEST_PAGE_RESTORE_PAGE_ID: u8 = 2;
pub const QUEST_PAGE_RESTORE_SOURCE_RUNTIME: u32 =
    QUEST_PAGE_CARRIER_START_RUNTIME + QUEST_FONT_SOURCE_BYTES as u32;
pub const QUEST_PAGE_TRANSITION_BODY_WORDS: usize = 53;
pub const QUEST_PAGE_TRANSITION_BODY_BYTES: usize = QUEST_PAGE_TRANSITION_BODY_WORDS * 4;
pub const QUEST_PAGE_TWO_DIALOGUES_BODY_WORDS: usize = 57;
pub const QUEST_PAGE_TWO_DIALOGUES_BODY_BYTES: usize = QUEST_PAGE_TWO_DIALOGUES_BODY_WORDS * 4;

const RPG_SEQ_PATH: &str = "/QUEST/RPG_SEQ.VDD;1";
const RPG_GRP_PATH: &str = "/QUEST/RPG_GRP.VDD;1";
const TOWN00_NAME: &str = "TOWN00.SEQ";
const ACTOR: &str = "pipeline::quest_page_first_dialogue_poc";
const QUEST_FONT_STATE_ACTIVE_OFFSET: i16 = QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET + 1;
const PSX_GPU_RAM_BYTES: usize = 1024 * 512 * 2;
const PSX_GPU_ROW_BYTES: usize = 1024 * 2;
const QUEST_FONT_VRAM_X_BYTES: usize = 576 * 2;
const QUEST_FONT_VRAM_ROW_BYTES: usize = 64 * 2;
const QUEST_FONT_VRAM_ROWS: usize = 256;

/// scene-bound page hook의 typed executable source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPageFirstDialogueSources {
    pub site: VerifiedMachineCode,
    pub body: VerifiedMachineCode,
}

/// 첫 승인 대사 page hook PoC의 정적·빌드 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPageFirstDialogueReport {
    pub schema_version: &'static str,
    pub target_record_runtime: u32,
    pub site_runtime: u32,
    pub body_runtime: u32,
    pub body_bytes: usize,
    pub body_slack_bytes: usize,
    pub page_source_runtime: u32,
    pub page_bytes: usize,
    pub relocated_quest_lba: u32,
    pub relocated_quest_bytes: u32,
    pub relocated_quest_sectors: u32,
    pub relocated_32m_lba: u32,
    pub relocated_32m_bytes: u32,
    pub town00_cnx_bytes: usize,
    pub machine_code_writes: usize,
    pub data_writes: usize,
    pub metadata_writes: usize,
    pub relocation_copy_writes: usize,
    pub touched_sectors: usize,
    pub final_expected_writes: usize,
    pub original_text_except_hook_exact: bool,
    pub resident_page_exact: bool,
    pub extension_outside_page_zero: bool,
    pub original_font_archive_unchanged: bool,
    pub original_quest_extent_unchanged: bool,
    pub extent_overlaps: usize,
    pub runtime_verified: bool,
    pub release_eligible: bool,
    pub image_sha256: String,
}

/// exact PoC image가 첫 대사 hook에서 만든 main RAM 상태.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPageFirstDialogueRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub ram_sha256: String,
    pub gpu_sha256: String,
    pub target_record_runtime: u32,
    pub page_source_runtime: u32,
    pub page_bytes: usize,
    pub page_sha256: String,
    pub uploaded_page_sha256: String,
    pub hook_site_exact: bool,
    pub hook_body_exact: bool,
    pub resident_page_exact: bool,
    pub uploaded_page_exact: bool,
    pub font_states_inactive: bool,
    pub page_state: u8,
    pub page_state_committed: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
}

#[derive(Debug)]
pub struct QuestPageFirstDialogueBuild {
    pub image: Vec<u8>,
    pub report: QuestPageFirstDialogueReport,
}

/// 첫 한글 record와 그 밖의 모든 원본 record를 분리하는 빌드 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPageTransitionReport {
    pub schema_version: &'static str,
    pub first_record_runtime: u32,
    pub site_runtime: u32,
    pub body_runtime: u32,
    pub body_bytes: usize,
    pub body_slack_bytes: usize,
    pub page_count: usize,
    pub korean_page_source_runtime: u32,
    pub original_page_source_runtime: u32,
    pub page_bytes: usize,
    pub relocated_quest_lba: u32,
    pub relocated_quest_bytes: u32,
    pub relocated_quest_sectors: u32,
    pub relocated_32m_lba: u32,
    pub relocated_32m_bytes: u32,
    pub town00_cnx_bytes: usize,
    pub machine_code_writes: usize,
    pub data_writes: usize,
    pub metadata_writes: usize,
    pub relocation_copy_writes: usize,
    pub touched_sectors: usize,
    pub final_expected_writes: usize,
    pub original_text_except_hook_exact: bool,
    pub korean_page_exact: bool,
    pub original_page_exact: bool,
    pub pages_differ: bool,
    pub extension_outside_pages_zero: bool,
    pub original_font_archive_unchanged: bool,
    pub original_quest_extent_unchanged: bool,
    pub extent_overlaps: usize,
    pub runtime_verified: bool,
    pub release_eligible: bool,
    pub image_sha256: String,
}

/// 두 page 전환 PoC의 exact 빌드.
#[derive(Debug)]
pub struct QuestPageTransitionBuild {
    pub image: Vec<u8>,
    pub report: QuestPageTransitionReport,
}

/// 같은 batch의 두 fixed-slot 대사와 공용 Korean page를 결합한 빌드 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPageTwoDialoguesReport {
    pub schema_version: &'static str,
    pub first_record_runtime: u32,
    pub second_record_runtime: u32,
    pub body_runtime: u32,
    pub body_bytes: usize,
    pub body_slack_bytes: usize,
    pub page_count: usize,
    pub korean_page_source_runtime: u32,
    pub original_page_source_runtime: u32,
    pub page_bytes: usize,
    pub korean_page_sha256: String,
    pub second_stream_sha256: String,
    pub second_glyph_mappings: usize,
    pub korean_page_glyph_mappings: usize,
    pub relocated_quest_lba: u32,
    pub relocated_quest_bytes: u32,
    pub relocated_quest_sectors: u32,
    pub relocated_32m_lba: u32,
    pub relocated_32m_bytes: u32,
    pub town00_cnx_bytes: usize,
    pub machine_code_writes: usize,
    pub data_writes: usize,
    pub metadata_writes: usize,
    pub relocation_copy_writes: usize,
    pub touched_sectors: usize,
    pub final_expected_writes: usize,
    pub original_text_except_hook_exact: bool,
    pub korean_page_exact: bool,
    pub original_page_exact: bool,
    pub pages_differ: bool,
    pub extension_outside_pages_zero: bool,
    pub original_font_archive_unchanged: bool,
    pub original_quest_extent_unchanged: bool,
    pub extent_overlaps: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
    pub image_sha256: String,
}

/// 두 대사 공용 page의 exact 비배포 PoC image.
#[derive(Debug)]
pub struct QuestPageTwoDialoguesBuild {
    pub image: Vec<u8>,
    pub report: QuestPageTwoDialoguesReport,
}

/// 두 page 전환의 exact runtime checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestPageTransitionPhase {
    Korean,
    Restored,
}

impl QuestPageTransitionPhase {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Korean => "korean",
            Self::Restored => "restored",
        }
    }

    const fn page_id(self) -> u8 {
        match self {
            Self::Korean => QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID,
            Self::Restored => QUEST_PAGE_RESTORE_PAGE_ID,
        }
    }

    const fn source_runtime(self) -> u32 {
        match self {
            Self::Korean => QUEST_PAGE_CARRIER_START_RUNTIME,
            Self::Restored => QUEST_PAGE_RESTORE_SOURCE_RUNTIME,
        }
    }
}

/// 두 page 전환의 exact RAM·VRAM 검증 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPageTransitionRuntimeReport {
    pub schema_version: &'static str,
    pub phase: QuestPageTransitionPhase,
    pub image_sha256: String,
    pub ram_sha256: String,
    pub gpu_sha256: String,
    pub expected_page_source_runtime: u32,
    pub page_bytes: usize,
    pub korean_page_sha256: String,
    pub original_page_sha256: String,
    pub uploaded_page_sha256: String,
    pub hook_site_exact: bool,
    pub hook_body_exact: bool,
    pub resident_pages_exact: bool,
    pub uploaded_page_exact: bool,
    pub font_states_inactive: bool,
    pub page_state: u8,
    pub page_state_committed: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
}

#[derive(Debug)]
pub(super) struct QuestResidentPagesBuild {
    pub(super) image: Vec<u8>,
    pub(super) report: QuestPageFirstDialogueReport,
    pub(super) original_page_exact: bool,
    pub(super) pages_differ: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QuestResidentPagesMode {
    FirstOnly,
    FirstTransition,
    TwoDialogues,
}

impl QuestResidentPagesMode {
    pub(super) const fn include_restore(self) -> bool {
        !matches!(self, Self::FirstOnly)
    }
}

#[derive(Debug)]
pub enum QuestPageFirstDialogueError {
    Message(String),
    MachineCode(VerifyError),
}

impl fmt::Display for QuestPageFirstDialogueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => f.write_str(message),
            Self::MachineCode(source) => {
                write!(f, "QUEST 첫 대사 page hook ISA 검산 실패: {source}")
            }
        }
    }
}

impl Error for QuestPageFirstDialogueError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MachineCode(source) => Some(source),
            Self::Message(_) => None,
        }
    }
}

impl From<VerifyError> for QuestPageFirstDialogueError {
    fn from(value: VerifyError) -> Self {
        Self::MachineCode(value)
    }
}

pub(super) fn error(message: impl Into<String>) -> QuestPageFirstDialogueError {
    QuestPageFirstDialogueError::Message(message.into())
}

pub(super) fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn build_quest_resident_pages_from_prepared(
    source: &VerifiedSourceImage,
    prepared: &PreparedQuestPocAssets,
    mode: QuestResidentPagesMode,
) -> Result<QuestResidentPagesBuild, QuestPageFirstDialogueError> {
    let planned = plan_quest_resident_pages(source, prepared, mode)?;
    apply_and_verify_quest_resident_pages(source, prepared, mode, planned)
}

fn build_quest_resident_pages(
    source: &VerifiedSourceImage,
    mode: QuestResidentPagesMode,
) -> Result<QuestResidentPagesBuild, QuestPageFirstDialogueError> {
    let prepared = prepare_quest_poc_assets(source, QuestPocMode::Korean)
        .map_err(|source| error(source.to_string()))?;
    build_quest_resident_pages_from_prepared(source, &prepared, mode)
}

/// 첫 승인 대사의 단일 resident page PoC를 불변 원본에서 만든다.
pub fn build_quest_page_first_dialogue(
    source: &VerifiedSourceImage,
) -> Result<QuestPageFirstDialogueBuild, QuestPageFirstDialogueError> {
    let build = build_quest_resident_pages(source, QuestResidentPagesMode::FirstOnly)?;
    Ok(QuestPageFirstDialogueBuild {
        image: build.image,
        report: build.report,
    })
}

/// 첫 한글 record와 나머지 원본 record의 page 전환 PoC를 불변 원본에서 만든다.
pub fn build_quest_page_transition(
    source: &VerifiedSourceImage,
) -> Result<QuestPageTransitionBuild, QuestPageFirstDialogueError> {
    let build = build_quest_resident_pages(source, QuestResidentPagesMode::FirstTransition)?;
    let base = build.report;
    Ok(QuestPageTransitionBuild {
        image: build.image,
        report: QuestPageTransitionReport {
            schema_version: QUEST_PAGE_TRANSITION_SCHEMA_VERSION,
            first_record_runtime: QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME,
            site_runtime: base.site_runtime,
            body_runtime: base.body_runtime,
            body_bytes: base.body_bytes,
            body_slack_bytes: base.body_slack_bytes,
            page_count: 2,
            korean_page_source_runtime: QUEST_PAGE_CARRIER_START_RUNTIME,
            original_page_source_runtime: QUEST_PAGE_RESTORE_SOURCE_RUNTIME,
            page_bytes: base.page_bytes,
            relocated_quest_lba: base.relocated_quest_lba,
            relocated_quest_bytes: base.relocated_quest_bytes,
            relocated_quest_sectors: base.relocated_quest_sectors,
            relocated_32m_lba: base.relocated_32m_lba,
            relocated_32m_bytes: base.relocated_32m_bytes,
            town00_cnx_bytes: base.town00_cnx_bytes,
            machine_code_writes: base.machine_code_writes,
            data_writes: base.data_writes,
            metadata_writes: base.metadata_writes,
            relocation_copy_writes: base.relocation_copy_writes,
            touched_sectors: base.touched_sectors,
            final_expected_writes: base.final_expected_writes,
            original_text_except_hook_exact: base.original_text_except_hook_exact,
            korean_page_exact: base.resident_page_exact,
            original_page_exact: build.original_page_exact,
            pages_differ: build.pages_differ,
            extension_outside_pages_zero: base.extension_outside_page_zero,
            original_font_archive_unchanged: base.original_font_archive_unchanged,
            original_quest_extent_unchanged: base.original_quest_extent_unchanged,
            extent_overlaps: base.extent_overlaps,
            runtime_verified: false,
            release_eligible: false,
            image_sha256: base.image_sha256,
        },
    })
}

/// 같은 batch의 첫·두 번째 대사를 공용 Korean page에 결합한 비배포 PoC를 만든다.
pub fn build_quest_page_two_dialogues(
    source: &VerifiedSourceImage,
    second: &QuestSecondDialogueAssets,
) -> Result<QuestPageTwoDialoguesBuild, QuestPageFirstDialogueError> {
    let prepared = prepare_quest_two_dialogue_assets(source, second)
        .map_err(|source| error(source.to_string()))?;
    let build = build_quest_resident_pages_from_prepared(
        source,
        &prepared,
        QuestResidentPagesMode::TwoDialogues,
    )?;
    let base = build.report;
    if base.image_sha256 != BOX_QUEST_PAGE_TWO_DIALOGUES_IMAGE_SHA256 {
        return Err(error(format!(
            "QUEST 두 대사 exact image SHA-256 drift: expected={}, actual={}",
            BOX_QUEST_PAGE_TWO_DIALOGUES_IMAGE_SHA256, base.image_sha256
        )));
    }
    Ok(QuestPageTwoDialoguesBuild {
        image: build.image,
        report: QuestPageTwoDialoguesReport {
            schema_version: QUEST_PAGE_TWO_DIALOGUES_SCHEMA_VERSION,
            first_record_runtime: QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME,
            second_record_runtime: QUEST_PAGE_SECOND_DIALOGUE_RECORD_RUNTIME,
            body_runtime: base.body_runtime,
            body_bytes: base.body_bytes,
            body_slack_bytes: base.body_slack_bytes,
            page_count: 2,
            korean_page_source_runtime: QUEST_PAGE_CARRIER_START_RUNTIME,
            original_page_source_runtime: QUEST_PAGE_RESTORE_SOURCE_RUNTIME,
            page_bytes: base.page_bytes,
            korean_page_sha256: second.plan.page_sha256.clone(),
            second_stream_sha256: second.plan.replaced_stream_sha256.clone(),
            second_glyph_mappings: second.plan.mappings.len(),
            korean_page_glyph_mappings: second.plan.batch_unique_mappings,
            relocated_quest_lba: base.relocated_quest_lba,
            relocated_quest_bytes: base.relocated_quest_bytes,
            relocated_quest_sectors: base.relocated_quest_sectors,
            relocated_32m_lba: base.relocated_32m_lba,
            relocated_32m_bytes: base.relocated_32m_bytes,
            town00_cnx_bytes: base.town00_cnx_bytes,
            machine_code_writes: base.machine_code_writes,
            data_writes: base.data_writes,
            metadata_writes: base.metadata_writes,
            relocation_copy_writes: base.relocation_copy_writes,
            touched_sectors: base.touched_sectors,
            final_expected_writes: base.final_expected_writes,
            original_text_except_hook_exact: base.original_text_except_hook_exact,
            korean_page_exact: base.resident_page_exact,
            original_page_exact: build.original_page_exact,
            pages_differ: build.pages_differ,
            extension_outside_pages_zero: base.extension_outside_page_zero,
            original_font_archive_unchanged: base.original_font_archive_unchanged,
            original_quest_extent_unchanged: base.original_quest_extent_unchanged,
            extent_overlaps: base.extent_overlaps,
            translation_approved: second.plan.translation_approved,
            layout_approved: second.plan.layout_approved,
            runtime_verified: false,
            release_eligible: false,
            image_sha256: base.image_sha256,
        },
    })
}

#[cfg(test)]
#[path = "quest_page_first_dialogue_poc_tests.rs"]
mod tests;
