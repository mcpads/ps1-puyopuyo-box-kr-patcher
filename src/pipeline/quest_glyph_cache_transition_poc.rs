//! QUEST sparse glyph cache의 서로 다른 Korean page 전환용 비배포 입력.
//!
//! 이미 runtime에서 연속 소비를 확인한 `TOWN00:0x21CC → 0x22DC →
//! 0x362C`를 page `62 → 62 → 50`으로 만든다. Korean page로 route한
//! OP21의 일부 glyph만 바꾸면 같은 record의 일본어 continuation이 덮어쓴
//! slot을 소비하므로 세 record의 모든 glyph를 재인코딩한다. 번역 초안을
//! 새로 복제하지 않고 batch 원장의 draft를 원본 control 경계에 맞춰 고정
//! 길이로 넣는다. 사람 번역·layout 승인과 runtime 판정은 계속 별도다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::disc::{Iso9660, VerifiedSourceImage};
use crate::font::QuestCodeTable;
use crate::script::{
    parse_quest_tokens, quest_control_meaning, QuestControlMeaning, QuestToken,
    QuestTranslationBatchAnalysis, QuestTranslationBatchRole, QuestTranslationWorkStatus,
    TOWN00_SEQ_RUNTIME_BASE,
};

use super::disc_archive_patch::DiscArchiveEntryPatch;
use super::quest_glyph_cache_disc::{build_quest_glyph_cache_disc, QuestGlyphCacheDiscReport};
use super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_glyph_cache_upload_poc::{
    code_map_for_text, map_catalog_page_to_codes, QuestGlyphCachePageCode,
    QUEST_GLYPH_CACHE_UPLOAD_POC_BATCH_ID, QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID,
    QUEST_GLYPH_CACHE_UPLOAD_POC_RECORD_OFFSET, QUEST_GLYPH_CACHE_UPLOAD_POC_STREAM_ID,
};
use super::quest_poc::{
    prepare_quest_poc_assets, quest_code_table, rebuild_prepared_town00, QuestPocMode,
};
use super::quest_translation_routing::{
    QuestTranslationPlacement, QuestTranslationRoute, QuestTranslationRoutingAnalysis,
};

const RPG_SEQ_PATH: &str = "/QUEST/RPG_SEQ.VDD;1";

pub const QUEST_GLYPH_CACHE_TRANSITION_POC_SCHEMA_VERSION: &str =
    "quest-glyph-cache-transition-poc-v2";
pub const QUEST_GLYPH_CACHE_TRANSITION_DISC_POC_SCHEMA_VERSION: &str =
    "quest-glyph-cache-transition-disc-poc-v2";
pub const QUEST_GLYPH_CACHE_TRANSITION_SECOND_STREAM_ID: &str =
    "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x22DC";
pub const QUEST_GLYPH_CACHE_TRANSITION_SECOND_PAGE_ID: u8 = 62;
pub const QUEST_GLYPH_CACHE_TRANSITION_SECOND_RECORD_OFFSET: usize = 0x22DC;
pub const QUEST_GLYPH_CACHE_TRANSITION_THIRD_STREAM_ID: &str =
    "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x362C";
pub const QUEST_GLYPH_CACHE_TRANSITION_THIRD_BATCH_ID: &str =
    "quest-town00-draco-puyoman-aftermath-001";
pub const QUEST_GLYPH_CACHE_TRANSITION_THIRD_PAGE_ID: u8 = 50;
pub const QUEST_GLYPH_CACHE_TRANSITION_THIRD_RECORD_OFFSET: usize = 0x362C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LineWordLayout {
    words: usize,
    separator: &'static str,
}

const fn spaced(words: usize) -> LineWordLayout {
    LineWordLayout {
        words,
        separator: " ",
    }
}

