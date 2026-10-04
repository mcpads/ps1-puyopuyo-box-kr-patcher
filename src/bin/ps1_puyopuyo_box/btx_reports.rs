//! BTX 이름·pending consumer 정적 및 runtime 보고서 명령.

use std::fs;
use std::path::{Path, PathBuf};

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::btx::PixelFormat;
use ps1_puyopuyo_box::compress::{decompress, parse_header};
use ps1_puyopuyo_box::disasm::{
    render_btx_name_consumers_tsv, render_common_font_cell_sources_tsv,
    render_common_font_fixed_draws_tsv, verify_box_btx_cell_consumers,
    verify_box_btx_descriptor_selections, verify_box_btx_name_consumers,
    verify_box_common_font_cell_sources, verify_box_common_font_draw_sites,
    verify_box_common_font_fixed_draws, BOX_BTX_NAME_CONSUMERS,
};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::font::{
    render_common_font_text_surfaces_tsv, verify_box_common_font_text_surfaces,
    BOX_COMMON_FONT_SPECS,
};
use ps1_puyopuyo_box::graphics::btx_family_candidates::{
    render_btx_candidate_families_tsv, render_btx_family_candidate_features_tsv,
    verify_box_btx_family_candidates, BtxConsumerEvidence, BtxFamilyCandidateAnalysis,
};
use ps1_puyopuyo_box::graphics::btx_patch_strategy::{
    render_btx_raster_profile_candidate_members_tsv, render_btx_raster_profile_candidates_tsv,
    verify_box_btx_patch_strategy,
};
use ps1_puyopuyo_box::graphics::text_candidates::{
    verify_pending_text_asset, BOX_PENDING_TEXT_ASSETS,
};
use ps1_puyopuyo_box::pipeline::{
    audit_btx_pending_consumer_plan, render_btx_pending_consumer_plan_tsv,
    render_btx_pending_static_cells_tsv, render_btx_review_layout_evidence_tsv,
    verify_box_btx_pending_static_cells,
};
use ps1_puyopuyo_box::review::render_pending_btx_family_review_decisions_tsv;
use ps1_puyopuyo_box::runtime::{
    inspect_pending_btx_runtime, render_pending_btx_runtime_tsv, PendingBtxRuntimeSource,
    BTX_DESCRIPTOR_IDENTITY_BYTES,
};
use ps1_puyopuyo_box::script::{
    verify_box_subgame_seq_layouts, verify_box_subgame_seq_text_candidates,
    BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS,
};

use super::analysis_inputs::read_subgame_seq_sources;
use super::btx_review_layout_inputs::read_btx_review_layout_source_inputs;
use super::disc_commands::read_box_executables;
use super::translation_reports::write_report_file;
use super::usage::usage;
use super::{BoxExecutable, PendingBtxRuntimePayload};

pub(super) struct BtxFamilySourceInputs {
    pub archives: Vec<(&'static str, VddArchive)>,
    pub candidates: BtxFamilyCandidateAnalysis,
}

/// 지원 원본에서 BTX family 후보와 대표 preview에 필요한 archive를 함께 읽는다.
pub(super) fn read_btx_family_source_inputs(
    source_path: &Path,
) -> Result<BtxFamilySourceInputs, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    read_btx_family_source_inputs_from(&iso, &executables)
}

