//! MM_MES storage, static owner와 MMENU 경유 source route를 검증한다.

use crate::cli::BoxExecutable;

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::graphics::mm_mes_family::verify_box_mm_mes_family;
use ps1_puyopuyo_box::pipeline::{
    verify_box_mm_mes_route, verify_box_mm_mes_static_owner, verify_box_mm_mes_storage_stress,
    verify_box_mmenu_descriptor_ingress, verify_box_mmenu_entry_routes,
    BtxPendingStaticCellAnalysis,
};

pub(super) fn audit_mm_mes(
    box_executables: &[BoxExecutable],
    cnx_archives: &[(&str, VddArchive)],
    subgame_seq_sources: &[(String, String, Vec<u8>)],
    text_surface_cells: &BtxPendingStaticCellAnalysis,
) -> Result<(), String> {
    let mm_mes_family = verify_box_mm_mes_family(
        cnx_archives
            .iter()
            .map(|(archive_path, archive)| (*archive_path, archive)),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS PP1/PP2 MM_MES baked-text surfaces: stored {}/{}B, \
         plain {}B at {}x{}, exact variant {} pixels in ({},{},{},{}); \
         baked bands count/height {}/{} with occupied review runs {}/{}; \
         synthetic 16x16 storage grid {}/{}, active {}/{}, blank {}/{}, unique {}/{}, \
         shared exact {}, variant partitions {:?}, palette histogram {:?}/{:?}; \
         consumer/draw/screen unverified",
        mm_mes_family.pp1_stored_bytes,
        mm_mes_family.pp2_stored_bytes,
        mm_mes_family.plain_bytes,
        mm_mes_family.layout.width,
        mm_mes_family.layout.height,
        mm_mes_family.changed_pixels,
        mm_mes_family.difference_rect.x,
        mm_mes_family.difference_rect.y,
        mm_mes_family.difference_rect.width,
        mm_mes_family.difference_rect.height,
        mm_mes_family.baked_surface.bands,
        mm_mes_family.baked_surface.band_height,
        mm_mes_family
            .baked_surface
            .pp1_occupied_runs
            .iter()
            .map(Vec::len)
            .sum::<usize>(),
        mm_mes_family
            .baked_surface
            .pp2_occupied_runs
            .iter()
            .map(Vec::len)
            .sum::<usize>(),
        mm_mes_family.storage_grid.columns,
        mm_mes_family.storage_grid.rows,
        mm_mes_family.storage_grid.pp1_active_cells,
        mm_mes_family.storage_grid.pp2_active_cells,
        mm_mes_family.storage_grid.pp1_blank_cells,
        mm_mes_family.storage_grid.pp2_blank_cells,
        mm_mes_family.storage_grid.pp1_unique_cells,
        mm_mes_family.storage_grid.pp2_unique_cells,
        mm_mes_family.storage_grid.shared_exact_cells,
        mm_mes_family.storage_grid.variant_cells,
        mm_mes_family.pp1_palette_histogram,
        mm_mes_family.pp2_palette_histogram,
    );
    let mm_mes_storage_stress = verify_box_mm_mes_storage_stress(
        cnx_archives
            .iter()
            .map(|(archive_path, archive)| (*archive_path, archive)),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS PP1/PP2 MM_MES synthetic 512-grid-cell distinct-Hangul storage stress: \
         {mm_mes_storage_stress}"
    );
    let mm_mes_owner = verify_box_mm_mes_static_owner(
        text_surface_cells,
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS PP1/PP2 MM_MES separate static owners: PP1 resource {} descriptor 0x{:08X} \
         setup {}, PP2 resource {} descriptor 0x{:08X}, setup 0x{:08X} -> table 0x{:08X} \
         {}x{}, record+6 {}, valid prefix {} + protected tail {}B, typed handler 0x{:08X}; \
         execution/draw/screen unverified",
        mm_mes_owner.pp1_resource_id,
        mm_mes_owner.pp1_descriptor_runtime,
        mm_mes_owner.pp1_setup_records,
        mm_mes_owner.pp2_resource_id,
        mm_mes_owner.pp2_descriptor_runtime,
        mm_mes_owner.pp2_setup_record_runtime,
        mm_mes_owner.pp2_table_runtime,
        mm_mes_owner.pp2_table_width,
        mm_mes_owner.pp2_table_height,
        mm_mes_owner.pp2_record_plus_6,
        mm_mes_owner.pp2_valid_prefix_cells,
        mm_mes_owner.pp2_protected_tail_bytes,
        mm_mes_owner.pp2_handler_runtime,
    );
    let mm_mes_route = verify_box_mm_mes_route(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS PUYO2 MM_MES lazy-init route: MMENU.SEQ {}B at 0x{:08X}, module root \
         +0x{:X}=0x{:08X}, pre-call OP{:02X} source 0x{:08X} -> default no-advance \
         0x{:08X}, opcode registration batches {} with E8 batch call 0x{:08X} and \
         registration target address-load/raw-pointer {}/{}, initial dispatch 0x{:08X} -> \
         bootstrap 0x{:08X}, direct OP04 root call \
         +0x{:X}=0x{:08X}, root 0x{:08X} \
         from offsets {:X?}, global[0x{:02X}]=0x{:08X}, blocks {}, resource {} setup \
         0x{:08X}; type-1 selectors SEQ records/resources/fallback {}/{}/{}, EXE records/fallback \
         {}/{}, direct resource-62 selections {}/{} across {} PP2 SEQ; \
         MMENU selection/execution/draw/screen unverified",
        mm_mes_route.sequence_bytes,
        mm_mes_route.load_base,
        mm_mes_route.module_root_pointer_field_offset,
        mm_mes_route.module_root_runtime,
        mm_mes_route.pre_call_opcode,
        mm_mes_route.pre_call_source_slot_runtime,
        mm_mes_route.pre_call_default_handler_runtime,
        mm_mes_route.pre_call_registration_batches,
        mm_mes_route.pre_call_registration_runtime,
        mm_mes_route.pre_call_registration_target_address_loads,
        mm_mes_route.pre_call_registration_target_raw_pointers,
        mm_mes_route.pre_call_dispatch_slot_runtime,
        mm_mes_route.pre_call_initial_dispatch_handler_runtime,
        mm_mes_route.root_call_record_offset,
        mm_mes_route.root_call_record_runtime,
        mm_mes_route.root_runtime,
        mm_mes_route.root_pointer_field_offsets,
        mm_mes_route.global_word_index,
        mm_mes_route.global_word_runtime,
        mm_mes_route.blocks.len(),
        mm_mes_route.mm_mes_resource_id,
        mm_mes_route.mm_mes_setup_runtime,
        mm_mes_route.direct_sequence_selector_records,
        mm_mes_route.direct_sequence_selector_resources,
        mm_mes_route.direct_sequence_fallback_records,
        mm_mes_route.direct_executable_selector_records,
        mm_mes_route.direct_executable_fallback_records,
        mm_mes_route.direct_sequence_selections,
        mm_mes_route.direct_executable_selections,
        mm_mes_route.pp2_seq_files,
    );
    let mmenu_entry_route = verify_box_mmenu_entry_routes(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        cnx_archives
            .iter()
            .map(|(archive_path, archive)| (*archive_path, archive)),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS PUYO2 MMENU alternate entry boundary: archive entries SEQ/CEL/total {}/{}/{}, \
         OP12/type1 records SEQ/CEL/EXE {}/{}/{}, \
         known target record forms/SEQ/CEL/EXE {}/{}/{}/{}, argument-only selector entries {}, \
         candidate argument entries/wrappers {}, direct JAL/raw pointer/address-load references all zero \
         across EXE and all PP2 archive entries; runtime indirect call/code-record mutation remains unverified",
        mmenu_entry_route.pp2_seq_files,
        mmenu_entry_route.pp2_cel_files,
        mmenu_entry_route.pp2_archive_entries,
        mmenu_entry_route.alternate_type1_sequence_records,
        mmenu_entry_route.alternate_type1_cell_records,
        mmenu_entry_route.alternate_type1_executable_records,
        mmenu_entry_route.known_selector_record_forms,
        mmenu_entry_route.known_target_sequence_records.len(),
        mmenu_entry_route.known_target_cell_records.len(),
        mmenu_entry_route.known_target_executable_records.len(),
        mmenu_entry_route.argument_only_selector_entries,
        mmenu_entry_route.candidates.len(),
    );
    let mmenu_descriptor_ingress = verify_box_mmenu_descriptor_ingress(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS PUYO2 descriptor literal ingress: type table 0x{:08X}, literal/selectors/alternate/registration \
         {}/{}/{}/{}, selector entry static referenced/unreferenced {}/{}, \
         MMENU/MM_MES descriptor direct address-load/raw-pointer references 0/0 and 0/0; \
         computed aliases, runtime code mutation, indirect calls, selection/draw remain unverified",
        mmenu_descriptor_ingress.type_table_root,
        mmenu_descriptor_ingress.literal_combines.len(),
        mmenu_descriptor_ingress.resource_x36_selectors.len(),
        mmenu_descriptor_ingress.alternate_branch_combines.len(),
        mmenu_descriptor_ingress.registration_stores.len(),
        mmenu_descriptor_ingress.statically_referenced_selectors,
        mmenu_descriptor_ingress.statically_unreferenced_selectors,
    );

    Ok(())
}
