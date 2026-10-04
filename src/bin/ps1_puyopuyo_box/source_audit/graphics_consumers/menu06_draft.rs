//! MENU06 text와 glyph atlas의 source-bound draft 입력을 검증한다.

use std::collections::BTreeSet;

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::compress::decompress;
use ps1_puyopuyo_box::graphics::menu06_glyph_patch::{
    patch_box_menu06_glyph_asset, verify_box_menu06_glyph_assets, BOX_MENU06_GLYPH_ASSETS,
};
use ps1_puyopuyo_box::script::{verify_box_menu06_text, verify_box_menu06_translation_manifest};

pub(super) fn audit_menu06_draft(
    cnx_archives: &[(&str, VddArchive)],
    subgame_seq_sources: &[(String, String, Vec<u8>)],
) -> Result<(), String> {
    let menu06_text = verify_box_menu06_text(
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS MENU06 code-to-cell text: physical {}, FF02 segments {}, glyph tokens {}, \
         repertoire {}, controls {}, stream words glyph/control/args/pad/slack \
         {}/{}/{}/{}/{}, typed no-op roundtrip exact, static cell prefix {} \
         (valid {}, empty {}); runtime execution/draw/screen unverified",
        menu06_text.sources.len(),
        menu06_text.segments.len(),
        menu06_text.glyph_tokens,
        menu06_text.distinct_glyph_cells,
        menu06_text.control_tokens.values().sum::<usize>(),
        menu06_text.rewrite_capacity.glyph_positions,
        menu06_text.rewrite_capacity.control_words,
        menu06_text.rewrite_capacity.control_argument_words,
        menu06_text.rewrite_capacity.terminator_padding_words,
        menu06_text.rewrite_capacity.slack_words,
        menu06_text.static_cell_prefix_records,
        menu06_text.static_valid_cells,
        menu06_text.static_empty_cells,
    );
    let menu06_translation =
        verify_box_menu06_translation_manifest(&menu06_text).map_err(|error| error.to_string())?;
    println!(
        "PASS MENU06 translation manifest and fixed-slot draft compile: entries {}, \
         source/KO/padding cells {}/{}/{}, approved/draft {}/{}, visible/used/remaining \
         glyph slots {}/{}/{}, stream SHA-256 {}, product eligible {}",
        menu06_translation.entries.len(),
        menu06_translation.protected_source_code_units,
        menu06_translation.translated_code_units,
        menu06_translation.padding_cells,
        menu06_translation.approved,
        menu06_translation.draft_needs_human_review,
        menu06_translation.unique_translation_glyphs,
        menu06_translation.plan.used_slots,
        menu06_translation.plan.remaining_slots,
        menu06_translation.plan.rewritten_stream_sha256,
        menu06_translation.product_eligible,
    );
    let menu06_glyph_sources = BOX_MENU06_GLYPH_ASSETS
        .iter()
        .map(|(archive_path, asset_name)| {
            let archive = cnx_archives
                .iter()
                .find(|(source_id, _)| source_id == archive_path)
                .map(|(_, archive)| archive)
                .ok_or_else(|| format!("{archive_path} MENU06 GRP 누락"))?;
            let entry = archive
                .find(asset_name)
                .ok_or_else(|| format!("{archive_path}:{asset_name} MENU06 glyph asset 누락"))?;
            let plain = decompress(archive.read_file(entry)).map_err(|source| {
                format!("{archive_path}:{asset_name} MENU06 glyph 해제 실패: {source}")
            })?;
            Ok((*archive_path, *asset_name, plain))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let menu06_glyph_assets = verify_box_menu06_glyph_assets(
        menu06_glyph_sources
            .iter()
            .map(|(archive, asset, plain)| (*archive, *asset, plain.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS MENU06 glyph atlas sources: physical {}, narrow-space palette {:?}, \
         glyph palette {:?}, histogram {:?}; Maplestory raster/runtime unverified",
        menu06_glyph_assets.physical_assets,
        menu06_glyph_assets.narrow_space_palette_indices,
        menu06_glyph_assets.glyph_palette_indices,
        menu06_glyph_assets.glyph_palette_histogram,
    );
    let menu06_draft_patches = menu06_glyph_sources
        .iter()
        .map(|(_, _, plain)| {
            patch_box_menu06_glyph_asset(plain, &menu06_translation.plan.glyph_mappings)
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, String>>()?;
    let patch_summaries = menu06_draft_patches
        .iter()
        .map(|patch| {
            (
                patch.changed_cells,
                patch.changed_pixels,
                patch.changed_bytes,
                patch.output_sha256.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    if patch_summaries.len() != 1 {
        return Err(format!(
            "MENU06 두 물리 draft glyph patch 결과가 서로 다름: {patch_summaries:?}"
        ));
    }
    println!(
        "PASS MENU06 Maplestory Light draft glyph compile: physical {}, changed \
         cells/pixels/bytes {}/{}/{}, output SHA-256 {}; translation approved false, \
         runtime unverified",
        menu06_draft_patches.len(),
        menu06_draft_patches[0].changed_cells,
        menu06_draft_patches[0].changed_pixels,
        menu06_draft_patches[0].changed_bytes,
        menu06_draft_patches[0].output_sha256,
    );
    Ok(())
}
