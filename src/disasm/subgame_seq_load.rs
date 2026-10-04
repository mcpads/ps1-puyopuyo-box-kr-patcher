//! 비-QUEST 장면의 DRAC descriptor에서 overlay 목적지까지의 정적 적재 경계.
//!
//! opcode `0x10` 장면 record의 16바이트 전체와 flags `0x97` 경로를 따라
//! resource object `+0x18`에 목적지 `0x801B5270`이 저장된다. completion
//! callback은 그 값을 CNX wrapper의 출력 인자로 넘기고, 완료 시 descriptor
//! `+0x10`에 같은 주소를 기록한다. 이 모듈은 그 dataflow만 검증하며 정상
//! 실행 시점이나 화면 표시를 주장하지 않는다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::{ExeMap, RuntimeAssetMap};
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::{SubgameSeqLayoutAnalysis, SubgameSeqLayoutKind};

use super::linear;
use super::subgame_seq_selection::{SubgameSeqSelectionAnalysis, BOX_SUBGAME_SEQ_SELECTION_SPECS};

pub const SUBGAME_SEQ_LOAD_SCHEMA_VERSION: &str = "subgame-seq-loads-v1";
pub const SUBGAME_SEQ_LOAD_DESTINATION: u32 = 0x801B_5270;
pub const SUBGAME_SEQ_LOAD_FLAGS: u8 = 0x97;
pub const BOX_SUBGAME_SEQ_LOAD_ROWS: usize = 3;

/// 지원 원본에서 생성한 3행 TSV의 SHA-256.
pub const BOX_SUBGAME_SEQ_LOAD_REPORT_SHA256: [u8; 32] = [
    0x7E, 0x2B, 0x64, 0xE9, 0xAE, 0xBB, 0x24, 0x7D, 0x1D, 0x1B, 0xA4, 0xAB, 0x1A, 0x3D, 0x77, 0x2E,
    0x05, 0x4C, 0x5F, 0xE7, 0x28, 0x5D, 0x42, 0x95, 0x2C, 0xAE, 0x48, 0xD3, 0xA2, 0xAB, 0x53, 0xCD,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqLoadSpec {
    pub exe: &'static str,
    pub selector_runtime: u32,
    pub completion_callback_runtime: u32,
    pub queued_callback_runtime: u32,
    pub streaming_callback_runtime: u32,
    pub streaming_init_wrapper_runtime: u32,
    pub streaming_continue_wrapper_runtime: u32,
    pub direct_wrapper_runtime: u32,
    pub cnx_output_gp_offset: i16,
    pub cnx_selector_gp_offset: i16,
}

pub const BOX_SUBGAME_SEQ_LOAD_SPECS: [SubgameSeqLoadSpec; 2] = [
    SubgameSeqLoadSpec {
        exe: "PUYO1.EXE",
        selector_runtime: 0x8001_ACF4,
        completion_callback_runtime: 0x8001_B280,
        queued_callback_runtime: 0x8001_D33C,
        streaming_callback_runtime: 0x8001_D048,
        streaming_init_wrapper_runtime: 0x8001_6ED8,
        streaming_continue_wrapper_runtime: 0x8001_6F44,
        direct_wrapper_runtime: 0x8001_6E30,
        cnx_output_gp_offset: 0x03F0,
        cnx_selector_gp_offset: 0x042C,
    },
    SubgameSeqLoadSpec {
        exe: "PUYO2.EXE",
        selector_runtime: 0x8001_9780,
        completion_callback_runtime: 0x8001_9D0C,
        queued_callback_runtime: 0x8001_BDC8,
        streaming_callback_runtime: 0x8001_BAD4,
        streaming_init_wrapper_runtime: 0x8001_5798,
        streaming_continue_wrapper_runtime: 0x8001_5804,
        direct_wrapper_runtime: 0x8001_56F0,
        cnx_output_gp_offset: 0x03FC,
        cnx_selector_gp_offset: 0x0438,
    },
];

const LOAD_RECORD: [u8; 16] = [
    0x28, 0x10, 0x97, 0x01, 0x2E, 0, 0, 0, 0x70, 0x52, 0x1B, 0x80, 0, 0, 0, 0,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqLoadRow {
    pub exe: &'static str,
    pub archive: &'static str,
    pub scene: &'static str,
    pub record_runtime: u32,
    pub destination_runtime: u32,
    pub selected_asset: &'static str,
    pub asset_descriptor_runtime: u32,
    pub completion_callback_runtime: u32,
    pub streaming_init_wrapper_runtime: u32,
    pub streaming_continue_wrapper_runtime: u32,
    pub direct_wrapper_runtime: u32,
    pub descriptor_destination_field_runtime: u32,
    pub initialized_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqLoadAnalysis {
    pub rows: Vec<SubgameSeqLoadRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqLoadError(String);

impl fmt::Display for SubgameSeqLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqLoadError {}

fn error(message: impl Into<String>) -> SubgameSeqLoadError {
    SubgameSeqLoadError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, SubgameSeqLoadError> {
    instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| error(format!("SEQ load 명령 주소 없음: 0x{address:08X}")))
}

fn require(
    instructions: &[Instruction],
    spec: SubgameSeqLoadSpec,
    address: u32,
    expected: K,
) -> Result<(), SubgameSeqLoadError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "{} SEQ load 명령 불일치: address=0x{address:08X}, \
             expected={expected:?}, actual={:?}",
            spec.exe, actual.kind
        )));
    }
    Ok(())
}

