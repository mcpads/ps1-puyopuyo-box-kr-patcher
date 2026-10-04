//! `scene_e0.tsv`에서 작성이 끝난 PUYO1 family를 SEQ와 same-stem BTX로 준비한다.
//!
//! 일반 scene 분석·compiler·relinker·glyph writer를 한 경로로 조립하고,
//! 미작성 family나 PUYO2의 미확정 pixel source는 개발 쓰기에 포함하지 않는다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::analysis::{verify_box_subgame_seq_scene_glyphs, verify_box_subgame_seq_scene_messages};
use crate::archive::VddArchive;
use crate::coords::ExeMap;
use crate::disasm::{
    audit_btx_cell_consumers, verify_box_subgame_seq_drac_consumers,
    verify_box_subgame_seq_drac_glyphs, BtxNameAnalysis, BOX_SUBGAME_SEQ_DRAC_CONSUMER_SPECS,
    BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS,
};
use crate::font::SCENE_TRANSLATION_DRAFT_RASTER_PROFILE;
use crate::script::{
    verify_box_subgame_seq_drac_text, SubgameSeqLayoutAnalysis, SubgameSeqTextCandidateAnalysis,
    SubgameSeqTextRootAnalysis,
};

use super::btx_development_archive::{BtxDevelopmentArchiveSpec, PreparedBtxDevelopmentEntry};
use super::cnx_development_entry::prepare_cnx_development_entry_from_vdd;
use super::subgame_seq_scene_glyph_patch::patch_box_subgame_seq_scene_glyph_asset_with_profile;
use super::{
    collect_authored_subgame_seq_scene_translations, plan_subgame_seq_scene_translation,
    verify_box_subgame_seq_scene_relinks, verify_box_subgame_seq_scene_translation_manifest,
    SubgameSeqSceneTranslationStatus,
};

const ACTOR: &str = "pipeline::puyo1_scene_translation_development_entries";
const PUYO1_SEQ_PATH: &str = "/PUYO1/PP1_SEQ.VDD;1";
const PUYO1_GRP_PATH: &str = "/PUYO1/PP1_GRP.VDD;1";
const PUYO2_GRP_PATH: &str = "/PUYO2/PP2_GRP.VDD;1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Puyo1SceneTranslationDevelopmentReport {
    pub families: usize,
    pub translation_units: usize,
    pub draft_units: usize,
    pub approved_units: usize,
    pub glyph_mappings: usize,
    pub used_bytes: usize,
    pub remaining_bytes: usize,
}

pub(super) struct PreparedPuyo1SceneTranslationDevelopmentEntries {
    pub entries: Vec<PreparedBtxDevelopmentEntry>,
    pub surfaces: Vec<Puyo1SceneTranslationDevelopmentSurface>,
    pub report: Puyo1SceneTranslationDevelopmentReport,
}

pub(super) struct Puyo1SceneTranslationDevelopmentSurface {
    pub preview_stem: String,
    pub bytes: Vec<u8>,
}

pub(super) struct Puyo1SceneDevelopmentInputs<'a> {
    pub executables: &'a [(&'static ExeMap, &'a [u8])],
    pub sequence_sources: &'a [(String, String, Vec<u8>)],
    pub raw_archives: &'a BTreeMap<&'static str, Vec<u8>>,
    pub archives: &'a BTreeMap<&'static str, VddArchive>,
    pub text: &'a SubgameSeqTextCandidateAnalysis,
    pub layouts: &'a SubgameSeqLayoutAnalysis,
    pub roots: &'a SubgameSeqTextRootAnalysis,
    pub btx_names: &'a BtxNameAnalysis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Puyo1SceneTranslationDevelopmentError(String);

impl fmt::Display for Puyo1SceneTranslationDevelopmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for Puyo1SceneTranslationDevelopmentError {}

fn error(message: impl Into<String>) -> Puyo1SceneTranslationDevelopmentError {
    Puyo1SceneTranslationDevelopmentError(message.into())
}

fn source_view(sources: &[(String, String, Vec<u8>)]) -> impl Iterator<Item = (&str, &str, &[u8])> {
    sources
        .iter()
        .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice()))
}

