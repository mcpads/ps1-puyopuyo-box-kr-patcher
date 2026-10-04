//! GOCHA1 `G_MESS.BTX`의 정적 setup·cell 선택·object 생성 경로.
//!
//! 이 모듈은 지원 원본 하나에서 `resource 0x0050` descriptor, 512×256
//! `OP28/sub12` cell table, 결과 메시지 선택표와 object writer를 함께
//! 검산한다. 공용 renderer의 실제 화면 표시는 별도 runtime 증거다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::disasm::{BtxNameAnalysis, BtxRendererFamily, BOX_BTX_RENDERER_CORES};
use crate::isa::{decode, InstructionKind as K, Reg};

const G_MESS_ARCHIVE: &str = "/GOCHA1/TUU_GRP.VDD;1";
const G_MESS_NAME: &str = "G_MESS.BTX";
const G_MESS_RESOURCE_ID: u16 = 0x0050;
const G_MESS_STORED_BYTES: u32 = 20_248;
const G_MESS_PLAIN_BYTES: u32 = 65_536;

const SETUP_RECORD_RUNTIME: u32 = 0x8007_0A98;
const CELL_TABLE_RUNTIME: u32 = 0x8006_D390;
const CELL_TABLE_BOUNDARY_RUNTIME: u32 = 0x8006_D4B4;
const SELECTION_TABLE_RUNTIME: u32 = 0x8009_34F0;
const SELECTION_ROWS: usize = 3;
const SELECTION_ROW_BYTES: usize = 120;
const RESULT_BUILDER_RUNTIME: u32 = 0x8002_0D70;

const RESOURCE_STORE_SITES: [u32; 3] = [0x8002_0ECC, 0x8002_0F48, 0x8002_0F88];
const CELL_STORE_SITES: [u32; 3] = [0x8002_0EE8, 0x8002_0F64, 0x8002_0FA0];
const RENDERER_FUNCTIONS: [u32; 2] = [0x8002_B504, 0x8002_B7EC];

type GMessSelectionGroups = Vec<Vec<u16>>;
type GMessSelectionRows = Vec<GMessSelectionGroups>;

/// G_MESS atlas의 12바이트 cell 레코드.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GMessCell {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub anchor_x: i16,
    pub anchor_y: i16,
}

const fn cell(x: u16, y: u16) -> GMessCell {
    GMessCell {
        x,
        y,
        width: 256,
        height: 16,
        anchor_x: 0,
        anchor_y: 0,
    }
}

const EXPECTED_CELLS: [GMessCell; 23] = [
    cell(0, 0),
    cell(0, 16),
    cell(0, 32),
    cell(0, 48),
    cell(0, 64),
    cell(0, 80),
    cell(0, 96),
    cell(0, 128),
    cell(0, 144),
    cell(0, 160),
    cell(0, 176),
    cell(0, 192),
    cell(0, 208),
    cell(0, 224),
    cell(0, 240),
    cell(256, 0),
    cell(256, 16),
    cell(256, 32),
    cell(256, 48),
    cell(256, 64),
    cell(256, 80),
    cell(256, 96),
    cell(256, 128),
];

/// 원본에서 함께 닫힌 G_MESS 정적 소비 계약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GMessStaticOwner {
    pub resource_id: u16,
    pub descriptor_runtime: u32,
    pub stored_bytes: u32,
    pub plain_bytes: u32,
    pub setup_record_runtime: u32,
    pub cell_table_runtime: u32,
    pub atlas_width: u16,
    pub atlas_height: u16,
    pub cells: Vec<GMessCell>,
    pub cell_table_padding_bytes: usize,
    pub selection_table_runtime: u32,
    pub selection_rows: usize,
    pub selection_groups: usize,
    pub selected_cell_indices: Vec<u16>,
    pub result_builder_runtime: u32,
    pub resource_store_sites: [u32; 3],
    pub cell_store_sites: [u32; 3],
    pub renderer_functions: [u32; 2],
}

/// G_MESS 정적 소비 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GMessStaticError(String);

impl fmt::Display for GMessStaticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for GMessStaticError {}

fn error(message: impl Into<String>) -> GMessStaticError {
    GMessStaticError(message.into())
}