fn high_low(address: u32) -> (u16, i16) {
    (
        ((address.wrapping_add(0x8000)) >> 16) as u16,
        address as u16 as i16,
    )
}

fn verify_selector_load_path(
    instructions: &[Instruction],
    spec: SubgameSeqLoadSpec,
) -> Result<(), SubgameSeqLoadError> {
    let start = spec.selector_runtime;
    let (callback_high, callback_low) = high_low(spec.completion_callback_runtime);
    let checks = [
        (
            start + 0x108,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::S1,
                imm: 2,
            },
        ),
        (
            start + 0x110,
            K::Lui {
                rt: Reg::V0,
                imm: callback_high,
            },
        ),
        (
            start + 0x114,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: callback_low,
            },
        ),
        (
            start + 0x118,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 8,
            },
        ),
        (
            start + 0x11C,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::S1,
                imm: 0x20,
            },
        ),
        (
            start + 0x130,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S3,
                offset: 0x1C,
            },
        ),
        (
            start + 0x138,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 8,
            },
        ),
        (
            start + 0x140,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x18,
            },
        ),
        (
            start + 0x150,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::S1,
                imm: 1,
            },
        ),
        (
            start + 0x15C,
            K::Jal {
                target: spec.queued_callback_runtime,
            },
        ),
        (
            start + 0x18C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 16,
            },
        ),
        (
            start + 0x190,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S3,
                offset: 0x1C,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, spec, address, expected)?;
    }
    Ok(())
}

fn verify_completion_callback(
    instructions: &[Instruction],
    spec: SubgameSeqLoadSpec,
) -> Result<(), SubgameSeqLoadError> {
    let start = spec.completion_callback_runtime;
    let (streaming_high, streaming_low) = high_low(spec.streaming_callback_runtime);
    let checks = [
        (
            start + 0x14,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x18,
            },
        ),
        (
            start + 0x18,
            K::Lw {
                rt: Reg::S1,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            start + 0x68,
            K::Lw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: spec.cnx_selector_gp_offset,
            },
        ),
        (
            start + 0x78,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x18,
            },
        ),
        (
            start + 0x7C,
            K::Lw {
                rt: Reg::A1,
                base: Reg::S0,
                offset: 0x14,
            },
        ),
        (
            start + 0x80,
            K::Jal {
                target: spec.streaming_init_wrapper_runtime,
            },
        ),
        (
            start + 0x98,
            K::Lw {
                rt: Reg::A3,
                base: Reg::GP,
                offset: spec.cnx_selector_gp_offset,
            },
        ),
        (
            start + 0x9C,
            K::Jal {
                target: spec.streaming_continue_wrapper_runtime,
            },
        ),
        (
            start + 0xAC,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x18,
            },
        ),
        (
            start + 0xB4,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x10,
            },
        ),
        (
            start + 0xB8,
            K::Lui {
                rt: Reg::V1,
                imm: streaming_high,
            },
        ),
        (
            start + 0xCC,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: streaming_low,
            },
        ),
        (
            start + 0xD0,
            K::Sw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 8,
            },
        ),
        (
            start + 0xE0,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x18,
            },
        ),
        (
            start + 0xE4,
            K::Lw {
                rt: Reg::A1,
                base: Reg::S0,
                offset: 0x14,
            },
        ),
        (
            start + 0xE8,
            K::Jal {
                target: spec.direct_wrapper_runtime,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, spec, address, expected)?;
    }
    Ok(())
}

