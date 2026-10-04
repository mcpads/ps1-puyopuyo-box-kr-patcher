//! PCOMI custom byte의 direct cell과 실제 BTX atlas를 잇는 원본 계약.
//!
//! custom byte consumer는 byte를 그대로 object의 cell field에 쓴다. 이
//! 모듈은 같은 object의 resource field가 `4`임을 검산하고, PCOMI renderer의
//! `resource × 36` lookup을 통해 `PCOMI03.BTX` descriptor를 고른다. 이어
//! `PCOMI1.SEQ`의 `OP28/sub12` setup, 311개 cell record와 원본 CNX 픽셀을
//! 함께 검증한다. 정상 실행에서 해당 UI가 보였는지는 이 정적 경계 밖이다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha1::{Digest as _, Sha1};
use sha2::Sha256;

use crate::archive::VddArchive;
use crate::btx::{decode, Layout, PixelFormat};
use crate::compress::{decompress, parse_header};
use crate::coords::{ExeMap, RuntimeAssetMap};
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::SubgameSeqCustomByteRecordAnalysis;

use super::{
    linear, BtxCellAnalysis, BtxNameAnalysis, BtxRendererFamily, PcomiCustomByteConsumerAnalysis,
    PCOMI_CUSTOM_BYTE_DIRECT_CELL_STORE_RUNTIME, PCOMI_CUSTOM_BYTE_RENDERER_EXTENDED_RUNTIME,
    PCOMI_CUSTOM_BYTE_RENDERER_SHORT_RUNTIME, PCOMI_CUSTOM_BYTE_UI_CONSUMER_RUNTIME,
};

pub const PCOMI_CUSTOM_BYTE_ATLAS_RESOURCE: u16 = 4;
pub const PCOMI_CUSTOM_BYTE_DESCRIPTOR_RUNTIME: u32 = 0x8002_F478;
pub const PCOMI_CUSTOM_BYTE_ATLAS_WIDTH: usize = 768;
pub const PCOMI_CUSTOM_BYTE_ATLAS_HEIGHT: usize = 256;
pub const PCOMI_CUSTOM_BYTE_CELL_RECORDS: usize = 311;

