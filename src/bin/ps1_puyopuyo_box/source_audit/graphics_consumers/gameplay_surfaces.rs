//! 결과 화면, 방해뿌요와 QUEST level-up의 source 소비 경계를 검증한다.

use crate::cli::BoxExecutable;

use ps1_puyopuyo_box::disasm::BtxNameAnalysis;
use ps1_puyopuyo_box::pipeline::{
    verify_box_g_mess_static_owner, verify_gocha1_garbage_route, verify_gocha1_result_name_entry,
    verify_puyo1_garbage_route, verify_puyo2_garbage_route, verify_quest_garbage_route,
    verify_quest_rpg_sys1_level_up,
};

pub(super) fn audit_gameplay_surfaces(
    box_executables: &[BoxExecutable],
    btx_name_consumers: &BtxNameAnalysis,
) -> Result<(), String> {
    let g_mess_owner = verify_box_g_mess_static_owner(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        btx_name_consumers,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS GOCHA1 G_MESS static consumer: resource 0x{:04X}, descriptor 0x{:08X}, \
         stored/plain {}/{}B, setup 0x{:08X}, table 0x{:08X} {}x{} / {} cells + {}B \
         padding, selection rows/groups/unique {}/{}/{}, result builder 0x{:08X} -> \
         common renderers {:08X?}; runtime evidence separate",
        g_mess_owner.resource_id,
        g_mess_owner.descriptor_runtime,
        g_mess_owner.stored_bytes,
        g_mess_owner.plain_bytes,
        g_mess_owner.setup_record_runtime,
        g_mess_owner.cell_table_runtime,
        g_mess_owner.atlas_width,
        g_mess_owner.atlas_height,
        g_mess_owner.cells.len(),
        g_mess_owner.cell_table_padding_bytes,
        g_mess_owner.selection_rows,
        g_mess_owner.selection_groups,
        g_mess_owner.selected_cell_indices.len(),
        g_mess_owner.result_builder_runtime,
        g_mess_owner.renderer_functions,
    );
    let gocha1 = box_executables
        .iter()
        .find(|exe| exe.map.name == "GOCHA1.EXE")
        .ok_or_else(|| "GOCHA1.EXE source 입력이 없음".to_string())?;
    let garbage_route =
        verify_gocha1_garbage_route(gocha1.map, &gocha1.text).map_err(|error| error.to_string())?;
    println!(
        "PASS GOCHA1 garbage route: field objects 0x{:08X} stride 0x{:04X}, \
         cell bytes/row bytes {}/{}, types empty/garbage/wall {}/{}/{}, \
         pending offset 0x{:04X}, opponent pending 0x{:08X} (RAM 0x{:06X}), \
         consumer 0x{:08X}, batch limit {}",
        garbage_route.field_objects_runtime,
        garbage_route.field_object_stride,
        garbage_route.cell_bytes,
        garbage_route.cell_row_bytes,
        garbage_route.empty_cell_type,
        garbage_route.garbage_cell_type,
        garbage_route.wall_cell_type,
        garbage_route.pending_garbage_field_offset,
        garbage_route.opponent_pending_garbage_runtime,
        garbage_route.opponent_pending_garbage_ram_offset,
        garbage_route.incoming_garbage_consumer_runtime,
        garbage_route.garbage_drop_batch_limit,
    );
    let puyo1 = box_executables
        .iter()
        .find(|exe| exe.map.name == "PUYO1.EXE")
        .ok_or_else(|| "PUYO1.EXE source 입력이 없음".to_string())?;
    let puyo1_garbage =
        verify_puyo1_garbage_route(puyo1.map, &puyo1.text).map_err(|error| error.to_string())?;
    println!(
        "PASS PUYO1 garbage route: field objects 0x{:08X} stride 0x{:04X}, \
         cell bytes/row bytes {}/{}, types empty/garbage/wall {}/{}/{}, \
         pending offset/bytes 0x{:04X}/{}, opponent pending 0x{:08X} (RAM 0x{:06X}), \
         consumer 0x{:08X}, batch limit {}",
        puyo1_garbage.field_objects_runtime,
        puyo1_garbage.field_object_stride,
        puyo1_garbage.cell_bytes,
        puyo1_garbage.cell_row_bytes,
        puyo1_garbage.empty_cell_type,
        puyo1_garbage.garbage_cell_type,
        puyo1_garbage.wall_cell_type,
        puyo1_garbage.pending_garbage_field_offset,
        puyo1_garbage.pending_garbage_bytes,
        puyo1_garbage.opponent_pending_garbage_runtime,
        puyo1_garbage.opponent_pending_garbage_ram_offset,
        puyo1_garbage.incoming_garbage_consumer_runtime,
        puyo1_garbage.garbage_drop_batch_limit,
    );
    let puyo2 = box_executables
        .iter()
        .find(|exe| exe.map.name == "PUYO2.EXE")
        .ok_or_else(|| "PUYO2.EXE source 입력이 없음".to_string())?;
    let puyo2_garbage =
        verify_puyo2_garbage_route(puyo2.map, &puyo2.text).map_err(|error| error.to_string())?;
    println!(
        "PASS PUYO2 garbage route: field objects 0x{:08X} stride 0x{:04X}, \
         cell bytes/row bytes {}/{}, types empty/garbage/wall {}/{}/{}, \
         pending offset/bytes 0x{:04X}/{}, opponent pending 0x{:08X} (RAM 0x{:06X}), \
         consumer 0x{:08X}, batch limit {}",
        puyo2_garbage.field_objects_runtime,
        puyo2_garbage.field_object_stride,
        puyo2_garbage.cell_bytes,
        puyo2_garbage.cell_row_bytes,
        puyo2_garbage.empty_cell_type,
        puyo2_garbage.garbage_cell_type,
        puyo2_garbage.wall_cell_type,
        puyo2_garbage.pending_garbage_field_offset,
        puyo2_garbage.pending_garbage_bytes,
        puyo2_garbage.opponent_pending_garbage_runtime,
        puyo2_garbage.opponent_pending_garbage_ram_offset,
        puyo2_garbage.incoming_garbage_consumer_runtime,
        puyo2_garbage.garbage_drop_batch_limit,
    );
    let quest = box_executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE source 입력이 없음".to_string())?;
    let quest_garbage =
        verify_quest_garbage_route(quest.map, &quest.text).map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST garbage route: field objects 0x{:08X} stride 0x{:04X}, \
         allocated/controlled fields {}/{}, callback field +0x{:04X}, \
         cell bytes/row bytes {}/{}, types empty/garbage/wall {}/{}/{}, \
         pending offset/bytes 0x{:04X}/{}, opponent pending 0x{:08X} (RAM 0x{:06X}), \
         consumer 0x{:08X}, batch limit {}",
        quest_garbage.field_objects_runtime,
        quest_garbage.field_object_stride,
        quest_garbage.allocated_field_objects,
        quest_garbage.controlled_battle_fields,
        quest_garbage.callback_field_pointer_offset,
        quest_garbage.cell_bytes,
        quest_garbage.cell_row_bytes,
        quest_garbage.empty_cell_type,
        quest_garbage.garbage_cell_type,
        quest_garbage.wall_cell_type,
        quest_garbage.pending_garbage_field_offset,
        quest_garbage.pending_garbage_bytes,
        quest_garbage.opponent_pending_garbage_runtime,
        quest_garbage.opponent_pending_garbage_ram_offset,
        quest_garbage.incoming_garbage_consumer_runtime,
        quest_garbage.garbage_drop_batch_limit,
    );
    let quest_level_up = verify_quest_rpg_sys1_level_up(quest.map, &quest.text)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST RPG_SYS1 level-up source route: resource {}, object selector {} -> \
         record {} ({}B stride), writer 0x{:08X}, renderer/read 0x{:08X}/0x{:08X}; \
         runtime descriptor/table/bbox/screen evidence separate",
        quest_level_up.resource_id,
        quest_level_up.level_up_cell_selector,
        quest_level_up.level_up_cell_record_index,
        quest_level_up.cell_record_bytes,
        quest_level_up.level_up_object_writer_runtime,
        quest_level_up.short_renderer_runtime,
        quest_level_up.level_up_record_read_runtime,
    );
    let result_name_entry = verify_gocha1_result_name_entry(gocha1.map, &gocha1.text)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS GOCHA1 result name entry: input 0x{:08X}, event masks confirm/erase \
         0x{:04X}/0x{:04X}, slots {}, selected/name/state/position fields \
         0x{:04X}/0x{:04X}/0x{:04X}/0x{:04X}, state store 0x{:08X}",
        result_name_entry.input_update_runtime,
        result_name_entry.confirm_event_mask,
        result_name_entry.erase_event_mask,
        result_name_entry.name_slots,
        result_name_entry.selected_character_field_offset,
        result_name_entry.name_characters_field_offset,
        result_name_entry.state_field_offset,
        result_name_entry.position_field_offset,
        result_name_entry.state_increment_store_runtime,
    );

    Ok(())
}