fn verify_cnx_output_argument(
    instructions: &[Instruction],
    spec: SubgameSeqLoadSpec,
) -> Result<(), SubgameSeqLoadError> {
    let start = spec.streaming_init_wrapper_runtime;
    let checks = [
        (
            start,
            K::Lui {
                rt: Reg::V1,
                imm: 0x0258,
            },
        ),
        (
            start + 4,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 0,
            },
        ),
        (
            start + 8,
            K::Ori {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x4E43,
            },
        ),
        (
            start + 0x20,
            K::Sw {
                rt: Reg::A0,
                base: Reg::GP,
                offset: spec.cnx_output_gp_offset,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, spec, address, expected)?;
    }
    Ok(())
}

fn audit_subgame_seq_loads<'a>(
    specs: &[SubgameSeqLoadSpec],
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    selections: &SubgameSeqSelectionAnalysis,
    layouts: &SubgameSeqLayoutAnalysis,
) -> Result<SubgameSeqLoadAnalysis, SubgameSeqLoadError> {
    let mut executable_map = BTreeMap::new();
    for (map, text) in executables {
        if executable_map.insert(map.name, (map, text)).is_some() {
            return Err(error(format!("{} EXE 입력이 중복됨", map.name)));
        }
    }
    let mut source_map = BTreeMap::new();
    for (archive, scene, raw) in sources {
        if source_map.insert((archive, scene), raw).is_some() {
            return Err(error(format!("{archive}:{scene} 입력이 중복됨")));
        }
    }
    let mut spec_map = BTreeMap::new();
    for &spec in specs {
        if spec_map.insert(spec.exe, spec).is_some() {
            return Err(error(format!("{} load spec이 중복됨", spec.exe)));
        }
    }
    if specs.len() != 2 || executable_map.len() != 2 || spec_map.len() != 2 {
        return Err(error("SEQ load EXE/spec 집합 불일치"));
    }
    for &spec in specs {
        let &(map, text) = executable_map
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} EXE 입력이 없음", spec.exe)))?;
        if text.len() != map.text_len as usize {
            return Err(error(format!("{} text 길이 불일치", spec.exe)));
        }
        let instructions = linear::sweep(map, text)
            .ok_or_else(|| error(format!("{} text 정렬 불일치", spec.exe)))?;
        verify_selector_load_path(&instructions, spec)?;
        verify_completion_callback(&instructions, spec)?;
        verify_cnx_output_argument(&instructions, spec)?;
    }

    if selections.rows.len() != BOX_SUBGAME_SEQ_SELECTION_SPECS.len() {
        return Err(error("SEQ selection 행 수 불일치"));
    }
    let mut rows = Vec::with_capacity(selections.rows.len());
    for (selection, selection_spec) in selections.rows.iter().zip(BOX_SUBGAME_SEQ_SELECTION_SPECS) {
        let raw = source_map
            .get(&(selection.archive, selection.scene))
            .ok_or_else(|| {
                error(format!(
                    "{}:{} 입력이 없음",
                    selection.archive, selection.scene
                ))
            })?;
        let source_len = u32::try_from(raw.len()).map_err(|_| error("SEQ source 길이 초과"))?;
        let source_map = RuntimeAssetMap::new(selection_spec.load_base, source_len)
            .ok_or_else(|| error("SEQ source runtime 사상 범위 초과"))?;
        let expected_record_runtime = source_map
            .runtime_addr(selection_spec.record_offset)
            .ok_or_else(|| error("SEQ selection record runtime 사상 범위 이탈"))?;
        if selection.exe != selection_spec.exe
            || selection.archive != selection_spec.archive
            || selection.scene != selection_spec.scene
            || selection.record_runtime != expected_record_runtime
        {
            return Err(error("SEQ selection 행 순서·spec 불일치"));
        }
        let record = raw
            .get(selection_spec.record_offset..selection_spec.record_offset + LOAD_RECORD.len())
            .ok_or_else(|| error("SEQ load record 범위 이탈"))?;
        if record != LOAD_RECORD {
            return Err(error(format!(
                "{}:{} 16바이트 load record 불일치",
                selection.archive, selection.scene
            )));
        }
        let layout = layouts
            .rows
            .iter()
            .find(|row| {
                row.archive == selection.archive
                    && row.sequence == selection.selected_asset
                    && row.kind == SubgameSeqLayoutKind::Overlay
            })
            .ok_or_else(|| error(format!("{} DRAC overlay layout이 없음", selection.exe)))?;
        let footer = layout
            .footer
            .ok_or_else(|| error("DRAC overlay footer가 없음"))?;
        if footer.load_base != SUBGAME_SEQ_LOAD_DESTINATION || layout.root.is_none() {
            return Err(error(format!(
                "{} DRAC overlay 목적지/root 불일치",
                selection.exe
            )));
        }
        let spec = *spec_map
            .get(selection.exe)
            .ok_or_else(|| error(format!("{} load spec이 없음", selection.exe)))?;
        rows.push(SubgameSeqLoadRow {
            exe: selection.exe,
            archive: selection.archive,
            scene: selection.scene,
            record_runtime: selection.record_runtime,
            destination_runtime: footer.load_base,
            selected_asset: selection.selected_asset,
            asset_descriptor_runtime: selection.asset_descriptor_runtime,
            completion_callback_runtime: spec.completion_callback_runtime,
            streaming_init_wrapper_runtime: spec.streaming_init_wrapper_runtime,
            streaming_continue_wrapper_runtime: spec.streaming_continue_wrapper_runtime,
            direct_wrapper_runtime: spec.direct_wrapper_runtime,
            descriptor_destination_field_runtime: selection
                .asset_descriptor_runtime
                .checked_add(0x10)
                .ok_or_else(|| error("descriptor destination field 주소 초과"))?,
            initialized_bytes: footer.initialized_len,
        });
    }
    Ok(SubgameSeqLoadAnalysis { rows })
}

