//! fixed-cell 승격 provenance의 원본 descriptor와 SEQ cell-table 공급 후보.
//!
//! GRP descriptor의 초기 `+0x1C/+0x20`은 cell table과 개수를 직접
//! 소유하지 않는다. 이 모듈은 exact descriptor 행에서 resource ID를
//! 계산하고, 같은 서브게임의 모든 overlay에서 `OP28/sub12` setup record를
//! 찾아 그 resource에 연결한다. 정적 record는 실제 실행·draw 증거가 아니며,
//! record가 없다는 결과도 runtime 생성 경로의 부재를 뜻하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::btx::{Layout, PixelFormat};
use crate::coords::ExeMap;
use crate::disasm::{BtxNameAnalysis, BtxNameLink};
use crate::graphics::btx_classification::P2_INFO2_CLASSIFICATION;
use crate::graphics::text_candidates::{
    BOX_MENU_STATIC_TEXT_PROVENANCE, BOX_PROMOTED_CELL_GRID_PROVENANCE,
};
use crate::graphics::text_catalog::P2RULED1_TEXT_ASSET;
use crate::script::{SubgameSeqLayoutAnalysis, SubgameSeqLayoutKind};

pub const BTX_PENDING_STATIC_CELLS_SCHEMA_VERSION: &str = "btx-pending-static-cells-v1";
pub const BOX_BTX_PENDING_STATIC_CELLS_REPORT_SHA256: [u8; 32] = [
    0x13, 0xA9, 0xCE, 0x12, 0xE3, 0x07, 0xDC, 0x95, 0x41, 0x8C, 0x43, 0xD5, 0x73, 0xE1, 0xF1, 0x65,
    0x2B, 0x84, 0xDC, 0x11, 0xD9, 0x2C, 0x5F, 0x28, 0xF5, 0x62, 0x69, 0x4C, 0x22, 0xF7, 0xB3, 0xBC,
];

const BOX_PENDING_FINDINGS: usize = 2;
const BOX_PENDING_PHYSICAL_ASSETS: usize = 4;
const BOX_PENDING_SETUP_RECORDS: usize = 6;
const BOX_PENDING_ROWS_WITH_SETUP: usize = 4;
const BOX_PENDING_LAYOUT_MATCHES: usize = 2;
const BOX_PENDING_PLAIN_LAYOUT_MATCHES: usize = 2;

const BOX_P2_INFO2_FINDINGS: usize = 1;
const BOX_P2_INFO2_PHYSICAL_ASSETS: usize = 2;
const BOX_P2_INFO2_SETUP_RECORDS: usize = 3;
const BOX_P2_INFO2_ROWS_WITH_SETUP: usize = 2;
const BOX_P2_INFO2_LAYOUT_MATCHES: usize = 1;
const BOX_P2_INFO2_PLAIN_LAYOUT_MATCHES: usize = 1;

const BOX_P2RULED1_FINDINGS: usize = 1;
const BOX_P2RULED1_PHYSICAL_ASSETS: usize = 2;
const BOX_P2RULED1_SETUP_RECORDS: usize = 5;

const BOX_MENU_STATIC_FINDINGS: usize = 7;
const BOX_MENU_STATIC_PHYSICAL_ASSETS: usize = 14;
const BOX_MENU_STATIC_SETUP_RECORDS: usize = 18;
const BOX_MENU_STATIC_ROWS_WITH_SETUP: usize = 14;

const DESCRIPTOR_BYTES: usize = 36;
const DESCRIPTOR_STRIDE: u32 = 36;
const CELL_HEADER_BYTES: usize = 12;
const SETUP_RECORD_BYTES: usize = 16;
const SETUP_PREFIX: [u8; 4] = [0x28, 0x12, 0x97, 0x00];

const BOX_ARCHIVE_PAIRS: [(&str, &str); 2] = [
    ("/PUYO1/PP1_GRP.VDD;1", "/PUYO1/PP1_SEQ.VDD;1"),
    ("/PUYO2/PP2_GRP.VDD;1", "/PUYO2/PP2_SEQ.VDD;1"),
];

/// 정적 descriptor와 SEQ cell setup을 교차할 최소 자산 계약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxStaticCellAssetSpec<'a> {
    pub finding_id: &'a str,
    pub archive_paths: &'a [&'a str],
    pub name: &'a str,
    pub review_layout: Layout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxPendingStaticSetup {
    pub sequence: String,
    pub record_offset: usize,
    pub record_runtime: u32,
    pub table_offset: usize,
    pub table_runtime: u32,
    pub record_cell_count: u16,
    pub atlas_width: u16,
    pub atlas_height: u16,
    pub review_layout_matches: bool,
    pub descriptor_plain_layout_matches: bool,
    pub next_referenced_table_offset: Option<usize>,
    pub next_referenced_overlay_offset: Option<usize>,
    pub next_overlay_cell_span: Option<BtxPendingStaticCellSpan>,
    pub inferred_cell_records: Option<usize>,
    pub table_padding_bytes: Option<usize>,
    pub valid_cells: Option<usize>,
    pub empty_cells: Option<usize>,
    pub outside_cells: Option<usize>,
    pub inferred_cells: Vec<BtxPendingStaticCellRecord>,
    pub leading_cell_prefix: Option<BtxPendingStaticCellPrefix>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxPendingStaticCellRecord {
    pub index: usize,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub anchor_x: i16,
    pub anchor_y: i16,
    pub empty: bool,
    pub outside_atlas: bool,
}

/// 다음 cell table이 없을 때 가까운 overlay 참조로 보수적으로 닫은 cell span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxPendingStaticCellSpan {
    pub records: usize,
    pub padding_bytes: usize,
    pub valid_cells: usize,
    pub empty_cells: usize,
    pub outside_cells: usize,
    pub cells: Vec<BtxPendingStaticCellRecord>,
}

