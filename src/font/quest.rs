//! QUEST 전용 `RPG_FNT1.BTX` layout과 코드표.
//!
//! `QUEST.EXE`의 `0x8002BA40` 소비 루틴은 `0x800B26E4`의 u16 코드표를
//! 0x159개 검색하고, 찾은 index로 12바이트 레코드를 고른다. runtime
//! 레코드 표본과 아틀라스 경계를 함께 만족하는 배치는 첫 19개만 6×16,
//! 나머지 326개는 12×16이다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use fontdue::Font;

use crate::btx::{Layout, PixelFormat};
use crate::font::MAPLESTORY_LIGHT_FONT_ID;
use crate::text::decode_sjis;

/// QUEST 문자 코드표의 runtime 주소.
pub const QUEST_CODE_TABLE_RUNTIME: u32 = 0x800B_26E4;

/// QUEST 문자 코드표의 u16 엔트리 수.
pub const QUEST_CODE_TABLE_COUNT: usize = 0x159;

/// QUEST 문자 코드표의 원시 바이트 수.
pub const QUEST_CODE_TABLE_BYTES: usize = QUEST_CODE_TABLE_COUNT * 2;

pub const QUEST_FONT_ASSET_NAME: &str = "RPG_FNT1.BTX";

pub const QUEST_FONT_SHA256: &str =
    "98151af59e76b8be63bb99d4a7c0c3ec96615ffdf1090ab511dc1dabf8ae0a4c";

pub const QUEST_POC_FONT_PLAN_SCHEMA_VERSION: &str = "quest-poc-font-plan-v1";

/// `RPG_FNT1.BTX`의 소비 layout.
pub const QUEST_FONT_LAYOUT: Layout = Layout {
    width: 256,
    height: 256,
    format: PixelFormat::Indexed4,
};

pub const QUEST_FONT_NARROW_SLOTS: usize = 19;
pub const QUEST_FONT_WIDE_SLOTS: usize = 326;
pub const QUEST_FONT_FIRST_ROW_WIDE_SLOTS: usize = 11;
pub const QUEST_FONT_WIDE_COLUMNS: usize = 21;
pub const QUEST_FONT_NARROW_CELL_WIDTH: usize = 6;
pub const QUEST_FONT_WIDE_CELL_WIDTH: usize = 12;
pub const QUEST_FONT_CELL_HEIGHT: usize = 16;

/// 원본 코드표에서 `!!` 모양을 한 cell로 공급하는 전용 코드.
pub const QUEST_DOUBLE_EXCLAMATION_CODE: [u8; 2] = [0x21, 0x21];

/// 원본 코드표에서 `??` 모양을 한 cell로 공급하는 전용 코드.
pub const QUEST_DOUBLE_QUESTION_CODE: [u8; 2] = [0x3F, 0x3F];

/// `!!` 한 cell을 사람 검토 원장에서 나타내는 단일 의미 문자.
pub const QUEST_DOUBLE_EXCLAMATION_REVIEW_GLYPH: char = '‼';

/// `??` 한 cell을 사람 검토 원장에서 나타내는 단일 의미 문자.
pub const QUEST_DOUBLE_QUESTION_REVIEW_GLYPH: char = '⁇';

/// QUEST 코드표의 2바이트 code를 한 cell의 검토용 의미 문자로 바꾼다.
///
/// `21 21`과 `3F 3F`는 ASCII 두 글자가 아니라 각각 `!!`, `??` 모양의
/// 단일 cell이다. 검토 원장에서는 token과 cell의 1:1 관계를 보존하기 위해
/// Unicode `‼`와 `⁇`로 표기한다. 원본 raw code는 별도로 보존한다.
pub fn decode_quest_source_glyph(code: [u8; 2]) -> Option<char> {
    match code {
        QUEST_DOUBLE_EXCLAMATION_CODE => Some(QUEST_DOUBLE_EXCLAMATION_REVIEW_GLYPH),
        QUEST_DOUBLE_QUESTION_CODE => Some(QUEST_DOUBLE_QUESTION_REVIEW_GLYPH),
        _ => {
            let decoded = decode_sjis(&code).ok()?;
            let mut characters = decoded.chars();
            let character = characters.next()?;
            characters.next().is_none().then_some(character)
        }
    }
}

