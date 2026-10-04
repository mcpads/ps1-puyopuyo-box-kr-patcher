//! 비-QUEST text·control 정적 보고서와 text bridge RAM 관측.

use std::fs;
use std::path::PathBuf;

use ps1_puyopuyo_box::analysis::render_subgame_seq_overlay_call_arguments_tsv;
use ps1_puyopuyo_box::disasm::{
    render_subgame_seq_control_targets_tsv, render_subgame_seq_controls_tsv,
    render_subgame_seq_text_bridges_tsv, render_subgame_text_consumers_tsv,
    verify_box_subgame_seq_controls, verify_box_subgame_text_consumers,
};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::runtime::{
    inspect_box_subgame_seq_text_bridge_call, inspect_box_subgame_seq_text_bridge_ram,
    render_subgame_seq_text_bridge_calls_tsv, render_subgame_seq_text_bridge_ram_tsv,
};
use ps1_puyopuyo_box::script::{
    render_subgame_seq_scene_text_tsv, render_subgame_seq_text_candidates_tsv,
    render_subgame_seq_text_roots_tsv, render_subgame_seq_text_summary_tsv,
};

use crate::cli::analysis_inputs::{
    read_subgame_seq_control_target_analysis, read_subgame_seq_min_text_analysis,
    read_subgame_seq_overlay_call_argument_analysis, read_subgame_seq_text_analysis,
    read_subgame_seq_text_bridge_analysis, read_subgame_seq_text_bridge_source,
    read_subgame_seq_text_root_analysis,
};
use crate::cli::disc_commands::read_box_executables;
use crate::cli::translation_reports::write_report_file;
use crate::cli::usage::usage;

pub(in crate::cli) fn write_subgame_text_consumer_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let analysis = verify_box_subgame_text_consumers(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let report = render_subgame_text_consumers_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame SEQ text consumer static analysis TSV",
        analysis.consumers.len()
            + analysis.control_tables.len()
            + analysis
                .control_tables
                .iter()
                .map(|table| table.handlers.len())
                .sum::<usize>(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_text_bridge_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_text_bridge_analysis(source_path)?;
    let report = render_subgame_seq_text_bridges_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame SEQ text constructor bridge static analysis TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_overlay_call_argument_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_overlay_call_argument_analysis(source_path)?;
    let report = render_subgame_seq_overlay_call_arguments_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame SEQ overlay outgoing stack argument flow TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_text_bridge_ram_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, ram_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = read_subgame_seq_text_bridge_source(source_path)?;
    let executables = source.executables;
    let ram =
        fs::read(ram_path).map_err(|error| format!("{} 읽기 실패: {error}", ram_path.display()))?;
    let observation = inspect_box_subgame_seq_text_bridge_ram(
        &ram,
        executables
            .iter()
            .filter(|exe| matches!(exe.map.name, "PCOMI.EXE" | "PUYO1.EXE" | "PUYO2.EXE"))
            .map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let report = render_subgame_seq_text_bridge_ram_tsv(&observation);
    write_report_file(
        output_path,
        "subgame SEQ text constructor bridge RAM observation TSV",
        1,
        &report,
    )?;
    println!(
        "  EXE {}, source code windows {}, synthetic pointer 0x{:08X}, record occurrences {}, \
         constructor pointer occurrences {}, outside slots {}, {}, {}",
        observation.exe,
        observation.source_code_windows_verified,
        observation.synthetic_pointer,
        observation.synthetic_record_occurrences,
        observation.constructor_pointer_occurrences,
        observation
            .outside_text_slots
            .iter()
            .map(|slot| format!("0x{:08X}=0x{:08X}", slot.runtime, slot.value))
            .collect::<Vec<_>>()
            .join(";"),
        observation.observation_status(),
        observation.outside_text_status(),
    );
    Ok(())
}

pub(in crate::cli) fn write_subgame_seq_text_bridge_call_reports(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    if paths.len() < 5 || !(paths.len() - 2).is_multiple_of(3) {
        return Err(usage().to_string());
    }
    let source_path = &paths[0];
    let output_path = paths.last().expect("최소 길이를 검증했다");
    let source = read_subgame_seq_text_bridge_source(source_path)?;
    let executables = source.executables;
    let mut observations = Vec::new();
    for inputs in paths[1..paths.len() - 1].chunks_exact(3) {
        let [ram_path, events_path, screen_path] = inputs else {
            unreachable!("3개 단위로 나눴다")
        };
        let ram = fs::read(ram_path)
            .map_err(|error| format!("{} 읽기 실패: {error}", ram_path.display()))?;
        let events = fs::read(events_path)
            .map_err(|error| format!("{} 읽기 실패: {error}", events_path.display()))?;
        let screen = fs::read(screen_path)
            .map_err(|error| format!("{} 읽기 실패: {error}", screen_path.display()))?;
        observations.push(
            inspect_box_subgame_seq_text_bridge_call(
                &ram,
                &events,
                &screen,
                executables
                    .iter()
                    .filter(|exe| matches!(exe.map.name, "PCOMI.EXE" | "PUYO1.EXE" | "PUYO2.EXE"))
                    .map(|exe| (exe.map, exe.text.as_slice())),
            )
            .map_err(|error| error.to_string())?,
        );
    }
    observations.sort_by_key(|observation| (observation.exe, observation.call_runtime));
    if observations
        .windows(2)
        .any(|rows| rows[0].exe == rows[1].exe && rows[0].call_runtime == rows[1].call_runtime)
    {
        return Err("subgame text bridge call 입력에 같은 EXE/call site가 중복됨".to_string());
    }
    let report = render_subgame_seq_text_bridge_calls_tsv(&observations);
    write_report_file(
        output_path,
        "subgame SEQ text bridge runtime-field call TSV",
        observations.len(),
        &report,
    )?;
    for observation in &observations {
        println!(
            "  EXE {}, call 0x{:08X}, target 0x{:08X} ({}), field 0x{:08X}, arg5 0x{:08X}",
            observation.exe,
            observation.call_runtime,
            observation.target_runtime,
            observation.target_classification,
            observation.field_runtime,
            observation.fifth_argument,
        );
    }
    Ok(())
}

pub(in crate::cli) fn write_subgame_seq_control_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let consumers = verify_box_subgame_text_consumers(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let analysis = verify_box_subgame_seq_controls(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        &consumers,
    )
    .map_err(|error| error.to_string())?;
    let report = render_subgame_seq_controls_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame SEQ control stream analysis TSV",
        analysis.sites.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_control_target_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_control_target_analysis(source_path)?;
    let report = render_subgame_seq_control_targets_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame SEQ control target static analysis TSV",
        analysis.rows.len() + analysis.occurrences.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_text_candidate_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_text_analysis(source_path)?;
    let report = render_subgame_seq_text_candidates_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame SEQ text candidate TSV",
        analysis.candidates.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_text_summary_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_text_analysis(source_path)?;
    let report = render_subgame_seq_text_summary_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame SEQ text summary TSV",
        analysis.summaries.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_text_root_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_text_root_analysis(source_path)?;
    let report = render_subgame_seq_text_roots_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame SEQ text internal pointer root TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_min_text_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_min_text_analysis(source_path)?;
    let report = render_subgame_seq_scene_text_tsv(&analysis);
    write_report_file(
        output_path,
        "MIN scene-local repertoire and E0 message TSV",
        analysis.rows.len(),
        &report,
    )
}
