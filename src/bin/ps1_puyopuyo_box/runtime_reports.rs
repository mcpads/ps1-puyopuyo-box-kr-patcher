//! 주소·RAM·font hook의 runtime 관측 보고서 명령.

use std::fs;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

use ps1_puyopuyo_box::coords::RuntimeAssetMap;
use ps1_puyopuyo_box::disasm::{
    verify_box_quest_control_meanings, verify_box_quest_font_consumer,
    verify_box_quest_hook_space_candidates, verify_box_quest_text_window_consumer,
    QUEST_SEQ_BRANCH_STATE_BASE_RUNTIME, QUEST_SEQ_DEFAULT_RESOURCE_ID_RUNTIME,
    QUEST_SEQ_INTERPRETER_RUNTIME, QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::font::{
    parse_runtime_cell_selection, parse_runtime_cell_table, render_runtime_cell_table_tsv,
};
use ps1_puyopuyo_box::runtime::{
    inspect_quest_font_windows, inspect_quest_font_windows_ram, inspect_town00_quest_font_scenes,
    render_quest_font_window_geometry_tsv, MednafenPsxState,
};
use ps1_puyopuyo_box::script::{
    audit_quest_seq_addresses, parse_quest_token, render_quest_seq_address_candidates_tsv,
    render_town00_internal_address_candidates_tsv, town00_internal_address_candidates,
    verify_box_quest_seq_addresses, verify_box_quest_seq_translation_units,
    verify_box_quest_terminology_candidates, verify_box_quest_translation_batches, QuestToken,
    Town00Seq, QUEST_SEQ_POINTER_RECORD_SPECS, TOWN00_SEQ_LEN, TOWN00_SEQ_RUNTIME_BASE,
};

use super::analysis_inputs::read_town00_source;
use super::disc_commands::read_box_executables;
use super::quest_analysis_inputs::{
    read_quest_seq_sources_from_disc, read_quest_seq_text_extraction_analysis,
};
use super::translation_reports::write_report_file;
use super::usage::usage;
use super::QUEST_SEQ_INTERPRETER_CALLBACK_FIELD_OFFSET;

pub(super) fn write_town00_address_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let raw = read_town00_source(source_path)?;
    let candidates = town00_internal_address_candidates(&raw).map_err(|error| error.to_string())?;
    let report = render_town00_internal_address_candidates_tsv(&candidates);
    write_report_file(
        output_path,
        "TOWN00 internal address candidate TSV",
        candidates.len(),
        &report,
    )?;
    println!(
        "  runtime-read {}, range-only {}",
        candidates
            .iter()
            .filter(|candidate| candidate.runtime_consumer.is_some())
            .count(),
        candidates
            .iter()
            .filter(|candidate| candidate.runtime_consumer.is_none())
            .count(),
    );
    Ok(())
}

pub(super) fn write_quest_seq_address_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let sources = read_quest_seq_sources_from_disc(source_path)?;
    let analysis = audit_quest_seq_addresses(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    verify_box_quest_seq_addresses(&analysis).map_err(|error| error.to_string())?;
    let report = render_quest_seq_address_candidates_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ address candidate TSV",
        analysis.candidates.len(),
        &report,
    )?;
    let pointer_counts = analysis.summaries.iter().fold(
        [0; QUEST_SEQ_POINTER_RECORD_SPECS.len()],
        |mut sum, summary| {
            for (index, count) in summary.pointer_records.iter().enumerate() {
                sum[index] += count;
            }
            sum
        },
    );
    let pointer_breakdown = QUEST_SEQ_POINTER_RECORD_SPECS
        .iter()
        .zip(pointer_counts)
        .map(|(spec, count)| format!("{:02X}+{}:{count}", spec.opcode, spec.pointer_offset))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "  sequences {}, address-bearing {}, pointer shapes {} [{}], \
         OP89 inner relations {} fields / {} edges, runtime-read {}",
        analysis.summaries.len(),
        analysis
            .summaries
            .iter()
            .filter(|summary| summary.fields > 0)
            .count(),
        pointer_counts.iter().sum::<usize>(),
        pointer_breakdown,
        analysis
            .summaries
            .iter()
            .map(|summary| summary.relational_pointer_fields)
            .sum::<usize>(),
        analysis
            .summaries
            .iter()
            .map(|summary| summary.relational_pointer_edges)
            .sum::<usize>(),
        analysis
            .summaries
            .iter()
            .map(|summary| summary.runtime_read_fields)
            .sum::<usize>(),
    );
    Ok(())
}