/// Shift-JIS decoder 표기 차이를 QUEST에서 같은 glyph 의미로 정규화한다.
///
/// `0x8160`은 U+FF5E `～`로 decode되지만 번역 입력의 U+301C `〜`와
/// 같은 원본 atlas cell을 가리킨다.
pub fn canonicalize_quest_source_glyph(character: char) -> char {
    match character {
        '～' => '〜',
        _ => character,
    }
}

/// QUEST 검토 표기를 게임의 단일 cell 표시 문자로 바꾸는 동안의 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDisplayTextError(String);

impl fmt::Display for QuestDisplayTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestDisplayTextError {}

/// 검토 원장의 ASCII 문장부호를 QUEST 표시 glyph로 정규화한다.
/// 숫자는 번역 글꼴로 그릴 반각과 명시적인 원본 전각을 구별한다.
///
/// 개행과 같은 control 문자는 SEQ control로 별도 작성해야 하므로
/// 표시 문자열에서 거부한다.
pub fn canonicalize_quest_display_glyph(character: char) -> Result<char, QuestDisplayTextError> {
    if character.is_control() {
        return Err(QuestDisplayTextError(format!(
            "QUEST 선택문에 stream control로 표현되지 않은 제어 문자가 있음: {character:?}"
        )));
    }
    let normalized = match character {
        ' ' => '　',
        '!' => '！',
        '(' => '（',
        ')' => '）',
        ',' => '、',
        '.' => '。',
        '/' => '／',
        ':' => '：',
        '?' => '？',
        '~' => '〜',
        '·' => '・',
        _ => character,
    };
    Ok(canonicalize_quest_source_glyph(normalized))
}

pub fn normalize_quest_display_text(text: &str) -> Result<String, QuestDisplayTextError> {
    text.chars().map(canonicalize_quest_display_glyph).collect()
}

/// 한 문장 PoC에서 검증할 Maplestory Light raster profile.
pub const QUEST_POC_RASTER_PROFILE: QuestRasterProfile = QuestRasterProfile {
    id: "quest-dialogue-12x16",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 12.0,
    baseline: 13,
    effect: QuestRasterEffect::None,
};

/// 지원 원본 SEQ 전체에서 raw pair로 관측되지 않은 코드 하한.
pub const QUEST_POC_UNOBSERVED_CODE_COUNT: usize = 26;

/// 위 하한 중 12×16 wide slot인 임시 PoC 후보.
pub const QUEST_POC_WIDE_CANDIDATE_COUNT: usize = 24;

/// 한 문장 PoC의 한글과 임시 코드표 slot 연결.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestPocGlyphMapping {
    pub glyph: char,
    pub index: usize,
    pub code: [u8; 2],
}

/// 현재 사람 승인 문장에 필요한 한글 11자의 고정 plan.
pub const QUEST_POC_GLYPH_MAPPINGS: [QuestPocGlyphMapping; 11] = [
    QuestPocGlyphMapping {
        glyph: '드',
        index: 237,
        code: [0x90, 0xCF],
    },
    QuestPocGlyphMapping {
        glyph: '라',
        index: 242,
        code: [0x93, 0xBB],
    },
    QuestPocGlyphMapping {
        glyph: '코',
        index: 248,
        code: [0x8C, 0x88],
    },
    QuestPocGlyphMapping {
        glyph: '해',
        index: 258,
        code: [0x97, 0xF1],
    },
    QuestPocGlyphMapping {
        glyph: '냈',
        index: 260,
        code: [0x92, 0x69],
    },
    QuestPocGlyphMapping {
        glyph: '다',
        index: 261,
        code: [0x91, 0x97],
    },
    QuestPocGlyphMapping {
        glyph: '내',
        index: 269,
        code: [0x95, 0xDC],
    },
    QuestPocGlyphMapping {
        glyph: '가',
        index: 271,
        code: [0x8A, 0xD4],
    },
    QuestPocGlyphMapping {
        glyph: '이',
        index: 305,
        code: [0x8F, 0x49],
    },
    QuestPocGlyphMapping {
        glyph: '겼',
        index: 310,
        code: [0x93, 0xA6],
    },
    QuestPocGlyphMapping {
        glyph: '어',
        index: 328,
        code: [0x8C, 0xE3],
    },
];

