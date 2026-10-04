use super::*;
use crate::isa::Reg;

#[test]
fn e2_resource와_e1_repertoire_pointer를_연속_record로_검증한다() {
    let spec = BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS[0];
    let mut raw = vec![0; spec.e1_record_offset + 8];
    raw[spec.e2_record_offset..spec.e2_record_offset + 8]
        .copy_from_slice(&[0xE2, 0, 0x5D, 0, 0x59, 0, 0, 0]);
    raw[spec.e1_record_offset] = 0xE1;
    raw[spec.e1_record_offset + 4..spec.e1_record_offset + 8]
        .copy_from_slice(&0x801B_6D1Cu32.to_le_bytes());
    verify_setup_records(&raw, spec, 0x801B_6D1C).unwrap();

    raw[spec.e2_record_offset + 2] ^= 1;
    assert!(verify_setup_records(&raw, spec, 0x801B_6D1C).is_err());
}

#[test]
fn same_named_asset_resource를_가진_별도_e2_record를_검증한다() {
    let spec = BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS[1];
    let mut raw = vec![0; spec.same_named_e2_record_offset + 8];
    raw[spec.same_named_e2_record_offset..spec.same_named_e2_record_offset + 8]
        .copy_from_slice(&[0xE2, 0, 0x8D, 0, 0x88, 0, 0, 0]);
    verify_same_named_resource_record(&raw, spec).unwrap();

    raw[spec.same_named_e2_record_offset + 2] ^= 1;
    assert!(verify_same_named_resource_record(&raw, spec).is_err());
}

#[test]
fn main_resource_gp는_e2_write와_glyph_read외_접근을_거부한다() {
    let spec = BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS[0];
    let mut instructions = vec![
        Instruction {
            addr: spec.gp_lui_runtime,
            word: 0,
            kind: K::Lui {
                rt: Reg::GP,
                imm: 0x8009,
            },
        },
        Instruction {
            addr: spec.gp_addiu_runtime,
            word: 0,
            kind: K::Addiu {
                rt: Reg::GP,
                rs: Reg::GP,
                imm: 0x56C4,
            },
        },
        Instruction {
            addr: spec.resource_global_load_runtime,
            word: 0,
            kind: K::Lhu {
                rt: Reg::A1,
                base: Reg::GP,
                offset: spec.resource_gp_offset,
            },
        },
        Instruction {
            addr: spec.e2_handler_runtime + 0x10,
            word: 0,
            kind: K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: spec.resource_gp_offset,
            },
        },
    ];
    verify_main_resource_consumers(&instructions, spec).unwrap();

    instructions.push(Instruction {
        addr: spec.e2_handler_runtime + 0x14,
        word: 0,
        kind: K::Lw {
            rt: Reg::V0,
            base: Reg::GP,
            offset: spec.resource_gp_offset,
        },
    });
    assert!(verify_main_resource_consumers(&instructions, spec).is_err());
}

