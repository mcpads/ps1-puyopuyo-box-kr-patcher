//! development full-disc의 synthetic route 소비 진입 검증.
//!
//! 자연 게임 진행 대신 body 진입 시점의 object stream pointer만 특정 검토문
//! route로 바꾼 증거를 소비한다. exact disc에서 재구성한 전체 SEQ가 RAM에
//! 올라왔는지, body entry register와 object field가 그 route를 가리키는지,
//! 표시 뒤 화면이 결속됐는지를 함께 검사한다. 이 결과는 tail·원문 유지
//! 공통 연결의 표본이지 자연 도달·번역 승인·제품 적격의 증거가 아니다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::decompress;
use crate::coords::{psx_main_ram_offset, RuntimeAssetMap, PSX_MAIN_RAM_LEN};
use crate::disasm::QUEST_HOOK_GP_RUNTIME;
use crate::disc::{DiscImage, Iso9660};
use crate::script::TOWN00_SEQ_RUNTIME_BASE;

use super::quest_development_disc::{
    QuestDevelopmentDiscBuild, QUEST_DEVELOPMENT_DISC_SCHEMA_VERSION,
};
use super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::quest_manual_stream_writer::QuestManualStreamWriterAnalysis;
use super::quest_translation_selection_routing::{
    QuestTranslationSelectionPlacement, QuestTranslationSelectionRoutingAnalysis,
};

pub const QUEST_DEVELOPMENT_ROUTE_RUNTIME_SCHEMA_VERSION: &str =
    "quest-development-route-runtime-v1";

const OBJECT_STREAM_OFFSET: u32 = 0x1C;
const OBJECT_STREAM_FIELD_END: usize = 0x20;
const PNG_WIDTH: u32 = 280;
const PNG_HEIGHT: u32 = 240;
const RPG_SEQ_PATH: &str = "/QUEST/RPG_SEQ.VDD;1";

/// pointer 개입으로 특정 route를 소비시킨 runtime 표본.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentRouteRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub sequence: String,
    pub sequence_bytes: usize,
    pub sequence_sha256: String,
    pub stream_id: String,
    pub placement: &'static str,
    pub routing_page_id: u8,
    pub record_page_id: u8,
    pub record_offset: usize,
    pub record_runtime: u32,
    pub routed_record: [u8; 4],
    pub body_offset: usize,
    pub body_runtime: u32,
    pub object_runtime: u32,
    pub object_stream_runtime: u32,
    pub entry_state_sha256: String,
    pub entry_ram_sha256: String,
    pub visible_png_sha256: String,
    pub visible_png_width: u32,
    pub visible_png_height: u32,
    pub sequence_loaded_exact: bool,
    pub route_record_exact: bool,
    pub body_entry_exact: bool,
    pub object_stream_exact: bool,
    pub visible_capture_shape_exact: bool,
    pub synthetic_pointer_intervention: bool,
    pub synthetic_route_verified: bool,
    pub natural_reach_verified: bool,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub product_eligible: bool,
}

/// 한 synthetic pointer 표본의 entry와 표시 증거.
#[derive(Debug, Clone, Copy)]
pub struct QuestDevelopmentRouteRuntimeEvidence<'a> {
    pub stream_id: &'a str,
    pub entry_state_json: &'a [u8],
    pub entry_ram: &'a [u8],
    pub visible_png: &'a [u8],
}

struct QuestDevelopmentRouteRuntimeInput<'a> {
    build: &'a QuestDevelopmentDiscBuild,
    writer: &'a QuestManualStreamWriterAnalysis,
    routing: &'a QuestTranslationSelectionRoutingAnalysis,
    hook: &'a QuestGlyphCacheHookPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentRouteRuntimeError(String);

impl fmt::Display for QuestDevelopmentRouteRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestDevelopmentRouteRuntimeError {}

fn error(message: impl Into<String>) -> QuestDevelopmentRouteRuntimeError {
    QuestDevelopmentRouteRuntimeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn json_u32(value: &Value, key: &str) -> Result<u32, QuestDevelopmentRouteRuntimeError> {
    let raw = value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("QUEST route state에 {key}가 없음")))?;
    u32::try_from(raw).map_err(|_| error(format!("QUEST route {key} 범위 이탈")))
}

