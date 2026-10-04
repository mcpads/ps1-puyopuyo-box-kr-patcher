//! PUYO2 E6 합성 한 글자 전체 디스크 PoC.
//!
//! 승인 번역 대신 59개 E6 span을 각각 `가` 한 글자로 바꾼 합성 입력을
//! 사용한다. 원본 EXE/code table은 쓰지 않고 33개 SEQ와 `DEMOFONT.BTX`
//! 하나를 원본 CNX subtype으로 재압축한다. 두 VDD의 기존 entry offset과
//! slot을 유지한 뒤 하나의 Mode2/Form1 Expected Write plan과 최종 readback으로
//! 검증한다. 이 결과는 번역·UX·runtime 표시 승인이 아니다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::{compress, decompress, is_cnx, parse_header};
use crate::coords::{exe_map, ExeMap};
use crate::disasm::{
    audit_btx_name_consumers, verify_box_subgame_seq_e6_text, BOX_BTX_NAME_CONSUMERS,
    DEMO_FONT_CODE_TABLE_ENTRIES, DEMO_FONT_CODE_TABLE_RUNTIME,
};
use crate::disc::{
    add_single_extent_record_update, apply_form1_write_plan, partition_extent_with_remainder,
    verify_box_32m_reservation, DiscImage, Extent, Iso9660, PsxExe, VerifiedSourceImage,
    BOX_32M_PATH,
};
use crate::graphics::e6_demo_font_patch::verify_box_puyo2_e6_demo_font_single_glyph_poc;
use crate::patch::{UnchangedBoundary, WriteIntent, WritePlan};
use crate::script::{
    verify_box_subgame_seq_layouts, verify_box_subgame_seq_text_candidates,
    verify_box_subgame_seq_text_roots, BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS,
};
use crate::source::{FORM1_DATA_LEN, SECTOR_LEN};

use super::puyo2_scene_glyph_population_poc::{
    prepare_puyo2_scene_glyph_population, Puyo2SceneGlyphPopulationAssetReport,
    BOX_PUYO2_SCENE_GLYPH_POPULATION_ASSETS, BOX_PUYO2_SCENE_GLYPH_POPULATION_CANDIDATE_LINKS,
    BOX_PUYO2_SCENE_GLYPH_POPULATION_CELLS, BOX_PUYO2_SCENE_GLYPH_POPULATION_CHANGED_BYTES,
    BOX_PUYO2_SCENE_GLYPH_POPULATION_CHANGED_PIXELS,
    BOX_PUYO2_SCENE_GLYPH_POPULATION_INCOMPATIBLE_SELECTED,
};
use super::quest_exe_relocation_control::{count_extent_overlaps, extent_intervals};
use super::vdd_plan::{add_logical_span, add_vdd_repack, logical_file};
use super::{
    plan_box_puyo2_subgame_seq_e6_translation, repack_box_puyo2_subgame_seq_e6_sequence,
    verify_box_puyo2_e6_translation_manifest, verify_box_puyo2_subgame_seq_e6_translation_plan,
};

pub const PUYO2_E6_SINGLE_GLYPH_POC_SCHEMA_VERSION: &str = "puyo2-e6-single-glyph-poc-v1";
pub const PUYO2_FULL_GLYPH_POPULATION_POC_SCHEMA_VERSION: &str =
    "puyo2-full-glyph-population-poc-v1";
pub const PUYO2_E6_APPROVED_TRANSLATION_SCHEMA_VERSION: &str = "puyo2-e6-approved-translation-v1";
/// 현재 E6 DEMOFONT profile은 runtime/HITL 외형 승인을 받기 전이다.
pub const PUYO2_E6_DEMO_FONT_PROFILE_PRODUCT_APPROVED: bool = false;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_SEQUENCE_ASSETS: usize = 33;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_STREAMS: usize = 59;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_SEQUENCE_CNX_BEFORE: usize = 77_608;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_SEQUENCE_CNX_AFTER: usize = 74_123;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_FONT_CNX_BEFORE: usize = 9_832;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_FONT_CNX_AFTER: usize = 10_553;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_RELOCATED_GRP_LBA: u32 = 43_778;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_RELOCATED_GRP_BYTES: u32 = 2_725_888;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_RELOCATED_GRP_SECTORS: u32 = 1_331;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_RELOCATED_32M_LBA: u32 = 45_109;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_RELOCATED_32M_BYTES: u32 = 30_828_544;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_PLANNED_WRITES: usize = 1_402;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_TOUCHED_SECTORS: usize = 1_402;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_FINAL_EXPECTED_WRITES: usize = 2_804;
pub const BOX_PUYO2_E6_SINGLE_GLYPH_POC_IMAGE_SHA256: &str =
    "12176db4b43f7141b7bd232d4c89bc23aa444873463ce76d76fb57cf8fdd468e";
pub const BOX_PUYO2_FULL_GLYPH_POPULATION_POC_RELOCATED_GRP_LBA: u32 = 28_530;
pub const BOX_PUYO2_FULL_GLYPH_POPULATION_POC_RELOCATED_GRP_BYTES: u32 = 2_723_840;
pub const BOX_PUYO2_FULL_GLYPH_POPULATION_POC_RELOCATED_GRP_SECTORS: u32 = 1_330;
pub const BOX_PUYO2_FULL_GLYPH_POPULATION_POC_RELOCATED_32M_LBA: u32 = 43_778;
pub const BOX_PUYO2_FULL_GLYPH_POPULATION_POC_RELOCATED_32M_BYTES: u32 = 33_554_432;
pub const BOX_PUYO2_FULL_GLYPH_POPULATION_POC_DATA_WRITES: usize = 802;
pub const BOX_PUYO2_FULL_GLYPH_POPULATION_POC_METADATA_WRITES: usize = 6;
pub const BOX_PUYO2_FULL_GLYPH_POPULATION_POC_PLANNED_WRITES: usize = 808;
pub const BOX_PUYO2_FULL_GLYPH_POPULATION_POC_TOUCHED_SECTORS: usize = 808;
pub const BOX_PUYO2_FULL_GLYPH_POPULATION_POC_FINAL_EXPECTED_WRITES: usize = 1_616;
pub const BOX_PUYO2_FULL_GLYPH_POPULATION_POC_IMAGE_SHA256: &str =
    "5eb10de9bf74d2bfe79b7abab61283b38cc9ffd9d0952bc436a9e0f38f95115f";