#[test]
fn report는_정적_asset연결과_동적_cell경계를_분리한다() {
    let row = SubgameSeqDracGlyphRow {
        scope: "PUYO1",
        exe: "PUYO1.EXE",
        seq_archive: "/PUYO1/PP1_SEQ.VDD;1",
        grp_archive: "/PUYO1/PP1_GRP.VDD;1",
        sequence: "DRAC_SEQ.SEQ",
        e2_record_offset: 0xB5C,
        e1_record_offset: 0xB64,
        e1_record_runtime: 1,
        repertoire_runtime: 2,
        e1_handler_runtime: 3,
        e2_handler_runtime: 4,
        repertoire_buffer_runtime: 5,
        tokenizer_runtime: 6,
        pair_lookup_runtime: 7,
        pair_index_store_runtime: 8,
        repertoire_glyphs: 56,
        source_unique_pairs: 52,
        source_duplicate_slots: 4,
        cell_base: 30,
        cell_last: 85,
        glyph_object_allocator_runtime: 9,
        glyph_object_callback_runtime: 10,
        main_resource_branch_sites: 0,
        main_gp_accesses: 2,
        gp_runtime: PUYO1_DRAC_GP_RUNTIME,
        main_resource_global_runtime: PUYO1_DRAC_MAIN_RESOURCE_GLOBAL_RUNTIME,
        auxiliary_resource_global_runtime: PUYO1_DRAC_AUXILIARY_RESOURCE_GLOBAL_RUNTIME,
        resource_id: 93,
        auxiliary_id: 89,
        auxiliary_asset: "MZ_ARLE.BTX",
        auxiliary_descriptor_runtime: 14,
        auxiliary_gp_accesses: 4,
        auxiliary_descriptor_load_runtime: 15,
        auxiliary_cell_store_runtime: 16,
        auxiliary_next_cell_store_runtime: 17,
        auxiliary_object_load_runtimes: [18, 19],
        auxiliary_object_cell_store_runtimes: [20, 21],
        auxiliary_object_resource_store_runtimes: [22, 23],
        cell_setup_record_offset: 0x139C,
        cell_setup_record_runtime: 24,
        cell_setup_handler_runtime: 25,
        cell_table_offset: 0x39C,
        cell_table_runtime: 26,
        cell_table_prefix_sha256: "11".repeat(32),
        atlas_width: 1024,
        atlas_height: 256,
        cell_grid_x: 640,
        cell_grid_y: 0,
        cell_grid_columns: 8,
        cell_grid_rows: 7,
        cell_width: 16,
        cell_height: 16,
        renderer_runtime: 9,
        renderer_resource_load_runtime: 10,
        renderer_cell_load_runtime: 11,
        descriptor_root_runtime: 12,
        descriptor_runtime: 13,
        asset: "MZ_DRA.BTX",
        asset_vdd_index: 55,
        asset_stored_bytes: 30_584,
        asset_plain_bytes: 131_072,
        asset_plain_sha256: "00".repeat(32),
        same_named_asset: "MZ_DRA.BTX",
        same_named_resource_id: 93,
        same_named_descriptor_runtime: 13,
        same_named_asset_vdd_index: 55,
        same_named_asset_stored_bytes: 30_584,
        same_named_asset_plain_sha256: "00".repeat(32),
        selected_same_named_equal_cells: 56,
        same_named_e2_sequence: "DRAC_SEQ.SEQ",
        same_named_e2_record_offset: 0xB5C,
        atlas_pixels: 262_144,
    };
    let report =
        render_subgame_seq_drac_glyphs_tsv(&SubgameSeqDracGlyphAnalysis { rows: vec![row] });
    assert!(report.contains("MZ_DRA.BTX"));
    assert!(report.contains("MZ_ARLE.BTX"));
    assert!(report.contains("0x00000012,0x00000013"));
    assert!(report.contains("auxiliary_gp_accesses"));
    assert!(report.contains("main_resource_branch_sites"));
    assert!(report.contains("main_gp_accesses"));
    assert!(report.contains("main_resource_global_runtime"));
    assert!(report.contains("selected_same_named_equal_cells"));
    assert!(report.contains("same_named_e2_sequence"));
    assert!(report.contains("1024x256-exact-seq-cell-table"));
    assert!(report.contains("exact-direct-selected-and-same-named-assets-static"));
    assert!(report.contains("clut-vram-upload-draw-hit-unverified"));
}

#[test]
fn gp상대_main과_auxiliary_global주소를_고정한다() {
    assert_eq!(
        gp_runtime_address(PUYO1_DRAC_GP_RUNTIME, 0x03FC),
        Some(PUYO1_DRAC_MAIN_RESOURCE_GLOBAL_RUNTIME)
    );
    assert_eq!(
        gp_runtime_address(PUYO1_DRAC_GP_RUNTIME, 0x0400),
        Some(PUYO1_DRAC_AUXILIARY_RESOURCE_GLOBAL_RUNTIME)
    );
    assert_eq!(
        gp_runtime_address(PUYO2_DRAC_GP_RUNTIME, 0x0408),
        Some(PUYO2_DRAC_MAIN_RESOURCE_GLOBAL_RUNTIME)
    );
    assert_eq!(
        gp_runtime_address(PUYO2_DRAC_GP_RUNTIME, 0x040C),
        Some(PUYO2_DRAC_AUXILIARY_RESOURCE_GLOBAL_RUNTIME)
    );
}

