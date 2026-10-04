//! QUEST sparse glyph cache의 첫 active upload용 비배포 입력.
//!
//! 기존 `poc-approved` 첫 패배 대사를 page 62에 결속한다. 문장용 code는
//! 정적 page 집합을 다시 계산해 믿지 않고, hook이 실제 읽는 package의
//! delta assignment를 해제한 순서와 원본 QUEST code table을 결합한다.
//! 이 모듈은 stream·VDD 준비만 소유하며 full-disc 쓰기는 `src/patch`를
//! 사용하는 sparse cache disc builder에 맡긴다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::disc::{Iso9660, VerifiedSourceImage};
use crate::font::{QuestCodeTable, QuestGlyphSlot, QUEST_FONT_WIDE_CELL_WIDTH};
use crate::script::{
    approved_quest_poc_translation, parse_quest_token, QuestToken, QuestTranslationBatchAnalysis,
    QUEST_DEFEAT_DIALOGUE_ID,
};

use super::disc_archive_patch::DiscArchiveEntryPatch;
use super::quest_glyph_cache_disc::{build_quest_glyph_cache_disc, QuestGlyphCacheDiscReport};
use super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_glyph_cache_plan::QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX;
use super::quest_poc::{
    encode_quest_poc_character, prepare_quest_poc_assets, quest_code_table,
    rebuild_prepared_town00, translate_town00_with_codes, PreparedQuestPocAssets, QuestPocMode,
};
use super::quest_translation_routing::{
    QuestTranslationPlacement, QuestTranslationRoutingAnalysis,
};

pub const QUEST_GLYPH_CACHE_UPLOAD_POC_SCHEMA_VERSION: &str = "quest-glyph-cache-upload-poc-v2";
pub const QUEST_GLYPH_CACHE_UPLOAD_DISC_POC_SCHEMA_VERSION: &str =
    "quest-glyph-cache-upload-disc-poc-v2";
pub const QUEST_GLYPH_CACHE_UPLOAD_POC_STREAM_ID: &str = "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x21CC";
pub const QUEST_GLYPH_CACHE_UPLOAD_POC_BATCH_ID: &str = "quest-town00-puyoman-results-001";
pub const QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID: u8 = 62;
pub const QUEST_GLYPH_CACHE_UPLOAD_POC_RECORD_OFFSET: usize = 0x21CC;

/// page assignment 한 글자가 실제 text code로 이어지는 결속.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestGlyphCachePageCode {
    pub glyph: char,
    pub catalog_index: u16,
    pub slot_index: usize,
    pub code: [u8; 2],
}

/// 첫 active upload 입력의 정적 검산 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheUploadPocPlan {
    pub schema_version: &'static str,
    pub stream_id: &'static str,
    pub batch_id: &'static str,
    pub page_id: u8,
    pub routed_record: [u8; 4],
    pub page_glyphs: usize,
    pub first_slot_index: usize,
    pub last_slot_index: usize,
    pub poc_glyphs: usize,
    pub source_stream_bytes: usize,
    pub source_stream_sha256: String,
    pub replacement_stream_sha256: String,
    pub town00_cnx_bytes: usize,
    pub page_assignment_exact: bool,
    pub stream_roundtrip_exact: bool,
    pub poc_translation_approved: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
}

/// page 62 stream과 sparse cache 실행기를 함께 설치한 exact disc 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheUploadPocReport {
    pub schema_version: &'static str,
    pub input: QuestGlyphCacheUploadPocPlan,
    pub disc: QuestGlyphCacheDiscReport,
    pub seq_vdd_exact: bool,
    pub town00_plain_exact: bool,
    pub original_font_archive_unchanged: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheUploadPocBuild {
    pub image: Vec<u8>,
    pub report: QuestGlyphCacheUploadPocReport,
}

/// full-disc builder가 직접 소비하는 VDD 교체 준비물.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedQuestGlyphCacheUploadPoc {
    pub prepared: PreparedQuestPocAssets,
    pub page_codes: Vec<QuestGlyphCachePageCode>,
    pub plan: QuestGlyphCacheUploadPocPlan,
}

/// page·code·stream 또는 원본 VDD 계약 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheUploadPocError(String);

impl fmt::Display for QuestGlyphCacheUploadPocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestGlyphCacheUploadPocError {}

