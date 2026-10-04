//! `COM_FONT.BTX` short renderer의 lookup과 primitive 진입 관측.
//!
//! 같은 guest frame에서 기록한 두 breakpoint를 순서대로 대조해 cell table
//! lookup과 clipping 통과 뒤 primitive 할당 호출 진입을 분리한다. 9개 EXE의
//! 주소와 live register는 [`crate::disasm::CommonFontDrawSite`]가 원본
//! 명령열에서 검증한다. 화면 PNG는 관측 장면에 귀속하지만 최종 합성
//! 픽셀에서 개별 primitive가 보였다는 판정은 하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::coords::{psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::disasm::CommonFontDrawSite;
use crate::font::{
    parse_runtime_cell_table, RuntimeCell, RuntimeCellTableError, BOX_COMMON_FONT_SPECS,
    BOX_COMMON_FONT_TEXT_SURFACES,
};
use crate::isa::Reg;

pub const COMMON_FONT_DRAW_SCHEMA_VERSION: &str = "common-font-draw-runtime-v2";

const CELL_RECORD_BYTES: u32 = 12;
const CELL_RECORD_HEADER_BYTES: u32 = 12;
const SCREEN_WIDTH: u32 = 350;
const SCREEN_HEIGHT: u32 = 240;

/// 한 cell의 lookup과 primitive 진입 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontDrawRow {
    pub cell: RuntimeCell,
    pub source_glyph: Option<char>,
    pub semantic_category: &'static str,
    pub translation_disposition: &'static str,
    pub japanese_surface_ids: Vec<&'static str>,
    pub japanese_source_texts: Vec<&'static str>,
    pub lookup_hits: usize,
    pub primitive_hits: usize,
    pub clipped_hits: usize,
    pub object_count: usize,
    /// clipping을 통과해 primitive 할당까지 도달한 object 주소.
    ///
    /// TSV에는 노출하지 않고 같은 runtime 증거를 writer 관측과 결속할 때
    /// 사용한다.
    pub primitive_object_runtimes: Vec<u32>,
}

/// 한 guest frame의 `COM_FONT` short renderer 관측 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontDrawReport {
    pub schema_version: &'static str,
    pub exe: &'static str,
    pub archive: &'static str,
    pub renderer_source_sha256: String,
    pub ram_sha256: String,
    pub event_sha256: String,
    pub screen_sha256: String,
    pub descriptor_runtime: u32,
    pub table_runtime: u32,
    pub atlas_width: u16,
    pub atlas_height: u16,
    pub record_count: usize,
    pub event_count: usize,
    pub observed_event_count: usize,
    pub lookup_hits: usize,
    pub primitive_hits: usize,
    pub clipped_hits: usize,
    pub renderer_return_runtimes: Vec<u32>,
    pub japanese_surface_candidate_cells_observed: usize,
    pub screen_capture_shape_exact: bool,
    pub final_pixel_attribution_verified: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
    pub rows: Vec<CommonFontDrawRow>,
}

/// 원본 source와 runtime 증거를 한 번에 검증할 입력.
pub struct CommonFontDrawObservationInput<'a> {
    pub site: &'a CommonFontDrawSite,
    pub descriptor_runtime: u32,
    pub cell_count: usize,
    pub ram: &'a [u8],
    pub renderer_source: &'a [u8],
    pub events_json: &'a [u8],
    pub screen_png: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontDrawError(String);

impl fmt::Display for CommonFontDrawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for CommonFontDrawError {}

impl From<RuntimeCellTableError> for CommonFontDrawError {
    fn from(source: RuntimeCellTableError) -> Self {
        error(format!("COM_FONT cell table 오류: {source}"))
    }
}

fn error(message: impl Into<String>) -> CommonFontDrawError {
    CommonFontDrawError(message.into())
}