/// 다음 setup table까지 다른 명령이 섞인 경우에도 첫 empty-width record 앞에서 닫힌 prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxPendingStaticCellPrefix {
    pub valid_cells: Vec<BtxPendingStaticCellRecord>,
    pub boundary_record: BtxPendingStaticCellRecord,
    pub protected_tail_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxPendingStaticCellRow {
    pub finding_id: String,
    pub exe: &'static str,
    pub grp_archive: &'static str,
    pub seq_archive: &'static str,
    pub asset_name: String,
    pub resource_id: u16,
    pub descriptor_runtime: u32,
    pub descriptor_stored_bytes: u32,
    pub descriptor_plain_bytes: u32,
    pub initial_cell_table_runtime: u32,
    pub initial_cell_count: u16,
    pub setups: Vec<BtxPendingStaticSetup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxPendingStaticCellAnalysis {
    pub rows: Vec<BtxPendingStaticCellRow>,
    pub findings: usize,
    pub physical_assets: usize,
    pub setup_records: usize,
    pub rows_with_setup: usize,
    pub layout_matching_setup_records: usize,
    pub plain_layout_matching_setup_records: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxPendingStaticCellError(String);

impl fmt::Display for BtxPendingStaticCellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for BtxPendingStaticCellError {}

fn error(message: impl Into<String>) -> BtxPendingStaticCellError {
    BtxPendingStaticCellError(message.into())
}

fn exact_link<'a>(
    names: &'a BtxNameAnalysis,
    archive: &str,
    asset_name: &str,
) -> Result<&'a BtxNameLink, BtxPendingStaticCellError> {
    let links = names
        .links
        .iter()
        .filter(|link| link.archive == archive && link.asset_name == asset_name)
        .collect::<Vec<_>>();
    match links.as_slice() {
        [link] => Ok(*link),
        _ => Err(error(format!(
            "pending BTX 이름표 연결 수가 1이 아님: {archive}:{asset_name}: {}",
            links.len()
        ))),
    }
}

pub(super) fn descriptor_entry_runtime(
    link: &BtxNameLink,
) -> Result<u32, BtxPendingStaticCellError> {
    link.descriptor_pointer_runtime
        .checked_sub(4)
        .ok_or_else(|| error(format!("{} descriptor +4 주소 underflow", link.asset_name)))
}

pub(super) fn resource_id(
    link: &BtxNameLink,
    descriptor_runtime: u32,
) -> Result<u16, BtxPendingStaticCellError> {
    let relative = descriptor_runtime
        .checked_sub(link.descriptor_runtime_start)
        .ok_or_else(|| {
            error(format!(
                "{} descriptor가 block 선두보다 앞임",
                link.asset_name
            ))
        })?;
    if !relative.is_multiple_of(DESCRIPTOR_STRIDE) {
        return Err(error(format!(
            "{} descriptor가 36바이트 resource 격자에 없음: +0x{relative:X}",
            link.asset_name
        )));
    }
    u16::try_from(relative / DESCRIPTOR_STRIDE)
        .map_err(|_| error(format!("{} resource ID가 u16을 넘음", link.asset_name)))
}

fn executable_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    runtime: u32,
    len: usize,
) -> Result<&'a [u8], BtxPendingStaticCellError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "{} runtime 주소가 EXE text 밖임: 0x{runtime:08X}",
            map.name
        ))
    })?;
    text.get(offset..offset + len).ok_or_else(|| {
        error(format!(
            "{} EXE text 범위 이탈: 0x{runtime:08X}+{len}",
            map.name
        ))
    })
}

fn read_u16(raw: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([raw[offset], raw[offset + 1]])
}

fn read_i16(raw: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([raw[offset], raw[offset + 1]])
}

fn read_u32(raw: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(raw[offset..offset + 4].try_into().expect("4바이트 slice"))
}

