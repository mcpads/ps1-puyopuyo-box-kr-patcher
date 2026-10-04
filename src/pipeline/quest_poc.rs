//! QUEST 첫 패배 대사 한 문장 PoC.
//!
//! 저장된 CNX → VDD → ISO → Mode2/2352 경계를 모두 Rust로 다시 만들며,
//! 원문의 세 glyph 구간만 같은 길이로 교체한다. `Control`은 해제 내용은
//! 그대로 두고 CNX만 재압축해 실제 소비 호환성 C의 선행 표본을 만든다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::btx::{decode, encode};
use crate::compress::{compress, decompress, is_cnx, parse_header};
use crate::coords::exe_map;
use crate::disc::{apply_form1_write_plan, DiscImage, Iso9660, PsxExe, VerifiedSourceImage};
use crate::font::{
    bundled_maplestory_light, quest_poc_code, rasterize_wide_glyph, verify_quest_poc_font_plan,
    QuestCodeTable, QuestGlyphSlot, QuestPocFontPlanReport, QUEST_CODE_TABLE_BYTES,
    QUEST_CODE_TABLE_RUNTIME, QUEST_FONT_LAYOUT, QUEST_FONT_SHA256, QUEST_POC_GLYPH_MAPPINGS,
    QUEST_POC_RASTER_PROFILE,
};
use crate::patch::WritePlan;
use crate::script::{
    approved_quest_poc_translation, parse_quest_token, parse_quest_tokens,
    verify_town00_seq_roundtrip, QuestToken, QUEST_DEFEAT_DIALOGUE_ID, TOWN00_SEQ_RUNTIME_BASE,
    TOWN00_SEQ_SHA256,
};
use crate::text::encode_sjis;

use super::vdd_plan::{add_vdd_entry_replacement, logical_file};
use super::{QuestSecondDialogueAssets, QUEST_PAGE_SUPPLY_PLAN_TARGET_RECORD_OFFSET};

const QUEST_EXE_PATH: &str = "/QUEST/QUEST.EXE;1";
const RPG_SEQ_PATH: &str = "/QUEST/RPG_SEQ.VDD;1";
const RPG_GRP_PATH: &str = "/QUEST/RPG_GRP.VDD;1";
const TOWN00_NAME: &str = "TOWN00.SEQ";
const RPG_FONT_NAME: &str = "RPG_FNT1.BTX";

/// CNX 무변경 재압축 control 또는 한글 한 문장 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestPocMode {
    Control,
    Korean,
}

impl QuestPocMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Korean => "ko",
        }
    }
}

/// 한 문장 빌드의 정적 검증 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPocReport {
    pub mode: QuestPocMode,
    pub town00_cnx_before: usize,
    pub town00_cnx_after: usize,
    pub font_cnx_before: usize,
    pub font_cnx_after: usize,
    pub font_plan: QuestPocFontPlanReport,
    pub data_writes: usize,
    pub final_expected_writes: usize,
    pub touched_sectors: usize,
    pub image_sha256: String,
}

/// 완전히 검증된 raw BIN.
#[derive(Debug)]
pub struct QuestPocBuild {
    pub image: Vec<u8>,
    pub report: QuestPocReport,
}

/// QUEST 한 문장 PoC의 VDD 교체물과 resident font page 준비물.
///
/// 최종 디스크 쓰기 전 단계만 소유한다. 다른 pipeline은 이 값을 다시
/// 파일로 저장해 입력으로 삼지 않고, 같은 불변 원본의 단일 WritePlan에
/// 직접 합성해야 한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedQuestPocAssets {
    pub mode: QuestPocMode,
    pub seq_vdd: Vec<u8>,
    pub new_seq_vdd: Vec<u8>,
    pub new_town_plain: Vec<u8>,
    pub new_town_cnx: Vec<u8>,
    pub grp_vdd: Vec<u8>,
    pub new_grp_vdd: Vec<u8>,
    pub original_font_plain: Vec<u8>,
    pub new_font_plain: Vec<u8>,
    pub new_font_cnx: Vec<u8>,
    pub font_plan: QuestPocFontPlanReport,
    pub town00_cnx_before: usize,
    pub font_cnx_before: usize,
}

/// 한 문장 파이프라인 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPocError(String);

impl fmt::Display for QuestPocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestPocError {}

