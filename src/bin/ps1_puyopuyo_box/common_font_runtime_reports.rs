//! `COM_FONT.BTX` runtime 관측 보고서 명령.

use std::fs;
use std::path::{Path, PathBuf};

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::disasm::{
    verify_box_btx_cell_consumers, verify_box_btx_name_consumers,
    verify_box_common_font_cell_sources, verify_box_common_font_draw_sites,
    verify_quest_common_font_direct_draw, BOX_BTX_NAME_CONSUMERS,
};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::runtime::{
    inspect_common_font_direct_draw, inspect_common_font_draw, inspect_common_font_writer_draw,
    render_common_font_direct_draw_tsv, render_common_font_draw_tsv,
    render_common_font_writer_draw_tsv, CommonFontDirectDrawObservationInput,
    CommonFontDrawObservationInput, CommonFontWriterDrawObservationInput,
};

use super::btx_reports::executable_source_slice;
use super::disc_commands::read_box_executables;
use super::translation_reports::write_report_file;
use super::usage::usage;
use super::BoxExecutable;

struct CommonFontRuntimeSource {
    executables: Vec<BoxExecutable>,
    draw_sites: ps1_puyopuyo_box::disasm::CommonFontDrawSiteAnalysis,
    cell_sources: ps1_puyopuyo_box::disasm::CommonFontCellSourceAnalysis,
}