fn executable_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    runtime: u32,
    len: usize,
) -> Result<&'a [u8], GMessStaticError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "GOCHA1 runtime 주소가 EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    text.get(offset..offset + len)
        .ok_or_else(|| error(format!("GOCHA1 EXE text 범위 이탈: 0x{runtime:08X}+{len}")))
}

fn read_u16(raw: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([raw[offset], raw[offset + 1]])
}

fn read_i16(raw: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([raw[offset], raw[offset + 1]])
}

fn read_u32(raw: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(raw[offset..offset + 4].try_into().expect("4바이트 word"))
}

fn read_i32(raw: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(raw[offset..offset + 4].try_into().expect("4바이트 word"))
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), GMessStaticError> {
    let raw = executable_slice(map, text, runtime, 4)?;
    let actual = decode(runtime, read_u32(raw, 0));
    if actual.kind != expected {
        return Err(error(format!(
            "G_MESS typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn parse_cell_table(
    map: &ExeMap,
    text: &[u8],
) -> Result<(u16, u16, Vec<GMessCell>, usize), GMessStaticError> {
    let table_len = usize::try_from(CELL_TABLE_BOUNDARY_RUNTIME - CELL_TABLE_RUNTIME)
        .expect("G_MESS table 길이");
    let raw = executable_slice(map, text, CELL_TABLE_RUNTIME, table_len)?;
    let header = [
        read_u16(raw, 0),
        read_u16(raw, 2),
        read_u16(raw, 4),
        read_u16(raw, 6),
        read_u16(raw, 8),
        read_u16(raw, 10),
    ];
    if header != [0, 0, 512, 256, 0, 0] {
        return Err(error(format!(
            "G_MESS cell table header 불일치: {header:?}"
        )));
    }

    let record_bytes = EXPECTED_CELLS.len() * 12;
    let cells = raw[12..12 + record_bytes]
        .chunks_exact(12)
        .map(|record| GMessCell {
            x: read_u16(record, 0),
            y: read_u16(record, 2),
            width: read_u16(record, 4),
            height: read_u16(record, 6),
            anchor_x: read_i16(record, 8),
            anchor_y: read_i16(record, 10),
        })
        .collect::<Vec<_>>();
    if cells.as_slice() != EXPECTED_CELLS {
        return Err(error(format!("G_MESS 23개 cell 계약 불일치: {cells:?}")));
    }
    let padding = &raw[12 + record_bytes..];
    if padding != [0; 4] {
        return Err(error(format!(
            "G_MESS cell table tail padding 불일치: {padding:02X?}"
        )));
    }
    Ok((header[2], header[3], cells, padding.len()))
}

fn parse_selection_row(raw: &[u8], cell_count: usize) -> Result<Vec<Vec<u16>>, GMessStaticError> {
    if raw.len() != SELECTION_ROW_BYTES {
        return Err(error(format!("G_MESS 선택 행 길이 불일치: {}", raw.len())));
    }
    let mut groups = Vec::new();
    let mut group = Vec::new();
    let mut terminated = false;
    for (word_index, word) in raw.chunks_exact(4).enumerate() {
        let value = read_i32(word, 0);
        if terminated {
            if value != 0 {
                return Err(error(format!(
                    "G_MESS 선택 행 terminator 뒤 비영 값: word={word_index}, value={value}"
                )));
            }
            continue;
        }
        match value {
            -999 => {
                if !group.is_empty() {
                    return Err(error("G_MESS 선택 행 terminator 앞 separator가 없음"));
                }
                terminated = true;
            }
            -1 => {
                if group.is_empty() {
                    return Err(error("G_MESS 선택 행에 빈 group이 있음"));
                }
                groups.push(std::mem::take(&mut group));
            }
            0.. => {
                let cell = u16::try_from(value)
                    .map_err(|_| error(format!("G_MESS 선택 cell 변환 실패: {value}")))?;
                if usize::from(cell) >= cell_count {
                    return Err(error(format!("G_MESS 선택 cell 범위 이탈: {cell}")));
                }
                group.push(cell);
            }
            _ => return Err(error(format!("G_MESS 선택 행 미지원 값: {value}"))),
        }
    }
    if !terminated {
        return Err(error("G_MESS 선택 행 terminator -999가 없음"));
    }
    Ok(groups)
}

fn parse_selection_table(
    map: &ExeMap,
    text: &[u8],
    cell_count: usize,
) -> Result<(GMessSelectionRows, Vec<u16>), GMessStaticError> {
    let raw = executable_slice(
        map,
        text,
        SELECTION_TABLE_RUNTIME,
        SELECTION_ROWS * SELECTION_ROW_BYTES,
    )?;
    let rows = raw
        .chunks_exact(SELECTION_ROW_BYTES)
        .map(|row| parse_selection_row(row, cell_count))
        .collect::<Result<Vec<_>, _>>()?;
    let expected = vec![
        vec![vec![0, 1], vec![2, 3], vec![4, 5]],
        vec![vec![6], vec![7, 8], vec![9], vec![10, 11]],
        vec![vec![13], vec![14, 15], vec![16], vec![17], vec![18, 19]],
    ];
    if rows != expected {
        return Err(error(format!("G_MESS 선택표 계약 불일치: {rows:?}")));
    }
    let selected = rows
        .iter()
        .flatten()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok((rows, selected))
}

fn verify_typed_result_builder(map: &ExeMap, text: &[u8]) -> Result<(), GMessStaticError> {
    let checks = [
        (
            0x8002_0DB0,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8009,
            },
        ),
        (
            0x8002_0DD0,
            K::Addiu {
                rt: Reg::A3,
                rs: Reg::V0,
                imm: 0x34F0,
            },
        ),
        (
            0x8002_0DD4,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800C,
            },
        ),
        (
            0x8002_0DD8,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::V0,
                imm: -0x2038,
            },
        ),
        (
            0x8002_0DE0,
            K::Lw {
                rt: Reg::A0,
                base: Reg::A1,
                offset: 0x0128,
            },
        ),
        (
            0x8002_0DE8,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A0,
                sa: 4,
            },
        ),
        (
            0x8002_0DEC,
            K::Subu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            0x8002_0DF0,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 3,
            },
        ),
        (
            0x8002_0EC8,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x0050,
            },
        ),
        (
            0x8002_0ECC,
            K::Sh {
                rt: Reg::V0,
                base: Reg::A2,
                offset: 0x006C,
            },
        ),
        (
            0x8002_0EE8,
            K::Sh {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 0x006A,
            },
        ),
        (
            0x8002_0F2C,
            K::Addiu {
                rt: Reg::S1,
                rs: Reg::ZERO,
                imm: 0x0050,
            },
        ),
        (
            0x8002_0F48,
            K::Sh {
                rt: Reg::S1,
                base: Reg::A2,
                offset: 0x006C,
            },
        ),
        (
            0x8002_0F64,
            K::Sh {
                rt: Reg::V0,
                base: Reg::A2,
                offset: 0x006A,
            },
        ),
        (
            0x8002_0F88,
            K::Sh {
                rt: Reg::S1,
                base: Reg::A2,
                offset: 0x006C,
            },
        ),
        (
            0x8002_0FA0,
            K::Sh {
                rt: Reg::V0,
                base: Reg::A2,
                offset: 0x006A,
            },
        ),
        (
            0x8002_12F8,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8002,
            },
        ),
        (
            0x8002_12FC,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x0D70,
            },
        ),
        (
            0x8002_1300,
            K::Jal {
                target: 0x8001_3404,
            },
        ),
        (
            0x8002_1304,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 6,
            },
        ),
    ];
    for (runtime, expected) in checks {
        require_instruction(map, text, runtime, expected)?;
    }
    Ok(())
}

