//! 일반 raster BTX의 원본 배경 보존 정책.
//!
//! 번역문과 font profile은 별도 manifest가 소유한다. 이 모듈은 정확한 원본
//! bbox를 통째로 지워도 되는지, 원본 pixel을 보존하는 composer가 필요한지만
//! block 분모에 결속한다. 자산 한 행으로 묶은 block ID 집합도 catalog가 바뀌면
//! 자동으로 승계하지 않고 실패한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::btx::PixelFormat;

use super::btx_patch_strategy::{
    verify_box_btx_patch_strategy, BtxPatchStrategyError, BtxRasterProfileCandidateMember,
};
use super::text_catalog::{PaletteIndexSpec, TextAssetSpec, TextStorageSpec, BOX_TEXT_ASSETS};

pub const BTX_RASTER_SURFACE_POLICY_SCHEMA_VERSION: &str = "btx-raster-surface-policy-v1";
const SURFACE_POLICY_MANIFEST: &str =
    include_str!("../../assets/graphics/btx/raster_surface_policies.tsv");
const SURFACE_POLICY_HEADER: &str = "schema_version\tarchive_paths\tasset_name\tasset_sha256\t\
                                     block_ids\tbackground_policy\tbackground_index\t\
                                     write_mechanism";

/// 원문 bbox에서 번역문 밖의 pixel을 어떻게 다뤄야 하는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterBackgroundPolicy {
    FlatBackground,
    GeneratedArtwork,
    SourceBackgroundPreserved,
    ScopeExcludedUnassessed,
}

/// 배경 정책을 만족하는 writer의 종류와 현재 구현 경계.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterWriteMechanism {
    WholeBbox,
    SourceTextMask,
    SourceTextMaskRequired,
    SparseSelectorAtlasRequired,
    ProtectedNoWrite,
}

/// 같은 정책을 공유하는 한 자산의 명시적 block 집합.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterSurfacePolicyEntry {
    pub archive_paths: String,
    pub asset_name: String,
    pub asset_sha256: String,
    pub block_ids: Vec<String>,
    pub background_policy: RasterBackgroundPolicy,
    pub background_index: Option<u8>,
    pub write_mechanism: RasterWriteMechanism,
}

/// 일반 raster 분모의 배경·writer 분류 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterSurfacePolicyReport {
    pub policy_groups: usize,
    pub assets: usize,
    pub blocks: usize,
    pub flat_background_blocks: usize,
    pub source_background_preserved_blocks: usize,
    pub generated_artwork_blocks: usize,
    pub scope_excluded_unassessed_blocks: usize,
    pub whole_bbox_blocks: usize,
    pub source_text_mask_blocks: usize,
    pub composer_required_blocks: usize,
    pub protected_no_write_blocks: usize,
}

/// surface 정책 입력·catalog 결속 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterSurfacePolicyError(String);

impl fmt::Display for RasterSurfacePolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "일반 raster surface 정책 실패: {}", self.0)
    }
}

impl Error for RasterSurfacePolicyError {}

impl From<BtxPatchStrategyError> for RasterSurfacePolicyError {
    fn from(source: BtxPatchStrategyError) -> Self {
        Self(format!("patch strategy 검증 실패: {source}"))
    }
}

fn error(message: impl Into<String>) -> RasterSurfacePolicyError {
    RasterSurfacePolicyError(message.into())
}

fn unescape_tsv_field(value: &str, line: usize) -> Result<String, RasterSurfacePolicyError> {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let escaped = characters
            .next()
            .ok_or_else(|| error(format!("{line}행 TSV escape가 끝에서 잘림")))?;
        match escaped {
            '\\' => output.push('\\'),
            'n' => output.push('\n'),
            _ => {
                return Err(error(format!(
                    "{line}행 알 수 없는 TSV escape: \\{escaped}"
                )))
            }
        }
    }
    Ok(output)
}

fn parse_background_policy(
    value: &str,
    line: usize,
) -> Result<RasterBackgroundPolicy, RasterSurfacePolicyError> {
    match value {
        "generated-artwork" => Ok(RasterBackgroundPolicy::GeneratedArtwork),
        "flat-background" => Ok(RasterBackgroundPolicy::FlatBackground),
        "source-background-preserved" => Ok(RasterBackgroundPolicy::SourceBackgroundPreserved),
        "scope-excluded-unassessed" => Ok(RasterBackgroundPolicy::ScopeExcludedUnassessed),
        _ => Err(error(format!("{line}행 background_policy 미지원: {value}"))),
    }
}

