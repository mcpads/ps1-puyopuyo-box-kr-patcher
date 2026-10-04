//! 다섯 서브게임 `SP_FONT.BTX`의 상단 glyph code 소비 경계.
//!
//! `SP_FONT.BTX`는 위쪽 12x12 glyph atlas와 아래쪽 UI cell을 한 자산에
//! 담는다. 이 모듈은 각 EXE의 descriptor 선택·비동기 load 예약, 전용 u16
//! code renderer, callback 등록과 GPU primitive의 UV 계산을 원본 명령에
//! 결속한다. 코드 존재와 정적 연결은 실제 실행·VRAM upload·draw hit를
//! 대신하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::disasm::BtxDescriptorSelectionAnalysis;
use crate::isa::{decode, InstructionKind as K, Reg};
use crate::text::decode_sjis;

use super::BtxFamilyStaticCellAnalysis;

const SP_FONT_FAMILY_KEY: &str = "subgame:SP_FONT:indexed4";
const SP_FONT_ASSET_NAME: &str = "SP_FONT.BTX";
const GLYPH_COLUMNS: usize = 21;
const GLYPH_ROWS: usize = 12;
const GLYPH_WIDTH: usize = 12;
const GLYPH_HEIGHT: usize = 12;
const GLYPH_MAPPINGS: usize = 251;
const GLYPH_CODE_TABLE_BYTES: usize = GLYPH_MAPPINGS * 2;
const GLYPH_REGION_HEIGHT: usize = GLYPH_ROWS * GLYPH_HEIGHT;
const ATLAS_WIDTH: usize = 256;
const ATLAS_HEIGHT: usize = 208;
const LOWER_UI_CELLS: usize = 7;
const CODE_TABLE_SHA256: &str = "54f456d49f473719fa613043dbf0cddaedd236a3c779c53715e7f4841d3597a0";

const GOCHA1_BUILDER_CALLS: [u32; 1] = [0x8003_15C0];
const GOCHA2_BUILDER_CALLS: [u32; 1] = [0x8002_D274];
const HORI2_BUILDER_CALLS: [u32; 0] = [];
const HORI4_BUILDER_CALLS: [u32; 0] = [];
const MINPUYO_BUILDER_CALLS: [u32; 1] = [0x8002_D4FC];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SpFontStaticSpec {
    exe: &'static str,
    archive: &'static str,
    resource_id: u16,
    glyph_renderer_runtime: u32,
    descriptor_root_pointer_runtime: u32,
    code_table_runtime: u32,
    text_length_runtime: u32,
    primitive_initialize_runtime: u32,
    primitive_finalize_runtime: u32,
    builder_runtime: u32,
    builder_frame_bytes: i16,
    callback_runtime: u32,
    callback_pointer_load_runtime: u32,
    callback_store_runtimes: [u32; 2],
    renderer_call_runtime: u32,
    load_schedule_runtime: u32,
    load_schedule_target: u32,
    direct_builder_call_sites: &'static [u32],
}