const PCOMI_DESCRIPTOR_ROOT_FIELD_RUNTIME: u32 = 0x8007_7888;
const PCOMI_DESCRIPTOR_ROOT_RUNTIME: u32 = 0x8002_F3E8;
const PCOMI_CUSTOM_BYTE_OBJECT_GP_OFFSET: i16 = 0x01A4;
const PCOMI_CUSTOM_BYTE_OBJECT_CREATE_RUNTIME: u32 = 0x8002_2170;
const PCOMI_CUSTOM_BYTE_OBJECT_HANDOFF_RUNTIME: u32 = 0x8002_34B4;
const PCOMI_CUSTOM_BYTE_SETUP_RUNTIME: u32 = 0x801A_6378;
const PCOMI_CUSTOM_BYTE_SETUP_METADATA_RUNTIME: u32 = 0x801A_6390;
const PCOMI_CUSTOM_BYTE_CELL_TABLE_RUNTIME: u32 = 0x801A_B9D4;
const PCOMI_NEXT_CELL_TABLE_RUNTIME: u32 = 0x801A_C874;
const PCOMI_NESTED_DISPATCH_TABLE_RUNTIME: u32 = 0x8005_01D8;
const PCOMI_CELL_TABLE_HANDLER_RUNTIME: u32 = 0x8001_2A2C;
const PCOMI1_LOAD_BASE: u32 = 0x801A_6270;
const PCOMI03_NAME_RUNTIME: u32 = 0x8002_F6E7;
const PCOMI03_ARCHIVE: &str = "/PCOMI/PC1_GRP.VDD;1";
const PCOMI03_ASSET: &str = "PCOMI03.BTX";
const PCOMI03_ARCHIVE_INDEX: usize = 3;
const PCOMI03_STORED_BYTES: usize = 52_171;
const PCOMI03_PLAIN_BYTES: usize = 98_304;
const PCOMI03_PLAIN_SHA1: [u8; 20] = [
    0x9A, 0x07, 0x5C, 0x38, 0xAE, 0x91, 0xAC, 0xA3, 0xBD, 0x10, 0x94, 0xB2, 0xAB, 0x12, 0x51, 0x94,
    0xBD, 0xEE, 0x15, 0x3A,
];
const PCOMI_CELL_TABLE_SHA256: [u8; 32] = [
    0x91, 0xF5, 0xFC, 0x9E, 0xD6, 0x9C, 0xB3, 0xA6, 0x56, 0x66, 0xE3, 0xAD, 0x33, 0xC1, 0x9C, 0x0A,
    0xD6, 0x31, 0x18, 0x4C, 0x39, 0xB9, 0xA7, 0x03, 0xE0, 0x6C, 0x78, 0x19, 0x2D, 0x3D, 0x0A, 0x37,
];
const PCOMI_EMPTY_CELL_INDICES: [usize; 4] = [274, 275, 287, 288];
const PCOMI_BLANK_PIXEL_CODES: [u8; 1] = [0x20];
const PCOMI_CUSTOM_BYTE_SETUP: [u8; 16] = [
    0x28, 0x12, 0x97, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xD4, 0xB9, 0x1A, 0x80,
];
const PCOMI_CUSTOM_BYTE_SETUP_METADATA: [u8; 16] = [
    0x28, 0x12, 0x88, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct PcomiCell {
    index: usize,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    empty: bool,
    outside_atlas: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcomiCustomByteAtlasAnalysis {
    pub resource: u16,
    pub descriptor_runtime: u32,
    pub asset_name: &'static str,
    pub archive_index: usize,
    pub stored_bytes: usize,
    pub plain_bytes: usize,
    pub plain_sha1: [u8; 20],
    pub atlas_width: usize,
    pub atlas_height: usize,
    pub cell_table_runtime: u32,
    pub cell_records: usize,
    pub empty_cells: usize,
    pub repertoire_codes: usize,
    pub used_codes: usize,
    pub repertoire_nonempty_pixel_cells: usize,
    pub used_nonempty_pixel_cells: usize,
    pub repertoire_blank_pixel_cells: usize,
    pub used_blank_pixel_cells: usize,
    pub renderer_short_runtime: u32,
    pub renderer_extended_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcomiCustomByteAtlasError(String);

impl fmt::Display for PcomiCustomByteAtlasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for PcomiCustomByteAtlasError {}

fn error(message: impl Into<String>) -> PcomiCustomByteAtlasError {
    PcomiCustomByteAtlasError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, PcomiCustomByteAtlasError> {
    instructions
        .binary_search_by_key(&address, |instruction| instruction.addr)
        .ok()
        .map(|index| &instructions[index])
        .ok_or_else(|| error(format!("PCOMI atlas 명령 주소 없음: 0x{address:08X}")))
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: K,
) -> Result<(), PcomiCustomByteAtlasError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "PCOMI atlas 명령 불일치: address=0x{address:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn read_text_word(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
) -> Result<u32, PcomiCustomByteAtlasError> {
    let offset = map
        .text_offset(runtime)
        .ok_or_else(|| error(format!("PCOMI atlas EXE 주소 범위 이탈: 0x{runtime:08X}")))?;
    read_u32(text, offset)
}

fn read_u16(raw: &[u8], offset: usize) -> Result<u16, PcomiCustomByteAtlasError> {
    let bytes = raw
        .get(offset..offset + 2)
        .ok_or_else(|| error(format!("PCOMI atlas u16 범위 이탈: 0x{offset:X}")))?;
    Ok(u16::from_le_bytes(bytes.try_into().expect("2바이트 u16")))
}

fn read_u32(raw: &[u8], offset: usize) -> Result<u32, PcomiCustomByteAtlasError> {
    let bytes = raw
        .get(offset..offset + 4)
        .ok_or_else(|| error(format!("PCOMI atlas u32 범위 이탈: 0x{offset:X}")))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 u32")))
}

fn verify_analysis_boundaries<'a>(
    storage: &'a SubgameSeqCustomByteRecordAnalysis,
    consumer: &PcomiCustomByteConsumerAnalysis,
) -> Result<&'a crate::script::SubgameSeqCustomByteRecordRow, PcomiCustomByteAtlasError> {
    let [row] = storage.rows.as_slice() else {
        return Err(error("PCOMI atlas storage 행 수 불일치"));
    };
    if row.scope != "PCOMI"
        || row.archive != "/PCOMI/PC1_SEQ.VDD;1"
        || row.sequence != "PCOMI1.SEQ"
        || consumer.direct_cell_store_runtime != PCOMI_CUSTOM_BYTE_DIRECT_CELL_STORE_RUNTIME
        || consumer.ui_consumer_runtime != PCOMI_CUSTOM_BYTE_UI_CONSUMER_RUNTIME
        || consumer.renderer_short_runtime != PCOMI_CUSTOM_BYTE_RENDERER_SHORT_RUNTIME
        || consumer.renderer_extended_runtime != PCOMI_CUSTOM_BYTE_RENDERER_EXTENDED_RUNTIME
        || consumer.slot_count != row.slot_count
        || consumer.slot_payload_bytes != row.slot_payload_bytes
    {
        return Err(error(
            "PCOMI custom byte storage/consumer/atlas 경계 불일치",
        ));
    }
    Ok(row)
}

fn verify_object_and_root(instructions: &[Instruction]) -> Result<(), PcomiCustomByteAtlasError> {
    let checks = [
        (
            0x8001_801C,
            K::Lui {
                rt: Reg::V1,
                imm: 0x8007,
            },
        ),
        (
            0x8001_8024,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8003,
            },
        ),
        (
            0x8001_8028,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: PCOMI_DESCRIPTOR_ROOT_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8001_8034,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: PCOMI_DESCRIPTOR_ROOT_FIELD_RUNTIME as u16 as i16,
            },
        ),
        (
            PCOMI_CUSTOM_BYTE_OBJECT_CREATE_RUNTIME,
            K::Jal {
                target: 0x8001_1CA4,
            },
        ),
        (
            0x8002_2190,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: PCOMI_CUSTOM_BYTE_ATLAS_RESOURCE as i16,
            },
        ),
        (
            0x8002_2194,
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: PCOMI_CUSTOM_BYTE_OBJECT_GP_OFFSET,
            },
        ),
        (
            0x8002_21A0,
            K::Sh {
                rt: Reg::A2,
                base: Reg::V0,
                offset: 0x006C,
            },
        ),
        (
            0x8002_21A4,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: 2,
            },
        ),
        (
            0x8002_21B0,
            K::Sh {
                rt: Reg::A2,
                base: Reg::V0,
                offset: 0x006A,
            },
        ),
        (
            PCOMI_CUSTOM_BYTE_OBJECT_HANDOFF_RUNTIME,
            K::Lw {
                rt: Reg::A0,
                base: Reg::GP,
                offset: PCOMI_CUSTOM_BYTE_OBJECT_GP_OFFSET,
            },
        ),
        (
            0x8002_34C4,
            K::Lw {
                rt: Reg::V1,
                base: Reg::GP,
                offset: PCOMI_CUSTOM_BYTE_OBJECT_GP_OFFSET,
            },
        ),
        (
            0x8002_34D0,
            K::Lw {
                rt: Reg::A0,
                base: Reg::GP,
                offset: PCOMI_CUSTOM_BYTE_OBJECT_GP_OFFSET,
            },
        ),
        (
            0x8002_34D4,
            K::Jal {
                target: PCOMI_CUSTOM_BYTE_UI_CONSUMER_RUNTIME,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, address, expected)?;
    }
    Ok(())
}

