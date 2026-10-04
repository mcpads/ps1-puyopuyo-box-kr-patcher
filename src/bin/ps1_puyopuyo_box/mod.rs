//! `kr-patch` 명령행 구현.
//!
//! 진입점은 이 모듈만 호출하며, 명령 구현은 역할별 하위 모듈이 소유한다.

mod analysis_inputs;
mod analysis_reports;
mod btx_family_review_server;
mod btx_reports;
mod btx_review_layout_inputs;
mod build_commands;
mod build_dispatch;
mod common_font_runtime_reports;
mod current_development_outputs;
mod development_disc_inputs;
mod disc_commands;
mod dispatch;
mod local_review_http;
mod output_writers;
mod quest_analysis_inputs;
mod quest_audio_runtime_commands;
mod quest_carrier_runtime_command;
mod quest_development_disc_runtime_commands;
mod quest_executable_runtime_command;
mod quest_font_page_reports;
mod quest_font_runtime_commands;
mod quest_glyph_cache_bank_runtime_command;
mod quest_glyph_cache_collision_runtime_commands;
mod quest_glyph_cache_outputs;
mod quest_glyph_cache_runtime_commands;
mod quest_hook_space_report;
mod quest_item_container_contact_runtime_report;
mod quest_manual_composition_development;
mod quest_map_collision_runtime_input;
mod quest_map_collision_runtime_report;
mod quest_map_route_runtime_report;
mod quest_page_runtime_commands;
mod quest_review_server;
mod quest_save_runtime_command;
mod quest_text_runtime_report;
mod quest_text_source_reports;
mod quest_translation_approval_reports;
mod quest_translation_glyph_cache_reports;
mod quest_translation_review_reports;
mod quest_vram_bank_ownership_runtime_command;
mod report_dispatch;
mod review_dispatch;
mod runtime_reports;
mod source_audit;
mod translation_reports;
mod usage;
mod verify_dispatch;

use build_dispatch::dispatch_build;
use report_dispatch::dispatch_report;
use review_dispatch::dispatch_review;
use usage::usage;
use verify_dispatch::dispatch_verify;

use std::process::ExitCode;

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::btx::PixelFormat;
use ps1_puyopuyo_box::compress::decompress;
use ps1_puyopuyo_box::coords::ExeMap;
use ps1_puyopuyo_box::disc::Iso9660;
use ps1_puyopuyo_box::font::{QuestCodeTable, QUEST_CODE_TABLE_BYTES, QUEST_CODE_TABLE_RUNTIME};

const BOX_ISO_ENTRIES: usize = 53;
const BOX_ROUNDTRIP_VDDS: usize = 18;
const BOX_GRP_VDDS: usize = 11;
const RPG_SEQ_PATH: &str = "/QUEST/RPG_SEQ.VDD;1";
const RPG_GRP_PATH: &str = "/QUEST/RPG_GRP.VDD;1";
const RPG_FONT_NAME: &str = "RPG_FNT1.BTX";
const RPG_MAP_PATH: &str = "/QUEST/RPG_MAP.VDD;1";
const PP2_GRP_PATH: &str = "/PUYO2/PP2_GRP.VDD;1";
const DEMO_FONT_NAME: &str = "DEMOFONT.BTX";
const TOWN00_NAME: &str = "TOWN00.SEQ";
const QUEST_SEQ_INTERPRETER_CALLBACK_FIELD_OFFSET: u32 = 0x18;
#[derive(Debug, Default)]

struct AuditTotals {
    archives: usize,
    entries: usize,
    cnx: usize,
    raw: usize,
    cnx_self_roundtrip: usize,
    cnx_original_bytes: usize,
    cnx_not_larger: usize,
    original_cnx_bytes: u64,
    recompressed_cnx_bytes: u64,
    largest_growth: i64,
}

#[derive(Debug)]
struct BoxExecutable {
    map: &'static ExeMap,
    text: Vec<u8>,
}

#[derive(Debug)]
struct PendingBtxRuntimePayload {
    finding_id: String,
    exe: &'static str,
    archive: &'static str,
    asset_name: String,
    format: PixelFormat,
    raw: Vec<u8>,
    expected_sha256: &'static str,
    descriptor_entry_runtime: u32,
    original_source_descriptor_identity: Vec<u8>,
    working_descriptor_runtime: u32,
    selection_code_runtime: u32,
    selection_code: Vec<u8>,
}

fn quest_code_table_from_executable(
    map: &'static ExeMap,
    text: &[u8],
) -> Result<QuestCodeTable, String> {
    if map.name != "QUEST.EXE" {
        return Err(format!(
            "QUEST 코드표에 다른 EXE 사상을 사용함: {}",
            map.name
        ));
    }
    let offset = QUEST_CODE_TABLE_RUNTIME
        .checked_sub(map.load_addr)
        .ok_or_else(|| "QUEST 코드표가 EXE 적재 주소보다 작음".to_string())?
        as usize;
    let end = offset
        .checked_add(QUEST_CODE_TABLE_BYTES)
        .ok_or_else(|| "QUEST 코드표 범위 overflow".to_string())?;
    let raw = text
        .get(offset..end)
        .ok_or_else(|| "QUEST 코드표가 EXE text 범위 밖임".to_string())?;
    QuestCodeTable::parse(raw).map_err(|error| error.to_string())
}

fn read_quest_original_font(iso: &Iso9660<'_>) -> Result<Vec<u8>, String> {
    let vdd = iso
        .read_file(RPG_GRP_PATH)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&vdd).map_err(|error| error.to_string())?;
    let entry = archive
        .find(RPG_FONT_NAME)
        .ok_or_else(|| format!("{RPG_GRP_PATH}:{RPG_FONT_NAME}가 없음"))?;
    decompress(archive.read_file(entry)).map_err(|error| error.to_string())
}

pub(crate) fn exit_code() -> ExitCode {
    match dispatch::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
