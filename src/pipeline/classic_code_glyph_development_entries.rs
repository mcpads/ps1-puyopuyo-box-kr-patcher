//! classic PUYO의 code glyph 계열 수동 번역을 CNX 엔트리 단위로 준비한다.
//!
//! `MENU06`, PUYO2 E6와 작성 완료된 PUYO1 scene을 각 소유 compiler에 맡긴다.
//! 이 모듈은 검증·합성 결과를 한 번 읽은 불변 VDD에 결속할
//! 뿐이며, VDD 재패킹과 disc 배치는 상위 조정자에게 넘긴다. PUYO2 DRAC는
//! player-reachable 소비 경로가 아직 닫히지 않았으므로 의도적으로 쓰지 않는다.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use crate::archive::VddArchive;
use crate::compress::{decompress, is_cnx};
use crate::coords::{exe_map, ExeMap};
use crate::disasm::{
    audit_btx_name_consumers, verify_box_subgame_seq_e6_text, BOX_BTX_NAME_CONSUMERS,
    DEMO_FONT_CODE_TABLE_ENTRIES, DEMO_FONT_CODE_TABLE_RUNTIME,
};
use crate::disc::{Iso9660, PsxExe, VerifiedSourceImage};
use crate::graphics::e6_demo_font_patch::{
    audit_box_puyo2_e6_demo_font_geometry, patch_box_puyo2_e6_demo_font,
};
use crate::graphics::menu06_glyph_patch::{
    patch_box_menu06_glyph_asset, verify_box_menu06_glyph_assets,
};
use crate::script::{
    serialize_menu06_text_tokens, verify_box_menu06_text, verify_box_menu06_translation_manifest,
    verify_box_subgame_seq_layouts, verify_box_subgame_seq_text_candidates,
    verify_box_subgame_seq_text_roots, BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS, MENU06_STREAM_END_OFFSET,
    MENU06_STREAM_OFFSET,
};

use super::btx_development_archive::{BtxDevelopmentArchiveSpec, PreparedBtxDevelopmentEntry};
use super::cnx_development_entry::prepare_cnx_development_entry_from_vdd;
use super::puyo1_scene_translation_development_entries::{
    prepare_puyo1_scene_translation_entries, PreparedPuyo1SceneTranslationDevelopmentEntries,
    Puyo1SceneDevelopmentInputs, Puyo1SceneTranslationDevelopmentSurface,
};
use super::{
    plan_box_puyo2_subgame_seq_e6_translation, repack_box_puyo2_subgame_seq_e6_sequence,
    verify_box_puyo2_e6_translation_manifest,
};

const ACTOR: &str = "pipeline::classic_code_glyph_development_entries";
const PUYO1_EXE_PATH: &str = "/PUYO1/PUYO1.EXE;1";
const PUYO2_EXE_PATH: &str = "/PUYO2/PUYO2.EXE;1";
const PUYO1_SEQ_PATH: &str = "/PUYO1/PP1_SEQ.VDD;1";
const PUYO2_SEQ_PATH: &str = "/PUYO2/PP2_SEQ.VDD;1";
const PUYO1_GRP_PATH: &str = "/PUYO1/PP1_GRP.VDD;1";
const PUYO2_GRP_PATH: &str = "/PUYO2/PP2_GRP.VDD;1";
const MENU1_SEQUENCE: &str = "MENU1.SEQ";
const MENU06_ASSET: &str = "MENU06.BTX";
const DEMO_FONT_ASSET: &str = "DEMOFONT.BTX";

const ARCHIVE_PATHS: [&str; 4] = [
    PUYO1_SEQ_PATH,
    PUYO2_SEQ_PATH,
    PUYO1_GRP_PATH,
    PUYO2_GRP_PATH,
];