const PUYO1_EXE_PATH: &str = "/PUYO1/PUYO1.EXE;1";
const PUYO2_EXE_PATH: &str = "/PUYO2/PUYO2.EXE;1";
const PUYO1_GRP_PATH: &str = "/PUYO1/PP1_GRP.VDD;1";
const PUYO2_GRP_PATH: &str = "/PUYO2/PP2_GRP.VDD;1";
const PUYO2_SEQ_PATH: &str = "/PUYO2/PP2_SEQ.VDD;1";
const DEMO_FONT_NAME: &str = "DEMOFONT.BTX";

/// 한 E6 SEQ의 합성 재삽입·재압축 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6SingleGlyphSequenceReport {
    pub sequence: String,
    pub streams: usize,
    pub cnx_before: usize,
    pub cnx_after: usize,
    pub capacity_bytes: usize,
    pub used_bytes: usize,
    pub remaining_bytes: usize,
    pub output_plain_sha256: String,
}

/// DEMOFONT 합성·재압축 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6SingleGlyphFontReport {
    pub asset: &'static str,
    pub cnx_before: usize,
    pub cnx_after: usize,
    pub changed_cells: usize,
    pub changed_pixels: usize,
    pub changed_plain_bytes: usize,
    pub output_plain_sha256: String,
}

/// 전체 디스크 PoC의 정적 검증 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6SingleGlyphPocReport {
    pub schema_version: &'static str,
    pub synthetic_glyph: char,
    pub sequence_assets: Vec<Puyo2E6SingleGlyphSequenceReport>,
    pub font_asset: Puyo2E6SingleGlyphFontReport,
    pub sequence_cnx_before: usize,
    pub sequence_cnx_after: usize,
    pub data_writes: usize,
    pub metadata_writes: usize,
    pub planned_writes: usize,
    pub final_expected_writes: usize,
    pub touched_sectors: usize,
    pub grp_relocated: bool,
    pub relocated_grp_lba: u32,
    pub relocated_grp_bytes: u32,
    pub relocated_grp_sectors: u32,
    pub relocated_32m_lba: u32,
    pub relocated_32m_bytes: u32,
    pub extent_overlaps: usize,
    pub image_sha256: String,
}

/// 완전히 readback한 PUYO2 E6 합성 한 글자 raw BIN.
#[derive(Debug)]
pub struct Puyo2E6SingleGlyphPocBuild {
    pub image: Vec<u8>,
    pub report: Puyo2E6SingleGlyphPocReport,
}

/// E6 overlay와 scene-local atlas를 원본에서 함께 합성한 진단 보고서.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2FullGlyphPopulationPocReport {
    pub schema_version: &'static str,
    pub synthetic_glyph: char,
    pub disc: Puyo2E6DiscReport,
    pub scene_assets: Vec<Puyo2SceneGlyphPopulationAssetReport>,
    pub scene_candidate_links: usize,
    pub scene_incompatible_selected_assets: Vec<&'static str>,
    pub scene_changed_cells: usize,
    pub scene_changed_pixels: usize,
    pub scene_changed_plain_bytes: usize,
    pub product_eligible: bool,
}

/// 완전히 readback한 PUYO2 E6+scene 전체 모집단 진단 raw BIN.
#[derive(Debug)]
pub struct Puyo2FullGlyphPopulationPocBuild {
    pub image: Vec<u8>,
    pub report: Puyo2FullGlyphPopulationPocReport,
}

/// 승인 번역과 합성 PoC가 공유하는 최종 디스크 검증 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6DiscReport {
    pub sequence_assets: Vec<Puyo2E6SingleGlyphSequenceReport>,
    pub font_asset: Puyo2E6SingleGlyphFontReport,
    pub sequence_cnx_before: usize,
    pub sequence_cnx_after: usize,
    pub data_writes: usize,
    pub metadata_writes: usize,
    pub planned_writes: usize,
    pub final_expected_writes: usize,
    pub touched_sectors: usize,
    pub grp_relocated: bool,
    pub relocated_grp_lba: u32,
    pub relocated_grp_bytes: u32,
    pub relocated_grp_sectors: u32,
    pub relocated_32m_lba: u32,
    pub relocated_32m_bytes: u32,
    pub extent_overlaps: usize,
    pub image_sha256: String,
}

/// 133개 span이 모두 승인된 manifest로만 만들 수 있는 제품 보고서.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6ApprovedTranslationReport {
    pub schema_version: &'static str,
    pub manifest_sha256: String,
    pub retained_source_characters: usize,
    pub hangul_glyphs: usize,
    pub remaining_slots: usize,
    pub disc: Puyo2E6DiscReport,
}

/// 승인 manifest와 최종 readback에 귀속된 raw BIN.
#[derive(Debug)]
pub struct Puyo2E6ApprovedTranslationBuild {
    pub image: Vec<u8>,
    pub report: Puyo2E6ApprovedTranslationReport,
}

#[derive(Debug)]
struct Puyo2E6CoreBuild {
    image: Vec<u8>,
    report: Puyo2E6DiscReport,
    manifest_sha256: Option<String>,
    retained_source_characters: usize,
    hangul_glyphs: usize,
    remaining_slots: usize,
    scene_population: Option<Puyo2ScenePopulationCoreReport>,
}