const SP_FONT_SPECS: [SpFontStaticSpec; 5] = [
    SpFontStaticSpec {
        exe: "GOCHA1.EXE",
        archive: "/GOCHA1/TUU_GRP.VDD;1",
        resource_id: 87,
        glyph_renderer_runtime: 0x8003_C724,
        descriptor_root_pointer_runtime: 0x800B_DE68,
        code_table_runtime: 0x8009_4E24,
        text_length_runtime: 0x8007_69A0,
        primitive_initialize_runtime: 0x8007_AFE0,
        primitive_finalize_runtime: 0x8007_B020,
        builder_runtime: 0x8003_ED9C,
        builder_frame_bytes: 0x38,
        callback_runtime: 0x8004_0400,
        callback_pointer_load_runtime: 0x8003_EEB8,
        callback_store_runtimes: [0x8003_EED0, 0x8003_EF04],
        renderer_call_runtime: 0x8004_04A8,
        load_schedule_runtime: 0x8001_6218,
        load_schedule_target: 0x8001_9848,
        direct_builder_call_sites: &GOCHA1_BUILDER_CALLS,
    },
    SpFontStaticSpec {
        exe: "GOCHA2.EXE",
        archive: "/GOCHA2/GOTY_GRP.VDD;1",
        resource_id: 6,
        glyph_renderer_runtime: 0x8003_83A4,
        descriptor_root_pointer_runtime: 0x800A_3910,
        code_table_runtime: 0x8007_B0D4,
        text_length_runtime: 0x8005_D720,
        primitive_initialize_runtime: 0x8006_1AD0,
        primitive_finalize_runtime: 0x8006_1AF0,
        builder_runtime: 0x8003_A9A0,
        builder_frame_bytes: 0x40,
        callback_runtime: 0x8003_B1EC,
        callback_pointer_load_runtime: 0x8003_AABC,
        callback_store_runtimes: [0x8003_AAD4, 0x8003_AB10],
        renderer_call_runtime: 0x8003_B294,
        load_schedule_runtime: 0x8001_E778,
        load_schedule_target: 0x8002_1D50,
        direct_builder_call_sites: &GOCHA2_BUILDER_CALLS,
    },
    SpFontStaticSpec {
        exe: "HORI2.EXE",
        archive: "/HORI2/HORI2GRP.VDD;1",
        resource_id: 6,
        glyph_renderer_runtime: 0x8003_8080,
        descriptor_root_pointer_runtime: 0x800A_3FC0,
        code_table_runtime: 0x8007_AF64,
        text_length_runtime: 0x8005_D3E8,
        primitive_initialize_runtime: 0x8006_1798,
        primitive_finalize_runtime: 0x8006_17B8,
        builder_runtime: 0x8003_A67C,
        builder_frame_bytes: 0x40,
        callback_runtime: 0x8003_AEC8,
        callback_pointer_load_runtime: 0x8003_A798,
        callback_store_runtimes: [0x8003_A7B0, 0x8003_A7EC],
        renderer_call_runtime: 0x8003_AF70,
        load_schedule_runtime: 0x8001_E7A4,
        load_schedule_target: 0x8002_1D70,
        direct_builder_call_sites: &HORI2_BUILDER_CALLS,
    },
    SpFontStaticSpec {
        exe: "HORI4.EXE",
        archive: "/HORI4/HORI4GRP.VDD;1",
        resource_id: 6,
        glyph_renderer_runtime: 0x8003_7FB8,
        descriptor_root_pointer_runtime: 0x800A_4268,
        code_table_runtime: 0x8007_B218,
        text_length_runtime: 0x8005_D69C,
        primitive_initialize_runtime: 0x8006_1A4C,
        primitive_finalize_runtime: 0x8006_1A6C,
        builder_runtime: 0x8003_A6FC,
        builder_frame_bytes: 0x40,
        callback_runtime: 0x8003_AF58,
        callback_pointer_load_runtime: 0x8003_A820,
        callback_store_runtimes: [0x8003_A838, 0x8003_A874],
        renderer_call_runtime: 0x8003_B000,
        load_schedule_runtime: 0x8001_E774,
        load_schedule_target: 0x8002_1D40,
        direct_builder_call_sites: &HORI4_BUILDER_CALLS,
    },
    SpFontStaticSpec {
        exe: "MINPUYO.EXE",
        archive: "/MINPUYO/MIN_GRP.VDD;1",
        resource_id: 6,
        glyph_renderer_runtime: 0x8003_8608,
        descriptor_root_pointer_runtime: 0x800A_3F38,
        code_table_runtime: 0x8007_B704,
        text_length_runtime: 0x8005_DD50,
        primitive_initialize_runtime: 0x8006_2100,
        primitive_finalize_runtime: 0x8006_2120,
        builder_runtime: 0x8003_AD4C,
        builder_frame_bytes: 0x40,
        callback_runtime: 0x8003_B5A8,
        callback_pointer_load_runtime: 0x8003_AE70,
        callback_store_runtimes: [0x8003_AE88, 0x8003_AEC4],
        renderer_call_runtime: 0x8003_B650,
        load_schedule_runtime: 0x8001_E758,
        load_schedule_target: 0x8002_1D34,
        direct_builder_call_sites: &MINPUYO_BUILDER_CALLS,
    },
];

/// u16 source code 하나와 상단 glyph atlas slot의 일대일 대응.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpFontGlyphMapping {
    pub index: usize,
    pub source_code: u16,
    pub source_text: char,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// 한 EXE 안에서 닫힌 `SP_FONT` 정적 load·draw 경로.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpFontStaticOwner {
    pub exe: &'static str,
    pub archive: &'static str,
    pub resource_id: u16,
    pub source_descriptor_runtime: u32,
    pub descriptor_selection_runtime: u32,
    pub load_schedule_runtime: u32,
    pub load_schedule_target: u32,
    pub glyph_renderer_runtime: u32,
    pub code_table_runtime: u32,
    pub builder_runtime: u32,
    pub callback_runtime: u32,
    pub renderer_call_runtime: u32,
    pub direct_builder_call_sites: Vec<u32>,
}