pub(super) fn verify_initial_descriptor(
    map: &ExeMap,
    text: &[u8],
    link: &BtxNameLink,
    descriptor_runtime: u32,
) -> Result<(u32, u32, u32, u16), BtxPendingStaticCellError> {
    let raw = executable_slice(map, text, descriptor_runtime, DESCRIPTOR_BYTES)?;
    let words = (0..9)
        .map(|index| read_u32(raw, index * 4))
        .collect::<Vec<_>>();
    if words[0] != 0x0001_0000
        || words[1] != link.runtime_addr
        || words[2] == 0
        || words[3] == 0
        || words[4..].iter().any(|word| *word != 0)
    {
        return Err(error(format!(
            "{}:{} 초기 descriptor 계약 불일치: {words:08X?}",
            map.name, link.asset_name
        )));
    }
    Ok((words[2], words[3], words[7], words[8] as u16))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SetupScanContext {
    pub initialized_len: usize,
    pub load_base: u32,
    pub resource_id: u16,
    pub review_width: usize,
    pub review_height: usize,
    pub format: PixelFormat,
    pub descriptor_plain_bytes: u32,
}

pub(super) fn scan_setup_records(
    sequence: &str,
    raw: &[u8],
    context: SetupScanContext,
) -> Result<Vec<BtxPendingStaticSetup>, BtxPendingStaticCellError> {
    let SetupScanContext {
        initialized_len,
        load_base,
        resource_id,
        review_width,
        review_height,
        format,
        descriptor_plain_bytes,
    } = context;
    let scan_len = initialized_len.min(raw.len());
    if scan_len < SETUP_RECORD_BYTES {
        return Ok(Vec::new());
    }
    let table_offsets = referenced_table_offsets(raw, scan_len, load_base);
    let overlay_offsets = referenced_overlay_offsets(raw, scan_len, load_base);
    let mut setups = Vec::new();
    for record_offset in (0..=scan_len - SETUP_RECORD_BYTES).step_by(4) {
        let record = &raw[record_offset..record_offset + SETUP_RECORD_BYTES];
        if record[..4] != SETUP_PREFIX || read_u16(record, 4) != resource_id {
            continue;
        }
        let record_cell_count = read_u16(record, 6);
        let reserved = read_u32(record, 8);
        if reserved != 0 {
            return Err(error(format!(
                "{sequence} resource {resource_id} setup reserved가 0이 아님: +0x{record_offset:04X}"
            )));
        }
        let table_runtime = read_u32(record, 12);
        let table_offset = table_runtime
            .checked_sub(load_base)
            .and_then(|offset| usize::try_from(offset).ok())
            .filter(|offset| *offset + CELL_HEADER_BYTES <= scan_len)
            .ok_or_else(|| {
                error(format!(
                    "{sequence} resource {resource_id} cell table이 initialized overlay 밖임: 0x{table_runtime:08X}"
                ))
            })?;
        let header = &raw[table_offset..table_offset + CELL_HEADER_BYTES];
        let origin_x = read_u16(header, 0);
        let origin_y = read_u16(header, 2);
        let atlas_width = read_u16(header, 4);
        let atlas_height = read_u16(header, 6);
        let anchor_x = read_i16(header, 8);
        let anchor_y = read_i16(header, 10);
        if origin_x != 0
            || origin_y != 0
            || atlas_width == 0
            || atlas_height == 0
            || anchor_x != 0
            || anchor_y != 0
        {
            return Err(error(format!(
                "{sequence} resource {resource_id} cell header 불일치: \
                 ({origin_x},{origin_y},{atlas_width},{atlas_height},{anchor_x},{anchor_y})"
            )));
        }
        let record_runtime = load_base
            .checked_add(u32::try_from(record_offset).map_err(|_| error("record offset overflow"))?)
            .ok_or_else(|| error("record runtime overflow"))?;
        let next_referenced_table_offset =
            table_offsets.range((table_offset + 1)..).next().copied();
        let next_referenced_overlay_offset =
            overlay_offsets.range((table_offset + 1)..).next().copied();
        let inferred = next_referenced_table_offset
            .and_then(|next| infer_table_span(raw, table_offset, next, atlas_width, atlas_height));
        let next_overlay_cell_span = next_referenced_table_offset
            .is_none()
            .then(|| {
                next_referenced_overlay_offset.and_then(|next| {
                    infer_table_span(raw, table_offset, next, atlas_width, atlas_height)
                })
            })
            .flatten();
        let leading_cell_prefix = inferred
            .as_ref()
            .is_none_or(|span| span.outside_cells != 0)
            .then(|| {
                next_referenced_table_offset.and_then(|next| {
                    infer_leading_cell_prefix(raw, table_offset, next, atlas_width, atlas_height)
                })
            })
            .flatten();
        setups.push(BtxPendingStaticSetup {
            sequence: sequence.to_string(),
            record_offset,
            record_runtime,
            table_offset,
            table_runtime,
            record_cell_count,
            atlas_width,
            atlas_height,
            review_layout_matches: usize::from(atlas_width) == review_width
                && usize::from(atlas_height) == review_height,
            descriptor_plain_layout_matches: Layout {
                width: usize::from(atlas_width),
                height: usize::from(atlas_height),
                format,
            }
            .byte_len()
            .ok()
            .and_then(|bytes| u32::try_from(bytes).ok())
                == Some(descriptor_plain_bytes),
            next_referenced_table_offset,
            next_referenced_overlay_offset,
            next_overlay_cell_span,
            inferred_cell_records: inferred.as_ref().map(|span| span.records),
            table_padding_bytes: inferred.as_ref().map(|span| span.padding_bytes),
            valid_cells: inferred.as_ref().map(|span| span.valid_cells),
            empty_cells: inferred.as_ref().map(|span| span.empty_cells),
            outside_cells: inferred.as_ref().map(|span| span.outside_cells),
            inferred_cells: inferred.map(|span| span.cells).unwrap_or_default(),
            leading_cell_prefix,
        });
    }
    Ok(setups)
}

fn parse_cell_record(
    raw: &[u8],
    offset: usize,
    index: usize,
    atlas_width: u16,
    atlas_height: u16,
) -> Option<BtxPendingStaticCellRecord> {
    let cell = raw.get(offset..offset + 12)?;
    let x = read_u16(cell, 0);
    let y = read_u16(cell, 2);
    let width = read_u16(cell, 4);
    let height = read_u16(cell, 6);
    let anchor_x = read_i16(cell, 8);
    let anchor_y = read_i16(cell, 10);
    let empty = width == 0 || height == 0;
    let outside_atlas = !empty
        && (u32::from(x) + u32::from(width) > u32::from(atlas_width)
            || u32::from(y) + u32::from(height) > u32::from(atlas_height));
    Some(BtxPendingStaticCellRecord {
        index,
        x,
        y,
        width,
        height,
        anchor_x,
        anchor_y,
        empty,
        outside_atlas,
    })
}

/// full span을 cell table로 볼 수 없을 때 첫 empty record까지의 보수적 prefix만 찾는다.
fn infer_leading_cell_prefix(
    raw: &[u8],
    table_offset: usize,
    limit: usize,
    atlas_width: u16,
    atlas_height: u16,
) -> Option<BtxPendingStaticCellPrefix> {
    let mut offset = table_offset.checked_add(CELL_HEADER_BYTES)?;
    let mut valid_cells = Vec::new();
    let mut index = 0usize;
    while offset.checked_add(12)? <= limit {
        let cell = parse_cell_record(raw, offset, index, atlas_width, atlas_height)?;
        if cell.empty {
            if valid_cells.is_empty() {
                return None;
            }
            return Some(BtxPendingStaticCellPrefix {
                valid_cells,
                boundary_record: cell,
                protected_tail_bytes: limit.checked_sub(offset + 12)?,
            });
        }
        if cell.outside_atlas {
            return None;
        }
        valid_cells.push(cell);
        offset += 12;
        index += 1;
    }
    None
}

fn referenced_table_offsets(raw: &[u8], scan_len: usize, load_base: u32) -> BTreeSet<usize> {
    if scan_len < SETUP_RECORD_BYTES {
        return BTreeSet::new();
    }
    (0..=scan_len - SETUP_RECORD_BYTES)
        .step_by(4)
        .filter_map(|offset| {
            let record = &raw[offset..offset + SETUP_RECORD_BYTES];
            if record[..4] != SETUP_PREFIX || read_u32(record, 8) != 0 {
                return None;
            }
            let table_runtime = read_u32(record, 12);
            let table_offset = table_runtime.checked_sub(load_base)? as usize;
            let table_end = table_offset.checked_add(CELL_HEADER_BYTES)?;
            if table_end > scan_len {
                return None;
            }
            let header = raw.get(table_offset..table_end)?;
            (read_u16(header, 0) == 0
                && read_u16(header, 2) == 0
                && read_u16(header, 4) != 0
                && read_u16(header, 6) != 0
                && read_i16(header, 8) == 0
                && read_i16(header, 10) == 0)
                .then_some(table_offset)
        })
        .collect()
}

/// overlay 안을 가리키는 4바이트 정렬 포인터의 target offset을 모은다.
///
/// cell setup이 마지막 table을 가리켜 뒤쪽 table이 없을 때, 다음 command
/// stream 같은 별도 구조의 시작을 보수적인 상한으로 사용한다.
fn referenced_overlay_offsets(raw: &[u8], scan_len: usize, load_base: u32) -> BTreeSet<usize> {
    (0..scan_len.saturating_sub(3))
        .step_by(4)
        .filter_map(|offset| {
            let target = read_u32(raw, offset);
            let target_offset = target.checked_sub(load_base)? as usize;
            (target_offset < scan_len).then_some(target_offset)
        })
        .collect()
}

fn infer_table_span(
    raw: &[u8],
    table_offset: usize,
    next_table_offset: usize,
    atlas_width: u16,
    atlas_height: u16,
) -> Option<BtxPendingStaticCellSpan> {
    let span = next_table_offset.checked_sub(table_offset)?;
    let payload = span.checked_sub(CELL_HEADER_BYTES)?;
    let padding_bytes = payload % 12;
    let records = payload / 12;
    if records == 0 {
        return None;
    }
    let records_end = table_offset + CELL_HEADER_BYTES + records * 12;
    if raw
        .get(records_end..next_table_offset)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return None;
    }
    let mut valid_cells = 0usize;
    let mut empty_cells = 0usize;
    let mut outside_cells = 0usize;
    let mut cells = Vec::with_capacity(records);
    for index in 0..records {
        let offset = table_offset + CELL_HEADER_BYTES + index * 12;
        let cell = parse_cell_record(raw, offset, index, atlas_width, atlas_height)?;
        let empty = cell.empty;
        let outside_atlas = cell.outside_atlas;
        if empty {
            empty_cells += 1;
        } else if outside_atlas {
            outside_cells += 1;
        } else {
            valid_cells += 1;
        }
        cells.push(cell);
    }
    Some(BtxPendingStaticCellSpan {
        records,
        padding_bytes,
        valid_cells,
        empty_cells,
        outside_cells,
        cells,
    })
}