pub(super) fn write_runtime_cell_table_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let args = args.collect::<Vec<_>>();
    let [ram_path, descriptor_arg, count_arg, output_path] = args.as_slice() else {
        return Err(usage().to_string());
    };
    let descriptor_text = descriptor_arg
        .to_str()
        .ok_or_else(|| "descriptor runtime 주소가 UTF-8이 아님".to_string())?;
    let descriptor_runtime = if let Some(hex) = descriptor_text.strip_prefix("0x") {
        u32::from_str_radix(hex, 16)
    } else {
        descriptor_text.parse()
    }
    .map_err(|error| format!("descriptor runtime 주소 오류: {error}"))?;
    let count_text = count_arg
        .to_str()
        .ok_or_else(|| "record count가 UTF-8이 아님".to_string())?;
    let record_count = count_text
        .parse::<usize>()
        .map_err(|error| format!("record count 오류: {error}"))?;
    let ram_path = PathBuf::from(ram_path);
    let ram = fs::read(&ram_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", ram_path.display()))?;
    let table = parse_runtime_cell_table(&ram, descriptor_runtime, record_count)
        .map_err(|error| error.to_string())?;
    let report = render_runtime_cell_table_tsv(&table);
    write_report_file(
        &PathBuf::from(output_path),
        "runtime cell table TSV",
        table.cells.len(),
        &report,
    )?;
    println!(
        "  descriptor 0x{:08X}, table 0x{:08X}, atlas {}x{}, \
         distinct sizes {}, large cells {}",
        table.descriptor_runtime,
        table.table_runtime,
        table.atlas_width,
        table.atlas_height,
        table.distinct_sizes(),
        table.large_cells(),
    );
    Ok(())
}

