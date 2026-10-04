//! graphics source stage의 검증 순서와 결과 전달만 조정한다.

#[path = "graphics_consumers/classic_menu_surfaces.rs"]
mod classic_menu_surfaces;
#[path = "graphics_consumers/classic_registration.rs"]
mod classic_registration;
#[path = "graphics_consumers/font_assets.rs"]
mod font_assets;
#[path = "graphics_consumers/gameplay_surfaces.rs"]
mod gameplay_surfaces;
#[path = "graphics_consumers/menu06_draft.rs"]
mod menu06_draft;
#[path = "graphics_consumers/mm_mes.rs"]
mod mm_mes;
#[path = "graphics_consumers/pending_btx.rs"]
mod pending_btx;

use super::{
    ArchiveAuditData, CommonFontDrawSiteAnalysis, SourceAuditContext, SubgameAssetAuditData,
    SubgameTextAuditData,
};

use classic_menu_surfaces::audit_classic_menu_surfaces;
use classic_registration::audit_classic_registration;
use font_assets::audit_font_assets;
use gameplay_surfaces::audit_gameplay_surfaces;
use menu06_draft::audit_menu06_draft;
use mm_mes::audit_mm_mes;
use pending_btx::audit_pending_btx;

pub(super) fn audit_graphics_consumers(
    context: &SourceAuditContext<'_>,
    archives: &ArchiveAuditData,
    text: &SubgameTextAuditData,
    assets: &SubgameAssetAuditData,
    common_font_draw_sites: &CommonFontDrawSiteAnalysis,
) -> Result<(), String> {
    let box_executables = &context.box_executables;
    let cnx_archives = archives.archive_copies();
    let subgame_seq_sources = &text.sources;
    let btx_name_consumers = &assets.btx_names;
    let pending_btx_selections = &assets.btx_descriptor_selections;

    let text_surface_cells = audit_pending_btx(
        box_executables,
        subgame_seq_sources,
        &text.layouts,
        btx_name_consumers,
        pending_btx_selections,
    )?;
    audit_mm_mes(
        box_executables,
        &cnx_archives,
        subgame_seq_sources,
        &text_surface_cells,
    )?;
    audit_classic_registration(
        box_executables,
        &cnx_archives,
        btx_name_consumers,
        pending_btx_selections,
    )?;
    audit_gameplay_surfaces(box_executables, btx_name_consumers)?;
    audit_classic_menu_surfaces(
        box_executables,
        subgame_seq_sources,
        &text.layouts,
        btx_name_consumers,
        pending_btx_selections,
        common_font_draw_sites,
    )?;
    audit_menu06_draft(&cnx_archives, subgame_seq_sources)?;
    audit_font_assets(box_executables, &cnx_archives, &assets.e6_translation)
}
