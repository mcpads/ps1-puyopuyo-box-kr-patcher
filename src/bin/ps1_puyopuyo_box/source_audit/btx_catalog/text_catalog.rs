//! BTX text catalog의 원본 자산과 번역 개발 입력을 검증한다.

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::btx::PixelFormat;
use ps1_puyopuyo_box::compress::{decompress, parse_header};
use ps1_puyopuyo_box::graphics::btx_classification::{
    verify_btx_classification_asset, verify_btx_classification_boundary,
    verify_btx_classification_specs, BOX_BTX_CLASSIFICATION_SAMPLES,
};
use ps1_puyopuyo_box::graphics::btx_raster_composition::{
    verify_btx_raster_composition_manifest, RasterCompositionReport,
};
use ps1_puyopuyo_box::graphics::btx_raster_surface_policy::{
    verify_general_btx_raster_surface_policies, RasterSurfacePolicyReport,
};
use ps1_puyopuyo_box::graphics::btx_raster_translation_manifest::{
    verify_general_btx_raster_manifests, GeneralRasterManifestReport,
};
use ps1_puyopuyo_box::graphics::g_mess_translation::patch_gocha_result_message_drafts;
use ps1_puyopuyo_box::graphics::mm_spr_patch::{
    patch_mm_spr_approved_poc_translations, patch_mm_spr_review_translations,
};
use ps1_puyopuyo_box::graphics::mm_spr_translation::{
    verify_mm_spr_translation_manifest, MmSprTranslationReport,
};
use ps1_puyopuyo_box::graphics::p2_info_translation::{
    verify_p2_info_translation_manifest, P2InfoTranslationReport,
};
use ps1_puyopuyo_box::graphics::text_candidates::{
    verify_pending_text_asset, verify_pending_text_catalog_boundary,
    verify_pending_text_catalog_specs, verify_promoted_raster_text_catalog_provenance,
    verify_promoted_text_catalog_provenance, ReviewLayoutBasis, BOX_PENDING_TEXT_ASSETS,
    BOX_PROMOTED_CELL_GRID_PROVENANCE, BOX_PROMOTED_ENDING_RASTER_PROVENANCE,
    BOX_PROMOTED_G_MESS_PROVENANCE, BOX_PROMOTED_KANBAN_PROVENANCE,
    BOX_PROMOTED_MENU_RASTER_PROVENANCE, BOX_PROMOTED_P1_OVER3_PROVENANCE,
};
use ps1_puyopuyo_box::graphics::text_catalog::{
    verify_text_asset, verify_text_catalog_specs, BOX_TEXT_ASSETS,
};
use ps1_puyopuyo_box::graphics::text_patch::BakedTextPatchReport;
use ps1_puyopuyo_box::graphics::translation_coverage::{
    verify_translation_coverage, TranslationCoverageReport,
};

/// 후속 번역·stress build 단계가 소비하는 검증 완료 입력.
pub(super) struct BtxTranslationDevelopmentInputs {
    pub(super) mm_spr_translation: MmSprTranslationReport,
    pub(super) general_raster_manifests: GeneralRasterManifestReport,
    pub(super) raster_surface_policies: RasterSurfacePolicyReport,
    pub(super) raster_composition: RasterCompositionReport,
    pub(super) p2_info_translation: P2InfoTranslationReport,
    pub(super) translation_coverage: TranslationCoverageReport,
    pub(super) g_mess_draft_patch: BakedTextPatchReport,
    pub(super) info_page_sources: Vec<BtxInfoPageSource>,
    pub(super) mm_spr_approved_patch: BakedTextPatchReport,
    pub(super) mm_spr_review_draft_patch: BakedTextPatchReport,
}

pub(super) struct BtxInfoPageSource {
    pub(super) archive: &'static str,
    pub(super) name: &'static str,
    pub(super) plain: Vec<u8>,
}

#[derive(Default)]
struct TextCatalogTotals {
    physical_assets: usize,
    blocks: usize,
    block_pixels: usize,
    japanese_target_blocks: usize,
    japanese_target_pixels: usize,
}

struct CatalogAssetAudit {
    totals: TextCatalogTotals,
    g_mess_draft_patch: Option<BakedTextPatchReport>,
    info_page_sources: Vec<BtxInfoPageSource>,
    mm_spr_approved_patch: Option<BakedTextPatchReport>,
    mm_spr_review_draft_patch: Option<BakedTextPatchReport>,
}

