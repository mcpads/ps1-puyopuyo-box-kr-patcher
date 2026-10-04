//! 검증된 BTX catalog 입력으로 번역 전략을 검증한다.

use sha2::{Digest, Sha256};

use ps1_puyopuyo_box::graphics::btx_patch_strategy::{
    render_btx_raster_profile_candidate_members_tsv, render_btx_raster_profile_candidates_tsv,
    verify_box_btx_patch_strategy,
};
use ps1_puyopuyo_box::graphics::g_mess_translation::{
    verify_gocha_result_message_manifest, GOCHA_RESULT_MESSAGE_PROFILE,
};
use ps1_puyopuyo_box::graphics::verify_box_p2_info_page_patch_strategy;

use super::text_catalog::BtxTranslationDevelopmentInputs;

pub(super) fn audit_btx_translation_development(
    inputs: BtxTranslationDevelopmentInputs,
) -> Result<(), String> {
    println!(
        "PASS MM_SPR translation manifest: Japanese {}, PoC-approved {}, approved {}, \
         draft {}, pending {}, \
         categories 1-line {} / 2-line {} / 3-line {}",
        inputs.mm_spr_translation.japanese_targets,
        inputs.mm_spr_translation.poc_approved,
        inputs.mm_spr_translation.approved,
        inputs.mm_spr_translation.draft_human_review_required,
        inputs.mm_spr_translation.pending_human_translation,
        inputs.mm_spr_translation.one_line,
        inputs.mm_spr_translation.two_lines,
        inputs.mm_spr_translation.three_lines
    );
    println!(
        "PASS BTX translation coverage: catalog Japanese {}, managed {} \
         (PoC-approved {}, approved {}, draft {}, pending {}, scope-excluded no-write {}), \
         unmanaged {}",
        inputs.translation_coverage.catalog_japanese_targets,
        inputs.translation_coverage.managed_targets,
        inputs.translation_coverage.poc_approved,
        inputs.translation_coverage.approved,
        inputs.translation_coverage.draft_human_review_required,
        inputs.translation_coverage.pending_human_translation,
        inputs
            .translation_coverage
            .scope_excluded_protected_no_write,
        inputs.translation_coverage.unmanaged_targets
    );
    println!(
        "PASS general raster BTX manifests: translations/assets {}/{}, profile assignments/families \
         {}/{}, translation pending/draft/approved/excluded {}/{}/{}/{}, \
         profile pending/development/approved/excluded {}/{}/{}/{}, \
         delegated G_MESS {}, covered raster {}, \
         all in-scope human approved {}",
        inputs.general_raster_manifests.translation_blocks,
        inputs.general_raster_manifests.translation_assets,
        inputs.general_raster_manifests.profile_assignments,
        inputs.general_raster_manifests.semantic_families,
        inputs.general_raster_manifests.pending_translations,
        inputs.general_raster_manifests.draft_translations,
        inputs.general_raster_manifests.approved_translations,
        inputs.general_raster_manifests.scope_excluded_translations,
        inputs.general_raster_manifests.pending_profiles,
        inputs.general_raster_manifests.development_profiles,
        inputs.general_raster_manifests.approved_profiles,
        inputs.general_raster_manifests.scope_excluded_profiles,
        inputs.general_raster_manifests.delegated_g_mess_blocks,
        inputs.general_raster_manifests.covered_raster_blocks,
        inputs.general_raster_manifests.all_in_scope_human_approved,
    );
    println!(
        "PASS general raster surface policies: groups/assets/blocks {}/{}/{}, \
         background flat/preserved/generated/excluded {}/{}/{}/{}, write whole-bbox/mask/required/protected \
         {}/{}/{}/{}",
        inputs.raster_surface_policies.policy_groups,
        inputs.raster_surface_policies.assets,
        inputs.raster_surface_policies.blocks,
        inputs.raster_surface_policies.flat_background_blocks,
        inputs
            .raster_surface_policies
            .source_background_preserved_blocks,
        inputs.raster_surface_policies.generated_artwork_blocks,
        inputs
            .raster_surface_policies
            .scope_excluded_unassessed_blocks,
        inputs.raster_surface_policies.whole_bbox_blocks,
        inputs.raster_surface_policies.source_text_mask_blocks,
        inputs.raster_surface_policies.composer_required_blocks,
        inputs.raster_surface_policies.protected_no_write_blocks,
    );
    println!(
        "PASS raster composition units: units/member blocks {}/{}, pending/draft/approved \
         {}/{}/{}, runtime order verified {}, product eligible {}",
        inputs.raster_composition.units,
        inputs.raster_composition.member_blocks,
        inputs.raster_composition.pending_units,
        inputs.raster_composition.draft_units,
        inputs.raster_composition.approved_units,
        inputs.raster_composition.runtime_order_verified_units,
        inputs.raster_composition.product_eligible_units,
    );
    println!(
        "PASS P2 INFO page translation manifest: entries {}, pending/draft/approved {}/{}/{}, \
         shared profile approved {}, product eligible {}",
        inputs.p2_info_translation.entries,
        inputs.p2_info_translation.pending_translations,
        inputs.p2_info_translation.draft_translations,
        inputs.p2_info_translation.approved_translations,
        inputs.p2_info_translation.profile_approved,
        inputs.p2_info_translation.product_eligible,
    );
    let g_mess_manifest =
        verify_gocha_result_message_manifest().map_err(|error| error.to_string())?;
    println!(
        "PASS G_MESS full-family draft: entries/drafts/approved {}/{}/{}, profile {} \
         font/size/baseline {}/{}/{}, changed pixels/bytes {}/{}, output SHA-256 {}; \
         product eligible {}, runtime verified false",
        g_mess_manifest.entries,
        g_mess_manifest.drafts,
        g_mess_manifest.approved,
        GOCHA_RESULT_MESSAGE_PROFILE.id,
        GOCHA_RESULT_MESSAGE_PROFILE.font_id,
        GOCHA_RESULT_MESSAGE_PROFILE.font_size,
        GOCHA_RESULT_MESSAGE_PROFILE.baseline,
        inputs.g_mess_draft_patch.changed_pixels,
        inputs.g_mess_draft_patch.changed_bytes,
        inputs.g_mess_draft_patch.output_sha256,
        g_mess_manifest.product_eligible,
    );
    let patch_strategy = verify_box_btx_patch_strategy().map_err(|error| error.to_string())?;
    let profile_candidates =
        render_btx_raster_profile_candidates_tsv().map_err(|error| error.to_string())?;
    let profile_candidates_sha256 = format!("{:x}", Sha256::digest(profile_candidates.as_bytes()));
    let profile_candidate_members =
        render_btx_raster_profile_candidate_members_tsv().map_err(|error| error.to_string())?;
    let profile_candidate_members_sha256 =
        format!("{:x}", Sha256::digest(profile_candidate_members.as_bytes()));
    println!(
        "PASS BTX patch strategy partition: typed Japanese {}, MM_SPR {}, fixed-cell {}, \
         remaining raster blocks/assets {}/{}, profile candidate groups {}, pending MENU06 \
         findings/physical {}/{}, protected MM_MES findings/physical {}/{}, \
         automatic profile approval {}, semantic family proposals/automatic approval {}/{}, \
         group TSV SHA-256 {}, member rows/SHA-256 {}/{}",
        patch_strategy.typed_japanese_blocks,
        patch_strategy.mm_spr_managed_blocks,
        patch_strategy.fixed_cell_page_blocks,
        patch_strategy.remaining_raster_blocks,
        patch_strategy.remaining_raster_assets,
        patch_strategy.profile_candidate_groups.len(),
        patch_strategy.pending_menu06_findings,
        patch_strategy.pending_menu06_physical_assets,
        patch_strategy.protected_mm_mes_findings,
        patch_strategy.protected_mm_mes_physical_assets,
        patch_strategy.automatic_profile_approval,
        patch_strategy.semantic_family_proposals,
        patch_strategy.automatic_semantic_family_approval,
        profile_candidates_sha256,
        patch_strategy.profile_candidate_members.len(),
        profile_candidate_members_sha256,
    );
    let info_pages = verify_box_p2_info_page_patch_strategy(
        inputs
            .info_page_sources
            .iter()
            .map(|row| (row.archive, row.name, row.plain.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!("PASS P2 INFO fixed-cell strategy: {info_pages}");
    println!(
        "PASS BTX baked approved set: blocks {}, changed pixels {}, changed bytes {}, \
         output SHA-256 {}",
        inputs.mm_spr_approved_patch.replacement_blocks,
        inputs.mm_spr_approved_patch.changed_pixels,
        inputs.mm_spr_approved_patch.changed_bytes,
        inputs.mm_spr_approved_patch.output_sha256
    );
    println!(
        "PASS MM_SPR review draft raster: blocks {}, changed pixels {}, changed bytes {}, \
         output SHA-256 {}; product eligible false, runtime verified false",
        inputs.mm_spr_review_draft_patch.replacement_blocks,
        inputs.mm_spr_review_draft_patch.changed_pixels,
        inputs.mm_spr_review_draft_patch.changed_bytes,
        inputs.mm_spr_review_draft_patch.output_sha256
    );

    Ok(())
}