fn error(message: impl Into<String>) -> QuestPocError {
    QuestPocError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn require_digest(label: &str, raw: &[u8], expected: &str) -> Result<(), QuestPocError> {
    let actual = digest(raw);
    if actual != expected {
        return Err(error(format!(
            "{label} SHA-256 불일치: expected={expected}, actual={actual}"
        )));
    }
    Ok(())
}

pub(super) fn quest_code_table(iso: &Iso9660<'_>) -> Result<QuestCodeTable, QuestPocError> {
    let raw = iso
        .read_file(QUEST_EXE_PATH)
        .map_err(|source| error(source.to_string()))?;
    let exe = PsxExe::parse(&raw).map_err(|source| error(source.to_string()))?;
    let map = exe_map("QUEST.EXE").ok_or_else(|| error("QUEST.EXE 사상이 없음"))?;
    exe.verify_map(map)
        .map_err(|source| error(source.to_string()))?;
    let offset = QUEST_CODE_TABLE_RUNTIME
        .checked_sub(map.load_addr)
        .ok_or_else(|| error("QUEST 코드표가 EXE 적재 주소보다 작음"))? as usize;
    let end = offset
        .checked_add(QUEST_CODE_TABLE_BYTES)
        .ok_or_else(|| error("QUEST 코드표 범위 overflow"))?;
    let table = exe
        .text()
        .get(offset..end)
        .ok_or_else(|| error("QUEST 코드표가 EXE text 범위 밖임"))?;
    QuestCodeTable::parse(table).map_err(|source| error(source.to_string()))
}

fn verify_mapping_candidates(
    code_table: &QuestCodeTable,
    seq_archive: &VddArchive,
    town00: &[u8],
) -> Result<QuestPocFontPlanReport, QuestPocError> {
    let mut raw_pairs = BTreeSet::new();
    for entry in seq_archive.entries() {
        let raw = seq_archive.read_file(entry);
        if !is_cnx(raw) {
            return Err(error(format!(
                "{RPG_SEQ_PATH}:{}가 CNX v2가 아님",
                entry.name()
            )));
        }
        let plain = decompress(raw).map_err(|source| {
            error(format!(
                "{RPG_SEQ_PATH}:{} 해제 실패: {source}",
                entry.name()
            ))
        })?;
        raw_pairs.extend(plain.windows(2).map(|pair| [pair[0], pair[1]]));
    }
    let approved = approved_quest_poc_translation(town00, QUEST_DEFEAT_DIALOGUE_ID)
        .map_err(|source| error(source.to_string()))?;
    let required_glyphs = approved
        .text
        .chars()
        .filter(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
        .collect();
    verify_quest_poc_font_plan(code_table, &raw_pairs, &required_glyphs)
        .map_err(|source| error(source.to_string()))
}

pub(super) fn encode_quest_poc_character(character: char) -> Result<[u8; 2], QuestPocError> {
    if let Some(code) = quest_poc_code(character) {
        return Ok(code);
    }
    match character {
        '「' => Ok([0x81, 0x75]),
        '〜' | '~' => Ok([0x81, 0x60]),
        '！' | '!' => Ok([0x81, 0x49]),
        '。' | '.' => Ok([0x81, 0x42]),
        '　' => Ok([0x81, 0x40]),
        _ => Err(error(format!(
            "QUEST 한 문장 인코딩 mapping 누락: {character}"
        ))),
    }
}

fn encode_characters_with_codes(
    characters: &[char],
    code_by_character: &BTreeMap<char, [u8; 2]>,
) -> Result<Vec<u8>, QuestPocError> {
    let mut output = Vec::with_capacity(characters.len() * 2);
    for &character in characters {
        let code = code_by_character.get(&character).ok_or_else(|| {
            error(format!(
                "QUEST 한 문장 page mapping 누락: {character} (U+{:04X})",
                character as u32
            ))
        })?;
        if !matches!(parse_quest_token(code, 0), Ok(QuestToken::Glyph(raw)) if raw == *code) {
            return Err(error(format!(
                "QUEST 한 문장 page code가 단일 glyph가 아님: {character}={code:02X?}"
            )));
        }
        output.extend_from_slice(code);
    }
    Ok(output)
}

fn replace_exact(
    raw: &mut [u8],
    offset: usize,
    expected: &[u8],
    replacement: &[u8],
    label: &str,
) -> Result<(), QuestPocError> {
    if expected.len() != replacement.len() {
        return Err(error(format!(
            "{label}: 원문과 번역 encoding 길이 불일치: {} != {}",
            expected.len(),
            replacement.len()
        )));
    }
    let end = offset
        .checked_add(expected.len())
        .ok_or_else(|| error(format!("{label}: 범위 overflow")))?;
    let target = raw
        .get_mut(offset..end)
        .ok_or_else(|| error(format!("{label}: TOWN00 범위 이탈")))?;
    if target != expected {
        return Err(error(format!(
            "{label}: TOWN00 원본 바이트 불일치 at 0x{offset:X}"
        )));
    }
    target.copy_from_slice(replacement);
    Ok(())
}

pub(super) fn translate_town00_with_codes(
    original: &[u8],
    code_by_character: &BTreeMap<char, [u8; 2]>,
) -> Result<Vec<u8>, QuestPocError> {
    let approved = approved_quest_poc_translation(original, QUEST_DEFEAT_DIALOGUE_ID)
        .map_err(|source| error(source.to_string()))?;
    let lines: Vec<_> = approved.text.lines().collect();
    if lines.len() != 2 {
        return Err(error(format!(
            "한 문장 번역 입력은 정확히 2줄이어야 함: actual={}",
            lines.len()
        )));
    }
    let first: Vec<char> = lines[0].chars().collect();
    let second: Vec<char> = lines[1].chars().collect();
    if first.len() != 9 || second.len() != 13 {
        return Err(error(format!(
            "한 문장 glyph 수 불일치: first={}, second={}",
            first.len(),
            second.len()
        )));
    }

    let speaker = encode_characters_with_codes(&first[..4], code_by_character)?;
    let body1 = encode_characters_with_codes(&first[4..], code_by_character)?;
    let body2 = encode_characters_with_codes(&second, code_by_character)?;
    let replacements = [speaker, body1, body2];
    if approved.spans.len() != replacements.len() {
        return Err(error(format!(
            "승인 대사 span 수 불일치: expected={}, actual={}",
            replacements.len(),
            approved.spans.len()
        )));
    }
    let mut output = original.to_vec();
    for (index, (span, replacement)) in approved.spans.iter().zip(replacements.iter()).enumerate() {
        let expected = encode_sjis(span.protected_source)
            .map_err(|source| error(format!("승인 대사 span {index} 인코딩 실패: {source}")))?;
        replace_exact(
            &mut output,
            span.offset,
            &expected,
            replacement,
            &format!("승인 대사 span {index}"),
        )?;
    }

    let sample = &output[0x21E4..0x2280];
    let tokens = parse_quest_tokens(sample, TOWN00_SEQ_RUNTIME_BASE + 0x21E4)
        .map_err(|source| error(source.to_string()))?;
    let serialized: Vec<_> = tokens
        .iter()
        .flat_map(|token| token.raw().iter().copied())
        .collect();
    if serialized != sample {
        return Err(error("한글 대사 token 왕복 불일치"));
    }
    let glyphs = tokens
        .iter()
        .filter(|token| matches!(token, QuestToken::Glyph(_)))
        .count();
    if glyphs != 48 {
        return Err(error(format!(
            "한글 대사 표본 glyph 경계 변화: expected=48, actual={glyphs}"
        )));
    }
    Ok(output)
}

fn translate_town00(original: &[u8]) -> Result<Vec<u8>, QuestPocError> {
    let approved = approved_quest_poc_translation(original, QUEST_DEFEAT_DIALOGUE_ID)
        .map_err(|source| error(source.to_string()))?;
    let code_by_character = approved
        .text
        .chars()
        .filter(|character| *character != '\n')
        .map(|character| encode_quest_poc_character(character).map(|code| (character, code)))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    translate_town00_with_codes(original, &code_by_character)
}

fn patch_font(original: &[u8]) -> Result<Vec<u8>, QuestPocError> {
    let mut pixels =
        decode(original, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;
    let font = bundled_maplestory_light().map_err(|source| error(source.to_string()))?;
    for mapping in QUEST_POC_GLYPH_MAPPINGS {
        let slot =
            QuestGlyphSlot::for_index(mapping.index).map_err(|source| error(source.to_string()))?;
        let glyph = rasterize_wide_glyph(&font, mapping.glyph, QUEST_POC_RASTER_PROFILE)
            .map_err(|source| error(source.to_string()))?;
        for row in 0..slot.height {
            let source_start = row * slot.width;
            let target_start = (slot.y + row) * QUEST_FONT_LAYOUT.width + slot.x;
            pixels[target_start..target_start + slot.width]
                .copy_from_slice(&glyph[source_start..source_start + slot.width]);
        }
    }
    let encoded = encode(&pixels, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;
    if decode(&encoded, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))? != pixels {
        return Err(error("한글 RPG_FNT1.BTX encode readback 불일치"));
    }
    Ok(encoded)
}

fn verify_readback_asset(
    iso: &Iso9660<'_>,
    archive_path: &str,
    entry_name: &str,
    expected_vdd: &[u8],
    expected_plain: &[u8],
) -> Result<(), QuestPocError> {
    let vdd = iso
        .read_file(archive_path)
        .map_err(|source| error(source.to_string()))?;
    if vdd != expected_vdd {
        return Err(error(format!("{archive_path}: 최종 VDD readback 불일치")));
    }
    let archive = VddArchive::parse(&vdd).map_err(|source| error(source.to_string()))?;
    let entry = archive
        .find(entry_name)
        .ok_or_else(|| error(format!("{archive_path}:{entry_name} readback 없음")))?;
    let plain = decompress(archive.read_file(entry)).map_err(|source| error(source.to_string()))?;
    if plain != expected_plain {
        return Err(error(format!(
            "{archive_path}:{entry_name} 최종 해제 readback 불일치"
        )));
    }
    Ok(())
}

/// 지원 원본과 version 관리 입력에서 한 문장 자산 교체물을 준비한다.
pub(super) fn prepare_quest_poc_assets(
    source: &VerifiedSourceImage,
    mode: QuestPocMode,
) -> Result<PreparedQuestPocAssets, QuestPocError> {
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let code_table = quest_code_table(&iso)?;

    let seq_vdd = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|source| error(source.to_string()))?;
    let seq_archive = VddArchive::parse(&seq_vdd).map_err(|source| error(source.to_string()))?;
    let town_entry = seq_archive
        .find(TOWN00_NAME)
        .ok_or_else(|| error("RPG_SEQ.VDD에 TOWN00.SEQ가 없음"))?;
    let town_cnx = seq_archive.read_file(town_entry);
    let town_header = parse_header(town_cnx).map_err(|source| error(source.to_string()))?;
    if town_header.subtype != *b"bin" {
        return Err(error("TOWN00.SEQ CNX subtype이 bin이 아님"));
    }
    let town_plain = decompress(town_cnx).map_err(|source| error(source.to_string()))?;
    require_digest("TOWN00.SEQ", &town_plain, TOWN00_SEQ_SHA256)?;
    verify_town00_seq_roundtrip(&town_plain).map_err(|source| error(source.to_string()))?;
    let font_plan = verify_mapping_candidates(&code_table, &seq_archive, &town_plain)?;

    let grp_vdd = iso
        .read_file(RPG_GRP_PATH)
        .map_err(|source| error(source.to_string()))?;
    let grp_archive = VddArchive::parse(&grp_vdd).map_err(|source| error(source.to_string()))?;
    let font_entry = grp_archive
        .find(RPG_FONT_NAME)
        .ok_or_else(|| error("RPG_GRP.VDD에 RPG_FNT1.BTX가 없음"))?;
    let font_cnx = grp_archive.read_file(font_entry);
    let font_header = parse_header(font_cnx).map_err(|source| error(source.to_string()))?;
    if font_header.subtype != *b"bi_" {
        return Err(error("RPG_FNT1.BTX CNX subtype이 bi_가 아님"));
    }
    let font_plain = decompress(font_cnx).map_err(|source| error(source.to_string()))?;
    require_digest("RPG_FNT1.BTX", &font_plain, QUEST_FONT_SHA256)?;
    decode(&font_plain, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;

    let (new_town_plain, new_font_plain) = match mode {
        QuestPocMode::Control => (town_plain.clone(), font_plain.clone()),
        QuestPocMode::Korean => (translate_town00(&town_plain)?, patch_font(&font_plain)?),
    };
    let new_town_cnx = compress(&new_town_plain, town_header.subtype)
        .map_err(|source| error(source.to_string()))?;
    let new_font_cnx = compress(&new_font_plain, font_header.subtype)
        .map_err(|source| error(source.to_string()))?;
    if decompress(&new_town_cnx).map_err(|source| error(source.to_string()))? != new_town_plain {
        return Err(error("TOWN00.SEQ 재압축 자기 readback 불일치"));
    }
    if decompress(&new_font_cnx).map_err(|source| error(source.to_string()))? != new_font_plain {
        return Err(error("RPG_FNT1.BTX 재압축 자기 readback 불일치"));
    }

    let new_seq_vdd = seq_archive
        .replace_in_place(&BTreeMap::from([(
            TOWN00_NAME.to_string(),
            new_town_cnx.clone(),
        )]))
        .map_err(|source| error(source.to_string()))?;
    let new_grp_vdd = grp_archive
        .replace_in_place(&BTreeMap::from([(
            RPG_FONT_NAME.to_string(),
            new_font_cnx.clone(),
        )]))
        .map_err(|source| error(source.to_string()))?;

    Ok(PreparedQuestPocAssets {
        mode,
        seq_vdd,
        new_seq_vdd,
        new_town_plain,
        new_town_cnx,
        grp_vdd,
        new_grp_vdd,
        original_font_plain: font_plain,
        new_font_plain,
        new_font_cnx,
        font_plan,
        town00_cnx_before: town_cnx.len(),
        font_cnx_before: font_cnx.len(),
    })
}

/// `new_town_plain` 변경을 검산된 CNX와 고정 크기 VDD로 다시 결속한다.
pub(super) fn rebuild_prepared_town00(
    prepared: &mut PreparedQuestPocAssets,
) -> Result<(), QuestPocError> {
    let seq_archive =
        VddArchive::parse(&prepared.seq_vdd).map_err(|source| error(source.to_string()))?;
    let town_entry = seq_archive
        .find(TOWN00_NAME)
        .ok_or_else(|| error("RPG_SEQ.VDD에 TOWN00.SEQ가 없음"))?;
    let town_header = parse_header(seq_archive.read_file(town_entry))
        .map_err(|source| error(source.to_string()))?;
    let new_town_cnx = compress(&prepared.new_town_plain, town_header.subtype)
        .map_err(|source| error(source.to_string()))?;
    if decompress(&new_town_cnx).map_err(|source| error(source.to_string()))?
        != prepared.new_town_plain
    {
        return Err(error("QUEST TOWN00 CNX 자기 readback 불일치"));
    }
    prepared.new_seq_vdd = seq_archive
        .replace_in_place(&BTreeMap::from([(
            TOWN00_NAME.to_string(),
            new_town_cnx.clone(),
        )]))
        .map_err(|source| error(source.to_string()))?;
    prepared.new_town_cnx = new_town_cnx;
    Ok(())
}

/// 첫 대사 PoC 준비물에 같은 batch의 두 번째 fixed-slot stream과 공용 page를 합친다.
pub(super) fn prepare_quest_two_dialogue_assets(
    source: &VerifiedSourceImage,
    second: &QuestSecondDialogueAssets,
) -> Result<PreparedQuestPocAssets, QuestPocError> {
    if second.plan.translation_approved
        || second.plan.layout_approved
        || second.plan.runtime_verified
        || second.plan.build_eligible
    {
        return Err(error(
            "QUEST 두 대사 PoC 입력의 승인 경계가 비배포 계획과 다름",
        ));
    }
    let mut prepared = prepare_quest_poc_assets(source, QuestPocMode::Korean)?;
    let record_offset = usize::try_from(QUEST_PAGE_SUPPLY_PLAN_TARGET_RECORD_OFFSET)
        .map_err(|_| error("QUEST 두 번째 대사 record offset 변환 실패"))?;
    let record_end = record_offset
        .checked_add(second.original_stream.len())
        .ok_or_else(|| error("QUEST 두 번째 대사 stream 범위 overflow"))?;
    let target = prepared
        .new_town_plain
        .get_mut(record_offset..record_end)
        .ok_or_else(|| error("QUEST 두 번째 대사 stream이 TOWN00 범위 밖임"))?;
    if target != second.original_stream {
        return Err(error(
            "QUEST 두 번째 대사 Expected Write 원본 stream 불일치",
        ));
    }
    if second.replacement_stream.len() != second.original_stream.len() {
        return Err(error("QUEST 두 번째 대사 fixed-slot 길이 불일치"));
    }
    target.copy_from_slice(&second.replacement_stream);

    rebuild_prepared_town00(&mut prepared)?;

    if second.page.len() != prepared.original_font_plain.len() {
        return Err(error(format!(
            "QUEST 두 대사 공용 page 길이 불일치: expected={}, actual={}",
            prepared.original_font_plain.len(),
            second.page.len()
        )));
    }
    let grp_archive =
        VddArchive::parse(&prepared.grp_vdd).map_err(|source| error(source.to_string()))?;
    let font_entry = grp_archive
        .find(RPG_FONT_NAME)
        .ok_or_else(|| error("RPG_GRP.VDD에 RPG_FNT1.BTX가 없음"))?;
    let font_header = parse_header(grp_archive.read_file(font_entry))
        .map_err(|source| error(source.to_string()))?;
    let new_font_cnx =
        compress(&second.page, font_header.subtype).map_err(|source| error(source.to_string()))?;
    if decompress(&new_font_cnx).map_err(|source| error(source.to_string()))? != second.page {
        return Err(error("QUEST 두 대사 공용 page CNX 자기 readback 불일치"));
    }
    prepared.new_grp_vdd = grp_archive
        .replace_in_place(&BTreeMap::from([(
            RPG_FONT_NAME.to_string(),
            new_font_cnx.clone(),
        )]))
        .map_err(|source| error(source.to_string()))?;
    prepared.new_font_plain.clone_from(&second.page);
    prepared.new_font_cnx = new_font_cnx;
    Ok(prepared)
}

/// 지원 원본에서 control 또는 한글 한 문장 raw BIN을 만든다.
pub fn build_quest_first_dialogue(
    source: &VerifiedSourceImage,
    mode: QuestPocMode,
) -> Result<QuestPocBuild, QuestPocError> {
    let prepared = prepare_quest_poc_assets(source, mode)?;
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let seq_entry = iso
        .find(RPG_SEQ_PATH)
        .map_err(|source| error(source.to_string()))?
        .clone();
    let seq_file = logical_file(&seq_entry).map_err(error)?;
    let seq_archive =
        VddArchive::parse(&prepared.seq_vdd).map_err(|source| error(source.to_string()))?;
    let grp_entry = iso
        .find(RPG_GRP_PATH)
        .map_err(|source| error(source.to_string()))?
        .clone();
    let grp_file = logical_file(&grp_entry).map_err(error)?;
    let grp_archive =
        VddArchive::parse(&prepared.grp_vdd).map_err(|source| error(source.to_string()))?;

    let mut data_plan = WritePlan::new();
    add_vdd_entry_replacement(
        &mut data_plan,
        seq_file,
        &seq_archive,
        &prepared.seq_vdd,
        &prepared.new_seq_vdd,
        TOWN00_NAME,
        prepared.new_town_cnx.len(),
        "quest-town00",
        "pipeline::quest_poc",
    )
    .map_err(error)?;
    add_vdd_entry_replacement(
        &mut data_plan,
        grp_file,
        &grp_archive,
        &prepared.grp_vdd,
        &prepared.new_grp_vdd,
        RPG_FONT_NAME,
        prepared.new_font_cnx.len(),
        "quest-font",
        "pipeline::quest_poc",
    )
    .map_err(error)?;
    let data_writes = data_plan.writes.len();
    let touched_sectors = data_plan.touched_sectors().len();
    let final_expected_writes = data_writes + touched_sectors;
    let output = apply_form1_write_plan(source.image(), &data_plan)
        .map_err(|source| error(source.to_string()))?;

    let final_image = DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
    verify_readback_asset(
        &final_iso,
        RPG_SEQ_PATH,
        TOWN00_NAME,
        &prepared.new_seq_vdd,
        &prepared.new_town_plain,
    )?;
    verify_readback_asset(
        &final_iso,
        RPG_GRP_PATH,
        RPG_FONT_NAME,
        &prepared.new_grp_vdd,
        &prepared.new_font_plain,
    )?;
    let output = final_image.into_bytes();
    let image_sha256 = digest(&output);

    Ok(QuestPocBuild {
        image: output,
        report: QuestPocReport {
            mode: prepared.mode,
            town00_cnx_before: prepared.town00_cnx_before,
            town00_cnx_after: prepared.new_town_cnx.len(),
            font_cnx_before: prepared.font_cnx_before,
            font_cnx_after: prepared.new_font_cnx.len(),
            font_plan: prepared.font_plan,
            data_writes,
            final_expected_writes,
            touched_sectors,
            image_sha256,
        },
    })
}

#[cfg(test)]
#[path = "quest_poc_tests.rs"]
mod tests;
