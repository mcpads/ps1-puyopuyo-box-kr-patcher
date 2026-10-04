//! EXE loader, text consumer, font route와 QUEST 실행 코드를 검증한다.

use ps1_puyopuyo_box::disasm::{
    verify_box_bios_font_routes, verify_box_btx_cell_consumers, verify_box_cnx_consumers,
    verify_box_common_font_draw_sites, verify_box_exe_string_candidates,
    verify_box_executable_traces, verify_box_quest_control_meanings,
    verify_box_quest_font_consumer, verify_box_quest_font_hook_site, verify_box_quest_hook_space,
    verify_box_quest_hook_space_candidates, verify_box_quest_input_wait,
    verify_box_quest_page_source, verify_box_quest_sound_sequence_consumer,
    verify_box_quest_text_mode, verify_box_quest_text_window_consumer, verify_box_shell_exec_path,
    verify_box_subgame_seq_controls, verify_box_subgame_text_consumers,
    verify_quest_common_font_direct_draw, SubgameControlBehavior,
};
use ps1_puyopuyo_box::disc::verify_box_32m_reservation;
use ps1_puyopuyo_box::pipeline::{
    verify_quest_hook_noop_sources, verify_quest_page_hook_minimum_plan,
};
use ps1_puyopuyo_box::script::{
    verify_quest_control_dispatch, QUEST_CONTROL_DISPATCH_BYTES, QUEST_CONTROL_DISPATCH_RUNTIME,
};

use super::{ExecutableConsumerAuditData, SourceAuditContext};

