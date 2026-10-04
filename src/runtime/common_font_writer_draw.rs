//! `COM_FONT.BTX` 일본어 cell의 object writer와 primitive draw 결속.
//!
//! 정적 분석이 찾은 literal cell writer의 실제 실행 event를 원본 명령과
//! 대조하고, 같은 object의 종단 RAM cell/resource와 같은 관측 구간의
//! short renderer primitive 진입을 하나의 증거로 연결한다. 화면 PNG는
//! draw 관측에 귀속되지만 최종 framebuffer pixel의 소유권은 승인하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use serde_json::Value;

use crate::coords::{psx_main_ram_offset, ExeMap, PSX_MAIN_RAM_LEN};
use crate::disasm::{CommonFontCellSourceAnalysis, CommonFontDrawSite};
use crate::isa::{decode, InstructionKind as K};

use super::common_font_draw::{digest, event_register, event_u32};
use super::CommonFontDrawReport;

pub const COMMON_FONT_WRITER_DRAW_SCHEMA_VERSION: &str = "common-font-writer-draw-runtime-v1";

/// 실제 실행된 writer 하나와 primitive까지 도달한 object의 결속 결과.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommonFontWriterDrawRow {
    pub surface_id: &'static str,
    pub source_text: &'static str,
    pub cell: u16,
    pub resource: u16,
    pub writer_runtime: u32,
    pub resource_store_runtimes: Vec<u32>,
    pub object_runtime: u32,
    pub entry_reached_statically: bool,
}

/// writer event와 draw event를 같은 RAM object 집합으로 결속한 보고서.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontWriterDrawReport {
    pub schema_version: &'static str,
    pub exe: &'static str,
    pub archive: &'static str,
    pub ram_sha256: String,
    pub writer_event_sha256: String,
    pub draw_event_sha256: String,
    pub screen_sha256: String,
    pub writer_event_count: usize,
    pub observed_writer_event_count: usize,
    pub candidate_writer_sites: usize,
    pub observed_writer_sites: usize,
    pub writer_object_count: usize,
    pub draw_object_count: usize,
    pub draw_lookup_hits: usize,
    pub draw_primitive_hits: usize,
    pub draw_clipped_hits: usize,
    pub terminal_object_fields_verified: bool,
    pub writer_draw_object_sets_equal: bool,
    pub final_pixel_attribution_verified: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
    pub rows: Vec<CommonFontWriterDrawRow>,
}

/// 지원 원본, 정적 source 분석과 같은 runtime 관측을 결속할 입력.
pub struct CommonFontWriterDrawObservationInput<'a> {
    pub map: &'a ExeMap,
    pub exe_text: &'a [u8],
    pub site: &'a CommonFontDrawSite,
    pub cell_sources: &'a CommonFontCellSourceAnalysis,
    pub draw_report: &'a CommonFontDrawReport,
    pub ram: &'a [u8],
    pub writer_events_json: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontWriterDrawError(String);

impl fmt::Display for CommonFontWriterDrawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for CommonFontWriterDrawError {}

fn error(message: impl Into<String>) -> CommonFontWriterDrawError {
    CommonFontWriterDrawError(message.into())
}

fn read_object_u16(
    ram: &[u8],
    object_runtime: u32,
    field_offset: i16,
    role: &str,
) -> Result<u16, CommonFontWriterDrawError> {
    let field_offset = u32::try_from(field_offset)
        .map_err(|_| error(format!("COM_FONT {role} field offset이 음수임")))?;
    let runtime = object_runtime
        .checked_add(field_offset)
        .ok_or_else(|| error(format!("COM_FONT {role} 주소 overflow")))?;
    let offset = psx_main_ram_offset(runtime, 2).ok_or_else(|| {
        error(format!(
            "COM_FONT {role}가 main RAM 범위 밖임: 0x{runtime:08X}"
        ))
    })?;
    Ok(u16::from_le_bytes([ram[offset], ram[offset + 1]]))
}