/// 임의의 자산 계약과 원본 EXE descriptor·PP1/PP2 overlay setup을 교차한다.
pub fn audit_btx_static_cells_for_specs<'a>(
    specs: &[BtxStaticCellAssetSpec<'static>],
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    names: &BtxNameAnalysis,
) -> Result<BtxPendingStaticCellAnalysis, BtxPendingStaticCellError> {
    let executable_map = executables
        .into_iter()
        .map(|(map, text)| (map.name, (map, text)))
        .collect::<BTreeMap<_, _>>();
    let source_map = sources
        .into_iter()
        .map(|(archive, sequence, raw)| ((archive, sequence), raw))
        .collect::<BTreeMap<_, _>>();

    let mut rows = Vec::new();
    for spec in specs {
        for &grp_archive in spec.archive_paths {
            let seq_archive = BOX_ARCHIVE_PAIRS
                .iter()
                .find_map(|(grp, seq)| (*grp == grp_archive).then_some(*seq))
                .ok_or_else(|| error(format!("pending GRP의 SEQ owner 누락: {grp_archive}")))?;
            let link = exact_link(names, grp_archive, spec.name)?;
            let &(map, text) = executable_map
                .get(link.exe)
                .ok_or_else(|| error(format!("{} EXE 입력 누락", link.exe)))?;
            let descriptor_runtime = descriptor_entry_runtime(link)?;
            let resource_id = resource_id(link, descriptor_runtime)?;
            let (stored_bytes, plain_bytes, initial_cell_table_runtime, initial_cell_count) =
                verify_initial_descriptor(map, text, link, descriptor_runtime)?;

            let mut setups = Vec::new();
            for layout in layouts.rows.iter().filter(|layout| {
                layout.archive == seq_archive && layout.kind == SubgameSeqLayoutKind::Overlay
            }) {
                let raw = source_map
                    .get(&(layout.archive.as_str(), layout.sequence.as_str()))
                    .ok_or_else(|| {
                        error(format!(
                            "layout 원본 누락: {}:{}",
                            layout.archive, layout.sequence
                        ))
                    })?;
                let footer = layout
                    .footer
                    .ok_or_else(|| error("overlay layout의 footer 누락"))?;
                setups.extend(scan_setup_records(
                    &layout.sequence,
                    raw,
                    SetupScanContext {
                        initialized_len: usize::try_from(footer.initialized_len)
                            .map_err(|_| error("initialized 길이 overflow"))?,
                        load_base: footer.load_base,
                        resource_id,
                        review_width: spec.review_layout.width,
                        review_height: spec.review_layout.height,
                        format: spec.review_layout.format,
                        descriptor_plain_bytes: plain_bytes,
                    },
                )?);
            }
            setups.sort_by(|left, right| {
                left.sequence
                    .cmp(&right.sequence)
                    .then_with(|| left.record_offset.cmp(&right.record_offset))
            });
            rows.push(BtxPendingStaticCellRow {
                finding_id: spec.finding_id.to_string(),
                exe: link.exe,
                grp_archive,
                seq_archive,
                asset_name: spec.name.to_string(),
                resource_id,
                descriptor_runtime,
                descriptor_stored_bytes: stored_bytes,
                descriptor_plain_bytes: plain_bytes,
                initial_cell_table_runtime,
                initial_cell_count,
                setups,
            });
        }
    }
    rows.sort_by(|left, right| {
        left.finding_id
            .cmp(&right.finding_id)
            .then_with(|| left.exe.cmp(right.exe))
    });
    let findings = rows
        .iter()
        .map(|row| row.finding_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let setup_records = rows.iter().map(|row| row.setups.len()).sum();
    let rows_with_setup = rows.iter().filter(|row| !row.setups.is_empty()).count();
    let layout_matching_setup_records = rows
        .iter()
        .flat_map(|row| &row.setups)
        .filter(|setup| setup.review_layout_matches)
        .count();
    let plain_layout_matching_setup_records = rows
        .iter()
        .flat_map(|row| &row.setups)
        .filter(|setup| setup.descriptor_plain_layout_matches)
        .count();
    Ok(BtxPendingStaticCellAnalysis {
        findings,
        physical_assets: rows.len(),
        setup_records,
        rows_with_setup,
        layout_matching_setup_records,
        plain_layout_matching_setup_records,
        rows,
    })
}

/// fixed-cell 승격 provenance와 원본 setup을 교차한다.
pub fn audit_btx_pending_static_cells<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    names: &BtxNameAnalysis,
) -> Result<BtxPendingStaticCellAnalysis, BtxPendingStaticCellError> {
    let specs = BOX_PROMOTED_CELL_GRID_PROVENANCE
        .iter()
        .map(|spec| BtxStaticCellAssetSpec {
            finding_id: spec.finding_id,
            archive_paths: spec.archive_paths,
            name: spec.name,
            review_layout: spec.review_layout,
        })
        .collect::<Vec<_>>();
    audit_btx_static_cells_for_specs(&specs, executables, sources, layouts, names)
}