fn error(message: impl Into<String>) -> QuestGlyphCacheUploadPocError {
    QuestGlyphCacheUploadPocError(message.into())
}

fn sha256(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn is_hangul(character: char) -> bool {
    matches!(character, '\u{ac00}'..='\u{d7a3}')
}

pub(super) fn map_catalog_page_to_codes(
    repertoire: &[char],
    catalog_indices: &[u16],
    codes: &[[u8; 2]],
) -> Result<Vec<QuestGlyphCachePageCode>, QuestGlyphCacheUploadPocError> {
    if catalog_indices.is_empty() {
        return Err(error("QUEST sparse upload page assignment가 비어 있음"));
    }
    let mut seen_catalog = BTreeSet::new();
    let mut rows = Vec::with_capacity(catalog_indices.len());
    for (slot_offset, catalog_index) in catalog_indices.iter().copied().enumerate() {
        if !seen_catalog.insert(catalog_index) {
            return Err(error(format!(
                "QUEST sparse upload page catalog index 중복: {catalog_index}"
            )));
        }
        let glyph = *repertoire.get(usize::from(catalog_index)).ok_or_else(|| {
            error(format!(
                "QUEST sparse upload catalog index 범위 이탈: {catalog_index}"
            ))
        })?;
        if glyph.is_control() {
            return Err(error(format!(
                "QUEST sparse upload catalog가 제어 문자임: {glyph:?}"
            )));
        }
        let slot_index = QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX
            .checked_add(slot_offset)
            .ok_or_else(|| error("QUEST sparse upload slot index overflow"))?;
        let slot =
            QuestGlyphSlot::for_index(slot_index).map_err(|source| error(source.to_string()))?;
        if slot.width != QUEST_FONT_WIDE_CELL_WIDTH || slot.x % 4 != 0 {
            return Err(error(format!(
                "QUEST sparse upload slot이 wide halfword 경계가 아님: {slot:?}"
            )));
        }
        let code = *codes.get(slot_index).ok_or_else(|| {
            error(format!(
                "QUEST sparse upload code table slot 누락: index={slot_index}"
            ))
        })?;
        if !matches!(parse_quest_token(&code, 0), Ok(QuestToken::Glyph(raw)) if raw == code) {
            return Err(error(format!(
                "QUEST sparse upload slot code가 glyph token이 아님: index={slot_index}, code={code:02X?}"
            )));
        }
        rows.push(QuestGlyphCachePageCode {
            glyph,
            catalog_index,
            slot_index,
            code,
        });
    }
    Ok(rows)
}

pub(super) fn code_map_for_text(
    text: &str,
    code_table: &QuestCodeTable,
    page_codes: &[QuestGlyphCachePageCode],
) -> Result<BTreeMap<char, [u8; 2]>, QuestGlyphCacheUploadPocError> {
    let page_by_glyph = page_codes
        .iter()
        .map(|row| (row.glyph, row.code))
        .collect::<BTreeMap<_, _>>();
    let mut result = BTreeMap::new();
    for character in text.chars().filter(|character| *character != '\n') {
        let code = if is_hangul(character) {
            page_by_glyph.get(&character).copied().ok_or_else(|| {
                error(format!(
                    "QUEST sparse upload page에 PoC 한글이 없음: {character}"
                ))
            })?
        } else {
            encode_quest_poc_character(character).map_err(|source| error(source.to_string()))?
        };
        if code_table.index_of(code).is_none() {
            return Err(error(format!(
                "QUEST sparse upload PoC code가 원본 code table에 없음: {character}={code:02X?}"
            )));
        }
        match result.insert(character, code) {
            Some(existing) if existing != code => {
                return Err(error(format!(
                    "QUEST sparse upload PoC 문자 code가 일관되지 않음: {character}"
                )));
            }
            _ => {}
        }
    }
    Ok(result)
}

/// package page 62와 기존 PoC 문장을 결합해 TOWN00 VDD 교체물을 준비한다.
pub(super) fn prepare_quest_glyph_cache_upload_poc(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
    routing: &QuestTranslationRoutingAnalysis,
    package: &QuestGlyphCachePackage,
) -> Result<PreparedQuestGlyphCacheUploadPoc, QuestGlyphCacheUploadPocError> {
    let route = routing
        .routes
        .iter()
        .find(|route| route.stream_id == QUEST_GLYPH_CACHE_UPLOAD_POC_STREAM_ID)
        .ok_or_else(|| error("QUEST sparse upload 대상 route가 없음"))?;
    if route.batch_id != QUEST_GLYPH_CACHE_UPLOAD_POC_BATCH_ID
        || route.page_id != QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID
        || route.record_offset != QUEST_GLYPH_CACHE_UPLOAD_POC_RECORD_OFFSET
        || route.placement != QuestTranslationPlacement::InlineCapacityCandidate
        || route.target_word_offset != 0
        || route.relocated_record_offset.is_some()
        || route.routed_record != [0x21, QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID, 0, 0]
        || route.layout_approved
        || route.build_eligible
    {
        return Err(error("QUEST sparse upload 대상 route 계약 drift"));
    }

    let repertoire = batches.draft_hangul_repertoire.chars().collect::<Vec<_>>();
    if repertoire.iter().copied().collect::<BTreeSet<_>>().len() != repertoire.len()
        || !repertoire.windows(2).all(|pair| pair[0] < pair[1])
        || repertoire.iter().any(|glyph| !is_hangul(*glyph))
        || repertoire.len() != package.plan.glyphs
    {
        return Err(error(
            "QUEST sparse upload provisional repertoire 순서 drift",
        ));
    }
    let catalog_indices = package
        .page_catalog_indices(route.page_id)
        .map_err(|source| error(source.to_string()))?;

    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let code_table = quest_code_table(&iso).map_err(|source| error(source.to_string()))?;
    let page_codes = map_catalog_page_to_codes(&repertoire, &catalog_indices, code_table.codes())?;
    let mut prepared = prepare_quest_poc_assets(source, QuestPocMode::Control)
        .map_err(|source| error(source.to_string()))?;
    let approved =
        approved_quest_poc_translation(&prepared.new_town_plain, QUEST_DEFEAT_DIALOGUE_ID)
            .map_err(|source| error(source.to_string()))?;
    let code_by_character = code_map_for_text(approved.text, &code_table, &page_codes)?;

    let original_town = prepared.new_town_plain.clone();
    prepared.new_town_plain = translate_town00_with_codes(&original_town, &code_by_character)
        .map_err(|source| error(source.to_string()))?;
    let record_end = route
        .record_offset
        .checked_add(route.protected_record.len())
        .ok_or_else(|| error("QUEST sparse upload record 끝 overflow"))?;
    let record = prepared
        .new_town_plain
        .get_mut(route.record_offset..record_end)
        .ok_or_else(|| error("QUEST sparse upload record가 TOWN00 범위 밖임"))?;
    if record != route.protected_record {
        return Err(error("QUEST sparse upload OP21 원본 record 불일치"));
    }
    record.copy_from_slice(&route.routed_record);
    rebuild_prepared_town00(&mut prepared).map_err(|source| error(source.to_string()))?;

    let source_stream = original_town
        .get(route.record_offset..route.end_offset)
        .ok_or_else(|| error("QUEST sparse upload 원본 stream 범위 이탈"))?;
    let replacement_stream = prepared
        .new_town_plain
        .get(route.record_offset..route.end_offset)
        .ok_or_else(|| error("QUEST sparse upload 교체 stream 범위 이탈"))?;
    if source_stream.len() != route.source_stream_bytes
        || replacement_stream.len() != source_stream.len()
        || replacement_stream[..4] != route.routed_record
    {
        return Err(error(
            "QUEST sparse upload fixed-slot stream 길이·route 불일치",
        ));
    }
    let poc_glyphs = approved
        .text
        .chars()
        .filter(|character| is_hangul(*character))
        .collect::<BTreeSet<_>>()
        .len();
    let first_slot_index = page_codes
        .first()
        .map(|row| row.slot_index)
        .ok_or_else(|| error("QUEST sparse upload page code가 비어 있음"))?;
    let last_slot_index = page_codes
        .last()
        .map(|row| row.slot_index)
        .ok_or_else(|| error("QUEST sparse upload page code가 비어 있음"))?;

    Ok(PreparedQuestGlyphCacheUploadPoc {
        plan: QuestGlyphCacheUploadPocPlan {
            schema_version: QUEST_GLYPH_CACHE_UPLOAD_POC_SCHEMA_VERSION,
            stream_id: QUEST_GLYPH_CACHE_UPLOAD_POC_STREAM_ID,
            batch_id: QUEST_GLYPH_CACHE_UPLOAD_POC_BATCH_ID,
            page_id: route.page_id,
            routed_record: route.routed_record,
            page_glyphs: page_codes.len(),
            first_slot_index,
            last_slot_index,
            poc_glyphs,
            source_stream_bytes: source_stream.len(),
            source_stream_sha256: sha256(source_stream),
            replacement_stream_sha256: sha256(replacement_stream),
            town00_cnx_bytes: prepared.new_town_cnx.len(),
            page_assignment_exact: true,
            stream_roundtrip_exact: true,
            poc_translation_approved: true,
            runtime_verified: false,
            release_eligible: false,
        },
        prepared,
        page_codes,
    })
}

/// 첫 `poc-approved` 문장과 page 62 active upload를 단일 Expected Write로 만든다.
pub fn build_quest_glyph_cache_upload_poc(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
    routing: &QuestTranslationRoutingAnalysis,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
) -> Result<QuestGlyphCacheUploadPocBuild, QuestGlyphCacheUploadPocError> {
    let prepared = prepare_quest_glyph_cache_upload_poc(source, batches, routing, package)?;
    if prepared.page_codes.len() != prepared.plan.page_glyphs
        || prepared.plan.runtime_verified
        || prepared.plan.release_eligible
    {
        return Err(error("QUEST sparse upload 준비물 판정 drift"));
    }
    let archive_patch = DiscArchiveEntryPatch {
        iso_path: "/QUEST/RPG_SEQ.VDD;1",
        entry_name: "TOWN00.SEQ",
        original_vdd: &prepared.prepared.seq_vdd,
        replacement_vdd: &prepared.prepared.new_seq_vdd,
        replacement_cnx_bytes: prepared.prepared.new_town_cnx.len(),
        id: "quest-glyph-cache-upload-town00",
        actor: "pipeline::quest_glyph_cache_upload_poc",
    };
    let disc = build_quest_glyph_cache_disc(
        source,
        package,
        hook,
        QUEST_GLYPH_CACHE_UPLOAD_DISC_POC_SCHEMA_VERSION,
        &[archive_patch],
        &[],
        &[],
    )
    .map_err(|source| error(source.to_string()))?;
    if disc.report.schema_version != QUEST_GLYPH_CACHE_UPLOAD_DISC_POC_SCHEMA_VERSION
        || disc.report.runtime_verified
        || disc.report.build_eligible
    {
        return Err(error("QUEST sparse upload disc 판정 drift"));
    }

    Ok(QuestGlyphCacheUploadPocBuild {
        image: disc.image,
        report: QuestGlyphCacheUploadPocReport {
            schema_version: QUEST_GLYPH_CACHE_UPLOAD_DISC_POC_SCHEMA_VERSION,
            input: prepared.plan,
            disc: disc.report,
            seq_vdd_exact: true,
            town00_plain_exact: true,
            original_font_archive_unchanged: true,
            runtime_verified: false,
            release_eligible: false,
        },
    })
}

/// exact active-upload disc의 결정적 검토 행.
pub fn render_quest_glyph_cache_upload_poc_tsv(report: &QuestGlyphCacheUploadPocReport) -> String {
    let input = &report.input;
    let disc = &report.disc;
    format!(
        "schema_version\tstream_id\tbatch_id\tpage_id\trouted_record\tpage_glyphs\t\
         first_slot_index\tlast_slot_index\tpoc_glyphs\tsource_stream_bytes\t\
         source_stream_sha256\treplacement_stream_sha256\ttown00_cnx_bytes\t\
         image_sha256\tmachine_code_writes\tdata_writes\tmetadata_writes\t\
         relocation_copy_writes\ttouched_sectors\tfinal_expected_writes\t\
         page_assignment_exact\tstream_roundtrip_exact\tseq_vdd_exact\t\
         town00_plain_exact\toriginal_font_archive_unchanged\tpoc_translation_approved\t\
         runtime_verified\trelease_eligible\n\
         {}\t{}\t{}\t{}\t{:02x}{:02x}{:02x}{:02x}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        input.stream_id,
        input.batch_id,
        input.page_id,
        input.routed_record[0],
        input.routed_record[1],
        input.routed_record[2],
        input.routed_record[3],
        input.page_glyphs,
        input.first_slot_index,
        input.last_slot_index,
        input.poc_glyphs,
        input.source_stream_bytes,
        input.source_stream_sha256,
        input.replacement_stream_sha256,
        input.town00_cnx_bytes,
        disc.image_sha256,
        disc.machine_code_writes,
        disc.data_writes,
        disc.metadata_writes,
        disc.relocation_copy_writes,
        disc.touched_sectors,
        disc.final_expected_writes,
        input.page_assignment_exact,
        input.stream_roundtrip_exact,
        report.seq_vdd_exact,
        report.town00_plain_exact,
        report.original_font_archive_unchanged,
        input.poc_translation_approved,
        report.runtime_verified,
        report.release_eligible,
    )
}

/// 지원 원본과 현재 번역/package에서 page 62 active-upload PoC를 검증한다.
pub fn verify_box_quest_glyph_cache_upload_poc(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
    routing: &QuestTranslationRoutingAnalysis,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
) -> Result<QuestGlyphCacheUploadPocBuild, QuestGlyphCacheUploadPocError> {
    let build = build_quest_glyph_cache_upload_poc(source, batches, routing, package, hook)?;
    let input = &build.report.input;
    let disc = &build.report.disc;
    let route = routing
        .routes
        .iter()
        .find(|route| route.stream_id == QUEST_GLYPH_CACHE_UPLOAD_POC_STREAM_ID)
        .ok_or_else(|| error("QUEST sparse upload 대상 route가 없음"))?;
    let page_glyphs = package
        .page_catalog_indices(QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID)
        .map_err(|source| error(source.to_string()))?
        .len();
    let expected_final_writes = disc
        .machine_code_writes
        .checked_add(disc.data_writes)
        .and_then(|count| count.checked_add(disc.metadata_writes))
        .and_then(|count| count.checked_add(disc.relocation_copy_writes))
        .and_then(|count| count.checked_add(disc.touched_sectors))
        .ok_or_else(|| error("QUEST sparse upload Expected Write 수 overflow"))?;
    if input.schema_version != QUEST_GLYPH_CACHE_UPLOAD_POC_SCHEMA_VERSION
        || build.report.schema_version != QUEST_GLYPH_CACHE_UPLOAD_DISC_POC_SCHEMA_VERSION
        || input.stream_id != QUEST_GLYPH_CACHE_UPLOAD_POC_STREAM_ID
        || input.batch_id != QUEST_GLYPH_CACHE_UPLOAD_POC_BATCH_ID
        || input.page_id != QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID
        || input.routed_record != [0x21, QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID, 0, 0]
        || input.page_glyphs != page_glyphs
        || input.first_slot_index != QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX
        || input.last_slot_index + 1 != input.first_slot_index + input.page_glyphs
        || input.poc_glyphs == 0
        || input.poc_glyphs > input.page_glyphs
        || input.source_stream_bytes != route.source_stream_bytes
        || input.town00_cnx_bytes == 0
        || !is_sha256_hex(&input.source_stream_sha256)
        || !is_sha256_hex(&input.replacement_stream_sha256)
        || input.source_stream_sha256 == input.replacement_stream_sha256
        || disc.package_sha256 != package.sha256
        || disc.package_bytes != package.bytes.len()
        || disc.site_runtime != hook.site.base_addr()
        || disc.body_runtime != hook.body_runtime
        || disc.body_bytes != hook.body.bytes().len()
        || disc.relocated_files != 0
        || disc.relocated_file_bytes != 0
        || disc.relocated_file_sectors != 0
        || disc.final_expected_writes != expected_final_writes
        || disc.image_sha256 != sha256(&build.image)
        || !input.page_assignment_exact
        || !input.stream_roundtrip_exact
        || !input.poc_translation_approved
        || !build.report.seq_vdd_exact
        || !build.report.town00_plain_exact
        || !build.report.original_font_archive_unchanged
        || build.report.runtime_verified
        || build.report.release_eligible
    {
        return Err(error("QUEST sparse upload 현재 입력 파생 판정 drift"));
    }
    Ok(build)
}

#[cfg(test)]
#[path = "quest_glyph_cache_upload_poc_tests.rs"]
mod tests;
