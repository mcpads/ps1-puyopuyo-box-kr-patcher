//! E6 glyph BTX 자산, setup record와 cell table 검증.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::btx::{decode as decode_btx, Layout as BtxLayout, PixelFormat};
use crate::compress::{decompress, parse_header};
use crate::coords::ExeMap;
use crate::font::DemoFontSlot;

use super::super::BtxNameAnalysis;
use super::binary_checks::{read_i16, read_u16, read_u32, runtime_slice};
use super::{
    error, hex, GlyphAssetContract, P1CandidateCoverage, SetupRecord, SubgameSeqE6GlyphError,
    SubgameSeqE6GlyphSpec, BOX_SUBGAME_SEQ_E6_GLYPH_PUYO1_CANDIDATE_VALID_PREFIX,
    BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_CELLS, CELL_BYTES, CELL_HEADER_BYTES, CODE_TABLE_BYTES,
    CODE_TABLE_RUNTIME, CONTROLS, DESCRIPTOR_BYTES, P1_CANDIDATE_PREFIX_SHA256,
    P2_CELL_TABLE_BYTES, P2_TABLE_SHA256, SETUP_PREFIX, TERMINATOR,
};

pub(super) fn verify_asset(
    map: &ExeMap,
    text: &[u8],
    archive: &VddArchive,
    names: &BtxNameAnalysis,
    spec: SubgameSeqE6GlyphSpec,
    asset: GlyphAssetContract,
) -> Result<(String, Vec<u8>), SubgameSeqE6GlyphError> {
    let links = names
        .links
        .iter()
        .filter(|link| {
            link.exe == spec.exe
                && link.archive == spec.grp_archive
                && link.asset_name == asset.name
        })
        .collect::<Vec<_>>();
    let [link] = links.as_slice() else {
        return Err(error(format!(
            "{}:{} E6 glyph 이름 연결 수가 1이 아님: {}",
            spec.exe,
            asset.name,
            links.len()
        )));
    };
    let descriptor_runtime = spec
        .descriptor_base_runtime
        .checked_add(u32::from(asset.resource) * DESCRIPTOR_BYTES)
        .ok_or_else(|| error("E6 glyph descriptor overflow"))?;
    if link.descriptor_runtime_start != spec.descriptor_base_runtime
        || link.descriptor_pointer_runtime != descriptor_runtime + 4
    {
        return Err(error(format!(
            "{} resource {}가 {} descriptor를 고르지 않음",
            spec.exe, asset.resource, asset.name
        )));
    }
    let entry = archive
        .entries()
        .get(asset.vdd_index)
        .ok_or_else(|| error(format!("{} VDD index 범위 이탈", spec.grp_archive)))?;
    if entry.name() != asset.name || entry.size != asset.stored_bytes {
        return Err(error(format!(
            "{}:{} E6 glyph VDD 행 불일치",
            spec.grp_archive, asset.vdd_index
        )));
    }
    let cnx = archive.read_file(entry);
    let header = parse_header(cnx).map_err(|source| {
        error(format!(
            "{}:{} E6 glyph CNX header 실패: {source}",
            spec.grp_archive, asset.name
        ))
    })?;
    if header.subtype != *b"bi_" || header.decompressed_size as usize != asset.plain_bytes {
        return Err(error(format!(
            "{}:{} E6 glyph BTX 형식/길이 불일치",
            spec.grp_archive, asset.name
        )));
    }
    let plain = decompress(cnx).map_err(|source| {
        error(format!(
            "{}:{} E6 glyph CNX 해제 실패: {source}",
            spec.grp_archive, asset.name
        ))
    })?;
    let digest = hex(&Sha256::digest(&plain));
    if plain.len() != asset.plain_bytes || digest != asset.plain_sha256 {
        return Err(error(format!(
            "{}:{} E6 glyph 원문 불일치",
            spec.grp_archive, asset.name
        )));
    }
    let descriptor_offset = map
        .text_offset(descriptor_runtime)
        .ok_or_else(|| error("E6 glyph descriptor가 EXE 밖임"))?;
    let expected_words = [
        0x0001_0000,
        link.runtime_addr,
        asset.stored_bytes,
        asset.plain_bytes as u32,
        0,
        0,
        0,
        0,
        0,
    ];
    for (index, expected) in expected_words.into_iter().enumerate() {
        let actual = read_u32(text, descriptor_offset + index * 4)?;
        if actual != expected {
            return Err(error(format!(
                "{}:{} descriptor word {index} 불일치",
                spec.exe, asset.name
            )));
        }
    }
    Ok((digest, plain))
}
pub(super) fn setup_records(raw: &[u8], initialized_len: usize) -> Vec<SetupRecord> {
    let scan_len = initialized_len.min(raw.len());
    if scan_len < 16 {
        return Vec::new();
    }
    (0..=scan_len - 16)
        .step_by(4)
        .filter(|offset| raw.get(*offset..*offset + 4) == Some(SETUP_PREFIX.as_slice()))
        .map(|offset| SetupRecord {
            offset,
            resource: u16::from_le_bytes([raw[offset + 4], raw[offset + 5]]),
            count: u16::from_le_bytes([raw[offset + 6], raw[offset + 7]]),
            reserved: u32::from_le_bytes(
                raw[offset + 8..offset + 12]
                    .try_into()
                    .expect("cell setup reserved"),
            ),
            pointer: u32::from_le_bytes(
                raw[offset + 12..offset + 16]
                    .try_into()
                    .expect("cell setup pointer"),
            ),
        })
        .collect()
}