fn verify_initial_cell_fields_zero(
    analysis: &BtxPendingStaticCellAnalysis,
) -> Result<(), BtxPendingStaticCellError> {
    if let Some(row) = analysis
        .rows
        .iter()
        .find(|row| row.initial_cell_table_runtime != 0 || row.initial_cell_count != 0)
    {
        return Err(error(format!(
            "{}:{} 원본 descriptor cell field가 0이 아님: pointer=0x{:08X}, count={}",
            row.exe, row.asset_name, row.initial_cell_table_runtime, row.initial_cell_count
        )));
    }
    Ok(())
}

/// 지원 원본의 2 finding·4 physical descriptor와 6 setup을 고정한다.
pub fn verify_box_btx_pending_static_cells<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    names: &BtxNameAnalysis,
) -> Result<BtxPendingStaticCellAnalysis, BtxPendingStaticCellError> {
    let analysis = audit_btx_pending_static_cells(executables, sources, layouts, names)?;
    for (field, expected, actual) in [
        ("finding", BOX_PENDING_FINDINGS, analysis.findings),
        (
            "physical asset",
            BOX_PENDING_PHYSICAL_ASSETS,
            analysis.physical_assets,
        ),
        (
            "setup record",
            BOX_PENDING_SETUP_RECORDS,
            analysis.setup_records,
        ),
        (
            "setup 보유 physical row",
            BOX_PENDING_ROWS_WITH_SETUP,
            analysis.rows_with_setup,
        ),
        (
            "review layout 일치 setup",
            BOX_PENDING_LAYOUT_MATCHES,
            analysis.layout_matching_setup_records,
        ),
        (
            "descriptor plain layout 일치 setup",
            BOX_PENDING_PLAIN_LAYOUT_MATCHES,
            analysis.plain_layout_matching_setup_records,
        ),
    ] {
        if actual != expected {
            return Err(error(format!(
                "지원 원본 pending BTX static {field} 분모 불일치: \
                 expected={expected}, actual={actual}"
            )));
        }
    }
    verify_initial_cell_fields_zero(&analysis)?;
    let report = render_btx_pending_static_cells_tsv(&analysis);
    let actual_digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if actual_digest != BOX_BTX_PENDING_STATIC_CELLS_REPORT_SHA256 {
        return Err(error(format!(
            "지원 원본 pending BTX static report SHA-256 불일치: \
             expected={:02x?}, actual={actual_digest:02x?}",
            BOX_BTX_PENDING_STATIC_CELLS_REPORT_SHA256
        )));
    }
    Ok(analysis)
}