fn parse_write_mechanism(
    value: &str,
    line: usize,
) -> Result<RasterWriteMechanism, RasterSurfacePolicyError> {
    match value {
        "whole-bbox" => Ok(RasterWriteMechanism::WholeBbox),
        "source-text-mask" => Ok(RasterWriteMechanism::SourceTextMask),
        "source-text-mask-required" => Ok(RasterWriteMechanism::SourceTextMaskRequired),
        "sparse-selector-atlas-required" => Ok(RasterWriteMechanism::SparseSelectorAtlasRequired),
        "protected-no-write" => Ok(RasterWriteMechanism::ProtectedNoWrite),
        _ => Err(error(format!("{line}행 write_mechanism 미지원: {value}"))),
    }
}

fn parse_block_ids(value: &str, line: usize) -> Result<Vec<String>, RasterSurfacePolicyError> {
    if value.is_empty() {
        return Err(error(format!("{line}행 block_ids가 비어 있음")));
    }
    let block_ids = value.split(',').map(str::to_string).collect::<Vec<_>>();
    let unique = block_ids.iter().collect::<BTreeSet<_>>();
    if unique.len() != block_ids.len() {
        return Err(error(format!("{line}행 block_ids에 중복이 있음")));
    }
    let mut canonical = block_ids.clone();
    canonical.sort();
    if canonical != block_ids {
        return Err(error(format!("{line}행 block_ids가 오름차순이 아님")));
    }
    Ok(block_ids)
}

fn parse_manifest(raw: &str) -> Result<Vec<RasterSurfacePolicyEntry>, RasterSurfacePolicyError> {
    if !raw.ends_with('\n') || raw.contains('\r') {
        return Err(error(
            "surface 정책 manifest는 LF로 끝나야 하며 CR을 포함할 수 없음",
        ));
    }
    if raw.lines().next() != Some(SURFACE_POLICY_HEADER) {
        return Err(error("surface 정책 manifest header 불일치"));
    }
    raw.lines()
        .skip(1)
        .enumerate()
        .map(|(index, row)| {
            let line = index + 2;
            let fields = row.split('\t').collect::<Vec<_>>();
            let [schema, archives, asset, asset_sha, block_ids, background_policy, background_index, write_mechanism] =
                fields.as_slice()
            else {
                return Err(error(format!("{line}행 surface 정책 field 수 불일치")));
            };
            if *schema != BTX_RASTER_SURFACE_POLICY_SCHEMA_VERSION {
                return Err(error(format!("{line}행 surface 정책 schema 불일치")));
            }
            if asset.is_empty()
                || asset_sha.len() != 64
                || !asset_sha
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            {
                return Err(error(format!("{line}행 자산 identity 형식 불일치")));
            }
            let background_policy = parse_background_policy(background_policy, line)?;
            let background_index = if background_index.is_empty() {
                None
            } else {
                Some(background_index.parse::<u8>().map_err(|_| {
                    error(format!("{line}행 background_index가 u8이 아님"))
                })?)
            };
            let write_mechanism = parse_write_mechanism(write_mechanism, line)?;
            match (background_policy, background_index, write_mechanism) {
                (
                    RasterBackgroundPolicy::FlatBackground,
                    Some(_),
                    RasterWriteMechanism::WholeBbox,
                )
                | (RasterBackgroundPolicy::GeneratedArtwork, None, RasterWriteMechanism::WholeBbox)
                | (
                    RasterBackgroundPolicy::SourceBackgroundPreserved,
                    None,
                    RasterWriteMechanism::SourceTextMask
                    | RasterWriteMechanism::SourceTextMaskRequired
                    | RasterWriteMechanism::SparseSelectorAtlasRequired,
                )
                | (
                    RasterBackgroundPolicy::ScopeExcludedUnassessed,
                    None,
                    RasterWriteMechanism::ProtectedNoWrite,
                ) => {}
                _ => {
                    return Err(error(format!(
                        "{line}행 background 정책과 write mechanism 조합 불일치"
                    )))
                }
            }
            Ok(RasterSurfacePolicyEntry {
                archive_paths: unescape_tsv_field(archives, line)?,
                asset_name: (*asset).to_string(),
                asset_sha256: (*asset_sha).to_string(),
                block_ids: parse_block_ids(block_ids, line)?,
                background_policy,
                background_index,
                write_mechanism,
            })
        })
        .collect()
}

