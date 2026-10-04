//! typed BTX 일본어 block의 patch-family 분할과 raster profile 후보 집계.
//!
//! 현재 catalog, 번역 manifest, pending provenance를 한 분모에서 교차한다.
//! bbox 높이·원문 행 수·pixel format·원본 palette 집합이 같은 block을
//! profile 후보로 묶지만, 이 자동 집계는 category나 번역 승인이 아니다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::btx_raster_family::{
    proposed_family_for_asset, verify_btx_raster_family_proposals, BOX_BTX_RASTER_FAMILY_PROPOSALS,
};
use super::mm_spr_translation::{escape_tsv_field, mm_spr_translation_entries, push_tsv_row};
use super::text_candidates::{verify_pending_text_catalog_specs, BOX_PENDING_TEXT_ASSETS};
use super::text_catalog::{
    is_japanese_translation_target, verify_text_catalog_specs, PaletteIndexSpec, TextStorageSpec,
    BOX_TEXT_ASSETS,
};
use crate::btx::PixelFormat;

pub const BTX_PATCH_STRATEGY_SCHEMA_VERSION: &str = "btx-patch-strategy-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxRasterProfileCandidateGroup {
    pub pixel_format: &'static str,
    pub bbox_height: usize,
    pub source_lines: usize,
    pub palette_indices: Vec<u8>,
    pub blocks: usize,
    pub assets: usize,
    pub minimum_width: usize,
    pub maximum_width: usize,
    pub block_pixels: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxRasterProfileCandidateMember {
    pub candidate_index: usize,
    pub semantic_family_proposal: &'static str,
    pub semantic_family_purpose: &'static str,
    pub archive_paths: &'static [&'static str],
    pub asset_name: &'static str,
    pub asset_sha256: &'static str,
    pub block_id: &'static str,
    pub block_pixels_sha256: &'static str,
    pub bbox_x: usize,
    pub bbox_y: usize,
    pub bbox_width: usize,
    pub bbox_height: usize,
    pub protected_source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxPatchStrategyReport {
    pub schema_version: &'static str,
    pub typed_japanese_blocks: usize,
    pub mm_spr_managed_blocks: usize,
    pub fixed_cell_page_blocks: usize,
    pub remaining_raster_blocks: usize,
    pub remaining_raster_assets: usize,
    pub profile_candidate_groups: Vec<BtxRasterProfileCandidateGroup>,
    pub profile_candidate_members: Vec<BtxRasterProfileCandidateMember>,
    pub semantic_family_proposals: usize,
    pub automatic_semantic_family_approval: bool,
    pub pending_menu06_findings: usize,
    pub pending_menu06_physical_assets: usize,
    pub protected_mm_mes_findings: usize,
    pub protected_mm_mes_physical_assets: usize,
    pub automatic_profile_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxPatchStrategyError(String);

impl fmt::Display for BtxPatchStrategyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for BtxPatchStrategyError {}

fn error(message: impl Into<String>) -> BtxPatchStrategyError {
    BtxPatchStrategyError(message.into())
}

fn pixel_format_label(format: PixelFormat) -> &'static str {
    match format {
        PixelFormat::Indexed4 => "indexed4",
        PixelFormat::Indexed8 => "indexed8",
    }
}

fn palette_indices(spec: PaletteIndexSpec<'_>) -> Vec<u8> {
    match spec {
        PaletteIndexSpec::Values(values) => values.to_vec(),
        PaletteIndexSpec::Bitset256(words) => (0u16..=255)
            .filter(|value| {
                let value = usize::from(*value);
                words[value / 64] & (1u64 << (value % 64)) != 0
            })
            .map(|value| value as u8)
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ProfileCandidateKey {
    pixel_format: &'static str,
    bbox_height: usize,
    source_lines: usize,
    palette_indices: Vec<u8>,
}

#[derive(Debug, Default)]
struct ProfileCandidateAccumulator<'a> {
    blocks: usize,
    assets: BTreeSet<(&'a str, &'a str)>,
    minimum_width: usize,
    maximum_width: usize,
    block_pixels: usize,
}

#[derive(Debug)]
struct ProfileCandidateMemberSeed {
    key: ProfileCandidateKey,
    archive_paths: &'static [&'static str],
    asset_name: &'static str,
    asset_sha256: &'static str,
    block_id: &'static str,
    block_pixels_sha256: &'static str,
    bbox_x: usize,
    bbox_y: usize,
    bbox_width: usize,
    bbox_height: usize,
    protected_source: &'static str,
}