fn read_common_font_runtime_source(source_path: &Path) -> Result<CommonFontRuntimeSource, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let archives = BOX_BTX_NAME_CONSUMERS
        .iter()
        .map(|spec| {
            let raw = iso
                .read_file(spec.archive)
                .map_err(|error| error.to_string())?;
            let archive = VddArchive::parse(&raw)
                .map_err(|error| format!("{} VDD 파싱 실패: {error}", spec.archive))?;
            Ok((spec.archive, archive))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let names = verify_box_btx_name_consumers(
        executables
            .iter()
            .map(|executable| (executable.map, executable.text.as_slice())),
        archives
            .iter()
            .map(|(archive_id, archive)| (*archive_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    let cells = verify_box_btx_cell_consumers(
        executables
            .iter()
            .map(|executable| (executable.map, executable.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let draw_sites = verify_box_common_font_draw_sites(
        executables
            .iter()
            .map(|executable| (executable.map, executable.text.as_slice())),
        &cells,
    )
    .map_err(|error| error.to_string())?;
    let cell_sources = verify_box_common_font_cell_sources(
        executables
            .iter()
            .map(|executable| (executable.map, executable.text.as_slice())),
        &names,
        &draw_sites,
    )
    .map_err(|error| error.to_string())?;
    Ok(CommonFontRuntimeSource {
        executables,
        draw_sites,
        cell_sources,
    })
}

fn select_common_font_runtime_source<'a>(
    source: &'a CommonFontRuntimeSource,
    exe_name: &str,
) -> Result<
    (
        &'a BoxExecutable,
        &'a ps1_puyopuyo_box::disasm::CommonFontDrawSite,
    ),
    String,
> {
    let executable = source
        .executables
        .iter()
        .find(|executable| executable.map.name == exe_name)
        .ok_or_else(|| format!("{exe_name} source가 없음"))?;
    let site = source
        .draw_sites
        .sites
        .iter()
        .find(|site| site.exe == exe_name)
        .ok_or_else(|| format!("COM_FONT draw 대상 EXE가 아님: {exe_name}"))?;
    Ok((executable, site))
}

fn parse_runtime_address(raw: &std::ffi::OsStr) -> Result<u32, String> {
    let text = raw
        .to_str()
        .ok_or_else(|| "COM_FONT descriptor runtime 주소가 UTF-8이 아님".to_string())?;
    if let Some(hex) = text.strip_prefix("0x") {
        u32::from_str_radix(hex, 16)
    } else {
        text.parse()
    }
    .map_err(|error| format!("COM_FONT descriptor runtime 주소 오류: {error}"))
}

fn parse_cell_count(raw: &std::ffi::OsStr) -> Result<usize, String> {
    raw.to_str()
        .ok_or_else(|| "COM_FONT record count가 UTF-8이 아님".to_string())?
        .parse::<usize>()
        .map_err(|error| format!("COM_FONT record count 오류: {error}"))
}

pub(super) fn write_common_font_draw_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    const PUYO2_DESCRIPTOR_RUNTIME: u32 = 0x8006_3EC0;
    const PUYO2_CELL_COUNT: usize = 251;

    let args = args.collect::<Vec<_>>();
    let (
        source_path,
        exe_name,
        descriptor_runtime,
        cell_count,
        ram_path,
        event_path,
        screen_path,
        output_path,
    ) = match args.as_slice() {
        [source_path, ram_path, event_path, screen_path, output_path] => (
            PathBuf::from(source_path),
            "PUYO2.EXE",
            PUYO2_DESCRIPTOR_RUNTIME,
            PUYO2_CELL_COUNT,
            PathBuf::from(ram_path),
            PathBuf::from(event_path),
            PathBuf::from(screen_path),
            PathBuf::from(output_path),
        ),
        [source_path, exe_arg, descriptor_arg, count_arg, ram_path, event_path, screen_path, output_path] =>
        {
            let exe_name = exe_arg
                .to_str()
                .ok_or_else(|| "COM_FONT EXE 이름이 UTF-8이 아님".to_string())?;
            (
                PathBuf::from(source_path),
                exe_name,
                parse_runtime_address(descriptor_arg)?,
                parse_cell_count(count_arg)?,
                PathBuf::from(ram_path),
                PathBuf::from(event_path),
                PathBuf::from(screen_path),
                PathBuf::from(output_path),
            )
        }
        _ => return Err(usage().to_string()),
    };
    let source = read_common_font_runtime_source(&source_path)?;
    let (executable, site) = select_common_font_runtime_source(&source, exe_name)?;
    let renderer_source = executable_source_slice(
        executable,
        site.function_runtime,
        site.renderer_source_bytes,
        "COM_FONT short renderer",
    )?;
    let ram = fs::read(&ram_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", ram_path.display()))?;
    let events = fs::read(&event_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", event_path.display()))?;
    let screen = fs::read(&screen_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", screen_path.display()))?;
    let report = inspect_common_font_draw(CommonFontDrawObservationInput {
        site,
        descriptor_runtime,
        cell_count,
        ram: &ram,
        renderer_source,
        events_json: &events,
        screen_png: &screen,
    })
    .map_err(|error| error.to_string())?;
    let output = render_common_font_draw_tsv(&report);
    write_report_file(
        &output_path,
        "COM_FONT draw runtime TSV",
        report.rows.len(),
        &output,
    )?;
    println!(
        "  EXE {}, cells {}, lookup {}, primitive {}, clipped {}, Japanese surface candidates {}, callers {}, final pixel attribution {}",
        report.exe,
        report.rows.len(),
        report.lookup_hits,
        report.primitive_hits,
        report.clipped_hits,
        report.japanese_surface_candidate_cells_observed,
        report.renderer_return_runtimes.len(),
        report.final_pixel_attribution_verified,
    );
    Ok(())
}

pub(super) fn write_common_font_writer_draw_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let args = args.collect::<Vec<_>>();
    let [source_path, exe_arg, descriptor_arg, count_arg, ram_path, writer_event_path, draw_event_path, screen_path, output_path] =
        args.as_slice()
    else {
        return Err(usage().to_string());
    };
    let exe_name = exe_arg
        .to_str()
        .ok_or_else(|| "COM_FONT EXE 이름이 UTF-8이 아님".to_string())?;
    let descriptor_runtime = parse_runtime_address(descriptor_arg)?;
    let cell_count = parse_cell_count(count_arg)?;
    let source_path = PathBuf::from(source_path);
    let ram_path = PathBuf::from(ram_path);
    let writer_event_path = PathBuf::from(writer_event_path);
    let draw_event_path = PathBuf::from(draw_event_path);
    let screen_path = PathBuf::from(screen_path);
    let output_path = PathBuf::from(output_path);

    let source = read_common_font_runtime_source(&source_path)?;
    let (executable, site) = select_common_font_runtime_source(&source, exe_name)?;
    let renderer_source = executable_source_slice(
        executable,
        site.function_runtime,
        site.renderer_source_bytes,
        "COM_FONT short renderer",
    )?;
    let ram = fs::read(&ram_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", ram_path.display()))?;
    let writer_events = fs::read(&writer_event_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", writer_event_path.display()))?;
    let draw_events = fs::read(&draw_event_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", draw_event_path.display()))?;
    let screen = fs::read(&screen_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", screen_path.display()))?;
    let draw_report = inspect_common_font_draw(CommonFontDrawObservationInput {
        site,
        descriptor_runtime,
        cell_count,
        ram: &ram,
        renderer_source,
        events_json: &draw_events,
        screen_png: &screen,
    })
    .map_err(|error| error.to_string())?;
    let report = inspect_common_font_writer_draw(CommonFontWriterDrawObservationInput {
        map: executable.map,
        exe_text: &executable.text,
        site,
        cell_sources: &source.cell_sources,
        draw_report: &draw_report,
        ram: &ram,
        writer_events_json: &writer_events,
    })
    .map_err(|error| error.to_string())?;
    let output = render_common_font_writer_draw_tsv(&report);
    write_report_file(
        &output_path,
        "COM_FONT writer/draw runtime TSV",
        report.rows.len(),
        &output,
    )?;
    println!(
        "  EXE {}, writer sites {}/{}, writer objects {}, draw objects {}, lookup/primitive/clipped {}/{}/{}, object sets equal {}, final pixel attribution {}",
        report.exe,
        report.observed_writer_sites,
        report.candidate_writer_sites,
        report.writer_object_count,
        report.draw_object_count,
        report.draw_lookup_hits,
        report.draw_primitive_hits,
        report.draw_clipped_hits,
        report.writer_draw_object_sets_equal,
        report.final_pixel_attribution_verified,
    );
    Ok(())
}

pub(super) fn write_common_font_direct_draw_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let args = args.collect::<Vec<_>>();
    let [source_path, ram_path, writer_event_path, post_store_event_path, handoff_event_path, commit_event_path, screen_path, output_path] =
        args.as_slice()
    else {
        return Err(usage().to_string());
    };
    let source_path = PathBuf::from(source_path);
    let ram_path = PathBuf::from(ram_path);
    let writer_event_path = PathBuf::from(writer_event_path);
    let post_store_event_path = PathBuf::from(post_store_event_path);
    let handoff_event_path = PathBuf::from(handoff_event_path);
    let commit_event_path = PathBuf::from(commit_event_path);
    let screen_path = PathBuf::from(screen_path);
    let output_path = PathBuf::from(output_path);

    let source = read_common_font_runtime_source(&source_path)?;
    let executable = source
        .executables
        .iter()
        .find(|executable| executable.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE source가 없음".to_string())?;
    let site = verify_quest_common_font_direct_draw(executable.map, &executable.text)
        .map_err(|error| error.to_string())?;
    let ram = fs::read(&ram_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", ram_path.display()))?;
    let writer_events = fs::read(&writer_event_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", writer_event_path.display()))?;
    let post_store_events = fs::read(&post_store_event_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", post_store_event_path.display()))?;
    let handoff_events = fs::read(&handoff_event_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", handoff_event_path.display()))?;
    let commit_events = fs::read(&commit_event_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", commit_event_path.display()))?;
    let screen = fs::read(&screen_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", screen_path.display()))?;
    let report = inspect_common_font_direct_draw(CommonFontDirectDrawObservationInput {
        site: &site,
        ram: &ram,
        writer_events_json: &writer_events,
        post_store_events_json: &post_store_events,
        handoff_events_json: &handoff_events,
        commit_events_json: &commit_events,
        screen_png: &screen,
    })
    .map_err(|error| error.to_string())?;
    let output = render_common_font_direct_draw_tsv(&report);
    write_report_file(
        &output_path,
        "QUEST COM_FONT direct draw runtime TSV",
        1,
        &output,
    )?;
    println!(
        "  surface {}, object 0x{:08X}, cell/resource {}/{}, cell record 0x{:08X}, \
         command slot 0x{:08X}, xy ({},{}), flags {}, final pixel attribution {}",
        report.surface_id,
        report.object_runtime,
        report.cell.index,
        report.resource,
        report.cell_record_runtime,
        report.command_slot_runtime,
        report.command_x,
        report.command_y,
        report.command_flags,
        report.final_pixel_attribution_verified,
    );
    Ok(())
}