fn verify_renderer_boundary(cells: &BtxCellAnalysis) -> Result<(), PcomiCustomByteAtlasError> {
    let expected = [
        (
            BtxRendererFamily::ShortRenderer,
            PCOMI_CUSTOM_BYTE_RENDERER_SHORT_RUNTIME,
            0x8001_9010,
            0x8001_9014,
            0x8001_902C,
            0x8001_9038,
            0x8001_903C,
            0x8001_9044,
        ),
        (
            BtxRendererFamily::ExtendedRenderer,
            PCOMI_CUSTOM_BYTE_RENDERER_EXTENDED_RUNTIME,
            0x8001_9328,
            0x8001_932C,
            0x8001_9344,
            0x8001_9350,
            0x8001_9354,
            0x8001_935C,
        ),
    ];
    for (
        family,
        function,
        resource_load,
        cell_load,
        descriptor_table_load,
        descriptor_entry,
        cell_table_load,
        cell_entry,
    ) in expected
    {
        let matches = cells
            .consumers
            .iter()
            .filter(|cell| {
                cell.exe == "PCOMI.EXE"
                    && cell.renderer_family == Some(family)
                    && cell.function_candidate == Some(function)
                    && cell.resource_field_offset == 0x006C
                    && cell.cell_field_offset == 0x006A
                    && cell.resource_load == resource_load
                    && cell.cell_load == cell_load
                    && cell.descriptor_table_load == descriptor_table_load
                    && cell.descriptor_entry == descriptor_entry
                    && cell.cell_table_load == cell_table_load
                    && cell.cell_entry == cell_entry
            })
            .count();
        if matches != 1 {
            return Err(error(format!(
                "PCOMI custom byte renderer 경계 불일치: family={}, matches={matches}",
                family.name()
            )));
        }
    }
    Ok(())
}

