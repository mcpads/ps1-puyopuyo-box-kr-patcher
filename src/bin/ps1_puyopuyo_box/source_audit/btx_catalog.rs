//! 전체 BTX inventory와 검증 단계의 실행 순서를 조정한다.

#[path = "btx_catalog/families.rs"]
mod families;
#[path = "btx_catalog/text_catalog.rs"]
mod text_catalog;
#[path = "btx_catalog/translation_development.rs"]
mod translation_development;

use ps1_puyopuyo_box::source::BOX_EXE_ISO_PATHS;

use super::super::disc_commands::audit_btx_data;
use super::super::BOX_GRP_VDDS;
use super::{ArchiveAuditData, SourceAuditContext, SubgameAssetAuditData, SubgameTextAuditData};
use families::audit_btx_families;
use text_catalog::audit_btx_text_catalog;
use translation_development::audit_btx_translation_development;

pub(super) fn audit_btx_catalog(
    context: &SourceAuditContext<'_>,
    archives: &ArchiveAuditData,
    subgame_text: &SubgameTextAuditData,
    subgame_assets: &SubgameAssetAuditData,
) -> Result<(), String> {
    let vdd_sources = &archives.vdd_sources;
    let cnx_archives = archives.archive_copies();
    let grp_sources: Vec<_> = vdd_sources
        .iter()
        .filter(|(source_id, _)| source_id.ends_with("GRP.VDD;1"))
        .collect();
    if grp_sources.len() != BOX_GRP_VDDS {
        return Err(format!(
            "원본 GRP VDD 수 불일치: expected={BOX_GRP_VDDS}, actual={}",
            grp_sources.len()
        ));
    }
    audit_btx_data(
        grp_sources
            .iter()
            .map(|(source_id, raw)| (source_id.as_str(), raw.as_slice())),
    )?;

    audit_btx_families(context, subgame_text, subgame_assets, &cnx_archives)?;
    let development_inputs = audit_btx_text_catalog(&cnx_archives)?;
    audit_btx_translation_development(development_inputs)?;

    println!(
        "PASS source disc: sectors {} (Form1 {}, Form2 {}), ISO entries {}, EXE {}, \
         VDD {}, GRP {}",
        context.source.image().sector_count(),
        context.sectors.form1,
        context.sectors.form2,
        context.iso.entries().len(),
        BOX_EXE_ISO_PATHS.len(),
        vdd_sources.len(),
        grp_sources.len()
    );

    Ok(())
}