/// QUEST 코드표 또는 layout 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestFontError {
    CodeTableLength {
        expected: usize,
        actual: usize,
    },
    DuplicateCode {
        code: [u8; 2],
        first: usize,
        second: usize,
    },
    SlotIndexOutOfRange(usize),
    EmptyGlyph(char),
    GlyphDoesNotFit {
        glyph: char,
        x: i32,
        y: i32,
        width: usize,
        height: usize,
    },
    PocPlan(String),
    PocCapacityDrift {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for QuestFontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CodeTableLength { expected, actual } => write!(
                f,
                "QUEST 코드표 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::DuplicateCode {
                code,
                first,
                second,
            } => write!(
                f,
                "QUEST 코드표 중복: {:02X} {:02X}, index {first} / {second}",
                code[0], code[1]
            ),
            Self::SlotIndexOutOfRange(index) => {
                write!(f, "QUEST 글리프 slot index 범위 초과: {index}")
            }
            Self::EmptyGlyph(glyph) => {
                write!(f, "Maplestory Light가 빈 글리프를 반환: {glyph}")
            }
            Self::GlyphDoesNotFit {
                glyph,
                x,
                y,
                width,
                height,
            } => write!(
                f,
                "QUEST 12x16 cell에 글리프가 들어가지 않음: {glyph}, \
                origin=({x},{y}), bitmap={width}x{height}"
            ),
            Self::PocPlan(message) => write!(f, "QUEST PoC font plan 오류: {message}"),
            Self::PocCapacityDrift {
                field,
                expected,
                actual,
            } => write!(
                f,
                "QUEST PoC font 후보 수 drift: {field}, expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for QuestFontError {}

/// QUEST 12×16 cell용 글리프 raster 설정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestRasterEffect {
    None,
}

impl QuestRasterEffect {
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
        }
    }
}

/// QUEST 12×16 cell용 글리프 raster 설정.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuestRasterProfile {
    pub id: &'static str,
    pub font_id: &'static str,
    pub font_size: f32,
    pub baseline: i32,
    pub effect: QuestRasterEffect,
}

/// Maplestory Light 한 글자를 QUEST의 4bpp palette index로 만든다.
///
/// coverage는 0–15로 선형 양자화한다. cell 밖으로 잘라내지 않고, 한 픽셀
/// 이라도 넘으면 profile 오류로 실패한다.
pub fn rasterize_wide_glyph(
    font: &Font,
    glyph: char,
    profile: QuestRasterProfile,
) -> Result<Vec<u8>, QuestFontError> {
    if font.lookup_glyph_index(glyph) == 0 {
        return Err(QuestFontError::EmptyGlyph(glyph));
    }
    let (metrics, bitmap) = font.rasterize(glyph, profile.font_size);
    if metrics.width == 0 || metrics.height == 0 || bitmap.iter().all(|value| *value == 0) {
        return Err(QuestFontError::EmptyGlyph(glyph));
    }
    let x = (QUEST_FONT_WIDE_CELL_WIDTH as i32 - metrics.width as i32) / 2;
    let y = profile.baseline - metrics.ymin - metrics.height as i32;
    if x < 0
        || y < 0
        || x + metrics.width as i32 > QUEST_FONT_WIDE_CELL_WIDTH as i32
        || y + metrics.height as i32 > QUEST_FONT_CELL_HEIGHT as i32
    {
        return Err(QuestFontError::GlyphDoesNotFit {
            glyph,
            x,
            y,
            width: metrics.width,
            height: metrics.height,
        });
    }

    let mut pixels = vec![0u8; QUEST_FONT_WIDE_CELL_WIDTH * QUEST_FONT_CELL_HEIGHT];
    for row in 0..metrics.height {
        for column in 0..metrics.width {
            let coverage = u16::from(bitmap[row * metrics.width + column]);
            let palette = match profile.effect {
                QuestRasterEffect::None => ((coverage * 15 + 127) / 255) as u8,
            };
            let target = (y as usize + row) * QUEST_FONT_WIDE_CELL_WIDTH + x as usize + column;
            pixels[target] = palette;
        }
    }
    Ok(pixels)
}