#[derive(Debug)]
struct Puyo2ScenePopulationCoreReport {
    assets: Vec<Puyo2SceneGlyphPopulationAssetReport>,
    candidate_links: usize,
    incompatible_selected_assets: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Puyo2E6BuildInput {
    SyntheticSingleGlyph,
    SyntheticFullPopulation,
    ApprovedManifest,
}

#[derive(Debug, Clone, Copy)]
struct Puyo2E6WriteIdentity {
    sequence: &'static str,
    relocated_grp: &'static str,
    grp_record: &'static str,
    reservation_record: &'static str,
    original_grp_boundary: &'static str,
    actor: &'static str,
}

impl Puyo2E6BuildInput {
    fn write_identity(self) -> Puyo2E6WriteIdentity {
        match self {
            Self::SyntheticSingleGlyph => Puyo2E6WriteIdentity {
                sequence: "puyo2-e6-single-glyph:seq",
                relocated_grp: "puyo2-e6-single-glyph:relocated-grp",
                grp_record: "puyo2-e6-single-glyph:grp-record",
                reservation_record: "puyo2-e6-single-glyph:32m-record",
                original_grp_boundary: "puyo2-e6-single-glyph-original-grp-extent",
                actor: "pipeline::puyo2_e6_single_glyph_poc",
            },
            Self::SyntheticFullPopulation => Puyo2E6WriteIdentity {
                sequence: "puyo2-full-glyph-population:seq",
                relocated_grp: "puyo2-full-glyph-population:relocated-grp",
                grp_record: "puyo2-full-glyph-population:grp-record",
                reservation_record: "puyo2-full-glyph-population:32m-record",
                original_grp_boundary: "puyo2-full-glyph-population-original-grp-extent",
                actor: "pipeline::puyo2_full_glyph_population_poc",
            },
            Self::ApprovedManifest => Puyo2E6WriteIdentity {
                sequence: "puyo2-e6-approved:seq",
                relocated_grp: "puyo2-e6-approved:relocated-grp",
                grp_record: "puyo2-e6-approved:grp-record",
                reservation_record: "puyo2-e6-approved:32m-record",
                original_grp_boundary: "puyo2-e6-approved-original-grp-extent",
                actor: "pipeline::puyo2_e6_approved_translation",
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6SingleGlyphPocError(String);

impl fmt::Display for Puyo2E6SingleGlyphPocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Puyo2E6SingleGlyphPocError {}

pub type Puyo2E6ApprovedTranslationError = Puyo2E6SingleGlyphPocError;

fn error(message: impl Into<String>) -> Puyo2E6SingleGlyphPocError {
    Puyo2E6SingleGlyphPocError(message.into())
}

fn require_puyo2_e6_demo_font_profile_product_approval() -> Result<(), Puyo2E6SingleGlyphPocError> {
    if PUYO2_E6_DEMO_FONT_PROFILE_PRODUCT_APPROVED {
        Ok(())
    } else {
        Err(error(
            "PUYO2 E6 DEMOFONT profile runtime/HITL 외형 승인이 완료되지 않음",
        ))
    }
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

#[derive(Debug)]
struct LoadedArchive {
    path: &'static str,
    raw: Vec<u8>,
    archive: VddArchive,
}

#[derive(Debug)]
struct LoadedExecutable {
    map: &'static ExeMap,
    text: Vec<u8>,
}

#[derive(Debug)]
struct PreparedTarget {
    plain: Vec<u8>,
    subtype: [u8; 3],
}

#[derive(Debug)]
struct PreparedVdd {
    path: &'static str,
    original: Vec<u8>,
    original_archive: VddArchive,
    replacement: Vec<u8>,
    targets: BTreeMap<String, PreparedTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VddPlacement {
    FixedEntries,
    FixedContainer,
    CompactGrowth,
}

fn load_archive(
    iso: &Iso9660<'_>,
    path: &'static str,
) -> Result<LoadedArchive, Puyo2E6SingleGlyphPocError> {
    let raw = iso
        .read_file(path)
        .map_err(|source| error(source.to_string()))?;
    let archive = VddArchive::parse(&raw)
        .map_err(|source| error(format!("{path} VDD 파싱 실패: {source}")))?;
    Ok(LoadedArchive { path, raw, archive })
}

fn load_executable(
    iso: &Iso9660<'_>,
    path: &'static str,
    name: &'static str,
) -> Result<LoadedExecutable, Puyo2E6SingleGlyphPocError> {
    let map = exe_map(name).ok_or_else(|| error(format!("{name} 좌표 사상 누락")))?;
    let raw = iso
        .read_file(path)
        .map_err(|source| error(source.to_string()))?;
    let executable = PsxExe::parse(&raw)
        .map_err(|source| error(format!("{path} PS-X EXE 파싱 실패: {source}")))?;
    executable
        .verify_map(map)
        .map_err(|source| error(source.to_string()))?;
    Ok(LoadedExecutable {
        map,
        text: executable.text().to_vec(),
    })
}

fn read_sequence_sources(
    archives: &[LoadedArchive],
) -> Result<Vec<(String, String, Vec<u8>)>, Puyo2E6SingleGlyphPocError> {
    let mut sources = Vec::new();
    for expectation in BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS {
        let archive = archives
            .iter()
            .find(|archive| archive.path == expectation.archive)
            .ok_or_else(|| error(format!("{} SEQ archive 누락", expectation.archive)))?;
        for entry in archive
            .archive
            .entries()
            .iter()
            .filter(|entry| entry.name().to_ascii_uppercase().ends_with(".SEQ"))
        {
            let cnx = archive.archive.read_file(entry);
            if !is_cnx(cnx) {
                return Err(error(format!(
                    "{}:{}가 CNX v2가 아님",
                    archive.path,
                    entry.name()
                )));
            }
            let plain = decompress(cnx).map_err(|source| {
                error(format!(
                    "{}:{} 해제 실패: {source}",
                    archive.path,
                    entry.name()
                ))
            })?;
            sources.push((archive.path.to_string(), entry.name().to_string(), plain));
        }
    }
    Ok(sources)
}

fn recompress(
    archive: &LoadedArchive,
    target: &str,
    expected_subtype: [u8; 3],
    plain: &[u8],
) -> Result<(usize, Vec<u8>), Puyo2E6SingleGlyphPocError> {
    let entry = archive
        .archive
        .find(target)
        .ok_or_else(|| error(format!("{}:{target} 엔트리 누락", archive.path)))?;
    let original_cnx = archive.archive.read_file(entry);
    let header = parse_header(original_cnx)
        .map_err(|source| error(format!("{}:{target}: {source}", archive.path)))?;
    if header.subtype != expected_subtype {
        return Err(error(format!(
            "{}:{target} CNX subtype 불일치: expected={expected_subtype:?}, actual={:?}",
            archive.path, header.subtype
        )));
    }
    let replacement = compress(plain, header.subtype)
        .map_err(|source| error(format!("{}:{target} 재압축 실패: {source}", archive.path)))?;
    if decompress(&replacement).map_err(|source| {
        error(format!(
            "{}:{target} 자기 readback 실패: {source}",
            archive.path
        ))
    })? != plain
    {
        return Err(error(format!(
            "{}:{target} 재압축 자기 readback 불일치",
            archive.path
        )));
    }
    Ok((original_cnx.len(), replacement))
}

fn prepare_vdd(
    archive: &LoadedArchive,
    replacement_cnx: BTreeMap<String, Vec<u8>>,
    targets: BTreeMap<String, PreparedTarget>,
    placement: VddPlacement,
) -> Result<PreparedVdd, Puyo2E6SingleGlyphPocError> {
    if replacement_cnx.keys().collect::<Vec<_>>() != targets.keys().collect::<Vec<_>>() {
        return Err(error(format!(
            "{} VDD replacement/target 분모 불일치",
            archive.path
        )));
    }
    let replacement = match placement {
        VddPlacement::FixedEntries => archive
            .archive
            .replace_in_place(&replacement_cnx)
            .map_err(|source| error(format!("{} VDD 제자리 교체 실패: {source}", archive.path)))?,
        VddPlacement::FixedContainer => archive
            .archive
            .repack_in_container(&replacement_cnx)
            .map_err(|source| {
                error(format!(
                    "{} VDD 컨테이너 재패킹 실패: {source}",
                    archive.path
                ))
            })?,
        VddPlacement::CompactGrowth => archive
            .archive
            .rebuild(&replacement_cnx)
            .map_err(|source| error(format!("{} VDD 성장 재조립 실패: {source}", archive.path)))?,
    };
    if (placement != VddPlacement::CompactGrowth && replacement.len() != archive.raw.len())
        || (placement == VddPlacement::CompactGrowth && replacement.len() <= archive.raw.len())
    {
        return Err(error(format!(
            "{} VDD placement 길이 전제 불일치: original={}, replacement={}, placement={placement:?}",
            archive.path,
            archive.raw.len(),
            replacement.len(),
        )));
    }
    Ok(PreparedVdd {
        path: archive.path,
        original: archive.raw.clone(),
        original_archive: archive.archive.clone(),
        replacement,
        targets,
    })
}

fn verify_final_vdd(
    iso: &Iso9660<'_>,
    prepared: &PreparedVdd,
) -> Result<(), Puyo2E6SingleGlyphPocError> {
    let final_vdd = iso
        .read_file(prepared.path)
        .map_err(|source| error(source.to_string()))?;
    if final_vdd != prepared.replacement {
        return Err(error(format!("{} 최종 VDD readback 불일치", prepared.path)));
    }
    let final_archive = VddArchive::parse(&final_vdd)
        .map_err(|source| error(format!("{} 최종 VDD 파싱 실패: {source}", prepared.path)))?;
    if final_archive.entry_count() != prepared.original_archive.entry_count() {
        return Err(error(format!(
            "{} 최종 VDD 엔트리 수 불일치",
            prepared.path
        )));
    }
    for original_entry in prepared.original_archive.entries() {
        let name = original_entry.name();
        let final_entry = final_archive
            .find(name)
            .ok_or_else(|| error(format!("{}:{name} 최종 엔트리 누락", prepared.path)))?;
        if let Some(target) = prepared.targets.get(name) {
            let final_cnx = final_archive.read_file(final_entry);
            let header = parse_header(final_cnx).map_err(|source| {
                error(format!(
                    "{}:{name} 최종 CNX 파싱 실패: {source}",
                    prepared.path
                ))
            })?;
            if header.subtype != target.subtype
                || decompress(final_cnx).map_err(|source| {
                    error(format!(
                        "{}:{name} 최종 CNX 해제 실패: {source}",
                        prepared.path
                    ))
                })? != target.plain
            {
                return Err(error(format!(
                    "{}:{name} 최종 대상 readback 불일치",
                    prepared.path
                )));
            }
        } else if final_archive.read_file(final_entry)
            != prepared.original_archive.read_file(original_entry)
        {
            return Err(error(format!(
                "{}:{name} 비대상 엔트리 내용 변경",
                prepared.path
            )));
        }
    }
    Ok(())
}

fn build_puyo2_e6_disc(
    source: &VerifiedSourceImage,
    input: Puyo2E6BuildInput,
) -> Result<Puyo2E6CoreBuild, Puyo2E6SingleGlyphPocError> {
    let identity = input.write_identity();
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let sequence_archives = BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS
        .iter()
        .map(|expectation| load_archive(&iso, expectation.archive))
        .collect::<Result<Vec<_>, _>>()?;
    let sequence_sources = read_sequence_sources(&sequence_archives)?;
    let source_view = || {
        sequence_sources.iter().map(|(archive, sequence, plain)| {
            (archive.as_str(), sequence.as_str(), plain.as_slice())
        })
    };
    let text = verify_box_subgame_seq_text_candidates(source_view())
        .map_err(|source| error(source.to_string()))?;
    let layouts = verify_box_subgame_seq_layouts(source_view(), &text)
        .map_err(|source| error(source.to_string()))?;
    let roots = verify_box_subgame_seq_text_roots(source_view(), &text, &layouts)
        .map_err(|source| error(source.to_string()))?;

    let executables = [
        load_executable(&iso, PUYO1_EXE_PATH, "PUYO1.EXE")?,
        load_executable(&iso, PUYO2_EXE_PATH, "PUYO2.EXE")?,
    ];
    let glyph_archives = [
        load_archive(&iso, PUYO1_GRP_PATH)?,
        load_archive(&iso, PUYO2_GRP_PATH)?,
    ];
    let btx_specs = BOX_BTX_NAME_CONSUMERS
        .iter()
        .copied()
        .filter(|spec| matches!(spec.exe, "PUYO1.EXE" | "PUYO2.EXE"))
        .collect::<Vec<_>>();
    let btx_names = audit_btx_name_consumers(
        &btx_specs,
        executables
            .iter()
            .map(|executable| (executable.map, executable.text.as_slice())),
        glyph_archives
            .iter()
            .map(|archive| (archive.path, &archive.archive)),
    )
    .map_err(|source| error(source.to_string()))?;
    let e6_text = verify_box_subgame_seq_e6_text(
        executables
            .iter()
            .map(|executable| (executable.map, executable.text.as_slice())),
        source_view(),
        &layouts,
        &roots,
        &btx_names,
    )
    .map_err(|source| error(source.to_string()))?;
    let puyo2_executable = executables
        .iter()
        .find(|executable| executable.map.name == "PUYO2.EXE")
        .ok_or_else(|| error("PUYO2 E6 PoC EXE 누락"))?;
    let code_table_offset = puyo2_executable
        .map
        .text_offset(DEMO_FONT_CODE_TABLE_RUNTIME)
        .ok_or_else(|| error("PUYO2 E6 PoC code table이 EXE 밖임"))?;
    let code_table_end = code_table_offset
        .checked_add(DEMO_FONT_CODE_TABLE_ENTRIES * 2)
        .ok_or_else(|| error("PUYO2 E6 PoC code table 끝 overflow"))?;
    let code_table = puyo2_executable
        .text
        .get(code_table_offset..code_table_end)
        .ok_or_else(|| error("PUYO2 E6 PoC code table 범위 이탈"))?;
    let (plan, manifest_sha256) = match input {
        Puyo2E6BuildInput::SyntheticSingleGlyph | Puyo2E6BuildInput::SyntheticFullPopulation => {
            let analysis = verify_box_puyo2_subgame_seq_e6_translation_plan(
                code_table,
                source_view(),
                &e6_text,
            )
            .map_err(|source| error(source.to_string()))?;
            let [mapping] = analysis.plan.glyph_mappings.as_slice() else {
                return Err(error("PUYO2 E6 합성 PoC glyph mapping 분모 불일치"));
            };
            if mapping.glyph != '가' {
                return Err(error("PUYO2 E6 합성 PoC glyph가 `가`가 아님"));
            }
            (analysis.plan, None)
        }
        Puyo2E6BuildInput::ApprovedManifest => {
            let manifest = verify_box_puyo2_e6_translation_manifest(&e6_text)
                .map_err(|source| error(source.to_string()))?;
            if !manifest.product_eligible {
                return Err(error(format!(
                    "PUYO2 E6 translation manifest 사람 승인 미완료: approved={}, total={}",
                    manifest.approved, manifest.spans
                )));
            }
            require_puyo2_e6_demo_font_profile_product_approval()?;
            let plan = plan_box_puyo2_subgame_seq_e6_translation(
                &manifest.translations,
                code_table,
                &e6_text,
            )
            .map_err(|source| error(source.to_string()))?;
            (plan, Some(manifest.manifest_sha256))
        }
    };

    let sequence_archive = sequence_archives
        .iter()
        .find(|archive| archive.path == PUYO2_SEQ_PATH)
        .ok_or_else(|| error("PUYO2 E6 PoC PP2_SEQ archive 누락"))?;
    let mut sequence_replacements = BTreeMap::new();
    let mut sequence_targets = BTreeMap::new();
    let mut sequence_reports = Vec::new();
    let sequence_names = plan
        .replacements
        .iter()
        .map(|row| row.sequence.as_str())
        .collect::<BTreeSet<_>>();
    for sequence in sequence_names {
        let original_plain = sequence_sources
            .iter()
            .find(|(archive, source_sequence, _)| {
                archive == PUYO2_SEQ_PATH && source_sequence.eq_ignore_ascii_case(sequence)
            })
            .map(|(_, _, plain)| plain.as_slice())
            .ok_or_else(|| error(format!("PUYO2 E6 PoC {sequence} plain 누락")))?;
        let repack = repack_box_puyo2_subgame_seq_e6_sequence(sequence, original_plain, &plan)
            .map_err(|source| error(source.to_string()))?;
        let (cnx_before, replacement_cnx) =
            recompress(sequence_archive, sequence, *b"bin", &repack.sequence_bytes)?;
        let cnx_after = replacement_cnx.len();
        if sequence_replacements
            .insert(sequence.to_string(), replacement_cnx)
            .is_some()
            || sequence_targets
                .insert(
                    sequence.to_string(),
                    PreparedTarget {
                        plain: repack.sequence_bytes.clone(),
                        subtype: *b"bin",
                    },
                )
                .is_some()
        {
            return Err(error(format!(
                "PUYO2 E6 PoC SEQ replacement 중복: {sequence}"
            )));
        }
        sequence_reports.push(Puyo2E6SingleGlyphSequenceReport {
            sequence: sequence.to_string(),
            streams: repack.pointer_relations,
            cnx_before,
            cnx_after,
            capacity_bytes: repack.capacity_bytes,
            used_bytes: repack.used_bytes,
            remaining_bytes: repack.remaining_bytes,
            output_plain_sha256: digest(&repack.sequence_bytes),
        });
    }
    sequence_reports.sort_by(|left, right| left.sequence.cmp(&right.sequence));
    if sequence_reports.len() != 33
        || sequence_reports
            .iter()
            .map(|row| row.streams)
            .sum::<usize>()
            != 59
    {
        return Err(error("PUYO2 E6 PoC SEQ 결과 분모 불일치"));
    }
    let prepared_sequence_vdd = prepare_vdd(
        sequence_archive,
        sequence_replacements,
        sequence_targets,
        VddPlacement::FixedEntries,
    )?;

    let glyph_archive = glyph_archives
        .iter()
        .find(|archive| archive.path == PUYO2_GRP_PATH)
        .ok_or_else(|| error("PUYO2 E6 PoC PP2_GRP archive 누락"))?;
    let demo_entry = glyph_archive
        .archive
        .find(DEMO_FONT_NAME)
        .ok_or_else(|| error("PUYO2 E6 PoC DEMOFONT.BTX 누락"))?;
    let demo_plain = decompress(glyph_archive.archive.read_file(demo_entry))
        .map_err(|source| error(format!("PUYO2 E6 PoC DEMOFONT 해제 실패: {source}")))?;
    let demo_patch =
        verify_box_puyo2_e6_demo_font_single_glyph_poc(&demo_plain, &plan.glyph_mappings)
            .map_err(|source| error(source.to_string()))?;
    let (font_cnx_before, font_replacement_cnx) =
        recompress(glyph_archive, DEMO_FONT_NAME, *b"bi_", &demo_patch.plain)?;
    let font_cnx_after = font_replacement_cnx.len();
    let font_report = Puyo2E6SingleGlyphFontReport {
        asset: DEMO_FONT_NAME,
        cnx_before: font_cnx_before,
        cnx_after: font_cnx_after,
        changed_cells: demo_patch.changed_cells,
        changed_pixels: demo_patch.changed_pixels,
        changed_plain_bytes: demo_patch.changed_bytes,
        output_plain_sha256: demo_patch.output_sha256,
    };
    let mut glyph_replacements =
        BTreeMap::from([(DEMO_FONT_NAME.to_string(), font_replacement_cnx)]);
    let mut glyph_targets = BTreeMap::from([(
        DEMO_FONT_NAME.to_string(),
        PreparedTarget {
            plain: demo_patch.plain,
            subtype: *b"bi_",
        },
    )]);
    let scene_population = if input == Puyo2E6BuildInput::SyntheticFullPopulation {
        let scene = prepare_puyo2_scene_glyph_population(&glyph_archive.archive)
            .map_err(|source| error(source.to_string()))?;
        for (name, replacement) in scene.replacements {
            if glyph_replacements
                .insert(name.clone(), replacement)
                .is_some()
            {
                return Err(error(format!(
                    "PUYO2 E6+scene GRP replacement 중복: {name}"
                )));
            }
        }
        for (name, (subtype, plain)) in scene.targets {
            if glyph_targets
                .insert(name.clone(), PreparedTarget { plain, subtype })
                .is_some()
            {
                return Err(error(format!("PUYO2 E6+scene GRP target 중복: {name}")));
            }
        }
        Some(Puyo2ScenePopulationCoreReport {
            assets: scene.reports,
            candidate_links: scene.candidate_links,
            incompatible_selected_assets: scene.incompatible_selected_assets,
        })
    } else {
        None
    };
    let prepared_glyph_vdd = prepare_vdd(
        glyph_archive,
        glyph_replacements,
        glyph_targets,
        if input == Puyo2E6BuildInput::SyntheticFullPopulation {
            VddPlacement::FixedContainer
        } else {
            VddPlacement::CompactGrowth
        },
    )?;

    let prepared = [prepared_sequence_vdd, prepared_glyph_vdd];
    let mut data_plan = WritePlan::new();
    let sequence_target = prepared
        .iter()
        .find(|target| target.path == PUYO2_SEQ_PATH)
        .ok_or_else(|| error("PUYO2 E6 PoC prepared SEQ VDD 누락"))?;
    let sequence_entry = iso
        .find(PUYO2_SEQ_PATH)
        .map_err(|source| error(source.to_string()))?;
    add_vdd_repack(
        &mut data_plan,
        logical_file(sequence_entry).map_err(error)?,
        &sequence_target.original,
        &sequence_target.replacement,
        identity.sequence,
        identity.actor,
    )
    .map_err(error)?;

    let grp_target = prepared
        .iter()
        .find(|target| target.path == PUYO2_GRP_PATH)
        .ok_or_else(|| error("PUYO2 E6 PoC prepared GRP VDD 누락"))?;
    let grp_entry = iso
        .find(PUYO2_GRP_PATH)
        .map_err(|source| error(source.to_string()))?;
    let reservation_entry = iso
        .find(BOX_32M_PATH)
        .map_err(|source| error(source.to_string()))?;
    let reservation = verify_box_32m_reservation(
        &iso,
        source.image(),
        executables
            .iter()
            .map(|executable| (executable.map.name, executable.text.as_slice())),
    )
    .map_err(|source| error(source.to_string()))?;
    let [original_grp_extent] = grp_entry.extents() else {
        return Err(error("PUYO2 E6 원본 PP2_GRP extent 분모 불일치"));
    };
    let relocated_grp_bytes = u32::try_from(grp_target.replacement.len())
        .map_err(|_| error("PUYO2 E6 relocated GRP 길이가 u32 범위를 넘음"))?;
    let relocated_grp_sectors = relocated_grp_bytes.div_ceil(FORM1_DATA_LEN as u32);
    if relocated_grp_bytes != relocated_grp_sectors * FORM1_DATA_LEN as u32 {
        return Err(error("PUYO2 E6 relocated GRP가 Form1 sector 정렬이 아님"));
    }
    let reservation_extent = Extent {
        lba: reservation.lba,
        len: u32::try_from(reservation.bytes)
            .map_err(|_| error("PUYO2 32M reservation 길이가 u32 범위를 넘음"))?,
    };
    let (relocated_grp, relocated_32m) = if input == Puyo2E6BuildInput::SyntheticFullPopulation {
        if relocated_grp_bytes != original_grp_extent.len {
            return Err(error(format!(
                    "PUYO2 E6+scene 제자리 GRP 길이 불일치: original={}, replacement={relocated_grp_bytes}",
                    original_grp_extent.len
                )));
        }
        add_vdd_repack(
            &mut data_plan,
            logical_file(grp_entry).map_err(error)?,
            &grp_target.original,
            &grp_target.replacement,
            identity.relocated_grp,
            identity.actor,
        )
        .map_err(error)?;
        (*original_grp_extent, reservation_extent)
    } else {
        let partition = partition_extent_with_remainder(reservation_extent, &[relocated_grp_bytes])
            .map_err(|source| error(source.to_string()))?;
        let [relocated_grp] = partition.allocations() else {
            return Err(error("PUYO2 E6 relocation extent 분모 불일치"));
        };
        let relocated_grp = *relocated_grp;
        let relocated_32m = partition.remainder();
        let expected_zero = vec![0; grp_target.replacement.len()];
        add_logical_span(
            &mut data_plan,
            crate::coords::LogicalFile::new(relocated_grp.lba, relocated_grp.len),
            0,
            &expected_zero,
            &grp_target.replacement,
            identity.relocated_grp,
            identity.actor,
            "32M reservation에 성장한 PP2_GRP.VDD 배치",
            WriteIntent::Data,
        )
        .map_err(error)?;
        for (entry, replacement, id, purpose) in [
            (
                grp_entry,
                relocated_grp,
                identity.grp_record,
                "성장한 PP2_GRP.VDD를 32M reservation으로 재배치",
            ),
            (
                reservation_entry,
                relocated_32m,
                identity.reservation_record,
                "재배치 PP2_GRP.VDD 뒤에서 시작하도록 32M.BIN 축소",
            ),
        ] {
            add_single_extent_record_update(
                &mut data_plan,
                source.image(),
                entry,
                replacement,
                id,
                identity.actor,
                purpose,
            )
            .map_err(|source| error(source.to_string()))?;
        }
        let original_grp_start = u64::from(original_grp_extent.lba) * SECTOR_LEN as u64;
        let original_grp_end = original_grp_start
            .checked_add(
                u64::from(original_grp_extent.len.div_ceil(FORM1_DATA_LEN as u32))
                    * SECTOR_LEN as u64,
            )
            .ok_or_else(|| error("PUYO2 E6 원본 PP2_GRP raw extent 끝 overflow"))?;
        data_plan.unchanged.push(UnchangedBoundary::new(
            identity.original_grp_boundary,
            original_grp_start..original_grp_end,
            "재배치 뒤 원본 PP2_GRP raw extent byte-identical 보존",
        ));
        (relocated_grp, relocated_32m)
    };
    let relocated_32m_lba = relocated_32m.lba;
    let relocated_32m_bytes = relocated_32m.len;
    let data_writes = data_plan
        .writes
        .iter()
        .filter(|write| write.intent == WriteIntent::Data)
        .count();
    let metadata_writes = data_plan
        .writes
        .iter()
        .filter(|write| write.intent == WriteIntent::Metadata)
        .count();
    let planned_writes = data_plan.writes.len();
    let touched_sectors = data_plan.touched_sectors().len();
    let final_expected_writes = planned_writes + touched_sectors;
    let output = apply_form1_write_plan(source.image(), &data_plan)
        .map_err(|source| error(source.to_string()))?;
    let final_image = DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
    final_iso
        .verify_single_extent_file(PUYO2_GRP_PATH, relocated_grp)
        .map_err(|source| error(source.to_string()))?;
    final_iso
        .verify_single_extent_file(BOX_32M_PATH, relocated_32m)
        .map_err(|source| error(source.to_string()))?;
    let extent_overlaps = count_extent_overlaps(
        &extent_intervals(&final_iso).map_err(|source| error(source.to_string()))?,
    )
    .map_err(|source| error(source.to_string()))?;
    for target in &prepared {
        verify_final_vdd(&final_iso, target)?;
    }
    let output = final_image.into_bytes();
    let image_sha256 = digest(&output);
    let sequence_cnx_before = sequence_reports.iter().map(|row| row.cnx_before).sum();
    let sequence_cnx_after = sequence_reports.iter().map(|row| row.cnx_after).sum();
    let actual = [
        sequence_reports.len() as u64,
        sequence_reports
            .iter()
            .map(|row| row.streams)
            .sum::<usize>() as u64,
        sequence_cnx_before as u64,
        sequence_cnx_after as u64,
        font_report.cnx_before as u64,
        font_report.cnx_after as u64,
        u64::from(relocated_grp.lba),
        u64::from(relocated_grp_bytes),
        u64::from(relocated_grp_sectors),
        u64::from(relocated_32m_lba),
        u64::from(relocated_32m_bytes),
        planned_writes as u64,
        touched_sectors as u64,
        final_expected_writes as u64,
        extent_overlaps as u64,
    ];
    if data_writes + metadata_writes != planned_writes
        || extent_overlaps != 0
        || prepared.iter().map(|vdd| vdd.path).collect::<BTreeSet<_>>()
            != BTreeSet::from([PUYO2_SEQ_PATH, PUYO2_GRP_PATH])
    {
        return Err(error(format!(
            "PUYO2 E6 최종 공통 경계 불일치: actual={actual:?}, \
             data/metadata={data_writes}/{metadata_writes}"
        )));
    }
    match input {
        Puyo2E6BuildInput::SyntheticSingleGlyph => {
            let expected = [
                BOX_PUYO2_E6_SINGLE_GLYPH_POC_SEQUENCE_ASSETS as u64,
                BOX_PUYO2_E6_SINGLE_GLYPH_POC_STREAMS as u64,
                BOX_PUYO2_E6_SINGLE_GLYPH_POC_SEQUENCE_CNX_BEFORE as u64,
                BOX_PUYO2_E6_SINGLE_GLYPH_POC_SEQUENCE_CNX_AFTER as u64,
                BOX_PUYO2_E6_SINGLE_GLYPH_POC_FONT_CNX_BEFORE as u64,
                BOX_PUYO2_E6_SINGLE_GLYPH_POC_FONT_CNX_AFTER as u64,
                u64::from(BOX_PUYO2_E6_SINGLE_GLYPH_POC_RELOCATED_GRP_LBA),
                u64::from(BOX_PUYO2_E6_SINGLE_GLYPH_POC_RELOCATED_GRP_BYTES),
                u64::from(BOX_PUYO2_E6_SINGLE_GLYPH_POC_RELOCATED_GRP_SECTORS),
                u64::from(BOX_PUYO2_E6_SINGLE_GLYPH_POC_RELOCATED_32M_LBA),
                u64::from(BOX_PUYO2_E6_SINGLE_GLYPH_POC_RELOCATED_32M_BYTES),
                BOX_PUYO2_E6_SINGLE_GLYPH_POC_PLANNED_WRITES as u64,
                BOX_PUYO2_E6_SINGLE_GLYPH_POC_TOUCHED_SECTORS as u64,
                BOX_PUYO2_E6_SINGLE_GLYPH_POC_FINAL_EXPECTED_WRITES as u64,
                0,
            ];
            if actual != expected {
                return Err(error(format!(
                    "PUYO2 E6 PoC 최종 기준선 불일치: expected={expected:?}, actual={actual:?}"
                )));
            }
            if image_sha256 != BOX_PUYO2_E6_SINGLE_GLYPH_POC_IMAGE_SHA256 {
                return Err(error(format!(
                    "PUYO2 E6 PoC 최종 이미지 SHA-256 불일치: expected={}, actual={image_sha256}",
                    BOX_PUYO2_E6_SINGLE_GLYPH_POC_IMAGE_SHA256
                )));
            }
        }
        Puyo2E6BuildInput::SyntheticFullPopulation => {
            let scene = scene_population
                .as_ref()
                .ok_or_else(|| error("PUYO2 E6+scene 모집단 보고 누락"))?;
            let scene_changed_cells = scene
                .assets
                .iter()
                .map(|row| row.cell_indices.len())
                .sum::<usize>();
            let scene_changed_pixels = scene
                .assets
                .iter()
                .map(|row| row.changed_pixels)
                .sum::<usize>();
            let scene_changed_plain_bytes = scene
                .assets
                .iter()
                .map(|row| row.changed_plain_bytes)
                .sum::<usize>();
            let combined_actual = [
                sequence_reports.len() as u64,
                sequence_reports
                    .iter()
                    .map(|row| row.streams)
                    .sum::<usize>() as u64,
                font_report.cnx_before as u64,
                font_report.cnx_after as u64,
                scene.assets.len() as u64,
                scene.candidate_links as u64,
                scene.incompatible_selected_assets.len() as u64,
                scene_changed_cells as u64,
                scene_changed_pixels as u64,
                scene_changed_plain_bytes as u64,
                u64::from(relocated_grp.lba),
                u64::from(relocated_grp_bytes),
                u64::from(relocated_grp_sectors),
                u64::from(relocated_32m_lba),
                u64::from(relocated_32m_bytes),
                data_writes as u64,
                metadata_writes as u64,
                planned_writes as u64,
                touched_sectors as u64,
                final_expected_writes as u64,
                extent_overlaps as u64,
            ];
            let expected = [
                BOX_PUYO2_E6_SINGLE_GLYPH_POC_SEQUENCE_ASSETS as u64,
                BOX_PUYO2_E6_SINGLE_GLYPH_POC_STREAMS as u64,
                BOX_PUYO2_E6_SINGLE_GLYPH_POC_FONT_CNX_BEFORE as u64,
                BOX_PUYO2_E6_SINGLE_GLYPH_POC_FONT_CNX_AFTER as u64,
                BOX_PUYO2_SCENE_GLYPH_POPULATION_ASSETS as u64,
                BOX_PUYO2_SCENE_GLYPH_POPULATION_CANDIDATE_LINKS as u64,
                BOX_PUYO2_SCENE_GLYPH_POPULATION_INCOMPATIBLE_SELECTED as u64,
                BOX_PUYO2_SCENE_GLYPH_POPULATION_CELLS as u64,
                BOX_PUYO2_SCENE_GLYPH_POPULATION_CHANGED_PIXELS as u64,
                BOX_PUYO2_SCENE_GLYPH_POPULATION_CHANGED_BYTES as u64,
                u64::from(BOX_PUYO2_FULL_GLYPH_POPULATION_POC_RELOCATED_GRP_LBA),
                u64::from(BOX_PUYO2_FULL_GLYPH_POPULATION_POC_RELOCATED_GRP_BYTES),
                u64::from(BOX_PUYO2_FULL_GLYPH_POPULATION_POC_RELOCATED_GRP_SECTORS),
                u64::from(BOX_PUYO2_FULL_GLYPH_POPULATION_POC_RELOCATED_32M_LBA),
                u64::from(BOX_PUYO2_FULL_GLYPH_POPULATION_POC_RELOCATED_32M_BYTES),
                BOX_PUYO2_FULL_GLYPH_POPULATION_POC_DATA_WRITES as u64,
                BOX_PUYO2_FULL_GLYPH_POPULATION_POC_METADATA_WRITES as u64,
                BOX_PUYO2_FULL_GLYPH_POPULATION_POC_PLANNED_WRITES as u64,
                BOX_PUYO2_FULL_GLYPH_POPULATION_POC_TOUCHED_SECTORS as u64,
                BOX_PUYO2_FULL_GLYPH_POPULATION_POC_FINAL_EXPECTED_WRITES as u64,
                0,
            ];
            if combined_actual != expected {
                return Err(error(format!(
                    "PUYO2 E6+scene 모집단 PoC 최종 기준선 불일치: expected={expected:?}, actual={combined_actual:?}"
                )));
            }
            if image_sha256 != BOX_PUYO2_FULL_GLYPH_POPULATION_POC_IMAGE_SHA256 {
                return Err(error(format!(
                    "PUYO2 E6+scene 모집단 PoC 최종 이미지 SHA-256 불일치: expected={}, actual={image_sha256}",
                    BOX_PUYO2_FULL_GLYPH_POPULATION_POC_IMAGE_SHA256
                )));
            }
        }
        Puyo2E6BuildInput::ApprovedManifest => {}
    }

    Ok(Puyo2E6CoreBuild {
        image: output,
        report: Puyo2E6DiscReport {
            sequence_assets: sequence_reports,
            font_asset: font_report,
            sequence_cnx_before,
            sequence_cnx_after,
            data_writes,
            metadata_writes,
            planned_writes,
            final_expected_writes,
            touched_sectors,
            grp_relocated: input != Puyo2E6BuildInput::SyntheticFullPopulation,
            relocated_grp_lba: relocated_grp.lba,
            relocated_grp_bytes,
            relocated_grp_sectors,
            relocated_32m_lba,
            relocated_32m_bytes,
            extent_overlaps,
            image_sha256,
        },
        manifest_sha256,
        retained_source_characters: plan.retained_source_characters,
        hangul_glyphs: plan.glyph_mappings.len(),
        remaining_slots: plan.remaining_slots,
        scene_population,
    })
}

/// 지원 원본에서 합성 `가` PUYO2 E6 전체 디스크 PoC를 만든다.
pub fn build_puyo2_e6_single_glyph_poc(
    source: &VerifiedSourceImage,
) -> Result<Puyo2E6SingleGlyphPocBuild, Puyo2E6SingleGlyphPocError> {
    let core = build_puyo2_e6_disc(source, Puyo2E6BuildInput::SyntheticSingleGlyph)?;
    let report = core.report;
    Ok(Puyo2E6SingleGlyphPocBuild {
        image: core.image,
        report: Puyo2E6SingleGlyphPocReport {
            schema_version: PUYO2_E6_SINGLE_GLYPH_POC_SCHEMA_VERSION,
            synthetic_glyph: '가',
            sequence_assets: report.sequence_assets,
            font_asset: report.font_asset,
            sequence_cnx_before: report.sequence_cnx_before,
            sequence_cnx_after: report.sequence_cnx_after,
            data_writes: report.data_writes,
            metadata_writes: report.metadata_writes,
            planned_writes: report.planned_writes,
            final_expected_writes: report.final_expected_writes,
            touched_sectors: report.touched_sectors,
            grp_relocated: report.grp_relocated,
            relocated_grp_lba: report.relocated_grp_lba,
            relocated_grp_bytes: report.relocated_grp_bytes,
            relocated_grp_sectors: report.relocated_grp_sectors,
            relocated_32m_lba: report.relocated_32m_lba,
            relocated_32m_bytes: report.relocated_32m_bytes,
            extent_overlaps: report.extent_overlaps,
            image_sha256: report.image_sha256,
        },
    })
}

/// 지원 원본에서 E6 overlay와 scene atlas를 함께 채운 PUYO2 진단 디스크를 만든다.
pub fn build_puyo2_full_glyph_population_poc(
    source: &VerifiedSourceImage,
) -> Result<Puyo2FullGlyphPopulationPocBuild, Puyo2E6SingleGlyphPocError> {
    let core = build_puyo2_e6_disc(source, Puyo2E6BuildInput::SyntheticFullPopulation)?;
    let scene = core
        .scene_population
        .ok_or_else(|| error("PUYO2 E6+scene 모집단 보고 누락"))?;
    let scene_changed_cells = scene.assets.iter().map(|row| row.cell_indices.len()).sum();
    let scene_changed_pixels = scene.assets.iter().map(|row| row.changed_pixels).sum();
    let scene_changed_plain_bytes = scene.assets.iter().map(|row| row.changed_plain_bytes).sum();
    Ok(Puyo2FullGlyphPopulationPocBuild {
        image: core.image,
        report: Puyo2FullGlyphPopulationPocReport {
            schema_version: PUYO2_FULL_GLYPH_POPULATION_POC_SCHEMA_VERSION,
            synthetic_glyph: '가',
            disc: core.report,
            scene_assets: scene.assets,
            scene_candidate_links: scene.candidate_links,
            scene_incompatible_selected_assets: scene.incompatible_selected_assets,
            scene_changed_cells,
            scene_changed_pixels,
            scene_changed_plain_bytes,
            product_eligible: false,
        },
    })
}

/// 모든 manifest span이 승인된 경우에만 PUYO2 E6 제품 이미지를 만든다.
pub fn build_puyo2_e6_approved_translation(
    source: &VerifiedSourceImage,
) -> Result<Puyo2E6ApprovedTranslationBuild, Puyo2E6ApprovedTranslationError> {
    let core = build_puyo2_e6_disc(source, Puyo2E6BuildInput::ApprovedManifest)?;
    let manifest_sha256 = core
        .manifest_sha256
        .ok_or_else(|| error("PUYO2 E6 승인 manifest digest 누락"))?;
    Ok(Puyo2E6ApprovedTranslationBuild {
        image: core.image,
        report: Puyo2E6ApprovedTranslationReport {
            schema_version: PUYO2_E6_APPROVED_TRANSLATION_SCHEMA_VERSION,
            manifest_sha256,
            retained_source_characters: core.retained_source_characters,
            hangul_glyphs: core.hangul_glyphs,
            remaining_slots: core.remaining_slots,
            disc: core.report,
        },
    })
}

#[cfg(test)]
#[path = "puyo2_e6_single_glyph_poc_tests.rs"]
mod tests;