pub(crate) fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn runtime_slice<'a>(
    ram: &'a [u8],
    runtime: u32,
    len: usize,
    label: &str,
) -> Result<&'a [u8], CommonFontDrawError> {
    let offset = psx_main_ram_offset(runtime, len).ok_or_else(|| {
        error(format!(
            "{label} PS1 main RAM 범위 이탈: 0x{runtime:08X}+{len}"
        ))
    })?;
    Ok(&ram[offset..offset + len])
}

pub(crate) fn event_u32(
    event: &Value,
    path: &[&str],
    label: &str,
) -> Result<u32, CommonFontDrawError> {
    let value = path
        .iter()
        .try_fold(event, |value, key| value.get(*key))
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("COM_FONT event에 {label}가 없음")))?;
    u32::try_from(value).map_err(|_| error(format!("COM_FONT event {label} 범위 이탈")))
}

fn register_name(register: Reg) -> Result<&'static str, CommonFontDrawError> {
    const NAMES: [&str; 32] = [
        "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5", "t6",
        "t7", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1", "gp", "sp",
        "fp", "ra",
    ];
    NAMES
        .get(usize::from(register.0))
        .copied()
        .ok_or_else(|| error(format!("COM_FONT event register 범위 이탈: {register}")))
}

pub(crate) fn event_register(
    event: &Value,
    register: Reg,
    role: &str,
) -> Result<u32, CommonFontDrawError> {
    let name = register_name(register)?;
    event_u32(event, &["registers", name], &format!("{role} {register}"))
}

pub(crate) fn png_shape_exact(raw: &[u8]) -> bool {
    raw.get(..8) == Some(b"\x89PNG\r\n\x1a\n")
        && raw.get(12..16) == Some(b"IHDR")
        && raw
            .get(16..20)
            .and_then(|value| value.try_into().ok())
            .map(u32::from_be_bytes)
            == Some(SCREEN_WIDTH)
        && raw
            .get(20..24)
            .and_then(|value| value.try_into().ok())
            .map(u32::from_be_bytes)
            == Some(SCREEN_HEIGHT)
}