/// version 관리 E6 manifest가 선언한 물리 SEQ 집합.
const PUYO2_E6_SEQUENCES: [&str; 33] = [
    "MBAN_SEQ.SEQ",
    "MDRC_SEQ.SEQ",
    "MDRG_SEQ.SEQ",
    "MFUF_SEQ.SEQ",
    "MHAR_SEQ.SEQ",
    "MKER_SEQ.SEQ",
    "MKET_SEQ.SEQ",
    "MLUL_SEQ.SEQ",
    "MMAM_SEQ.SEQ",
    "MMAS_SEQ.SEQ",
    "MMIN_SEQ.SEQ",
    "MMOM_SEQ.SEQ",
    "MMUM_SEQ.SEQ",
    "MMZO_SEQ.SEQ",
    "MNAS_SEQ.SEQ",
    "MNOM_SEQ.SEQ",
    "MOWL_SEQ.SEQ",
    "MPAA_SEQ.SEQ",
    "MPAK_SEQ.SEQ",
    "MPAN_SEQ.SEQ",
    "MPAO_SEQ.SEQ",
    "MPOT_SEQ.SEQ",
    "MSAM_SEQ.SEQ",
    "MSAT_SEQ.SEQ",
    "MSCO_SEQ.SEQ",
    "MSEL_SEQ.SEQ",
    "MSHE_SEQ.SEQ",
    "MSKE_SEQ.SEQ",
    "MTAR_SEQ.SEQ",
    "MWIL_SEQ.SEQ",
    "MWIT_SEQ.SEQ",
    "MZOH_SEQ.SEQ",
    "MZON_SEQ.SEQ",
];

/// 서로 다른 compiler를 쓰는 한 code glyph 자산군의 정적 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicCodeGlyphFamilyReport {
    pub family: &'static str,
    pub translation_units: usize,
    pub draft_units: usize,
    pub approved_units: usize,
    pub sequence_entries: usize,
    pub glyph_entries: usize,
    pub glyph_mappings: usize,
    pub used_bytes: usize,
    pub remaining_bytes: usize,
    pub final_readback_exact: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 세 code glyph 자산군을 함께 준비한 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicCodeGlyphDevelopmentEntriesReport {
    pub menu06: ClassicCodeGlyphFamilyReport,
    pub puyo2_e6: ClassicCodeGlyphFamilyReport,
    pub puyo1_scene: ClassicCodeGlyphFamilyReport,
    pub physical_entries: usize,
    pub auxiliary_sequence_entries: usize,
    pub archives: usize,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// VDD 합성 전의 CNX 엔트리와 세 atlas preview 원천.
pub(super) struct PreparedClassicCodeGlyphDevelopmentEntries {
    entries: Vec<PreparedBtxDevelopmentEntry>,
    verified_archives: BTreeSet<&'static str>,
    pub menu06_surface: Vec<u8>,
    pub puyo2_e6_surface: Vec<u8>,
    pub puyo1_scene_surfaces: Vec<Puyo1SceneTranslationDevelopmentSurface>,
    pub report: ClassicCodeGlyphDevelopmentEntriesReport,
}

impl PreparedClassicCodeGlyphDevelopmentEntries {
    pub fn entry_refs(
        &self,
        iso_path: &str,
    ) -> Result<Vec<&PreparedBtxDevelopmentEntry>, ClassicCodeGlyphDevelopmentEntriesError> {
        let entries = self
            .entries
            .iter()
            .filter(|entry| entry.spec.iso_path.eq_ignore_ascii_case(iso_path))
            .collect::<Vec<_>>();
        if entries.is_empty() || !ARCHIVE_PATHS.iter().any(|path| path == &iso_path) {
            return Err(error(format!(
                "classic code glyph archive 입력 경로가 아님: {iso_path}"
            )));
        }
        Ok(entries)
    }

    pub fn record_verified_archive_readback(
        &mut self,
        iso_path: &'static str,
    ) -> Result<(), ClassicCodeGlyphDevelopmentEntriesError> {
        if !ARCHIVE_PATHS.contains(&iso_path) {
            return Err(error(format!(
                "classic code glyph 최종 readback 경로가 아님: {iso_path}"
            )));
        }
        if !self.verified_archives.insert(iso_path) {
            return Err(error(format!(
                "classic code glyph 최종 readback 중복: {iso_path}"
            )));
        }
        let all = |paths: &[&str]| {
            paths
                .iter()
                .all(|path| self.verified_archives.contains(path))
        };
        self.report.menu06.final_readback_exact = all(&ARCHIVE_PATHS);
        self.report.puyo2_e6.final_readback_exact = all(&[PUYO2_SEQ_PATH, PUYO2_GRP_PATH]);
        self.report.puyo1_scene.final_readback_exact = all(&[PUYO1_SEQ_PATH, PUYO1_GRP_PATH]);
        Ok(())
    }
}

/// 원본 분석·manifest·CNX 엔트리 준비 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicCodeGlyphDevelopmentEntriesError(String);

impl fmt::Display for ClassicCodeGlyphDevelopmentEntriesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ClassicCodeGlyphDevelopmentEntriesError {}

fn error(message: impl Into<String>) -> ClassicCodeGlyphDevelopmentEntriesError {
    ClassicCodeGlyphDevelopmentEntriesError(message.into())
}

#[derive(Debug)]
struct LoadedExecutable {
    map: &'static ExeMap,
    text: Vec<u8>,
}

fn load_executable(
    iso: &Iso9660<'_>,
    path: &'static str,
    name: &'static str,
) -> Result<LoadedExecutable, ClassicCodeGlyphDevelopmentEntriesError> {
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
    archives: &BTreeMap<&'static str, VddArchive>,
) -> Result<Vec<(String, String, Vec<u8>)>, ClassicCodeGlyphDevelopmentEntriesError> {
    let mut sources = Vec::new();
    for expectation in BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS {
        let archive = archives
            .get(expectation.archive)
            .ok_or_else(|| error(format!("{} SEQ archive 누락", expectation.archive)))?;
        for entry in archive
            .entries()
            .iter()
            .filter(|entry| entry.name().to_ascii_uppercase().ends_with(".SEQ"))
        {
            let cnx = archive.read_file(entry);
            if !is_cnx(cnx) {
                return Err(error(format!(
                    "{}:{}가 CNX v2가 아님",
                    expectation.archive,
                    entry.name()
                )));
            }
            let plain = decompress(cnx).map_err(|source| {
                error(format!(
                    "{}:{} 해제 실패: {source}",
                    expectation.archive,
                    entry.name()
                ))
            })?;
            sources.push((
                expectation.archive.to_string(),
                entry.name().to_string(),
                plain,
            ));
        }
    }
    Ok(sources)
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
        patch_id: "classic-code-glyph-development-entry",
        actor: ACTOR,
    }
}