const FIRST_WORDS_PER_LINE: [LineWordLayout; 6] = [
    spaced(1),
    spaced(2),
    LineWordLayout {
        words: 2,
        separator: "",
    },
    spaced(1),
    spaced(2),
    spaced(3),
];
const SECOND_WORDS_PER_LINE: [LineWordLayout; 4] = [spaced(4), spaced(4), spaced(5), spaced(2)];
const THIRD_WORDS_PER_LINE: [LineWordLayout; 5] =
    [spaced(1), spaced(3), spaced(2), spaced(2), spaced(1)];
const IDEOGRAPHIC_SPACE: char = '　';
const IDEOGRAPHIC_SPACE_CODE: [u8; 2] = [0x81, 0x40];

/// 한 routed stream의 fixed-slot 재삽입 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheTransitionStreamPlan {
    pub stream_id: String,
    pub batch_id: String,
    pub page_id: u8,
    pub record_offset: usize,
    pub routed_record: [u8; 4],
    pub page_glyphs: usize,
    pub first_slot_index: usize,
    pub last_slot_index: usize,
    pub line_capacities: Vec<usize>,
    pub line_used: Vec<usize>,
    pub line_padding: Vec<usize>,
    pub source_stream_bytes: usize,
    pub replacement_stream_bytes: usize,
    pub source_stream_sha256: String,
    pub replacement_stream_sha256: String,
    pub control_raw_sha256: String,
    pub work_status: QuestTranslationWorkStatus,
    pub stream_roundtrip_exact: bool,
    pub translation_approved: bool,
    pub layout_approved: bool,
}