pub(super) fn audit_btx_text_catalog(
    cnx_archives: &[(&str, VddArchive)],
) -> Result<BtxTranslationDevelopmentInputs, String> {
    audit_classified_samples(cnx_archives)?;

    let text_spec_report =
        verify_text_catalog_specs(BOX_TEXT_ASSETS).map_err(|error| error.to_string())?;
    let pending_text_spec_report = verify_pending_text_catalog_specs(&BOX_PENDING_TEXT_ASSETS)
        .map_err(|error| error.to_string())?;
    verify_catalog_boundaries()?;

    let mm_spr_translation =
        verify_mm_spr_translation_manifest().map_err(|error| error.to_string())?;
    let general_raster_manifests =
        verify_general_btx_raster_manifests().map_err(|error| error.to_string())?;
    let raster_surface_policies =
        verify_general_btx_raster_surface_policies().map_err(|error| error.to_string())?;
    let raster_composition =
        verify_btx_raster_composition_manifest().map_err(|error| error.to_string())?;
    let p2_info_translation =
        verify_p2_info_translation_manifest().map_err(|error| error.to_string())?;
    let translation_coverage = verify_translation_coverage().map_err(|error| error.to_string())?;

    let catalog = audit_catalog_assets(cnx_archives)?;
    let pending_physical_assets = audit_pending_assets(cnx_archives)?;
    if pending_physical_assets != pending_text_spec_report.declared_physical_assets {
        return Err(format!(
            "pending BTX 물리 분모 불일치: declared={}, actual={pending_physical_assets}",
            pending_text_spec_report.declared_physical_assets
        ));
    }
    if catalog.totals.physical_assets != text_spec_report.declared_physical_assets
        || catalog.totals.blocks != text_spec_report.blocks
        || catalog.totals.japanese_target_blocks != text_spec_report.japanese_target_blocks
        || catalog.totals.japanese_target_blocks != translation_coverage.catalog_japanese_targets
    {
        return Err(format!(
            "BTX text catalog 분모 불일치: declared physical/blocks/Japanese={}/{}/{}, \
             actual={}/{}/{}",
            text_spec_report.declared_physical_assets,
            text_spec_report.blocks,
            text_spec_report.japanese_target_blocks,
            catalog.totals.physical_assets,
            catalog.totals.blocks,
            catalog.totals.japanese_target_blocks
        ));
    }
    println!(
        "PASS BTX text catalog: assets {}, physical {}, blocks {}, block pixels {}, \
         Japanese targets {} blocks / {} pixels",
        text_spec_report.assets,
        catalog.totals.physical_assets,
        catalog.totals.blocks,
        catalog.totals.block_pixels,
        catalog.totals.japanese_target_blocks,
        catalog.totals.japanese_target_pixels
    );
    print_pending_summary(pending_text_spec_report.assets, pending_physical_assets);

    Ok(BtxTranslationDevelopmentInputs {
        mm_spr_translation,
        general_raster_manifests,
        raster_surface_policies,
        raster_composition,
        p2_info_translation,
        translation_coverage,
        g_mess_draft_patch: catalog
            .g_mess_draft_patch
            .ok_or_else(|| "G_MESS 전수 초안 합성 결과가 없음".to_string())?,
        info_page_sources: catalog.info_page_sources,
        mm_spr_approved_patch: catalog
            .mm_spr_approved_patch
            .ok_or_else(|| "MM_SPR 승인 집합 PoC 검증 결과가 없음".to_string())?,
        mm_spr_review_draft_patch: catalog
            .mm_spr_review_draft_patch
            .ok_or_else(|| "MM_SPR 검토 초안 patch report가 없음".to_string())?,
    })
}

fn audit_classified_samples(cnx_archives: &[(&str, VddArchive)]) -> Result<(), String> {
    let spec_report = verify_btx_classification_specs(&BOX_BTX_CLASSIFICATION_SAMPLES)
        .map_err(|error| error.to_string())?;
    let mut physical_assets = 0usize;
    for spec in BOX_BTX_CLASSIFICATION_SAMPLES {
        let mut prior_report = None;
        for archive_path in spec.archive_paths {
            let (_, archive) = find_archive(cnx_archives, archive_path, "BTX 분류")?;
            let entry = archive
                .find(spec.name)
                .ok_or_else(|| format!("BTX 분류 자산이 없음: {archive_path}:{}", spec.name))?;
            let compressed = archive.read_file(entry);
            let plain = verify_format_and_decompress(
                compressed,
                spec.layout.format,
                archive_path,
                spec.name,
                "분류 ",
            )?;
            let report = verify_btx_classification_asset(&plain, spec)
                .map_err(|error| format!("{archive_path}:{}: {error}", spec.name))?;
            if let Some(previous) = prior_report {
                if report != previous {
                    return Err(format!(
                        "{archive_path}:{} 분류 결과가 다른 사본과 다름",
                        spec.name
                    ));
                }
            } else {
                prior_report = Some(report);
            }
            physical_assets += 1;
        }
    }
    if physical_assets != spec_report.declared_physical_assets {
        return Err(format!(
            "BTX 분류 물리 분모 불일치: declared={}, actual={physical_assets}",
            spec_report.declared_physical_assets
        ));
    }
    println!(
        "PASS BTX classified non-text samples: samples {}, physical {}, P2_INFO2 144x176 / \
         26 sprite cells / baked text not observed",
        spec_report.samples, physical_assets,
    );
    Ok(())
}