fn verify_descriptor_and_name(
    map: &ExeMap,
    text: &[u8],
    names: &BtxNameAnalysis,
) -> Result<(), PcomiCustomByteAtlasError> {
    let expected = [
        0x0001_0000,
        PCOMI03_NAME_RUNTIME,
        PCOMI03_STORED_BYTES as u32,
        PCOMI03_PLAIN_BYTES as u32,
        0,
        0,
        0,
        0,
        0,
    ];
    for (index, value) in expected.into_iter().enumerate() {
        let runtime = PCOMI_CUSTOM_BYTE_DESCRIPTOR_RUNTIME
            + u32::try_from(index * 4).expect("9-word descriptor");
        if read_text_word(map, text, runtime)? != value {
            return Err(error(format!(
                "PCOMI03 source descriptor 불일치: word={index}"
            )));
        }
    }
    let links = names
        .links
        .iter()
        .filter(|link| {
            link.exe == "PCOMI.EXE"
                && link.archive == PCOMI03_ARCHIVE
                && link.asset_name == PCOMI03_ASSET
                && link.runtime_addr == PCOMI03_NAME_RUNTIME
                && link.descriptor_runtime_start == PCOMI_DESCRIPTOR_ROOT_RUNTIME
                && link.descriptor_pointer_runtime == PCOMI_CUSTOM_BYTE_DESCRIPTOR_RUNTIME + 4
        })
        .count();
    if links != 1 {
        return Err(error(format!(
            "PCOMI resource 4와 PCOMI03 이름표 연결 불일치: matches={links}"
        )));
    }
    Ok(())
}