/// 이미 검증한 disc view에서 BTX family 입력을 만든다.
///
/// 같은 명령이 추가 source-bound 분석을 이어갈 때 원본 식별·ISO parse·11개
/// EXE 검증을 다시 수행하지 않도록 한다.
pub(super) fn read_btx_family_source_inputs_from(
    iso: &Iso9660<'_>,
    executables: &[BoxExecutable],
) -> Result<BtxFamilySourceInputs, String> {
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
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        archives
            .iter()
            .map(|(archive_id, archive)| (*archive_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    let candidates = verify_box_btx_family_candidates(
        archives
            .iter()
            .map(|(archive_id, archive)| (*archive_id, archive)),
        names.links.iter().map(|link| BtxConsumerEvidence {
            archive: link.archive,
            name: &link.asset_name,
            static_evidence: link.static_evidence(),
            static_reference_sites: link.static_ref_sites.len()
                + link.entry_reached_ref_sites.len()
                + link.table_static_ref_sites.len()
                + link.table_entry_reached_ref_sites.len()
                + link.descriptor_static_ref_sites.len()
                + link.descriptor_entry_reached_ref_sites.len(),
            descriptor_runtime: link.descriptor_runtime_start,
        }),
    )
    .map_err(|error| error.to_string())?;
    Ok(BtxFamilySourceInputs {
        archives,
        candidates,
    })
}

pub(super) fn write_btx_name_report(
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
    let analysis = verify_box_btx_name_consumers(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        archives
            .iter()
            .map(|(archive_id, archive)| (*archive_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    let report = render_btx_name_consumers_tsv(&analysis);
    write_report_file(
        output_path,
        "BTX name consumer TSV",
        analysis.links.len(),
        &report,
    )
}

pub(super) fn write_common_font_text_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let archives = BOX_COMMON_FONT_SPECS
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
    let report = verify_box_common_font_text_surfaces(
        archives
            .iter()
            .map(|(archive_id, archive)| (*archive_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    let output = render_common_font_text_surfaces_tsv(&report);
    write_report_file(
        output_path,
        "COM_FONT Japanese text surface TSV",
        report.rows.len(),
        &output,
    )?;
    println!(
        "  archives {}, source texts {}, pixel surfaces {}, runtime cell candidates {}, runtime draw verified false",
        report.physical_archives,
        report.unique_source_texts,
        report.unique_pixel_surfaces,
        report.representative_runtime_cell_bindings,
    );
    Ok(())
}

pub(super) fn write_common_font_cell_source_report(
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
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        archives
            .iter()
            .map(|(archive_id, archive)| (*archive_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    let cells =
        verify_box_btx_cell_consumers(executables.iter().map(|exe| (exe.map, exe.text.as_slice())))
            .map_err(|error| error.to_string())?;
    let draw_sites = verify_box_common_font_draw_sites(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        &cells,
    )
    .map_err(|error| error.to_string())?;
    let analysis = verify_box_common_font_cell_sources(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        &names,
        &draw_sites,
    )
    .map_err(|error| error.to_string())?;
    let report = render_common_font_cell_sources_tsv(&analysis);
    write_report_file(
        output_path,
        "COM_FONT Japanese cell sources TSV",
        analysis.rows.len(),
        &report,
    )?;
    println!(
        "  resolved/unresolved surfaces {}/{}, literal writers {}, entry-reached {}",
        analysis.resolved_surfaces,
        analysis.unresolved_surfaces,
        analysis.literal_writer_sites,
        analysis.entry_reached_writer_sites,
    );
    Ok(())
}

pub(super) fn write_common_font_fixed_draw_report(
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
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        archives
            .iter()
            .map(|(archive_id, archive)| (*archive_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    let analysis = verify_box_common_font_fixed_draws(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        &names,
    )
    .map_err(|error| error.to_string())?;
    let report = render_common_font_fixed_draws_tsv(&analysis);
    write_report_file(
        output_path,
        "fixed COM_FONT direct draw TSV",
        analysis.sites.len(),
        &report,
    )?;
    println!(
        "  scanned EXEs {}, COM_FONT EXEs {}, fixed direct draws {}, analysis sha256 {}",
        analysis.scanned_executables,
        analysis.common_font_executables,
        analysis.sites.len(),
        analysis
            .analysis_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    Ok(())
}

pub(super) fn write_btx_pending_consumer_report(
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
    let name_analysis = verify_box_btx_name_consumers(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        archives
            .iter()
            .map(|(archive_id, archive)| (*archive_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    let selection_analysis = verify_box_btx_descriptor_selections(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let plan = audit_btx_pending_consumer_plan(
        &BOX_PENDING_TEXT_ASSETS,
        &name_analysis,
        &selection_analysis,
    )
    .map_err(|error| error.to_string())?;
    let report = render_btx_pending_consumer_plan_tsv(&plan);
    write_report_file(
        output_path,
        "pending BTX consumer plan TSV",
        plan.rows.len(),
        &report,
    )
}

pub(super) fn write_btx_pending_static_cell_report(
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
    let grp_archives = BOX_BTX_NAME_CONSUMERS
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
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        grp_archives
            .iter()
            .map(|(archive_id, archive)| (*archive_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    let seq_archives = BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS
        .iter()
        .map(|expectation| {
            let raw = iso
                .read_file(expectation.archive)
                .map_err(|error| error.to_string())?;
            let archive = VddArchive::parse(&raw)
                .map_err(|error| format!("{} VDD 파싱 실패: {error}", expectation.archive))?;
            Ok((expectation.archive, archive))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let sources = read_subgame_seq_sources(&seq_archives)?;
    let text = verify_box_subgame_seq_text_candidates(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let layouts = verify_box_subgame_seq_layouts(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text,
    )
    .map_err(|error| error.to_string())?;
    let analysis = verify_box_btx_pending_static_cells(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &layouts,
        &names,
    )
    .map_err(|error| error.to_string())?;
    let report = render_btx_pending_static_cells_tsv(&analysis);
    let report_rows = report.lines().skip(1).count();
    write_report_file(
        output_path,
        "pending BTX static cell supply TSV",
        report_rows,
        &report,
    )?;
    println!(
        "  findings {}, physical {}, setup records {}, rows with setup {}, \
         review-layout matches {}, plain-layout matches {}",
        analysis.findings,
        analysis.physical_assets,
        analysis.setup_records,
        analysis.rows_with_setup,
        analysis.layout_matching_setup_records,
        analysis.plain_layout_matching_setup_records,
    );
    Ok(())
}

pub(super) fn write_btx_profile_candidate_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let strategy = verify_box_btx_patch_strategy().map_err(|error| error.to_string())?;
    let report = render_btx_raster_profile_candidates_tsv().map_err(|error| error.to_string())?;
    write_report_file(
        output_path,
        "BTX raster profile candidate TSV",
        strategy.profile_candidate_groups.len(),
        &report,
    )?;
    println!(
        "  typed Japanese {}, MM_SPR {}, fixed-cell {}, remaining raster blocks/assets {}/{}, \
         pending MENU06 {}, protected MM_MES {}, automatic approval {}",
        strategy.typed_japanese_blocks,
        strategy.mm_spr_managed_blocks,
        strategy.fixed_cell_page_blocks,
        strategy.remaining_raster_blocks,
        strategy.remaining_raster_assets,
        strategy.pending_menu06_findings,
        strategy.protected_mm_mes_findings,
        strategy.automatic_profile_approval,
    );
    Ok(())
}

pub(super) fn write_btx_profile_candidate_member_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let strategy = verify_box_btx_patch_strategy().map_err(|error| error.to_string())?;
    let report =
        render_btx_raster_profile_candidate_members_tsv().map_err(|error| error.to_string())?;
    write_report_file(
        output_path,
        "BTX raster profile candidate member TSV",
        strategy.profile_candidate_members.len(),
        &report,
    )
}

pub(super) fn write_btx_family_candidate_reports(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, feature_output_path, family_output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let inputs = read_btx_family_source_inputs(source_path)?;
    let analysis = &inputs.candidates;
    let features = render_btx_family_candidate_features_tsv(analysis);
    let families = render_btx_candidate_families_tsv(analysis);
    write_report_file(
        feature_output_path,
        "unclassified BTX feature TSV",
        analysis.candidates.len(),
        &features,
    )?;
    write_report_file(
        family_output_path,
        "unclassified BTX family TSV",
        analysis.families.len(),
        &families,
    )?;
    println!(
        "  physical/unique {}/{}, declared/content-known physical {}/{}, \
         known-content unclassified aliases {}, unclassified unique {}, families {}, \
         automatic approval {}",
        analysis.physical_btx,
        analysis.unique_contents,
        analysis.declared_known_physical_btx,
        analysis.known_content_physical_btx,
        analysis.unclassified_known_content_aliases.len(),
        analysis.candidates.len(),
        analysis.families.len(),
        analysis.automatic_approval,
    );
    for alias in &analysis.unclassified_known_content_aliases {
        println!(
            "  unclassified physical alias {}:{} -> {} {}",
            alias.archive, alias.name, alias.known_content_class, alias.decompressed_sha256,
        );
    }
    Ok(())
}

pub(super) fn write_btx_family_review_template(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let inputs = read_btx_family_source_inputs(source_path)?;
    let report = render_pending_btx_family_review_decisions_tsv(&inputs.candidates)
        .map_err(|error| error.to_string())?;
    let rows = report.lines().skip(1).count();
    write_report_file(
        output_path,
        "BTX family review decision template",
        rows,
        &report,
    )
}

pub(super) fn write_btx_review_layout_evidence_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let inputs = read_btx_review_layout_source_inputs(source_path)?;
    let analysis = &inputs.evidence;
    let report = render_btx_review_layout_evidence_tsv(analysis);
    write_report_file(
        output_path,
        "BTX insufficient-review layout evidence TSV",
        report.lines().skip(1).count(),
        &report,
    )?;
    println!(
        "  families {}, unique/physical {}/{}, physical exact/compatible/total {}/{}/{}, \
         candidate exact/row-padded/partial/multiple/none {}/{}/{}/{}/{}, physical with graphics \
         selector {}, graphics/other-type selector records {}/{}, entry-trace code matches {}",
        analysis.families,
        analysis.unique_contents,
        analysis.physical_copies,
        analysis.copies_with_descriptor_sized_setup,
        analysis.copies_with_descriptor_compatible_setup,
        analysis.physical_copies,
        analysis.exact_candidates,
        analysis.row_padded_candidates,
        analysis.partial_candidates,
        analysis.multiple_layout_candidates,
        analysis.candidates_without_compatible_setup,
        analysis.copies_with_graphics_selector,
        analysis.graphics_selector_records,
        analysis.other_type_numeric_matches,
        analysis.entry_trace_code_matches,
    );
    Ok(())
}

pub(super) fn executable_source_slice<'a>(
    executable: &'a BoxExecutable,
    runtime: u32,
    len: usize,
    field: &str,
) -> Result<&'a [u8], String> {
    let offset = executable.map.text_offset(runtime).ok_or_else(|| {
        format!(
            "{} {field} 주소가 EXE text 밖임: 0x{runtime:08X}",
            executable.map.name
        )
    })?;
    let end = offset
        .checked_add(len)
        .ok_or_else(|| format!("{} {field} 길이 overflow", executable.map.name))?;
    executable.text.get(offset..end).ok_or_else(|| {
        format!(
            "{} {field} 범위가 EXE text 밖임: 0x{runtime:08X}+{len}",
            executable.map.name
        )
    })
}

pub(super) fn write_btx_pending_runtime_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, ram_path, gpu_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
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
    let name_analysis = verify_box_btx_name_consumers(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        archives
            .iter()
            .map(|(archive_id, archive)| (*archive_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    let selection_analysis = verify_box_btx_descriptor_selections(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let plan = audit_btx_pending_consumer_plan(
        &BOX_PENDING_TEXT_ASSETS,
        &name_analysis,
        &selection_analysis,
    )
    .map_err(|error| error.to_string())?;

    let payloads = plan
        .rows
        .iter()
        .map(|row| {
            let spec = BOX_PENDING_TEXT_ASSETS
                .iter()
                .find(|spec| spec.finding_id == row.finding_id)
                .ok_or_else(|| format!("{} pending BTX spec 누락", row.finding_id))?;
            if spec.name != row.asset_name || !spec.archive_paths.contains(&row.archive) {
                return Err(format!(
                    "{} pending BTX plan/spec 물리 자산 불일치",
                    row.finding_id
                ));
            }
            let archive = archives
                .iter()
                .find(|(archive_id, _)| *archive_id == row.archive)
                .map(|(_, archive)| archive)
                .ok_or_else(|| format!("{} archive 입력 누락", row.archive))?;
            let entry = archive
                .find(&row.asset_name)
                .ok_or_else(|| format!("{}에 {}이 없음", row.archive, row.asset_name))?;
            let compressed = archive.read_file(entry);
            let header = parse_header(compressed).map_err(|error| {
                format!("{}:{} CNX 헤더 실패: {error}", row.archive, row.asset_name)
            })?;
            let format = PixelFormat::from_cnx_subtype(header.subtype)
                .map_err(|error| format!("{}:{}: {error}", row.archive, row.asset_name))?;
            if format != spec.review_layout.format {
                return Err(format!(
                    "{}:{} pixel format 불일치",
                    row.archive, row.asset_name
                ));
            }
            let raw = decompress(compressed).map_err(|error| {
                format!("{}:{} CNX 해제 실패: {error}", row.archive, row.asset_name)
            })?;
            verify_pending_text_asset(&raw, *spec).map_err(|error| error.to_string())?;

            let executable = executables
                .iter()
                .find(|executable| executable.map.name == row.exe)
                .ok_or_else(|| format!("{} EXE 입력 누락", row.exe))?;
            let selection_code_len = row
                .selection_copy_end_runtime
                .checked_add(4)
                .and_then(|end| end.checked_sub(row.resource_multiply_runtime))
                .and_then(|len| usize::try_from(len).ok())
                .ok_or_else(|| format!("{} selection code window overflow", row.exe))?;
            let selection_code = executable_source_slice(
                executable,
                row.resource_multiply_runtime,
                selection_code_len,
                "selection code",
            )?
            .to_vec();
            let original_source_descriptor_identity = executable_source_slice(
                executable,
                row.descriptor_entry_runtime,
                BTX_DESCRIPTOR_IDENTITY_BYTES,
                "source descriptor identity",
            )?
            .to_vec();
            Ok(PendingBtxRuntimePayload {
                finding_id: row.finding_id.clone(),
                exe: row.exe,
                archive: row.archive,
                asset_name: row.asset_name.clone(),
                format,
                raw,
                expected_sha256: spec.expected_decompressed_sha256,
                descriptor_entry_runtime: row.descriptor_entry_runtime,
                original_source_descriptor_identity,
                working_descriptor_runtime: row.working_descriptor_runtime,
                selection_code_runtime: row.resource_multiply_runtime,
                selection_code,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let ram =
        fs::read(ram_path).map_err(|error| format!("{} 읽기 실패: {error}", ram_path.display()))?;
    let gpu =
        fs::read(gpu_path).map_err(|error| format!("{} 읽기 실패: {error}", gpu_path.display()))?;
    let observation = inspect_pending_btx_runtime(
        &ram,
        &gpu,
        payloads.iter().map(|payload| PendingBtxRuntimeSource {
            finding_id: &payload.finding_id,
            exe: payload.exe,
            archive: payload.archive,
            asset_name: &payload.asset_name,
            format: payload.format,
            raw: &payload.raw,
            expected_sha256: payload.expected_sha256,
            descriptor_entry_runtime: payload.descriptor_entry_runtime,
            original_source_descriptor_identity: &payload.original_source_descriptor_identity,
            working_descriptor_runtime: payload.working_descriptor_runtime,
            selection_code_runtime: payload.selection_code_runtime,
            selection_code: &payload.selection_code,
        }),
    )
    .map_err(|error| error.to_string())?;
    let report = render_pending_btx_runtime_tsv(&observation);
    write_report_file(
        output_path,
        "pending BTX raw RAM/VRAM observation TSV",
        observation.rows.len(),
        &report,
    )?;
    println!(
        "  EXE {}, rows {}, descriptor selected {}, VRAM exact {}, layout ambiguous {}, screen unverified",
        observation.exe,
        observation.rows.len(),
        observation
            .rows
            .iter()
            .filter(|row| row.descriptor_selected)
            .count(),
        observation
            .rows
            .iter()
            .filter(|row| !row.vram_placements.is_empty())
            .count(),
        observation
            .rows
            .iter()
            .filter(|row| row.vram_layouts.len() > 1)
            .count(),
    );
    Ok(())
}
