//! PUYO1/2 DRAC 합성 한 글자 전체 디스크 PoC.
//!
//! 승인 번역 대신 모든 원문 glyph run을 `가` 한 글자로 바꾼 합성 입력을
//! 사용한다. 두 `DRAC_SEQ.SEQ`와 두 scene-local BTX를 각각 원본 CNX subtype으로
//! 재압축하고 네 VDD를 고정 컨테이너 안에서 재패킹한 뒤, 하나의 Mode2/Form1
//! Expected Write plan과 최종 디스크 readback으로 전체 정적 경로를 검증한다.
//! 이 결과는 번역·UX·runtime 표시 승인이 아니다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::{compress, decompress, is_cnx, parse_header};
use crate::disc::{apply_form1_write_plan, DiscImage, Iso9660, VerifiedSourceImage};
use crate::graphics::drac_glyph_patch::{
    patch_box_drac_scene_glyph_asset, verify_box_drac_scene_glyph_assets,
    BOX_DRAC_SCENE_GLYPH_ASSET_SPECS,
};
use crate::patch::WritePlan;
use crate::script::{
    plan_box_subgame_seq_drac_translation, repack_box_subgame_seq_drac_messages,
    verify_box_subgame_seq_drac_relinks, verify_box_subgame_seq_drac_text,
    verify_box_subgame_seq_layouts, verify_box_subgame_seq_text_candidates,
    SubgameSeqDracTranslation, BOX_SUBGAME_SEQ_DRAC_RELINK_SPECS,
    BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS,
};

use super::vdd_plan::{add_vdd_repack, logical_file};

pub const DRAC_SINGLE_GLYPH_POC_SCHEMA_VERSION: &str = "drac-single-glyph-poc-v1";
pub const BOX_DRAC_SINGLE_GLYPH_POC_DATA_WRITES: usize = 44;
pub const BOX_DRAC_SINGLE_GLYPH_POC_TOUCHED_SECTORS: usize = 44;
pub const BOX_DRAC_SINGLE_GLYPH_POC_FINAL_EXPECTED_WRITES: usize = 88;
pub const BOX_DRAC_SINGLE_GLYPH_POC_IMAGE_SHA256: &str =
    "27b730d7ad03ee6c32b86666a02eb8121ae9df770f28311e5a7529445e6e06f2";

const EXPECTED_SEQUENCE_RESULTS: [(&str, usize, usize, &str); 2] = [
    (
        "PUYO1",
        3_152,
        2_916,
        "dad8791a68554742a079fdcf7fae6e58e07eb90b06fb934b10d9a05d822ccbbb",
    ),
    (
        "PUYO2",
        3_105,
        2_872,
        "22a8d0eb23acb6bc1a3dc4b1efbad19b9a3f46fe4bf59a01a5fdeb0a91f35ace",
    ),
];
const EXPECTED_GLYPH_RESULTS: [(&str, usize, usize, usize, usize, &str); 2] = [
    (
        "PUYO1",
        30_584,
        30_538,
        95,
        69,
        "c037ba53c34e5a227a6d45b37d2ae68fc067374a943f9fee65dc9ce6265fe278",
    ),
    (
        "PUYO2",
        30_021,
        30_135,
        63,
        52,
        "9d1efeae0a4beeeb9c856936a9a819ab213665a12d75c7f9fae5a0a1ec62e8b9",
    ),
];

const SYNTHETIC_ONE_SPAN: [&str; 1] = ["가"];
const SYNTHETIC_THREE_SPANS: [&str; 3] = ["가", "가", "가"];
const SYNTHETIC_TRANSLATIONS: [SubgameSeqDracTranslation<'static>; 5] = [
    SubgameSeqDracTranslation {
        index: 1,
        spans: &SYNTHETIC_ONE_SPAN,
    },
    SubgameSeqDracTranslation {
        index: 2,
        spans: &SYNTHETIC_THREE_SPANS,
    },
    SubgameSeqDracTranslation {
        index: 3,
        spans: &SYNTHETIC_THREE_SPANS,
    },
    SubgameSeqDracTranslation {
        index: 4,
        spans: &SYNTHETIC_THREE_SPANS,
    },
    SubgameSeqDracTranslation {
        index: 5,
        spans: &SYNTHETIC_THREE_SPANS,
    },
];