/// 다섯 물리 사본에 공통인 glyph mapping과 EXE별 정적 소유권.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpFontStaticAnalysis {
    pub owners: Vec<SpFontStaticOwner>,
    pub mappings: Vec<SpFontGlyphMapping>,
    pub code_table_sha256: String,
    pub glyph_columns: usize,
    pub glyph_rows: usize,
    pub glyph_width: usize,
    pub glyph_height: usize,
    pub mapped_slots: usize,
    pub physical_slots: usize,
    pub unused_slots: usize,
    pub glyph_region_height: usize,
    pub lower_ui_cells: usize,
    pub direct_builder_call_sites: usize,
}

/// `SP_FONT` 원본 정적 경계 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpFontStaticError(String);

impl fmt::Display for SpFontStaticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "SP_FONT 정적 소비 감사 실패: {}", self.0)
    }
}

impl Error for SpFontStaticError {}

fn error(message: impl Into<String>) -> SpFontStaticError {
    SpFontStaticError(message.into())
}

fn executable_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    runtime: u32,
    len: usize,
) -> Result<&'a [u8], SpFontStaticError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "{} 주소가 EXE text 밖임: 0x{runtime:08X}",
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

fn read_u32(raw: &[u8]) -> u32 {
    u32::from_le_bytes(raw.try_into().expect("4바이트 word"))
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), SpFontStaticError> {
    let actual = decode(runtime, read_u32(executable_slice(map, text, runtime, 4)?));
    if actual.kind != expected {
        return Err(error(format!(
            "{} typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            map.name, actual.kind
        )));
    }
    Ok(())
}

fn split_address(runtime: u32) -> (u16, i16) {
    (
        runtime.wrapping_add(0x8000).wrapping_shr(16) as u16,
        runtime as u16 as i16,
    )
}

fn glyph_rect(index: usize) -> Result<(usize, usize, usize, usize), SpFontStaticError> {
    let x = index
        .checked_rem(GLYPH_COLUMNS)
        .and_then(|column| column.checked_mul(GLYPH_WIDTH))
        .ok_or_else(|| error("SP_FONT glyph x 계산 overflow"))?;
    let y = index
        .checked_div(GLYPH_COLUMNS)
        .and_then(|row| row.checked_mul(GLYPH_HEIGHT))
        .ok_or_else(|| error("SP_FONT glyph y 계산 overflow"))?;
    if x + GLYPH_WIDTH > ATLAS_WIDTH || y + GLYPH_HEIGHT > GLYPH_REGION_HEIGHT {
        return Err(error(format!(
            "SP_FONT glyph slot이 상단 영역 밖임: index={index}, ({x},{y})"
        )));
    }
    Ok((x, y, GLYPH_WIDTH, GLYPH_HEIGHT))
}

fn decode_code_table(raw: &[u8]) -> Result<Vec<SpFontGlyphMapping>, SpFontStaticError> {
    if raw.len() != GLYPH_CODE_TABLE_BYTES {
        return Err(error(format!(
            "SP_FONT code table 길이 불일치: expected={GLYPH_CODE_TABLE_BYTES}, actual={}",
            raw.len()
        )));
    }
    let mut source_codes = BTreeSet::new();
    let mut source_texts = BTreeSet::new();
    let mut mappings = Vec::with_capacity(GLYPH_MAPPINGS);
    for (index, pair) in raw.chunks_exact(2).enumerate() {
        let source_code = u16::from_le_bytes([pair[0], pair[1]]);
        if !source_codes.insert(source_code) {
            return Err(error(format!(
                "SP_FONT code table에 중복 code가 있음: index={index}, code=0x{source_code:04X}"
            )));
        }
        let decoded = decode_sjis(pair).map_err(|decode_error| {
            error(format!(
                "SP_FONT code table Shift-JIS decode 실패: index={index}: {decode_error}"
            ))
        })?;
        let mut characters = decoded.chars();
        let source_text = characters
            .next()
            .ok_or_else(|| error(format!("SP_FONT code table 빈 문자: index={index}")))?;
        if characters.next().is_some() || !source_texts.insert(source_text) {
            return Err(error(format!(
                "SP_FONT code table이 단일 고유 문자가 아님: index={index}, text={decoded:?}"
            )));
        }
        let (x, y, width, height) = glyph_rect(index)?;
        mappings.push(SpFontGlyphMapping {
            index,
            source_code,
            source_text,
            x,
            y,
            width,
            height,
        });
    }
    Ok(mappings)
}