/// 한 코드표 index가 가리키는 아틀라스 사각형.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestGlyphSlot {
    pub index: usize,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl QuestGlyphSlot {
    /// runtime 레코드와 일치하는 index별 아틀라스 사각형.
    pub fn for_index(index: usize) -> Result<Self, QuestFontError> {
        if index >= QUEST_CODE_TABLE_COUNT {
            return Err(QuestFontError::SlotIndexOutOfRange(index));
        }
        let (x, y, width) = if index < QUEST_FONT_NARROW_SLOTS {
            (
                index * QUEST_FONT_NARROW_CELL_WIDTH,
                0,
                QUEST_FONT_NARROW_CELL_WIDTH,
            )
        } else if index < QUEST_FONT_NARROW_SLOTS + QUEST_FONT_FIRST_ROW_WIDE_SLOTS {
            (
                QUEST_FONT_NARROW_SLOTS * QUEST_FONT_NARROW_CELL_WIDTH
                    + (index - QUEST_FONT_NARROW_SLOTS) * QUEST_FONT_WIDE_CELL_WIDTH,
                0,
                QUEST_FONT_WIDE_CELL_WIDTH,
            )
        } else {
            let wide_index = index - QUEST_FONT_NARROW_SLOTS - QUEST_FONT_FIRST_ROW_WIDE_SLOTS;
            (
                (wide_index % QUEST_FONT_WIDE_COLUMNS) * QUEST_FONT_WIDE_CELL_WIDTH,
                (1 + wide_index / QUEST_FONT_WIDE_COLUMNS) * QUEST_FONT_CELL_HEIGHT,
                QUEST_FONT_WIDE_CELL_WIDTH,
            )
        };
        Ok(Self {
            index,
            x,
            y,
            width,
            height: QUEST_FONT_CELL_HEIGHT,
        })
    }
}

/// `QUEST.EXE`에서 읽은 345개 Shift-JIS 계열 코드.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestCodeTable {
    codes: Vec<[u8; 2]>,
}

impl QuestCodeTable {
    /// 코드표 원시 바이트를 파싱하며 중복을 거부한다.
    pub fn parse(raw: &[u8]) -> Result<Self, QuestFontError> {
        if raw.len() != QUEST_CODE_TABLE_BYTES {
            return Err(QuestFontError::CodeTableLength {
                expected: QUEST_CODE_TABLE_BYTES,
                actual: raw.len(),
            });
        }
        let codes: Vec<[u8; 2]> = raw.chunks_exact(2).map(|pair| [pair[0], pair[1]]).collect();
        for (second, code) in codes.iter().enumerate() {
            if let Some(first) = codes[..second].iter().position(|other| other == code) {
                return Err(QuestFontError::DuplicateCode {
                    code: *code,
                    first,
                    second,
                });
            }
        }
        Ok(Self { codes })
    }

    pub fn codes(&self) -> &[[u8; 2]] {
        &self.codes
    }

    /// 저장 바이트 두 개를 코드표 index로 바꾼다.
    pub fn index_of(&self, code: [u8; 2]) -> Option<usize> {
        self.codes.iter().position(|candidate| *candidate == code)
    }

    pub fn code_at(&self, index: usize) -> Option<[u8; 2]> {
        self.codes.get(index).copied()
    }

    pub fn slot_for_code(&self, code: [u8; 2]) -> Option<QuestGlyphSlot> {
        self.index_of(code)
            .and_then(|index| QuestGlyphSlot::for_index(index).ok())
    }
}

/// 원본 SEQ 부재 하한과 현재 승인 glyph plan의 검증 요약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestPocFontPlanReport {
    pub source_unobserved_codes: usize,
    pub provisional_wide_slots: usize,
    pub assigned_glyphs: usize,
    pub provisional_remaining: usize,
}

/// review row에서 구분하는 임시 후보의 사용 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestPocFontCandidateStatus {
    PocAssigned,
    ProvisionalUnobserved,
}

impl QuestPocFontCandidateStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PocAssigned => "poc-assigned",
            Self::ProvisionalUnobserved => "provisional-unobserved",
        }
    }
}