fn classify_cell(
    site: &CommonFontDrawSite,
    cell: RuntimeCell,
) -> (
    Option<char>,
    &'static str,
    &'static str,
    Vec<&'static str>,
    Vec<&'static str>,
) {
    let matching_surfaces = BOX_COMMON_FONT_TEXT_SURFACES
        .iter()
        .filter(|surface| {
            surface.archive == site.archive
                && surface
                    .representative_runtime_cell_candidates
                    .contains(&u16::try_from(cell.index).unwrap_or(u16::MAX))
        })
        .collect::<Vec<_>>();
    if !matching_surfaces.is_empty() {
        let surface_ids = matching_surfaces
            .iter()
            .map(|surface| surface.surface_id)
            .collect();
        let source_texts = matching_surfaces
            .iter()
            .map(|surface| surface.source_text)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        return (
            None,
            "japanese-baked-surface-candidate",
            "translate-japanese-candidate",
            surface_ids,
            source_texts,
        );
    }

    let digit = (cell.y == 0
        && cell.width == 8
        && cell.height == 16
        && cell.x <= 72
        && cell.x.is_multiple_of(8))
    .then(|| char::from(b'0' + u8::try_from(cell.x / 8).expect("0..=9 범위")));
    if let Some(glyph) = digit {
        return (
            Some(glyph),
            "digit-glyph",
            "protected-non-japanese-reference",
            Vec::new(),
            Vec::new(),
        );
    }

    let latin = (cell.y == 16
        && cell.width == 8
        && cell.height == 16
        && cell.x <= 200
        && cell.x.is_multiple_of(8))
    .then(|| char::from(b'A' + u8::try_from(cell.x / 8).expect("0..=25 범위")));
    if let Some(glyph) = latin {
        return (
            Some(glyph),
            "latin-uppercase-glyph",
            "protected-non-japanese-reference",
            Vec::new(),
            Vec::new(),
        );
    }

    (
        None,
        "unclassified",
        "analysis-required",
        Vec::new(),
        Vec::new(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingLookup {
    cell_index: usize,
    object_runtime: u32,
    return_runtime: u32,
}

#[derive(Debug, Default)]
struct CellHits {
    lookup: usize,
    primitive: usize,
    clipped: usize,
    objects: BTreeSet<u32>,
    primitive_objects: BTreeSet<u32>,
}

/// 지원 원본 renderer와 같은 frame의 breakpoint·화면 증거를 검증한다.
pub fn inspect_common_font_draw(
    input: CommonFontDrawObservationInput<'_>,
) -> Result<CommonFontDrawReport, CommonFontDrawError> {
    if input.ram.len() != PSX_MAIN_RAM_LEN {
        return Err(error(format!(
            "{} COM_FONT RAM 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
            input.site.exe,
            input.ram.len()
        )));
    }
    if input.renderer_source.len() != input.site.renderer_source_bytes {
        return Err(error(format!(
            "{} COM_FONT renderer source 길이 불일치: expected={}, actual={}",
            input.site.exe,
            input.site.renderer_source_bytes,
            input.renderer_source.len()
        )));
    }
    let renderer_source_sha256: [u8; 32] = Sha256::digest(input.renderer_source).into();
    if renderer_source_sha256 != input.site.renderer_source_sha256 {
        return Err(error(format!(
            "{} COM_FONT renderer source digest 불일치",
            input.site.exe
        )));
    }
    let runtime_renderer = runtime_slice(
        input.ram,
        input.site.function_runtime,
        input.renderer_source.len(),
        &format!("{} COM_FONT renderer", input.site.exe),
    )?;
    if runtime_renderer != input.renderer_source {
        return Err(error(format!(
            "{} COM_FONT runtime renderer가 지원 원본과 다름",
            input.site.exe
        )));
    }

    let common_font = BOX_COMMON_FONT_SPECS
        .iter()
        .find(|spec| spec.archive == input.site.archive)
        .ok_or_else(|| {
            error(format!(
                "{} COM_FONT archive 원본 사양이 없음: {}",
                input.site.exe, input.site.archive
            ))
        })?;
    let table = parse_runtime_cell_table(input.ram, input.descriptor_runtime, input.cell_count)?;
    if (
        usize::from(table.atlas_width),
        usize::from(table.atlas_height),
    ) != (common_font.width, common_font.height)
    {
        return Err(error(format!(
            "{} COM_FONT atlas 크기 불일치: {}x{}",
            input.site.exe, table.atlas_width, table.atlas_height
        )));
    }

    let document: Value = serde_json::from_slice(input.events_json)
        .map_err(|source| error(format!("COM_FONT event JSON 해석 실패: {source}")))?;
    let dropped = document
        .get("dropped")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("COM_FONT event JSON에 dropped가 없음"))?;
    if dropped != 0 {
        return Err(error(format!("COM_FONT event dropped={dropped}")));
    }
    let events = document
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| error("COM_FONT event JSON에 events가 없음"))?;

    let mut hits = BTreeMap::<usize, CellHits>::new();
    let mut pending = None::<PendingLookup>;
    let mut observed_event_count = 0usize;
    let mut renderer_return_runtimes = BTreeSet::new();

    for event in events {
        let pc = event_u32(event, &["pc"], "pc")?;
        if pc != input.site.lookup_runtime && pc != input.site.primitive_runtime {
            continue;
        }
        let descriptor = event_register(event, input.site.descriptor_register, "descriptor")?;
        if descriptor != input.descriptor_runtime {
            continue;
        }
        observed_event_count += 1;
        let object_runtime = event_register(event, input.site.object_register, "object")?;
        runtime_slice(input.ram, object_runtime, 1, "COM_FONT object")?;
        let return_runtime = event_register(event, Reg::RA, "renderer return")?;
        renderer_return_runtimes.insert(return_runtime);

        if pc == input.site.lookup_runtime {
            if let Some(previous) = pending.take() {
                hits.entry(previous.cell_index).or_default().clipped += 1;
            }
            let cell_index = usize::try_from(event_register(
                event,
                input.site.cell_index_register,
                "cell index",
            )?)
            .map_err(|_| error("COM_FONT lookup cell index 범위 이탈"))?;
            if cell_index >= table.cells.len() {
                return Err(error(format!(
                    "COM_FONT lookup cell index가 table 밖임: {cell_index}"
                )));
            }
            let cell_hits = hits.entry(cell_index).or_default();
            cell_hits.lookup += 1;
            cell_hits.objects.insert(object_runtime);
            pending = Some(PendingLookup {
                cell_index,
                object_runtime,
                return_runtime,
            });
            continue;
        }

        let record_runtime = event_register(event, input.site.cell_record_register, "cell record")?;
        let record_delta = record_runtime
            .checked_sub(table.table_runtime)
            .ok_or_else(|| {
                error(format!(
                    "COM_FONT primitive record가 table 앞임: 0x{record_runtime:08X}"
                ))
            })?;
        if !record_delta.is_multiple_of(CELL_RECORD_BYTES) {
            return Err(error(format!(
                "COM_FONT primitive record가 12바이트 정렬이 아님: 0x{record_runtime:08X}"
            )));
        }
        let cell_index = usize::try_from(record_delta / CELL_RECORD_BYTES)
            .map_err(|_| error("COM_FONT primitive cell index 범위 이탈"))?;
        if cell_index >= table.cells.len() {
            return Err(error(format!(
                "COM_FONT primitive cell index가 table 밖임: {cell_index}"
            )));
        }
        let record_fields_runtime =
            event_register(event, input.site.cell_fields_register, "cell fields")?;
        let expected_record_fields_runtime =
            record_runtime
                .checked_add(CELL_RECORD_HEADER_BYTES)
                .ok_or_else(|| error("COM_FONT primitive record field 주소 overflow"))?;
        if record_fields_runtime != expected_record_fields_runtime {
            return Err(error(format!(
                "COM_FONT primitive record field 주소 불일치: 0x{record_fields_runtime:08X}"
            )));
        }
        let Some(expected) = pending.take() else {
            return Err(error("COM_FONT primitive 앞에 대응 lookup이 없음"));
        };
        if expected.cell_index != cell_index
            || expected.object_runtime != object_runtime
            || expected.return_runtime != return_runtime
        {
            return Err(error(format!(
                "COM_FONT lookup/primitive 불일치: \
                 lookup=({},0x{:08X},0x{:08X}), primitive=({},0x{:08X},0x{:08X})",
                expected.cell_index,
                expected.object_runtime,
                expected.return_runtime,
                cell_index,
                object_runtime,
                return_runtime,
            )));
        }
        let cell_hits = hits.entry(cell_index).or_default();
        cell_hits.primitive += 1;
        cell_hits.primitive_objects.insert(object_runtime);
    }
    if let Some(previous) = pending {
        hits.entry(previous.cell_index).or_default().clipped += 1;
    }
    if hits.is_empty() {
        return Err(error(format!(
            "{} COM_FONT event에서 대상 descriptor 관측이 없음",
            input.site.exe
        )));
    }

    let mut rows = Vec::with_capacity(hits.len());
    for (cell_index, cell_hits) in hits {
        if cell_hits.lookup != cell_hits.primitive + cell_hits.clipped {
            return Err(error(format!("COM_FONT cell {cell_index} hit 분모 불일치")));
        }
        let cell = table.cells[cell_index];
        let (
            source_glyph,
            semantic_category,
            translation_disposition,
            japanese_surface_ids,
            japanese_source_texts,
        ) = classify_cell(input.site, cell);
        rows.push(CommonFontDrawRow {
            cell,
            source_glyph,
            semantic_category,
            translation_disposition,
            japanese_surface_ids,
            japanese_source_texts,
            lookup_hits: cell_hits.lookup,
            primitive_hits: cell_hits.primitive,
            clipped_hits: cell_hits.clipped,
            object_count: cell_hits.objects.len(),
            primitive_object_runtimes: cell_hits.primitive_objects.into_iter().collect(),
        });
    }

    let screen_capture_shape_exact = png_shape_exact(input.screen_png);
    if !screen_capture_shape_exact {
        return Err(error("COM_FONT 화면 PNG shape 불일치"));
    }
    let lookup_hits = rows.iter().map(|row| row.lookup_hits).sum();
    let primitive_hits = rows.iter().map(|row| row.primitive_hits).sum();
    let clipped_hits = rows.iter().map(|row| row.clipped_hits).sum();
    let japanese_surface_candidate_cells_observed = rows
        .iter()
        .filter(|row| !row.japanese_surface_ids.is_empty())
        .count();

    Ok(CommonFontDrawReport {
        schema_version: COMMON_FONT_DRAW_SCHEMA_VERSION,
        exe: input.site.exe,
        archive: input.site.archive,
        renderer_source_sha256: digest(input.renderer_source),
        ram_sha256: digest(input.ram),
        event_sha256: digest(input.events_json),
        screen_sha256: digest(input.screen_png),
        descriptor_runtime: table.descriptor_runtime,
        table_runtime: table.table_runtime,
        atlas_width: table.atlas_width,
        atlas_height: table.atlas_height,
        record_count: table.cells.len(),
        event_count: events.len(),
        observed_event_count,
        lookup_hits,
        primitive_hits,
        clipped_hits,
        renderer_return_runtimes: renderer_return_runtimes.into_iter().collect(),
        japanese_surface_candidate_cells_observed,
        screen_capture_shape_exact,
        final_pixel_attribution_verified: false,
        runtime_verified: true,
        release_eligible: false,
        rows,
    })
}

/// 검증된 cell별 draw 결과를 결정적 TSV로 만든다.
pub fn render_common_font_draw_tsv(report: &CommonFontDrawReport) -> String {
    let mut output = String::from(
        "schema_version\texe\tarchive\trenderer_source_sha256\tram_sha256\tevent_sha256\t\
         screen_sha256\tdescriptor_runtime\ttable_runtime\tatlas_width\tatlas_height\t\
         record_count\tevent_count\tobserved_event_count\trenderer_return_runtimes\t\
         cell_index\tsource_glyph\tsemantic_category\ttranslation_disposition\t\
         japanese_surface_ids\tjapanese_source_texts\tx\ty\twidth\theight\tanchor_x\t\
         anchor_y\tlookup_hits\tprimitive_hits\tclipped_hits\tobject_count\t\
         screen_capture_shape_exact\tfinal_pixel_attribution_verified\truntime_verified\t\
         release_eligible\n",
    );
    let return_runtimes = report
        .renderer_return_runtimes
        .iter()
        .map(|runtime| format!("0x{runtime:08X}"))
        .collect::<Vec<_>>()
        .join(",");
    for row in &report.rows {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            report.schema_version,
            report.exe,
            report.archive,
            report.renderer_source_sha256,
            report.ram_sha256,
            report.event_sha256,
            report.screen_sha256,
            report.descriptor_runtime,
            report.table_runtime,
            report.atlas_width,
            report.atlas_height,
            report.record_count,
            report.event_count,
            report.observed_event_count,
            return_runtimes,
            row.cell.index,
            row.source_glyph
                .map(|glyph| glyph.to_string())
                .unwrap_or_default(),
            row.semantic_category,
            row.translation_disposition,
            row.japanese_surface_ids.join(","),
            row.japanese_source_texts
                .iter()
                .map(|text| text.replace('\n', "\\n"))
                .collect::<Vec<_>>()
                .join("|"),
            row.cell.x,
            row.cell.y,
            row.cell.width,
            row.cell.height,
            row.cell.anchor_x,
            row.cell.anchor_y,
            row.lookup_hits,
            row.primitive_hits,
            row.clipped_hits,
            row.object_count,
            report.screen_capture_shape_exact,
            report.final_pixel_attribution_verified,
            report.runtime_verified,
            report.release_eligible,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

#[cfg(test)]
#[path = "common_font_draw_tests.rs"]
mod common_font_draw_tests;