/// page 62 공유 뒤 page 50 전환을 설치한 exact disc 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheTransitionPocReport {
    pub schema_version: &'static str,
    pub streams: Vec<QuestGlyphCacheTransitionStreamPlan>,
    pub disc: QuestGlyphCacheDiscReport,
    pub seq_vdd_exact: bool,
    pub town00_plain_exact: bool,
    pub original_font_archive_unchanged: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheTransitionPocBuild {
    pub image: Vec<u8>,
    pub report: QuestGlyphCacheTransitionPocReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheTransitionPocError(String);

impl fmt::Display for QuestGlyphCacheTransitionPocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestGlyphCacheTransitionPocError {}

fn error(message: impl Into<String>) -> QuestGlyphCacheTransitionPocError {
    QuestGlyphCacheTransitionPocError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn hex(raw: &[u8]) -> String {
    raw.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn split_words(
    draft: &str,
    words_per_line: &[LineWordLayout],
) -> Result<Vec<String>, QuestGlyphCacheTransitionPocError> {
    let words = draft.split(' ').collect::<Vec<_>>();
    if words.iter().any(|word| word.is_empty()) {
        return Err(error("QUEST sparse transition draft 공백 계약 불일치"));
    }
    let expected: usize = words_per_line.iter().map(|layout| layout.words).sum();
    if words.len() != expected {
        return Err(error(format!(
            "QUEST sparse transition draft 단어 수 불일치: expected={expected}, actual={}",
            words.len()
        )));
    }
    let mut cursor = 0usize;
    words_per_line
        .iter()
        .map(|layout| {
            let end = cursor
                .checked_add(layout.words)
                .ok_or_else(|| error("QUEST sparse transition line cursor overflow"))?;
            let line = words
                .get(cursor..end)
                .ok_or_else(|| error("QUEST sparse transition line 범위 이탈"))?
                .join(layout.separator);
            cursor = end;
            Ok(line)
        })
        .collect()
}

fn source_line_capacities(
    tokens: &[QuestToken<'_>],
) -> Result<Vec<usize>, QuestGlyphCacheTransitionPocError> {
    let mut capacities = Vec::new();
    let mut current = 0usize;
    for token in tokens {
        match token {
            QuestToken::Glyph(_) => {
                current = current
                    .checked_add(1)
                    .ok_or_else(|| error("QUEST sparse transition glyph 수 overflow"))?;
            }
            QuestToken::Control { spec, .. }
                if matches!(
                    quest_control_meaning(spec.code),
                    Some(QuestControlMeaning::AdvanceTextLine)
                        | Some(QuestControlMeaning::StartInputWait)
                ) =>
            {
                if current != 0 {
                    capacities.push(current);
                    current = 0;
                }
            }
            QuestToken::Control { .. } => {}
        }
    }
    if current != 0 {
        capacities.push(current);
    }
    Ok(capacities)
}

fn page_codes(
    page_id: u8,
    batches: &QuestTranslationBatchAnalysis,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<Vec<QuestGlyphCachePageCode>, QuestGlyphCacheTransitionPocError> {
    let repertoire = batches.draft_hangul_repertoire.chars().collect::<Vec<_>>();
    let indices = package
        .page_catalog_indices(page_id)
        .map_err(|source| error(source.to_string()))?;
    map_catalog_page_to_codes(&repertoire, &indices, code_table.codes())
        .map_err(|source| error(source.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FixedSlotEncoding {
    codes: Vec<[u8; 2]>,
    used: Vec<usize>,
    padding: Vec<usize>,
}

fn encode_lines(
    lines: &[String],
    capacities: &[usize],
    code_table: &QuestCodeTable,
    page_codes: &[QuestGlyphCachePageCode],
) -> Result<FixedSlotEncoding, QuestGlyphCacheTransitionPocError> {
    if lines.len() != capacities.len() {
        return Err(error("QUEST sparse transition line 수 불일치"));
    }
    let mapping_text = lines
        .iter()
        .flat_map(|line| line.chars())
        .filter(|character| !matches!(*character, ' ' | IDEOGRAPHIC_SPACE))
        .collect::<String>();
    let code_map = code_map_for_text(&mapping_text, code_table, page_codes)
        .map_err(|source| error(source.to_string()))?;
    if code_table.index_of(IDEOGRAPHIC_SPACE_CODE).is_none() {
        return Err(error("QUEST sparse transition 전각 공백 code 누락"));
    }
    let mut encoded = Vec::new();
    let mut used = Vec::new();
    let mut padding = Vec::new();
    for (line, capacity) in lines.iter().zip(capacities.iter().copied()) {
        let line_used = line.chars().count();
        let line_padding = capacity.checked_sub(line_used).ok_or_else(|| {
            error(format!(
                "QUEST sparse transition line 용량 초과: capacity={capacity}, used={line_used}, line={line:?}"
            ))
        })?;
        for character in line
            .chars()
            .chain(std::iter::repeat_n(IDEOGRAPHIC_SPACE, line_padding))
        {
            let code = if matches!(character, ' ' | IDEOGRAPHIC_SPACE) {
                IDEOGRAPHIC_SPACE_CODE
            } else {
                *code_map.get(&character).ok_or_else(|| {
                    error(format!("QUEST sparse transition code 누락: {character:?}"))
                })?
            };
            encoded.push(code);
        }
        used.push(line_used);
        padding.push(line_padding);
    }
    Ok(FixedSlotEncoding {
        codes: encoded,
        used,
        padding,
    })
}

fn replace_stream(
    source_stream: &[u8],
    route: &QuestTranslationRoute,
    tokens: &[QuestToken<'_>],
    encoded: &[[u8; 2]],
) -> Result<(Vec<u8>, Vec<u8>), QuestGlyphCacheTransitionPocError> {
    if source_stream.get(..4) != Some(route.protected_record.as_slice()) {
        return Err(error("QUEST sparse transition 원본 OP21 record 불일치"));
    }
    let mut glyphs = encoded.iter();
    let mut replacement = Vec::with_capacity(source_stream.len());
    let mut controls = Vec::new();
    replacement.extend_from_slice(&route.routed_record);
    for token in tokens {
        match token {
            QuestToken::Glyph(_) => replacement.extend_from_slice(
                glyphs
                    .next()
                    .ok_or_else(|| error("QUEST sparse transition glyph 부족"))?,
            ),
            QuestToken::Control { raw, .. } => {
                replacement.extend_from_slice(raw);
                controls.extend_from_slice(raw);
            }
        }
    }
    if glyphs.next().is_some() || replacement.len() != source_stream.len() {
        return Err(error("QUEST sparse transition fixed-slot 길이 불일치"));
    }
    let stream_runtime = TOWN00_SEQ_RUNTIME_BASE
        .checked_add(
            u32::try_from(route.record_offset + 4)
                .map_err(|_| error("QUEST sparse transition stream runtime offset 범위 이탈"))?,
        )
        .ok_or_else(|| error("QUEST sparse transition stream runtime overflow"))?;
    let parsed = parse_quest_tokens(&replacement[4..], stream_runtime)
        .map_err(|source| error(source.to_string()))?;
    let roundtrip = parsed
        .iter()
        .flat_map(|token| token.raw().iter().copied())
        .collect::<Vec<_>>();
    if roundtrip != replacement[4..] || parsed.len() != tokens.len() {
        return Err(error("QUEST sparse transition token 연속 왕복 불일치"));
    }
    Ok((replacement, controls))
}

#[derive(Debug, Clone, Copy)]
struct StreamPreparationContext<'a> {
    town00: &'a [u8],
    batches: &'a QuestTranslationBatchAnalysis,
    routing: &'a QuestTranslationRoutingAnalysis,
    package: &'a QuestGlyphCachePackage,
    code_table: &'a QuestCodeTable,
}

#[derive(Debug, Clone, Copy)]
struct TransitionStreamSpec<'a> {
    stream_id: &'a str,
    expected_batch_id: &'a str,
    expected_page_id: u8,
    expected_record_offset: usize,
    words_per_line: &'a [LineWordLayout],
}

fn prepare_stream(
    context: StreamPreparationContext<'_>,
    spec: TransitionStreamSpec<'_>,
) -> Result<(Vec<u8>, QuestGlyphCacheTransitionStreamPlan), QuestGlyphCacheTransitionPocError> {
    let route = context
        .routing
        .routes
        .iter()
        .find(|route| route.stream_id == spec.stream_id)
        .ok_or_else(|| {
            error(format!(
                "QUEST sparse transition route 누락: {}",
                spec.stream_id
            ))
        })?;
    let batch = context
        .batches
        .entries
        .iter()
        .find(|entry| entry.stream_id == spec.stream_id)
        .ok_or_else(|| {
            error(format!(
                "QUEST sparse transition batch 누락: {}",
                spec.stream_id
            ))
        })?;
    if route.batch_id != spec.expected_batch_id
        || batch.batch_id != spec.expected_batch_id
        || route.page_id != spec.expected_page_id
        || route.record_offset != spec.expected_record_offset
        || route.placement != QuestTranslationPlacement::InlineCapacityCandidate
        || route.target_word_offset != 0
        || route.relocated_record_offset.is_some()
        || route.routed_record != [0x21, spec.expected_page_id, 0, 0]
        || route.layout_approved
        || route.build_eligible
        || batch.role != QuestTranslationBatchRole::Translate
        || batch.work_status != QuestTranslationWorkStatus::DraftNeedsIndependentReview
        || batch.korean_draft.is_empty()
    {
        return Err(error(format!(
            "QUEST sparse transition route 계약 drift: {stream_id}",
            stream_id = spec.stream_id
        )));
    }
    let source_stream = context
        .town00
        .get(route.record_offset..route.end_offset)
        .ok_or_else(|| error("QUEST sparse transition 원본 stream 범위 이탈"))?;
    let stream_runtime = TOWN00_SEQ_RUNTIME_BASE
        .checked_add(
            u32::try_from(route.record_offset + 4)
                .map_err(|_| error("QUEST sparse transition source runtime offset 범위 이탈"))?,
        )
        .ok_or_else(|| error("QUEST sparse transition source runtime overflow"))?;
    let tokens = parse_quest_tokens(
        source_stream
            .get(4..)
            .ok_or_else(|| error("QUEST sparse transition source record 누락"))?,
        stream_runtime,
    )
    .map_err(|source| error(source.to_string()))?;
    let capacities = source_line_capacities(&tokens)?;
    let source_glyphs = capacities.iter().sum::<usize>();
    if hex(source_stream) != batch.protected_raw_hex
        || source_glyphs != batch.protected_source.chars().count()
    {
        return Err(error(format!(
            "QUEST sparse transition 보호 원문 불일치: {stream_id}",
            stream_id = spec.stream_id
        )));
    }
    let korean_lines = split_words(&batch.korean_draft, spec.words_per_line)?;
    let compact_draft = batch.korean_draft.replace(' ', "");
    let compact_lines = korean_lines.concat().replace(' ', "");
    if compact_lines != compact_draft {
        return Err(error("QUEST sparse transition draft 재결합 불일치"));
    }
    let codes = page_codes(
        spec.expected_page_id,
        context.batches,
        context.package,
        context.code_table,
    )?;
    let encoded = encode_lines(&korean_lines, &capacities, context.code_table, &codes)?;
    let (replacement, controls) = replace_stream(source_stream, route, &tokens, &encoded.codes)?;
    let first_slot_index = codes
        .first()
        .map(|code| code.slot_index)
        .ok_or_else(|| error("QUEST sparse transition page code가 비어 있음"))?;
    let last_slot_index = codes
        .last()
        .map(|code| code.slot_index)
        .ok_or_else(|| error("QUEST sparse transition page code가 비어 있음"))?;
    Ok((
        replacement.clone(),
        QuestGlyphCacheTransitionStreamPlan {
            stream_id: route.stream_id.clone(),
            batch_id: route.batch_id.clone(),
            page_id: route.page_id,
            record_offset: route.record_offset,
            routed_record: route.routed_record,
            page_glyphs: codes.len(),
            first_slot_index,
            last_slot_index,
            line_capacities: capacities,
            line_used: encoded.used,
            line_padding: encoded.padding,
            source_stream_bytes: source_stream.len(),
            replacement_stream_bytes: replacement.len(),
            source_stream_sha256: digest(source_stream),
            replacement_stream_sha256: digest(&replacement),
            control_raw_sha256: digest(&controls),
            work_status: batch.work_status,
            stream_roundtrip_exact: true,
            translation_approved: false,
            layout_approved: false,
        },
    ))
}

/// page 62 두 stream과 page 50 한 stream을 같은 sparse cache build로 만든다.
pub fn build_quest_glyph_cache_transition_poc(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
    routing: &QuestTranslationRoutingAnalysis,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
) -> Result<QuestGlyphCacheTransitionPocBuild, QuestGlyphCacheTransitionPocError> {
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let code_table = quest_code_table(&iso).map_err(|source| error(source.to_string()))?;
    let mut prepared = prepare_quest_poc_assets(source, QuestPocMode::Control)
        .map_err(|source| error(source.to_string()))?;
    let first_route = routing
        .routes
        .iter()
        .find(|route| route.stream_id == QUEST_GLYPH_CACHE_UPLOAD_POC_STREAM_ID)
        .ok_or_else(|| error("QUEST sparse transition first route 누락"))?;
    let (first_stream, first_plan) = prepare_stream(
        StreamPreparationContext {
            town00: &prepared.new_town_plain,
            batches,
            routing,
            package,
            code_table: &code_table,
        },
        TransitionStreamSpec {
            stream_id: QUEST_GLYPH_CACHE_UPLOAD_POC_STREAM_ID,
            expected_batch_id: QUEST_GLYPH_CACHE_UPLOAD_POC_BATCH_ID,
            expected_page_id: QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID,
            expected_record_offset: QUEST_GLYPH_CACHE_UPLOAD_POC_RECORD_OFFSET,
            words_per_line: &FIRST_WORDS_PER_LINE,
        },
    )?;
    prepared.new_town_plain[first_route.record_offset..first_route.end_offset]
        .copy_from_slice(&first_stream);

    let (second_stream, second_plan) = prepare_stream(
        StreamPreparationContext {
            town00: &prepared.new_town_plain,
            batches,
            routing,
            package,
            code_table: &code_table,
        },
        TransitionStreamSpec {
            stream_id: QUEST_GLYPH_CACHE_TRANSITION_SECOND_STREAM_ID,
            expected_batch_id: QUEST_GLYPH_CACHE_UPLOAD_POC_BATCH_ID,
            expected_page_id: QUEST_GLYPH_CACHE_TRANSITION_SECOND_PAGE_ID,
            expected_record_offset: QUEST_GLYPH_CACHE_TRANSITION_SECOND_RECORD_OFFSET,
            words_per_line: &SECOND_WORDS_PER_LINE,
        },
    )?;
    let second_route = routing
        .routes
        .iter()
        .find(|route| route.stream_id == QUEST_GLYPH_CACHE_TRANSITION_SECOND_STREAM_ID)
        .ok_or_else(|| error("QUEST sparse transition second route 재조회 실패"))?;
    prepared.new_town_plain[second_route.record_offset..second_route.end_offset]
        .copy_from_slice(&second_stream);

    let (third_stream, third_plan) = prepare_stream(
        StreamPreparationContext {
            town00: &prepared.new_town_plain,
            batches,
            routing,
            package,
            code_table: &code_table,
        },
        TransitionStreamSpec {
            stream_id: QUEST_GLYPH_CACHE_TRANSITION_THIRD_STREAM_ID,
            expected_batch_id: QUEST_GLYPH_CACHE_TRANSITION_THIRD_BATCH_ID,
            expected_page_id: QUEST_GLYPH_CACHE_TRANSITION_THIRD_PAGE_ID,
            expected_record_offset: QUEST_GLYPH_CACHE_TRANSITION_THIRD_RECORD_OFFSET,
            words_per_line: &THIRD_WORDS_PER_LINE,
        },
    )?;
    let third_route = routing
        .routes
        .iter()
        .find(|route| route.stream_id == QUEST_GLYPH_CACHE_TRANSITION_THIRD_STREAM_ID)
        .ok_or_else(|| error("QUEST sparse transition third route 재조회 실패"))?;
    prepared.new_town_plain[third_route.record_offset..third_route.end_offset]
        .copy_from_slice(&third_stream);
    rebuild_prepared_town00(&mut prepared).map_err(|source| error(source.to_string()))?;

    let archive_patch = DiscArchiveEntryPatch {
        iso_path: RPG_SEQ_PATH,
        entry_name: "TOWN00.SEQ",
        original_vdd: &prepared.seq_vdd,
        replacement_vdd: &prepared.new_seq_vdd,
        replacement_cnx_bytes: prepared.new_town_cnx.len(),
        id: "quest-glyph-cache-transition-town00",
        actor: "pipeline::quest_glyph_cache_transition_poc",
    };
    let disc = build_quest_glyph_cache_disc(
        source,
        package,
        hook,
        QUEST_GLYPH_CACHE_TRANSITION_DISC_POC_SCHEMA_VERSION,
        &[archive_patch],
        &[],
        &[],
    )
    .map_err(|source| error(source.to_string()))?;
    if disc.report.runtime_verified || disc.report.build_eligible {
        return Err(error("QUEST sparse transition disc 판정 drift"));
    }

    Ok(QuestGlyphCacheTransitionPocBuild {
        image: disc.image,
        report: QuestGlyphCacheTransitionPocReport {
            schema_version: QUEST_GLYPH_CACHE_TRANSITION_POC_SCHEMA_VERSION,
            streams: vec![first_plan, second_plan, third_plan],
            disc: disc.report,
            seq_vdd_exact: true,
            town00_plain_exact: true,
            original_font_archive_unchanged: true,
            runtime_verified: false,
            release_eligible: false,
        },
    })
}

fn join_usize(values: &[usize]) -> String {
    values
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// page 전환 PoC의 stream별 고정 길이 계약을 TSV로 만든다.
pub fn render_quest_glyph_cache_transition_poc_tsv(
    report: &QuestGlyphCacheTransitionPocReport,
) -> String {
    let mut out = String::from(
        "schema_version\tstream_id\tbatch_id\tpage_id\trecord_offset\trouted_record\t\
         page_glyphs\tfirst_slot_index\tlast_slot_index\tline_capacities\tline_used\t\
         line_padding\tsource_stream_bytes\treplacement_stream_bytes\t\
         source_stream_sha256\treplacement_stream_sha256\tcontrol_raw_sha256\t\
         work_status\tstream_roundtrip_exact\ttranslation_approved\tlayout_approved\t\
         image_sha256\truntime_verified\trelease_eligible\n",
    );
    for plan in &report.streams {
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t0x{:04X}\t{:02x}{:02x}{:02x}{:02x}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            report.schema_version,
            plan.stream_id,
            plan.batch_id,
            plan.page_id,
            plan.record_offset,
            plan.routed_record[0],
            plan.routed_record[1],
            plan.routed_record[2],
            plan.routed_record[3],
            plan.page_glyphs,
            plan.first_slot_index,
            plan.last_slot_index,
            join_usize(&plan.line_capacities),
            join_usize(&plan.line_used),
            join_usize(&plan.line_padding),
            plan.source_stream_bytes,
            plan.replacement_stream_bytes,
            plan.source_stream_sha256,
            plan.replacement_stream_sha256,
            plan.control_raw_sha256,
            plan.work_status.label(),
            plan.stream_roundtrip_exact,
            plan.translation_approved,
            plan.layout_approved,
            report.disc.image_sha256,
            report.runtime_verified,
            report.release_eligible,
        )
        .expect("String write는 실패하지 않음");
    }
    out
}

/// 지원 원본과 현재 번역/package에서 page 62→62→50 build를 검증한다.
pub fn verify_box_quest_glyph_cache_transition_poc(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
    routing: &QuestTranslationRoutingAnalysis,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
) -> Result<QuestGlyphCacheTransitionPocBuild, QuestGlyphCacheTransitionPocError> {
    let build = build_quest_glyph_cache_transition_poc(source, batches, routing, package, hook)?;
    let [first, second, third] = build.report.streams.as_slice() else {
        return Err(error("QUEST sparse transition stream 수 불일치"));
    };
    let disc = &build.report.disc;
    let expected_streams = [
        (
            QUEST_GLYPH_CACHE_UPLOAD_POC_STREAM_ID,
            QUEST_GLYPH_CACHE_UPLOAD_POC_BATCH_ID,
            QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID,
            QUEST_GLYPH_CACHE_UPLOAD_POC_RECORD_OFFSET,
        ),
        (
            QUEST_GLYPH_CACHE_TRANSITION_SECOND_STREAM_ID,
            QUEST_GLYPH_CACHE_UPLOAD_POC_BATCH_ID,
            QUEST_GLYPH_CACHE_TRANSITION_SECOND_PAGE_ID,
            QUEST_GLYPH_CACHE_TRANSITION_SECOND_RECORD_OFFSET,
        ),
        (
            QUEST_GLYPH_CACHE_TRANSITION_THIRD_STREAM_ID,
            QUEST_GLYPH_CACHE_TRANSITION_THIRD_BATCH_ID,
            QUEST_GLYPH_CACHE_TRANSITION_THIRD_PAGE_ID,
            QUEST_GLYPH_CACHE_TRANSITION_THIRD_RECORD_OFFSET,
        ),
    ];
    for (plan, (stream_id, batch_id, page_id, record_offset)) in
        build.report.streams.iter().zip(expected_streams)
    {
        let route = routing
            .routes
            .iter()
            .find(|route| route.stream_id == stream_id)
            .ok_or_else(|| error(format!("QUEST sparse transition route 누락: {stream_id}")))?;
        let page_glyphs = package
            .page_catalog_indices(page_id)
            .map_err(|source| error(source.to_string()))?
            .len();
        let line_lengths_exact = plan.line_capacities.len() == plan.line_used.len()
            && plan.line_used.len() == plan.line_padding.len()
            && plan
                .line_capacities
                .iter()
                .zip(&plan.line_used)
                .zip(&plan.line_padding)
                .all(|((capacity, used), padding)| *capacity == used + padding);
        if plan.stream_id != stream_id
            || plan.batch_id != batch_id
            || plan.page_id != page_id
            || plan.record_offset != record_offset
            || plan.routed_record != [0x21, page_id, 0, 0]
            || plan.page_glyphs != page_glyphs
            || plan.first_slot_index
                != super::quest_glyph_cache_plan::QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX
            || plan.last_slot_index + 1 != plan.first_slot_index + plan.page_glyphs
            || !line_lengths_exact
            || plan.line_capacities.is_empty()
            || plan.source_stream_bytes != route.source_stream_bytes
            || plan.replacement_stream_bytes != plan.source_stream_bytes
            || !is_sha256_hex(&plan.source_stream_sha256)
            || !is_sha256_hex(&plan.replacement_stream_sha256)
            || !is_sha256_hex(&plan.control_raw_sha256)
            || plan.source_stream_sha256 == plan.replacement_stream_sha256
            || !plan.stream_roundtrip_exact
            || plan.work_status != QuestTranslationWorkStatus::DraftNeedsIndependentReview
            || plan.translation_approved
            || plan.layout_approved
        {
            return Err(error(format!(
                "QUEST sparse transition 현재 stream 파생값 불일치: {plan:?}"
            )));
        }
    }
    let expected_final_writes = disc
        .machine_code_writes
        .checked_add(disc.data_writes)
        .and_then(|count| count.checked_add(disc.metadata_writes))
        .and_then(|count| count.checked_add(disc.relocation_copy_writes))
        .and_then(|count| count.checked_add(disc.touched_sectors))
        .ok_or_else(|| error("QUEST sparse transition Expected Write 수 overflow"))?;
    if build.report.schema_version != QUEST_GLYPH_CACHE_TRANSITION_POC_SCHEMA_VERSION
        || disc.schema_version != QUEST_GLYPH_CACHE_TRANSITION_DISC_POC_SCHEMA_VERSION
        || disc.package_sha256 != package.sha256
        || disc.package_bytes != package.bytes.len()
        || disc.site_runtime != hook.site.base_addr()
        || disc.body_runtime != hook.body_runtime
        || disc.body_bytes != hook.body.bytes().len()
        || disc.final_expected_writes != expected_final_writes
        || disc.image_sha256 != digest(&build.image)
        || !build.report.seq_vdd_exact
        || !build.report.town00_plain_exact
        || !build.report.original_font_archive_unchanged
        || build.report.runtime_verified
        || build.report.release_eligible
    {
        return Err(error(format!(
            "QUEST sparse transition 현재 입력 파생 판정 불일치: first={first:?}, second={second:?}, third={third:?}, disc={disc:?}"
        )));
    }
    let columns = render_quest_glyph_cache_transition_poc_tsv(&build.report)
        .lines()
        .map(|line| line.split('\t').count())
        .collect::<BTreeSet<_>>();
    if columns.len() != 1 {
        return Err(error("QUEST sparse transition TSV 열 수 불일치"));
    }
    Ok(build)
}

#[cfg(test)]
#[path = "quest_glyph_cache_transition_poc_tests.rs"]
mod tests;