fn replace_menu06_stream(
    original: &[u8],
    source_stream: &[u8],
    replacement_stream: &[u8],
) -> Result<Vec<u8>, String> {
    let span = MENU06_STREAM_OFFSET..MENU06_STREAM_END_OFFSET;
    if source_stream.len() != span.len()
        || replacement_stream.len() != span.len()
        || original.get(span.clone()) != Some(source_stream)
    {
        return Err("MENU06 개발 stream 원본/길이 불일치".to_string());
    }
    let mut output = original.to_vec();
    output[span].copy_from_slice(replacement_stream);
    Ok(output)
}

/// 세 자산군의 번역 상태와 정적 소비 판정을 행별 TSV로 직렬화한다.
pub fn render_classic_code_glyph_development_entries_tsv(
    report: &ClassicCodeGlyphDevelopmentEntriesReport,
) -> String {
    let mut output = String::from(
        "family\ttranslation_units\tdraft_units\tapproved_units\tsequence_entries\t\
         glyph_entries\tglyph_mappings\tused_bytes\tremaining_bytes\t\
         final_readback_exact\truntime_verified\tproduct_eligible\n",
    );
    for family in [&report.menu06, &report.puyo2_e6, &report.puyo1_scene] {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            family.family,
            family.translation_units,
            family.draft_units,
            family.approved_units,
            family.sequence_entries,
            family.glyph_entries,
            family.glyph_mappings,
            family.used_bytes,
            family.remaining_bytes,
            family.final_readback_exact,
            family.runtime_verified,
            family.product_eligible,
        )
        .expect("String 쓰기는 실패하지 않음");
    }
    output
}