/// `P2_INFO2`를 text가 아닌 sprite-cell 표본으로 판정한 정적 소비 계약.
pub fn verify_box_p2_info2_static_cells<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    names: &BtxNameAnalysis,
) -> Result<BtxPendingStaticCellAnalysis, BtxPendingStaticCellError> {
    let specs = [BtxStaticCellAssetSpec {
        finding_id: P2_INFO2_CLASSIFICATION.sample_id,
        archive_paths: P2_INFO2_CLASSIFICATION.archive_paths,
        name: P2_INFO2_CLASSIFICATION.name,
        review_layout: P2_INFO2_CLASSIFICATION.layout,
    }];
    let analysis = audit_btx_static_cells_for_specs(&specs, executables, sources, layouts, names)?;
    for (field, expected, actual) in [
        ("finding", BOX_P2_INFO2_FINDINGS, analysis.findings),
        (
            "physical asset",
            BOX_P2_INFO2_PHYSICAL_ASSETS,
            analysis.physical_assets,
        ),
        (
            "setup record",
            BOX_P2_INFO2_SETUP_RECORDS,
            analysis.setup_records,
        ),
        (
            "setup 보유 physical row",
            BOX_P2_INFO2_ROWS_WITH_SETUP,
            analysis.rows_with_setup,
        ),
        (
            "review layout 일치 setup",
            BOX_P2_INFO2_LAYOUT_MATCHES,
            analysis.layout_matching_setup_records,
        ),
        (
            "descriptor plain layout 일치 setup",
            BOX_P2_INFO2_PLAIN_LAYOUT_MATCHES,
            analysis.plain_layout_matching_setup_records,
        ),
    ] {
        if actual != expected {
            return Err(error(format!(
                "지원 원본 P2_INFO2 static {field} 분모 불일치: \
                 expected={expected}, actual={actual}"
            )));
        }
    }
    verify_initial_cell_fields_zero(&analysis)?;

    let pp1 = analysis
        .rows
        .iter()
        .find(|row| row.exe == "PUYO1.EXE")
        .ok_or_else(|| error("P2_INFO2 PUYO1 행 누락"))?;
    if pp1.resource_id != 145
        || pp1.descriptor_plain_bytes != 12_672
        || pp1.setups.len() != 2
        || pp1.setups.iter().any(|setup| {
            setup.atlas_width != 256
                || setup.atlas_height != 176
                || setup.review_layout_matches
                || setup.descriptor_plain_layout_matches
        })
    {
        return Err(error(format!(
            "P2_INFO2 PUYO1 상충 setup 계약 불일치: resource={}, plain={}, setups={:?}",
            pp1.resource_id, pp1.descriptor_plain_bytes, pp1.setups
        )));
    }

    let pp2 = analysis
        .rows
        .iter()
        .find(|row| row.exe == "PUYO2.EXE")
        .ok_or_else(|| error("P2_INFO2 PUYO2 행 누락"))?;
    let setup = pp2
        .setups
        .first()
        .ok_or_else(|| error("P2_INFO2 PUYO2 setup 누락"))?;
    if pp2.resource_id != 58
        || pp2.descriptor_plain_bytes != 12_672
        || pp2.setups.len() != 1
        || setup.sequence != "PP2_TTL.SEQ"
        || setup.record_offset != 0x16AC
        || setup.table_offset != 0x18FC
        || setup.atlas_width != 144
        || setup.atlas_height != 176
        || !setup.review_layout_matches
        || !setup.descriptor_plain_layout_matches
        || setup.next_referenced_table_offset != Some(0x1A44)
        || setup.inferred_cell_records != Some(26)
        || setup.table_padding_bytes != Some(4)
        || setup.valid_cells != Some(26)
        || setup.empty_cells != Some(0)
        || setup.outside_cells != Some(0)
    {
        return Err(error(format!(
            "P2_INFO2 PUYO2 exact setup 계약 불일치: resource={}, plain={}, setup={setup:?}",
            pp2.resource_id, pp2.descriptor_plain_bytes
        )));
    }
    Ok(analysis)
}

/// `P2RULED1` 두 물리 사본과 다섯 exact 256×176 setup을 고정한다.
pub fn verify_box_p2ruled1_static_cells<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    names: &BtxNameAnalysis,
) -> Result<BtxPendingStaticCellAnalysis, BtxPendingStaticCellError> {
    let specs = [BtxStaticCellAssetSpec {
        finding_id: "BTXF-013",
        archive_paths: P2RULED1_TEXT_ASSET.archive_paths,
        name: P2RULED1_TEXT_ASSET.name,
        review_layout: P2RULED1_TEXT_ASSET.layout,
    }];
    let analysis = audit_btx_static_cells_for_specs(&specs, executables, sources, layouts, names)?;
    for (field, expected, actual) in [
        ("finding", BOX_P2RULED1_FINDINGS, analysis.findings),
        (
            "physical asset",
            BOX_P2RULED1_PHYSICAL_ASSETS,
            analysis.physical_assets,
        ),
        (
            "setup record",
            BOX_P2RULED1_SETUP_RECORDS,
            analysis.setup_records,
        ),
        (
            "setup 보유 physical row",
            BOX_P2RULED1_PHYSICAL_ASSETS,
            analysis.rows_with_setup,
        ),
        (
            "review layout 일치 setup",
            BOX_P2RULED1_SETUP_RECORDS,
            analysis.layout_matching_setup_records,
        ),
        (
            "descriptor plain layout 일치 setup",
            BOX_P2RULED1_SETUP_RECORDS,
            analysis.plain_layout_matching_setup_records,
        ),
    ] {
        if actual != expected {
            return Err(error(format!(
                "지원 원본 P2RULED1 static {field} 분모 불일치: expected={expected}, actual={actual}"
            )));
        }
    }
    verify_initial_cell_fields_zero(&analysis)?;

    for row in &analysis.rows {
        let (expected_resource, expected_setups): (u16, &[(&str, usize, usize)]) = match row.exe {
            "PUYO1.EXE" => (
                123,
                &[
                    ("MBAN_SEQ.SEQ", 0x1108, 0x0E40),
                    ("MENU2.SEQ", 0x41D4, 0x6250),
                    ("MENU2.SEQ", 0x4760, 0x6250),
                ],
            ),
            "PUYO2.EXE" => (
                91,
                &[("MENU2.SEQ", 0x41B8, 0x6234), ("MENU2.SEQ", 0x4744, 0x6234)],
            ),
            _ => return Err(error(format!("P2RULED1 static의 예상 밖 EXE: {}", row.exe))),
        };
        let actual_setups = row
            .setups
            .iter()
            .map(|setup| {
                (
                    setup.sequence.as_str(),
                    setup.record_offset,
                    setup.table_offset,
                )
            })
            .collect::<Vec<_>>();
        if row.resource_id != expected_resource
            || row.descriptor_plain_bytes != 22_528
            || actual_setups != expected_setups
            || row
                .setups
                .iter()
                .filter(|setup| setup.sequence == "MENU2.SEQ")
                .any(|setup| {
                    setup.atlas_width != 256
                        || setup.atlas_height != 176
                        || setup.next_referenced_table_offset
                            != Some(if row.exe == "PUYO1.EXE" {
                                0x6490
                            } else {
                                0x6474
                            })
                        || setup.inferred_cell_records != Some(47)
                        || setup.valid_cells != Some(45)
                        || setup.empty_cells != Some(2)
                        || setup.outside_cells != Some(0)
                })
        {
            return Err(error(format!(
                "P2RULED1 {} exact setup 계약 불일치: resource={}, plain={}, setups={:?}",
                row.exe, row.resource_id, row.descriptor_plain_bytes, row.setups
            )));
        }
    }
    Ok(analysis)
}

