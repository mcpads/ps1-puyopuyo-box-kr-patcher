use std::collections::{BTreeMap, BTreeSet};

use crate::font::{decode_quest_source_glyph, QuestCodeTable};
use crate::script::{parse_quest_token, QuestToken};

use super::super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::super::quest_glyph_cache_plan::{
    render_quest_glyph_cache_repertoire_cells, QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX,
};
use super::super::quest_glyph_cache_upload_poc::map_catalog_page_to_codes;
use super::super::quest_translation_encoding::{
    canonicalize_quest_translation_glyph, normalize_quest_translation_text,
    QuestTranslationEncodingPlan,
};
use super::model::{error, QuestManualStreamWriterError};

/// page directory와 원본 code table을 이용해 표시 문자를 2바이트 glyph code로 만든다.
///
/// 독립 bank에서 실제로 표시할 비공백 glyph는 page assignment에 없으면
/// 실패한다. 눈에 보이지 않는 전각 공백만 기존 source code를 padding으로
/// 재사용한다. VDD manual writer와 EXE resident writer가 같은 규칙을 쓴다.
pub(in crate::pipeline) fn encode_quest_page_text(
    text: &str,
    page_id: u8,
    page_codes: &BTreeMap<u8, BTreeMap<char, [u8; 2]>>,
    code_table: &QuestCodeTable,
) -> Result<Vec<[u8; 2]>, QuestManualStreamWriterError> {
    let normalized =
        normalize_quest_translation_text(text).map_err(|source| error(source.to_string()))?;
    let dynamic = page_codes
        .get(&page_id)
        .ok_or_else(|| error(format!("QUEST writer page code 누락: page={page_id}")))?;
    let overwritten_end = QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX
        .checked_add(dynamic.len())
        .ok_or_else(|| error("QUEST writer 동적 slot 범위 overflow"))?;
    normalized
        .chars()
        .map(|glyph| {
            let code = if let Some(code) = dynamic.get(&glyph) {
                *code
            } else {
                if glyph != '\u{3000}' {
                    return Err(error(format!(
                        "QUEST writer 독립 bank 공급 누락: {glyph:?}, page={page_id}"
                    )));
                }
                let mut sources = code_table
                    .codes()
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(_, code)| {
                        decode_quest_source_glyph(*code)
                            .map(canonicalize_quest_translation_glyph)
                            == Some(glyph)
                    });
                let (source_index, source_code) = sources.next().ok_or_else(|| {
                    error(format!("QUEST writer glyph mapping 누락: {glyph:?}"))
                })?;
                if sources.next().is_some() {
                    return Err(error(format!(
                        "QUEST writer 원본 glyph mapping 중복: {glyph:?}"
                    )));
                }
                if (QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX..overwritten_end)
                    .contains(&source_index)
                {
                    return Err(error(format!(
                        "QUEST writer source glyph slot 충돌: {glyph:?}, index {}",
                        source_index
                    )));
                }
                source_code
            };
            if code_table.index_of(code).is_none()
                || !matches!(parse_quest_token(&code, 0), Ok(QuestToken::Glyph(raw)) if raw == code)
            {
                return Err(error(format!(
                    "QUEST writer glyph code가 원본 code table의 glyph가 아님: {glyph:?}={code:02X?}"
                )));
            }
            Ok(code)
        })
        .collect()
}