/// 수동 manifest 세 묶음을 불변 원본의 선택된 CNX 엔트리로 준비한다.
pub(super) fn prepare_classic_code_glyph_development_entries(
    source: &VerifiedSourceImage,
) -> Result<PreparedClassicCodeGlyphDevelopmentEntries, ClassicCodeGlyphDevelopmentEntriesError> {
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let mut raw_archives = BTreeMap::new();
    let mut archives = BTreeMap::new();
    let source_archive_paths = BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS
        .iter()
        .map(|expectation| expectation.archive)
        .chain([PUYO1_GRP_PATH, PUYO2_GRP_PATH])
        .collect::<BTreeSet<_>>();
    for path in source_archive_paths {
        let raw = iso
            .read_file(path)
            .map_err(|source| error(source.to_string()))?;
        let archive = VddArchive::parse(&raw)
            .map_err(|source| error(format!("{path} VDD 파싱 실패: {source}")))?;
        raw_archives.insert(path, raw);
        archives.insert(path, archive);
    }

    let sequence_sources = read_sequence_sources(&archives)?;
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

    let mut entries = Vec::new();

    let (title, ()) = prepare_cnx_development_entry_from_vdd(
        spec(PUYO2_SEQ_PATH, "PP2_TTL.SEQ", *b"bin"),
        raw_archives[PUYO2_SEQ_PATH].clone(),
        |plain| {
            if plain.len() != 18_440
                || format!("{:x}", Sha256::digest(plain))
                    != "59a7d64838d17f2ca018a3c3f8d67978a60a23e23b7cf46385c1c232eb9f2951"
            {
                return Err("PP2_TTL 원본 크기·SHA-256 불일치".into());
            }
            let mut patched = plain.to_vec();
            // PUYO2 short SPRT는 화면 x에서 cell anchor를 뺀다.
            // 두 글자와 여섯 눈 효과만 +32 이동해 금색 원판 중심에 맞춘다.
            // 통·원판·저작권·후속 scene과 스크립트 명령은 보존한다.
            for cell in [0, 1, 3, 4, 5, 6, 7, 8] {
                let anchor = 0x824 + cell * 12 + 8;
                if plain[anchor..anchor + 2] != [0, 0] {
                    return Err(format!("PP2_TTL cell {cell} 원본 anchor 불일치"));
                }
                patched[anchor..anchor + 2].copy_from_slice(&(-32_i16).to_le_bytes());
            }
            Ok((patched, ()))
        },
    )
    .map_err(|source| error(source.to_string()))?;
    entries.push(title);
    for path in [PUYO1_SEQ_PATH, PUYO2_SEQ_PATH] {
        let (entry, ()) = prepare_cnx_development_entry_from_vdd(
            spec(path, "MENU2.SEQ", *b"bin"),
            raw_archives[path].clone(),
            |plain| {
                crate::graphics::mode_help_surface::patch_sequence(plain, path == PUYO1_SEQ_PATH)
                    .map(|bytes| (bytes, ()))
            },
        )
        .map_err(|e| error(e.to_string()))?;
        entries.push(entry);
    }

    let menu06_text =
        verify_box_menu06_text(source_view()).map_err(|source| error(source.to_string()))?;
    let menu06_translation = verify_box_menu06_translation_manifest(&menu06_text)
        .map_err(|source| error(source.to_string()))?;
    let source_stream = serialize_menu06_text_tokens(&menu06_text.tokens)
        .map_err(|source| error(source.to_string()))?;
    for path in [PUYO1_SEQ_PATH, PUYO2_SEQ_PATH] {
        let (entry, ()) = prepare_cnx_development_entry_from_vdd(
            spec(path, MENU1_SEQUENCE, *b"bin"),
            raw_archives[path].clone(),
            |plain| {
                replace_menu06_stream(
                    plain,
                    &source_stream,
                    &menu06_translation.plan.rewritten_stream,
                )
                .map(|patched| (patched, ()))
            },
        )
        .map_err(|source| error(source.to_string()))?;
        entries.push(entry);
    }
    let menu06_glyph_sources = [PUYO1_GRP_PATH, PUYO2_GRP_PATH]
        .into_iter()
        .map(|path| {
            let archive = &archives[path];
            let entry = archive
                .find(MENU06_ASSET)
                .ok_or_else(|| error(format!("{path}:{MENU06_ASSET} 누락")))?;
            let plain =
                decompress(archive.read_file(entry)).map_err(|source| error(source.to_string()))?;
            Ok((path, MENU06_ASSET, plain))
        })
        .collect::<Result<Vec<_>, ClassicCodeGlyphDevelopmentEntriesError>>()?;
    verify_box_menu06_glyph_assets(
        menu06_glyph_sources
            .iter()
            .map(|(archive, asset, plain)| (*archive, *asset, plain.as_slice())),
    )
    .map_err(|source| error(source.to_string()))?;
    let mut menu06_surface = None;
    for path in [PUYO1_GRP_PATH, PUYO2_GRP_PATH] {
        let (entry, _) = prepare_cnx_development_entry_from_vdd(
            spec(path, MENU06_ASSET, *b"bi_"),
            raw_archives[path].clone(),
            |plain| {
                patch_box_menu06_glyph_asset(plain, &menu06_translation.plan.glyph_mappings)
                    .map(|patch| (patch.plain, ()))
                    .map_err(|source| source.to_string())
            },
        )
        .map_err(|source| error(source.to_string()))?;
        if let Some(canonical) = &menu06_surface {
            if canonical != entry.patched_plain() {
                return Err(error("MENU06 두 물리 atlas 합성 결과 불일치"));
            }
        } else {
            menu06_surface = Some(entry.patched_plain().to_vec());
        }
        entries.push(entry);
    }

    let executables = [
        load_executable(&iso, PUYO1_EXE_PATH, "PUYO1.EXE")?,
        load_executable(&iso, PUYO2_EXE_PATH, "PUYO2.EXE")?,
    ];
    let btx_specs = BOX_BTX_NAME_CONSUMERS
        .iter()
        .copied()
        .filter(|candidate| matches!(candidate.exe, "PUYO1.EXE" | "PUYO2.EXE"))
        .collect::<Vec<_>>();
    let btx_names = audit_btx_name_consumers(
        &btx_specs,
        executables
            .iter()
            .map(|executable| (executable.map, executable.text.as_slice())),
        [PUYO1_GRP_PATH, PUYO2_GRP_PATH]
            .into_iter()
            .map(|path| (path, &archives[path])),
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
        .ok_or_else(|| error("PUYO2.EXE 분석 입력 누락"))?;
    let code_table_offset = puyo2_executable
        .map
        .text_offset(DEMO_FONT_CODE_TABLE_RUNTIME)
        .ok_or_else(|| error("PUYO2 E6 code table이 EXE 밖임"))?;
    let code_table_end = code_table_offset
        .checked_add(DEMO_FONT_CODE_TABLE_ENTRIES * 2)
        .ok_or_else(|| error("PUYO2 E6 code table 끝 overflow"))?;
    let code_table = puyo2_executable
        .text
        .get(code_table_offset..code_table_end)
        .ok_or_else(|| error("PUYO2 E6 code table 범위 이탈"))?;
    let e6_translation = verify_box_puyo2_e6_translation_manifest(&e6_text)
        .map_err(|source| error(source.to_string()))?;
    let e6_plan = plan_box_puyo2_subgame_seq_e6_translation(
        &e6_translation.translations,
        code_table,
        &e6_text,
    )
    .map_err(|source| error(source.to_string()))?;
    audit_box_puyo2_e6_demo_font_geometry(&e6_plan.glyph_mappings)
        .map_err(|source| error(source.to_string()))?;
    let planned_sequences = e6_plan
        .replacements
        .iter()
        .map(|replacement| replacement.sequence.as_str())
        .collect::<BTreeSet<_>>();
    if planned_sequences != PUYO2_E6_SEQUENCES.into_iter().collect() {
        return Err(error("PUYO2 E6 manifest와 물리 SEQ 집합 불일치"));
    }
    for sequence in PUYO2_E6_SEQUENCES {
        let (entry, _) = prepare_cnx_development_entry_from_vdd(
            spec(PUYO2_SEQ_PATH, sequence, *b"bin"),
            raw_archives[PUYO2_SEQ_PATH].clone(),
            |plain| {
                repack_box_puyo2_subgame_seq_e6_sequence(sequence, plain, &e6_plan)
                    .map(|repack| (repack.sequence_bytes, ()))
                    .map_err(|source| source.to_string())
            },
        )
        .map_err(|source| error(source.to_string()))?;
        entries.push(entry);
    }
    let (e6_font, e6_font_patch) = prepare_cnx_development_entry_from_vdd(
        spec(PUYO2_GRP_PATH, DEMO_FONT_ASSET, *b"bi_"),
        raw_archives[PUYO2_GRP_PATH].clone(),
        |plain| {
            patch_box_puyo2_e6_demo_font(plain, &e6_plan.glyph_mappings)
                .map(|patch| (patch.plain.clone(), patch))
                .map_err(|source| source.to_string())
        },
    )
    .map_err(|source| error(source.to_string()))?;
    let puyo2_e6_surface = e6_font.patched_plain().to_vec();
    entries.push(e6_font);

    let executable_sources = executables
        .iter()
        .map(|executable| (executable.map, executable.text.as_slice()))
        .collect::<Vec<_>>();
    let PreparedPuyo1SceneTranslationDevelopmentEntries {
        entries: scene_entries,
        surfaces: puyo1_scene_surfaces,
        report: scene_report,
    } = prepare_puyo1_scene_translation_entries(Puyo1SceneDevelopmentInputs {
        executables: &executable_sources,
        sequence_sources: &sequence_sources,
        raw_archives: &raw_archives,
        archives: &archives,
        text: &text,
        layouts: &layouts,
        roots: &roots,
        btx_names: &btx_names,
    })
    .map_err(|source| error(source.to_string()))?;
    entries.extend(scene_entries);

    let report = ClassicCodeGlyphDevelopmentEntriesReport {
        menu06: ClassicCodeGlyphFamilyReport {
            family: "MENU06",
            translation_units: menu06_translation.entries.len(),
            draft_units: menu06_translation.draft_needs_human_review,
            approved_units: menu06_translation.approved,
            sequence_entries: 2,
            glyph_entries: 2,
            glyph_mappings: menu06_translation.plan.glyph_mappings.len(),
            used_bytes: menu06_translation.plan.rewritten_stream.len(),
            remaining_bytes: 0,
            final_readback_exact: false,
            runtime_verified: false,
            product_eligible: false,
        },
        puyo2_e6: ClassicCodeGlyphFamilyReport {
            family: "PUYO2-E6",
            translation_units: e6_translation.spans,
            draft_units: e6_translation.draft_needs_human_review,
            approved_units: e6_translation.approved,
            sequence_entries: PUYO2_E6_SEQUENCES.len(),
            glyph_entries: 1,
            glyph_mappings: e6_font_patch.changed_cells,
            used_bytes: e6_plan.used_bytes,
            remaining_bytes: e6_plan.remaining_bytes,
            final_readback_exact: false,
            runtime_verified: false,
            product_eligible: false,
        },
        puyo1_scene: ClassicCodeGlyphFamilyReport {
            family: "PUYO1-SCENE",
            translation_units: scene_report.translation_units,
            draft_units: scene_report.draft_units,
            approved_units: scene_report.approved_units,
            sequence_entries: scene_report.families,
            glyph_entries: scene_report.families,
            glyph_mappings: scene_report.glyph_mappings,
            used_bytes: scene_report.used_bytes,
            remaining_bytes: scene_report.remaining_bytes,
            final_readback_exact: false,
            runtime_verified: false,
            product_eligible: false,
        },
        physical_entries: entries.len(),
        auxiliary_sequence_entries: 3,
        archives: ARCHIVE_PATHS.len(),
        runtime_verified: false,
        product_eligible: false,
    };
    let expected_physical_entries = report.menu06.sequence_entries
        + report.menu06.glyph_entries
        + report.puyo2_e6.sequence_entries
        + report.puyo2_e6.glyph_entries
        + report.puyo1_scene.sequence_entries
        + report.puyo1_scene.glyph_entries
        + report.auxiliary_sequence_entries;
    if report.physical_entries != expected_physical_entries
        || report.menu06.translation_units == 0
        || report.puyo2_e6.translation_units == 0
        || report.puyo1_scene.translation_units == 0
        || report.menu06.product_eligible
        || report.puyo2_e6.product_eligible
        || report.puyo1_scene.product_eligible
        || report.runtime_verified
        || report.product_eligible
    {
        return Err(error("classic code glyph 개발 entry 판정 drift"));
    }
    Ok(PreparedClassicCodeGlyphDevelopmentEntries {
        entries,
        verified_archives: BTreeSet::new(),
        menu06_surface: menu06_surface.ok_or_else(|| error("MENU06 preview 누락"))?,
        puyo2_e6_surface,
        puyo1_scene_surfaces,
        report,
    })
}

#[cfg(test)]
#[path = "classic_code_glyph_development_entries_tests.rs"]
mod tests;
