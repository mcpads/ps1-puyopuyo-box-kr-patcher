//! QUEST opcode dispatch, pointer record와 advance 의미를 검증한다.

use ps1_puyopuyo_box::disasm::{
    verify_box_quest_breakable_contact, verify_box_quest_item_acquisition,
    verify_box_quest_item_container, verify_box_quest_item_description_selection,
    verify_box_quest_item_name_callback, verify_box_quest_item_text_consumer,
    verify_box_quest_map_collision, verify_box_quest_player_attack_contact,
    verify_box_quest_player_contact, verify_box_quest_player_interaction_contact,
    verify_box_quest_resident_seq_prerequisite, verify_box_quest_resident_seq_roots,
    verify_box_quest_resident_seq_selector, verify_box_quest_seq_base_direct_advances,
    verify_box_quest_seq_base_record_advances, verify_box_quest_seq_base_transitions,
    verify_box_quest_seq_child_activation, verify_box_quest_seq_child_configuration,
    verify_box_quest_seq_child_stream, verify_box_quest_seq_direct_advances,
    verify_box_quest_seq_flag_query, verify_box_quest_seq_global_word_assignment,
    verify_box_quest_seq_loader, verify_box_quest_seq_non_advance_meanings,
    verify_box_quest_seq_op28, verify_box_quest_seq_pointer_consumer,
    verify_box_quest_seq_registration, QuestItemAcquisitionSummary, QuestSeqChildActivationSummary,
    QuestSeqChildConfigurationSummary, QuestSeqRegistrationSummary,
};

use crate::cli::BoxExecutable;

pub(super) struct QuestVmAudit {
    pub registration: QuestSeqRegistrationSummary,
    pub child_activation: QuestSeqChildActivationSummary,
    pub child_configuration: QuestSeqChildConfigurationSummary,
    pub item_acquisition: QuestItemAcquisitionSummary,
}

