//! QUEST page 전환용 random-access glyph cache의 정적 저장 계획.
//!
//! provisional 한글은 6바이트 pixel row 사전과 13-bit row index로 정확히
//! 저장한다. 비어 있는 행은 16-bit bitmap으로 표시하고 index를 생략한다.
//! cell별 u16 offset으로 가변 길이 record를 찾는다.
//! page별 catalog index는 증가분 ULEB128로 둔다. 두 번째 이후
//! delta 0은 기준 -1로의 재시작이며 다음 양수에서 index를 복원한다.
//! 더 짧을 때는 `80 00` 뒤 absolute u16 배열을 쓴다. 과거 shared-atlas
//! runtime 증거 대조에 필요한 원본 cell은 독립 PackBits side data로만 남기고
//! post-BSS carrier 예산에는 넣지 않는다. 첫 wide 행은 indexed4 halfword
//! 경계에 맞지 않으므로 제외하고, 4-pixel 정렬된 두 번째 행 이후 slot만
//! 사용한다. 이 단계는 저장·무작위 해제 가능성만 판정하며 hook 실행 시간,
//! 승인 layout, page 수명을 대신하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::archive::VddArchive;
use crate::btx::{decode, encode, Layout, PixelFormat};
use crate::compress::decompress;
use crate::disasm::QUEST_PAGE_CARRIER_CAPACITY_BYTES;
use crate::disc::{Iso9660, VerifiedSourceImage};
use crate::font::{
    canonicalize_quest_source_glyph, decode_quest_source_glyph, QuestCodeTable, QuestGlyphSlot,
    QUEST_FONT_CELL_HEIGHT, QUEST_FONT_FIRST_ROW_WIDE_SLOTS, QUEST_FONT_LAYOUT,
    QUEST_FONT_NARROW_SLOTS, QUEST_FONT_WIDE_CELL_WIDTH, QUEST_FONT_WIDE_SLOTS,
};
use crate::script::{QuestTranslationBatchAnalysis, QuestTranslationBatchRole};

use super::quest_capacity_poc::{RPG_FONT_NAME, RPG_GRP_PATH};
use super::quest_poc::quest_code_table;
use super::quest_translation_routing::QuestTranslationRoutingAnalysis;

pub const QUEST_GLYPH_CACHE_PLAN_SCHEMA_VERSION: &str = "quest-glyph-cache-plan-v3";
pub const QUEST_GLYPH_CACHE_CELL_LAYOUT: Layout = Layout {
    width: QUEST_FONT_WIDE_CELL_WIDTH,
    height: QUEST_FONT_CELL_HEIGHT,
    format: PixelFormat::Indexed4,
};
pub const QUEST_GLYPH_CACHE_CELL_BYTES: usize =
    QUEST_FONT_WIDE_CELL_WIDTH * QUEST_FONT_CELL_HEIGHT / 2;
pub const QUEST_GLYPH_CACHE_ROW_BYTES: usize = QUEST_FONT_WIDE_CELL_WIDTH / 2;
pub const QUEST_GLYPH_CACHE_ROWS_PER_CELL: usize = QUEST_FONT_CELL_HEIGHT;
pub const QUEST_GLYPH_CACHE_ROW_INDEX_BITS: usize = 13;
pub const QUEST_GLYPH_CACHE_ROW_INDEX_LIMIT: usize = 1 << QUEST_GLYPH_CACHE_ROW_INDEX_BITS;
pub const QUEST_GLYPH_CACHE_MAX_CELL_ROW_INDEX_BYTES: usize =
    2 + QUEST_GLYPH_CACHE_ROWS_PER_CELL * QUEST_GLYPH_CACHE_ROW_INDEX_BITS / 8;
pub const QUEST_GLYPH_CACHE_CATALOG_OFFSET_BYTES: usize = 2;
pub const QUEST_GLYPH_CACHE_PAGE_OFFSET_BYTES: usize = 2;
pub const QUEST_GLYPH_CACHE_PLAIN_ASSIGNMENT_BYTES: usize = 2;
pub const QUEST_GLYPH_CACHE_SLOT_RECT_BYTES: usize = 4;
pub const QUEST_GLYPH_CACHE_KOREAN_DESCRIPTOR_BYTES: usize = 36;
pub const QUEST_GLYPH_CACHE_SCRATCH_ALIGNMENT: usize = 4;
pub const QUEST_GLYPH_CACHE_SCRATCH_BYTES: usize = QUEST_GLYPH_CACHE_CELL_BYTES;
/// sparse upload가 사용하는 첫 12x16 halfword 정렬 slot의 코드표 index.
pub const QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX: usize =
    QUEST_FONT_NARROW_SLOTS + QUEST_FONT_FIRST_ROW_WIDE_SLOTS;

/// 선택 폰트 글리프를 행 dictionary로 저장한 크기.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestRowDictionaryCatalogSummary {
    pub cells: usize,
    pub plain_bytes: usize,
    pub dictionary_rows: usize,
    pub dictionary_bytes: usize,
    pub row_index_bits: usize,
    pub cell_index_bytes: usize,
    pub stored_bytes: usize,
}