fn palette_contains(spec: PaletteIndexSpec<'_>, needle: u8) -> bool {
    match spec {
        PaletteIndexSpec::Values(values) => values.contains(&needle),
        PaletteIndexSpec::Bitset256(words) => {
            let needle = usize::from(needle);
            words[needle / 64] & (1u64 << (needle % 64)) != 0
        }
    }
}

fn expected_member<'a>(
    members: &'a [BtxRasterProfileCandidateMember],
    block_id: &str,
) -> Result<&'a BtxRasterProfileCandidateMember, RasterSurfacePolicyError> {
    members
        .iter()
        .find(|member| member.block_id == block_id)
        .ok_or_else(|| error(format!("catalog에 없는 surface 정책 block: {block_id}")))
}

fn expected_asset(asset_name: &str) -> Result<TextAssetSpec<'static>, RasterSurfacePolicyError> {
    BOX_TEXT_ASSETS
        .iter()
        .copied()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| error(format!("catalog에 없는 surface 정책 자산: {asset_name}")))
}

fn verify_entry(
    entry: &RasterSurfacePolicyEntry,
    members: &[BtxRasterProfileCandidateMember],
) -> Result<(), RasterSurfacePolicyError> {
    let asset = expected_asset(&entry.asset_name)?;
    if entry.archive_paths != asset.archive_paths.join("\n")
        || entry.asset_sha256 != asset.expected_decompressed_sha256
    {
        return Err(error(format!(
            "{}의 archive 또는 SHA-256이 catalog와 다름",
            entry.asset_name
        )));
    }
    match (entry.write_mechanism, asset.storage) {
        (
            RasterWriteMechanism::SparseSelectorAtlasRequired,
            TextStorageSpec::SparseSelectorAtlas(_),
        )
        | (
            RasterWriteMechanism::WholeBbox
            | RasterWriteMechanism::SourceTextMask
            | RasterWriteMechanism::SourceTextMaskRequired
            | RasterWriteMechanism::ProtectedNoWrite,
            TextStorageSpec::RasterBlocks,
        ) => {}
        _ => {
            return Err(error(format!(
                "{}의 write mechanism과 catalog storage 불일치",
                entry.asset_name
            )))
        }
    }
    if let Some(background) = entry.background_index {
        match asset.layout.format {
            PixelFormat::Indexed4 if background > 15 => {
                return Err(error(format!(
                    "{}의 background index가 indexed4 범위 밖임",
                    entry.asset_name
                )))
            }
            PixelFormat::Indexed4 | PixelFormat::Indexed8 => {}
        }
    }
    for block_id in &entry.block_ids {
        let member = expected_member(members, block_id)?;
        if member.asset_name != entry.asset_name
            || member.asset_sha256 != entry.asset_sha256
            || member.archive_paths.join("\n") != entry.archive_paths
        {
            return Err(error(format!(
                "{block_id}의 자산 identity가 surface 정책과 다름"
            )));
        }
        let block = asset
            .blocks
            .iter()
            .find(|block| block.id == block_id)
            .ok_or_else(|| error(format!("{}에 {block_id}가 없음", entry.asset_name)))?;
        if entry
            .background_index
            .is_some_and(|background| !palette_contains(block.palette_indices, background))
        {
            return Err(error(format!(
                "{block_id}의 background index가 보호 palette 집합 밖임"
            )));
        }
    }
    Ok(())
}

