//! `MENU06` 미승인 draft의 CNX/VDD·전체 디스크 재삽입 gate.
//!
//! 원본과 version 관리 manifest에서 만든 고정 길이 stream·glyph atlas만
//! 입력으로 받아 PP1/PP2의 네 VDD를 메모리에서 재조립한다. 전체 디스크
//! PoC는 각 ISO extent에 Expected Write를 적용하고 Mode2/Form1 EDC/ECC를
//! 다시 만든 뒤 최종 BIN에서 대상·비대상 엔트리를 전수 readback한다.
//! 번역 승인·제품 patch·runtime 표시는 이 gate의 판정 범위가 아니다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::archive::{VddArchive, VddError};
use crate::compress::{compress, decompress, parse_header};
use crate::disc::{apply_form1_write_plan, DiscImage, Iso9660, VerifiedSourceImage};
use crate::graphics::menu06_glyph_patch::{
    patch_box_menu06_glyph_asset, verify_box_menu06_glyph_assets, MENU06_GLYPH_ASSET_SHA256,
};
use crate::patch::{WriteIntent, WritePlan};
use crate::script::{
    serialize_menu06_text_tokens, verify_box_menu06_text, verify_box_menu06_translation_manifest,
    MENU06_STREAM_END_OFFSET, MENU06_STREAM_OFFSET,
};

use super::vdd_plan::{add_vdd_entry_replacement, logical_file};

pub const MENU06_DRAFT_REINSERTION_SCHEMA_VERSION: &str = "menu06-draft-reinsertion-v1";
pub const MENU06_DRAFT_DISC_SCHEMA_VERSION: &str = "menu06-draft-disc-v1";

const MENU1_SEQUENCE: &str = "MENU1.SEQ";
const MENU06_ASSET: &str = "MENU06.BTX";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu06VddRewriteMethod {
    InPlace,
    RepackInContainer,
}