/// typed catalog를 product family와 승인 전 raster profile 후보로 분할한다.
pub fn verify_box_btx_patch_strategy() -> Result<BtxPatchStrategyReport, BtxPatchStrategyError> {
    let catalog = verify_text_catalog_specs(BOX_TEXT_ASSETS)
        .map_err(|source| error(format!("BTX catalog 검증 실패: {source}")))?;
    let mm_spr_entries = mm_spr_translation_entries()
        .map_err(|source| error(format!("MM_SPR manifest 검증 실패: {source}")))?;
    verify_pending_text_catalog_specs(&BOX_PENDING_TEXT_ASSETS)
        .map_err(|source| error(format!("pending BTX 검증 실패: {source}")))?;

    let managed_ids = mm_spr_entries
        .iter()
        .map(|entry| entry.block_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut managed_blocks = 0usize;
    let mut fixed_cell_page_blocks = 0usize;
    let mut remaining_raster_blocks = 0usize;
    let mut remaining_assets = BTreeSet::new();
    let mut groups = BTreeMap::<ProfileCandidateKey, ProfileCandidateAccumulator<'_>>::new();
    let mut member_seeds = Vec::new();

    for asset in BOX_TEXT_ASSETS {
        for block in asset
            .blocks
            .iter()
            .filter(|block| is_japanese_translation_target(block.source))
        {
            if managed_ids.contains(block.id) {
                managed_blocks += 1;
                continue;
            }
            match asset.storage {
                TextStorageSpec::FixedCellGrid(_) => {
                    if !matches!(asset.name, "P2_INFO4.BTX" | "P2_INFO5.BTX") {
                        return Err(error(format!(
                            "미등록 fixed-cell patch family: {}:{}",
                            asset.name, block.id
                        )));
                    }
                    fixed_cell_page_blocks += 1;
                }
                TextStorageSpec::RasterBlocks | TextStorageSpec::SparseSelectorAtlas(_) => {
                    remaining_raster_blocks += 1;
                    remaining_assets.insert((asset.name, asset.expected_decompressed_sha256));
                    let key = ProfileCandidateKey {
                        pixel_format: pixel_format_label(asset.layout.format),
                        bbox_height: block.bbox.height,
                        source_lines: block.source.split('\n').count(),
                        palette_indices: palette_indices(block.palette_indices),
                    };
                    let row = groups.entry(key.clone()).or_default();
                    row.blocks += 1;
                    row.assets
                        .insert((asset.name, asset.expected_decompressed_sha256));
                    row.minimum_width = if row.minimum_width == 0 {
                        block.bbox.width
                    } else {
                        row.minimum_width.min(block.bbox.width)
                    };
                    row.maximum_width = row.maximum_width.max(block.bbox.width);
                    let pixels =
                        block
                            .bbox
                            .width
                            .checked_mul(block.bbox.height)
                            .ok_or_else(|| {
                                error(format!("{}:{} pixel 수 overflow", asset.name, block.id))
                            })?;
                    row.block_pixels = row.block_pixels.checked_add(pixels).ok_or_else(|| {
                        error(format!(
                            "{}:{} profile 후보 pixel 합계 overflow",
                            asset.name, block.id
                        ))
                    })?;
                    member_seeds.push(ProfileCandidateMemberSeed {
                        key,
                        archive_paths: asset.archive_paths,
                        asset_name: asset.name,
                        asset_sha256: asset.expected_decompressed_sha256,
                        block_id: block.id,
                        block_pixels_sha256: block.expected_pixels_sha256,
                        bbox_x: block.bbox.x,
                        bbox_y: block.bbox.y,
                        bbox_width: block.bbox.width,
                        bbox_height: block.bbox.height,
                        protected_source: block.source,
                    });
                }
            }
        }
    }

    let menu06 = BOX_PENDING_TEXT_ASSETS
        .iter()
        .filter(|asset| asset.name == "MENU06.BTX")
        .collect::<Vec<_>>();
    let mm_mes = BOX_PENDING_TEXT_ASSETS
        .iter()
        .filter(|asset| asset.name == "MM_MES.BTX")
        .collect::<Vec<_>>();
    if menu06.len() + mm_mes.len() != BOX_PENDING_TEXT_ASSETS.len() {
        return Err(error("미등록 pending BTX patch family가 있음"));
    }
    let mut group_indices = BTreeMap::new();
    let profile_candidate_groups = groups
        .into_iter()
        .enumerate()
        .map(|(index, (key, row))| {
            group_indices.insert(key.clone(), index + 1);
            BtxRasterProfileCandidateGroup {
                pixel_format: key.pixel_format,
                bbox_height: key.bbox_height,
                source_lines: key.source_lines,
                palette_indices: key.palette_indices,
                blocks: row.blocks,
                assets: row.assets.len(),
                minimum_width: row.minimum_width,
                maximum_width: row.maximum_width,
                block_pixels: row.block_pixels,
            }
        })
        .collect::<Vec<_>>();
    let mut profile_candidate_members = member_seeds
        .into_iter()
        .map(|member| {
            let candidate_index = *group_indices
                .get(&member.key)
                .ok_or_else(|| error(format!("{}의 profile 후보 key가 없음", member.block_id)))?;
            let semantic_family = proposed_family_for_asset(member.asset_name)
                .map_err(|source| error(format!("BTX semantic family 검증 실패: {source}")))?;
            Ok(BtxRasterProfileCandidateMember {
                candidate_index,
                semantic_family_proposal: semantic_family.id,
                semantic_family_purpose: semantic_family.purpose,
                archive_paths: member.archive_paths,
                asset_name: member.asset_name,
                asset_sha256: member.asset_sha256,
                block_id: member.block_id,
                block_pixels_sha256: member.block_pixels_sha256,
                bbox_x: member.bbox_x,
                bbox_y: member.bbox_y,
                bbox_width: member.bbox_width,
                bbox_height: member.bbox_height,
                protected_source: member.protected_source,
            })
        })
        .collect::<Result<Vec<_>, BtxPatchStrategyError>>()?;
    profile_candidate_members
        .sort_by_key(|member| (member.candidate_index, member.asset_name, member.block_id));
    let semantic_families = verify_btx_raster_family_proposals(
        profile_candidate_members
            .iter()
            .map(|member| (member.asset_name, member.block_id)),
    )
    .map_err(|source| error(format!("BTX semantic family 검증 실패: {source}")))?;

    let report = BtxPatchStrategyReport {
        schema_version: BTX_PATCH_STRATEGY_SCHEMA_VERSION,
        typed_japanese_blocks: catalog.japanese_target_blocks,
        mm_spr_managed_blocks: managed_blocks,
        fixed_cell_page_blocks,
        remaining_raster_blocks,
        remaining_raster_assets: remaining_assets.len(),
        profile_candidate_groups,
        profile_candidate_members,
        semantic_family_proposals: semantic_families.families,
        automatic_semantic_family_approval: semantic_families.automatic_approval,
        pending_menu06_findings: menu06.len(),
        pending_menu06_physical_assets: menu06.iter().map(|asset| asset.archive_paths.len()).sum(),
        protected_mm_mes_findings: mm_mes.len(),
        protected_mm_mes_physical_assets: mm_mes
            .iter()
            .map(|asset| asset.archive_paths.len())
            .sum(),
        automatic_profile_approval: false,
    };
    let typed_partition = managed_blocks
        .checked_add(fixed_cell_page_blocks)
        .and_then(|value| value.checked_add(remaining_raster_blocks))
        .ok_or_else(|| error("typed BTX patch family 합계 overflow"))?;
    let grouped_blocks = report
        .profile_candidate_groups
        .iter()
        .map(|group| group.blocks)
        .sum::<usize>();
    if typed_partition != report.typed_japanese_blocks
        || managed_blocks != managed_ids.len()
        || grouped_blocks != report.remaining_raster_blocks
        || report.profile_candidate_members.len() != report.remaining_raster_blocks
        || report.semantic_family_proposals != BOX_BTX_RASTER_FAMILY_PROPOSALS.len()
        || report.automatic_semantic_family_approval
        || report.automatic_profile_approval
    {
        return Err(error(format!(
            "BTX patch strategy 내부 분할 불일치: typed_partition={}/{}, \
             managed_ids={}/{}, grouped/member/raster={}/{}/{}, \
             semantic_families={}/{}, automatic_approval={}/{}",
            typed_partition,
            report.typed_japanese_blocks,
            managed_blocks,
            managed_ids.len(),
            grouped_blocks,
            report.profile_candidate_members.len(),
            report.remaining_raster_blocks,
            report.semantic_family_proposals,
            BOX_BTX_RASTER_FAMILY_PROPOSALS.len(),
            report.automatic_profile_approval,
            report.automatic_semantic_family_approval,
        )));
    }
    Ok(report)
}

/// 사람 검토용 raster profile 후보 TSV. 제품 build는 이를 다시 읽지 않는다.
pub fn render_btx_raster_profile_candidates_tsv() -> Result<String, BtxPatchStrategyError> {
    let report = verify_box_btx_patch_strategy()?;
    Ok(render_btx_raster_profile_candidates_tsv_from(&report))
}

fn render_btx_raster_profile_candidates_tsv_from(report: &BtxPatchStrategyReport) -> String {
    let mut output = String::from(
        "schema_version\tcandidate_index\tpixel_format\tbbox_height\tsource_lines\t\
         palette_indices\tblocks\tassets\tminimum_width\tmaximum_width\tblock_pixels\tstatus\n",
    );
    for (index, row) in report.profile_candidate_groups.iter().enumerate() {
        let palette = row
            .palette_indices
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tprofile-candidate-human-approval-required\n",
            report.schema_version,
            index + 1,
            row.pixel_format,
            row.bbox_height,
            row.source_lines,
            palette,
            row.blocks,
            row.assets,
            row.minimum_width,
            row.maximum_width,
            row.block_pixels,
        ));
    }
    output
}