pub(super) fn write_runtime_cell_selection_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let args = args.collect::<Vec<_>>();
    let [ram_path, descriptor_arg, count_arg, indices_arg, output_path] = args.as_slice() else {
        return Err(usage().to_string());
    };
    let descriptor_text = descriptor_arg
        .to_str()
        .ok_or_else(|| "descriptor runtime 주소가 UTF-8이 아님".to_string())?;
    let descriptor_runtime = if let Some(hex) = descriptor_text.strip_prefix("0x") {
        u32::from_str_radix(hex, 16)
    } else {
        descriptor_text.parse()
    }
    .map_err(|error| format!("descriptor runtime 주소 오류: {error}"))?;
    let count_text = count_arg
        .to_str()
        .ok_or_else(|| "record count가 UTF-8이 아님".to_string())?;
    let record_count = count_text
        .parse::<usize>()
        .map_err(|error| format!("record count 오류: {error}"))?;
    let indices_text = indices_arg
        .to_str()
        .ok_or_else(|| "cell index CSV가 UTF-8이 아님".to_string())?;
    let selected_indices = indices_text
        .split(',')
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|error| format!("cell index 오류 ({value}): {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let ram_path = PathBuf::from(ram_path);
    let ram = fs::read(&ram_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", ram_path.display()))?;
    let table =
        parse_runtime_cell_selection(&ram, descriptor_runtime, record_count, &selected_indices)
            .map_err(|error| error.to_string())?;
    let report = render_runtime_cell_table_tsv(&table);
    write_report_file(
        &PathBuf::from(output_path),
        "runtime selected cell TSV",
        table.cells.len(),
        &report,
    )?;
    println!(
        "  descriptor 0x{:08X}, table 0x{:08X}, atlas {}x{}, selected {:?}",
        table.descriptor_runtime,
        table.table_runtime,
        table.atlas_width,
        table.atlas_height,
        table
            .cells
            .iter()
            .map(|cell| cell.index)
            .collect::<Vec<_>>(),
    );
    Ok(())
}

pub(super) fn report_town00_root_state(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, state_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let town00 = read_town00_source(source_path)?;
    let seq = Town00Seq::parse_source(&town00).map_err(|error| error.to_string())?;
    let frontier = seq
        .root_global_word_branch()
        .map_err(|error| error.to_string())?;
    let state_word_runtime = QUEST_SEQ_BRANCH_STATE_BASE_RUNTIME
        .checked_add(u32::from(frontier.global_state_word_index()) * 4)
        .ok_or_else(|| "TOWN00 root state word 주소 overflow".to_string())?;

    let state_raw = fs::read(state_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", state_path.display()))?;
    let state = MednafenPsxState::parse(&state_raw).map_err(|error| error.to_string())?;
    let branch_index = state
        .read_u32_le(state_word_runtime)
        .map_err(|error| error.to_string())?;
    let target = seq
        .resolve_root_global_word_branch(branch_index)
        .map_err(|error| error.to_string())?;
    let op85 = seq
        .root_op85_node(branch_index)
        .map_err(|error| error.to_string())?;
    let op88 = seq
        .root_op88_node(branch_index)
        .map_err(|error| error.to_string())?;
    let op8f = seq
        .root_op8f_node(branch_index)
        .map_err(|error| error.to_string())?;
    let default_resource_id = state
        .read_u16_le(QUEST_SEQ_DEFAULT_RESOURCE_ID_RUNTIME)
        .map_err(|error| error.to_string())?;
    let effective_resource_id = if op85.record.seq_resource_id() == 0 {
        default_resource_id
    } else {
        u16::from(op85.record.seq_resource_id())
    };
    let renderer_pointers = op85.record.renderer_data_pointers();
    let [Some(renderer_offset_0), Some(renderer_offset_1)] = op85.renderer_target_offsets else {
        return Err("TOWN00 root OP85 renderer pointer가 0임".to_string());
    };
    let state_sha256 = format!("{:x}", Sha256::digest(&state_raw));
    let town00_map = RuntimeAssetMap::new(TOWN00_SEQ_RUNTIME_BASE, TOWN00_SEQ_LEN as u32)
        .ok_or_else(|| "TOWN00 runtime 사상 overflow".to_string())?;
    let mut active_streams = Vec::new();
    for callback_field_runtime in state.find_aligned_u32_le(QUEST_SEQ_INTERPRETER_RUNTIME) {
        let Some(object_runtime) =
            callback_field_runtime.checked_sub(QUEST_SEQ_INTERPRETER_CALLBACK_FIELD_OFFSET)
        else {
            continue;
        };
        let Some(stream_field_runtime) =
            object_runtime.checked_add(QUEST_SEQ_STREAM_POINTER_STATE_OFFSET as u32)
        else {
            continue;
        };
        let stream_runtime = state
            .read_u32_le(stream_field_runtime)
            .map_err(|error| error.to_string())?;
        if let Some(stream_offset) = town00_map.file_offset(stream_runtime) {
            active_streams.push((
                object_runtime,
                stream_field_runtime,
                stream_runtime,
                stream_offset,
            ));
        }
    }

    println!(
        "PASS TOWN00 root state: state SHA-256 {}, Mednafen version 0x{:X}, \
         sections {}, Main RAM {}B, word 0x{:08X}=0x{:08X}, \
         OP0D index {} -> field 0x{:X} -> target 0x{:X} (0x{:08X})",
        state_sha256,
        state.state_version(),
        state.section_count(),
        state.main_ram_bytes(),
        state_word_runtime,
        branch_index,
        target.branch_index,
        target.pointer_field_offset,
        target.target_offset,
        target.target_runtime,
    );
    println!(
        "  OP85 0x{:X}->0x{:X}: SEQ raw/effective {}/{}, MAP {:?}, GRP {:?}, \
         renderer [0x{:08X}->0x{:X}, 0x{:08X}->0x{:X}]",
        op85.stream_offset,
        op85.next_stream_offset,
        op85.record.seq_resource_id(),
        effective_resource_id,
        op85.record.map_resource_ids(),
        op85.record.grp_resource_ids(),
        renderer_pointers[0],
        renderer_offset_0,
        renderer_pointers[1],
        renderer_offset_1,
    );
    println!(
        "  OP88 0x{:X}->0x{:X}: entries raw/effective {}/{}, \
         table 0x{:08X}->0x{:X}..0x{:X}, unconsumed {:02X?}",
        op88.stream_offset,
        op88.next_stream_offset,
        op88.record.raw_entry_count(),
        op88.record.effective_entry_count(),
        op88.record.table_runtime(),
        op88.table_target_offset,
        op88.table_end_offset,
        op88.record.unconsumed_parameters(),
    );
    println!(
        "  OP8F 0x{:X}->0x{:X}: entries raw/effective {}/{}, \
         halfword-pair table 0x{:08X}->0x{:X}..0x{:X}, unconsumed {:02X?}",
        op8f.stream_offset,
        op8f.next_stream_offset,
        op8f.record.raw_entry_count(),
        op8f.record.effective_entry_count(),
        op8f.record.table_runtime(),
        op8f.table_target_offset,
        op8f.table_end_offset,
        op8f.record.unconsumed_parameters(),
    );
    println!(
        "  active TOWN00 interpreter candidates: {}",
        active_streams.len()
    );
    for (object_runtime, stream_field_runtime, stream_runtime, stream_offset) in active_streams {
        let token_input = town00
            .get(stream_offset..)
            .ok_or_else(|| format!("TOWN00 stream offset 범위 이탈: 0x{stream_offset:X}"))?;
        let token =
            parse_quest_token(token_input, stream_runtime).map_err(|error| error.to_string())?;
        let token_summary = match token {
            QuestToken::Glyph(raw) => format!("glyph {:02X?}", raw),
            QuestToken::Control { spec, raw } => {
                format!("control FF{:02X} {}B", spec.code, raw.len())
            }
        };
        println!(
            "    object 0x{object_runtime:08X}, stream field \
             0x{stream_field_runtime:08X}=0x{stream_runtime:08X} -> \
             offset 0x{stream_offset:X}, {token_summary}"
        );
    }
    Ok(())
}

pub(super) fn report_quest_font_boundary(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, state_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };

    let source = VerifiedSourceImage::open(source_path).map_err(|error| error.to_string())?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    verify_box_quest_control_meanings(quest.map, &quest.text).map_err(|error| error.to_string())?;
    verify_box_quest_font_consumer(quest.map, &quest.text).map_err(|error| error.to_string())?;
    verify_box_quest_text_window_consumer(quest.map, &quest.text)
        .map_err(|error| error.to_string())?;
    let extraction = read_quest_seq_text_extraction_analysis(source_path)?;
    let translation_units =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let terminology = verify_box_quest_terminology_candidates(&translation_units)
        .map_err(|error| error.to_string())?;
    let batches =
        verify_box_quest_translation_batches(&extraction, &translation_units, &terminology)
            .map_err(|error| error.to_string())?;

    let state_raw = fs::read(state_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", state_path.display()))?;
    let state = MednafenPsxState::parse(&state_raw).map_err(|error| error.to_string())?;
    let census = inspect_quest_font_windows(&state).map_err(|error| error.to_string())?;
    let scenes = inspect_town00_quest_font_scenes(&state, &extraction, &batches)
        .map_err(|error| error.to_string())?;
    let state_sha256 = format!("{:x}", Sha256::digest(&state_raw));
    println!(
        "PASS QUEST font boundary report: state SHA-256 {}, source-gated slots {}, \
         active {}, selection renderer {}, parent geometry {}, no-active-windows {}",
        state_sha256,
        census.slots.len(),
        census.active_windows,
        census.selection_renderer_slots,
        census.geometry_slots,
        census.has_no_active_windows(),
    );
    for slot in &census.slots {
        println!(
            "  slot {} state 0x{:08X}: parent 0x{:08X}, selector {}, active {}, \
             origin ({},{}), cursor ({},{}), cell {}x{}, line height {}, \
             window gfx {}/{}, spacing {}/{}, \
             input indicator 0x{:08X}, selection renderer 0x{:08X}, geometry {:?}",
            slot.index,
            slot.state_runtime,
            slot.parent_runtime,
            slot.resource_selector,
            slot.active,
            slot.origin_x,
            slot.origin_y,
            slot.cursor_x,
            slot.cursor_y,
            slot.cell_width,
            slot.cell_height,
            slot.line_height,
            slot.window_resource,
            slot.window_cell,
            slot.glyph_spacing,
            slot.line_spacing,
            slot.input_indicator_runtime,
            slot.selection_renderer_runtime,
            slot.geometry,
        );
    }
    println!(
        "  TOWN00 interpreter candidates {}, canonical {}, batch-bound {}, unique batch {:?}",
        scenes.candidates.len(),
        scenes.canonical_stream_candidates,
        scenes.batch_bound_candidates,
        scenes.unique_bound_batch_id(),
    );
    for candidate in &scenes.candidates {
        println!(
            "    object 0x{:08X}, stream 0x{:08X} -> 0x{:X}, owner {:?}..{:?}, \
             scene {:?}, batch {:?}",
            candidate.object_runtime,
            candidate.stream_runtime,
            candidate.stream_offset,
            candidate.record_offset,
            candidate.end_offset,
            candidate.scene_id,
            candidate.batch_id,
        );
    }
    println!(
        "  no-active-windows는 page 교체의 필요조건만 판정하며 scene binding, fresh source, \
         LoadImage/DrawSync 완료를 증명하지 않음; scene/batch는 callback scan 후보 결합임"
    );
    Ok(())
}

pub(super) fn write_quest_font_window_geometry_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, ram_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };

    let source = VerifiedSourceImage::open(source_path).map_err(|error| error.to_string())?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    verify_box_quest_control_meanings(quest.map, &quest.text).map_err(|error| error.to_string())?;
    verify_box_quest_font_consumer(quest.map, &quest.text).map_err(|error| error.to_string())?;
    let consumer = verify_box_quest_text_window_consumer(quest.map, &quest.text)
        .map_err(|error| error.to_string())?;

    let ram =
        fs::read(ram_path).map_err(|error| format!("{} 읽기 실패: {error}", ram_path.display()))?;
    let census = inspect_quest_font_windows_ram(&ram).map_err(|error| error.to_string())?;
    let source_sha256: [u8; 32] = Sha256::digest(source.image().raw_bytes()).into();
    let ram_sha256: [u8; 32] = Sha256::digest(&ram).into();
    let report = render_quest_font_window_geometry_tsv(&census, &source_sha256, &ram_sha256);
    write_report_file(
        output_path,
        "QUEST font window geometry TSV",
        census.slots.len(),
        &report,
    )?;
    println!(
        "  source-gated draw 0x{:08X}, state {}×{}B, active {}, parent geometry {}, \
         selection renderer {}, RAM SHA-256 {}",
        consumer.draw_runtime,
        census.slots.len(),
        consumer.state_record_stride,
        census.active_windows,
        census.geometry_slots,
        census.selection_renderer_slots,
        ram_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    println!(
        "  값은 이 RAM snapshot의 관측이며 target 장면·portrait 경계·layout 승인을 대신하지 않음"
    );
    Ok(())
}

pub(super) fn report_quest_hook_space_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, state_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path).map_err(|error| error.to_string())?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let analysis = verify_box_quest_hook_space_candidates(quest.map, &quest.text)
        .map_err(|error| error.to_string())?;

    let state_raw = fs::read(state_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", state_path.display()))?;
    let state = MednafenPsxState::parse(&state_raw).map_err(|error| error.to_string())?;
    let state_sha256 = format!("{:x}", Sha256::digest(&state_raw));
    let mut changed_candidates = 0usize;
    println!(
        "PASS QUEST hook-space state report: state SHA-256 {}, Mednafen version 0x{:X}, \
         source-gated candidates {}",
        state_sha256,
        state.state_version(),
        analysis.candidates.len(),
    );
    for candidate in &analysis.candidates {
        let ram = state
            .read_bytes(candidate.start_runtime, candidate.bytes)
            .map_err(|error| error.to_string())?;
        let nonzero_bytes = ram.iter().filter(|byte| **byte != 0).count();
        let first_nonzero_runtime = if let Some(offset) = ram.iter().position(|byte| *byte != 0) {
            Some(
                candidate
                    .start_runtime
                    .checked_add(
                        u32::try_from(offset)
                            .map_err(|_| "QUEST hook runtime offset overflow".to_string())?,
                    )
                    .ok_or_else(|| "QUEST hook runtime address overflow".to_string())?,
            )
        } else {
            None
        };
        changed_candidates += usize::from(nonzero_bytes != 0);
        println!(
            "  0x{:08X}..0x{:08X} {}B: static {}, RAM nonzero {} / first {}, \
             runtime {}",
            candidate.start_runtime,
            candidate.end_runtime,
            candidate.bytes,
            candidate.static_verdict.as_str(),
            nonzero_bytes,
            first_nonzero_runtime
                .map(|address| format!("0x{address:08X}"))
                .unwrap_or_else(|| "none".to_string()),
            if nonzero_bytes == 0 {
                "unchanged-in-state-needs-write-exec-observation"
            } else {
                "rejected-written-in-state"
            },
        );
    }
    println!(
        "  changed candidates {}, unchanged candidates {}; unchanged는 cave 승인이 아님",
        changed_candidates,
        analysis.candidates.len() - changed_candidates,
    );
    Ok(())
}