/// 한 `DRAC_SEQ.SEQ` 사본의 재삽입 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DracSingleGlyphPocSequenceReport {
    pub scope: &'static str,
    pub archive: &'static str,
    pub sequence: &'static str,
    pub cnx_before: usize,
    pub cnx_after: usize,
    pub used_bytes: usize,
    pub remaining_bytes: usize,
    pub output_plain_sha256: String,
}

/// 한 scene-local BTX의 글리프 합성 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DracSingleGlyphPocGlyphReport {
    pub scope: &'static str,
    pub archive: &'static str,
    pub asset: &'static str,
    pub cnx_before: usize,
    pub cnx_after: usize,
    pub changed_cells: usize,
    pub changed_pixels: usize,
    pub changed_plain_bytes: usize,
    pub output_plain_sha256: String,
}

/// 합성 한 글자 PoC의 정적 검증 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DracSingleGlyphPocReport {
    pub schema_version: &'static str,
    pub synthetic_glyph: char,
    pub translation_messages: usize,
    pub glyph_mappings: usize,
    pub sequence_assets: Vec<DracSingleGlyphPocSequenceReport>,
    pub glyph_assets: Vec<DracSingleGlyphPocGlyphReport>,
    pub data_writes: usize,
    pub final_expected_writes: usize,
    pub touched_sectors: usize,
    pub image_sha256: String,
}

/// 완전히 readback한 합성 한 글자 raw BIN.
#[derive(Debug)]
pub struct DracSingleGlyphPocBuild {
    pub image: Vec<u8>,
    pub report: DracSingleGlyphPocReport,
}

/// DRAC 합성 한 글자 전체 디스크 PoC 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DracSingleGlyphPocError(String);

impl fmt::Display for DracSingleGlyphPocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for DracSingleGlyphPocError {}