fn verify_manifest(
    raw: &str,
) -> Result<(Vec<RasterSurfacePolicyEntry>, RasterSurfacePolicyReport), RasterSurfacePolicyError> {
    let entries = parse_manifest(raw)?;
    let strategy = verify_box_btx_patch_strategy()?;
    let mut previous_key: Option<(&str, &str)> = None;
    let mut assets = BTreeSet::new();
    let mut blocks = BTreeSet::new();
    for entry in &entries {
        let first_block = entry
            .block_ids
            .first()
            .ok_or_else(|| error(format!("{}의 block 집합이 비어 있음", entry.asset_name)))?;
        let key = (entry.asset_name.as_str(), first_block.as_str());
        if previous_key.is_some_and(|previous| previous >= key) {
            return Err(error("surface 정책 행이 자산·block 순으로 정렬되지 않음"));
        }
        previous_key = Some(key);
        verify_entry(entry, &strategy.profile_candidate_members)?;
        assets.insert(entry.asset_name.as_str());
        for block_id in &entry.block_ids {
            if !blocks.insert(block_id.as_str()) {
                return Err(error(format!("surface 정책 block 중복: {block_id}")));
            }
        }
    }
    let expected_blocks = strategy
        .profile_candidate_members
        .iter()
        .map(|member| member.block_id)
        .collect::<BTreeSet<_>>();
    if blocks != expected_blocks
        || assets.len() != strategy.remaining_raster_assets
        || blocks.len() != strategy.remaining_raster_blocks
    {
        let missing = expected_blocks
            .difference(&blocks)
            .copied()
            .collect::<Vec<_>>();
        let extra = blocks
            .difference(&expected_blocks)
            .copied()
            .collect::<Vec<_>>();
        return Err(error(format!(
            "surface 정책 분모 불일치: assets {}/{}, blocks {}/{}, missing={missing:?}, extra={extra:?}",
            assets.len(),
            strategy.remaining_raster_assets,
            blocks.len(),
            strategy.remaining_raster_blocks,
        )));
    }

    let count_blocks = |predicate: fn(&RasterSurfacePolicyEntry) -> bool| -> usize {
        entries
            .iter()
            .filter(|entry| predicate(entry))
            .map(|entry| entry.block_ids.len())
            .sum()
    };
    let flat_background_blocks =
        count_blocks(|entry| entry.background_policy == RasterBackgroundPolicy::FlatBackground);
    let generated_artwork_blocks =
        count_blocks(|entry| entry.background_policy == RasterBackgroundPolicy::GeneratedArtwork);
    let source_background_preserved_blocks = count_blocks(|entry| {
        entry.background_policy == RasterBackgroundPolicy::SourceBackgroundPreserved
    });
    let scope_excluded_unassessed_blocks = count_blocks(|entry| {
        entry.background_policy == RasterBackgroundPolicy::ScopeExcludedUnassessed
    });
    let whole_bbox_blocks =
        count_blocks(|entry| entry.write_mechanism == RasterWriteMechanism::WholeBbox);
    let source_text_mask_blocks =
        count_blocks(|entry| entry.write_mechanism == RasterWriteMechanism::SourceTextMask);
    let composer_required_blocks = count_blocks(|entry| {
        matches!(
            entry.write_mechanism,
            RasterWriteMechanism::SourceTextMaskRequired
                | RasterWriteMechanism::SparseSelectorAtlasRequired
        )
    });
    let protected_no_write_blocks =
        count_blocks(|entry| entry.write_mechanism == RasterWriteMechanism::ProtectedNoWrite);
    if flat_background_blocks
        .checked_add(generated_artwork_blocks)
        .and_then(|count| count.checked_add(source_background_preserved_blocks))
        .and_then(|count| count.checked_add(scope_excluded_unassessed_blocks))
        != Some(blocks.len())
        || whole_bbox_blocks
            .checked_add(source_text_mask_blocks)
            .and_then(|count| count.checked_add(composer_required_blocks))
            .and_then(|count| count.checked_add(protected_no_write_blocks))
            != Some(blocks.len())
    {
        return Err(error("surface 정책 집계 overflow 또는 분할 불일치"));
    }
    let report = RasterSurfacePolicyReport {
        policy_groups: entries.len(),
        assets: assets.len(),
        blocks: blocks.len(),
        flat_background_blocks,
        source_background_preserved_blocks,
        generated_artwork_blocks,
        scope_excluded_unassessed_blocks,
        whole_bbox_blocks,
        source_text_mask_blocks,
        composer_required_blocks,
        protected_no_write_blocks,
    };
    Ok((entries, report))
}

/// 모든 일반 raster 일본어 block이 정확히 한 surface 정책에 속하는지 검증한다.
pub fn verify_general_btx_raster_surface_policies(
) -> Result<RasterSurfacePolicyReport, RasterSurfacePolicyError> {
    verify_manifest(SURFACE_POLICY_MANIFEST).map(|(_, report)| report)
}