fn verify_setup_and_handler(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    sequence: &[u8],
) -> Result<(usize, usize), PcomiCustomByteAtlasError> {
    let sequence_len = u32::try_from(sequence.len()).map_err(|_| error("PCOMI1 크기 overflow"))?;
    let sequence_map = RuntimeAssetMap::new(PCOMI1_LOAD_BASE, sequence_len)
        .ok_or_else(|| error("PCOMI1 runtime 사상 overflow"))?;
    for (runtime, expected, label) in [
        (
            PCOMI_CUSTOM_BYTE_SETUP_RUNTIME,
            PCOMI_CUSTOM_BYTE_SETUP.as_slice(),
            "resource setup",
        ),
        (
            PCOMI_CUSTOM_BYTE_SETUP_METADATA_RUNTIME,
            PCOMI_CUSTOM_BYTE_SETUP_METADATA.as_slice(),
            "resource metadata",
        ),
    ] {
        let offset = sequence_map
            .file_offset(runtime)
            .ok_or_else(|| error(format!("PCOMI1 {label} runtime 사상 이탈")))?;
        if sequence.get(offset..offset + expected.len()) != Some(expected) {
            return Err(error(format!("PCOMI1 {label} 원본 불일치")));
        }
    }
    let table_offset = sequence_map
        .file_offset(PCOMI_CUSTOM_BYTE_CELL_TABLE_RUNTIME)
        .ok_or_else(|| error("PCOMI03 cell table runtime 사상 이탈"))?;
    let next_table_offset = sequence_map
        .file_offset(PCOMI_NEXT_CELL_TABLE_RUNTIME)
        .ok_or_else(|| error("PCOMI04 cell table runtime 사상 이탈"))?;

    let nested_slot = PCOMI_NESTED_DISPATCH_TABLE_RUNTIME + 0x12 * 4;
    if read_text_word(map, text, nested_slot)? != PCOMI_CELL_TABLE_HANDLER_RUNTIME {
        return Err(error("PCOMI OP28/sub12 dispatcher slot 불일치"));
    }
    let checks = [
        (
            0x8001_2CE0,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::S2,
                imm: 4,
            },
        ),
        (
            0x8001_2CEC,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S4,
                offset: 0x001C,
            },
        ),
        (
            0x8001_2CF4,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x000C,
            },
        ),
        (
            0x8001_2CFC,
            K::Sw {
                rt: Reg::V1,
                base: Reg::S1,
                offset: 0x001C,
            },
        ),
        (
            0x8001_2D08,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x0006,
            },
        ),
        (
            0x8001_2D14,
            K::Sh {
                rt: Reg::V1,
                base: Reg::S1,
                offset: 0x0020,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, address, expected)?;
    }
    Ok((table_offset, next_table_offset))
}

fn parse_cells(
    sequence: &[u8],
    table_offset: usize,
    next_table_offset: usize,
) -> Result<Vec<PcomiCell>, PcomiCustomByteAtlasError> {
    let table = sequence
        .get(table_offset..next_table_offset)
        .ok_or_else(|| error("PCOMI03 cell table 범위 이탈"))?;
    if table.len() != 12 + PCOMI_CUSTOM_BYTE_CELL_RECORDS * 12 {
        return Err(error(format!(
            "PCOMI03 cell table 길이 불일치: {}",
            table.len()
        )));
    }
    if <Sha256 as sha2::Digest>::digest(table).as_slice() != PCOMI_CELL_TABLE_SHA256 {
        return Err(error("PCOMI03 cell table SHA-256 불일치"));
    }
    let header = (
        read_u16(table, 0)?,
        read_u16(table, 2)?,
        read_u16(table, 4)?,
        read_u16(table, 6)?,
        read_u16(table, 8)?,
        read_u16(table, 10)?,
    );
    if header != (0, 0, 768, 256, 0, 0) {
        return Err(error(format!(
            "PCOMI03 cell table header 불일치: {header:?}"
        )));
    }
    let mut cells = Vec::with_capacity(PCOMI_CUSTOM_BYTE_CELL_RECORDS);
    for index in 0..PCOMI_CUSTOM_BYTE_CELL_RECORDS {
        let offset = 12 + index * 12;
        let x = read_u16(table, offset)?;
        let y = read_u16(table, offset + 2)?;
        let width = read_u16(table, offset + 4)?;
        let height = read_u16(table, offset + 6)?;
        let empty = width == 0 || height == 0;
        let outside_atlas = !empty
            && (usize::from(x) + usize::from(width) > PCOMI_CUSTOM_BYTE_ATLAS_WIDTH
                || usize::from(y) + usize::from(height) > PCOMI_CUSTOM_BYTE_ATLAS_HEIGHT);
        cells.push(PcomiCell {
            index,
            x,
            y,
            width,
            height,
            empty,
            outside_atlas,
        });
    }
    let empty = cells
        .iter()
        .filter(|cell| cell.empty)
        .map(|cell| cell.index)
        .collect::<Vec<_>>();
    if empty != PCOMI_EMPTY_CELL_INDICES || cells.iter().any(|cell| cell.outside_atlas) {
        return Err(error(format!(
            "PCOMI03 cell 분모 불일치: empty={empty:?}, outside={}",
            cells.iter().filter(|cell| cell.outside_atlas).count()
        )));
    }
    Ok(cells)
}