fn direct_jal_sites(map: &ExeMap, text: &[u8], target: u32) -> Vec<u32> {
    let jal_word = 0x0C00_0000 | ((target >> 2) & 0x03FF_FFFF);
    text.chunks_exact(4)
        .enumerate()
        .filter(|(_, word)| read_u32(word) == jal_word)
        .map(|(index, _)| map.load_addr + index as u32 * 4)
        .collect()
}

fn verify_glyph_renderer(
    spec: SpFontStaticSpec,
    map: &ExeMap,
    text: &[u8],
) -> Result<(), SpFontStaticError> {
    let renderer = spec.glyph_renderer_runtime;
    let descriptor_offset = i16::try_from(u32::from(spec.resource_id) * 36)
        .map_err(|_| error(format!("{} resource offset 변환 실패", spec.exe)))?;
    let (descriptor_root_high, descriptor_root_low) =
        split_address(spec.descriptor_root_pointer_runtime);
    let (code_table_high, code_table_low) = split_address(spec.code_table_runtime);
    let checks = [
        (
            renderer,
            K::Lui {
                rt: Reg::V0,
                imm: descriptor_root_high,
            },
        ),
        (
            renderer + 4,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: descriptor_root_low,
            },
        ),
        (
            renderer + 0x4C,
            K::Addiu {
                rt: Reg::S6,
                rs: Reg::V0,
                imm: descriptor_offset,
            },
        ),
        (
            renderer + 0x58,
            K::Jal {
                target: spec.text_length_runtime,
            },
        ),
        (
            renderer + 0x7C,
            K::Lhu {
                rt: Reg::A0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            renderer + 0xA8,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: GLYPH_MAPPINGS as i16,
            },
        ),
        (
            renderer + 0xB8,
            K::Lui {
                rt: Reg::V0,
                imm: code_table_high,
            },
        ),
        (
            renderer + 0xBC,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V0,
                imm: code_table_low,
            },
        ),
        (
            renderer + 0xC0,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            renderer + 0xE8,
            K::Lui {
                rt: Reg::V1,
                imm: 0x8618,
            },
        ),
        (
            renderer + 0xEC,
            K::Ori {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x6187,
            },
        ),
        (
            renderer + 0xF0,
            K::Multu {
                rs: Reg::A1,
                rt: Reg::V1,
            },
        ),
        (
            renderer + 0x124,
            K::Srl {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 4,
            },
        ),
        (
            renderer + 0x138,
            K::Subu {
                rd: Reg::A1,
                rs: Reg::A1,
                rt: Reg::V0,
            },
        ),
        (
            renderer + 0x148,
            K::Sll {
                rd: Reg::S2,
                rt: Reg::S2,
                sa: 2,
            },
        ),
        (
            renderer + 0x15C,
            K::Sll {
                rd: Reg::S1,
                rt: Reg::S1,
                sa: 2,
            },
        ),
        (
            renderer + 0x158,
            K::Jal {
                target: spec.primitive_initialize_runtime,
            },
        ),
        (
            renderer + 0x16C,
            K::Sh {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x10,
            },
        ),
        (
            renderer + 0x170,
            K::Sh {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x12,
            },
        ),
        (
            renderer + 0x17C,
            K::Sb {
                rt: Reg::S2,
                base: Reg::S0,
                offset: 0x0C,
            },
        ),
        (
            renderer + 0x180,
            K::Sb {
                rt: Reg::S1,
                base: Reg::S0,
                offset: 0x0D,
            },
        ),
        (
            renderer + 0x1F0,
            K::Lh {
                rt: Reg::T0,
                base: Reg::S6,
                offset: 0x20,
            },
        ),
        (
            renderer + 0x1F4,
            K::Lhu {
                rt: Reg::A3,
                base: Reg::S6,
                offset: 0x1A,
            },
        ),
        (
            renderer + 0x1F8,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S6,
                offset: 0x18,
            },
        ),
        (
            renderer + 0x24C,
            K::Jal {
                target: spec.primitive_finalize_runtime,
            },
        ),
    ];
    for (runtime, expected) in checks {
        require_instruction(map, text, runtime, expected)?;
    }
    Ok(())
}