fn parse_entry_state(
    raw: &[u8],
    expected_pc: u32,
) -> Result<u32, QuestDevelopmentRouteRuntimeError> {
    let state: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST route state JSON 실패: {source}")))?;
    let pc = json_u32(&state, "g0.PC")?;
    let gp = json_u32(&state, "g0.gp")?;
    let object_runtime = json_u32(&state, "g0.a0")?;
    if pc != expected_pc || gp != QUEST_HOOK_GP_RUNTIME {
        return Err(error(format!(
            "QUEST route body entry 불일치: pc=0x{pc:08X}, gp=0x{gp:08X}"
        )));
    }
    psx_main_ram_offset(object_runtime, OBJECT_STREAM_FIELD_END)
        .ok_or_else(|| error("QUEST route object가 PS1 main RAM 밖임"))?;
    Ok(object_runtime)
}

fn png_dimensions(raw: &[u8]) -> Option<(u32, u32)> {
    if raw.get(..8) != Some(b"\x89PNG\r\n\x1a\n") || raw.get(12..16) != Some(b"IHDR") {
        return None;
    }
    Some((
        u32::from_be_bytes(raw.get(16..20)?.try_into().ok()?),
        u32::from_be_bytes(raw.get(20..24)?.try_into().ok()?),
    ))
}

fn routed_target_word(record: [u8; 4]) -> usize {
    usize::from(record[2]) | (usize::from(record[3]) << 8)
}

fn route_target_matches(
    placement: QuestTranslationSelectionPlacement,
    page_id: u8,
    routed_record: [u8; 4],
    body_offset: usize,
) -> bool {
    match placement {
        QuestTranslationSelectionPlacement::InlineCapacityCandidate => {
            routed_record[1] == page_id && routed_target_word(routed_record) == 0
        }
        QuestTranslationSelectionPlacement::RelocatedTailCapacityCandidate => {
            routed_record[1] == page_id
                && body_offset.is_multiple_of(4)
                && routed_target_word(routed_record) == body_offset / 4
        }
        QuestTranslationSelectionPlacement::RetainSource => routed_target_word(routed_record) == 0,
    }
}

