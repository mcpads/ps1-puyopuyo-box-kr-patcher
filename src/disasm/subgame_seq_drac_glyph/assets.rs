//! DRAC repertoire가 선택하는 BTX/CNX 원문과 descriptor 결속 검증.

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::{decompress, parse_header};
use crate::coords::ExeMap;

use super::{
    error, hex, read_u32, BtxNameAnalysis, SubgameSeqDracGlyphError, SubgameSeqDracGlyphSpec,
    BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES, BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH,
    BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS, BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X,
    BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y, BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT,
    BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH, BOX_SUBGAME_SEQ_DRAC_DESCRIPTOR_STRIDE,
    BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NamedGlyphAssetSpec {
    asset: &'static str,
    resource_id: u16,
    vdd_index: usize,
    stored_bytes: u32,
    plain_sha256: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadedNamedGlyphAsset {
    descriptor_runtime: u32,
    plain: Vec<u8>,
    plain_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VerifiedDracAssets {
    pub(super) descriptor_runtime: u32,
    pub(super) auxiliary_descriptor_runtime: u32,
    pub(super) selected_plain: Vec<u8>,
    pub(super) selected_plain_sha256: String,
    pub(super) same_named_descriptor_runtime: u32,
    pub(super) same_named_plain_sha256: String,
    pub(super) equal_cells: usize,
}

fn load_named_asset(
    map: &ExeMap,
    text: &[u8],
    archive: &VddArchive,
    btx_names: &BtxNameAnalysis,
    spec: SubgameSeqDracGlyphSpec,
    named: NamedGlyphAssetSpec,
) -> Result<LoadedNamedGlyphAsset, SubgameSeqDracGlyphError> {
    let links = btx_names
        .links
        .iter()
        .filter(|link| {
            link.exe == spec.exe
                && link.archive == spec.grp_archive
                && link.asset_name == named.asset
        })
        .collect::<Vec<_>>();
    let [link] = links.as_slice() else {
        return Err(error(format!(
            "{}:{} BTX 이름 연결 수가 1이 아님: {}",
            spec.exe,
            named.asset,
            links.len()
        )));
    };
    if link.descriptor_runtime_start != spec.descriptor_base_runtime {
        return Err(error(format!("{} DRAC descriptor base 불일치", spec.exe)));
    }
    let descriptor_runtime = spec
        .descriptor_base_runtime
        .checked_add(u32::from(named.resource_id) * BOX_SUBGAME_SEQ_DRAC_DESCRIPTOR_STRIDE)
        .ok_or_else(|| error("DRAC descriptor runtime overflow"))?;
    if link.descriptor_pointer_runtime != descriptor_runtime + 4 {
        return Err(error(format!(
            "{} resource {}가 {} descriptor를 고르지 않음",
            spec.exe, named.resource_id, named.asset
        )));
    }

    let entry = archive
        .entries()
        .get(named.vdd_index)
        .ok_or_else(|| error(format!("{} VDD index 범위 이탈", spec.grp_archive)))?;
    if entry.name() != named.asset || entry.size != named.stored_bytes {
        return Err(error(format!(
            "{}:{} VDD 행 불일치",
            spec.grp_archive, named.vdd_index
        )));
    }
    let cnx = archive.read_file(entry);
    let header = parse_header(cnx).map_err(|source| {
        error(format!(
            "{}:{} CNX header 실패: {source}",
            spec.grp_archive, named.asset
        ))
    })?;
    if header.subtype != *b"bi_"
        || header.decompressed_size as usize != BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES
    {
        return Err(error(format!(
            "{}:{} BTX 형식/해제 길이 불일치",
            spec.grp_archive, named.asset
        )));
    }
    let plain = decompress(cnx).map_err(|source| {
        error(format!(
            "{}:{} CNX 해제 실패: {source}",
            spec.grp_archive, named.asset
        ))
    })?;
    let plain_sha256 = hex(&Sha256::digest(&plain));
    if plain.len() != BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES || plain_sha256 != named.plain_sha256 {
        return Err(error(format!(
            "{}:{} BTX 원문 불일치",
            spec.grp_archive, named.asset
        )));
    }

    let descriptor_offset = map
        .text_offset(descriptor_runtime)
        .ok_or_else(|| error(format!("{} DRAC descriptor가 EXE 밖임", spec.exe)))?;
    let expected_words = [
        0x0001_0000,
        link.runtime_addr,
        named.stored_bytes,
        BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES as u32,
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
                "{}:{} descriptor word {} 불일치: expected=0x{expected:08X}, actual=0x{actual:08X}",
                spec.exe, named.asset, index
            )));
        }
    }
    Ok(LoadedNamedGlyphAsset {
        descriptor_runtime,
        plain,
        plain_sha256,
    })
}

