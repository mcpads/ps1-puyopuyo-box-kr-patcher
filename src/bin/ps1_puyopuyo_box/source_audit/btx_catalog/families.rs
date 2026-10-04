//! 미분류 BTX family와 정적 consumer 근거를 검증한다.

use sha2::{Digest, Sha256};

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::graphics::btx_family_candidates::{
    render_btx_candidate_families_tsv, render_btx_family_candidate_features_tsv,
    verify_box_btx_family_candidates, BtxConsumerEvidence,
};
use ps1_puyopuyo_box::graphics::text_catalog::verify_boxopt_text_catalog_selectors;
use ps1_puyopuyo_box::pipeline::{
    audit_btx_review_layout_evidence, verify_box_boxopt_static, verify_box_btx_family_static_cells,
    verify_box_sp_font_static,
};
use ps1_puyopuyo_box::review::{
    verify_box_btx_family_review_decisions, verify_box_btx_family_roles, BtxFamilyRoleBasis,
};

use super::super::{SourceAuditContext, SubgameAssetAuditData, SubgameTextAuditData};

pub(super) fn audit_btx_families(
    context: &SourceAuditContext<'_>,
    subgame_text: &SubgameTextAuditData,
    subgame_assets: &SubgameAssetAuditData,
    cnx_archives: &[(&str, VddArchive)],
) -> Result<(), String> {
    let family_candidates = verify_box_btx_family_candidates(
        cnx_archives
            .iter()
            .filter(|(source_id, _)| source_id.ends_with("GRP.VDD;1"))
            .map(|(source_id, archive)| (*source_id, archive)),
        subgame_assets
            .btx_names
            .links
            .iter()
            .map(|link| BtxConsumerEvidence {
                archive: link.archive,
                name: &link.asset_name,
                static_evidence: link.static_evidence(),
                static_reference_sites: link.static_ref_sites.len()
                    + link.entry_reached_ref_sites.len()
                    + link.table_static_ref_sites.len()
                    + link.table_entry_reached_ref_sites.len()
                    + link.descriptor_static_ref_sites.len()
                    + link.descriptor_entry_reached_ref_sites.len(),
                descriptor_runtime: link.descriptor_runtime_start,
            }),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS unclassified BTX family candidates: physical/unique {}/{}, known \
         catalog/pending/visual-positive/nontext {}/{}/{}/{}, unclassified unique {}, \
         families {}, known-content aliases {}, automatic approval {}",
        family_candidates.physical_btx,
        family_candidates.unique_contents,
        family_candidates.known_text_contents,
        family_candidates.known_pending_contents,
        family_candidates.known_visual_positive_contents,
        family_candidates.known_nontext_contents,
        family_candidates.candidates.len(),
        family_candidates.families.len(),
        family_candidates.unclassified_known_content_aliases.len(),
        family_candidates.automatic_approval,
    );
    for alias in &family_candidates.unclassified_known_content_aliases {
        println!(
            "  unclassified physical alias {}:{} -> {} {}",
            alias.archive, alias.name, alias.known_content_class, alias.decompressed_sha256,
        );
    }
    let feature_report = render_btx_family_candidate_features_tsv(&family_candidates);
    let family_report = render_btx_candidate_families_tsv(&family_candidates);
    let feature_sha256 = format!("{:x}", Sha256::digest(feature_report.as_bytes()));
    let family_sha256 = format!("{:x}", Sha256::digest(family_report.as_bytes()));
    println!(
        "PASS BTX family candidate reports: feature/family SHA-256 {feature_sha256}/{family_sha256}"
    );
    let family_review = verify_box_btx_family_review_decisions(&family_candidates)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS BTX family human review ledger: families {}, pending {}, Japanese pixels {}, \
         no Japanese in representative {}, insufficient {}, SHA-256 {}",
        family_review.families,
        family_review.pending_human_review,
        family_review.japanese_pixels_observed,
        family_review.no_japanese_observed_in_representative,
        family_review.review_insufficient,
        family_review.manifest_sha256,
    );
    let review_layout_evidence = audit_btx_review_layout_evidence(
        context
            .box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        subgame_text
            .sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &subgame_text.layouts,
        &subgame_assets.btx_names,
        &subgame_assets.btx_descriptor_selections,
        &family_candidates,
        &family_review,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS BTX insufficient-review static layout evidence: families {}, unique/physical \
         {}/{}, physical with any/exact/compatible setup {}/{}/{}, candidate \
         exact/row-padded/partial/multiple/none {}/{}/{}/{}/{}, physical with graphics selector {}, \
         graphics/other-type selector records {}/{}, entry-trace code matches {}; runtime execution/upload/draw unverified",
        review_layout_evidence.families,
        review_layout_evidence.unique_contents,
        review_layout_evidence.physical_copies,
        review_layout_evidence.copies_with_setup,
        review_layout_evidence.copies_with_descriptor_sized_setup,
        review_layout_evidence.copies_with_descriptor_compatible_setup,
        review_layout_evidence.exact_candidates,
        review_layout_evidence.row_padded_candidates,
        review_layout_evidence.partial_candidates,
        review_layout_evidence.multiple_layout_candidates,
        review_layout_evidence.candidates_without_compatible_setup,
        review_layout_evidence.copies_with_graphics_selector,
        review_layout_evidence.graphics_selector_records,
        review_layout_evidence.other_type_numeric_matches,
        review_layout_evidence.entry_trace_code_matches,
    );
    for candidate in &review_layout_evidence.candidates {
        println!(
            "  review layout {} {} physical compatible {}/{} graphics selectors copies/records \
             {}/{} other-type/code numeric matches {}/{} storage layouts {:?}",
            candidate.family_key,
            candidate.resolution.label(),
            candidate.copies_with_descriptor_compatible_setup,
            candidate.physical_copies,
            candidate.copies_with_graphics_selector,
            candidate.graphics_selector_records,
            candidate.other_type_numeric_matches,
            candidate.entry_trace_code_matches,
            candidate.descriptor_storage_layouts,
        );
    }
    let family_roles = verify_box_btx_family_roles(&family_candidates, &family_review)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS BTX positive family roles: families {}, unique/physical {}/{}, \
         runtime-consumer/source-structure/visual-candidate {}/{}/{}",
        family_roles.families,
        family_roles.unique_contents,
        family_roles.physical_copies,
        family_roles.runtime_consumer_confirmed,
        family_roles.source_structure_confirmed,
        family_roles.human_visual_candidates,
    );
    for row in family_roles
        .rows
        .iter()
        .filter(|row| row.basis == BtxFamilyRoleBasis::HumanVisualCandidate)
    {
        let layout = row
            .common_layout
            .map(|layout| format!("{}x{}", layout.width, layout.height))
            .unwrap_or_else(|| "unresolved".to_owned());
        println!(
            "  visual candidate {} {} {} {}",
            row.family_key,
            row.role.label(),
            row.basis.label(),
            layout,
        );
    }
    let family_static_cells = verify_box_btx_family_static_cells(
        context
            .box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        &subgame_assets.btx_names,
        &subgame_assets.btx_descriptor_selections,
        &family_candidates,
        &family_roles,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS BTX visual-family conservative setup cells: families {}, physical {}, setup rows/records \
         {}/{}, cells {}, full-table/leading-prefix {}/{}; runtime execution/upload/draw unverified",
        family_static_cells.families,
        family_static_cells.physical_assets,
        family_static_cells.rows_with_setup,
        family_static_cells.setup_records,
        family_static_cells.cell_records,
        family_static_cells.full_table_setups,
        family_static_cells.leading_prefix_setups,
    );
    for row in &family_static_cells.rows {
        for setup in &row.setups {
            println!(
                "  embedded setup {} {} VDD entry {} / descriptor resource {}: \
                 setup/table 0x{:08X}/0x{:08X}, coordinate {}x{}, cells {}, extent {}",
                row.family_key,
                row.exe,
                row.vdd_entry_index,
                row.descriptor_resource_id,
                setup.setup_runtime,
                setup.table_runtime,
                setup.coordinate_width,
                setup.coordinate_height,
                setup.cells.len(),
                setup.extent_basis.label(),
            );
        }
    }
    let boxopt_static = verify_box_boxopt_static(
        context
            .box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        &subgame_assets.btx_names,
        &subgame_assets.btx_descriptor_selections,
    )
    .map_err(|error| error.to_string())?;
    verify_boxopt_text_catalog_selectors(&boxopt_static.selectors)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS BOXOPT sparse selector consumers: owners {}, selector table SHA-256 {}, \
         slots/asset-backed/empty {}/{}/{}, phrases/translation targets {}/{}, \
         overlap groups {}, phrase selector/union pixels {}/{}, first outside {}; \
         GOCHA1 runtime draw evidence is documented separately",
        boxopt_static.owners.len(),
        boxopt_static.selector_table_sha256,
        boxopt_static.selector_slots,
        boxopt_static.asset_backed_selectors,
        boxopt_static.empty_selectors,
        boxopt_static.phrase_selectors,
        boxopt_static.translation_target_selectors,
        boxopt_static.overlap_groups,
        boxopt_static.phrase_selector_pixels,
        boxopt_static.phrase_surface_union_pixels,
        boxopt_static.first_non_asset_selector,
    );
    for owner in &boxopt_static.owners {
        println!(
            "  BOXOPT {} resource {} descriptor/select 0x{:08X}/0x{:08X}, \
             setup/table 0x{:08X}/0x{:08X}, renderer selector loads \
             0x{:08X}/0x{:08X}",
            owner.exe,
            owner.resource_id,
            owner.descriptor_runtime,
            owner.descriptor_selection_runtime,
            owner.setup_runtime,
            owner.table_runtime,
            owner.renderer_selector_loads[0],
            owner.renderer_selector_loads[1],
        );
    }
    let sp_font_static = verify_box_sp_font_static(
        context
            .box_executables
            .iter()
            .filter(|exe| {
                family_static_cells
                    .rows
                    .iter()
                    .any(|row| row.exe == exe.map.name && row.asset_name == "SP_FONT.BTX")
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        &subgame_assets.btx_descriptor_selections,
        &family_static_cells,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS SP_FONT static glyph consumers: owners {}, common code mappings {}, code table \
         SHA-256 {}, glyph grid {}x{} of {}x{} (mapped/physical/unused {}/{}/{}), \
         glyph/lower-UI boundary y={}/{} cells, direct builder JAL {}; \
         descriptor selection and async load scheduling to GPU primitive construction are static; \
         execution/VRAM upload/draw hit unverified",
        sp_font_static.owners.len(),
        sp_font_static.mappings.len(),
        sp_font_static.code_table_sha256,
        sp_font_static.glyph_columns,
        sp_font_static.glyph_rows,
        sp_font_static.glyph_width,
        sp_font_static.glyph_height,
        sp_font_static.mapped_slots,
        sp_font_static.physical_slots,
        sp_font_static.unused_slots,
        sp_font_static.glyph_region_height,
        sp_font_static.lower_ui_cells,
        sp_font_static.direct_builder_call_sites,
    );
    for owner in &sp_font_static.owners {
        let direct_builder_calls = owner
            .direct_builder_call_sites
            .iter()
            .map(|address| format!("0x{address:08X}"))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "  SP_FONT {} resource {} descriptor/select 0x{:08X}/0x{:08X}, \
             load schedule/target 0x{:08X}/0x{:08X}, builder/callback/renderer \
             0x{:08X}/0x{:08X}/0x{:08X}, code table 0x{:08X}, direct builder JAL [{}]",
            owner.exe,
            owner.resource_id,
            owner.source_descriptor_runtime,
            owner.descriptor_selection_runtime,
            owner.load_schedule_runtime,
            owner.load_schedule_target,
            owner.builder_runtime,
            owner.callback_runtime,
            owner.glyph_renderer_runtime,
            owner.code_table_runtime,
            direct_builder_calls,
        );
    }

    Ok(())
}