fn verify_renderer_contract() -> Result<(), GMessStaticError> {
    let actual = BOX_BTX_RENDERER_CORES
        .iter()
        .filter(|spec| spec.exe == "GOCHA1.EXE")
        .map(|spec| (spec.family, spec.function))
        .collect::<Vec<_>>();
    let expected = vec![
        (BtxRendererFamily::ShortRenderer, RENDERER_FUNCTIONS[0]),
        (BtxRendererFamily::ExtendedRenderer, RENDERER_FUNCTIONS[1]),
    ];
    if actual != expected {
        return Err(error(format!(
            "GOCHA1 공용 renderer 계약 불일치: {actual:?}"
        )));
    }
    Ok(())
}

/// GOCHA1 G_MESS의 descriptor부터 결과 object writer까지 원본에서 검증한다.
pub fn verify_box_g_mess_static_owner<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    names: &BtxNameAnalysis,
) -> Result<GMessStaticOwner, GMessStaticError> {
    let links = names
        .links
        .iter()
        .filter(|link| link.archive == G_MESS_ARCHIVE && link.asset_name == G_MESS_NAME)
        .collect::<Vec<_>>();
    let [link] = links.as_slice() else {
        return Err(error(format!(
            "G_MESS 이름 연결 수가 1이 아님: {}",
            links.len()
        )));
    };
    if link.exe != "GOCHA1.EXE" || link.descriptor_entry_reached_ref_sites.is_empty() {
        return Err(error(format!(
            "G_MESS descriptor 정적 owner 불일치: {link:?}"
        )));
    }
    let descriptor_runtime = link
        .descriptor_pointer_runtime
        .checked_sub(4)
        .ok_or_else(|| error("G_MESS descriptor 시작 주소 underflow"))?;
    let resource_id = descriptor_runtime
        .checked_sub(link.descriptor_runtime_start)
        .filter(|delta| delta.is_multiple_of(36))
        .and_then(|delta| u16::try_from(delta / 36).ok())
        .ok_or_else(|| error("G_MESS descriptor가 36바이트 격자 밖임"))?;

    let inputs = executables
        .into_iter()
        .filter(|(map, _)| map.name == link.exe)
        .collect::<Vec<_>>();
    let [(map, text)] = inputs.as_slice() else {
        return Err(error(format!(
            "G_MESS GOCHA1 EXE 입력 수가 1이 아님: {}",
            inputs.len()
        )));
    };
    let descriptor = executable_slice(map, text, descriptor_runtime, 36)?;
    let words = (0..9)
        .map(|index| read_u32(descriptor, index * 4))
        .collect::<Vec<_>>();
    if resource_id != G_MESS_RESOURCE_ID
        || words
            != [
                0x0001_0000,
                link.runtime_addr,
                G_MESS_STORED_BYTES,
                G_MESS_PLAIN_BYTES,
                0,
                0,
                0,
                0,
                0,
            ]
    {
        return Err(error(format!(
            "G_MESS descriptor 계약 불일치: resource={resource_id}, words={words:08X?}"
        )));
    }

    let boundary_pointer = executable_slice(map, text, SETUP_RECORD_RUNTIME - 4, 4)?;
    if read_u32(boundary_pointer, 0) != CELL_TABLE_BOUNDARY_RUNTIME {
        return Err(error(format!(
            "G_MESS cell table 다음 data 경계 pointer 불일치: 0x{:08X}",
            read_u32(boundary_pointer, 0)
        )));
    }
    let setup = executable_slice(map, text, SETUP_RECORD_RUNTIME, 16)?;
    if setup[..4] != [0x28, 0x12, 0x97, 0x00]
        || read_u16(setup, 4) != G_MESS_RESOURCE_ID
        || read_u16(setup, 6) != 0
        || read_u32(setup, 8) != 0
        || read_u32(setup, 12) != CELL_TABLE_RUNTIME
    {
        return Err(error(format!(
            "G_MESS OP28/sub12 setup 불일치: {setup:02X?}"
        )));
    }

    let (atlas_width, atlas_height, cells, cell_table_padding_bytes) = parse_cell_table(map, text)?;
    let (selection_rows, selected_cell_indices) = parse_selection_table(map, text, cells.len())?;
    verify_typed_result_builder(map, text)?;
    verify_renderer_contract()?;

    Ok(GMessStaticOwner {
        resource_id,
        descriptor_runtime,
        stored_bytes: words[2],
        plain_bytes: words[3],
        setup_record_runtime: SETUP_RECORD_RUNTIME,
        cell_table_runtime: CELL_TABLE_RUNTIME,
        atlas_width,
        atlas_height,
        cells,
        cell_table_padding_bytes,
        selection_table_runtime: SELECTION_TABLE_RUNTIME,
        selection_rows: selection_rows.len(),
        selection_groups: selection_rows.iter().map(Vec::len).sum(),
        selected_cell_indices,
        result_builder_runtime: RESULT_BUILDER_RUNTIME,
        resource_store_sites: RESOURCE_STORE_SITES,
        cell_store_sites: CELL_STORE_SITES,
        renderer_functions: RENDERER_FUNCTIONS,
    })
}

#[cfg(test)]
#[path = "g_mess_static_tests.rs"]
mod tests;
