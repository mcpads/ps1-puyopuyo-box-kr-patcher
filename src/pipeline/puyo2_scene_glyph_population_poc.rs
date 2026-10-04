//! PUYO2 scene-local atlas 전체 모집단의 반복 한글 진단 PoC.
//!
//! 룰렛 상대를 미리 맞추지 않고도 runtime pixel source를 판정하도록 17개
//! same-stem atlas와 그 안에 포함되는 16개 atlas-compatible selected 후보의
//! canonical cell을 `가`로 채운다. SKEL selected `MZ_ARLE.BTX`는 형상이
//! 다르므로 쓰지 않는다. 실제 번역문과 SEQ는 바꾸지 않으며 제품 적격이 아니다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::analysis::{
    BOX_SUBGAME_SEQ_SCENE_COMPOUND_SPECS, BOX_SUBGAME_SEQ_SCENE_FAMILIES,
    BOX_SUBGAME_SEQ_SCENE_GLYPH_FAMILIES,
};
use crate::archive::VddArchive;
use crate::compress::{compress, decompress, parse_header};
use crate::disc::{apply_form1_write_plan, DiscImage, Iso9660, VerifiedSourceImage};
use crate::patch::WritePlan;
use crate::pipeline::subgame_seq_scene_glyph_probe::{
    patch_box_subgame_seq_scene_glyph_probe_asset, SubgameSeqSceneGlyphProbePatch,
};

use super::vdd_plan::{add_vdd_repack, logical_file};

pub const PUYO2_SCENE_GLYPH_POPULATION_POC_SCHEMA_VERSION: &str =
    "puyo2-scene-glyph-population-poc-v1";
pub const BOX_PUYO2_SCENE_GLYPH_POPULATION_ASSETS: usize = 17;
pub const BOX_PUYO2_SCENE_GLYPH_POPULATION_CANDIDATE_LINKS: usize = 33;
pub const BOX_PUYO2_SCENE_GLYPH_POPULATION_INCOMPATIBLE_SELECTED: usize = 1;

pub const BOX_PUYO2_SCENE_GLYPH_POPULATION_CELLS: usize = 850;
pub const BOX_PUYO2_SCENE_GLYPH_POPULATION_CHANGED_PIXELS: usize = 86_734;
pub const BOX_PUYO2_SCENE_GLYPH_POPULATION_CHANGED_BYTES: usize = 56_222;
pub const BOX_PUYO2_SCENE_GLYPH_POPULATION_DATA_WRITES: usize = 727;
pub const BOX_PUYO2_SCENE_GLYPH_POPULATION_TOUCHED_SECTORS: usize = 727;
pub const BOX_PUYO2_SCENE_GLYPH_POPULATION_FINAL_EXPECTED_WRITES: usize = 1_454;
pub const BOX_PUYO2_SCENE_GLYPH_POPULATION_IMAGE_SHA256: &str =
    "d29517cdb91ebf4eef0cd673ecb31ed719b873dcb65309865c44f19f1b1eb31e";

const PP2_GRP_PATH: &str = "/PUYO2/PP2_GRP.VDD;1";
const INCOMPATIBLE_SELECTED_ASSET: &str = "MZ_ARLE.BTX";