/// 평탄 배경 writer가 소비할 exact block별 배경 index를 검증해 반환한다.
pub fn flat_background_indices_for_asset(
    asset: TextAssetSpec<'static>,
) -> Result<BTreeMap<String, u8>, RasterSurfacePolicyError> {
    let (entries, _) = verify_manifest(SURFACE_POLICY_MANIFEST)?;
    let matching = entries
        .iter()
        .filter(|entry| entry.asset_name == asset.name)
        .collect::<Vec<_>>();
    if matching.is_empty()
        || matching.iter().any(|entry| {
            entry.asset_sha256 != asset.expected_decompressed_sha256
                || entry.archive_paths != asset.archive_paths.join("\n")
                || entry.background_policy != RasterBackgroundPolicy::FlatBackground
                || entry.write_mechanism != RasterWriteMechanism::WholeBbox
                || entry.background_index.is_none()
        })
    {
        return Err(error(format!(
            "{}는 검증된 flat-background whole-bbox 자산이 아님",
            asset.name
        )));
    }
    let mut backgrounds = BTreeMap::new();
    for entry in matching {
        let background = entry
            .background_index
            .ok_or_else(|| error(format!("{}의 background index 누락", entry.asset_name)))?;
        for block_id in &entry.block_ids {
            backgrounds.insert(block_id.clone(), background);
        }
    }
    Ok(backgrounds)
}

/// source text mask writer가 소비할 자산·block 집합을 검증한다.
pub fn verify_source_text_mask_policy_for_asset(
    asset: TextAssetSpec<'static>,
) -> Result<usize, RasterSurfacePolicyError> {
    let (entries, _) = verify_manifest(SURFACE_POLICY_MANIFEST)?;
    let matching = entries
        .iter()
        .filter(|entry| entry.asset_name == asset.name)
        .collect::<Vec<_>>();
    if matching.is_empty()
        || matching.iter().any(|entry| {
            entry.asset_sha256 != asset.expected_decompressed_sha256
                || entry.archive_paths != asset.archive_paths.join("\n")
                || entry.background_policy != RasterBackgroundPolicy::SourceBackgroundPreserved
                || entry.write_mechanism != RasterWriteMechanism::SourceTextMask
                || entry.background_index.is_some()
        })
    {
        return Err(error(format!(
            "{}는 검증된 source-text-mask 자산이 아님",
            asset.name
        )));
    }
    Ok(matching.iter().map(|entry| entry.block_ids.len()).sum())
}

/// 생성된 배경과 글자를 함께 교체할 자산·block 집합을 검증한다.
pub fn verify_generated_artwork_policy_for_asset(
    asset: TextAssetSpec<'static>,
) -> Result<usize, RasterSurfacePolicyError> {
    let (entries, _) = verify_manifest(SURFACE_POLICY_MANIFEST)?;
    let matching = entries
        .iter()
        .filter(|entry| entry.asset_name == asset.name)
        .collect::<Vec<_>>();
    if matching.is_empty()
        || matching.iter().any(|entry| {
            entry.asset_sha256 != asset.expected_decompressed_sha256
                || entry.archive_paths != asset.archive_paths.join("\n")
                || entry.background_policy != RasterBackgroundPolicy::GeneratedArtwork
                || entry.write_mechanism != RasterWriteMechanism::WholeBbox
                || entry.background_index.is_some()
        })
    {
        return Err(error(format!(
            "{}는 검증된 generated-artwork 자산이 아님",
            asset.name
        )));
    }
    Ok(matching.iter().map(|entry| entry.block_ids.len()).sum())
}

/// sparse selector atlas composer가 소비할 자산·block 집합을 검증한다.
pub fn verify_sparse_selector_policy_for_asset(
    asset: TextAssetSpec<'static>,
) -> Result<usize, RasterSurfacePolicyError> {
    let (entries, _) = verify_manifest(SURFACE_POLICY_MANIFEST)?;
    let matching = entries
        .iter()
        .filter(|entry| entry.asset_name == asset.name)
        .collect::<Vec<_>>();
    if matching.is_empty()
        || matching.iter().any(|entry| {
            entry.asset_sha256 != asset.expected_decompressed_sha256
                || entry.archive_paths != asset.archive_paths.join("\n")
                || entry.background_policy != RasterBackgroundPolicy::SourceBackgroundPreserved
                || entry.write_mechanism != RasterWriteMechanism::SparseSelectorAtlasRequired
                || entry.background_index.is_some()
        })
    {
        return Err(error(format!(
            "{}는 검증된 sparse-selector-atlas 자산이 아님",
            asset.name
        )));
    }
    Ok(matching.iter().map(|entry| entry.block_ids.len()).sum())
}

#[cfg(test)]
#[path = "btx_raster_surface_policy_tests.rs"]
mod tests;