fn collect_codes(
    sequence: &[u8],
    row: &crate::script::SubgameSeqCustomByteRecordRow,
) -> Result<(BTreeSet<u8>, BTreeSet<u8>), PcomiCustomByteAtlasError> {
    let repertoire = sequence
        .get(row.repertoire_offset..row.repertoire_offset + row.repertoire_bytes)
        .ok_or_else(|| error("PCOMI repertoire 범위 이탈"))?
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let slot_array = sequence
        .get(row.slot_array_start_offset..row.slot_array_end_offset)
        .ok_or_else(|| error("PCOMI custom byte slot array 범위 이탈"))?;
    let mut used = BTreeSet::new();
    for slot in slot_array.chunks_exact(row.slot_stride) {
        used.extend(slot[..row.slot_payload_bytes].iter().copied());
    }
    if repertoire.len() != row.repertoire_codes
        || used.len() != row.used_codes
        || repertoire.contains(&0)
        || used.contains(&0)
        || !used.is_subset(&repertoire)
    {
        return Err(error("PCOMI custom byte code 분모 불일치"));
    }
    Ok((repertoire, used))
}

fn cell_has_nonzero_pixel(cell: &PcomiCell, pixels: &[u8]) -> bool {
    if cell.empty || cell.outside_atlas {
        return false;
    }
    let x = usize::from(cell.x);
    let y = usize::from(cell.y);
    let width = usize::from(cell.width);
    let height = usize::from(cell.height);
    (y..y + height).any(|row| {
        let start = row * PCOMI_CUSTOM_BYTE_ATLAS_WIDTH + x;
        pixels[start..start + width].iter().any(|pixel| *pixel != 0)
    })
}

fn validate_codes(
    label: &str,
    codes: &BTreeSet<u8>,
    cells: &[PcomiCell],
    pixels: &[u8],
) -> Result<usize, PcomiCustomByteAtlasError> {
    let mut nonempty_pixels = 0usize;
    let mut empty_pixel_codes = Vec::new();
    for &code in codes {
        let cell = cells
            .get(usize::from(code))
            .ok_or_else(|| error(format!("PCOMI {label} code에 cell 없음: 0x{code:02X}")))?;
        if cell.empty || cell.outside_atlas {
            return Err(error(format!(
                "PCOMI {label} code가 유효하지 않은 cell을 참조: 0x{code:02X}"
            )));
        }
        if !cell_has_nonzero_pixel(cell, pixels) {
            empty_pixel_codes.push(code);
        } else {
            nonempty_pixels += 1;
        }
    }
    let expected_blank_pixel_codes = PCOMI_BLANK_PIXEL_CODES
        .iter()
        .copied()
        .filter(|code| codes.contains(code))
        .collect::<Vec<_>>();
    if empty_pixel_codes != expected_blank_pixel_codes {
        return Err(error(format!(
            "PCOMI {label} blank-pixel code 불일치: expected={expected_blank_pixel_codes:02X?}, \
             actual={empty_pixel_codes:02X?}"
        )));
    }
    Ok(nonempty_pixels)
}

fn verify_asset(archive: &VddArchive) -> Result<(Vec<u8>, [u8; 20]), PcomiCustomByteAtlasError> {
    let entry = archive
        .entries()
        .get(PCOMI03_ARCHIVE_INDEX)
        .ok_or_else(|| error("PC1_GRP의 PCOMI03 index가 없음"))?;
    if entry.name() != PCOMI03_ASSET || entry.size as usize != PCOMI03_STORED_BYTES {
        return Err(error(format!(
            "PC1_GRP PCOMI03 entry 불일치: name={}, size={}",
            entry.name(),
            entry.size
        )));
    }
    let stored = archive.read_file(entry);
    let header = parse_header(stored).map_err(|failure| error(failure.to_string()))?;
    if header.subtype != *b"bi_" || header.decompressed_size as usize != PCOMI03_PLAIN_BYTES {
        return Err(error(format!("PCOMI03 CNX header 불일치: {header:?}")));
    }
    let plain = decompress(stored).map_err(|failure| error(failure.to_string()))?;
    let plain_sha1: [u8; 20] = Sha1::digest(&plain).into();
    if plain.len() != PCOMI03_PLAIN_BYTES || plain_sha1 != PCOMI03_PLAIN_SHA1 {
        return Err(error("PCOMI03 해제 크기 또는 SHA-1 불일치"));
    }
    Ok((plain, plain_sha1))
}