/// 정적 writer, 실행 event, 종단 object field와 primitive object를 결속한다.
pub fn inspect_common_font_writer_draw(
    input: CommonFontWriterDrawObservationInput<'_>,
) -> Result<CommonFontWriterDrawReport, CommonFontWriterDrawError> {
    if input.map.name != input.site.exe
        || input.draw_report.exe != input.site.exe
        || input.draw_report.archive != input.site.archive
    {
        return Err(error(format!(
            "COM_FONT writer/draw EXE·archive 불일치: map={}, site={}/{}, draw={}/{}",
            input.map.name,
            input.site.exe,
            input.site.archive,
            input.draw_report.exe,
            input.draw_report.archive,
        )));
    }
    if input.exe_text.len() != input.map.text_len as usize {
        return Err(error(format!(
            "{} COM_FONT writer source 길이 불일치: expected={}, actual={}",
            input.map.name,
            input.map.text_len,
            input.exe_text.len()
        )));
    }
    if input.ram.len() != PSX_MAIN_RAM_LEN {
        return Err(error(format!(
            "{} COM_FONT writer RAM 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
            input.map.name,
            input.ram.len()
        )));
    }
    let ram_sha256 = digest(input.ram);
    if ram_sha256 != input.draw_report.ram_sha256 {
        return Err(error("COM_FONT writer와 draw report의 RAM digest가 다름"));
    }
    if !input.draw_report.runtime_verified {
        return Err(error("COM_FONT draw report가 runtime 검증 상태가 아님"));
    }

    type SurfaceCell = (&'static str, u16);
    let mut draw_objects = BTreeMap::<SurfaceCell, BTreeSet<u32>>::new();
    for row in &input.draw_report.rows {
        if row.primitive_hits == 0 || row.japanese_surface_ids.is_empty() {
            continue;
        }
        if row.clipped_hits != 0 {
            return Err(error(format!(
                "COM_FONT 일본어 cell {}에 clipped lookup이 있어 writer/draw 전수 결속 불가",
                row.cell.index
            )));
        }
        let cell = u16::try_from(row.cell.index)
            .map_err(|_| error("COM_FONT draw cell index가 u16 범위 밖임"))?;
        if row.primitive_object_runtimes.is_empty() {
            return Err(error(format!(
                "COM_FONT 일본어 cell {cell}의 primitive object가 없음"
            )));
        }
        for &surface_id in &row.japanese_surface_ids {
            draw_objects
                .entry((surface_id, cell))
                .or_default()
                .extend(row.primitive_object_runtimes.iter().copied());
        }
    }
    if draw_objects.is_empty() {
        return Err(error("COM_FONT 일본어 primitive draw가 없음"));
    }

    let mut writer_sources = BTreeMap::<u32, Vec<_>>::new();
    for row in input.cell_sources.rows.iter().filter(|row| {
        row.exe == input.site.exe
            && row.archive == input.site.archive
            && row
                .matched_cell
                .is_some_and(|cell| draw_objects.contains_key(&(row.surface_id, cell)))
    }) {
        let writer_runtime = row
            .cell_store_runtime
            .ok_or_else(|| error("COM_FONT matched cell에 writer runtime이 없음"))?;
        writer_sources.entry(writer_runtime).or_default().push(row);
    }
    for &(surface_id, cell) in draw_objects.keys() {
        if !writer_sources
            .values()
            .flatten()
            .any(|row| row.surface_id == surface_id && row.matched_cell == Some(cell))
        {
            return Err(error(format!(
                "COM_FONT {surface_id} cell {cell}의 정적 writer가 없음"
            )));
        }
    }

    let document: Value = serde_json::from_slice(input.writer_events_json)
        .map_err(|source| error(format!("COM_FONT writer event JSON 해석 실패: {source}")))?;
    let dropped = document
        .get("dropped")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("COM_FONT writer event JSON에 dropped가 없음"))?;
    if dropped != 0 {
        return Err(error(format!("COM_FONT writer event dropped={dropped}")));
    }
    let events = document
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| error("COM_FONT writer event JSON에 events가 없음"))?;

    let mut observed_writer_event_count = 0usize;
    let mut observed_writer_sites = BTreeSet::new();
    let mut writer_objects = BTreeMap::<SurfaceCell, BTreeSet<u32>>::new();
    let mut result_rows = BTreeSet::new();

    for event in events {
        let pc =
            event_u32(event, &["pc"], "writer pc").map_err(|source| error(source.to_string()))?;
        let Some(sources) = writer_sources.get(&pc) else {
            continue;
        };
        observed_writer_event_count += 1;
        observed_writer_sites.insert(pc);

        let source_offset = input
            .map
            .text_offset(pc)
            .ok_or_else(|| error(format!("COM_FONT writer가 EXE text 밖임: 0x{pc:08X}")))?;
        let source_word = input
            .exe_text
            .get(source_offset..source_offset + 4)
            .and_then(|raw| raw.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or_else(|| error(format!("COM_FONT writer source가 잘림: 0x{pc:08X}")))?;
        let instruction = decode(pc, source_word);
        let K::Sh { rt, base, offset } = instruction.kind else {
            return Err(error(format!(
                "COM_FONT writer 원본 명령이 sh가 아님: 0x{pc:08X}"
            )));
        };
        if offset != input.site.cell_field_offset {
            return Err(error(format!(
                "COM_FONT writer cell field 불일치: 0x{pc:08X} offset=0x{offset:04X}"
            )));
        }
        let cell = u16::try_from(
            event_register(event, rt, "writer cell").map_err(|source| error(source.to_string()))?,
        )
        .map_err(|_| error(format!("COM_FONT writer cell이 u16 범위 밖임: 0x{pc:08X}")))?;
        let object_runtime = event_register(event, base, "writer object")
            .map_err(|source| error(source.to_string()))?;
        let matching_sources = sources
            .iter()
            .copied()
            .filter(|source| source.matched_cell == Some(cell))
            .collect::<Vec<_>>();
        if matching_sources.is_empty() {
            return Err(error(format!(
                "COM_FONT writer event cell이 정적 source와 다름: 0x{pc:08X} cell={cell}"
            )));
        }

        let terminal_cell = read_object_u16(
            input.ram,
            object_runtime,
            input.site.cell_field_offset,
            "object cell",
        )?;
        let terminal_resource = read_object_u16(
            input.ram,
            object_runtime,
            input.site.resource_field_offset,
            "object resource",
        )?;
        if terminal_cell != cell {
            return Err(error(format!(
                "COM_FONT object 종단 cell 불일치: object=0x{object_runtime:08X}, event={cell}, RAM={terminal_cell}"
            )));
        }

        for source in matching_sources {
            if terminal_resource != source.common_font_resource_id {
                return Err(error(format!(
                    "COM_FONT object 종단 resource 불일치: object=0x{object_runtime:08X}, expected={}, RAM={terminal_resource}",
                    source.common_font_resource_id
                )));
            }
            let target = (source.surface_id, cell);
            let drawn = draw_objects
                .get(&target)
                .expect("정적 source는 draw target에 한정했다");
            if !drawn.contains(&object_runtime) {
                return Err(error(format!(
                    "COM_FONT writer object가 primitive draw 집합에 없음: surface={}, cell={}, object=0x{object_runtime:08X}",
                    source.surface_id, cell
                )));
            }
            writer_objects
                .entry(target)
                .or_default()
                .insert(object_runtime);
            result_rows.insert(CommonFontWriterDrawRow {
                surface_id: source.surface_id,
                source_text: source.source_text,
                cell,
                resource: source.common_font_resource_id,
                writer_runtime: pc,
                resource_store_runtimes: source.resource_store_runtimes.clone(),
                object_runtime,
                entry_reached_statically: source.entry_reached,
            });
        }
    }
    if observed_writer_event_count == 0 {
        return Err(error("COM_FONT 대상 writer event 관측이 없음"));
    }

    for (&(surface_id, cell), drawn) in &draw_objects {
        let written = writer_objects.get(&(surface_id, cell)).ok_or_else(|| {
            error(format!(
                "COM_FONT {surface_id} cell {cell}의 writer object가 없음"
            ))
        })?;
        if written != drawn {
            return Err(error(format!(
                "COM_FONT {surface_id} cell {cell} writer/draw object 집합 불일치: writer={written:?}, draw={drawn:?}"
            )));
        }
    }

    let all_writer_objects = writer_objects
        .values()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    let all_draw_objects = draw_objects
        .values()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    if all_writer_objects != all_draw_objects {
        return Err(error("COM_FONT 전체 writer/draw object 집합 불일치"));
    }

    Ok(CommonFontWriterDrawReport {
        schema_version: COMMON_FONT_WRITER_DRAW_SCHEMA_VERSION,
        exe: input.site.exe,
        archive: input.site.archive,
        ram_sha256,
        writer_event_sha256: digest(input.writer_events_json),
        draw_event_sha256: input.draw_report.event_sha256.clone(),
        screen_sha256: input.draw_report.screen_sha256.clone(),
        writer_event_count: events.len(),
        observed_writer_event_count,
        candidate_writer_sites: writer_sources.len(),
        observed_writer_sites: observed_writer_sites.len(),
        writer_object_count: all_writer_objects.len(),
        draw_object_count: all_draw_objects.len(),
        draw_lookup_hits: input.draw_report.lookup_hits,
        draw_primitive_hits: input.draw_report.primitive_hits,
        draw_clipped_hits: input.draw_report.clipped_hits,
        terminal_object_fields_verified: true,
        writer_draw_object_sets_equal: true,
        final_pixel_attribution_verified: input.draw_report.final_pixel_attribution_verified,
        runtime_verified: true,
        release_eligible: false,
        rows: result_rows.into_iter().collect(),
    })
}