/// 직렬화 package의 page assignment를 문자→2바이트 code map으로 검산한다.
pub(in crate::pipeline) fn quest_page_code_maps(
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<BTreeMap<u8, BTreeMap<char, [u8; 2]>>, QuestManualStreamWriterError> {
    verify_quest_package_repertoire_cells(encoding, package)?;
    let repertoire = package.catalog_glyphs.chars().collect::<Vec<_>>();
    if repertoire.len() != package.plan.glyphs {
        return Err(error(
            "QUEST writer encoding repertoire와 package glyph 수 불일치",
        ));
    }
    let mut result = BTreeMap::new();
    for page in &encoding.pages {
        let indices = package
            .page_catalog_indices(page.page_id)
            .map_err(|source| error(source.to_string()))?;
        if page.glyphs.is_empty() {
            if !indices.is_empty() {
                return Err(error(format!(
                    "QUEST writer 빈 page에 package assignment가 있음: page={}",
                    page.page_id
                )));
            }
            result.insert(page.page_id, BTreeMap::new());
            continue;
        }
        let rows = map_catalog_page_to_codes(&repertoire, &indices, code_table.codes())
            .map_err(|source| error(source.to_string()))?;
        let expected_catalog_text = if Some(page.page_id) == package.menu_font_page {
            crate::font::selected::menu_catalog_text(&page.glyphs)
        } else {
            page.glyphs.clone()
        };
        if rows.iter().map(|row| row.glyph).collect::<String>() != expected_catalog_text {
            return Err(error("QUEST page별 선택 폰트와 catalog identity 불일치"));
        }
        let package_glyphs = rows
            .iter()
            .map(|row| crate::font::selected::menu_glyph(row.glyph).unwrap_or(row.glyph))
            .collect::<String>();
        if package_glyphs != page.glyphs {
            return Err(error(format!(
                "QUEST writer page와 package glyph 순서 불일치: page={}",
                page.page_id
            )));
        }
        let map = rows
            .into_iter()
            .map(|row| {
                (
                    crate::font::selected::menu_glyph(row.glyph).unwrap_or(row.glyph),
                    row.code,
                )
            })
            .collect::<BTreeMap<_, _>>();
        if map.len() != page.glyphs.chars().count() {
            return Err(error(format!(
                "QUEST writer page glyph code 중복: page={}",
                page.page_id
            )));
        }
        result.insert(page.page_id, map);
    }
    Ok(result)
}

#[cfg(test)]
pub(super) fn verify_quest_repertoire_cells(
    repertoire: &str,
    actual_cells: &[Vec<u8>],
) -> Result<(), QuestManualStreamWriterError> {
    let glyphs = repertoire.chars().collect::<BTreeSet<_>>();
    if glyphs.iter().copied().collect::<String>() != repertoire {
        return Err(error(
            "QUEST writer encoding repertoire가 중복되거나 정렬되지 않음",
        ));
    }
    let expected_cells = render_quest_glyph_cache_repertoire_cells(&glyphs)
        .map_err(|source| error(source.to_string()))?;
    if expected_cells != actual_cells {
        return Err(error(
            "QUEST writer encoding Unicode repertoire와 package raster 불일치",
        ));
    }
    Ok(())
}

fn verify_quest_package_repertoire_cells(
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
) -> Result<(), QuestManualStreamWriterError> {
    let actual_cells = (0..package.plan.glyphs)
        .map(|index| {
            let index = u16::try_from(index)
                .map_err(|_| error("QUEST package glyph index가 u16을 초과함"))?;
            package
                .glyph_cell(index)
                .map_err(|source| error(source.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let glyphs = package.catalog_glyphs.chars().collect::<BTreeSet<_>>();
    let semantic = glyphs
        .iter()
        .map(|&c| crate::font::selected::menu_glyph(c).unwrap_or(c))
        .collect::<BTreeSet<_>>();
    if semantic != encoding.repertoire.chars().collect() {
        return Err(error("QUEST 폰트 catalog와 encoding 의미 문자 집합 불일치"));
    }
    if glyphs.iter().copied().collect::<String>() != package.catalog_glyphs
        || glyphs.len() != actual_cells.len()
    {
        return Err(error(
            "QUEST writer encoding repertoire가 중복되었거나 package 분모와 다름",
        ));
    }

    let mut rendered_glyphs = BTreeSet::new();
    let mut rendered_cells = Vec::new();
    for (glyph, cell) in glyphs.iter().copied().zip(&actual_cells) {
        if encoding.source_code(glyph).is_some() {
            verify_source_atlas_cell(glyph, cell)?;
        } else {
            rendered_glyphs.insert(glyph);
            rendered_cells.push(cell.clone());
        }
    }
    if !rendered_glyphs.is_empty() {
        let expected_cells = render_quest_glyph_cache_repertoire_cells(&rendered_glyphs)
            .map_err(|source| error(source.to_string()))?;
        if expected_cells != rendered_cells {
            return Err(error(
                "QUEST writer 선택 폰트 repertoire와 package raster 불일치",
            ));
        }
    }
    Ok(())
}

pub(super) fn verify_source_atlas_cell(
    glyph: char,
    cell: &[u8],
) -> Result<(), QuestManualStreamWriterError> {
    let empty = cell.iter().all(|byte| *byte == 0);
    if glyph == '\u{3000}' {
        if !empty {
            return Err(error(
                "QUEST writer 전각 공백 source atlas cell이 비어 있지 않음",
            ));
        }
    } else if empty {
        return Err(error(format!(
            "QUEST writer source atlas glyph cell이 비었음: {glyph:?}"
        )));
    }
    Ok(())
}