pub(super) fn equal_glyph_cells(
    selected: &[u8],
    same_named: &[u8],
) -> Result<usize, SubgameSeqDracGlyphError> {
    let row_bytes = usize::from(BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH) / 2;
    let cell_row_bytes = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH) / 2;
    if selected.len() != BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES
        || same_named.len() != BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES
    {
        return Err(error("DRAC cell 비교 atlas 길이 불일치"));
    }
    let mut equal = 0usize;
    for index in 0..BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS {
        let cell_x = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X)
            + index % BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS
                * usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH);
        let cell_y = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y)
            + index / BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS
                * usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT);
        let matches = (0..usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT)).all(|row| {
            let start = (cell_y + row) * row_bytes + cell_x / 2;
            selected[start..start + cell_row_bytes] == same_named[start..start + cell_row_bytes]
        });
        equal += usize::from(matches);
    }
    Ok(equal)
}

pub(super) fn verify_assets(
    map: &ExeMap,
    text: &[u8],
    archive: &VddArchive,
    btx_names: &BtxNameAnalysis,
    spec: SubgameSeqDracGlyphSpec,
) -> Result<VerifiedDracAssets, SubgameSeqDracGlyphError> {
    let selected = load_named_asset(
        map,
        text,
        archive,
        btx_names,
        spec,
        NamedGlyphAssetSpec {
            asset: spec.asset,
            resource_id: spec.resource_id,
            vdd_index: spec.asset_vdd_index,
            stored_bytes: spec.asset_stored_bytes,
            plain_sha256: spec.asset_plain_sha256,
        },
    )?;
    let same_named = load_named_asset(
        map,
        text,
        archive,
        btx_names,
        spec,
        NamedGlyphAssetSpec {
            asset: spec.same_named_asset,
            resource_id: spec.same_named_resource_id,
            vdd_index: spec.same_named_asset_vdd_index,
            stored_bytes: spec.same_named_asset_stored_bytes,
            plain_sha256: spec.same_named_asset_plain_sha256,
        },
    )?;

    let auxiliary_links = btx_names
        .links
        .iter()
        .filter(|link| {
            link.exe == spec.exe
                && link.archive == spec.grp_archive
                && link.asset_name == spec.auxiliary_asset
        })
        .collect::<Vec<_>>();
    let [auxiliary_link] = auxiliary_links.as_slice() else {
        return Err(error(format!(
            "{}:{} 보조 BTX 이름 연결 수가 1이 아님: {}",
            spec.exe,
            spec.auxiliary_asset,
            auxiliary_links.len()
        )));
    };
    let auxiliary_descriptor_runtime = spec
        .descriptor_base_runtime
        .checked_add(u32::from(spec.auxiliary_id) * BOX_SUBGAME_SEQ_DRAC_DESCRIPTOR_STRIDE)
        .ok_or_else(|| error("DRAC auxiliary descriptor runtime overflow"))?;
    if auxiliary_link.descriptor_runtime_start != spec.descriptor_base_runtime
        || auxiliary_link.descriptor_pointer_runtime != auxiliary_descriptor_runtime + 4
        || auxiliary_descriptor_runtime == selected.descriptor_runtime
        || spec.auxiliary_asset == spec.asset
    {
        return Err(error(format!(
            "{} resource {}가 별도 보조 자산 {} descriptor를 고르지 않음",
            spec.exe, spec.auxiliary_id, spec.auxiliary_asset
        )));
    }

    let equal_cells = equal_glyph_cells(&selected.plain, &same_named.plain)?;
    if equal_cells != spec.selected_same_named_equal_cells {
        return Err(error(format!(
            "{} selected/same-named DRAC cell 일치 수 불일치: expected={}, actual={equal_cells}",
            spec.exe, spec.selected_same_named_equal_cells
        )));
    }
    Ok(VerifiedDracAssets {
        descriptor_runtime: selected.descriptor_runtime,
        auxiliary_descriptor_runtime,
        selected_plain: selected.plain,
        selected_plain_sha256: selected.plain_sha256,
        same_named_descriptor_runtime: same_named.descriptor_runtime,
        same_named_plain_sha256: same_named.plain_sha256,
        equal_cells,
    })
}