fn verify_callback_route(
    spec: SpFontStaticSpec,
    map: &ExeMap,
    text: &[u8],
) -> Result<Vec<u32>, SpFontStaticError> {
    require_instruction(
        map,
        text,
        spec.builder_runtime,
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -spec.builder_frame_bytes,
        },
    )?;
    let (callback_high, callback_low) = split_address(spec.callback_runtime);
    require_instruction(
        map,
        text,
        spec.callback_pointer_load_runtime,
        K::Lui {
            rt: Reg::S1,
            imm: callback_high,
        },
    )?;
    require_instruction(
        map,
        text,
        spec.callback_pointer_load_runtime + 4,
        K::Addiu {
            rt: Reg::S1,
            rs: Reg::S1,
            imm: callback_low,
        },
    )?;
    for runtime in spec.callback_store_runtimes {
        require_instruction(
            map,
            text,
            runtime,
            K::Sw {
                rt: Reg::S1,
                base: Reg::A2,
                offset: 0x78,
            },
        )?;
    }
    require_instruction(
        map,
        text,
        spec.callback_runtime,
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -0x18,
        },
    )?;
    require_instruction(
        map,
        text,
        spec.renderer_call_runtime,
        K::Jal {
            target: spec.glyph_renderer_runtime,
        },
    )?;

    let direct_calls = direct_jal_sites(map, text, spec.builder_runtime);
    if direct_calls != spec.direct_builder_call_sites {
        return Err(error(format!(
            "{} glyph builder direct JAL 분모 불일치: expected={:?}, actual={direct_calls:?}",
            spec.exe, spec.direct_builder_call_sites
        )));
    }
    Ok(direct_calls)
}

fn verify_load_schedule(
    spec: SpFontStaticSpec,
    map: &ExeMap,
    text: &[u8],
) -> Result<(), SpFontStaticError> {
    require_instruction(
        map,
        text,
        spec.load_schedule_runtime,
        K::Jal {
            target: spec.load_schedule_target,
        },
    )?;
    require_instruction(
        map,
        text,
        spec.load_schedule_runtime + 4,
        K::Addu {
            rd: Reg::A0,
            rs: Reg::S1,
            rt: Reg::ZERO,
        },
    )
}