#[test]
fn selected와_same_named_asset의_56개_cell을_독립_비교한다() {
    let mut selected = vec![0; BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES];
    let mut same_named = selected.clone();
    assert_eq!(
        equal_glyph_cells(&selected, &same_named).unwrap(),
        BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS
    );

    let row_bytes = usize::from(BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH) / 2;
    let first_cell = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y) * row_bytes
        + usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X) / 2;
    same_named[first_cell] = 0x0F;
    assert_eq!(
        equal_glyph_cells(&selected, &same_named).unwrap(),
        BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS - 1
    );

    selected.pop();
    assert!(equal_glyph_cells(&selected, &same_named).is_err());
}

#[test]
fn cell범위는_56개_repertoire와_정확히_맞는다() {
    assert_eq!(
        usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_LAST - BOX_SUBGAME_SEQ_DRAC_CELL_BASE + 1),
        BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS
    );
    assert_eq!(
        BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES * 2,
        BOX_SUBGAME_SEQ_DRAC_ATLAS_PIXELS
    );
    assert_eq!(
        usize::from(BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH)
            * usize::from(BOX_SUBGAME_SEQ_DRAC_ATLAS_HEIGHT),
        BOX_SUBGAME_SEQ_DRAC_ATLAS_PIXELS
    );
    assert_eq!(
        BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS * BOX_SUBGAME_SEQ_DRAC_CELL_GRID_ROWS,
        BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS
    );
    assert_eq!(
        BOX_SUBGAME_SEQ_DRAC_SOURCE_UNIQUE_PAIRS + BOX_SUBGAME_SEQ_DRAC_SOURCE_DUPLICATE_SLOTS,
        BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS
    );
    assert_eq!(
        BOX_SUBGAME_SEQ_DRAC_SPACE_PAIR_INDICES,
        [27, 28, 29, 30, 31]
    );
}

#[test]
fn auxiliary_cell_metadata_offset은_12바이트_stride를_따른다() {
    assert_eq!(
        BOX_SUBGAME_SEQ_DRAC_AUXILIARY_CELL_TABLE_OFFSET,
        BOX_SUBGAME_SEQ_DRAC_AUXILIARY_CELL as i16 * BOX_SUBGAME_SEQ_DRAC_CELL_STRIDE as i16
    );
    assert_eq!(
        BOX_SUBGAME_SEQ_DRAC_AUXILIARY_NEXT_CELL_TABLE_OFFSET,
        BOX_SUBGAME_SEQ_DRAC_AUXILIARY_NEXT_CELL as i16 * BOX_SUBGAME_SEQ_DRAC_CELL_STRIDE as i16
    );
}

#[test]
fn auxiliary_gp전수는_모든_memory_width를_센다() {
    let kinds = [
        K::Lb {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 1,
        },
        K::Lbu {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 1,
        },
        K::Lh {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 1,
        },
        K::Lhu {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 1,
        },
        K::Lw {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 1,
        },
        K::Lwl {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 1,
        },
        K::Lwr {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 1,
        },
        K::Sb {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 1,
        },
        K::Sh {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 1,
        },
        K::Sw {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 1,
        },
        K::Swl {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 1,
        },
        K::Swr {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 1,
        },
    ];
    for kind in kinds {
        assert_eq!(memory_base_and_offset(kind), Some((Reg::GP, 1)));
    }
    assert_eq!(
        memory_base_and_offset(K::Addiu {
            rt: Reg::V0,
            rs: Reg::GP,
            imm: 1,
        }),
        None
    );
}