fn verify_catalog_boundaries() -> Result<(), String> {
    verify_btx_classification_boundary(
        &BOX_BTX_CLASSIFICATION_SAMPLES,
        BOX_TEXT_ASSETS,
        &BOX_PENDING_TEXT_ASSETS,
    )
    .map_err(|error| error.to_string())?;
    verify_pending_text_catalog_boundary(&BOX_PENDING_TEXT_ASSETS, BOX_TEXT_ASSETS)
        .map_err(|error| error.to_string())?;
    verify_promoted_text_catalog_provenance(&BOX_PROMOTED_CELL_GRID_PROVENANCE, BOX_TEXT_ASSETS)
        .map_err(|error| error.to_string())?;
    verify_promoted_raster_text_catalog_provenance(
        &BOX_PROMOTED_MENU_RASTER_PROVENANCE,
        BOX_TEXT_ASSETS,
    )
    .map_err(|error| error.to_string())?;
    verify_promoted_raster_text_catalog_provenance(
        &BOX_PROMOTED_P1_OVER3_PROVENANCE,
        BOX_TEXT_ASSETS,
    )
    .map_err(|error| error.to_string())?;
    verify_promoted_raster_text_catalog_provenance(
        &BOX_PROMOTED_ENDING_RASTER_PROVENANCE,
        BOX_TEXT_ASSETS,
    )
    .map_err(|error| error.to_string())?;
    verify_promoted_raster_text_catalog_provenance(
        &BOX_PROMOTED_G_MESS_PROVENANCE,
        BOX_TEXT_ASSETS,
    )
    .map_err(|error| error.to_string())?;
    verify_promoted_raster_text_catalog_provenance(
        &BOX_PROMOTED_KANBAN_PROVENANCE,
        BOX_TEXT_ASSETS,
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn audit_catalog_assets(cnx_archives: &[(&str, VddArchive)]) -> Result<CatalogAssetAudit, String> {
    let mut audit = CatalogAssetAudit {
        totals: TextCatalogTotals::default(),
        g_mess_draft_patch: None,
        info_page_sources: Vec::new(),
        mm_spr_approved_patch: None,
        mm_spr_review_draft_patch: None,
    };
    for spec in BOX_TEXT_ASSETS {
        let mut spec_report = None;
        let mut spec_physical_assets = 0usize;
        for archive_path in spec.archive_paths {
            let (_, archive) = find_archive(cnx_archives, archive_path, "BTX 텍스트")?;
            let entry = archive
                .find(spec.name)
                .ok_or_else(|| format!("BTX 텍스트 자산이 없음: {archive_path}:{}", spec.name))?;
            let compressed = archive.read_file(entry);
            let plain = verify_format_and_decompress(
                compressed,
                spec.layout.format,
                archive_path,
                spec.name,
                "",
            )?;
            let report = verify_text_asset(&plain, *spec)
                .map_err(|error| format!("{archive_path}:{}: {error}", spec.name))?;
            collect_translation_inputs(&mut audit, archive_path, spec.name, &plain)?;
            audit.totals.physical_assets += 1;
            spec_physical_assets += 1;
            if let Some(previous) = spec_report {
                if report != previous {
                    return Err(format!(
                        "{archive_path}:{} text catalog report가 다른 사본과 다름",
                        spec.name
                    ));
                }
            } else {
                spec_report = Some(report);
            }
        }
        let report =
            spec_report.ok_or_else(|| format!("{}의 물리 BTX 검증 결과가 없음", spec.name))?;
        if spec_physical_assets != report.declared_physical_assets {
            return Err(format!(
                "{} 물리 사본 수 불일치: expected={}, actual={spec_physical_assets}",
                spec.name, report.declared_physical_assets
            ));
        }
        audit.totals.blocks += report.blocks;
        audit.totals.block_pixels += report.block_pixels;
        audit.totals.japanese_target_blocks += report.japanese_target_blocks;
        audit.totals.japanese_target_pixels += report.japanese_target_pixels;
    }
    Ok(audit)
}

fn collect_translation_inputs(
    audit: &mut CatalogAssetAudit,
    archive_path: &'static str,
    name: &'static str,
    plain: &[u8],
) -> Result<(), String> {
    if archive_path == "/FIL_GRP.VDD;1" && name == "MM_SPR.BTX" {
        let (_, report) =
            patch_mm_spr_approved_poc_translations(plain).map_err(|error| error.to_string())?;
        audit.mm_spr_approved_patch = Some(report);
        let (_, report) =
            patch_mm_spr_review_translations(plain).map_err(|error| error.to_string())?;
        audit.mm_spr_review_draft_patch = Some(report);
    }
    if archive_path == "/GOCHA1/TUU_GRP.VDD;1" && name == "G_MESS.BTX" {
        let (_, report) =
            patch_gocha_result_message_drafts(plain).map_err(|error| error.to_string())?;
        audit.g_mess_draft_patch = Some(report);
    }
    if matches!(name, "P2_INFO4.BTX" | "P2_INFO5.BTX") {
        audit.info_page_sources.push(BtxInfoPageSource {
            archive: archive_path,
            name,
            plain: plain.to_vec(),
        });
    }
    Ok(())
}

fn audit_pending_assets(cnx_archives: &[(&str, VddArchive)]) -> Result<usize, String> {
    let mut physical_assets = 0usize;
    for spec in BOX_PENDING_TEXT_ASSETS {
        for archive_path in spec.archive_paths {
            let (_, archive) = find_archive(cnx_archives, archive_path, "pending BTX")?;
            let entry = archive
                .find(spec.name)
                .ok_or_else(|| format!("pending BTX 자산이 없음: {archive_path}:{}", spec.name))?;
            let compressed = archive.read_file(entry);
            let plain = verify_format_and_decompress(
                compressed,
                spec.review_layout.format,
                archive_path,
                spec.name,
                "pending ",
            )?;
            verify_pending_text_asset(&plain, spec)
                .map_err(|error| format!("{archive_path}:{}: {error}", spec.name))?;
            physical_assets += 1;
        }
    }
    Ok(physical_assets)
}

fn print_pending_summary(assets: usize, physical_assets: usize) {
    let runtime_layouts = BOX_PENDING_TEXT_ASSETS
        .iter()
        .filter(|spec| {
            matches!(
                spec.layout_basis,
                ReviewLayoutBasis::RuntimeWholeAssetExact
                    | ReviewLayoutBasis::RuntimeVisibleWholeAssetExact
            )
        })
        .count();
    let static_layouts = BOX_PENDING_TEXT_ASSETS
        .iter()
        .filter(|spec| spec.layout_basis == ReviewLayoutBasis::StaticConsumerExact)
        .count();
    let visual_layouts = BOX_PENDING_TEXT_ASSETS.len() - runtime_layouts - static_layouts;
    if assets == 0 {
        println!("PASS BTX pending extraction: queue empty; known positive assets promoted");
    } else {
        println!(
            "PASS BTX pending extraction: assets {}, physical {}, runtime/static/visual layout \
             {}/{}/{}; product promotion or runtime proof pending",
            assets, physical_assets, runtime_layouts, static_layouts, visual_layouts,
        );
    }
}

fn find_archive<'a>(
    cnx_archives: &'a [(&str, VddArchive)],
    archive_path: &str,
    label: &str,
) -> Result<&'a (&'a str, VddArchive), String> {
    cnx_archives
        .iter()
        .find(|(source_id, _)| source_id == &archive_path)
        .ok_or_else(|| format!("{label} archive가 없음: {archive_path}"))
}

fn verify_format_and_decompress(
    compressed: &[u8],
    expected_format: PixelFormat,
    archive_path: &str,
    name: &str,
    mismatch_prefix: &str,
) -> Result<Vec<u8>, String> {
    let header =
        parse_header(compressed).map_err(|error| format!("{archive_path}:{name}: {error}"))?;
    let format = PixelFormat::from_cnx_subtype(header.subtype)
        .map_err(|error| format!("{archive_path}:{name}: {error}"))?;
    if format != expected_format {
        return Err(format!(
            "{archive_path}:{name} {mismatch_prefix}format 불일치: \
             expected={expected_format:?}, actual={format:?}"
        ));
    }
    decompress(compressed).map_err(|error| format!("{archive_path}:{name}: {error}"))
}