/// PUYO1/PUYO2 공용 menu family 7개의 exact atlas setup 계약.
pub fn verify_box_menu_static_cells<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    names: &BtxNameAnalysis,
) -> Result<BtxPendingStaticCellAnalysis, BtxPendingStaticCellError> {
    const MENU_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
    const MENU04_RECORDS: [usize; 3] = [0x23F4, 0x29DC, 0x2AE8];
    const MENU01_RECORDS: [usize; 1] = [0x01B4];
    const MENU03_RECORDS: [usize; 1] = [0x186C];
    const MENU06_RECORDS: [usize; 1] = [0x2D00];
    const MENU201_RECORDS: [usize; 1] = [0x00FC];
    const MENU202_RECORDS: [usize; 1] = [0x0F1C];
    const MENU203_PP1_RECORDS: [usize; 1] = [0x1E08];
    const MENU203_PP2_RECORDS: [usize; 1] = [0x1E18];

    let mut specs = BOX_MENU_STATIC_TEXT_PROVENANCE
        .iter()
        .map(|spec| BtxStaticCellAssetSpec {
            finding_id: spec.finding_id,
            archive_paths: spec.archive_paths,
            name: spec.name,
            review_layout: spec.review_layout,
        })
        .collect::<Vec<_>>();
    specs.extend([
        BtxStaticCellAssetSpec {
            finding_id: "BTXS-MENU03",
            archive_paths: &MENU_ARCHIVES,
            name: "MENU03.BTX",
            review_layout: Layout {
                width: 768,
                height: 256,
                format: crate::btx::PixelFormat::Indexed4,
            },
        },
        BtxStaticCellAssetSpec {
            finding_id: "BTXS-MENU04",
            archive_paths: &MENU_ARCHIVES,
            name: "MENU04.BTX",
            review_layout: Layout {
                width: 256,
                height: 128,
                format: crate::btx::PixelFormat::Indexed4,
            },
        },
    ]);
    let analysis = audit_btx_static_cells_for_specs(&specs, executables, sources, layouts, names)?;
    for (field, expected, actual) in [
        ("finding", BOX_MENU_STATIC_FINDINGS, analysis.findings),
        (
            "physical asset",
            BOX_MENU_STATIC_PHYSICAL_ASSETS,
            analysis.physical_assets,
        ),
        (
            "setup record",
            BOX_MENU_STATIC_SETUP_RECORDS,
            analysis.setup_records,
        ),
        (
            "setup 보유 physical row",
            BOX_MENU_STATIC_ROWS_WITH_SETUP,
            analysis.rows_with_setup,
        ),
        (
            "review layout 일치 setup",
            BOX_MENU_STATIC_SETUP_RECORDS,
            analysis.layout_matching_setup_records,
        ),
        (
            "descriptor plain layout 일치 setup",
            BOX_MENU_STATIC_SETUP_RECORDS,
            analysis.plain_layout_matching_setup_records,
        ),
    ] {
        if actual != expected {
            return Err(error(format!(
                "지원 원본 menu static {field} 분모 불일치: expected={expected}, actual={actual}"
            )));
        }
    }
    verify_initial_cell_fields_zero(&analysis)?;

    for row in &analysis.rows {
        let is_pp1 = row.exe == "PUYO1.EXE";
        if !is_pp1 && row.exe != "PUYO2.EXE" {
            return Err(error(format!("menu static의 예상 밖 EXE: {}", row.exe)));
        }
        let (
            expected_resource,
            expected_plain,
            expected_sequence,
            expected_record_offsets,
            expected_table,
            expected_next,
            expected_records,
            expected_valid,
            expected_empty,
        ) = match row.asset_name.as_str() {
            "MENU01.BTX" => (
                if is_pp1 { 110 } else { 78 },
                65_536,
                "MENU1.SEQ",
                MENU01_RECORDS.as_slice(),
                0x30D8,
                Some(0x32DC),
                Some(42),
                Some(42),
                Some(0),
            ),
            "MENU03.BTX" => (
                if is_pp1 { 112 } else { 80 },
                98_304,
                "MENU1.SEQ",
                MENU03_RECORDS.as_slice(),
                0x36D8,
                Some(0x3864),
                Some(32),
                Some(32),
                Some(0),
            ),
            "MENU04.BTX" => (
                if is_pp1 { 113 } else { 81 },
                16_384,
                "MENU1.SEQ",
                MENU04_RECORDS.as_slice(),
                0x3864,
                Some(0x3B34),
                Some(59),
                Some(55),
                Some(4),
            ),
            "MENU06.BTX" => (
                if is_pp1 { 114 } else { 82 },
                65_536,
                "MENU1.SEQ",
                MENU06_RECORDS.as_slice(),
                0x3B34,
                None,
                None,
                None,
                None,
            ),
            "MENU201.BTX" => (
                if is_pp1 { 115 } else { 83 },
                65_536,
                "MENU2.SEQ",
                MENU201_RECORDS.as_slice(),
                if is_pp1 { 0x4C6C } else { 0x4C50 },
                Some(if is_pp1 { 0x544C } else { 0x5430 }),
                Some(167),
                Some(151),
                Some(16),
            ),
            "MENU202.BTX" => (
                if is_pp1 { 116 } else { 84 },
                98_304,
                "MENU2.SEQ",
                MENU202_RECORDS.as_slice(),
                if is_pp1 { 0x544C } else { 0x5430 },
                Some(if is_pp1 { 0x57E8 } else { 0x57CC }),
                Some(76),
                Some(75),
                Some(1),
            ),
            "MENU203.BTX" => (
                if is_pp1 { 117 } else { 85 },
                32_768,
                "MENU2.SEQ",
                if is_pp1 {
                    MENU203_PP1_RECORDS.as_slice()
                } else {
                    MENU203_PP2_RECORDS.as_slice()
                },
                if is_pp1 { 0x57E8 } else { 0x57CC },
                Some(if is_pp1 { 0x5AAC } else { 0x5A90 }),
                Some(58),
                Some(56),
                Some(2),
            ),
            other => return Err(error(format!("menu static의 예상 밖 자산: {other}"))),
        };
        let actual_record_offsets = row
            .setups
            .iter()
            .map(|setup| setup.record_offset)
            .collect::<Vec<_>>();
        if row.resource_id != expected_resource
            || row.descriptor_plain_bytes != expected_plain
            || actual_record_offsets != expected_record_offsets
            || row.setups.iter().any(|setup| {
                setup.sequence != expected_sequence
                    || setup.table_offset != expected_table
                    || setup.next_referenced_table_offset != expected_next
                    || setup.inferred_cell_records != expected_records
                    || setup.table_padding_bytes != expected_records.map(|_| 0)
                    || setup.valid_cells != expected_valid
                    || setup.empty_cells != expected_empty
                    || setup.outside_cells != expected_records.map(|_| 0)
                    || !setup.review_layout_matches
                    || !setup.descriptor_plain_layout_matches
            })
        {
            return Err(error(format!(
                "menu static exact 계약 불일치: {}:{} resource={} plain={} setups={:?}",
                row.exe, row.asset_name, row.resource_id, row.descriptor_plain_bytes, row.setups
            )));
        }
    }
    Ok(analysis)
}