impl Menu06VddRewriteMethod {
    pub const fn label(self) -> &'static str {
        match self {
            Self::InPlace => "in-place",
            Self::RepackInContainer => "repack-in-container",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06DraftReinsertionRow {
    pub scope: &'static str,
    pub kind: &'static str,
    pub archive: &'static str,
    pub target: &'static str,
    pub cnx_before: usize,
    pub cnx_after: usize,
    pub method: Menu06VddRewriteMethod,
    pub vdd_bytes: usize,
    pub changed_vdd_bytes: usize,
    pub output_vdd_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06DraftReinsertionAnalysis {
    pub rows: Vec<Menu06DraftReinsertionRow>,
    pub in_place: usize,
    pub repacked: usize,
    pub product_eligible: bool,
}

/// `MENU06` draft 전체 디스크 Expected Write·readback 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06DraftDiscReport {
    pub schema_version: &'static str,
    pub reinsertion: Menu06DraftReinsertionAnalysis,
    pub planned_writes: usize,
    pub vdd_metadata_writes: usize,
    pub payload_writes: usize,
    pub touched_sectors: usize,
    pub final_expected_writes: usize,
    pub image_sha256: String,
    pub product_eligible: bool,
    pub runtime_verified: bool,
}

/// 최종 readback까지 통과한 `MENU06` 미승인 draft raw BIN.
#[derive(Debug)]
pub struct Menu06DraftDiscBuild {
    pub image: Vec<u8>,
    pub report: Menu06DraftDiscReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06DraftReinsertionError(String);

impl fmt::Display for Menu06DraftReinsertionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Menu06DraftReinsertionError {}

fn error(message: impl Into<String>) -> Menu06DraftReinsertionError {
    Menu06DraftReinsertionError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

#[derive(Debug, Clone, Copy)]
struct TargetSpec {
    scope: &'static str,
    kind: &'static str,
    archive: &'static str,
    target: &'static str,
    subtype: [u8; 3],
}

#[derive(Debug)]
struct PreparedVdd {
    spec: TargetSpec,
    original_vdd: Vec<u8>,
    original_archive: VddArchive,
    replacement_vdd: Vec<u8>,
    replacement_cnx_len: usize,
    expected_plain: Vec<u8>,
}

const TARGETS: [TargetSpec; 4] = [
    TargetSpec {
        scope: "PUYO1",
        kind: "sequence",
        archive: "/PUYO1/PP1_SEQ.VDD;1",
        target: MENU1_SEQUENCE,
        subtype: *b"bin",
    },
    TargetSpec {
        scope: "PUYO1",
        kind: "glyph",
        archive: "/PUYO1/PP1_GRP.VDD;1",
        target: MENU06_ASSET,
        subtype: *b"bi_",
    },
    TargetSpec {
        scope: "PUYO2",
        kind: "sequence",
        archive: "/PUYO2/PP2_SEQ.VDD;1",
        target: MENU1_SEQUENCE,
        subtype: *b"bin",
    },
    TargetSpec {
        scope: "PUYO2",
        kind: "glyph",
        archive: "/PUYO2/PP2_GRP.VDD;1",
        target: MENU06_ASSET,
        subtype: *b"bi_",
    },
];

const EXPECTED_RESULTS: [(&str, &str, usize, usize, usize, &str); 4] = [
    (
        "PUYO1",
        "sequence",
        7_853,
        7_509,
        7_224,
        "13a6508cca622c3091ec3c0f889c94b12865f5a6044e4c880b29546b56470234",
    ),
    (
        "PUYO1",
        "glyph",
        19_544,
        19_157,
        18_628,
        "c4226293c35c1332930b3023892a201ed7accfd3ff61f2f91c6e83dda8d4b771",
    ),
    (
        "PUYO2",
        "sequence",
        7_853,
        7_498,
        7_220,
        "aead93f1bba456cec9986035fcfee502286d7fb92db26e6c0df3e716c4a3ed02",
    ),
    (
        "PUYO2",
        "glyph",
        19_544,
        19_157,
        18_628,
        "32894ccc2c5b0e8117ee52a28587101c931160eda5d76ed785421b8b50f8cb87",
    ),
];

fn replace_stream(
    original: &[u8],
    source_stream: &[u8],
    replacement_stream: &[u8],
) -> Result<Vec<u8>, Menu06DraftReinsertionError> {
    let span = MENU06_STREAM_OFFSET..MENU06_STREAM_END_OFFSET;
    if source_stream.len() != span.len()
        || replacement_stream.len() != span.len()
        || original.get(span.clone()) != Some(source_stream)
    {
        return Err(error("MENU06 draft sequence stream 원본/길이 불일치"));
    }
    let mut output = original.to_vec();
    output[span].copy_from_slice(replacement_stream);
    Ok(output)
}

fn recompress(
    spec: TargetSpec,
    archive: &VddArchive,
    source_stream: &[u8],
    replacement_stream: &[u8],
    replacement_glyph: &[u8],
) -> Result<(usize, Vec<u8>, Vec<u8>), Menu06DraftReinsertionError> {
    let entry = archive
        .find(spec.target)
        .ok_or_else(|| error(format!("{}:{} 누락", spec.archive, spec.target)))?;
    let original_cnx = archive.read_file(entry);
    let header = parse_header(original_cnx)
        .map_err(|source| error(format!("{}:{}: {source}", spec.archive, spec.target)))?;
    if header.subtype != spec.subtype {
        return Err(error(format!(
            "{}:{} subtype 불일치: expected={:?}, actual={:?}",
            spec.archive, spec.target, spec.subtype, header.subtype
        )));
    }
    let original_plain = decompress(original_cnx)
        .map_err(|source| error(format!("{}:{}: {source}", spec.archive, spec.target)))?;
    let replacement_plain = if spec.kind == "sequence" {
        replace_stream(&original_plain, source_stream, replacement_stream)?
    } else {
        if digest(&original_plain) != MENU06_GLYPH_ASSET_SHA256 {
            return Err(error(format!(
                "{}:{} glyph 원본 drift",
                spec.archive, spec.target
            )));
        }
        replacement_glyph.to_vec()
    };
    let replacement_cnx = compress(&replacement_plain, header.subtype).map_err(|source| {
        error(format!(
            "{}:{} 재압축 실패: {source}",
            spec.archive, spec.target
        ))
    })?;
    let readback = decompress(&replacement_cnx).map_err(|source| {
        error(format!(
            "{}:{} 재압축 readback 실패: {source}",
            spec.archive, spec.target
        ))
    })?;
    if readback != replacement_plain {
        return Err(error(format!(
            "{}:{} 재압축 readback 불일치",
            spec.archive, spec.target
        )));
    }
    Ok((original_cnx.len(), replacement_cnx, replacement_plain))
}

fn replace_vdd(
    spec: TargetSpec,
    archive: &VddArchive,
    replacement_cnx: Vec<u8>,
    expected_plain: &[u8],
) -> Result<(Vec<u8>, Menu06VddRewriteMethod), Menu06DraftReinsertionError> {
    let replacements = BTreeMap::from([(spec.target.to_string(), replacement_cnx.clone())]);
    let (rebuilt, method) = match archive.replace_in_place(&replacements) {
        Ok(rebuilt) => (rebuilt, Menu06VddRewriteMethod::InPlace),
        Err(VddError::ReplacementDoesNotFit { .. }) => (
            archive
                .repack_in_container(&replacements)
                .map_err(|source| error(format!("{} VDD 재패킹 실패: {source}", spec.archive)))?,
            Menu06VddRewriteMethod::RepackInContainer,
        ),
        Err(source) => {
            return Err(error(format!(
                "{} VDD 제자리 교체 실패: {source}",
                spec.archive
            )))
        }
    };
    let original = archive.serialize_lossless();
    if rebuilt.len() != original.len() {
        return Err(error(format!("{} VDD 전체 길이 변경", spec.archive)));
    }
    let reparsed = VddArchive::parse(&rebuilt)
        .map_err(|source| error(format!("{} VDD readback 실패: {source}", spec.archive)))?;
    for entry in archive.entries() {
        let readback_entry = reparsed
            .find(entry.name())
            .ok_or_else(|| error(format!("{}:{} readback 누락", spec.archive, entry.name())))?;
        if entry.name().eq_ignore_ascii_case(spec.target) {
            if reparsed.read_file(readback_entry) != replacement_cnx {
                return Err(error(format!(
                    "{}:{} CNX readback 불일치",
                    spec.archive, spec.target
                )));
            }
            let plain = decompress(reparsed.read_file(readback_entry)).map_err(|source| {
                error(format!(
                    "{}:{} plain readback 실패: {source}",
                    spec.archive, spec.target
                ))
            })?;
            if plain != expected_plain {
                return Err(error(format!(
                    "{}:{} plain readback 불일치",
                    spec.archive, spec.target
                )));
            }
        } else {
            let original_entry = archive
                .find(entry.name())
                .ok_or_else(|| error("MENU06 original VDD entry 누락"))?;
            if reparsed.read_file(readback_entry) != archive.read_file(original_entry) {
                return Err(error(format!(
                    "{}:{} 비대상 entry 변경",
                    spec.archive,
                    entry.name()
                )));
            }
        }
    }
    Ok((rebuilt, method))
}

fn prepare_reinsertion<'a>(
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
    source_stream: &[u8],
    replacement_stream: &[u8],
    replacement_glyph: &[u8],
) -> Result<(Menu06DraftReinsertionAnalysis, Vec<PreparedVdd>), Menu06DraftReinsertionError> {
    let archives = archives.into_iter().collect::<BTreeMap<_, _>>();
    let mut rows = Vec::new();
    let mut prepared = Vec::new();
    for spec in TARGETS {
        let archive = archives
            .get(spec.archive)
            .copied()
            .ok_or_else(|| error(format!("{} VDD 입력 누락", spec.archive)))?;
        let original_vdd = archive.serialize_lossless();
        let (cnx_before, replacement_cnx, expected_plain) = recompress(
            spec,
            archive,
            source_stream,
            replacement_stream,
            replacement_glyph,
        )?;
        let cnx_after = replacement_cnx.len();
        let (replacement_vdd, method) =
            replace_vdd(spec, archive, replacement_cnx, &expected_plain)?;
        rows.push(Menu06DraftReinsertionRow {
            scope: spec.scope,
            kind: spec.kind,
            archive: spec.archive,
            target: spec.target,
            cnx_before,
            cnx_after,
            method,
            vdd_bytes: replacement_vdd.len(),
            changed_vdd_bytes: original_vdd
                .iter()
                .zip(&replacement_vdd)
                .filter(|(before, after)| before != after)
                .count(),
            output_vdd_sha256: digest(&replacement_vdd),
        });
        prepared.push(PreparedVdd {
            spec,
            original_vdd,
            original_archive: archive.clone(),
            replacement_vdd,
            replacement_cnx_len: cnx_after,
            expected_plain,
        });
    }
    let in_place = rows
        .iter()
        .filter(|row| row.method == Menu06VddRewriteMethod::InPlace)
        .count();
    let repacked = rows.len() - in_place;
    let actual = rows
        .iter()
        .map(|row| {
            (
                row.scope,
                row.kind,
                row.cnx_before,
                row.cnx_after,
                row.changed_vdd_bytes,
                row.output_vdd_sha256.as_str(),
            )
        })
        .collect::<Vec<_>>();
    if actual != EXPECTED_RESULTS || in_place != 4 || repacked != 0 {
        return Err(error(format!(
            "MENU06 draft reinsertion 기준선 불일치: rows={actual:?}, in-place/repacked={in_place}/{repacked}"
        )));
    }
    Ok((
        Menu06DraftReinsertionAnalysis {
            rows,
            in_place,
            repacked,
            product_eligible: false,
        },
        prepared,
    ))
}

fn verify_final_vdd(
    iso: &Iso9660<'_>,
    prepared: &PreparedVdd,
) -> Result<(), Menu06DraftReinsertionError> {
    let final_vdd = iso
        .read_file(prepared.spec.archive)
        .map_err(|source| error(source.to_string()))?;
    if final_vdd != prepared.replacement_vdd {
        return Err(error(format!(
            "{} 최종 VDD readback 불일치",
            prepared.spec.archive
        )));
    }
    let final_archive = VddArchive::parse(&final_vdd).map_err(|source| {
        error(format!(
            "{} 최종 VDD 파싱 실패: {source}",
            prepared.spec.archive
        ))
    })?;
    if final_archive.entry_count() != prepared.original_archive.entry_count() {
        return Err(error(format!(
            "{} 최종 VDD 엔트리 수 불일치",
            prepared.spec.archive
        )));
    }
    for original_entry in prepared.original_archive.entries() {
        let final_entry = final_archive.find(original_entry.name()).ok_or_else(|| {
            error(format!(
                "{}:{} 최종 엔트리 누락",
                prepared.spec.archive,
                original_entry.name()
            ))
        })?;
        if original_entry
            .name()
            .eq_ignore_ascii_case(prepared.spec.target)
        {
            continue;
        }
        if final_archive.read_file(final_entry)
            != prepared.original_archive.read_file(original_entry)
        {
            return Err(error(format!(
                "{}:{} 최종 비대상 엔트리 변경",
                prepared.spec.archive,
                original_entry.name()
            )));
        }
    }
    let target = final_archive.find(prepared.spec.target).ok_or_else(|| {
        error(format!(
            "{}:{} 최종 대상 누락",
            prepared.spec.archive, prepared.spec.target
        ))
    })?;
    let cnx = final_archive.read_file(target);
    if cnx.len() != prepared.replacement_cnx_len {
        return Err(error(format!(
            "{}:{} 최종 CNX 길이 불일치",
            prepared.spec.archive, prepared.spec.target
        )));
    }
    let header = parse_header(cnx).map_err(|source| {
        error(format!(
            "{}:{} 최종 CNX 파싱 실패: {source}",
            prepared.spec.archive, prepared.spec.target
        ))
    })?;
    if header.subtype != prepared.spec.subtype {
        return Err(error(format!(
            "{}:{} 최종 CNX subtype 변경",
            prepared.spec.archive, prepared.spec.target
        )));
    }
    let plain = decompress(cnx).map_err(|source| {
        error(format!(
            "{}:{} 최종 CNX 해제 실패: {source}",
            prepared.spec.archive, prepared.spec.target
        ))
    })?;
    if plain != prepared.expected_plain {
        return Err(error(format!(
            "{}:{} 최종 plain readback 불일치",
            prepared.spec.archive, prepared.spec.target
        )));
    }
    Ok(())
}

/// 네 VDD의 draft CNX 재압축·고정 컨테이너 재삽입·전수 readback을 검증한다.
pub fn verify_box_menu06_draft_reinsertion_capacity<'a>(
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
    source_stream: &[u8],
    replacement_stream: &[u8],
    replacement_glyph: &[u8],
) -> Result<Menu06DraftReinsertionAnalysis, Menu06DraftReinsertionError> {
    prepare_reinsertion(
        archives,
        source_stream,
        replacement_stream,
        replacement_glyph,
    )
    .map(|(analysis, _)| analysis)
}

/// 지원 원본과 version 관리 manifest만으로 draft를 만들고 전수 검산한다.
pub fn build_box_menu06_draft_disc(
    source: &VerifiedSourceImage,
) -> Result<Menu06DraftDiscBuild, Menu06DraftReinsertionError> {
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let loaded = TARGETS
        .iter()
        .map(|spec| {
            let raw = iso
                .read_file(spec.archive)
                .map_err(|source| error(source.to_string()))?;
            let archive = VddArchive::parse(&raw)
                .map_err(|source| error(format!("{} VDD 파싱 실패: {source}", spec.archive)))?;
            Ok((spec.archive, archive))
        })
        .collect::<Result<Vec<_>, Menu06DraftReinsertionError>>()?;

    let sequence_sources = TARGETS
        .iter()
        .filter(|spec| spec.kind == "sequence")
        .map(|spec| {
            let archive = loaded
                .iter()
                .find(|(path, _)| *path == spec.archive)
                .map(|(_, archive)| archive)
                .ok_or_else(|| error(format!("{} VDD 입력 누락", spec.archive)))?;
            let entry = archive
                .find(spec.target)
                .ok_or_else(|| error(format!("{}:{} 입력 누락", spec.archive, spec.target)))?;
            let plain = decompress(archive.read_file(entry)).map_err(|source| {
                error(format!(
                    "{}:{} 해제 실패: {source}",
                    spec.archive, spec.target
                ))
            })?;
            Ok((spec.archive, spec.target, plain))
        })
        .collect::<Result<Vec<_>, Menu06DraftReinsertionError>>()?;
    let text = verify_box_menu06_text(
        sequence_sources
            .iter()
            .map(|(archive, sequence, plain)| (*archive, *sequence, plain.as_slice())),
    )
    .map_err(|source| error(source.to_string()))?;
    let translation = verify_box_menu06_translation_manifest(&text)
        .map_err(|source| error(source.to_string()))?;
    if translation.product_eligible {
        return Err(error("MENU06 draft build가 승인 제품 입력을 받음"));
    }
    let source_stream =
        serialize_menu06_text_tokens(&text.tokens).map_err(|source| error(source.to_string()))?;

    let glyph_sources = TARGETS
        .iter()
        .filter(|spec| spec.kind == "glyph")
        .map(|spec| {
            let archive = loaded
                .iter()
                .find(|(path, _)| *path == spec.archive)
                .map(|(_, archive)| archive)
                .ok_or_else(|| error(format!("{} VDD 입력 누락", spec.archive)))?;
            let entry = archive
                .find(spec.target)
                .ok_or_else(|| error(format!("{}:{} 입력 누락", spec.archive, spec.target)))?;
            let plain = decompress(archive.read_file(entry)).map_err(|source| {
                error(format!(
                    "{}:{} 해제 실패: {source}",
                    spec.archive, spec.target
                ))
            })?;
            Ok((spec.archive, spec.target, plain))
        })
        .collect::<Result<Vec<_>, Menu06DraftReinsertionError>>()?;
    verify_box_menu06_glyph_assets(
        glyph_sources
            .iter()
            .map(|(archive, asset, plain)| (*archive, *asset, plain.as_slice())),
    )
    .map_err(|source| error(source.to_string()))?;
    let glyph_patches = glyph_sources
        .iter()
        .map(|(_, _, plain)| {
            patch_box_menu06_glyph_asset(plain, &translation.plan.glyph_mappings)
                .map_err(|source| error(source.to_string()))
        })
        .collect::<Result<Vec<_>, Menu06DraftReinsertionError>>()?;
    let replacement_glyph = glyph_patches
        .first()
        .map(|patch| patch.plain.as_slice())
        .ok_or_else(|| error("MENU06 draft glyph patch 누락"))?;
    if glyph_patches
        .iter()
        .any(|patch| patch.plain != replacement_glyph)
    {
        return Err(error("MENU06 두 물리 draft glyph patch 불일치"));
    }

    let (reinsertion, prepared) = prepare_reinsertion(
        loaded.iter().map(|(archive, vdd)| (*archive, vdd)),
        &source_stream,
        &translation.plan.rewritten_stream,
        replacement_glyph,
    )?;

    let mut data_plan = WritePlan::new();
    for target in &prepared {
        let entry = iso
            .find(target.spec.archive)
            .map_err(|source| error(source.to_string()))?;
        let file = logical_file(entry).map_err(error)?;
        add_vdd_entry_replacement(
            &mut data_plan,
            file,
            &target.original_archive,
            &target.original_vdd,
            &target.replacement_vdd,
            target.spec.target,
            target.replacement_cnx_len,
            &format!("menu06-draft:{}:{}", target.spec.scope, target.spec.kind),
            "pipeline::menu06_draft_reinsertion",
        )
        .map_err(error)?;
    }
    let planned_writes = data_plan.writes.len();
    let vdd_metadata_writes = data_plan
        .writes
        .iter()
        .filter(|write| matches!(write.intent, WriteIntent::Metadata))
        .count();
    let payload_writes = data_plan
        .writes
        .iter()
        .filter(|write| matches!(write.intent, WriteIntent::Data))
        .count();
    let touched_sectors = data_plan.touched_sectors().len();
    let final_expected_writes = planned_writes + touched_sectors;
    let output = apply_form1_write_plan(source.image(), &data_plan)
        .map_err(|source| error(source.to_string()))?;
    let final_image = DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
    for target in &prepared {
        verify_final_vdd(&final_iso, target)?;
    }
    let output = final_image.into_bytes();
    let image_sha256 = digest(&output);
    Ok(Menu06DraftDiscBuild {
        image: output,
        report: Menu06DraftDiscReport {
            schema_version: MENU06_DRAFT_DISC_SCHEMA_VERSION,
            reinsertion,
            planned_writes,
            vdd_metadata_writes,
            payload_writes,
            touched_sectors,
            final_expected_writes,
            image_sha256,
            product_eligible: false,
            runtime_verified: false,
        },
    })
}

#[cfg(test)]
#[path = "menu06_draft_reinsertion_tests.rs"]
mod tests;
