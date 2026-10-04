//! TUU CEL/texture와 PUYO2 DEMOFONT 소비 경계를 검증한다.

use crate::cli::{BoxExecutable, DEMO_FONT_NAME, PP2_GRP_PATH};

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::compress::{decompress, parse_header};
use ps1_puyopuyo_box::disasm::verify_box_demo_font_consumer;
use ps1_puyopuyo_box::font::verify_box_demo_font;
use ps1_puyopuyo_box::font::E6_DEMO_POC_RASTER_PROFILE;
use ps1_puyopuyo_box::graphics::e6_demo_font_patch::verify_box_puyo2_e6_demo_font_single_glyph_poc;
use ps1_puyopuyo_box::graphics::tuu_cel::{verify_box_tuu_cel_inventory, TUU_SEQ_PATH};
use ps1_puyopuyo_box::graphics::tuu_cel_texture::{
    verify_box_tuu_cel_texture_pairing, TUU_GRP_PATH,
};
use ps1_puyopuyo_box::pipeline::SubgameSeqE6TranslationPlanAnalysis;

pub(super) fn audit_font_assets(
    box_executables: &[BoxExecutable],
    cnx_archives: &[(&str, VddArchive)],
    e6_translation_plan: &SubgameSeqE6TranslationPlanAnalysis,
) -> Result<(), String> {
    let (_, tuu_seq) = cnx_archives
        .iter()
        .find(|(source_id, _)| *source_id == TUU_SEQ_PATH)
        .ok_or_else(|| format!("{TUU_SEQ_PATH} archive가 없음"))?;
    let tuu_cel = verify_box_tuu_cel_inventory(tuu_seq).map_err(|error| error.to_string())?;
    println!(
        "PASS TUU CEL inventory: entries {}, CEL/SEQ {}/{}, records {}, \
         unique 9-bit values {}, maximum 0x{:03X}",
        tuu_cel.entries,
        tuu_cel.cel_tables,
        tuu_cel.seq_entries,
        tuu_cel.records,
        tuu_cel.unique_values,
        tuu_cel.maximum_value,
    );
    let (_, tuu_grp) = cnx_archives
        .iter()
        .find(|(source_id, _)| *source_id == TUU_GRP_PATH)
        .ok_or_else(|| format!("{TUU_GRP_PATH} archive가 없음"))?;
    let tuu_textures =
        verify_box_tuu_cel_texture_pairing(tuu_seq, tuu_grp).map_err(|error| error.to_string())?;
    println!(
        "PASS TUU CEL character texture pairing: pairs {}, exact-stem/alias {}/{}, \
         atlas {}x{} indexed4 = {} 8x8 tiles, CEL unique/max {}/0x{:03X}; \
         G_MESS resource 0x{:04X} = {} tiles and is excluded",
        tuu_textures.pairs,
        tuu_textures.exact_stem_pairs,
        tuu_textures.alias_pairs,
        tuu_textures.character_atlas_width,
        tuu_textures.character_atlas_height,
        tuu_textures.character_tiles,
        tuu_textures.unique_cel_values,
        tuu_textures.maximum_cel_value,
        tuu_textures.g_mess_resource_id,
        tuu_textures.g_mess_tiles,
    );

    let (_, pp2_grp) = cnx_archives
        .iter()
        .find(|(source_id, _)| *source_id == PP2_GRP_PATH)
        .ok_or_else(|| format!("{PP2_GRP_PATH} archive가 없음"))?;
    let demo_font_entry = pp2_grp
        .find(DEMO_FONT_NAME)
        .ok_or_else(|| format!("{PP2_GRP_PATH}:{DEMO_FONT_NAME} 엔트리가 없음"))?;
    let demo_font_cnx = pp2_grp.read_file(demo_font_entry);
    let demo_font_header = parse_header(demo_font_cnx)
        .map_err(|error| format!("{PP2_GRP_PATH}:{DEMO_FONT_NAME}: {error}"))?;
    if &demo_font_header.subtype != b"bi_" {
        return Err(format!(
            "{PP2_GRP_PATH}:{DEMO_FONT_NAME}: CNX subtype이 bi_가 아님"
        ));
    }
    let demo_font_raw = decompress(demo_font_cnx)
        .map_err(|error| format!("{PP2_GRP_PATH}:{DEMO_FONT_NAME}: {error}"))?;
    let demo_font = verify_box_demo_font(&demo_font_raw).map_err(|error| error.to_string())?;
    println!(
        "PASS DEMOFONT physical grid: slots {}, nonempty {}, empty {}, \
         unique cell pixels {}, palette indices {:?}",
        demo_font.slots,
        demo_font.nonempty_slots,
        demo_font.empty_slots.len(),
        demo_font.unique_cell_pixels,
        demo_font.palette_indices,
    );
    let demo_font_patch = verify_box_puyo2_e6_demo_font_single_glyph_poc(
        &demo_font_raw,
        &e6_translation_plan.plan.glyph_mappings,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS PUYO2 E6 DEMOFONT synthetic glyph patch: profile {}, cells {}, \
         changed pixels/bytes {}/{}, output SHA-256 {}, runtime unverified",
        E6_DEMO_POC_RASTER_PROFILE.id,
        demo_font_patch.changed_cells,
        demo_font_patch.changed_pixels,
        demo_font_patch.changed_bytes,
        demo_font_patch.output_sha256,
    );
    let puyo2_executable = box_executables
        .iter()
        .find(|exe| exe.map.name == "PUYO2.EXE")
        .ok_or_else(|| "PUYO2.EXE 검증 입력이 없음".to_string())?;
    let demo_font_consumer =
        verify_box_demo_font_consumer(puyo2_executable.map, &puyo2_executable.text, &demo_font)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS DEMOFONT consumer: code table 0x{:08X} / {} entries, \
         resource 0x{:04X} descriptor 0x{:08X}, mapped cells {}, \
         physical empty {} / mapped empty {} / unregistered empty {}",
        demo_font_consumer.code_table_runtime,
        demo_font_consumer.code_table_entries,
        demo_font_consumer.resource_index,
        demo_font_consumer.descriptor_runtime,
        demo_font_consumer.mapped_cells,
        demo_font_consumer.physical_empty_cells,
        demo_font_consumer.mapped_empty_cells,
        demo_font_consumer.unregistered_empty_cells,
    );

    Ok(())
}