fn verify_quest_development_route_runtime_inner(
    input: QuestDevelopmentRouteRuntimeInput<'_>,
    evidence: QuestDevelopmentRouteRuntimeEvidence<'_>,
) -> Result<QuestDevelopmentRouteRuntimeReport, QuestDevelopmentRouteRuntimeError> {
    let QuestDevelopmentRouteRuntimeInput {
        build,
        writer,
        routing,
        hook,
    } = input;
    let QuestDevelopmentRouteRuntimeEvidence {
        stream_id,
        entry_state_json,
        entry_ram,
        visible_png,
    } = evidence;
    let disc = &build.report;
    if disc.schema_version != QUEST_DEVELOPMENT_DISC_SCHEMA_VERSION
        || digest(&build.image) != disc.disc.image_sha256
        || disc.sequence_entries != writer.sequences.len()
        || disc.selected_routes != writer.selected_routes
        || disc.retained_source_routes != writer.retained_source_routes
        || disc.inline_routes != writer.inline_routes
        || disc.relocated_routes != writer.relocated_routes
        || disc.output_plain_bytes != writer.output_bytes
        || writer.runtime_verified
        || writer.product_eligible
        || disc.runtime_verified
        || disc.product_eligible
    {
        return Err(error("QUEST synthetic route 정적 입력 계약 drift"));
    }
    if entry_ram.len() != PSX_MAIN_RAM_LEN {
        return Err(error(format!(
            "QUEST route RAM 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
            entry_ram.len()
        )));
    }

    let route = routing
        .routes
        .iter()
        .find(|route| route.stream_id == stream_id)
        .ok_or_else(|| error(format!("QUEST route stream ID가 없음: {stream_id}")))?;
    let sequence = writer
        .sequences
        .iter()
        .find(|sequence| sequence.sequence == route.sequence)
        .ok_or_else(|| {
            error(format!(
                "{}: QUEST manual writer SEQ가 없음",
                route.sequence
            ))
        })?;
    let image = DiscImage::from_bytes(build.image.clone())
        .map_err(|source| error(format!("QUEST route disc 파싱 실패: {source}")))?;
    let iso = Iso9660::parse(&image)
        .map_err(|source| error(format!("QUEST route ISO 파싱 실패: {source}")))?;
    let vdd_raw = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|source| error(format!("QUEST route VDD 읽기 실패: {source}")))?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|source| error(format!("QUEST route VDD 파싱 실패: {source}")))?;
    let entry = archive
        .find(&route.sequence)
        .ok_or_else(|| error(format!("{}: QUEST route disc SEQ가 없음", route.sequence)))?;
    let disc_sequence = decompress(archive.read_file(entry))
        .map_err(|source| error(format!("{}: CNX 해제 실패: {source}", route.sequence)))?;
    if disc_sequence != sequence.output {
        return Err(error(format!(
            "{}: QUEST manual writer와 exact disc SEQ 불일치",
            route.sequence
        )));
    }
    let sequence_len = u32::try_from(sequence.output.len())
        .map_err(|_| error("QUEST route SEQ 길이 범위 이탈"))?;
    let map = RuntimeAssetMap::new(TOWN00_SEQ_RUNTIME_BASE, sequence_len)
        .ok_or_else(|| error("QUEST route SEQ runtime map overflow"))?;
    let sequence_start = psx_main_ram_offset(TOWN00_SEQ_RUNTIME_BASE, sequence.output.len())
        .ok_or_else(|| error("QUEST route SEQ RAM 범위 이탈"))?;
    let sequence_end = sequence_start
        .checked_add(sequence.output.len())
        .ok_or_else(|| error("QUEST route SEQ RAM end overflow"))?;
    let loaded_sequence = entry_ram
        .get(sequence_start..sequence_end)
        .ok_or_else(|| error("QUEST route SEQ RAM dump 범위 이탈"))?;
    if loaded_sequence != sequence.output {
        return Err(error(format!(
            "{}: QUEST manual writer SEQ runtime readback 불일치",
            route.sequence
        )));
    }

    let record_runtime = map
        .runtime_addr(route.record_offset)
        .ok_or_else(|| error(format!("{}: record runtime 범위 이탈", route.stream_id)))?;
    let record = entry_ram
        .get(
            sequence_start + route.record_offset
                ..sequence_start + route.record_offset + route.routed_record.len(),
        )
        .ok_or_else(|| error(format!("{}: record RAM 범위 이탈", route.stream_id)))?;
    if record != route.routed_record {
        return Err(error(format!(
            "{}: routed record runtime readback 불일치",
            route.stream_id
        )));
    }

    let body_offset = match route.placement {
        QuestTranslationSelectionPlacement::InlineCapacityCandidate => route
            .record_offset
            .checked_add(4)
            .ok_or_else(|| error(format!("{}: inline body offset overflow", route.stream_id)))?,
        QuestTranslationSelectionPlacement::RelocatedTailCapacityCandidate => route
            .relocated_stream_offset
            .ok_or_else(|| error(format!("{}: relocated offset 누락", route.stream_id)))?,
        QuestTranslationSelectionPlacement::RetainSource => {
            if route.routed_record != route.protected_record {
                return Err(error(format!(
                    "{}: 원문 유지 route 계약 drift",
                    route.stream_id
                )));
            }
            route
                .record_offset
                .checked_add(4)
                .ok_or_else(|| error(format!("{}: 원문 body offset overflow", route.stream_id)))?
        }
    };
    if !route_target_matches(
        route.placement,
        route.page_id,
        route.routed_record,
        body_offset,
    ) {
        return Err(error(format!(
            "{}: placement별 routed target/body offset 불일치",
            route.stream_id
        )));
    }
    let body_runtime = map
        .runtime_addr(body_offset)
        .ok_or_else(|| error(format!("{}: body runtime 범위 이탈", route.stream_id)))?;

    let object_runtime = parse_entry_state(entry_state_json, hook.body_runtime)?;
    let stream_field_runtime = object_runtime
        .checked_add(OBJECT_STREAM_OFFSET)
        .ok_or_else(|| error("QUEST route object stream field overflow"))?;
    let stream_field_offset = psx_main_ram_offset(stream_field_runtime, 4)
        .ok_or_else(|| error("QUEST route object stream field 범위 이탈"))?;
    let object_stream_runtime = u32::from_le_bytes(
        entry_ram[stream_field_offset..stream_field_offset + 4]
            .try_into()
            .expect("u32 RAM 범위를 확인함"),
    );
    if object_stream_runtime != record_runtime {
        return Err(error(format!(
            "{}: object stream pointer 불일치: expected=0x{record_runtime:08X}, actual=0x{object_stream_runtime:08X}",
            route.stream_id
        )));
    }

    let (visible_png_width, visible_png_height) = png_dimensions(visible_png)
        .ok_or_else(|| error("QUEST route visible PNG header 불일치"))?;
    if visible_png_width != PNG_WIDTH || visible_png_height != PNG_HEIGHT {
        return Err(error(format!(
            "QUEST route visible PNG 크기 불일치: expected={PNG_WIDTH}x{PNG_HEIGHT}, actual={visible_png_width}x{visible_png_height}"
        )));
    }

    Ok(QuestDevelopmentRouteRuntimeReport {
        schema_version: QUEST_DEVELOPMENT_ROUTE_RUNTIME_SCHEMA_VERSION,
        image_sha256: disc.disc.image_sha256.clone(),
        sequence: route.sequence.clone(),
        sequence_bytes: sequence.output.len(),
        sequence_sha256: digest(loaded_sequence),
        stream_id: route.stream_id.clone(),
        placement: route.placement.label(),
        routing_page_id: route.page_id,
        record_page_id: route.routed_record[1],
        record_offset: route.record_offset,
        record_runtime,
        routed_record: route.routed_record,
        body_offset,
        body_runtime,
        object_runtime,
        object_stream_runtime,
        entry_state_sha256: digest(entry_state_json),
        entry_ram_sha256: digest(entry_ram),
        visible_png_sha256: digest(visible_png),
        visible_png_width,
        visible_png_height,
        sequence_loaded_exact: true,
        route_record_exact: true,
        body_entry_exact: true,
        object_stream_exact: true,
        visible_capture_shape_exact: true,
        synthetic_pointer_intervention: true,
        synthetic_route_verified: true,
        natural_reach_verified: false,
        translation_approved: false,
        layout_approved: false,
        product_eligible: false,
    })
}