pub(super) fn audit_quest_vm(quest_executable: &BoxExecutable) -> Result<QuestVmAudit, String> {
    let registration =
        verify_box_quest_seq_registration(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ registration: root 0x{:08X}, stride {}, prefix rows {}, \
         archive loader 0x{:08X}",
        registration.descriptor_root_runtime,
        registration.descriptor_stride,
        registration.prefix_descriptor_rows,
        registration.archive_loader_runtime,
    );
    let loader = verify_box_quest_seq_loader(quest_executable.map, &quest_executable.text)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ common loader: descriptor slot 0x{:08X} / stride {}, \
         staging 0x{:08X} -> active 0x{:08X}, {} words, stream sites {:?}",
        loader.descriptor_slot_runtime,
        loader.descriptor_stride,
        loader.staging_runtime,
        loader.active_runtime,
        loader.active_copy_words,
        loader.stream_entry_sites,
    );
    let pointer_consumer =
        verify_box_quest_seq_pointer_consumer(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    let child_stream =
        verify_box_quest_seq_child_stream(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    let child_activation =
        verify_box_quest_seq_child_activation(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    let player_contact =
        verify_box_quest_player_contact(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    let player_interaction_contact =
        verify_box_quest_player_interaction_contact(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    let player_attack_contact =
        verify_box_quest_player_attack_contact(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    let breakable_contact =
        verify_box_quest_breakable_contact(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    let map_collision =
        verify_box_quest_map_collision(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    let child_configuration =
        verify_box_quest_seq_child_configuration(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    let flag_query = verify_box_quest_seq_flag_query(quest_executable.map, &quest_executable.text)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ base dispatch: interpreter 0x{:08X}, table 0x{:08X}, \
         {} entries / {} unique handlers / {} fallback entries",
        pointer_consumer.interpreter_runtime,
        pointer_consumer.base_dispatch_table_runtime,
        pointer_consumer.base_dispatch_entries,
        pointer_consumer.unique_base_dispatch_handlers,
        pointer_consumer.base_fallback_dispatch_entries,
    );
    println!(
        "PASS QUEST SEQ pointer consumer: dispatch 0x{:08X} \
         0x{:02X}..0x{:02X} + 0x{:08X} 0x{:02X}..0x{:02X}, \
         {} entries / {} unique handlers / {} fallback entries, \
         {} pointer record specs / {} sites, A1/A2 pc \
         0x{:08X}/0x{:08X}, record +{}, stride {}, stream state +0x{:X}",
        pointer_consumer.dispatch_table_runtime,
        pointer_consumer.dispatch_opcode_base,
        u16::from(pointer_consumer.dispatch_opcode_base)
            + u16::from(pointer_consumer.dispatch_opcode_count)
            - 1,
        pointer_consumer.extended_dispatch_table_runtime,
        pointer_consumer.extended_dispatch_opcode_base,
        u16::from(pointer_consumer.extended_dispatch_opcode_base)
            + u16::from(pointer_consumer.extended_dispatch_opcode_count)
            - 1,
        pointer_consumer.dispatch_entries,
        pointer_consumer.unique_dispatch_handlers,
        pointer_consumer.fallback_dispatch_entries,
        pointer_consumer.pointer_record_specs,
        pointer_consumer.pointer_consumer_sites,
        pointer_consumer.consumer_runtime,
        pointer_consumer.stack_consumer_runtime,
        pointer_consumer.record_pointer_offset,
        pointer_consumer.record_stride,
        pointer_consumer.stream_pointer_state_offset,
    );
    println!(
        "PASS QUEST SEQ OP82 child stream: handler 0x{:08X}, subtype < {}, \
         record +{} / {}B, SUKE subtype 2 arm 0x{:08X}, child store 0x{:08X}",
        child_stream.handler_runtime,
        child_stream.subtype_limit,
        child_stream.pointer_offset,
        child_stream.record_bytes,
        child_stream.subtype2_arm_runtime,
        child_stream.child_stream_store_runtime,
    );
    println!(
        "PASS QUEST SEQ OP82 child activation: subtype 2 arm/callback \
         0x{:08X}/0x{:08X}, state/contact +0x{:X}/+0x{:X}, masks \
         0x{:04X}/0x{:04X}, collision query/writer 0x{:08X}/0x{:08X}, \
         interpreter 0x{:08X}",
        child_activation.subtype2_arm_runtime,
        child_activation.update_callback_runtime,
        child_activation.state_offset,
        child_activation.contact_flags_offset,
        child_activation.direct_contact_mask,
        child_activation.alternate_contact_mask,
        child_activation.collision_query_runtime,
        child_activation.contact_writer_runtime,
        child_activation.interpreter_runtime,
    );
    println!(
        "PASS QUEST player contact: callback/call/writer \
         0x{:08X}/0x{:08X}/0x{:08X}, direction/contact +0x{:X}/+0x{:X}, \
         masks {:04X?}, collision query 0x{:08X}",
        player_contact.update_callback_runtime,
        player_contact.contact_writer_call_runtime,
        player_contact.contact_writer_runtime,
        player_contact.direction_offset,
        player_contact.contact_flags_offset,
        player_contact.direction_contact_masks,
        player_contact.collision_query_runtime,
    );
    println!(
        "PASS QUEST player interaction contact: input/mask 0x{:08X}/0x{:04X}, \
         call/writer/query 0x{:08X}/0x{:08X}/0x{:08X}, direction/contact \
         +0x{:X}/+0x{:X}, vectors {:?}, masks {:04X?}, probe gap/extent/inset/trim \
         {}/{}/{}/{}",
        player_interaction_contact.input_runtime,
        player_interaction_contact.input_mask,
        player_interaction_contact.writer_call_runtime,
        player_interaction_contact.writer_runtime,
        player_interaction_contact.collision_query_runtime,
        player_interaction_contact.direction_offset,
        player_interaction_contact.contact_flags_offset,
        player_interaction_contact.direction_vectors,
        player_interaction_contact.direction_contact_masks,
        player_interaction_contact.probe_gap_pixels,
        player_interaction_contact.probe_extent_pixels,
        player_interaction_contact.probe_cross_axis_inset_pixels,
        player_interaction_contact.probe_cross_axis_extent_trim_pixels,
    );
    println!(
        "PASS QUEST player attack contact: input/mask 0x{:08X}/0x{:04X}, \
         state +0x{:X}={}, direction +0x{:X}, collision call/query \
         0x{:08X}/0x{:08X}, target contact +0x{:X}, masks {:04X?}",
        player_attack_contact.input_runtime,
        player_attack_contact.input_mask,
        player_attack_contact.attack_state_offset,
        player_attack_contact.attack_state,
        player_attack_contact.direction_offset,
        player_attack_contact.collision_call_runtime,
        player_attack_contact.collision_query_runtime,
        player_attack_contact.target_contact_flags_offset,
        player_attack_contact.direction_contact_masks,
    );
    println!(
        "PASS QUEST breakable attack contact: callback 0x{:08X}, state/contact \
         +0x{:X}/+0x{:X}, mask 0x{:04X}, release call/target 0x{:08X}/0x{:08X}",
        breakable_contact.update_callback_runtime,
        breakable_contact.state_offset,
        breakable_contact.contact_flags_offset,
        breakable_contact.attack_contact_mask,
        breakable_contact.release_call_runtime,
        breakable_contact.release_object_runtime,
    );
    println!(
        "PASS QUEST QMP map collision: slots/root/width/height {}/0x{:08X}/0x{:08X}/0x{:08X}, \
         collision plane +0x{:X} ({} cells) -> 0x{:08X}, classifier/movement \
         0x{:08X}/0x{:08X}, low mask 0x{:04X}, flags {:04X?}, classes {}/{}/{}, \
         OP{:02X} conditions {:08X?}",
        map_collision.slot_count,
        map_collision.slot_buffer_roots_runtime,
        map_collision.slot_widths_runtime,
        map_collision.slot_heights_runtime,
        map_collision.collision_plane_offset,
        map_collision.plane_capacity_cells,
        map_collision.collision_buffer_runtime,
        map_collision.classifier_runtime,
        map_collision.movement_runtime,
        map_collision.tile_low_byte_mask,
        map_collision.tile_flag_masks,
        map_collision.none_class,
        map_collision.special_class,
        map_collision.solid_class,
        map_collision.condition_opcode,
        map_collision.condition_runtimes,
    );
    println!(
        "PASS QUEST QMP movement geometry: actor fixed offsets {:X?}, rectangle offsets {:X?}, \
         membership +0x{:X} & 0x{:04X}, fixed shift {}, tile/inset/extent/margin {}/{}/{}/{}, \
         map bounds {:08X?}, normal state/input {}/0x{:08X}, masks {:04X?}, \
         deltas {:?}, map/object/cancel/commit 0x{:08X}/0x{:08X}/0x{:08X}/0x{:08X}",
        map_collision.actor_fixed_position_offsets,
        map_collision.actor_rectangle_offsets,
        map_collision.actor_member_flags_offset,
        map_collision.actor_map_member_mask,
        map_collision.fixed_point_shift,
        map_collision.tile_size_pixels,
        map_collision.tile_interior_inset_pixels,
        map_collision.tile_interior_extent_pixels,
        map_collision.probe_extent_margin_pixels,
        map_collision.map_bounds_runtimes,
        map_collision.player_normal_movement_state,
        map_collision.player_normal_movement_input_runtime,
        map_collision.player_normal_movement_input_masks,
        map_collision.player_normal_movement_deltas_fixed,
        map_collision.player_movement_call_runtime,
        map_collision.player_object_movement_call_runtime,
        map_collision.player_collision_cancel_runtime,
        map_collision.player_position_commit_runtime,
    );
    println!(
        "PASS QUEST SEQ OP83 child configuration: handler/table \
         0x{:08X}/0x{:08X}, subtype < {}, record {}B, child +0x{:X}, \
         position/bounds/motion 0x{:08X}/0x{:08X}/0x{:08X}, fixed shift {}",
        child_configuration.handler_runtime,
        child_configuration.table_runtime,
        child_configuration.subtype_limit,
        child_configuration.record_bytes,
        child_configuration.child_pointer_field_offset,
        child_configuration.position_arm_runtime,
        child_configuration.bounds_arm_runtime,
        child_configuration.motion_arm_runtime,
        child_configuration.fixed_point_shift,
    );
    println!(
        "PASS QUEST SEQ OPA1 flag query: handler/query 0x{:08X}/0x{:08X}, \
         operands +{}/+{}, expected +{}, state base 0x{:08X}, branch pointer +{} / {}B",
        flag_query.handler_runtime,
        flag_query.query_runtime,
        flag_query.flag_base_offset,
        flag_query.flag_offset_offset,
        flag_query.expected_offset,
        flag_query.state_base_runtime,
        flag_query.branch_pointer_offset,
        flag_query.record_bytes,
    );
    let direct_advances =
        verify_box_quest_seq_direct_advances(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    let without_direct_advance = direct_advances
        .without_direct_advance
        .iter()
        .map(|opcode| format!("{opcode:02X}"))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "PASS QUEST SEQ direct advances: {} sites / {} opcodes, widths {:?}, \
         without direct advance [{}]",
        direct_advances.sites,
        direct_advances.opcodes,
        direct_advances.widths,
        without_direct_advance,
    );
    let base_advances =
        verify_box_quest_seq_base_direct_advances(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    let base_without_direct_advance = base_advances
        .without_direct_advance
        .iter()
        .map(|opcode| format!("{opcode:02X}"))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "PASS QUEST SEQ base direct advances: {} sites / {} active opcodes / \
         {} handlers, {} fallback entries, widths {:?}, without direct advance [{}]",
        base_advances.sites,
        base_advances.active_opcodes,
        base_advances.handlers,
        base_advances.fallback_entries,
        base_advances.widths,
        base_without_direct_advance,
    );
    let base_record_advances =
        verify_box_quest_seq_base_record_advances(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ base record-selected advances: {} mode opcodes / mask 0x{:02X}, \
         OP3A {} entries / {} static subopcodes, {} fallback entries",
        base_record_advances.mode_selected_opcodes,
        base_record_advances.mode_mask,
        base_record_advances.op3a_dispatch_entries,
        base_record_advances.op3a_static_advance_subopcodes,
        base_record_advances.fallback_entries,
    );
    let global_word_assignment =
        verify_box_quest_seq_global_word_assignment(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ global word assignment: OP{:02X}/0x{:02X}, {}B, \
         state base 0x{:08X}, operation table/assignment 0x{:08X}/0x{:08X}",
        global_word_assignment.opcode,
        global_word_assignment.mode,
        global_word_assignment.record_bytes,
        global_word_assignment.state_base_runtime,
        global_word_assignment.operation_table_runtime,
        global_word_assignment.assignment_operation_runtime,
    );
    let base_transitions =
        verify_box_quest_seq_base_transitions(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ base transitions: {} branch/return meanings / {} lifecycle meanings / \
         {} stack branches / {} returns / {} branch tables, state/catalog tables \
         0x{:08X}/0x{:08X}, release 0x{:08X} / yield gp+0x{:X}",
        base_transitions.meanings,
        base_transitions.lifecycle_meanings,
        base_transitions.stack_branch_meanings,
        base_transitions.return_meanings,
        base_transitions.branch_table_meanings,
        base_transitions.branch_state_base_runtime,
        base_transitions.catalog_group_table_runtime,
        base_transitions.release_interpreter_runtime,
        base_transitions.dispatch_yield_gp_offset,
    );
    let resident_roots =
        verify_box_quest_resident_seq_roots(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    let literal_roots = resident_roots
        .literal_roots
        .iter()
        .map(|runtime| format!("0x{runtime:08X}"))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "PASS QUEST resident SEQ constructor roots: installs {}, literal sites/roots {}/{}, \
         current-record {} / OP{:02X} handler 0x{:08X}, forwarded-object {}, roots [{}], selector state/table \
         0x{:08X}/0x{:08X}, farewell/defeat {}->0x{:08X}/{}->0x{:08X}",
        resident_roots.installs.len(),
        resident_roots.literal_install_sites,
        resident_roots.literal_roots.len(),
        resident_roots.current_record_pointer_sites,
        resident_roots.current_record_pointer_opcode,
        resident_roots.current_record_pointer_handler_runtime,
        resident_roots.forwarded_object_pointer_sites,
        literal_roots,
        resident_roots.selection.selector_runtime,
        resident_roots.selection.dispatch_table_runtime,
        resident_roots.selection.farewell_selector,
        resident_roots.selection.farewell_handler_runtime,
        resident_roots.selection.defeat_selector,
        resident_roots.selection.defeat_handler_runtime,
    );
    let item_text =
        verify_box_quest_item_text_consumer(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST item text consumers: table 0x{:08X} / {} address loads, \
         name +0x{:X} route 0x{:08X}, description +0x{:X} constructor 0x{:08X}, \
         direct glyph/whole-string sites {}/{}, whole-string draw/glyph return \
         0x{:08X}/0x{:08X}, discarded result {}, direct text mode {}",
        item_text.record_table_runtime,
        item_text.table_address_loads.len(),
        item_text.name_pointer_field_offset,
        item_text.name_draw_route_runtime,
        item_text.description_pointer_field_offset,
        item_text.description_constructor_runtime,
        item_text.direct_name_glyph_call_runtimes.len(),
        item_text.whole_string_name_call_runtimes.len(),
        item_text.whole_string_draw_runtime,
        item_text.whole_string_glyph_return_runtime,
        item_text.whole_string_result_ignored_by_item_callers,
        item_text.direct_text_mode,
    );
    let item_acquisition =
        verify_box_quest_item_acquisition(quest_executable.map, &quest_executable.text, &item_text)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST item acquisition consumer: OP{:02X} 0x{:08X}, submodes {:?}, \
         record item/parameter +{}/+{}, state machine 0x{:08X}, whole-string sites {:?}, \
         fixed SJIS fragments {}",
        item_acquisition.opcode,
        item_acquisition.handler_runtime,
        item_acquisition.whole_string_submodes,
        item_acquisition.record_item_id_offset,
        item_acquisition.record_parameter_offset,
        item_acquisition.state_machine_runtime,
        item_acquisition.whole_string_call_runtimes,
        item_acquisition.fixed_messages.len(),
    );
    let item_container =
        verify_box_quest_item_container(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST OP{:02X} item container: handler/callback \
         0x{:08X}/0x{:08X}, type {}, record {}B, local rectangle {:?}, \
         open direction/mask {}/0x{:04X}, attack mask 0x{:04X}, acquisition 0x{:08X}",
        item_container.opcode,
        item_container.handler_runtime,
        item_container.callback_runtime,
        item_container.object_type,
        item_container.record_bytes,
        item_container.local_rectangle,
        item_container.open_direction,
        item_container.open_contact_mask,
        item_container.attack_reaction_mask,
        item_container.item_acquisition_state_machine_runtime,
    );
    let item_description_selection =
        verify_box_quest_item_description_selection(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST item description selection: constructor callers {}, cursor paths {}, \
         cursor result {}, same-iteration recreation {}, later no-input iteration required {}",
        item_description_selection.constructor_call_runtimes.len(),
        item_description_selection.selection_paths.len(),
        item_description_selection.cursor_changed_result,
        item_description_selection.cursor_change_recreates_in_same_iteration,
        item_description_selection.recreation_requires_later_no_input_iteration,
    );
    let item_name_callback =
        verify_box_quest_item_name_callback(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST item name callback: callback 0x{:08X}, state +0x{:X}={}, \
         residency site/continuation 0x{:08X}/0x{:08X}, renderer returns \
         item/fallback 0x{:08X}/0x{:08X}, retry before state advance {}",
        item_name_callback.callback_runtime,
        item_name_callback.state_offset,
        item_name_callback.draw_state,
        item_name_callback.residency_site_runtime,
        item_name_callback.residency_continuation_runtime,
        item_name_callback.glyph_writer_return_runtime,
        item_name_callback.fallback_glyph_writer_return_runtime,
        item_name_callback.retry_boundary_before_state_advance,
    );
    let resident_prerequisite =
        verify_box_quest_resident_seq_prerequisite(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST resident SEQ defeat prerequisite: state 0x{:08X}+{} / {} bytes, \
         initial {}, decrement/increment 0x{:08X}/0x{:08X}, snapshot capture/restore {}/{}",
        resident_prerequisite.state_runtime,
        resident_prerequisite.prerequisite_offset,
        resident_prerequisite.state_bytes,
        resident_prerequisite.initial_value,
        resident_prerequisite.decrement_store_runtime,
        resident_prerequisite.increment_store_runtime,
        resident_prerequisite.snapshot_capture_loads.len(),
        resident_prerequisite.snapshot_restore_stores.len(),
    );
    let resident_selector = verify_box_quest_resident_seq_selector(
        quest_executable.map,
        &quest_executable.text,
        &resident_roots.selection,
    )
    .map_err(|error| error.to_string())?;
    let defeat_writer_sites = resident_selector
        .defeat_writers
        .iter()
        .map(|writer| {
            format!(
                "0x{:08X}->0x{:08X}",
                writer.callback_runtime, writer.selector_store_runtime
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "PASS QUEST resident SEQ selector producers: reads/writes {}/{}, defeat value {} \
         writers [{}], prerequisite 0x{:08X}, standalone constructor 0x{:08X} direct \
         control/address/aligned-pointer refs {}/{}/{}",
        resident_selector.reads.len(),
        resident_selector.writes.len(),
        resident_roots.selection.defeat_selector,
        defeat_writer_sites,
        resident_selector.defeat_prerequisite_runtime,
        resident_selector.standalone_defeat_constructor_runtime,
        resident_selector.standalone_direct_control_refs.len(),
        resident_selector.standalone_address_load_refs.len(),
        resident_selector.standalone_aligned_pointer_refs.len(),
    );
    let op28 = verify_box_quest_seq_op28(quest_executable.map, &quest_executable.text)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ OP28 sub-dispatch: {} entries / {} unique handlers / \
         {} fallback entries, {} direct sites / {} handlers, {} branch subopcodes, \
         widths {:?}, dispatcher/table 0x{:08X}/0x{:08X}",
        op28.dispatch_entries,
        op28.unique_handlers,
        op28.fallback_entries,
        op28.direct_advance_sites,
        op28.direct_advance_handlers,
        op28.branch_subopcodes,
        op28.widths,
        op28.dispatch_runtime,
        op28.dispatch_table_runtime,
    );
    let non_advance =
        verify_box_quest_seq_non_advance_meanings(quest_executable.map, &quest_executable.text)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ non-advance meanings: {} meanings, OP8A RPG_SEQ slot \
         0x{:08X} + resource*{}, ID 0 default 0x{:08X} -> active 0x{:08X} via 0x{:08X}, \
         preset stream 0x{:08X}, release/allocate 0x{:08X}/0x{:08X}",
        non_advance.meanings,
        non_advance.seq_descriptor_slot_runtime,
        non_advance.descriptor_stride,
        non_advance.default_resource_id_runtime,
        non_advance.active_stream_runtime,
        non_advance.resource_ready_runtime,
        non_advance.preset_stream_runtime,
        non_advance.release_object_runtime,
        non_advance.allocate_object_runtime,
    );
    println!(
        "PASS TOWN00 OP85 prefix consumer: entry 0x{:08X}, handler 0x{:08X}, \
         state 0x{:08X}, selector +0x{:X}, halfwords {:?}, render pointers {:?}, \
         stride {}",
        pointer_consumer.prefix_entry_runtime,
        pointer_consumer.op85_consumer_runtime,
        pointer_consumer.op85_state_runtime,
        pointer_consumer.op85_selector_state_offset,
        pointer_consumer.op85_halfword_state_offsets,
        pointer_consumer.op85_render_pointer_state_offsets,
        pointer_consumer.op85_record_stride,
    );
    println!(
        "PASS QUEST resource slots: GRP 0x{:08X}->0x{:08X}, \
         SEQ 0x{:08X}->0x{:08X}, MAP 0x{:08X}->0x{:08X}; \
         OP85 MAP loader 0x{:08X}, GRP loader 0x{:08X}, \
         copy source [0x{:08X}, 0x{:08X}] -> staging [0x{:08X}, 0x{:08X}], \
         copy words [0x{:X}, 0x{:X}] / bytes [0x{:X}, 0x{:X}]",
        pointer_consumer.grp_descriptor_slot_runtime,
        pointer_consumer.grp_descriptor_root_runtime,
        pointer_consumer.seq_descriptor_slot_runtime,
        pointer_consumer.seq_descriptor_root_runtime,
        pointer_consumer.map_descriptor_slot_runtime,
        pointer_consumer.map_descriptor_root_runtime,
        pointer_consumer.op85_map_loader_runtime,
        pointer_consumer.op85_grp_loader_runtime,
        pointer_consumer.op85_grp_copy_source_runtime[0],
        pointer_consumer.op85_grp_copy_source_runtime[1],
        pointer_consumer.op85_grp_staging_runtime[0],
        pointer_consumer.op85_grp_staging_runtime[1],
        pointer_consumer.op85_grp_copy_words[0],
        pointer_consumer.op85_grp_copy_words[1],
        pointer_consumer.op85_grp_copy_words[0] * 4,
        pointer_consumer.op85_grp_copy_words[1] * 4,
    );

    Ok(QuestVmAudit {
        registration,
        child_activation,
        child_configuration,
        item_acquisition,
    })
}
