//! DRAC glyph 정적 분석 결과의 안정된 TSV 표현.

use std::fmt::Write as _;

use super::{
    SubgameSeqDracGlyphAnalysis, BOX_SUBGAME_SEQ_DRAC_CELL_STRIDE,
    BOX_SUBGAME_SEQ_DRAC_DESCRIPTOR_STRIDE, SUBGAME_SEQ_DRAC_GLYPH_SCHEMA_VERSION,
};

pub fn render_subgame_seq_drac_glyphs_tsv(analysis: &SubgameSeqDracGlyphAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tscope\texe\tseq_archive\tgrp_archive\tsequence\te2_record_offset\t\
         e1_record_offset\te1_record_runtime\trepertoire_runtime\te1_handler_runtime\te2_handler_runtime\t\
         repertoire_buffer_runtime\ttokenizer_runtime\tpair_lookup_runtime\tpair_index_store_runtime\t\
         repertoire_glyphs\tsource_unique_pairs\tsource_duplicate_slots\tcell_base\tcell_last\t\
         glyph_object_allocator_runtime\tglyph_object_callback_runtime\tmain_resource_branch_sites\t\
         main_gp_accesses\tgp_runtime\tmain_resource_global_runtime\t\
         auxiliary_resource_global_runtime\t\
         descriptor_stride\tcell_stride\tresource_id\t\
         auxiliary_id\tauxiliary_asset\tauxiliary_descriptor_runtime\t\
         auxiliary_gp_accesses\tauxiliary_descriptor_load_runtime\t\
         auxiliary_cell_store_runtime\tauxiliary_next_cell_store_runtime\t\
         auxiliary_object_load_runtimes\tauxiliary_object_cell_store_runtimes\t\
         auxiliary_object_resource_store_runtimes\t\
         cell_setup_record_offset\tcell_setup_record_runtime\tcell_setup_handler_runtime\t\
         cell_table_offset\tcell_table_runtime\tcell_table_prefix_sha256\t\
         atlas_width\tatlas_height\tcell_grid_x\tcell_grid_y\tcell_grid_columns\tcell_grid_rows\t\
         cell_width\tcell_height\trenderer_runtime\trenderer_resource_load_runtime\trenderer_cell_load_runtime\t\
         descriptor_root_runtime\tdescriptor_runtime\tasset\tasset_vdd_index\tasset_stored_bytes\t\
         asset_plain_bytes\tasset_plain_sha256\tsame_named_asset\tsame_named_resource_id\t\
         same_named_descriptor_runtime\tsame_named_asset_vdd_index\t\
         same_named_asset_stored_bytes\tsame_named_asset_plain_sha256\t\
         selected_same_named_equal_cells\tsame_named_e2_sequence\tsame_named_e2_record_offset\t\
         pixel_format\tatlas_pixels\tlayout_basis\tstatic_status\t\
         runtime_status\n",
    );
    for row in &analysis.rows {
        let fields = [
            SUBGAME_SEQ_DRAC_GLYPH_SCHEMA_VERSION.to_string(),
            row.scope.to_string(),
            row.exe.to_string(),
            row.seq_archive.to_string(),
            row.grp_archive.to_string(),
            row.sequence.to_string(),
            format!("0x{:04X}", row.e2_record_offset),
            format!("0x{:04X}", row.e1_record_offset),
            format!("0x{:08X}", row.e1_record_runtime),
            format!("0x{:08X}", row.repertoire_runtime),
            format!("0x{:08X}", row.e1_handler_runtime),
            format!("0x{:08X}", row.e2_handler_runtime),
            format!("0x{:08X}", row.repertoire_buffer_runtime),
            format!("0x{:08X}", row.tokenizer_runtime),
            format!("0x{:08X}", row.pair_lookup_runtime),
            format!("0x{:08X}", row.pair_index_store_runtime),
            row.repertoire_glyphs.to_string(),
            row.source_unique_pairs.to_string(),
            row.source_duplicate_slots.to_string(),
            row.cell_base.to_string(),
            row.cell_last.to_string(),
            format!("0x{:08X}", row.glyph_object_allocator_runtime),
            format!("0x{:08X}", row.glyph_object_callback_runtime),
            row.main_resource_branch_sites.to_string(),
            row.main_gp_accesses.to_string(),
            format!("0x{:08X}", row.gp_runtime),
            format!("0x{:08X}", row.main_resource_global_runtime),
            format!("0x{:08X}", row.auxiliary_resource_global_runtime),
            BOX_SUBGAME_SEQ_DRAC_DESCRIPTOR_STRIDE.to_string(),
            BOX_SUBGAME_SEQ_DRAC_CELL_STRIDE.to_string(),
            row.resource_id.to_string(),
            row.auxiliary_id.to_string(),
            row.auxiliary_asset.to_string(),
            format!("0x{:08X}", row.auxiliary_descriptor_runtime),
            row.auxiliary_gp_accesses.to_string(),
            format!("0x{:08X}", row.auxiliary_descriptor_load_runtime),
            format!("0x{:08X}", row.auxiliary_cell_store_runtime),
            format!("0x{:08X}", row.auxiliary_next_cell_store_runtime),
            format_runtime_list(&row.auxiliary_object_load_runtimes),
            format_runtime_list(&row.auxiliary_object_cell_store_runtimes),
            format_runtime_list(&row.auxiliary_object_resource_store_runtimes),
            format!("0x{:04X}", row.cell_setup_record_offset),
            format!("0x{:08X}", row.cell_setup_record_runtime),
            format!("0x{:08X}", row.cell_setup_handler_runtime),
            format!("0x{:04X}", row.cell_table_offset),
            format!("0x{:08X}", row.cell_table_runtime),
            row.cell_table_prefix_sha256.clone(),
            row.atlas_width.to_string(),
            row.atlas_height.to_string(),
            row.cell_grid_x.to_string(),
            row.cell_grid_y.to_string(),
            row.cell_grid_columns.to_string(),
            row.cell_grid_rows.to_string(),
            row.cell_width.to_string(),
            row.cell_height.to_string(),
            format!("0x{:08X}", row.renderer_runtime),
            format!("0x{:08X}", row.renderer_resource_load_runtime),
            format!("0x{:08X}", row.renderer_cell_load_runtime),
            format!("0x{:08X}", row.descriptor_root_runtime),
            format!("0x{:08X}", row.descriptor_runtime),
            row.asset.to_string(),
            row.asset_vdd_index.to_string(),
            row.asset_stored_bytes.to_string(),
            row.asset_plain_bytes.to_string(),
            row.asset_plain_sha256.clone(),
            row.same_named_asset.to_string(),
            row.same_named_resource_id.to_string(),
            format!("0x{:08X}", row.same_named_descriptor_runtime),
            row.same_named_asset_vdd_index.to_string(),
            row.same_named_asset_stored_bytes.to_string(),
            row.same_named_asset_plain_sha256.clone(),
            row.selected_same_named_equal_cells.to_string(),
            row.same_named_e2_sequence.to_string(),
            format!("0x{:04X}", row.same_named_e2_record_offset),
            "indexed4".to_string(),
            row.atlas_pixels.to_string(),
            "1024x256-exact-seq-cell-table".to_string(),
            "exact-direct-selected-and-same-named-assets-static".to_string(),
            "clut-vram-upload-draw-hit-unverified".to_string(),
        ];
        writeln!(output, "{}", fields.join("\t")).expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn format_runtime_list(values: &[u32]) -> String {
    values
        .iter()
        .map(|value| format!("0x{value:08X}"))
        .collect::<Vec<_>>()
        .join(",")
}