/// 지원 원본에서 raw pair가 미관측인 wide slot 한 행.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestPocFontCandidate {
    pub index: usize,
    pub code: [u8; 2],
    pub slot: QuestGlyphSlot,
    pub status: QuestPocFontCandidateStatus,
    pub glyph: Option<char>,
}

fn verify_quest_poc_font_plan_from(
    code_table: &QuestCodeTable,
    observed_pairs: &BTreeSet<[u8; 2]>,
    required_glyphs: &BTreeSet<char>,
    mappings: &[QuestPocGlyphMapping],
) -> Result<QuestPocFontPlanReport, QuestFontError> {
    let source_unobserved_codes = code_table
        .codes()
        .iter()
        .filter(|code| !observed_pairs.contains(*code))
        .count();
    if source_unobserved_codes != QUEST_POC_UNOBSERVED_CODE_COUNT {
        return Err(QuestFontError::PocCapacityDrift {
            field: "source-unobserved-codes",
            expected: QUEST_POC_UNOBSERVED_CODE_COUNT,
            actual: source_unobserved_codes,
        });
    }
    let provisional_wide_slots = code_table
        .codes()
        .iter()
        .enumerate()
        .filter(|(index, code)| {
            !observed_pairs.contains(*code)
                && QuestGlyphSlot::for_index(*index)
                    .is_ok_and(|slot| slot.width == QUEST_FONT_WIDE_CELL_WIDTH)
        })
        .count();
    if provisional_wide_slots != QUEST_POC_WIDE_CANDIDATE_COUNT {
        return Err(QuestFontError::PocCapacityDrift {
            field: "provisional-wide-slots",
            expected: QUEST_POC_WIDE_CANDIDATE_COUNT,
            actual: provisional_wide_slots,
        });
    }

    let mut glyphs = BTreeSet::new();
    let mut indices = BTreeSet::new();
    let mut codes = BTreeSet::new();
    for mapping in mappings {
        if !glyphs.insert(mapping.glyph) {
            return Err(QuestFontError::PocPlan(format!(
                "glyph 중복: {}",
                mapping.glyph
            )));
        }
        if !indices.insert(mapping.index) {
            return Err(QuestFontError::PocPlan(format!(
                "index 중복: {}",
                mapping.index
            )));
        }
        if !codes.insert(mapping.code) {
            return Err(QuestFontError::PocPlan(format!(
                "code 중복: {:02X} {:02X}",
                mapping.code[0], mapping.code[1]
            )));
        }
        if code_table.code_at(mapping.index) != Some(mapping.code) {
            return Err(QuestFontError::PocPlan(format!(
                "index {} 원본 code 불일치: expected={:02X} {:02X}",
                mapping.index, mapping.code[0], mapping.code[1]
            )));
        }
        if observed_pairs.contains(&mapping.code) {
            return Err(QuestFontError::PocPlan(format!(
                "SEQ에서 관측된 code를 재매핑함: index {}, {:02X} {:02X}",
                mapping.index, mapping.code[0], mapping.code[1]
            )));
        }
        let slot = QuestGlyphSlot::for_index(mapping.index)?;
        if slot.width != QUEST_FONT_WIDE_CELL_WIDTH || slot.height != QUEST_FONT_CELL_HEIGHT {
            return Err(QuestFontError::PocPlan(format!(
                "wide slot이 아님: index {}",
                mapping.index
            )));
        }
    }
    if &glyphs != required_glyphs {
        return Err(QuestFontError::PocPlan(format!(
            "승인 번역 glyph와 mapping 불일치: required={required_glyphs:?}, mapped={glyphs:?}"
        )));
    }

    Ok(QuestPocFontPlanReport {
        source_unobserved_codes,
        provisional_wide_slots,
        assigned_glyphs: mappings.len(),
        provisional_remaining: provisional_wide_slots
            .checked_sub(mappings.len())
            .ok_or_else(|| QuestFontError::PocPlan("후보보다 mapping이 많음".to_string()))?,
    })
}