/// 검증된 writer→object→primitive 결속을 결정적 TSV로 만든다.
pub fn render_common_font_writer_draw_tsv(report: &CommonFontWriterDrawReport) -> String {
    let mut output = String::from(
        "schema_version\texe\tarchive\tram_sha256\twriter_event_sha256\tdraw_event_sha256\t\
         screen_sha256\twriter_event_count\tobserved_writer_event_count\t\
         candidate_writer_sites\tobserved_writer_sites\twriter_object_count\t\
         draw_object_count\tdraw_lookup_hits\tdraw_primitive_hits\tdraw_clipped_hits\t\
         surface_id\tsource_text\tcell\tresource\twriter_runtime\t\
         resource_store_runtimes\tobject_runtime\tentry_reached_statically\t\
         terminal_object_fields_verified\twriter_draw_object_sets_equal\t\
         final_pixel_attribution_verified\truntime_verified\trelease_eligible\n",
    );
    for row in &report.rows {
        let resource_store_runtimes = row
            .resource_store_runtimes
            .iter()
            .map(|runtime| format!("0x{runtime:08X}"))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:08X}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}",
            report.schema_version,
            report.exe,
            report.archive,
            report.ram_sha256,
            report.writer_event_sha256,
            report.draw_event_sha256,
            report.screen_sha256,
            report.writer_event_count,
            report.observed_writer_event_count,
            report.candidate_writer_sites,
            report.observed_writer_sites,
            report.writer_object_count,
            report.draw_object_count,
            report.draw_lookup_hits,
            report.draw_primitive_hits,
            report.draw_clipped_hits,
            row.surface_id,
            row.source_text.replace('\n', "\\n"),
            row.cell,
            row.resource,
            row.writer_runtime,
            resource_store_runtimes,
            row.object_runtime,
            row.entry_reached_statically,
            report.terminal_object_fields_verified,
            report.writer_draw_object_sets_equal,
            report.final_pixel_attribution_verified,
            report.runtime_verified,
            report.release_eligible,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

#[cfg(test)]
#[path = "common_font_writer_draw_tests.rs"]
mod common_font_writer_draw_tests;