#[derive(Debug, Clone, PartialEq, Eq)]
struct PopulationTarget {
    asset: &'static str,
    consumers: Vec<String>,
    repertoire_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2SceneGlyphPopulationAssetReport {
    pub asset: &'static str,
    pub consumers: Vec<String>,
    pub repertoire_indices: Vec<usize>,
    pub cell_indices: Vec<usize>,
    pub source_palette_indices: Vec<u8>,
    pub cnx_before: usize,
    pub cnx_after: usize,
    pub changed_pixels: usize,
    pub changed_plain_bytes: usize,
    pub source_plain_sha256: String,
    pub output_plain_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2SceneGlyphPopulationPocReport {
    pub schema_version: &'static str,
    pub synthetic_glyph: char,
    pub assets: Vec<Puyo2SceneGlyphPopulationAssetReport>,
    pub candidate_links: usize,
    pub incompatible_selected_assets: Vec<&'static str>,
    pub changed_cells: usize,
    pub changed_pixels: usize,
    pub changed_plain_bytes: usize,
    pub data_writes: usize,
    pub touched_sectors: usize,
    pub final_expected_writes: usize,
    pub image_sha256: String,
    pub product_eligible: bool,
}

#[derive(Debug)]
pub struct Puyo2SceneGlyphPopulationPocBuild {
    pub image: Vec<u8>,
    pub report: Puyo2SceneGlyphPopulationPocReport,
}

#[derive(Debug)]
pub(crate) struct PreparedPuyo2SceneGlyphPopulation {
    pub(crate) replacements: BTreeMap<String, Vec<u8>>,
    pub(crate) targets: BTreeMap<String, ([u8; 3], Vec<u8>)>,
    pub(crate) reports: Vec<Puyo2SceneGlyphPopulationAssetReport>,
    pub(crate) candidate_links: usize,
    pub(crate) incompatible_selected_assets: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2SceneGlyphPopulationPocError(String);

impl fmt::Display for Puyo2SceneGlyphPopulationPocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Puyo2SceneGlyphPopulationPocError {}

fn error(message: impl Into<String>) -> Puyo2SceneGlyphPopulationPocError {
    Puyo2SceneGlyphPopulationPocError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn canonical_slots(sequence: &str, grid_slots: usize) -> usize {
    BOX_SUBGAME_SEQ_SCENE_COMPOUND_SPECS
        .iter()
        .find(|spec| spec.sequence == sequence)
        .map_or(grid_slots, |spec| spec.canonical_glyph_slots)
}

fn population_targets(
) -> Result<(Vec<PopulationTarget>, usize, usize), Puyo2SceneGlyphPopulationPocError> {
    let mut targets = BTreeMap::<&'static str, (BTreeSet<String>, BTreeSet<usize>)>::new();
    let mut candidate_links = 0usize;
    let mut incompatible_selected = 0usize;

    for family in BOX_SUBGAME_SEQ_SCENE_FAMILIES {
        let slots = canonical_slots(family.sequence, family.grid_slots);
        let own = targets.entry(family.same_stem_asset).or_default();
        own.0.insert(format!("same-stem:{}", family.sequence));
        own.1.extend(0..slots);
        candidate_links += 1;

        let selected = BOX_SUBGAME_SEQ_SCENE_FAMILIES.iter().find(|candidate| {
            candidate
                .puyo2_resource_id
                .checked_add(1)
                .is_some_and(|resource| resource == family.puyo2_resource_id)
        });
        if let Some(selected) = selected {
            let target = targets.entry(selected.same_stem_asset).or_default();
            target.0.insert(format!("selected:{}", family.sequence));
            target.1.extend(0..slots);
            candidate_links += 1;
        } else {
            if family.sequence != "SKEL_SEQ.SEQ" || family.puyo2_resource_id != 0x0089 {
                return Err(error(format!(
                    "{} PUYO2 selected predecessor 누락",
                    family.sequence
                )));
            }
            incompatible_selected += 1;
        }
    }

    let rows = targets
        .into_iter()
        .map(
            |(asset, (consumers, repertoire_indices))| PopulationTarget {
                asset,
                consumers: consumers.into_iter().collect(),
                repertoire_indices: repertoire_indices.into_iter().collect(),
            },
        )
        .collect::<Vec<_>>();
    if rows.len() != BOX_PUYO2_SCENE_GLYPH_POPULATION_ASSETS
        || rows.len() != BOX_SUBGAME_SEQ_SCENE_GLYPH_FAMILIES
        || candidate_links != BOX_PUYO2_SCENE_GLYPH_POPULATION_CANDIDATE_LINKS
        || incompatible_selected != BOX_PUYO2_SCENE_GLYPH_POPULATION_INCOMPATIBLE_SELECTED
    {
        return Err(error(format!(
            "PUYO2 scene glyph population 분모 불일치: assets={}, links={candidate_links}, incompatible={incompatible_selected}",
            rows.len()
        )));
    }
    Ok((rows, candidate_links, incompatible_selected))
}

fn verify_final_archive(
    final_raw: &[u8],
    original: &VddArchive,
    expected: &BTreeMap<&str, ([u8; 3], Vec<u8>)>,
) -> Result<(), Puyo2SceneGlyphPopulationPocError> {
    let final_archive = VddArchive::parse(final_raw)
        .map_err(|source| error(format!("최종 PP2_GRP VDD 파싱 실패: {source}")))?;
    if final_archive.entry_count() != original.entry_count() {
        return Err(error("최종 PP2_GRP VDD 엔트리 수 변경"));
    }
    for entry in original.entries() {
        let final_entry = final_archive
            .find(entry.name())
            .ok_or_else(|| error(format!("최종 PP2_GRP 엔트리 누락: {}", entry.name())))?;
        if let Some((subtype, plain)) = expected.get(entry.name()) {
            let cnx = final_archive.read_file(final_entry);
            let header = parse_header(cnx).map_err(|source| {
                error(format!(
                    "최종 PP2_GRP:{} CNX header 실패: {source}",
                    entry.name()
                ))
            })?;
            if header.subtype != *subtype {
                return Err(error(format!(
                    "최종 PP2_GRP:{} CNX subtype 변경",
                    entry.name()
                )));
            }
            let actual = decompress(cnx).map_err(|source| {
                error(format!("최종 PP2_GRP:{} 해제 실패: {source}", entry.name()))
            })?;
            if actual != *plain {
                return Err(error(format!(
                    "최종 PP2_GRP:{} plain readback 불일치",
                    entry.name()
                )));
            }
        } else if final_archive.read_file(final_entry) != original.read_file(entry) {
            return Err(error(format!(
                "최종 PP2_GRP:{} 비대상 payload 변경",
                entry.name()
            )));
        }
    }
    Ok(())
}

pub(crate) fn prepare_puyo2_scene_glyph_population(
    archive: &VddArchive,
) -> Result<PreparedPuyo2SceneGlyphPopulation, Puyo2SceneGlyphPopulationPocError> {
    let (targets, candidate_links, _incompatible_selected) = population_targets()?;
    let mut replacements = BTreeMap::new();
    let mut expected = BTreeMap::new();
    let mut reports = Vec::new();
    for target in targets {
        let entry = archive
            .find(target.asset)
            .ok_or_else(|| error(format!("{PP2_GRP_PATH}:{} 엔트리 누락", target.asset)))?;
        let original_cnx = archive.read_file(entry);
        let header = parse_header(original_cnx).map_err(|source| {
            error(format!(
                "{PP2_GRP_PATH}:{} CNX header 실패: {source}",
                target.asset
            ))
        })?;
        if header.subtype != *b"bi_" {
            return Err(error(format!(
                "{PP2_GRP_PATH}:{} CNX subtype 불일치: {:?}",
                target.asset, header.subtype
            )));
        }
        let plain = decompress(original_cnx).map_err(|source| {
            error(format!(
                "{PP2_GRP_PATH}:{} 해제 실패: {source}",
                target.asset
            ))
        })?;
        let patch: SubgameSeqSceneGlyphProbePatch = patch_box_subgame_seq_scene_glyph_probe_asset(
            target.asset,
            &plain,
            &target.repertoire_indices,
            '가',
        )
        .map_err(|source| error(source.to_string()))?;
        let replacement = compress(&patch.plain, header.subtype).map_err(|source| {
            error(format!(
                "{PP2_GRP_PATH}:{} 재압축 실패: {source}",
                target.asset
            ))
        })?;
        if decompress(&replacement).map_err(|source| {
            error(format!(
                "{PP2_GRP_PATH}:{} 재압축 readback 실패: {source}",
                target.asset
            ))
        })? != patch.plain
        {
            return Err(error(format!(
                "{PP2_GRP_PATH}:{} 재압축 readback 불일치",
                target.asset
            )));
        }
        reports.push(Puyo2SceneGlyphPopulationAssetReport {
            asset: target.asset,
            consumers: target.consumers,
            repertoire_indices: patch.repertoire_indices,
            cell_indices: patch.cell_indices,
            source_palette_indices: patch.source_palette_indices,
            cnx_before: original_cnx.len(),
            cnx_after: replacement.len(),
            changed_pixels: patch.changed_pixels,
            changed_plain_bytes: patch.changed_bytes,
            source_plain_sha256: patch.source_sha256,
            output_plain_sha256: patch.output_sha256,
        });
        if expected
            .insert(target.asset.to_string(), (header.subtype, patch.plain))
            .is_some()
            || replacements
                .insert(target.asset.to_string(), replacement)
                .is_some()
        {
            return Err(error(format!(
                "{PP2_GRP_PATH}:{} 전체 모집단 replacement 중복",
                target.asset
            )));
        }
    }
    Ok(PreparedPuyo2SceneGlyphPopulation {
        replacements,
        targets: expected,
        reports,
        candidate_links,
        incompatible_selected_assets: vec![INCOMPATIBLE_SELECTED_ASSET],
    })
}

/// 지원 원본에서 PUYO2 scene atlas 전체 모집단 진단 디스크를 만든다.
pub fn build_puyo2_scene_glyph_population_poc(
    source: &VerifiedSourceImage,
) -> Result<Puyo2SceneGlyphPopulationPocBuild, Puyo2SceneGlyphPopulationPocError> {
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let original_vdd = iso
        .read_file(PP2_GRP_PATH)
        .map_err(|source| error(source.to_string()))?;
    let archive = VddArchive::parse(&original_vdd)
        .map_err(|source| error(format!("{PP2_GRP_PATH} VDD 파싱 실패: {source}")))?;
    let prepared = prepare_puyo2_scene_glyph_population(&archive)?;
    let reports = prepared.reports;
    let candidate_links = prepared.candidate_links;
    let incompatible_selected_assets = prepared.incompatible_selected_assets;
    let replacements = prepared.replacements;
    let expected = prepared
        .targets
        .iter()
        .map(|(name, target)| (name.as_str(), target.clone()))
        .collect::<BTreeMap<_, _>>();

    let replacement_vdd = archive
        .repack_in_container(&replacements)
        .map_err(|source| error(format!("{PP2_GRP_PATH} 전체 모집단 재패킹 실패: {source}")))?;
    if replacement_vdd.len() != original_vdd.len() {
        return Err(error("PP2_GRP 전체 모집단 재패킹으로 컨테이너 길이 변경"));
    }
    verify_final_archive(&replacement_vdd, &archive, &expected)?;

    let entry = iso
        .find(PP2_GRP_PATH)
        .map_err(|source| error(source.to_string()))?;
    let file = logical_file(entry).map_err(error)?;
    let mut data_plan = WritePlan::new();
    add_vdd_repack(
        &mut data_plan,
        file,
        &original_vdd,
        &replacement_vdd,
        "puyo2-scene-glyph-population:grp",
        "pipeline::puyo2_scene_glyph_population_poc",
    )
    .map_err(error)?;
    let data_writes = data_plan.writes.len();
    let touched_sectors = data_plan.touched_sectors().len();
    let final_expected_writes = data_writes + touched_sectors;
    let output = apply_form1_write_plan(source.image(), &data_plan)
        .map_err(|source| error(source.to_string()))?;
    let final_image = DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
    let final_vdd = final_iso
        .read_file(PP2_GRP_PATH)
        .map_err(|source| error(source.to_string()))?;
    if final_vdd != replacement_vdd {
        return Err(error("최종 디스크 PP2_GRP byte readback 불일치"));
    }
    verify_final_archive(&final_vdd, &archive, &expected)?;

    let image = final_image.into_bytes();
    let image_sha256 = digest(&image);
    let changed_cells = reports.iter().map(|row| row.cell_indices.len()).sum();
    let changed_pixels = reports.iter().map(|row| row.changed_pixels).sum();
    let changed_plain_bytes = reports.iter().map(|row| row.changed_plain_bytes).sum();
    let actual = (
        changed_cells,
        changed_pixels,
        changed_plain_bytes,
        data_writes,
        touched_sectors,
        final_expected_writes,
        image_sha256.as_str(),
    );
    let expected_baseline = (
        BOX_PUYO2_SCENE_GLYPH_POPULATION_CELLS,
        BOX_PUYO2_SCENE_GLYPH_POPULATION_CHANGED_PIXELS,
        BOX_PUYO2_SCENE_GLYPH_POPULATION_CHANGED_BYTES,
        BOX_PUYO2_SCENE_GLYPH_POPULATION_DATA_WRITES,
        BOX_PUYO2_SCENE_GLYPH_POPULATION_TOUCHED_SECTORS,
        BOX_PUYO2_SCENE_GLYPH_POPULATION_FINAL_EXPECTED_WRITES,
        BOX_PUYO2_SCENE_GLYPH_POPULATION_IMAGE_SHA256,
    );
    if actual != expected_baseline {
        return Err(error(format!(
            "PUYO2 scene glyph population 결정값 불일치: expected={expected_baseline:?}, actual={actual:?}"
        )));
    }

    Ok(Puyo2SceneGlyphPopulationPocBuild {
        image,
        report: Puyo2SceneGlyphPopulationPocReport {
            schema_version: PUYO2_SCENE_GLYPH_POPULATION_POC_SCHEMA_VERSION,
            synthetic_glyph: '가',
            assets: reports,
            candidate_links,
            incompatible_selected_assets,
            changed_cells,
            changed_pixels,
            changed_plain_bytes,
            data_writes,
            touched_sectors,
            final_expected_writes,
            image_sha256,
            product_eligible: false,
        },
    })
}

#[cfg(test)]
#[path = "puyo2_scene_glyph_population_poc_tests.rs"]
mod tests;