/// development exact build와 synthetic pointer 개입의 route 진입을 결속한다.
pub fn verify_quest_development_synthetic_route_runtime(
    build: &QuestDevelopmentDiscBuild,
    writer: &QuestManualStreamWriterAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    hook: &QuestGlyphCacheHookPlan,
    evidence: QuestDevelopmentRouteRuntimeEvidence<'_>,
) -> Result<QuestDevelopmentRouteRuntimeReport, QuestDevelopmentRouteRuntimeError> {
    verify_quest_development_route_runtime_inner(
        QuestDevelopmentRouteRuntimeInput {
            build,
            writer,
            routing,
            hook,
        },
        evidence,
    )
}

/// synthetic route 표본을 한 행 TSV로 만든다.
pub fn render_quest_development_route_runtime_tsv(
    report: &QuestDevelopmentRouteRuntimeReport,
) -> String {
    let mut out = String::from(
        "schema_version\timage_sha256\tsequence\tsequence_bytes\tsequence_sha256\tstream_id\t\
         placement\trouting_page_id\trecord_page_id\trecord_offset\trecord_runtime\trouted_record\tbody_offset\t\
         body_runtime\tobject_runtime\tobject_stream_runtime\tentry_state_sha256\t\
         entry_ram_sha256\tvisible_png_sha256\tvisible_png_width\tvisible_png_height\t\
         sequence_loaded_exact\troute_record_exact\tbody_entry_exact\tobject_stream_exact\t\
         visible_capture_shape_exact\tsynthetic_pointer_intervention\tsynthetic_route_verified\t\
         natural_reach_verified\ttranslation_approved\tlayout_approved\tproduct_eligible\n",
    );
    let record = report
        .routed_record
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    writeln!(
        out,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:X}\t0x{:08X}\t{}\t0x{:X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        report.schema_version,
        report.image_sha256,
        report.sequence,
        report.sequence_bytes,
        report.sequence_sha256,
        report.stream_id,
        report.placement,
        report.routing_page_id,
        report.record_page_id,
        report.record_offset,
        report.record_runtime,
        record,
        report.body_offset,
        report.body_runtime,
        report.object_runtime,
        report.object_stream_runtime,
        report.entry_state_sha256,
        report.entry_ram_sha256,
        report.visible_png_sha256,
        report.visible_png_width,
        report.visible_png_height,
        report.sequence_loaded_exact,
        report.route_record_exact,
        report.body_entry_exact,
        report.object_stream_exact,
        report.visible_capture_shape_exact,
        report.synthetic_pointer_intervention,
        report.synthetic_route_verified,
        report.natural_reach_verified,
        report.translation_approved,
        report.layout_approved,
        report.product_eligible,
    )
    .expect("String write는 실패하지 않음");
    out
}

#[cfg(test)]
#[path = "quest_development_route_runtime_tests.rs"]
mod tests;
