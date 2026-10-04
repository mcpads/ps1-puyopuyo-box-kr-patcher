//! PUYO1 classic BTX descriptor 등록 경계를 검증한다.

use crate::cli::BoxExecutable;

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::disasm::{BtxDescriptorSelectionAnalysis, BtxNameAnalysis};
use ps1_puyopuyo_box::pipeline::verify_box_classic_btx_registration;

pub(super) fn audit_classic_registration(
    box_executables: &[BoxExecutable],
    cnx_archives: &[(&str, VddArchive)],
    btx_name_consumers: &BtxNameAnalysis,
    pending_btx_selections: &BtxDescriptorSelectionAnalysis,
) -> Result<(), String> {
    let classic_btx_registration = verify_box_classic_btx_registration(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        cnx_archives
            .iter()
            .map(|(archive_path, archive)| (*archive_path, archive)),
        btx_name_consumers,
        pending_btx_selections,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS PUYO1 classic BTX registration: type {}, OP29 0x{:08X} -> handler/loader \
        0x{:08X}/0x{:08X}, descriptor table 0x{:08X}, prefix rows {}, targets {:?}; \
         selector resource*36/descriptor/copy 0x{:08X}/0x{:08X}/0x{:08X}..0x{:08X}; \
         target selection/upload/draw remain unverified",
        classic_btx_registration.resource_type,
        classic_btx_registration.registration_record_runtime,
        classic_btx_registration.registration_handler_runtime,
        classic_btx_registration.loader_runtime,
        classic_btx_registration.descriptor_table_runtime,
        classic_btx_registration.prefix_descriptor_rows,
        classic_btx_registration
            .targets
            .iter()
            .map(|target| (
                target.asset_name,
                target.vdd_entry_index,
                target.resource_id
            ))
            .collect::<Vec<_>>(),
        classic_btx_registration
            .descriptor_selection
            .resource_multiply_runtime,
        classic_btx_registration
            .descriptor_selection
            .descriptor_select_runtime,
        classic_btx_registration
            .descriptor_selection
            .descriptor_copy_runtime,
        classic_btx_registration
            .descriptor_selection
            .descriptor_copy_end_runtime,
    );

    Ok(())
}