/// 승인 번역의 한글 집합과 지원 원본의 임시 font plan을 함께 검증한다.
pub fn verify_quest_poc_font_plan(
    code_table: &QuestCodeTable,
    observed_pairs: &BTreeSet<[u8; 2]>,
    required_glyphs: &BTreeSet<char>,
) -> Result<QuestPocFontPlanReport, QuestFontError> {
    verify_quest_poc_font_plan_from(
        code_table,
        observed_pairs,
        required_glyphs,
        &QUEST_POC_GLYPH_MAPPINGS,
    )
}

/// 검증된 24개 임시 wide 후보를 code table index 순서로 반환한다.
pub fn quest_poc_font_candidates(
    code_table: &QuestCodeTable,
    observed_pairs: &BTreeSet<[u8; 2]>,
    required_glyphs: &BTreeSet<char>,
) -> Result<Vec<QuestPocFontCandidate>, QuestFontError> {
    verify_quest_poc_font_plan(code_table, observed_pairs, required_glyphs)?;
    code_table
        .codes()
        .iter()
        .copied()
        .enumerate()
        .filter(|(index, code)| {
            !observed_pairs.contains(code)
                && QuestGlyphSlot::for_index(*index)
                    .is_ok_and(|slot| slot.width == QUEST_FONT_WIDE_CELL_WIDTH)
        })
        .map(|(index, code)| {
            let slot = QuestGlyphSlot::for_index(index)?;
            let mapping = QUEST_POC_GLYPH_MAPPINGS
                .iter()
                .find(|mapping| mapping.index == index);
            let (status, glyph) = match mapping {
                Some(mapping) => (
                    QuestPocFontCandidateStatus::PocAssigned,
                    Some(mapping.glyph),
                ),
                None => (QuestPocFontCandidateStatus::ProvisionalUnobserved, None),
            };
            Ok(QuestPocFontCandidate {
                index,
                code,
                slot,
                status,
                glyph,
            })
        })
        .collect()
}

/// 임시 후보 24개를 사람이 검토할 수 있는 결정적 TSV로 만든다.
///
/// 이 파생물은 제품 입력이 아니며 `provisional-unobserved`는 release용
/// 빈 slot이나 번역 가능 수용량을 뜻하지 않는다.
pub fn render_quest_poc_font_plan_tsv(
    code_table: &QuestCodeTable,
    observed_pairs: &BTreeSet<[u8; 2]>,
    required_glyphs: &BTreeSet<char>,
) -> Result<String, QuestFontError> {
    let candidates = quest_poc_font_candidates(code_table, observed_pairs, required_glyphs)?;
    let mut output = String::from(
        "schema_version\tasset_name\tasset_sha256\tcode_table_runtime\t\
         profile_id\tfont_id\tfont_size\tbaseline\teffect\tindex\tcode\t\
         slot_x\tslot_y\tslot_width\tslot_height\tstatus\tglyph\n",
    );
    for candidate in candidates {
        output.push_str(&format!(
            "{}\t{}\t{}\t0x{:08X}\t{}\t{}\t{:.1}\t{}\t{}\t{}\t\
             {:02X} {:02X}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            QUEST_POC_FONT_PLAN_SCHEMA_VERSION,
            QUEST_FONT_ASSET_NAME,
            QUEST_FONT_SHA256,
            QUEST_CODE_TABLE_RUNTIME,
            QUEST_POC_RASTER_PROFILE.id,
            QUEST_POC_RASTER_PROFILE.font_id,
            QUEST_POC_RASTER_PROFILE.font_size,
            QUEST_POC_RASTER_PROFILE.baseline,
            QUEST_POC_RASTER_PROFILE.effect.label(),
            candidate.index,
            candidate.code[0],
            candidate.code[1],
            candidate.slot.x,
            candidate.slot.y,
            candidate.slot.width,
            candidate.slot.height,
            candidate.status.label(),
            candidate
                .glyph
                .map(|glyph| glyph.to_string())
                .unwrap_or_default(),
        ));
    }
    Ok(output)
}

/// 현재 PoC plan에서 한글 한 글자의 2바이트 code를 찾는다.
pub fn quest_poc_code(glyph: char) -> Option<[u8; 2]> {
    QUEST_POC_GLYPH_MAPPINGS
        .iter()
        .find(|mapping| mapping.glyph == glyph)
        .map(|mapping| mapping.code)
}

#[cfg(test)]
#[path = "quest_tests.rs"]
mod tests;
