//! DRAC object field와 공용 short renderer의 결속 검증.

use super::{
    error, BtxCellAnalysis, BtxRendererFamily, SubgameSeqDracGlyphError, SubgameSeqDracGlyphSpec,
};

pub(super) fn verify_renderer(
    btx_cells: &BtxCellAnalysis,
    spec: SubgameSeqDracGlyphSpec,
) -> Result<(), SubgameSeqDracGlyphError> {
    let matches = btx_cells
        .consumers
        .iter()
        .filter(|consumer| {
            consumer.exe == spec.exe
                && consumer.renderer_family == Some(BtxRendererFamily::ShortRenderer)
                && consumer.function_candidate == Some(spec.renderer_runtime)
                && consumer.resource_field_offset == 0x6C
                && consumer.cell_field_offset == 0x6A
                && consumer.resource_load == spec.renderer_resource_load_runtime
                && consumer.cell_load == spec.renderer_cell_load_runtime
                && consumer.descriptor_table_load == spec.renderer_descriptor_root_load_runtime
                && consumer.cell_table_load == spec.renderer_cell_table_load_runtime
                && consumer.cell_entry == spec.renderer_cell_entry_runtime
                && consumer.core_entry_reached
        })
        .count();
    if matches != 1 {
        return Err(error(format!(
            "{} DRAC short renderer 일치 수가 1이 아님: {matches}",
            spec.exe
        )));
    }
    Ok(())
}
