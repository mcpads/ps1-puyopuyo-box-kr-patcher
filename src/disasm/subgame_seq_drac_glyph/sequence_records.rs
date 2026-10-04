//! DRAC SEQ record, command table, message repertoire 검증.

use std::collections::BTreeSet;

use crate::coords::ExeMap;
use crate::script::{SubgameSeqDracTextAnalysis, SubgameSeqDracTextKind};

use super::{
    error, read_u32, SubgameSeqDracGlyphError, SubgameSeqDracGlyphSpec,
    BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS, BOX_SUBGAME_SEQ_DRAC_SOURCE_UNIQUE_PAIRS,
    BOX_SUBGAME_SEQ_DRAC_SPACE_PAIR_INDICES,
};

pub(super) fn verify_command_table(
    map: &ExeMap,
    text: &[u8],
    spec: SubgameSeqDracGlyphSpec,
) -> Result<(), SubgameSeqDracGlyphError> {
    for (slot, expected) in [
        (1u32, spec.e1_handler_runtime),
        (2, spec.e2_handler_runtime),
    ] {
        let runtime = spec
            .command_source_table_runtime
            .checked_add(slot * 4)
            .ok_or_else(|| error("DRAC glyph command table 주소 overflow"))?;
        let offset = map
            .text_offset(runtime)
            .ok_or_else(|| error(format!("{} DRAC glyph command table이 EXE 밖임", spec.exe)))?;
        let actual = read_u32(text, offset)?;
        if actual != expected {
            return Err(error(format!(
                "{} DRAC E{} handler 불일치: expected=0x{expected:08X}, actual=0x{actual:08X}",
                spec.exe, slot
            )));
        }
    }
    Ok(())
}

pub(super) fn verify_setup_records(
    raw: &[u8],
    spec: SubgameSeqDracGlyphSpec,
    repertoire_runtime: u32,
) -> Result<(), SubgameSeqDracGlyphError> {
    if spec.e2_record_offset + 8 != spec.e1_record_offset {
        return Err(error("DRAC E2/E1 record가 연속하지 않음"));
    }
    let e2 = raw
        .get(spec.e2_record_offset..spec.e2_record_offset + 8)
        .ok_or_else(|| error("DRAC E2 record 범위 이탈"))?;
    let expected_e2 = [
        0xE2,
        0,
        spec.resource_id as u8,
        (spec.resource_id >> 8) as u8,
        spec.auxiliary_id as u8,
        (spec.auxiliary_id >> 8) as u8,
        0,
        0,
    ];
    if e2 != expected_e2 {
        return Err(error(format!("{} DRAC E2 record 불일치", spec.scope)));
    }
    let e1 = raw
        .get(spec.e1_record_offset..spec.e1_record_offset + 8)
        .ok_or_else(|| error("DRAC E1 record 범위 이탈"))?;
    let mut expected_e1 = [0u8; 8];
    expected_e1[0] = 0xE1;
    expected_e1[4..].copy_from_slice(&repertoire_runtime.to_le_bytes());
    if e1 != expected_e1 {
        return Err(error(format!("{} DRAC E1 record 불일치", spec.scope)));
    }
    Ok(())
}

pub(super) fn verify_same_named_resource_record(
    raw: &[u8],
    spec: SubgameSeqDracGlyphSpec,
) -> Result<(), SubgameSeqDracGlyphError> {
    let record = raw
        .get(spec.same_named_e2_record_offset..spec.same_named_e2_record_offset + 8)
        .ok_or_else(|| error("DRAC same-named asset E2 record 범위 이탈"))?;
    let expected = [
        0xE2,
        0,
        spec.same_named_resource_id as u8,
        (spec.same_named_resource_id >> 8) as u8,
        spec.same_named_e2_auxiliary_id as u8,
        (spec.same_named_e2_auxiliary_id >> 8) as u8,
        0,
        0,
    ];
    if record != expected {
        return Err(error(format!(
            "{}:{} same-named asset E2 record 불일치",
            spec.scope, spec.same_named_e2_sequence
        )));
    }
    Ok(())
}

pub(super) fn verify_message_subset(
    drac_text: &SubgameSeqDracTextAnalysis,
    spec: SubgameSeqDracGlyphSpec,
) -> Result<(usize, usize), SubgameSeqDracGlyphError> {
    let repertoire = drac_text
        .rows
        .iter()
        .find(|row| row.scope == spec.scope && row.kind == SubgameSeqDracTextKind::Repertoire)
        .ok_or_else(|| error(format!("{} DRAC repertoire 행 누락", spec.scope)))?;
    let repertoire_chars = repertoire
        .spans
        .iter()
        .flat_map(|span| span.decoded.chars())
        .collect::<Vec<_>>();
    let repertoire_pair_list = repertoire
        .raw
        .chunks_exact(2)
        .map(|pair| [pair[0], pair[1]])
        .collect::<Vec<_>>();
    let repertoire_pairs = repertoire_pair_list
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if repertoire.raw.len() % 2 != 0
        || repertoire_chars.len() != BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS
        || repertoire_pair_list.len() != BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS
        || repertoire_pairs.len() != BOX_SUBGAME_SEQ_DRAC_SOURCE_UNIQUE_PAIRS
    {
        return Err(error(format!(
            "{} DRAC repertoire 2-byte glyph/unique pair 수 불일치",
            spec.scope
        )));
    }
    let space_indices = repertoire_pair_list
        .iter()
        .enumerate()
        .filter_map(|(index, pair)| (*pair == [0x81, 0x40]).then_some(index))
        .collect::<Vec<_>>();
    if space_indices != BOX_SUBGAME_SEQ_DRAC_SPACE_PAIR_INDICES {
        return Err(error(format!(
            "{} DRAC repertoire 전각 공백 중복 위치 불일치",
            spec.scope
        )));
    }
    for row in drac_text
        .rows
        .iter()
        .filter(|row| row.scope == spec.scope && row.kind == SubgameSeqDracTextKind::Message)
    {
        if row.spans.iter().any(|span| span.raw.len() % 2 != 0)
            || row
                .spans
                .iter()
                .flat_map(|span| span.raw.chunks_exact(2))
                .any(|pair| !repertoire_pairs.contains(&[pair[0], pair[1]]))
        {
            return Err(error(format!(
                "{} DRAC message에 repertoire 밖 2-byte pair가 있음",
                spec.scope
            )));
        }
    }
    Ok((repertoire_chars.len(), repertoire_pairs.len()))
}