fn spec(
    iso_path: &'static str,
    entry_name: &'static str,
    expected_cnx_subtype: [u8; 3],
) -> BtxDevelopmentArchiveSpec {
    BtxDevelopmentArchiveSpec {
        iso_path,
        entry_name,
        expected_cnx_subtype,
        patch_id: "puyo1-scene-translation-development-entry",
        actor: ACTOR,
    }
}

fn preview_stem(sequence: &str) -> Result<String, Puyo1SceneTranslationDevelopmentError> {
    let stem = sequence
        .strip_suffix("_SEQ.SEQ")
        .filter(|stem| {
            !stem.is_empty()
                && stem
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        })
        .ok_or_else(|| error(format!("PUYO1 scene sequence 이름 형식 불일치: {sequence}")))?;
    Ok(format!("puyo1-{}", stem.to_ascii_lowercase()))
}

/// 작성이 끝난 scene family를 일반 경로의 PUYO1 SEQ/BTX 엔트리로 준비한다.
pub(super) fn prepare_puyo1_scene_translation_entries(
    inputs: Puyo1SceneDevelopmentInputs<'_>,
) -> Result<PreparedPuyo1SceneTranslationDevelopmentEntries, Puyo1SceneTranslationDevelopmentError>
{
    let Puyo1SceneDevelopmentInputs {
        executables,
        sequence_sources,
        raw_archives,
        archives,
        text,
        layouts,
        roots,
        btx_names,
    } = inputs;
    let sources = || source_view(sequence_sources);
    let drac_text = verify_box_subgame_seq_drac_text(sources(), text, layouts)
        .map_err(|source| error(source.to_string()))?;
    let drac_consumers = verify_box_subgame_seq_drac_consumers(
        executables.iter().copied().filter(|(map, _)| {
            BOX_SUBGAME_SEQ_DRAC_CONSUMER_SPECS
                .iter()
                .any(|candidate| candidate.exe == map.name)
        }),
        sources(),
        &drac_text,
    )
    .map_err(|source| error(source.to_string()))?;
    let btx_cells = audit_btx_cell_consumers(executables.iter().copied())
        .map_err(|source| error(source.to_string()))?;
    let drac_glyphs = verify_box_subgame_seq_drac_glyphs(
        executables.iter().copied().filter(|(map, _)| {
            BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS
                .iter()
                .any(|candidate| candidate.exe == map.name)
        }),
        sources(),
        [PUYO1_GRP_PATH, PUYO2_GRP_PATH]
            .into_iter()
            .map(|path| (path, &archives[path])),
        &drac_text,
        btx_names,
        &btx_cells,
    )
    .map_err(|source| error(source.to_string()))?;
    let scene_glyphs = verify_box_subgame_seq_scene_glyphs(
        executables.iter().copied(),
        sources(),
        [PUYO1_GRP_PATH, PUYO2_GRP_PATH]
            .into_iter()
            .map(|path| (path, &archives[path])),
        layouts,
        roots,
        btx_names,
        (&drac_consumers, &drac_glyphs),
    )
    .map_err(|source| error(source.to_string()))?;
    let scene_messages =
        verify_box_subgame_seq_scene_messages(sources(), layouts, &scene_glyphs, &drac_consumers)
            .map_err(|source| error(source.to_string()))?;
    let scene_relinks =
        verify_box_subgame_seq_scene_relinks(sources(), layouts, &scene_messages, &scene_glyphs)
            .map_err(|source| error(source.to_string()))?;
    let scene_manifest = verify_box_subgame_seq_scene_translation_manifest(&scene_messages)
        .map_err(|source| error(source.to_string()))?;
    let translations = collect_authored_subgame_seq_scene_translations(&scene_manifest)
        .map_err(|source| error(source.to_string()))?;
    if translations.is_empty() {
        return Err(error("PUYO1 scene 작성 완료 family가 없음"));
    }
    let selected_ids = translations
        .iter()
        .map(|translation| translation.logical_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let draft_units = scene_manifest
        .entries
        .iter()
        .filter(|entry| {
            selected_ids.contains(entry.logical_id.as_str())
                && entry.status == SubgameSeqSceneTranslationStatus::DraftNeedsHumanReview
        })
        .count();
    let approved_units = scene_manifest
        .entries
        .iter()
        .filter(|entry| {
            selected_ids.contains(entry.logical_id.as_str())
                && entry.status == SubgameSeqSceneTranslationStatus::Approved
        })
        .count();
    let plan = plan_subgame_seq_scene_translation(
        &translations,
        sources(),
        &scene_manifest,
        &scene_messages,
        &scene_glyphs,
        &scene_relinks,
    )
    .map_err(|source| error(source.to_string()))?;

    let puyo1_repacks = plan
        .repacks
        .iter()
        .filter(|repack| repack.scope == "PUYO1")
        .collect::<Vec<_>>();
    if puyo1_repacks.len() != plan.families {
        return Err(error("PUYO1 scene family/repack 분모 불일치"));
    }
    let mut entries = Vec::with_capacity(puyo1_repacks.len() * 2);
    let mut surfaces = Vec::with_capacity(puyo1_repacks.len());
    let mut glyph_mapping_count = 0usize;
    let mut used_bytes = 0usize;
    let mut remaining_bytes = 0usize;
    for repack in puyo1_repacks {
        let sequence = repack.sequence_name;
        let sequence_source = sequence_sources
            .iter()
            .find(|(archive, candidate, _)| archive == PUYO1_SEQ_PATH && candidate == sequence)
            .map(|(_, _, raw)| raw.as_slice())
            .ok_or_else(|| error(format!("PUYO1 {sequence} 원본 SEQ 누락")))?;
        let (sequence_entry, ()) = prepare_cnx_development_entry_from_vdd(
            spec(PUYO1_SEQ_PATH, sequence, *b"bin"),
            raw_archives[PUYO1_SEQ_PATH].clone(),
            |plain| {
                if plain != sequence_source {
                    return Err(format!("PUYO1 {sequence} 개발 SEQ 원본 drift"));
                }
                Ok((repack.sequence.clone(), ()))
            },
        )
        .map_err(|source| error(source.to_string()))?;

        let glyph_row = scene_glyphs
            .rows
            .iter()
            .find(|row| row.scope == "PUYO1" && row.sequence == sequence)
            .ok_or_else(|| error(format!("PUYO1 {sequence} scene glyph 행 누락")))?;
        let glyph_mappings = plan
            .glyph_mappings
            .iter()
            .filter(|mapping| mapping.sequence == sequence)
            .cloned()
            .collect::<Vec<_>>();
        let (glyph_entry, glyph_patch) = prepare_cnx_development_entry_from_vdd(
            spec(glyph_row.grp_archive, glyph_row.same_stem_asset, *b"bi_"),
            raw_archives[glyph_row.grp_archive].clone(),
            |plain| {
                patch_box_subgame_seq_scene_glyph_asset_with_profile(
                    glyph_row,
                    sequence_source,
                    plain,
                    &glyph_mappings,
                    SCENE_TRANSLATION_DRAFT_RASTER_PROFILE,
                )
                .map(|patch| (patch.plain.clone(), patch))
                .map_err(|source| source.to_string())
            },
        )
        .map_err(|source| error(source.to_string()))?;
        surfaces.push(Puyo1SceneTranslationDevelopmentSurface {
            preview_stem: preview_stem(sequence)?,
            bytes: glyph_entry.patched_plain().to_vec(),
        });
        glyph_mapping_count += glyph_patch.cell_indices.len();
        used_bytes += repack.used_bytes;
        remaining_bytes += repack.remaining_bytes;
        entries.extend([sequence_entry, glyph_entry]);
    }
    let report = Puyo1SceneTranslationDevelopmentReport {
        families: plan.families,
        translation_units: translations.len(),
        draft_units,
        approved_units,
        glyph_mappings: glyph_mapping_count,
        used_bytes,
        remaining_bytes,
    };
    if report.families == 0
        || report.translation_units == 0
        || report.translation_units != report.draft_units + report.approved_units
        || report.glyph_mappings == 0
        || entries.len() != report.families * 2
        || surfaces.len() != report.families
    {
        return Err(error("PUYO1 scene 개발 보고 경계 불일치"));
    }
    Ok(PreparedPuyo1SceneTranslationDevelopmentEntries {
        entries,
        surfaces,
        report,
    })
}
