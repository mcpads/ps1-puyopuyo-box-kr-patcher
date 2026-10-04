//! QUEST `RPG_SEQ.VDD`의 type-1 등록과 resource ID 기준점.
//!
//! descriptor root의 첫 36바이트는 archive 자체가 소유하고, 공용 loader는
//! 다음 36바이트부터 VDD entry descriptor를 순서대로 채운다. 따라서 VDD
//! entry index에 archive descriptor 한 행을 더한 값이 SEQ resource ID다.
//! 개별 ID의 실행 선택은 이 정적 등록 검증의 주장이 아니다.

use crate::coords::ExeMap;

use super::quest_seq::QUEST_SEQ_DESCRIPTOR_ROOT_RUNTIME;
use super::vdd_registration::{
    verify_vdd_registration_source, VddRegistrationError, VddRegistrationFamily,
    VddRegistrationSourceSpec, VDD_DESCRIPTOR_STRIDE,
};

pub const QUEST_SEQ_RESOURCE_TYPE: u8 = 1;
pub const QUEST_SEQ_REGISTRATION_RECORD_RUNTIME: u32 = 0x8008_9330;
pub const QUEST_SEQ_REGISTRATION_HANDLER_RUNTIME: u32 = 0x8002_3114;
pub const QUEST_SEQ_ARCHIVE_LOADER_RUNTIME: u32 = 0x8001_8E00;
pub const QUEST_SEQ_RESOURCE_TABLE_RUNTIME: u32 = 0x800D_B328;
pub const QUEST_SEQ_ARCHIVE_NAME_RUNTIME: u32 = 0x8009_09FC;
pub const QUEST_SEQ_PREFIX_DESCRIPTOR_ROWS: usize = 1;

const QUEST_SEQ_REGISTRATION_SPEC: VddRegistrationSourceSpec = VddRegistrationSourceSpec {
    exe: "QUEST.EXE",
    resource_type: QUEST_SEQ_RESOURCE_TYPE,
    archive: "RPG_SEQ.VDD",
    registration_record_runtime: QUEST_SEQ_REGISTRATION_RECORD_RUNTIME,
    registration_handler_runtime: QUEST_SEQ_REGISTRATION_HANDLER_RUNTIME,
    loader_runtime: QUEST_SEQ_ARCHIVE_LOADER_RUNTIME,
    table_root_runtime: QUEST_SEQ_RESOURCE_TABLE_RUNTIME,
    archive_descriptor_runtime: QUEST_SEQ_DESCRIPTOR_ROOT_RUNTIME,
    archive_name_runtime: QUEST_SEQ_ARCHIVE_NAME_RUNTIME,
    family: VddRegistrationFamily::Shared,
};

/// 지원 원본에서 검증한 `RPG_SEQ.VDD` resource ID 환산.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqRegistrationSummary {
    pub descriptor_root_runtime: u32,
    pub descriptor_stride: u32,
    pub prefix_descriptor_rows: usize,
    pub archive_loader_runtime: u32,
}

/// QUEST type-1 등록과 VDD entry 앞 archive descriptor 한 행을 검증한다.
pub fn verify_box_quest_seq_registration(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqRegistrationSummary, VddRegistrationError> {
    verify_vdd_registration_source(map, text, QUEST_SEQ_REGISTRATION_SPEC)?;
    Ok(QuestSeqRegistrationSummary {
        descriptor_root_runtime: QUEST_SEQ_DESCRIPTOR_ROOT_RUNTIME,
        descriptor_stride: VDD_DESCRIPTOR_STRIDE,
        prefix_descriptor_rows: QUEST_SEQ_PREFIX_DESCRIPTOR_ROWS,
        archive_loader_runtime: QUEST_SEQ_ARCHIVE_LOADER_RUNTIME,
    })
}