/// 지원 원본의 다섯 `SP_FONT` source→load 예약→glyph primitive 경계를 검증한다.
pub fn verify_box_sp_font_static<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    selections: &BtxDescriptorSelectionAnalysis,
    static_cells: &BtxFamilyStaticCellAnalysis,
) -> Result<SpFontStaticAnalysis, SpFontStaticError> {
    let executable_map = executables
        .into_iter()
        .map(|(map, text)| (map.name, (map, text)))
        .collect::<BTreeMap<_, _>>();
    if executable_map.len() != SP_FONT_SPECS.len() {
        return Err(error(format!(
            "SP_FONT EXE 입력 분모 불일치: expected={}, actual={}",
            SP_FONT_SPECS.len(),
            executable_map.len()
        )));
    }

    let family_rows = static_cells
        .rows
        .iter()
        .filter(|row| row.family_key == SP_FONT_FAMILY_KEY)
        .collect::<Vec<_>>();
    if family_rows.len() != SP_FONT_SPECS.len() {
        return Err(error(format!(
            "SP_FONT family 물리 행 분모 불일치: expected={}, actual={}",
            SP_FONT_SPECS.len(),
            family_rows.len()
        )));
    }

    let mut owners = Vec::with_capacity(SP_FONT_SPECS.len());
    let mut canonical_code_table = None;
    for spec in SP_FONT_SPECS {
        let (map, text) = executable_map
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} EXE 입력 누락", spec.exe)))?;
        if text.len() != map.text_len as usize {
            return Err(error(format!(
                "{} text 길이 불일치: expected={}, actual={}",
                spec.exe,
                map.text_len,
                text.len()
            )));
        }
        let matching_family_rows = family_rows
            .iter()
            .copied()
            .filter(|row| row.exe == spec.exe)
            .collect::<Vec<_>>();
        let [family_row] = matching_family_rows.as_slice() else {
            return Err(error(format!(
                "{} SP_FONT family 행이 유일하지 않음",
                spec.exe
            )));
        };
        if family_row.archive != spec.archive
            || family_row.asset_name != SP_FONT_ASSET_NAME
            || family_row.descriptor_resource_id != spec.resource_id
            || family_row.review_width != ATLAS_WIDTH
            || family_row.review_height != ATLAS_HEIGHT
            || family_row.setups.len() != 1
        {
            return Err(error(format!(
                "{} SP_FONT family source 경계 불일치: {family_row:?}",
                spec.exe
            )));
        }
        let setup = &family_row.setups[0];
        if usize::from(setup.coordinate_width) != ATLAS_WIDTH
            || usize::from(setup.coordinate_height) != ATLAS_HEIGHT
            || setup.cells.len() != LOWER_UI_CELLS
            || setup
                .cells
                .iter()
                .any(|cell| usize::from(cell.y) < GLYPH_REGION_HEIGHT)
        {
            return Err(error(format!(
                "{} SP_FONT 상단 glyph와 하단 UI cell 경계 불일치: {setup:?}",
                spec.exe
            )));
        }

        let selection = selections
            .selections
            .iter()
            .find(|selection| selection.exe == spec.exe && selection.archive == spec.archive)
            .ok_or_else(|| error(format!("{} descriptor 선택 경계 누락", spec.exe)))?;
        if selection.descriptor_select_runtime != family_row.descriptor_selection_runtime {
            return Err(error(format!(
                "{} descriptor 선택 주소 불일치: family=0x{:08X}, selection=0x{:08X}",
                spec.exe,
                family_row.descriptor_selection_runtime,
                selection.descriptor_select_runtime
            )));
        }

        verify_load_schedule(spec, map, text)?;
        verify_glyph_renderer(spec, map, text)?;
        let direct_builder_call_sites = verify_callback_route(spec, map, text)?;

        let code_table =
            executable_slice(map, text, spec.code_table_runtime, GLYPH_CODE_TABLE_BYTES)?;
        match &canonical_code_table {
            Some(canonical) if canonical != code_table => {
                return Err(error(format!(
                    "{} SP_FONT code table이 다른 EXE와 다름",
                    spec.exe
                )))
            }
            None => canonical_code_table = Some(code_table.to_vec()),
            _ => {}
        }
        owners.push(SpFontStaticOwner {
            exe: spec.exe,
            archive: spec.archive,
            resource_id: spec.resource_id,
            source_descriptor_runtime: family_row.descriptor_runtime,
            descriptor_selection_runtime: selection.descriptor_select_runtime,
            load_schedule_runtime: spec.load_schedule_runtime,
            load_schedule_target: spec.load_schedule_target,
            glyph_renderer_runtime: spec.glyph_renderer_runtime,
            code_table_runtime: spec.code_table_runtime,
            builder_runtime: spec.builder_runtime,
            callback_runtime: spec.callback_runtime,
            renderer_call_runtime: spec.renderer_call_runtime,
            direct_builder_call_sites,
        });
    }

    let code_table = canonical_code_table.ok_or_else(|| error("SP_FONT code table이 없음"))?;
    let code_table_sha256 = format!("{:x}", Sha256::digest(&code_table));
    if code_table_sha256 != CODE_TABLE_SHA256 {
        return Err(error(format!(
            "SP_FONT code table SHA-256 불일치: expected={CODE_TABLE_SHA256}, actual={code_table_sha256}"
        )));
    }
    let mappings = decode_code_table(&code_table)?;
    let physical_slots = GLYPH_COLUMNS * GLYPH_ROWS;
    if mappings.len() != GLYPH_MAPPINGS || physical_slots <= mappings.len() {
        return Err(error("SP_FONT glyph mapping/물리 slot 분모 불일치"));
    }
    let direct_builder_call_sites = owners
        .iter()
        .map(|owner| owner.direct_builder_call_sites.len())
        .sum();

    Ok(SpFontStaticAnalysis {
        owners,
        mappings,
        code_table_sha256,
        glyph_columns: GLYPH_COLUMNS,
        glyph_rows: GLYPH_ROWS,
        glyph_width: GLYPH_WIDTH,
        glyph_height: GLYPH_HEIGHT,
        mapped_slots: GLYPH_MAPPINGS,
        physical_slots,
        unused_slots: physical_slots - GLYPH_MAPPINGS,
        glyph_region_height: GLYPH_REGION_HEIGHT,
        lower_ui_cells: LOWER_UI_CELLS,
        direct_builder_call_sites,
    })
}

#[cfg(test)]
#[path = "sp_font_static_tests.rs"]
mod tests;