/// 과거 shared-atlas 증거와 대조할 원본 cell의 독립 PackBits catalog 크기.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPackBitsCatalogSummary {
    pub cells: usize,
    pub plain_bytes: usize,
    pub index_bytes: usize,
    pub payload_bytes: usize,
    pub stored_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct QuestEncodedRowDictionaryCatalog {
    pub(super) rows: Vec<[u8; QUEST_GLYPH_CACHE_ROW_BYTES]>,
    pub(super) packed_cell_indices: Vec<u8>,
    pub(super) cells: Vec<Vec<u8>>,
    pub(super) summary: QuestRowDictionaryCatalogSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct QuestEncodedPackBitsCatalog {
    pub(super) offsets: Vec<u16>,
    pub(super) payload: Vec<u8>,
    pub(super) cells: Vec<Vec<u8>>,
    pub(super) summary: QuestPackBitsCatalogSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct QuestEncodedPageAssignments {
    pub(super) directory: Vec<u16>,
    pub(super) payload: Vec<u8>,
    pub(super) pages: Vec<Vec<u16>>,
    pub(super) assignments: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct QuestGlyphCacheMaterials {
    pub(super) catalog_glyphs: String,
    pub(super) menu_font_page: Option<u8>,
    pub(super) space_slots: Vec<Option<u16>>,
    pub(super) latin_advance_slots: Vec<Vec<(u16, i16)>>,
    pub(super) object_particle_codes: Vec<Vec<(u16, u16)>>,
    pub(super) plan: QuestGlyphCachePlan,
    pub(super) glyph_catalog: QuestEncodedRowDictionaryCatalog,
    pub(super) original_atlas_source_catalog: QuestEncodedPackBitsCatalog,
    pub(super) page_assignments: QuestEncodedPageAssignments,
    pub(super) original_atlas_source_slots: Vec<QuestGlyphSlot>,
}

/// page별 sparse upload를 위한 carrier 저장 계획.
///
/// `original_atlas_source*`는 과거 shared-atlas runtime 증거를 다시 읽기 위한
/// 원본 대조 자료이며 `package_bytes`에는 포함하지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCachePlan {
    pub schema_version: &'static str,
    pub glyphs: usize,
    pub pages: usize,
    pub max_page_glyphs: usize,
    pub page_assignments: usize,
    pub aligned_wide_slots: usize,
    pub original_atlas_source_slots: usize,
    pub catalog: QuestRowDictionaryCatalogSummary,
    pub original_atlas_source: QuestPackBitsCatalogSummary,
    pub page_directory_bytes: usize,
    pub page_assignment_plain_bytes: usize,
    pub page_assignment_bytes: usize,
    pub slot_rect_bytes: usize,
    pub korean_descriptor_bytes: usize,
    pub alignment_padding_bytes: usize,
    pub scratch_bytes: usize,
    pub package_bytes: usize,
    pub carrier_bytes: usize,
    pub carrier_remaining_bytes: usize,
    pub package_fits_carrier: bool,
    pub glyph_roundtrip_exact: bool,
    pub original_atlas_source_roundtrip_exact: bool,
    pub page_assignment_roundtrip_exact: bool,
    pub halfword_aligned_slots: bool,
    pub runtime_verified: bool,
    pub build_eligible: bool,
}

/// cache package의 원본·codec·carrier 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCachePlanError(String);

impl fmt::Display for QuestGlyphCachePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestGlyphCachePlanError {}

fn error(message: impl Into<String>) -> QuestGlyphCachePlanError {
    QuestGlyphCachePlanError(message.into())
}

fn is_hangul(character: char) -> bool {
    matches!(character, '\u{ac00}'..='\u{d7a3}')
}

fn repeated_run(raw: &[u8], start: usize) -> usize {
    let mut end = start + 1;
    while end < raw.len() && end - start < 128 && raw[end] == raw[start] {
        end += 1;
    }
    end - start
}

fn encode_packbits(raw: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::new();
    let mut cursor = 0usize;
    while cursor < raw.len() {
        let run = repeated_run(raw, cursor);
        if run >= 3 {
            encoded.push((257usize - run) as u8);
            encoded.push(raw[cursor]);
            cursor += run;
            continue;
        }

        let literal_start = cursor;
        cursor += run;
        while cursor < raw.len() && cursor - literal_start < 128 {
            let next_run = repeated_run(raw, cursor);
            if next_run >= 3 {
                break;
            }
            if cursor - literal_start + next_run > 128 {
                cursor = literal_start + 128;
                break;
            }
            cursor += next_run;
        }
        let literal_len = cursor - literal_start;
        encoded.push((literal_len - 1) as u8);
        encoded.extend_from_slice(&raw[literal_start..cursor]);
    }
    encoded
}

pub(super) fn decode_packbits(
    encoded: &[u8],
    expected_len: usize,
) -> Result<Vec<u8>, QuestGlyphCachePlanError> {
    let mut decoded = Vec::with_capacity(expected_len);
    let mut cursor = 0usize;
    while cursor < encoded.len() {
        let control = encoded[cursor];
        cursor += 1;
        match control {
            0..=127 => {
                let count = usize::from(control) + 1;
                let end = cursor
                    .checked_add(count)
                    .ok_or_else(|| error("QUEST glyph PackBits literal 범위 overflow"))?;
                let literal = encoded
                    .get(cursor..end)
                    .ok_or_else(|| error("QUEST glyph PackBits literal이 잘림"))?;
                decoded.extend_from_slice(literal);
                cursor = end;
            }
            128 => return Err(error("QUEST glyph PackBits 예약 control 128")),
            129..=255 => {
                let count = 257usize - usize::from(control);
                let value = *encoded
                    .get(cursor)
                    .ok_or_else(|| error("QUEST glyph PackBits repeat 값이 잘림"))?;
                cursor += 1;
                let new_len = decoded
                    .len()
                    .checked_add(count)
                    .ok_or_else(|| error("QUEST glyph PackBits repeat 길이 overflow"))?;
                decoded.resize(new_len, value);
            }
        }
        if decoded.len() > expected_len {
            return Err(error("QUEST glyph PackBits 해제 길이 초과"));
        }
    }
    if decoded.len() != expected_len {
        return Err(error(format!(
            "QUEST glyph PackBits 해제 길이 불일치: expected={expected_len}, actual={}",
            decoded.len()
        )));
    }
    Ok(decoded)
}

fn encode_uleb128(mut value: u16, encoded: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        encoded.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn decode_uleb128(encoded: &[u8], cursor: &mut usize) -> Result<u16, QuestGlyphCachePlanError> {
    let mut value = 0u32;
    let mut shift = 0u32;
    loop {
        let byte = *encoded
            .get(*cursor)
            .ok_or_else(|| error("QUEST glyph page ULEB128이 잘림"))?;
        *cursor = cursor
            .checked_add(1)
            .ok_or_else(|| error("QUEST glyph page ULEB128 cursor overflow"))?;
        value |= u32::from(byte & 0x7F)
            .checked_shl(shift)
            .ok_or_else(|| error("QUEST glyph page ULEB128 shift overflow"))?;
        if value > u32::from(u16::MAX) {
            return Err(error("QUEST glyph page ULEB128 u16 범위 초과"));
        }
        if byte & 0x80 == 0 {
            return u16::try_from(value)
                .map_err(|_| error("QUEST glyph page ULEB128 u16 변환 실패"));
        }
        shift = shift
            .checked_add(7)
            .ok_or_else(|| error("QUEST glyph page ULEB128 shift 합계 overflow"))?;
        if shift > 14 {
            return Err(error("QUEST glyph page ULEB128이 3바이트를 초과함"));
        }
    }
}

fn encode_page_indices(indices: &[u16]) -> Result<Vec<u8>, QuestGlyphCachePlanError> {
    let mut encoded = Vec::new();
    let mut previous = None;
    for &index in indices {
        if previous.is_some_and(|previous| index <= previous) {
            encoded.push(0);
            // 두 번째 이후 delta 0은 기준을 -1로 되돌린다.
            let Some(next) = index.checked_add(1) else {
                return Ok(encode_absolute_page_indices(indices));
            };
            encode_uleb128(next, &mut encoded);
        } else {
            encode_uleb128(index - previous.unwrap_or(0), &mut encoded);
        }
        previous = Some(index);
    }
    let absolute = encode_absolute_page_indices(indices);
    Ok(if absolute.len() < encoded.len() {
        absolute
    } else {
        encoded
    })
}

fn encode_absolute_page_indices(indices: &[u16]) -> Vec<u8> {
    let mut encoded = vec![0x80, 0x00];
    for index in indices {
        encoded.extend_from_slice(&index.to_le_bytes());
    }
    encoded
}

pub(super) fn decode_page_indices(encoded: &[u8]) -> Result<Vec<u16>, QuestGlyphCachePlanError> {
    if let Some(absolute) = encoded.strip_prefix(&[0x80, 0x00]) {
        if absolute.len() % 2 != 0 {
            return Err(error("QUEST glyph absolute page가 u16 경계에서 잘림"));
        }
        return Ok(absolute
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .collect());
    }

    let mut indices = Vec::new();
    let mut cursor = 0usize;
    let mut previous: Option<u16> = None;
    while cursor < encoded.len() {
        let delta = decode_uleb128(encoded, &mut cursor)?;
        let index = match previous {
            Some(_) if delta == 0 => decode_uleb128(encoded, &mut cursor)?
                .checked_sub(1)
                .ok_or_else(|| error("QUEST glyph page 재시작 index가 0임"))?,
            Some(previous) => previous
                .checked_add(delta)
                .ok_or_else(|| error("QUEST glyph page catalog index overflow"))?,
            None => delta,
        };
        indices.push(index);
        previous = Some(index);
    }
    Ok(indices)
}

fn pack_row_indices(indices: &[u16]) -> Result<Vec<u8>, QuestGlyphCachePlanError> {
    if indices.len() != QUEST_GLYPH_CACHE_ROWS_PER_CELL {
        return Err(error(format!(
            "QUEST glyph row index 수 불일치: expected={}, actual={}",
            QUEST_GLYPH_CACHE_ROWS_PER_CELL,
            indices.len()
        )));
    }
    let mask = indices.iter().enumerate().fold(0u16, |mask, (row, index)| {
        mask | (u16::from(*index != 0) << row)
    });
    let bits = mask.count_ones() as usize * QUEST_GLYPH_CACHE_ROW_INDEX_BITS;
    let mut packed = vec![0u8; 2 + bits.div_ceil(8)];
    packed[..2].copy_from_slice(&mask.to_le_bytes());
    let mut bit_offset = 0usize;
    for &index in indices {
        if usize::from(index) >= QUEST_GLYPH_CACHE_ROW_INDEX_LIMIT {
            return Err(error(format!(
                "QUEST glyph row index가 {}bit 범위를 넘음: {index}",
                QUEST_GLYPH_CACHE_ROW_INDEX_BITS
            )));
        }
        if index == 0 {
            continue;
        }
        for bit in 0..QUEST_GLYPH_CACHE_ROW_INDEX_BITS {
            if index & (1u16 << bit) != 0 {
                let target = bit_offset + bit;
                packed[2 + target / 8] |= 1 << (target % 8);
            }
        }
        bit_offset += QUEST_GLYPH_CACHE_ROW_INDEX_BITS;
    }
    if bit_offset != bits {
        return Err(error("QUEST glyph row index packing 경계 불일치"));
    }
    Ok(packed)
}

pub(super) fn unpack_row_indices(packed: &[u8]) -> Result<Vec<u16>, QuestGlyphCachePlanError> {
    let header = packed
        .get(..2)
        .ok_or_else(|| error("QUEST glyph row bitmap이 잘림"))?;
    let mask = u16::from_le_bytes([header[0], header[1]]);
    let bits = mask.count_ones() as usize * QUEST_GLYPH_CACHE_ROW_INDEX_BITS;
    if packed.len() != 2 + bits.div_ceil(8) {
        return Err(error("QUEST glyph row bitmap과 index 길이 불일치"));
    }
    if !bits.is_multiple_of(8) && packed[packed.len() - 1] >> (bits % 8) != 0 {
        return Err(error("QUEST glyph row index padding이 0이 아님"));
    }
    let mut indices = Vec::with_capacity(QUEST_GLYPH_CACHE_ROWS_PER_CELL);
    let mut bit_offset = 0usize;
    for row in 0..QUEST_GLYPH_CACHE_ROWS_PER_CELL {
        if mask & (1 << row) == 0 {
            indices.push(0);
            continue;
        }
        let mut index = 0u16;
        for bit in 0..QUEST_GLYPH_CACHE_ROW_INDEX_BITS {
            let source = bit_offset + bit;
            if packed[2 + source / 8] & (1 << (source % 8)) != 0 {
                index |= 1u16 << bit;
            }
        }
        if index == 0 {
            return Err(error("QUEST glyph 비어 있지 않은 행의 index가 0임"));
        }
        indices.push(index);
        bit_offset += QUEST_GLYPH_CACHE_ROW_INDEX_BITS;
    }
    Ok(indices)
}

pub(super) fn cell_row_index_record(
    encoded: &[u8],
    cells: usize,
    index: usize,
) -> Result<&[u8], QuestGlyphCachePlanError> {
    if index >= cells {
        return Err(error("QUEST glyph cell row index 범위 이탈"));
    }
    let directory_bytes = cells
        .checked_add(1)
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(|| error("QUEST glyph cell directory 크기 overflow"))?;
    let directory = encoded
        .get(..directory_bytes)
        .ok_or_else(|| error("QUEST glyph cell directory가 잘림"))?;
    let offset =
        |i: usize| usize::from(u16::from_le_bytes([directory[i * 2], directory[i * 2 + 1]]));
    let payload = &encoded[directory_bytes..];
    let payload_end = offset(cells);
    if offset(0) != 0
        || payload_end + payload_end % 2 != payload.len()
        || payload
            .get(payload_end..)
            .is_none_or(|padding| padding.iter().any(|byte| *byte != 0))
    {
        return Err(error("QUEST glyph cell directory 시작·끝 불일치"));
    }
    payload
        .get(offset(index)..offset(index + 1))
        .ok_or_else(|| error("QUEST glyph cell record 범위 이탈"))
}

fn encode_row_dictionary_catalog(
    cells: &[Vec<u8>],
) -> Result<QuestEncodedRowDictionaryCatalog, QuestGlyphCachePlanError> {
    // bitmap에서 생략한 행은 항상 투명한 row 0으로 복원한다.
    let mut row_set = BTreeSet::from([[0u8; QUEST_GLYPH_CACHE_ROW_BYTES]]);
    for cell in cells {
        if cell.len() != QUEST_GLYPH_CACHE_CELL_BYTES {
            return Err(error(format!(
                "QUEST glyph cache cell 길이 불일치: expected={}, actual={}",
                QUEST_GLYPH_CACHE_CELL_BYTES,
                cell.len()
            )));
        }
        for row in cell.chunks_exact(QUEST_GLYPH_CACHE_ROW_BYTES) {
            let mut key = [0; QUEST_GLYPH_CACHE_ROW_BYTES];
            key.copy_from_slice(row);
            row_set.insert(key);
        }
    }
    if row_set.len() >= QUEST_GLYPH_CACHE_ROW_INDEX_LIMIT {
        return Err(error(format!(
            "QUEST glyph row dictionary가 {}bit index 범위를 넘음: {}",
            QUEST_GLYPH_CACHE_ROW_INDEX_BITS,
            row_set.len()
        )));
    }
    let rows = row_set.into_iter().collect::<Vec<_>>();
    let row_indices = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            u16::try_from(index)
                .map(|index| (*row, index))
                .map_err(|_| error("QUEST glyph row dictionary index 변환 실패"))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut cell_directory = vec![0u16];
    let mut payload = Vec::new();
    for cell in cells {
        let indices = cell
            .chunks_exact(QUEST_GLYPH_CACHE_ROW_BYTES)
            .map(|row| {
                let mut key = [0; QUEST_GLYPH_CACHE_ROW_BYTES];
                key.copy_from_slice(row);
                row_indices
                    .get(&key)
                    .copied()
                    .ok_or_else(|| error("QUEST glyph row dictionary lookup 누락"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let packed = pack_row_indices(&indices)?;
        let decoded = unpack_row_indices(&packed)?;
        if decoded != indices {
            return Err(error("QUEST glyph row index 자기 roundtrip 불일치"));
        }
        payload.extend_from_slice(&packed);
        cell_directory.push(
            u16::try_from(payload.len())
                .map_err(|_| error("QUEST glyph cell payload가 u16 offset을 넘음"))?,
        );
    }
    let mut packed_cell_indices = cell_directory
        .iter()
        .flat_map(|n| n.to_le_bytes())
        .collect::<Vec<_>>();
    packed_cell_indices.extend_from_slice(&payload);
    if packed_cell_indices.len() % 2 != 0 {
        packed_cell_indices.push(0);
    }
    for index in 0..cells.len() {
        unpack_row_indices(cell_row_index_record(
            &packed_cell_indices,
            cells.len(),
            index,
        )?)?;
    }
    let dictionary_bytes = rows
        .len()
        .checked_mul(QUEST_GLYPH_CACHE_ROW_BYTES)
        .ok_or_else(|| error("QUEST glyph row dictionary 크기 overflow"))?;
    let plain_bytes = cells
        .len()
        .checked_mul(QUEST_GLYPH_CACHE_CELL_BYTES)
        .ok_or_else(|| error("QUEST glyph catalog plain 크기 overflow"))?;
    let stored_bytes = dictionary_bytes
        .checked_add(packed_cell_indices.len())
        .ok_or_else(|| error("QUEST glyph row catalog 저장 크기 overflow"))?;
    Ok(QuestEncodedRowDictionaryCatalog {
        rows,
        packed_cell_indices,
        cells: cells.to_vec(),
        summary: QuestRowDictionaryCatalogSummary {
            cells: cells.len(),
            plain_bytes,
            dictionary_rows: row_indices.len(),
            dictionary_bytes,
            row_index_bits: QUEST_GLYPH_CACHE_ROW_INDEX_BITS,
            cell_index_bytes: stored_bytes - dictionary_bytes,
            stored_bytes,
        },
    })
}

fn encode_packbits_catalog(
    cells: &[Vec<u8>],
) -> Result<QuestEncodedPackBitsCatalog, QuestGlyphCachePlanError> {
    let mut offsets = Vec::with_capacity(cells.len() + 1);
    let mut payload = Vec::new();
    offsets.push(0);
    for cell in cells {
        if cell.len() != QUEST_GLYPH_CACHE_CELL_BYTES {
            return Err(error(format!(
                "QUEST glyph cache cell 길이 불일치: expected={}, actual={}",
                QUEST_GLYPH_CACHE_CELL_BYTES,
                cell.len()
            )));
        }
        let encoded = encode_packbits(cell);
        if decode_packbits(&encoded, cell.len())? != *cell {
            return Err(error("QUEST glyph PackBits 자기 roundtrip 불일치"));
        }
        payload.extend_from_slice(&encoded);
        offsets.push(
            u16::try_from(payload.len())
                .map_err(|_| error("QUEST glyph catalog payload가 u16 offset 범위를 초과함"))?,
        );
    }
    let index_bytes = cells
        .len()
        .checked_add(1)
        .and_then(|entries| entries.checked_mul(QUEST_GLYPH_CACHE_CATALOG_OFFSET_BYTES))
        .ok_or_else(|| error("QUEST glyph catalog index 크기 overflow"))?;
    let plain_bytes = cells
        .len()
        .checked_mul(QUEST_GLYPH_CACHE_CELL_BYTES)
        .ok_or_else(|| error("QUEST glyph catalog plain 크기 overflow"))?;
    let summary = QuestPackBitsCatalogSummary {
        cells: cells.len(),
        plain_bytes,
        index_bytes,
        payload_bytes: payload.len(),
        stored_bytes: index_bytes
            .checked_add(payload.len())
            .ok_or_else(|| error("QUEST glyph catalog 저장 크기 overflow"))?,
    };
    Ok(QuestEncodedPackBitsCatalog {
        offsets,
        payload,
        cells: cells.to_vec(),
        summary,
    })
}

fn extract_cell(
    atlas_pixels: &[u8],
    slot: QuestGlyphSlot,
) -> Result<Vec<u8>, QuestGlyphCachePlanError> {
    if slot.width > QUEST_FONT_WIDE_CELL_WIDTH || slot.height != QUEST_FONT_CELL_HEIGHT {
        return Err(error("QUEST original atlas source cell 크기 불일치"));
    }
    let destination_x = (QUEST_FONT_WIDE_CELL_WIDTH - slot.width) / 2;
    let mut pixels = vec![0; QUEST_FONT_WIDE_CELL_WIDTH * QUEST_FONT_CELL_HEIGHT];
    for row in 0..QUEST_FONT_CELL_HEIGHT {
        let start = (slot.y + row)
            .checked_mul(QUEST_FONT_LAYOUT.width)
            .and_then(|offset| offset.checked_add(slot.x))
            .ok_or_else(|| error("QUEST original atlas source cell offset overflow"))?;
        let end = start
            .checked_add(slot.width)
            .ok_or_else(|| error("QUEST original atlas source cell 끝 offset overflow"))?;
        let source_row = atlas_pixels
            .get(start..end)
            .ok_or_else(|| error("QUEST original atlas source cell 범위 이탈"))?;
        let destination_start = row
            .checked_mul(QUEST_FONT_WIDE_CELL_WIDTH)
            .and_then(|offset| offset.checked_add(destination_x))
            .ok_or_else(|| error("QUEST source cell destination offset overflow"))?;
        let destination_end = destination_start
            .checked_add(slot.width)
            .ok_or_else(|| error("QUEST source cell destination 끝 offset overflow"))?;
        pixels
            .get_mut(destination_start..destination_end)
            .ok_or_else(|| error("QUEST source cell destination 범위 이탈"))?
            .copy_from_slice(source_row);
    }
    encode(&pixels, QUEST_GLYPH_CACHE_CELL_LAYOUT).map_err(|source| error(source.to_string()))
}

fn source_glyph_slots(
    code_table: &QuestCodeTable,
) -> Result<BTreeMap<char, QuestGlyphSlot>, QuestGlyphCachePlanError> {
    let mut slots = BTreeMap::new();
    for (index, code) in code_table.codes().iter().copied().enumerate() {
        let Some(glyph) = decode_quest_source_glyph(code) else {
            continue;
        };
        let glyph = canonicalize_quest_source_glyph(glyph);
        let slot = QuestGlyphSlot::for_index(index).map_err(|source| error(source.to_string()))?;
        if let Some(previous) = slots.insert(glyph, slot) {
            return Err(error(format!(
                "QUEST source glyph slot 의미 중복: {glyph:?}, index {} / {index}",
                previous.index
            )));
        }
    }
    Ok(slots)
}

fn aligned_wide_slots() -> Result<Vec<QuestGlyphSlot>, QuestGlyphCachePlanError> {
    let slots = (QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX
        ..QUEST_FONT_NARROW_SLOTS + QUEST_FONT_WIDE_SLOTS)
        .map(|index| QuestGlyphSlot::for_index(index).map_err(|source| error(source.to_string())))
        .collect::<Result<Vec<_>, _>>()?;
    if slots.iter().any(|slot| slot.x % 4 != 0) {
        return Err(error(
            "QUEST glyph cache slot이 indexed4 halfword 경계에 맞지 않음",
        ));
    }
    Ok(slots)
}

fn scratch_alignment_padding(content_bytes: usize) -> usize {
    (QUEST_GLYPH_CACHE_SCRATCH_ALIGNMENT - content_bytes % QUEST_GLYPH_CACHE_SCRATCH_ALIGNMENT)
        % QUEST_GLYPH_CACHE_SCRATCH_ALIGNMENT
}

fn page_glyphs(
    batches: &QuestTranslationBatchAnalysis,
    routing: &QuestTranslationRoutingAnalysis,
) -> Result<BTreeMap<u8, BTreeSet<char>>, QuestGlyphCachePlanError> {
    let mut page_by_batch = BTreeMap::new();
    for route in &routing.routes {
        match page_by_batch.insert(route.batch_id.as_str(), route.page_id) {
            Some(existing) if existing != route.page_id => {
                return Err(error(format!(
                    "{}: QUEST batch가 여러 page ID에 결속됨",
                    route.batch_id
                )));
            }
            _ => {}
        }
    }
    let mut pages = BTreeMap::<u8, BTreeSet<char>>::new();
    for entry in &batches.entries {
        if entry.role != QuestTranslationBatchRole::Translate {
            continue;
        }
        let page_id = *page_by_batch
            .get(entry.batch_id.as_str())
            .ok_or_else(|| error(format!("{}: QUEST cache page ID 누락", entry.batch_id)))?;
        pages
            .entry(page_id)
            .or_default()
            .extend(entry.korean_draft.chars().filter(|glyph| is_hangul(*glyph)));
    }
    if pages.len() != routing.pages
        || !pages
            .keys()
            .copied()
            .eq(1..=u8::try_from(routing.pages).map_err(|_| error("QUEST page 수 u8 초과"))?)
    {
        return Err(error(format!(
            "QUEST glyph cache page ID가 연속 1..={}가 아님",
            routing.pages
        )));
    }
    Ok(pages)
}

fn page_assignment_storage(
    pages: &BTreeMap<u8, Vec<char>>,
    repertoire: &BTreeSet<char>,
) -> Result<QuestEncodedPageAssignments, QuestGlyphCachePlanError> {
    let catalog_indices = repertoire
        .iter()
        .enumerate()
        .map(|(index, glyph)| {
            u16::try_from(index)
                .map(|index| (*glyph, index))
                .map_err(|_| error("QUEST glyph catalog index가 u16 범위를 초과함"))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut directory = Vec::with_capacity(pages.len() + 1);
    let mut payload = Vec::new();
    let mut page_indices = Vec::with_capacity(pages.len());
    let mut assignments = 0usize;
    directory.push(0);
    for glyphs in pages.values() {
        if glyphs.iter().copied().collect::<BTreeSet<_>>().len() != glyphs.len() {
            return Err(error("QUEST glyph page assignment에 중복 glyph가 있음"));
        }
        let indices = glyphs
            .iter()
            .map(|glyph| {
                catalog_indices
                    .get(glyph)
                    .copied()
                    .ok_or_else(|| error(format!("QUEST glyph {glyph:?} catalog index 누락")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let encoded = encode_page_indices(&indices)?;
        if decode_page_indices(&encoded)? != indices {
            return Err(error("QUEST glyph page assignment 자기 roundtrip 불일치"));
        }
        payload.extend_from_slice(&encoded);
        assignments = assignments
            .checked_add(indices.len())
            .ok_or_else(|| error("QUEST glyph page assignment 수 합계 overflow"))?;
        page_indices.push(indices);
        directory.push(
            u16::try_from(payload.len())
                .map_err(|_| error("QUEST glyph page assignment가 u16 offset 범위를 초과함"))?,
        );
    }
    let directory_bytes = pages
        .len()
        .checked_add(1)
        .and_then(|entries| entries.checked_mul(QUEST_GLYPH_CACHE_PAGE_OFFSET_BYTES))
        .ok_or_else(|| error("QUEST glyph cache page directory 크기 overflow"))?;
    if directory
        .len()
        .checked_mul(QUEST_GLYPH_CACHE_PAGE_OFFSET_BYTES)
        != Some(directory_bytes)
    {
        return Err(error("QUEST glyph cache page directory 직렬화 크기 불일치"));
    }
    Ok(QuestEncodedPageAssignments {
        directory,
        payload,
        pages: page_indices,
        assignments,
    })
}

/// 정렬된 Unicode repertoire를 package catalog와 같은 선택 폰트 cell로 만든다.
pub(super) fn render_quest_glyph_cache_repertoire_cells(
    repertoire: &BTreeSet<char>,
) -> Result<Vec<Vec<u8>>, QuestGlyphCachePlanError> {
    if repertoire.is_empty() || repertoire.iter().any(|glyph| glyph.is_control()) {
        return Err(error(
            "QUEST glyph cache repertoire가 비었거나 제어 문자를 포함함",
        ));
    }
    let font = crate::font::selected::load(crate::font::selected::DENKI).map_err(error)?;
    repertoire
        .iter()
        .map(|glyph| {
            let pixels = crate::font::selected::quest_cell(&font, *glyph, 12).map_err(error)?;
            encode(&pixels, QUEST_GLYPH_CACHE_CELL_LAYOUT)
                .map_err(|source| error(source.to_string()))
        })
        .collect()
}

fn render_quest_glyph_cache_repertoire_cells_with_source_atlas(
    repertoire: &BTreeSet<char>,
    original_pixels: &[u8],
    source_slots: &BTreeMap<char, QuestGlyphSlot>,
) -> Result<Vec<Vec<u8>>, QuestGlyphCachePlanError> {
    if repertoire.is_empty() || repertoire.iter().any(|glyph| glyph.is_control()) {
        return Err(error(
            "QUEST glyph cache repertoire가 비었거나 제어 문자를 포함함",
        ));
    }
    let font = crate::font::selected::load(crate::font::selected::DENKI).map_err(error)?;
    repertoire
        .iter()
        .map(|glyph| {
            if let Some(slot) = source_slots
                .get(glyph)
                .filter(|_| !crate::font::selected::quest_common_glyph(*glyph))
            {
                return extract_cell(original_pixels, *slot);
            }
            let pixels = crate::font::selected::quest_cell(&font, *glyph, 12).map_err(error)?;
            encode(&pixels, QUEST_GLYPH_CACHE_CELL_LAYOUT)
                .map_err(|source| error(source.to_string()))
        })
        .collect()
}

pub(super) fn build_quest_glyph_cache_materials_for_pages(
    source: &VerifiedSourceImage,
    schema_version: &'static str,
    pages: BTreeMap<u8, BTreeSet<char>>,
    repertoire: BTreeSet<char>,
) -> Result<QuestGlyphCacheMaterials, QuestGlyphCachePlanError> {
    let pages = pages
        .into_iter()
        .map(|(page_id, glyphs)| (page_id, glyphs.into_iter().collect()))
        .collect();
    build_quest_glyph_cache_materials_for_ordered_pages(source, schema_version, pages, repertoire)
}

// 공백이 동적 slot으로 승격된 page와 원본 공백 slot을 보존하는 page를 구별한다.
fn space_slot_for_page(glyphs: &[char], source_space: usize) -> Option<u16> {
    let first = QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX;
    if let Some(index) = glyphs.iter().position(|glyph| matches!(glyph, ' ' | '　')) {
        return u16::try_from(first + index).ok();
    }
    if (first..first + glyphs.len()).contains(&source_space) {
        None
    } else {
        u16::try_from(source_space).ok()
    }
}

/// page의 glyph 순서를 그대로 code slot 순서로 사용하는 저장 재료를 만든다.
///
/// 공통 glyph를 여러 page의 같은 code에 둘 때만 ordered 입력을 사용한다.
/// 기존 set 기반 경로는 위 wrapper에서 정렬된 순서로 변환된다.
pub(super) fn build_quest_glyph_cache_materials_for_ordered_pages(
    source: &VerifiedSourceImage,
    schema_version: &'static str,
    pages: BTreeMap<u8, Vec<char>>,
    repertoire: BTreeSet<char>,
) -> Result<QuestGlyphCacheMaterials, QuestGlyphCachePlanError> {
    build_quest_glyph_cache_materials_with_menu_font(
        source,
        schema_version,
        pages,
        repertoire,
        None,
    )
}

pub(super) fn build_quest_glyph_cache_materials_with_menu_font(
    source: &VerifiedSourceImage,
    schema_version: &'static str,
    pages: BTreeMap<u8, Vec<char>>,
    repertoire: BTreeSet<char>,
    menu_font_page: Option<u8>,
) -> Result<QuestGlyphCacheMaterials, QuestGlyphCachePlanError> {
    if pages.is_empty()
        || !pages
            .keys()
            .copied()
            .eq(1..=u8::try_from(pages.len()).map_err(|_| error("QUEST page 수 u8 초과"))?)
    {
        return Err(error(
            "QUEST glyph cache page ID가 연속 1부터 시작하지 않음",
        ));
    }
    if pages.values().any(|glyphs| {
        glyphs.iter().any(|glyph| glyph.is_control())
            || glyphs.iter().copied().collect::<BTreeSet<_>>().len() != glyphs.len()
    }) {
        return Err(error(
            "QUEST glyph cache ordered page가 제어·중복 glyph를 포함함",
        ));
    }
    let page_repertoire = pages
        .values()
        .flat_map(|glyphs| glyphs.iter().copied())
        .collect::<BTreeSet<_>>();
    if page_repertoire != repertoire {
        return Err(error(
            "QUEST glyph cache page와 repertoire glyph 집합 불일치",
        ));
    }
    if menu_font_page.is_some_and(|page| !pages.contains_key(&page)) {
        return Err(error("메뉴 폰트 page가 공급 목록에 없음"));
    }
    // 코드 순서는 원래 pages를 유지하고 catalog 참조만 폰트별로 분리한다.
    let catalog_pages = pages
        .iter()
        .map(|(&id, glyphs)| {
            let text: String = glyphs.iter().collect();
            let text = if Some(id) == menu_font_page {
                crate::font::selected::menu_catalog_text(&text)
            } else {
                text
            };
            (id, text.chars().collect::<Vec<_>>())
        })
        .collect::<BTreeMap<_, _>>();
    let repertoire = catalog_pages
        .values()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let code_table = quest_code_table(&iso).map_err(|source| error(source.to_string()))?;
    let original_vdd = iso
        .read_file(RPG_GRP_PATH)
        .map_err(|source| error(source.to_string()))?;
    let archive = VddArchive::parse(&original_vdd).map_err(|source| error(source.to_string()))?;
    let font_entry = archive
        .find(RPG_FONT_NAME)
        .ok_or_else(|| error("QUEST glyph cache 원본 RPG_FNT1.BTX 누락"))?;
    let original_plain =
        decompress(archive.read_file(font_entry)).map_err(|source| error(source.to_string()))?;
    let original_pixels =
        decode(&original_plain, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;
    let source_slots = source_glyph_slots(&code_table)?;
    let glyph_cells = render_quest_glyph_cache_repertoire_cells_with_source_atlas(
        &repertoire,
        &original_pixels,
        &source_slots,
    )?;
    let glyph_catalog = encode_row_dictionary_catalog(&glyph_cells)?;

    let max_page_glyphs = pages.values().map(Vec::len).max().unwrap_or(0);
    let page_assignments = page_assignment_storage(&catalog_pages, &repertoire)?;
    let slots = aligned_wide_slots()?;
    if max_page_glyphs == 0 || max_page_glyphs > slots.len() {
        return Err(error(format!(
            "QUEST glyph cache 정렬 slot 부족: demand={max_page_glyphs}, slots={}",
            slots.len()
        )));
    }
    let original_atlas_source_cells = slots
        .iter()
        .take(max_page_glyphs)
        .map(|slot| extract_cell(&original_pixels, *slot))
        .collect::<Result<Vec<_>, _>>()?;
    let original_atlas_source_catalog = encode_packbits_catalog(&original_atlas_source_cells)?;

    let page_assignment_plain_bytes = page_assignments
        .assignments
        .checked_mul(QUEST_GLYPH_CACHE_PLAIN_ASSIGNMENT_BYTES)
        .ok_or_else(|| error("QUEST glyph cache page assignment 크기 overflow"))?;
    let slot_rect_bytes = max_page_glyphs
        .checked_mul(QUEST_GLYPH_CACHE_SLOT_RECT_BYTES)
        .ok_or_else(|| error("QUEST glyph cache slot rect 크기 overflow"))?;
    let page_directory_bytes = page_assignments
        .directory
        .len()
        .checked_mul(QUEST_GLYPH_CACHE_PAGE_OFFSET_BYTES)
        .ok_or_else(|| error("QUEST glyph cache page directory 크기 overflow"))?;
    let unaligned_package_bytes = glyph_catalog
        .summary
        .stored_bytes
        .checked_add(page_directory_bytes)
        .and_then(|value| value.checked_add(page_assignments.payload.len()))
        .and_then(|value| value.checked_add(slot_rect_bytes))
        .and_then(|value| value.checked_add(QUEST_GLYPH_CACHE_KOREAN_DESCRIPTOR_BYTES))
        .ok_or_else(|| error("QUEST glyph cache package 크기 overflow"))?;
    let alignment_padding_bytes = scratch_alignment_padding(unaligned_package_bytes);
    let package_bytes = unaligned_package_bytes
        .checked_add(alignment_padding_bytes)
        .and_then(|value| value.checked_add(QUEST_GLYPH_CACHE_SCRATCH_BYTES))
        .ok_or_else(|| error("QUEST glyph cache package 크기 overflow"))?;
    let carrier_remaining_bytes = QUEST_PAGE_CARRIER_CAPACITY_BYTES.saturating_sub(package_bytes);

    let plan = QuestGlyphCachePlan {
        schema_version,
        glyphs: repertoire.len(),
        pages: pages.len(),
        max_page_glyphs,
        page_assignments: page_assignments.assignments,
        aligned_wide_slots: slots.len(),
        original_atlas_source_slots: original_atlas_source_catalog.summary.cells,
        catalog: glyph_catalog.summary.clone(),
        original_atlas_source: original_atlas_source_catalog.summary.clone(),
        page_directory_bytes,
        page_assignment_plain_bytes,
        page_assignment_bytes: page_assignments.payload.len(),
        slot_rect_bytes,
        korean_descriptor_bytes: QUEST_GLYPH_CACHE_KOREAN_DESCRIPTOR_BYTES,
        alignment_padding_bytes,
        scratch_bytes: QUEST_GLYPH_CACHE_SCRATCH_BYTES,
        package_bytes,
        carrier_bytes: QUEST_PAGE_CARRIER_CAPACITY_BYTES,
        carrier_remaining_bytes,
        package_fits_carrier: package_bytes <= QUEST_PAGE_CARRIER_CAPACITY_BYTES,
        glyph_roundtrip_exact: true,
        original_atlas_source_roundtrip_exact: true,
        page_assignment_roundtrip_exact: true,
        halfword_aligned_slots: true,
        runtime_verified: false,
        build_eligible: false,
    };
    let source_space = code_table
        .index_of([0x81, 0x40])
        .ok_or_else(|| error("QUEST 원본 공백 code 누락"))?;
    let space_slots = pages
        .values()
        .map(|glyphs| space_slot_for_page(glyphs, source_space))
        .collect();
    let non_hangul_tails = super::quest_item_name_particles::non_hangul_tails().map_err(error)?;
    let object_particle_codes = pages
        .values()
        .map(|glyphs| {
            let code = |ch| {
                glyphs.iter().position(|g| *g == ch).map(|index| {
                    u16::from_le_bytes(
                        code_table.codes()[QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX + index],
                    )
                })
            };
            glyphs
                .iter()
                .filter_map(|&glyph| {
                    let tail = non_hangul_tails.get(&glyph).copied().unwrap_or(glyph);
                    let particle = crate::text::select_object_particle(tail).ok()?;
                    Some((code(glyph)?, code(particle)?))
                })
                .collect()
        })
        .collect();
    // 반각 숫자는 12px 글꼴의 최대 잉크 폭 8px에 맞춰 같은 advance를 쓴다.
    // cm도 표기·배율은 유지하고 다음 cursor만 당긴다. 전각 숫자는 제외한다.
    let latin_advance_slots = pages
        .values()
        .map(|glyphs| {
            glyphs
                .iter()
                .enumerate()
                .filter_map(|(index, glyph)| {
                    let delta = match glyph {
                        '0'..='9' => -4,
                        'c' => -4,
                        'm' => -2,
                        _ => return None,
                    };
                    Some((
                        (QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX + index) as u16,
                        delta,
                    ))
                })
                .collect()
        })
        .collect();
    Ok(QuestGlyphCacheMaterials {
        catalog_glyphs: repertoire.iter().collect(),
        menu_font_page,
        latin_advance_slots,
        object_particle_codes,
        space_slots,
        plan,
        glyph_catalog,
        original_atlas_source_catalog,
        page_assignments,
        original_atlas_source_slots: slots.into_iter().take(max_page_glyphs).collect(),
    })
}

pub(super) fn build_quest_glyph_cache_materials(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
    routing: &QuestTranslationRoutingAnalysis,
) -> Result<QuestGlyphCacheMaterials, QuestGlyphCachePlanError> {
    let pages = page_glyphs(batches, routing)?;
    let repertoire = batches
        .draft_hangul_repertoire
        .chars()
        .collect::<BTreeSet<_>>();
    build_quest_glyph_cache_materials_for_pages(
        source,
        QUEST_GLYPH_CACHE_PLAN_SCHEMA_VERSION,
        pages,
        repertoire,
    )
}

/// 지원 원본과 provisional batch에서 sparse glyph cache 저장량을 계산한다.
pub fn audit_quest_glyph_cache_plan(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
    routing: &QuestTranslationRoutingAnalysis,
) -> Result<QuestGlyphCachePlan, QuestGlyphCachePlanError> {
    Ok(build_quest_glyph_cache_materials(source, batches, routing)?.plan)
}

pub(super) fn verify_quest_glyph_cache_plan_structure(
    plan: QuestGlyphCachePlan,
) -> Result<QuestGlyphCachePlan, QuestGlyphCachePlanError> {
    verify_plan_structure(&plan)?;
    Ok(plan)
}

/// 현재 번역 입력에서 파생한 glyph cache 분모와 저장량을 검증한다.
pub fn verify_box_quest_glyph_cache_plan(
    source: &VerifiedSourceImage,
    batches: &QuestTranslationBatchAnalysis,
    routing: &QuestTranslationRoutingAnalysis,
) -> Result<QuestGlyphCachePlan, QuestGlyphCachePlanError> {
    let plan = audit_quest_glyph_cache_plan(source, batches, routing)?;
    let pages = page_glyphs(batches, routing)?;
    let expected_glyphs = pages
        .values()
        .flat_map(|glyphs| glyphs.iter().copied())
        .collect::<BTreeSet<_>>()
        .len();
    let expected_assignments = pages.values().map(BTreeSet::len).sum::<usize>();
    let expected_max_page_glyphs = pages.values().map(BTreeSet::len).max().unwrap_or(0);
    if plan.schema_version != QUEST_GLYPH_CACHE_PLAN_SCHEMA_VERSION
        || plan.glyphs != expected_glyphs
        || plan.pages != pages.len()
        || plan.max_page_glyphs != expected_max_page_glyphs
        || plan.page_assignments != expected_assignments
    {
        return Err(error("QUEST glyph cache 현재 입력 파생 분모 불일치"));
    }
    verify_quest_glyph_cache_plan_structure(plan)
}

fn verify_plan_structure(plan: &QuestGlyphCachePlan) -> Result<(), QuestGlyphCachePlanError> {
    let expected_page_directory_bytes = plan
        .pages
        .checked_add(1)
        .and_then(|entries| entries.checked_mul(QUEST_GLYPH_CACHE_PAGE_OFFSET_BYTES))
        .ok_or_else(|| error("QUEST glyph cache page directory 크기 overflow"))?;
    let expected_plain_assignment_bytes = plan
        .page_assignments
        .checked_mul(QUEST_GLYPH_CACHE_PLAIN_ASSIGNMENT_BYTES)
        .ok_or_else(|| error("QUEST glyph cache plain assignment 크기 overflow"))?;
    let expected_slot_rect_bytes = plan
        .max_page_glyphs
        .checked_mul(QUEST_GLYPH_CACHE_SLOT_RECT_BYTES)
        .ok_or_else(|| error("QUEST glyph cache slot RECT 크기 overflow"))?;
    let expected_dictionary_bytes = plan
        .catalog
        .dictionary_rows
        .checked_mul(QUEST_GLYPH_CACHE_ROW_BYTES)
        .ok_or_else(|| error("QUEST glyph cache dictionary 크기 overflow"))?;
    let maximum_cell_index_bytes = plan
        .glyphs
        .checked_mul(QUEST_GLYPH_CACHE_MAX_CELL_ROW_INDEX_BYTES + 2)
        .and_then(|n| n.checked_add(2))
        .ok_or_else(|| error("QUEST glyph cache cell index 크기 overflow"))?;
    let minimum_cell_index_bytes = plan
        .glyphs
        .checked_mul(4)
        .and_then(|n| n.checked_add(2))
        .ok_or_else(|| error("QUEST glyph cache 최소 cell index 크기 overflow"))?;
    let expected_catalog_plain_bytes = plan
        .glyphs
        .checked_mul(QUEST_GLYPH_CACHE_CELL_BYTES)
        .ok_or_else(|| error("QUEST glyph cache catalog plain 크기 overflow"))?;
    let expected_catalog_stored_bytes = plan
        .catalog
        .dictionary_bytes
        .checked_add(plan.catalog.cell_index_bytes)
        .ok_or_else(|| error("QUEST glyph cache catalog 저장 크기 overflow"))?;
    let expected_original_atlas_source_plain_bytes = plan
        .max_page_glyphs
        .checked_mul(QUEST_GLYPH_CACHE_CELL_BYTES)
        .ok_or_else(|| error("QUEST glyph cache original atlas source plain 크기 overflow"))?;
    let expected_original_atlas_source_index_bytes = plan
        .max_page_glyphs
        .checked_add(1)
        .and_then(|entries| entries.checked_mul(QUEST_GLYPH_CACHE_CATALOG_OFFSET_BYTES))
        .ok_or_else(|| error("QUEST glyph cache original atlas source index 크기 overflow"))?;
    let expected_original_atlas_source_stored_bytes = plan
        .original_atlas_source
        .index_bytes
        .checked_add(plan.original_atlas_source.payload_bytes)
        .ok_or_else(|| error("QUEST glyph cache original atlas source 저장 크기 overflow"))?;
    let expected_package_bytes = plan
        .catalog
        .stored_bytes
        .checked_add(plan.page_directory_bytes)
        .and_then(|value| value.checked_add(plan.page_assignment_bytes))
        .and_then(|value| value.checked_add(plan.slot_rect_bytes))
        .and_then(|value| value.checked_add(plan.korean_descriptor_bytes))
        .and_then(|value| value.checked_add(plan.alignment_padding_bytes))
        .and_then(|value| value.checked_add(plan.scratch_bytes))
        .ok_or_else(|| error("QUEST glyph cache package 합계 overflow"))?;
    let expected_carrier_remaining = plan
        .carrier_bytes
        .checked_sub(plan.package_bytes)
        .ok_or_else(|| error("QUEST glyph cache package가 carrier를 초과함"))?;
    let scratch_offset = plan
        .package_bytes
        .checked_sub(plan.scratch_bytes)
        .ok_or_else(|| error("QUEST glyph cache scratch가 package를 초과함"))?;
    if plan.schema_version != QUEST_GLYPH_CACHE_PLAN_SCHEMA_VERSION
        || plan.glyphs == 0
        || plan.pages == 0
        || plan.max_page_glyphs == 0
        || plan.catalog.cells != plan.glyphs
        || plan.catalog.plain_bytes != expected_catalog_plain_bytes
        || plan.catalog.dictionary_bytes != expected_dictionary_bytes
        || !(minimum_cell_index_bytes..=maximum_cell_index_bytes)
            .contains(&plan.catalog.cell_index_bytes)
        || plan.catalog.stored_bytes != expected_catalog_stored_bytes
        || plan.original_atlas_source.cells != plan.max_page_glyphs
        || plan.original_atlas_source_slots != plan.max_page_glyphs
        || plan.original_atlas_source.plain_bytes != expected_original_atlas_source_plain_bytes
        || plan.original_atlas_source.index_bytes != expected_original_atlas_source_index_bytes
        || plan.original_atlas_source.stored_bytes != expected_original_atlas_source_stored_bytes
        || plan.page_directory_bytes != expected_page_directory_bytes
        || plan.page_assignment_plain_bytes != expected_plain_assignment_bytes
        || plan.page_assignment_bytes > plan.page_assignment_plain_bytes
        || plan.slot_rect_bytes != expected_slot_rect_bytes
        || plan.korean_descriptor_bytes != QUEST_GLYPH_CACHE_KOREAN_DESCRIPTOR_BYTES
        || plan.scratch_bytes != QUEST_GLYPH_CACHE_SCRATCH_BYTES
        || plan.alignment_padding_bytes >= QUEST_GLYPH_CACHE_SCRATCH_ALIGNMENT
        || !scratch_offset.is_multiple_of(QUEST_GLYPH_CACHE_SCRATCH_ALIGNMENT)
        || plan.package_bytes != expected_package_bytes
        || plan.carrier_bytes != QUEST_PAGE_CARRIER_CAPACITY_BYTES
        || plan.carrier_remaining_bytes != expected_carrier_remaining
        || plan.aligned_wide_slots < plan.max_page_glyphs
    {
        return Err(error("QUEST glyph cache plan 파생 저장량 불일치"));
    }
    if !plan.package_fits_carrier
        || !plan.glyph_roundtrip_exact
        || !plan.original_atlas_source_roundtrip_exact
        || !plan.page_assignment_roundtrip_exact
        || !plan.halfword_aligned_slots
        || plan.runtime_verified
        || plan.build_eligible
    {
        return Err(error("QUEST glyph cache plan 판정 drift"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_glyph_cache_plan_tests.rs"]
mod tests;