pub(super) fn cell(
    raw: &[u8],
    offset: usize,
) -> Result<(u16, u16, u16, u16, i16, i16), SubgameSeqE6GlyphError> {
    Ok((
        read_u16(raw, offset)?,
        read_u16(raw, offset + 2)?,
        read_u16(raw, offset + 4)?,
        read_u16(raw, offset + 6)?,
        read_i16(raw, offset + 8)?,
        read_i16(raw, offset + 10)?,
    ))
}

pub(super) fn verify_p2_table(raw: &[u8], offset: usize) -> Result<String, SubgameSeqE6GlyphError> {
    let header = cell(raw, offset)?;
    if header != (0, 0, 512, 256, 0, 0) {
        return Err(error(format!(
            "PUYO2 E6 512x256 cell header 불일치: {header:?}"
        )));
    }
    for index in 0..BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_CELLS {
        let actual = cell(raw, offset + CELL_HEADER_BYTES + index * CELL_BYTES)?;
        let physical = DemoFontSlot::for_index(index)
            .map_err(|source| error(format!("PUYO2 E6 physical cell 실패: {source}")))?;
        let expected = (
            u16::try_from(physical.x).expect("cell x 범위"),
            u16::try_from(physical.y).expect("cell y 범위"),
            u16::try_from(physical.width).expect("cell width 범위"),
            u16::try_from(physical.height).expect("cell height 범위"),
            0,
            0,
        );
        if actual != expected {
            return Err(error(format!(
                "PUYO2 E6 cell {index} 불일치: expected={expected:?}, actual={actual:?}"
            )));
        }
    }
    let table = raw
        .get(offset..offset + P2_CELL_TABLE_BYTES)
        .ok_or_else(|| error("PUYO2 E6 cell table 범위 이탈"))?;
    let digest = hex(&Sha256::digest(table));
    if digest != P2_TABLE_SHA256 {
        return Err(error(format!(
            "PUYO2 E6 cell table digest 불일치: {digest}"
        )));
    }
    Ok(digest)
}

pub(super) fn valid_rect(cell: (u16, u16, u16, u16, i16, i16), width: u16, height: u16) -> bool {
    cell.2 != 0
        && cell.3 != 0
        && cell
            .0
            .checked_add(cell.2)
            .is_some_and(|right| right <= width)
        && cell
            .1
            .checked_add(cell.3)
            .is_some_and(|bottom| bottom <= height)
}