/// 자산마다 setup이 없을 때도 한 행을 남기는 결정적 TSV.
pub fn render_btx_pending_static_cells_tsv(analysis: &BtxPendingStaticCellAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tfinding_id\texe\tgrp_archive\tseq_archive\tasset_name\tresource_id\t\
         descriptor_runtime\tdescriptor_stored_bytes\tdescriptor_plain_bytes\t\
         initial_cell_table_runtime\tinitial_cell_count\tsequence\trecord_offset\t\
         record_runtime\ttable_offset\ttable_runtime\trecord_cell_count\tatlas_width\t\
         atlas_height\treview_layout_matches\tdescriptor_plain_layout_matches\t\
         next_referenced_table_offset\t\
         inferred_cell_records\ttable_padding_bytes\tvalid_cells\tempty_cells\t\
         outside_cells\tinferred_cells\tstatic_verdict\tclaim_boundary\n",
    );
    for row in &analysis.rows {
        if row.setups.is_empty() {
            let mut fields = vec![
                BTX_PENDING_STATIC_CELLS_SCHEMA_VERSION.to_string(),
                row.finding_id.clone(),
                row.exe.to_string(),
                row.grp_archive.to_string(),
                row.seq_archive.to_string(),
                row.asset_name.clone(),
                row.resource_id.to_string(),
                format!("0x{:08X}", row.descriptor_runtime),
                row.descriptor_stored_bytes.to_string(),
                row.descriptor_plain_bytes.to_string(),
                format!("0x{:08X}", row.initial_cell_table_runtime),
                row.initial_cell_count.to_string(),
            ];
            fields.extend((0..17).map(|_| String::new()));
            fields.push("no-canonical-overlay-setup-found".to_string());
            fields.push("static-source-only-runtime-generated-route-unverified".to_string());
            output.push_str(&fields.join("\t"));
            output.push('\n');
            continue;
        }
        for setup in &row.setups {
            let verdict = if setup.review_layout_matches && setup.descriptor_plain_layout_matches {
                "canonical-overlay-setup-layout-match"
            } else if setup.descriptor_plain_layout_matches {
                "canonical-overlay-setup-corrected-layout"
            } else {
                "canonical-overlay-setup-plain-layout-mismatch"
            };
            let fields = [
                BTX_PENDING_STATIC_CELLS_SCHEMA_VERSION.to_string(),
                row.finding_id.clone(),
                row.exe.to_string(),
                row.grp_archive.to_string(),
                row.seq_archive.to_string(),
                row.asset_name.clone(),
                row.resource_id.to_string(),
                format!("0x{:08X}", row.descriptor_runtime),
                row.descriptor_stored_bytes.to_string(),
                row.descriptor_plain_bytes.to_string(),
                format!("0x{:08X}", row.initial_cell_table_runtime),
                row.initial_cell_count.to_string(),
                setup.sequence.clone(),
                format!("0x{:04X}", setup.record_offset),
                format!("0x{:08X}", setup.record_runtime),
                format!("0x{:04X}", setup.table_offset),
                format!("0x{:08X}", setup.table_runtime),
                setup.record_cell_count.to_string(),
                setup.atlas_width.to_string(),
                setup.atlas_height.to_string(),
                setup.review_layout_matches.to_string(),
                setup.descriptor_plain_layout_matches.to_string(),
                optional_offset(setup.next_referenced_table_offset),
                optional_usize(setup.inferred_cell_records),
                optional_usize(setup.table_padding_bytes),
                optional_usize(setup.valid_cells),
                optional_usize(setup.empty_cells),
                optional_usize(setup.outside_cells),
                render_cells(&setup.inferred_cells),
                verdict.to_string(),
                "static-source-only-execution-draw-screen-unverified".to_string(),
            ];
            output.push_str(&fields.join("\t"));
            output.push('\n');
        }
    }
    output
}

fn render_cells(cells: &[BtxPendingStaticCellRecord]) -> String {
    cells
        .iter()
        .map(|cell| {
            let status = if cell.empty {
                "empty"
            } else if cell.outside_atlas {
                "outside"
            } else {
                "valid"
            };
            format!(
                "{}:{},{},{},{},{},{},{}",
                cell.index,
                cell.x,
                cell.y,
                cell.width,
                cell.height,
                cell.anchor_x,
                cell.anchor_y,
                status
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn optional_offset(value: Option<usize>) -> String {
    value
        .map(|offset| format!("0x{offset:04X}"))
        .unwrap_or_default()
}

fn optional_usize(value: Option<usize>) -> String {
    value.map(|number| number.to_string()).unwrap_or_default()
}

#[cfg(test)]
#[path = "btx_pending_static_cells_tests.rs"]
mod tests;