/// 지원 원본의 장면 record→CNX 출력→DRAC overlay 목적지 dataflow를 검증한다.
pub fn verify_box_subgame_seq_loads<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    selections: &SubgameSeqSelectionAnalysis,
    layouts: &SubgameSeqLayoutAnalysis,
) -> Result<SubgameSeqLoadAnalysis, SubgameSeqLoadError> {
    let analysis = audit_subgame_seq_loads(
        &BOX_SUBGAME_SEQ_LOAD_SPECS,
        executables,
        sources,
        selections,
        layouts,
    )?;
    if analysis.rows.len() != BOX_SUBGAME_SEQ_LOAD_ROWS {
        return Err(error(format!(
            "SEQ load 행 수 불일치: expected={}, actual={}",
            BOX_SUBGAME_SEQ_LOAD_ROWS,
            analysis.rows.len()
        )));
    }
    let report = render_subgame_seq_loads_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_LOAD_REPORT_SHA256 {
        return Err(error(format!(
            "SEQ load report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_LOAD_REPORT_SHA256),
            hex(&digest)
        )));
    }
    Ok(analysis)
}

pub fn render_subgame_seq_loads_tsv(analysis: &SubgameSeqLoadAnalysis) -> String {
    let mut output = String::from(
        "schema_version\texe\tarchive\tscene\trecord_runtime\tflags\tdestination_runtime\t\
         selected_asset\tasset_descriptor_runtime\tcompletion_callback_runtime\t\
         streaming_init_wrapper_runtime\tstreaming_continue_wrapper_runtime\t\
         direct_wrapper_runtime\tdescriptor_destination_field_runtime\tinitialized_bytes\t\
         static_status\n",
    );
    for row in &analysis.rows {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t0x{:08X}\t0x{:02X}\t0x{:08X}\t{}\t0x{:08X}\t\
             0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t\
             scene-record-to-cnx-output-overlay-only",
            SUBGAME_SEQ_LOAD_SCHEMA_VERSION,
            row.exe,
            row.archive,
            row.scene,
            row.record_runtime,
            SUBGAME_SEQ_LOAD_FLAGS,
            row.destination_runtime,
            row.selected_asset,
            row.asset_descriptor_runtime,
            row.completion_callback_runtime,
            row.streaming_init_wrapper_runtime,
            row.streaming_continue_wrapper_runtime,
            row.direct_wrapper_runtime,
            row.descriptor_destination_field_runtime,
            row.initialized_bytes,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_load_tests.rs"]
mod subgame_seq_load_tests;