pub(super) fn verify_p1_candidate(
    raw: &[u8],
    offset: usize,
) -> Result<(usize, String), SubgameSeqE6GlyphError> {
    let header = cell(raw, offset)?;
    if header != (0, 0, 1008, 232, 0, 0) {
        return Err(error(format!(
            "PUYO1 CHR 1008x232 cell header 불일치: {header:?}"
        )));
    }
    let mut valid_prefix = 0usize;
    loop {
        let entry = cell(raw, offset + CELL_HEADER_BYTES + valid_prefix * CELL_BYTES)?;
        if !valid_rect(entry, header.2, header.3) {
            break;
        }
        valid_prefix += 1;
    }
    if valid_prefix != BOX_SUBGAME_SEQ_E6_GLYPH_PUYO1_CANDIDATE_VALID_PREFIX {
        return Err(error(format!(
            "PUYO1 CHR candidate 정상 prefix 불일치: {valid_prefix}"
        )));
    }
    let prefix_bytes = CELL_HEADER_BYTES + valid_prefix * CELL_BYTES;
    let digest = hex(&Sha256::digest(
        raw.get(offset..offset + prefix_bytes)
            .ok_or_else(|| error("PUYO1 CHR candidate prefix 범위 이탈"))?,
    ));
    if digest != P1_CANDIDATE_PREFIX_SHA256 {
        return Err(error(format!(
            "PUYO1 CHR candidate prefix digest 불일치: {digest}"
        )));
    }
    Ok((valid_prefix, digest))
}

pub(super) fn verify_p1_candidate_coverage(
    raw: &[u8],
    offset: usize,
    used_indices: &BTreeSet<usize>,
    glyph_plain: &[u8],
) -> Result<P1CandidateCoverage, SubgameSeqE6GlyphError> {
    let header = cell(raw, offset)?;
    if header != (0, 0, 1008, 232, 0, 0) {
        return Err(error("PUYO1 CHR candidate coverage header 불일치"));
    }
    let layout = BtxLayout {
        width: usize::from(header.2),
        height: usize::from(header.3),
        format: PixelFormat::Indexed4,
    };
    let pixels = decode_btx(glyph_plain, layout)
        .map_err(|source| error(format!("PUYO1 CHR candidate BTX decode 실패: {source}")))?;
    let mut valid_cells = 0usize;
    let mut invalid_indices = Vec::new();
    let mut nonempty_cells = 0usize;
    let mut empty_indices = Vec::new();
    for &index in used_indices {
        let entry = cell(raw, offset + CELL_HEADER_BYTES + index * CELL_BYTES)?;
        if !valid_rect(entry, header.2, header.3) {
            invalid_indices.push(index);
            continue;
        }
        valid_cells += 1;
        let (x, y, width, height) = (
            usize::from(entry.0),
            usize::from(entry.1),
            usize::from(entry.2),
            usize::from(entry.3),
        );
        let nonempty = (y..y + height).any(|row| {
            let start = row * layout.width + x;
            pixels[start..start + width].iter().any(|value| *value != 0)
        });
        if nonempty {
            nonempty_cells += 1;
        } else {
            empty_indices.push(index);
        }
    }
    if valid_cells + invalid_indices.len() != used_indices.len()
        || nonempty_cells + empty_indices.len() != valid_cells
    {
        return Err(error("PUYO1 CHR candidate coverage 분모 불일치"));
    }
    Ok(P1CandidateCoverage {
        valid_cells,
        invalid_indices,
        nonempty_cells,
        empty_indices,
    })
}

pub(super) fn code_table_indices(
    map: &ExeMap,
    text: &[u8],
    rows: impl Iterator<Item = impl AsRef<[u8]>>,
) -> Result<(usize, BTreeSet<usize>), SubgameSeqE6GlyphError> {
    let table = runtime_slice(map, text, CODE_TABLE_RUNTIME, CODE_TABLE_BYTES)?
        .chunks_exact(2)
        .map(|pair| [pair[0], pair[1]])
        .collect::<Vec<_>>();
    let mut tokens = 0usize;
    let mut indices = BTreeSet::new();
    for raw in rows {
        let raw = raw.as_ref();
        let mut terminated = false;
        for pair in raw.chunks_exact(2) {
            let pair = [pair[0], pair[1]];
            if pair == TERMINATOR {
                terminated = true;
                break;
            }
            if CONTROLS.contains(&pair) {
                continue;
            }
            let index = table
                .iter()
                .position(|entry| *entry == pair)
                .ok_or_else(|| {
                    error(format!(
                        "E6 glyph code table pair 누락: {:02X}{:02X}",
                        pair[0], pair[1]
                    ))
                })?;
            tokens += 1;
            indices.insert(index);
        }
        if !terminated {
            return Err(error("E6 glyph stream terminator 누락"));
        }
    }
    Ok((tokens, indices))
}