fn error(message: impl Into<String>) -> DracSingleGlyphPocError {
    DracSingleGlyphPocError(message.into())
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
struct PreparedVdd {
    scope: &'static str,
    kind: &'static str,
    path: &'static str,
    target: &'static str,
    original: Vec<u8>,
    original_archive: VddArchive,
    replacement: Vec<u8>,
    expected_plain: Vec<u8>,
    expected_subtype: [u8; 3],
}

fn load_archive(
    iso: &Iso9660<'_>,
    path: &'static str,
) -> Result<LoadedArchive, DracSingleGlyphPocError> {
    let raw = iso
        .read_file(path)
        .map_err(|source| error(source.to_string()))?;
    let archive = VddArchive::parse(&raw)
        .map_err(|source| error(format!("{path} VDD 파싱 실패: {source}")))?;
    Ok(LoadedArchive { path, raw, archive })
}

fn read_sequence_sources(
    archives: &[LoadedArchive],
) -> Result<Vec<(String, String, Vec<u8>)>, DracSingleGlyphPocError> {
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
    target: &'static str,
    expected_subtype: [u8; 3],
    plain: &[u8],
) -> Result<(usize, Vec<u8>), DracSingleGlyphPocError> {
    let entry = archive
        .archive
        .find(target)
        .ok_or_else(|| error(format!("{}:{target} 엔트리 누락", archive.path)))?;
    let original_cnx = archive.archive.read_file(entry);
    let header = parse_header(original_cnx)
        .map_err(|source| error(format!("{}:{target}: {source}", archive.path)))?;
    if header.subtype != expected_subtype {
        return Err(error(format!(
            "{}:{target} CNX subtype 불일치: expected={:?}, actual={:?}",
            archive.path, expected_subtype, header.subtype
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
    scope: &'static str,
    kind: &'static str,
    target: &'static str,
    expected_plain: Vec<u8>,
    expected_subtype: [u8; 3],
    replacement_cnx: Vec<u8>,
) -> Result<PreparedVdd, DracSingleGlyphPocError> {
    let replacement = archive
        .archive
        .repack_in_container(&BTreeMap::from([(target.to_string(), replacement_cnx)]))
        .map_err(|source| {
            error(format!(
                "{}:{target} VDD 재패킹 실패: {source}",
                archive.path
            ))
        })?;
    if replacement.len() != archive.raw.len() {
        return Err(error(format!("{} VDD 컨테이너 길이 변경", archive.path)));
    }
    Ok(PreparedVdd {
        scope,
        kind,
        path: archive.path,
        target,
        original: archive.raw.clone(),
        original_archive: archive.archive.clone(),
        replacement,
        expected_plain,
        expected_subtype,
    })
}

fn verify_final_vdd(
    iso: &Iso9660<'_>,
    prepared: &PreparedVdd,
) -> Result<(), DracSingleGlyphPocError> {
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
        if original_entry.name().eq_ignore_ascii_case(prepared.target) {
            continue;
        }
        let final_entry = final_archive.find(original_entry.name()).ok_or_else(|| {
            error(format!(
                "{}:{} 비대상 엔트리 readback 누락",
                prepared.path,
                original_entry.name()
            ))
        })?;
        if final_archive.read_file(final_entry)
            != prepared.original_archive.read_file(original_entry)
        {
            return Err(error(format!(
                "{}:{} 비대상 엔트리 내용 변경",
                prepared.path,
                original_entry.name()
            )));
        }
    }
    let target = final_archive.find(prepared.target).ok_or_else(|| {
        error(format!(
            "{}:{} 최종 대상 엔트리 누락",
            prepared.path, prepared.target
        ))
    })?;
    let final_cnx = final_archive.read_file(target);
    let header = parse_header(final_cnx).map_err(|source| {
        error(format!(
            "{}:{} 최종 CNX 파싱 실패: {source}",
            prepared.path, prepared.target
        ))
    })?;
    if header.subtype != prepared.expected_subtype {
        return Err(error(format!(
            "{}:{} 최종 CNX subtype 변경",
            prepared.path, prepared.target
        )));
    }
    let plain = decompress(final_cnx).map_err(|source| {
        error(format!(
            "{}:{} 최종 CNX 해제 실패: {source}",
            prepared.path, prepared.target
        ))
    })?;
    if plain != prepared.expected_plain {
        return Err(error(format!(
            "{}:{} 최종 해제 readback 불일치",
            prepared.path, prepared.target
        )));
    }
    Ok(())
}

/// 지원 원본에서 합성 `가` 한 글자 DRAC 전체 디스크 PoC를 만든다.
pub fn build_drac_single_glyph_poc(
    source: &VerifiedSourceImage,
) -> Result<DracSingleGlyphPocBuild, DracSingleGlyphPocError> {
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
    let drac_text = verify_box_subgame_seq_drac_text(source_view(), &text, &layouts)
        .map_err(|source| error(source.to_string()))?;
    let relinks = verify_box_subgame_seq_drac_relinks(source_view(), &drac_text, &layouts)
        .map_err(|source| error(source.to_string()))?;
    let translation = plan_box_subgame_seq_drac_translation(&SYNTHETIC_TRANSLATIONS, &drac_text)
        .map_err(|source| error(source.to_string()))?;
    let [mapping] = translation.glyph_mappings.as_slice() else {
        return Err(error("DRAC 합성 PoC glyph mapping 분모 불일치"));
    };
    if mapping.glyph != '가' || mapping.repertoire_index != 0 || mapping.pair != [0x82, 0xA0] {
        return Err(error("DRAC 합성 PoC 결정적 glyph mapping 불일치"));
    }
    if translation.replacement_records.len() != 5
        || translation.used_bytes != 88
        || translation.remaining_bytes != 182
    {
        return Err(error("DRAC 합성 PoC message 용량 검산 불일치"));
    }

    let mut prepared = Vec::new();
    let mut sequence_reports = Vec::new();
    for spec in BOX_SUBGAME_SEQ_DRAC_RELINK_SPECS {
        let loaded = sequence_archives
            .iter()
            .find(|archive| archive.path == spec.archive)
            .ok_or_else(|| error(format!("{} target SEQ archive 누락", spec.archive)))?;
        let original_plain = sequence_sources
            .iter()
            .find(|(archive, sequence, _)| {
                archive == spec.archive && sequence.eq_ignore_ascii_case(spec.sequence)
            })
            .map(|(_, _, plain)| plain.as_slice())
            .ok_or_else(|| {
                error(format!(
                    "{}:{} 원본 plain 누락",
                    spec.archive, spec.sequence
                ))
            })?;
        let repack = repack_box_subgame_seq_drac_messages(
            spec.scope,
            original_plain,
            &translation.replacement_records,
            &drac_text,
            &relinks,
        )
        .map_err(|source| error(source.to_string()))?;
        let output_plain_sha256 = digest(&repack.sequence);
        let (cnx_before, replacement_cnx) =
            recompress(loaded, spec.sequence, *b"bin", &repack.sequence)?;
        let cnx_after = replacement_cnx.len();
        sequence_reports.push(DracSingleGlyphPocSequenceReport {
            scope: spec.scope,
            archive: spec.archive,
            sequence: spec.sequence,
            cnx_before,
            cnx_after,
            used_bytes: repack.used_bytes,
            remaining_bytes: repack.remaining_bytes,
            output_plain_sha256,
        });
        prepared.push(prepare_vdd(
            loaded,
            spec.scope,
            "sequence",
            spec.sequence,
            repack.sequence,
            *b"bin",
            replacement_cnx,
        )?);
    }

    let glyph_archives = BOX_DRAC_SCENE_GLYPH_ASSET_SPECS
        .iter()
        .map(|spec| load_archive(&iso, spec.archive))
        .collect::<Result<Vec<_>, _>>()?;
    let glyph_sources = BOX_DRAC_SCENE_GLYPH_ASSET_SPECS
        .iter()
        .map(|spec| {
            let loaded = glyph_archives
                .iter()
                .find(|archive| archive.path == spec.archive)
                .ok_or_else(|| error(format!("{} glyph archive 누락", spec.archive)))?;
            let entry = loaded.archive.find(spec.asset).ok_or_else(|| {
                error(format!("{}:{} glyph asset 누락", spec.archive, spec.asset))
            })?;
            let plain = decompress(loaded.archive.read_file(entry)).map_err(|source| {
                error(format!(
                    "{}:{} 해제 실패: {source}",
                    spec.archive, spec.asset
                ))
            })?;
            Ok((spec.archive, spec.asset, plain))
        })
        .collect::<Result<Vec<_>, DracSingleGlyphPocError>>()?;
    verify_box_drac_scene_glyph_assets(
        glyph_sources
            .iter()
            .map(|(archive, asset, plain)| (*archive, *asset, plain.as_slice())),
    )
    .map_err(|source| error(source.to_string()))?;

    let mut glyph_reports = Vec::new();
    for spec in BOX_DRAC_SCENE_GLYPH_ASSET_SPECS {
        let loaded = glyph_archives
            .iter()
            .find(|archive| archive.path == spec.archive)
            .ok_or_else(|| error(format!("{} target glyph archive 누락", spec.archive)))?;
        let original_plain = glyph_sources
            .iter()
            .find(|(archive, asset, _)| *archive == spec.archive && *asset == spec.asset)
            .map(|(_, _, plain)| plain.as_slice())
            .ok_or_else(|| error(format!("{}:{} glyph plain 누락", spec.archive, spec.asset)))?;
        let patch = patch_box_drac_scene_glyph_asset(
            spec.scope,
            original_plain,
            &translation.glyph_mappings,
        )
        .map_err(|source| error(source.to_string()))?;
        let (cnx_before, replacement_cnx) = recompress(loaded, spec.asset, *b"bi_", &patch.plain)?;
        let cnx_after = replacement_cnx.len();
        glyph_reports.push(DracSingleGlyphPocGlyphReport {
            scope: spec.scope,
            archive: spec.archive,
            asset: spec.asset,
            cnx_before,
            cnx_after,
            changed_cells: patch.changed_cells,
            changed_pixels: patch.changed_pixels,
            changed_plain_bytes: patch.changed_bytes,
            output_plain_sha256: patch.output_sha256,
        });
        prepared.push(prepare_vdd(
            loaded,
            spec.scope,
            "glyph",
            spec.asset,
            patch.plain,
            *b"bi_",
            replacement_cnx,
        )?);
    }
    let actual_sequence_results = sequence_reports
        .iter()
        .map(|row| {
            (
                row.scope,
                row.cnx_before,
                row.cnx_after,
                row.output_plain_sha256.as_str(),
            )
        })
        .collect::<Vec<_>>();
    if actual_sequence_results != EXPECTED_SEQUENCE_RESULTS {
        return Err(error(format!(
            "DRAC 합성 PoC SEQ 결정값 불일치: expected={EXPECTED_SEQUENCE_RESULTS:?}, actual={actual_sequence_results:?}"
        )));
    }
    let actual_glyph_results = glyph_reports
        .iter()
        .map(|row| {
            (
                row.scope,
                row.cnx_before,
                row.cnx_after,
                row.changed_pixels,
                row.changed_plain_bytes,
                row.output_plain_sha256.as_str(),
            )
        })
        .collect::<Vec<_>>();
    if actual_glyph_results != EXPECTED_GLYPH_RESULTS {
        return Err(error(format!(
            "DRAC 합성 PoC BTX 결정값 불일치: expected={EXPECTED_GLYPH_RESULTS:?}, actual={actual_glyph_results:?}"
        )));
    }

    let mut data_plan = WritePlan::new();
    for target in &prepared {
        let entry = iso
            .find(target.path)
            .map_err(|source| error(source.to_string()))?;
        let file = logical_file(entry).map_err(error)?;
        add_vdd_repack(
            &mut data_plan,
            file,
            &target.original,
            &target.replacement,
            &format!("drac-single-glyph:{}:{}", target.scope, target.kind),
            "pipeline::drac_single_glyph_poc",
        )
        .map_err(error)?;
    }
    let data_writes = data_plan.writes.len();
    let touched_sectors = data_plan.touched_sectors().len();
    let final_expected_writes = data_writes + touched_sectors;
    if data_writes != BOX_DRAC_SINGLE_GLYPH_POC_DATA_WRITES
        || touched_sectors != BOX_DRAC_SINGLE_GLYPH_POC_TOUCHED_SECTORS
        || final_expected_writes != BOX_DRAC_SINGLE_GLYPH_POC_FINAL_EXPECTED_WRITES
    {
        return Err(error(format!(
            "DRAC 합성 PoC Expected Write 결정값 불일치: expected=({}, {}, {}), actual=({data_writes}, {touched_sectors}, {final_expected_writes})",
            BOX_DRAC_SINGLE_GLYPH_POC_DATA_WRITES,
            BOX_DRAC_SINGLE_GLYPH_POC_TOUCHED_SECTORS,
            BOX_DRAC_SINGLE_GLYPH_POC_FINAL_EXPECTED_WRITES,
        )));
    }
    let output = apply_form1_write_plan(source.image(), &data_plan)
        .map_err(|source| error(source.to_string()))?;
    let final_image = DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
    for target in &prepared {
        verify_final_vdd(&final_iso, target)?;
    }
    let output = final_image.into_bytes();
    let image_sha256 = digest(&output);
    if image_sha256 != BOX_DRAC_SINGLE_GLYPH_POC_IMAGE_SHA256 {
        return Err(error(format!(
            "DRAC 합성 PoC 최종 이미지 SHA-256 불일치: expected={}, actual={image_sha256}",
            BOX_DRAC_SINGLE_GLYPH_POC_IMAGE_SHA256
        )));
    }

    Ok(DracSingleGlyphPocBuild {
        image: output,
        report: DracSingleGlyphPocReport {
            schema_version: DRAC_SINGLE_GLYPH_POC_SCHEMA_VERSION,
            synthetic_glyph: '가',
            translation_messages: translation.replacement_records.len(),
            glyph_mappings: translation.glyph_mappings.len(),
            sequence_assets: sequence_reports,
            glyph_assets: glyph_reports,
            data_writes,
            final_expected_writes,
            touched_sectors,
            image_sha256,
        },
    })
}