/// 후보별 실제 block·보호 원문을 함께 내보내는 사람 검토용 TSV.
pub fn render_btx_raster_profile_candidate_members_tsv() -> Result<String, BtxPatchStrategyError> {
    let report = verify_box_btx_patch_strategy()?;
    render_btx_raster_profile_candidate_members_tsv_from(&report)
}

fn render_btx_raster_profile_candidate_members_tsv_from(
    report: &BtxPatchStrategyReport,
) -> Result<String, BtxPatchStrategyError> {
    let mut output = String::from(
        "schema_version\tcandidate_index\tsemantic_family_proposal\tsemantic_family_purpose\t\
         semantic_family_status\tpixel_format\tsource_lines\tpalette_indices\t\
         archive_paths\tasset_name\tasset_sha256\tblock_id\tblock_pixels_sha256\t\
         bbox_x\tbbox_y\tbbox_width\tbbox_height\tprotected_source\tstatus\n",
    );
    for member in &report.profile_candidate_members {
        let group_index = member
            .candidate_index
            .checked_sub(1)
            .ok_or_else(|| error(format!("{}의 profile 후보 index가 0임", member.block_id)))?;
        let group = report
            .profile_candidate_groups
            .get(group_index)
            .ok_or_else(|| {
                error(format!(
                    "{}의 profile 후보 index가 범위 밖임: {}",
                    member.block_id, member.candidate_index
                ))
            })?;
        let palette = group
            .palette_indices
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        push_tsv_row(
            &mut output,
            &[
                report.schema_version.to_string(),
                member.candidate_index.to_string(),
                member.semantic_family_proposal.to_string(),
                escape_tsv_field(member.semantic_family_purpose),
                "semantic-family-proposal-human-approval-required".to_string(),
                group.pixel_format.to_string(),
                group.source_lines.to_string(),
                palette,
                escape_tsv_field(&member.archive_paths.join("\n")),
                member.asset_name.to_string(),
                member.asset_sha256.to_string(),
                member.block_id.to_string(),
                member.block_pixels_sha256.to_string(),
                member.bbox_x.to_string(),
                member.bbox_y.to_string(),
                member.bbox_width.to_string(),
                member.bbox_height.to_string(),
                escape_tsv_field(member.protected_source),
                "profile-candidate-human-approval-required".to_string(),
            ],
        );
    }
    Ok(output)
}

#[cfg(test)]
#[path = "btx_patch_strategy_tests.rs"]
mod tests;