/// 지원 원본에서 PCOMI custom byte code가 실제 PCOMI03 cell과 픽셀을 고르는지 검산한다.
#[allow(clippy::too_many_arguments)]
pub fn verify_box_pcomi_custom_byte_atlas(
    map: &ExeMap,
    text: &[u8],
    sequence: &[u8],
    archive: &VddArchive,
    names: &BtxNameAnalysis,
    cells: &BtxCellAnalysis,
    storage: &SubgameSeqCustomByteRecordAnalysis,
    consumer: &PcomiCustomByteConsumerAnalysis,
) -> Result<PcomiCustomByteAtlasAnalysis, PcomiCustomByteAtlasError> {
    if map.name != "PCOMI.EXE" || text.len() != map.text_len as usize {
        return Err(error("PCOMI custom byte atlas EXE 사상 불일치"));
    }
    let row = verify_analysis_boundaries(storage, consumer)?;
    let instructions =
        linear::sweep(map, text).ok_or_else(|| error("PCOMI.EXE text 정렬 불일치"))?;
    verify_object_and_root(&instructions)?;
    verify_renderer_boundary(cells)?;
    verify_descriptor_and_name(map, text, names)?;
    let (table_offset, next_table_offset) =
        verify_setup_and_handler(map, text, &instructions, sequence)?;
    let parsed_cells = parse_cells(sequence, table_offset, next_table_offset)?;
    let (repertoire, used) = collect_codes(sequence, row)?;
    let (plain, plain_sha1) = verify_asset(archive)?;
    let pixels = decode(
        &plain,
        Layout {
            width: PCOMI_CUSTOM_BYTE_ATLAS_WIDTH,
            height: PCOMI_CUSTOM_BYTE_ATLAS_HEIGHT,
            format: PixelFormat::Indexed4,
        },
    )
    .map_err(|failure| error(failure.to_string()))?;
    let repertoire_nonempty_pixel_cells =
        validate_codes("repertoire", &repertoire, &parsed_cells, &pixels)?;
    let used_nonempty_pixel_cells = validate_codes("used", &used, &parsed_cells, &pixels)?;

    Ok(PcomiCustomByteAtlasAnalysis {
        resource: PCOMI_CUSTOM_BYTE_ATLAS_RESOURCE,
        descriptor_runtime: PCOMI_CUSTOM_BYTE_DESCRIPTOR_RUNTIME,
        asset_name: PCOMI03_ASSET,
        archive_index: PCOMI03_ARCHIVE_INDEX,
        stored_bytes: PCOMI03_STORED_BYTES,
        plain_bytes: plain.len(),
        plain_sha1,
        atlas_width: PCOMI_CUSTOM_BYTE_ATLAS_WIDTH,
        atlas_height: PCOMI_CUSTOM_BYTE_ATLAS_HEIGHT,
        cell_table_runtime: PCOMI_CUSTOM_BYTE_CELL_TABLE_RUNTIME,
        cell_records: parsed_cells.len(),
        empty_cells: parsed_cells.iter().filter(|cell| cell.empty).count(),
        repertoire_codes: repertoire.len(),
        used_codes: used.len(),
        repertoire_nonempty_pixel_cells,
        used_nonempty_pixel_cells,
        repertoire_blank_pixel_cells: repertoire.len() - repertoire_nonempty_pixel_cells,
        used_blank_pixel_cells: used.len() - used_nonempty_pixel_cells,
        renderer_short_runtime: PCOMI_CUSTOM_BYTE_RENDERER_SHORT_RUNTIME,
        renderer_extended_runtime: PCOMI_CUSTOM_BYTE_RENDERER_EXTENDED_RUNTIME,
    })
}

#[cfg(test)]
#[path = "subgame_seq_pcomi_custom_byte_atlas_tests.rs"]
mod tests;