pub(super) fn audit_executable_consumers(
    context: &SourceAuditContext<'_>,
) -> Result<ExecutableConsumerAuditData, String> {
    let image = context.source.image();
    let iso = &context.iso;
    let box_executables = &context.box_executables;
    let reserved = verify_box_32m_reservation(
        iso,
        image,
        box_executables
            .iter()
            .map(|exe| (exe.map.name, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS 32M.BIN reservation: LBA {}..{}, {}B / {} Form1 sectors, \
         nonzero {}, EXE direct filename literals {}, volume tail {} sectors",
        reserved.lba,
        reserved.end_lba,
        reserved.bytes,
        reserved.form1_sectors,
        reserved.nonzero_bytes,
        reserved.executable_filename_literals,
        reserved.volume_tail_sectors,
    );
    let shell_executable = box_executables
        .iter()
        .find(|exe| exe.map.name == "SLPS_031.14")
        .ok_or_else(|| "SLPS_031.14 검증 입력이 없음".to_string())?;
    let shell_exec = verify_box_shell_exec_path(shell_executable.map, &shell_executable.text)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS shell EXE loader: selectors {} + defaults {}, QUEST selector {} \
         path 0x{:08X}, open 0x{:08X}, load state 0x{:08X}, \
         exec loader 0x{:08X} -> BIOS A(0x{:02X})",
        shell_exec.selectors,
        shell_exec.default_selectors,
        shell_exec.quest_selector,
        shell_exec.quest_path_runtime,
        shell_exec.open_function,
        shell_exec.load_state_function,
        shell_exec.exec_loader_function,
        shell_exec.bios_exec_function,
    );
    let executable_traces = verify_box_executable_traces(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS EXE recursive traces: EXE {}, reached instructions {}, unknown {}, \
         indirect sites {}, out-of-range edges {}",
        executable_traces.len(),
        executable_traces
            .iter()
            .map(|summary| summary.reached_instructions)
            .sum::<usize>(),
        executable_traces
            .iter()
            .map(|summary| summary.unknown_instructions)
            .sum::<usize>(),
        executable_traces
            .iter()
            .map(|summary| summary.indirect_sites)
            .sum::<usize>(),
        executable_traces
            .iter()
            .map(|summary| summary.out_of_range_edges)
            .sum::<usize>(),
    );
    let exe_strings = verify_box_exe_string_candidates(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let exe_string_analysis_sha256 = exe_strings
        .analysis_sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    println!(
        "PASS EXE string candidates: EXE {}, candidates {}, code units {}, strict {}, \
         heuristic-only {}, static refs {}, entry refs {}, entry-code overlaps {}, \
         analysis SHA-256 {}",
        exe_strings.summaries.len(),
        exe_strings.candidates.len(),
        exe_strings
            .summaries
            .iter()
            .map(|summary| summary.code_units)
            .sum::<usize>(),
        exe_strings
            .summaries
            .iter()
            .map(|summary| summary.strict_valid_candidates)
            .sum::<usize>(),
        exe_strings
            .summaries
            .iter()
            .map(|summary| summary.heuristic_only_candidates)
            .sum::<usize>(),
        exe_strings
            .summaries
            .iter()
            .map(|summary| summary.statically_referenced_candidates)
            .sum::<usize>(),
        exe_strings
            .summaries
            .iter()
            .map(|summary| summary.entry_referenced_candidates)
            .sum::<usize>(),
        exe_strings
            .summaries
            .iter()
            .map(|summary| summary.entry_code_overlap_candidates)
            .sum::<usize>(),
        exe_string_analysis_sha256,
    );
    let bios_font = verify_box_bios_font_routes(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS BIOS font routes: EXE {}, B(0x51) routes {}, entry-reached callers {}, \
         wrapper callers {}, entry-reached wrapper callers {}, callsite comparisons {}",
        bios_font.routes.len(),
        bios_font
            .routes
            .iter()
            .map(|route| route.stubs.len())
            .sum::<usize>(),
        bios_font
            .routes
            .iter()
            .map(|route| route.entry_reached_callers.len())
            .sum::<usize>(),
        bios_font
            .routes
            .iter()
            .map(|route| route.wrapper_callers.len())
            .sum::<usize>(),
        bios_font
            .routes
            .iter()
            .map(|route| route.entry_reached_wrapper_callers.len())
            .sum::<usize>(),
        bios_font.callsite_similarities.len(),
    );
    let subgame_text_consumers = verify_box_subgame_text_consumers(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame SEQ text consumers: EXE {}, consumers {}, u16/+2 {}, FF00-FF2F tables {} × 48, \
         defined/zero slots {}/{}, unique handlers {}, BIOS glyph-cache routes {}, \
         entry-reached consumers {}",
        subgame_text_consumers.control_tables.len(),
        subgame_text_consumers.consumers.len(),
        subgame_text_consumers.consumers.len(),
        subgame_text_consumers.control_tables.len(),
        subgame_text_consumers
            .control_tables
            .iter()
            .map(|table| table.defined_entries)
            .sum::<usize>(),
        subgame_text_consumers
            .control_tables
            .iter()
            .map(|table| table.zero_entries)
            .sum::<usize>(),
        subgame_text_consumers
            .control_tables
            .iter()
            .map(|table| table.unique_handlers)
            .sum::<usize>(),
        subgame_text_consumers.control_tables.len(),
        subgame_text_consumers
            .consumers
            .iter()
            .filter(|consumer| consumer.entry_reached)
            .count(),
    );
    let subgame_controls = verify_box_subgame_seq_controls(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        &subgame_text_consumers,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame SEQ controls: sites {}, no-advance {}, sequential {}, dynamic-select {}, \
         branch/call/return/commit {}/{}/{}/{}, max inline argument {}B",
        subgame_controls.sites.len(),
        subgame_controls
            .sites
            .iter()
            .filter(|site| site.behavior == SubgameControlBehavior::NoAdvance)
            .count(),
        subgame_controls
            .sites
            .iter()
            .filter(|site| matches!(site.behavior, SubgameControlBehavior::Sequential { .. }))
            .count(),
        subgame_controls
            .sites
            .iter()
            .filter(|site| matches!(site.behavior, SubgameControlBehavior::DynamicSelect { .. }))
            .count(),
        subgame_controls
            .sites
            .iter()
            .filter(|site| matches!(site.behavior, SubgameControlBehavior::BranchAbsolute { .. }))
            .count(),
        subgame_controls
            .sites
            .iter()
            .filter(|site| matches!(site.behavior, SubgameControlBehavior::CallAbsolute { .. }))
            .count(),
        subgame_controls
            .sites
            .iter()
            .filter(|site| site.behavior == SubgameControlBehavior::Return)
            .count(),
        subgame_controls
            .sites
            .iter()
            .filter(|site| site.behavior == SubgameControlBehavior::CommitObject)
            .count(),
        subgame_controls
            .sites
            .iter()
            .map(|site| site.behavior.argument_bytes())
            .max()
            .unwrap_or(0),
    );
    let btx_cells = verify_box_btx_cell_consumers(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let btx_cell_digest = btx_cells
        .analysis_sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    println!(
        "PASS BTX cell consumers: EXE {}, lookup cores {}, entry-reached {}, \
         renderer cores {}, analysis SHA-256 {}",
        btx_cells.summaries.len(),
        btx_cells.consumers.len(),
        btx_cells
            .consumers
            .iter()
            .filter(|consumer| consumer.core_entry_reached)
            .count(),
        btx_cells
            .consumers
            .iter()
            .filter(|consumer| consumer.renderer_family.is_some())
            .count(),
        btx_cell_digest,
    );
    let common_font_draw_sites = verify_box_common_font_draw_sites(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        &btx_cells,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS COM_FONT draw capture sites: EXE {}, lookup/primitive pairs {}, \
         live register roles {}",
        common_font_draw_sites.sites.len(),
        common_font_draw_sites.sites.len(),
        common_font_draw_sites.sites.len() * 5,
    );
    let cnx_consumers = verify_box_cnx_consumers(
        box_executables
            .iter()
            .map(|exe| (exe.map.name, exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS CNX consumers: EXE {}, magic checks {}, decode paths {}, wrapper calls {}, \
         selector initializers {}, selectors {}/{}, cores {}, families {} (shared {}, PCOMI {})",
        cnx_consumers.executables,
        cnx_consumers.magic_checks,
        cnx_consumers.decode_paths,
        cnx_consumers.wrapper_calls,
        cnx_consumers.selector_initializer_calls,
        cnx_consumers.selector_loads,
        cnx_consumers.selector_enable_stores,
        cnx_consumers.cores,
        cnx_consumers.families,
        cnx_consumers.shared_family_executables,
        cnx_consumers.pcomi_family_executables
    );

    let quest_executable = box_executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let quest_map = quest_executable.map;
    let quest_text = &quest_executable.text;
    let quest_sound_sequence = verify_box_quest_sound_sequence_consumer(quest_map, quest_text)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST sound sequence consumer: descriptor 0x{:08X} / {}B, load 0x{:08X}, \
         open 0x{:08X}, app slot 0x{:08X} / {}B, start 0x{:08X} -> 0x{:08X}",
        quest_sound_sequence.sequence_descriptor_runtime,
        quest_sound_sequence.sequence_bytes,
        quest_sound_sequence.load_destination_runtime,
        quest_sound_sequence.sequence_open_runtime,
        quest_sound_sequence.application_slot_runtime,
        quest_sound_sequence.application_slot_stride,
        quest_sound_sequence.sequence_start_runtime,
        quest_sound_sequence.low_level_start_core_runtime,
    );
    let quest_chain_draw = verify_quest_common_font_direct_draw(quest_map, quest_text)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST COM_FONT direct draw: surface {}, writer 0x{:08X}, callback 0x{:08X}, \
         commit 0x{:08X}, cell/resource {}/{}, command buffer 0x{:08X}",
        quest_chain_draw.surface_id,
        quest_chain_draw.writer_runtime,
        quest_chain_draw.function_runtime,
        quest_chain_draw.commit_runtime,
        quest_chain_draw.cell,
        quest_chain_draw.resource,
        quest_chain_draw.command_buffer_runtime,
    );
    let dispatch_offset = quest_map
        .text_offset(QUEST_CONTROL_DISPATCH_RUNTIME)
        .ok_or_else(|| "QUEST control dispatch가 QUEST.EXE text 범위 밖임".to_string())?;
    let dispatch_end = dispatch_offset
        .checked_add(QUEST_CONTROL_DISPATCH_BYTES)
        .ok_or_else(|| "QUEST control dispatch 범위 overflow".to_string())?;
    let dispatch = quest_text
        .get(dispatch_offset..dispatch_end)
        .ok_or_else(|| "QUEST control dispatch slice 범위 이탈".to_string())?;
    let quest_controls =
        verify_quest_control_dispatch(dispatch).map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST control dispatch: {} controls / {} unique handlers",
        quest_controls.controls, quest_controls.unique_handlers
    );
    let quest_control_meanings = verify_box_quest_control_meanings(quest_map, quest_text)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST control meanings: {} confirmed; line/open/close \
         0x{:08X}/0x{:08X}/0x{:08X}, object allocate/release \
         0x{:08X}/0x{:08X}",
        quest_control_meanings.confirmed_meanings,
        quest_control_meanings.advance_line_runtime,
        quest_control_meanings.open_text_window_runtime,
        quest_control_meanings.close_text_window_runtime,
        quest_control_meanings.allocate_object_runtime,
        quest_control_meanings.release_object_runtime,
    );
    let quest_input_wait =
        verify_box_quest_input_wait(quest_map, quest_text).map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST FF0F input wait: handler/start/poll \
         0x{:08X}/0x{:08X}/0x{:08X}, stream {}B, state +0x{:X}={} / \
         indicator +0x{:X}, input mask 0x{:04X}, cursor·active writes {}, \
         known window transition calls {}",
        quest_input_wait.handler_runtime,
        quest_input_wait.start_runtime,
        quest_input_wait.poll_runtime,
        quest_input_wait.stream_bytes,
        ps1_puyopuyo_box::disasm::QUEST_INPUT_WAIT_STATE_OFFSET,
        quest_input_wait.initial_count,
        ps1_puyopuyo_box::disasm::QUEST_INPUT_WAIT_INDICATOR_OFFSET,
        quest_input_wait.button_mask,
        quest_input_wait.cursor_or_window_activation_writes,
        quest_input_wait.known_window_transition_calls,
    );
    let quest_text_mode =
        verify_box_quest_text_mode(quest_map, quest_text).map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST OP21 text mode: opcode 0x{:02X} / {}B, handler 0x{:08X} -> \
         interpreter 0x{:08X} -> consumer 0x{:08X} -> glyph renderer 0x{:08X}, \
         state offsets active +0x{:X} / stream +0x{:X}",
        quest_text_mode.opcode,
        quest_text_mode.record_bytes,
        quest_text_mode.handler_runtime,
        quest_text_mode.interpreter_runtime,
        quest_text_mode.consumer_runtime,
        quest_text_mode.glyph_renderer_runtime,
        quest_text_mode.active_word_offset,
        quest_text_mode.stream_pointer_offset,
    );
    let quest_font_hook = verify_box_quest_font_hook_site(quest_map, quest_text)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST font hook site candidate: profile {}, handler 0x{:08X} / {} words, \
         overwrite {}B, preserved delay 0x{:08X}, continuation 0x{:08X}, \
         caller jalr/return 0x{:08X}/0x{:08X}, live object/return {}/{}",
        quest_font_hook.profile,
        quest_font_hook.site_runtime,
        quest_font_hook.handler_words,
        quest_font_hook.overwrite_bytes,
        quest_font_hook.preserved_delay_runtime,
        quest_font_hook.continuation_runtime,
        quest_font_hook.caller_jalr_runtime,
        quest_font_hook.caller_return_runtime,
        quest_font_hook.object_register,
        quest_font_hook.return_register,
    );
    let quest_hook_space =
        verify_box_quest_hook_space(quest_map, quest_text).map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST hook tail rejection: profile {}, initialized end 0x{:08X}, \
         alignment padding {}B, startup clear 0x{:08X}..0x{:08X} / {}B \
         (text overlap {}B), text end 0x{:08X}, gp 0x{:08X}, mutable selector 0x{:08X}, \
         embedded zero section 0x{:08X} / {}B",
        quest_hook_space.profile,
        quest_hook_space.initialized_end_runtime,
        quest_hook_space.alignment_padding_bytes,
        quest_hook_space.zero_clear_start_runtime,
        quest_hook_space.zero_clear_end_runtime,
        quest_hook_space.zero_cleared_bytes,
        quest_hook_space.zero_cleared_text_overlap_bytes,
        quest_hook_space.text_end_runtime,
        quest_hook_space.gp_runtime,
        quest_hook_space.mutable_selector_runtime,
        quest_hook_space.section_zero_start_runtime,
        quest_hook_space.section_zero_bytes,
    );
    let quest_hook_candidates = verify_box_quest_hook_space_candidates(quest_map, quest_text)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST hook zero-run candidates: >=64B {} runs / {}B, \
         entry-reached {}, referenced {}, unreferenced runtime-pending {}, \
         open indirect sites {}, analysis SHA-256 {}",
        quest_hook_candidates.candidates.len(),
        quest_hook_candidates.total_zero_bytes,
        quest_hook_candidates.entry_reached_candidates,
        quest_hook_candidates.referenced_candidates,
        quest_hook_candidates.unreferenced_candidates,
        quest_hook_candidates.indirect_sites,
        quest_hook_candidates
            .analysis_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    let quest_hook_noop =
        verify_quest_hook_noop_sources(quest_map, quest_text, &quest_hook_candidates)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST no-op hook typed source: site 0x{:08X} / {} word, \
         body 0x{:08X} / {} words, reserved {}B, continuation 0x{:08X}",
        quest_hook_noop.site.base_addr(),
        quest_hook_noop.site.len_words(),
        quest_hook_noop.body.base_addr(),
        quest_hook_noop.body.len_words(),
        quest_hook_noop.reserved_bytes,
        ps1_puyopuyo_box::disasm::QUEST_FONT_HOOK_CONTINUATION_RUNTIME,
    );
    let quest_page_source =
        verify_box_quest_page_source(quest_map, quest_text).map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST page source dependency: profile {}, direct wrapper/call/core \
         0x{:08X}/0x{:08X}/0x{:08X}, streaming wrapper/core call 0x{:08X}/0x{:08X}, \
         shared gp fields {}, loader arena 0x{:08X}..0x{:08X} / {}B",
        quest_page_source.profile,
        quest_page_source.direct_wrapper_runtime,
        quest_page_source.direct_call_runtime,
        quest_page_source.core_runtime,
        quest_page_source.streaming_wrapper_runtime,
        quest_page_source.streaming_core_call_runtime,
        quest_page_source.shared_gp_state_fields,
        quest_page_source.arena_start_runtime,
        quest_page_source.arena_end_runtime,
        quest_page_source.arena_bytes,
    );
    let quest_page_hook =
        verify_quest_page_hook_minimum_plan(quest_map, quest_text, &quest_hook_candidates)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST page hook minimum budget: schema {}, unbound body 0x{:08X} / \
         {} words / {}B, reserved {}B, state 0x{:08X}, raw page {}B, \
         source 0x{:08X}, sync/commit {}/{}, build-eligible {}",
        quest_page_hook.schema_version,
        quest_page_hook.body.base_addr(),
        quest_page_hook.body.len_words(),
        quest_page_hook.body_bytes,
        quest_page_hook.reserved_bytes,
        quest_page_hook.state_runtime,
        quest_page_hook.page_bytes,
        quest_page_hook.source_runtime,
        quest_page_hook.waits_for_draw_sync,
        quest_page_hook.commits_state_after_sync,
        quest_page_hook.build_eligible,
    );
    let quest_font_consumer =
        verify_box_quest_font_consumer(quest_map, quest_text).map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST font consumer: renderer 0x{:08X}, code table 0x{:08X} / {} entries, \
         fallback {}, object cell +0x{:X}, resource state +0x{:X}, descriptor cell table \
         +0x{:X}, cell stride {}; state init 0x{:08X}, direct calls {} \
         (boot 0x{:08X} / conditional reset 0x{:08X}), base 0x{:08X}, \
         {} records × {}B, selector {}; descriptor 0x{:08X} from root 0x{:08X} \
         / {}B, copy 0x{:08X}, source field initial 0x{:08X}, name 0x{:08X}, \
         compressed/decoded {}/{}B, source stores 0x{:08X}/0x{:08X}, \
         cell table candidate 0x{:08X}, pixel format {}, upload shape {}×{} words/rows, \
         upload callback/call/target \
         0x{:08X}/0x{:08X}/0x{:08X}, sync call/target 0x{:08X}/0x{:08X}",
        quest_font_consumer.renderer_runtime,
        quest_font_consumer.code_table_runtime,
        quest_font_consumer.code_table_entries,
        quest_font_consumer.fallback_index,
        quest_font_consumer.cell_index_object_offset,
        quest_font_consumer.resource_selector_state_offset,
        quest_font_consumer.cell_table_descriptor_offset,
        quest_font_consumer.cell_record_stride,
        quest_font_consumer.state_initializer_runtime,
        quest_font_consumer.state_initializer_direct_calls,
        quest_font_consumer.state_boot_call_runtime,
        quest_font_consumer.state_conditional_reset_call_runtime,
        quest_font_consumer.state_base_runtime,
        quest_font_consumer.state_record_count,
        quest_font_consumer.state_record_stride,
        quest_font_consumer.initial_resource_selector,
        quest_font_consumer.descriptor_runtime,
        quest_font_consumer.descriptor_root_runtime,
        quest_font_consumer.descriptor_stride,
        quest_font_consumer.descriptor_copy_runtime,
        quest_font_consumer.source_field_initial,
        quest_font_consumer.asset_name_runtime,
        quest_font_consumer.compressed_bytes,
        quest_font_consumer.source_bytes,
        quest_font_consumer.source_staging_store_runtime,
        quest_font_consumer.source_decoded_store_runtime,
        quest_font_consumer.cell_table_runtime,
        quest_font_consumer.pixel_format,
        quest_font_consumer.vram_width_words,
        quest_font_consumer.vram_height,
        quest_font_consumer.upload_callback_runtime,
        quest_font_consumer.upload_call_runtime,
        quest_font_consumer.upload_target_runtime,
        quest_font_consumer.upload_sync_call_runtime,
        quest_font_consumer.upload_sync_target_runtime,
    );
    let quest_text_window = verify_box_quest_text_window_consumer(quest_map, quest_text)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST text window consumer: draw 0x{:08X}, state base 0x{:08X} / {}B stride, \
         parent state +0x{:X}, parent fixed12 x/y +0x{:X}/+0x{:X}, extent +0x{:X}/+0x{:X}, \
         window resource/cell +0x{:X}/+0x{:X}, frame outset {}px, tile {}px",
        quest_text_window.draw_runtime,
        quest_text_window.state_base_runtime,
        quest_text_window.state_record_stride,
        quest_text_window.parent_state_offset,
        quest_text_window.parent_x_fixed12_offset,
        quest_text_window.parent_y_fixed12_offset,
        quest_text_window.parent_width_offset,
        quest_text_window.parent_height_offset,
        quest_text_window.resource_state_offset,
        quest_text_window.cell_state_offset,
        quest_text_window.frame_outset_px,
        quest_text_window.tile_step_px,
    );

    Ok(ExecutableConsumerAuditData {
        reserved,
        bios_font,
        subgame_text_consumers,
        subgame_controls,
        btx_cells,
        common_font_draw_sites,
        quest_font_consumer,
    })
}
