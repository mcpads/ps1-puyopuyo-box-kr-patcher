//! 비-QUEST SEQ의 descriptor 선택기와 대표 source descriptor 경계.
//!
//! 세 EXE의 object handler는 현재 record의 type/resource를 읽어 type table과
//! 36바이트 stride로 descriptor를 고른다. 이 모듈은 그 generic 선택 코드와
//! `PC1/PP1/PP2_SEQ.VDD`의 대표 source descriptor를 각각 검증한다.
//! type table에 SEQ descriptor block을 대입하는 실제 dataflow와 선택된 자산의
//! runtime 적재·표시는 아직 이 경계가 주장하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::decompress;
use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;

pub const SUBGAME_SEQ_DESCRIPTOR_SCHEMA_VERSION: &str = "subgame-seq-descriptors-v1";
pub const BOX_SUBGAME_SEQ_DESCRIPTOR_ROWS: usize = 3;

/// 지원 원본에서 생성한 3행 TSV의 SHA-256.
pub const BOX_SUBGAME_SEQ_DESCRIPTOR_REPORT_SHA256: [u8; 32] = [
    0x9C, 0xDD, 0x81, 0x0A, 0x15, 0xD1, 0x75, 0xC0, 0x77, 0xF3, 0x39, 0x62, 0x54, 0xA7, 0x6F, 0x3E,
    0xAC, 0x11, 0xF6, 0xC9, 0xE9, 0x26, 0xC5, 0xAF, 0x3A, 0x27, 0x6F, 0x36, 0x66, 0xEF, 0x63, 0x8D,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectorFamily {
    Shared,
    Pcomi,
}

/// EXE별 selector와 대표 source descriptor의 exact 사양.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqDescriptorSpec {
    pub exe: &'static str,
    pub archive: &'static str,
    pub asset: &'static str,
    pub archive_entries: usize,
    pub vdd_index: usize,
    pub selector_runtime: u32,
    pub table_root_runtime: u32,
    pub descriptor_object_runtime: u32,
    pub descriptor_runtime: u32,
    pub asset_name_runtime: u32,
    pub stored_bytes: u32,
    pub decompressed_bytes: u32,
    family: SelectorFamily,
}

/// text consumer를 가진 세 EXE와 대응 SEQ archive의 대표 행.
pub const BOX_SUBGAME_SEQ_DESCRIPTOR_SPECS: [SubgameSeqDescriptorSpec; 3] = [
    SubgameSeqDescriptorSpec {
        exe: "PCOMI.EXE",
        archive: "/PCOMI/PC1_SEQ.VDD;1",
        asset: "MCRD1.SEQ",
        archive_entries: 2,
        vdd_index: 1,
        selector_runtime: 0x8001_2874,
        table_root_runtime: 0x8007_7888,
        descriptor_object_runtime: 0x8001_4ED0,
        descriptor_runtime: 0x8002_F7AC,
        asset_name_runtime: 0x8002_F80B,
        stored_bytes: 0x0A19,
        decompressed_bytes: 0x19E8,
        family: SelectorFamily::Pcomi,
    },
    SubgameSeqDescriptorSpec {
        exe: "PUYO1.EXE",
        archive: "/PUYO1/PP1_SEQ.VDD;1",
        asset: "DRAC_SEQ.SEQ",
        archive_entries: 136,
        vdd_index: 39,
        selector_runtime: 0x8001_ACF4,
        table_root_runtime: 0x800B_4330,
        descriptor_object_runtime: 0x8001_D2CC,
        descriptor_runtime: 0x8006_A564,
        asset_name_runtime: 0x8006_B987,
        stored_bytes: 0x0C50,
        decompressed_bytes: 0x1DEC,
        family: SelectorFamily::Shared,
    },
    SubgameSeqDescriptorSpec {
        exe: "PUYO2.EXE",
        archive: "/PUYO2/PP2_SEQ.VDD;1",
        asset: "DRAC_SEQ.SEQ",
        archive_entries: 201,
        vdd_index: 42,
        selector_runtime: 0x8001_9780,
        table_root_runtime: 0x800B_6710,
        descriptor_object_runtime: 0x8001_BD58,
        descriptor_runtime: 0x8006_6E9C,
        asset_name_runtime: 0x8006_8784,
        stored_bytes: 0x0C21,
        decompressed_bytes: 0x1D4C,
        family: SelectorFamily::Shared,
    },
];

/// 원본 검산을 통과한 selector와 대표 source descriptor 한 행.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDescriptorRow {
    pub exe: &'static str,
    pub archive: &'static str,
    pub asset: &'static str,
    pub vdd_index: usize,
    pub selector_runtime: u32,
    pub record_type_read_runtime: u32,
    pub record_resource_read_runtime: u32,
    pub table_root_runtime: u32,
    pub resource_x36_runtime: u32,
    pub descriptor_object_runtime: u32,
    pub descriptor_runtime: u32,
    pub asset_name_runtime: u32,
    pub stored_bytes: u32,
    pub decompressed_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDescriptorAnalysis {
    pub rows: Vec<SubgameSeqDescriptorRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDescriptorError(String);

impl fmt::Display for SubgameSeqDescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqDescriptorError {}

fn error(message: impl Into<String>) -> SubgameSeqDescriptorError {
    SubgameSeqDescriptorError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, SubgameSeqDescriptorError> {
    instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| error(format!("SEQ descriptor 명령 주소 없음: 0x{address:08X}")))
}

fn require(
    instructions: &[Instruction],
    spec: SubgameSeqDescriptorSpec,
    address: u32,
    expected: K,
) -> Result<(), SubgameSeqDescriptorError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "{} SEQ descriptor 명령 불일치: address=0x{address:08X}, \
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

fn verify_shared_selector(
    instructions: &[Instruction],
    spec: SubgameSeqDescriptorSpec,
) -> Result<(u32, u32, u32), SubgameSeqDescriptorError> {
    let start = spec.selector_runtime;
    let (table_high, table_low) = high_low(spec.table_root_runtime);
    let checks = [
        (
            start + 0x1C,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S3,
                offset: 0x1C,
            },
        ),
        (
            start + 0x24,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 4,
            },
        ),
        (
            start + 0x2C,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 5,
            },
        ),
        (
            start + 0x30,
            K::Lui {
                rt: Reg::V0,
                imm: table_high,
            },
        ),
        (
            start + 0x34,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 3,
            },
        ),
        (
            start + 0x38,
            K::Lhu {
                rt: Reg::A0,
                base: Reg::A0,
                offset: 4,
            },
        ),
        (
            start + 0x3C,
            K::J {
                target: start + 0x50,
            },
        ),
        (
            start + 0x40,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: table_low,
            },
        ),
        (
            start + 0x44,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: table_low,
            },
        ),
        (
            start + 0x48,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 3,
            },
        ),
        (
            start + 0x4C,
            K::Lhu {
                rt: Reg::A0,
                base: Reg::S3,
                offset: 0x6C,
            },
        ),
        (
            start + 0x50,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
        ),
        (
            start + 0x54,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            start + 0x58,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A0,
                sa: 3,
            },
        ),
        (
            start + 0x5C,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            start + 0x60,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            start + 0x64,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            start + 0x68,
            K::Addu {
                rd: Reg::S2,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            start + 0x6C,
            K::Jal {
                target: spec.descriptor_object_runtime,
            },
        ),
        (
            start + 0x70,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S2,
                rt: Reg::ZERO,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, spec, address, expected)?;
    }
    Ok((start + 0x34, start + 0x38, start + 0x58))
}

fn verify_pcomi_selector(
    instructions: &[Instruction],
    spec: SubgameSeqDescriptorSpec,
) -> Result<(u32, u32, u32), SubgameSeqDescriptorError> {
    let start = spec.selector_runtime;
    let (table_high, table_low) = high_low(spec.table_root_runtime);
    let checks = [
        (
            start + 0x1C,
            K::Lw {
                rt: Reg::A1,
                base: Reg::S3,
                offset: 0x1C,
            },
        ),
        (
            start + 0x24,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 4,
            },
        ),
        (
            start + 0x2C,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 10,
            },
        ),
        (
            start + 0x34,
            K::Lui {
                rt: Reg::A0,
                imm: table_high,
            },
        ),
        (
            start + 0x38,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: table_low,
            },
        ),
        (
            start + 0x3C,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A1,
                offset: 3,
            },
        ),
        (
            start + 0x40,
            K::Addu {
                rd: Reg::A1,
                rs: Reg::V0,
                rt: Reg::ZERO,
            },
        ),
        (
            start + 0x44,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A1,
                sa: 3,
            },
        ),
        (
            start + 0x48,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A1,
            },
        ),
        (
            start + 0x4C,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
        ),
        (
            start + 0x50,
            K::J {
                target: start + 0x78,
            },
        ),
        (
            start + 0x54,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::A0,
            },
        ),
        (
            start + 0x58,
            K::Lui {
                rt: Reg::A0,
                imm: table_high,
            },
        ),
        (
            start + 0x5C,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: table_low,
            },
        ),
        (
            start + 0x60,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A1,
                offset: 3,
            },
        ),
        (
            start + 0x64,
            K::Lhu {
                rt: Reg::A1,
                base: Reg::S3,
                offset: 0x6C,
            },
        ),
        (
            start + 0x68,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
        ),
        (
            start + 0x6C,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::A0,
            },
        ),
        (
            start + 0x70,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A1,
                sa: 3,
            },
        ),
        (
            start + 0x74,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A1,
            },
        ),
        (
            start + 0x78,
            K::Lw {
                rt: Reg::A0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            start + 0x7C,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            start + 0x80,
            K::Addu {
                rd: Reg::S2,
                rs: Reg::A0,
                rt: Reg::V0,
            },
        ),
        (
            start + 0x84,
            K::Jal {
                target: spec.descriptor_object_runtime,
            },
        ),
        (
            start + 0x88,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S2,
                rt: Reg::ZERO,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, spec, address, expected)?;
    }
    Ok((start + 0x3C, start + 0x24, start + 0x70))
}

fn verify_object_cache(
    instructions: &[Instruction],
    spec: SubgameSeqDescriptorSpec,
) -> Result<(), SubgameSeqDescriptorError> {
    let base = spec.descriptor_object_runtime;
    match spec.family {
        SelectorFamily::Shared => {
            require(
                instructions,
                spec,
                base,
                K::Lw {
                    rt: Reg::V0,
                    base: Reg::A0,
                    offset: 0x14,
                },
            )?;
            require(
                instructions,
                spec,
                base + 8,
                K::Beq {
                    rs: Reg::V0,
                    rt: Reg::ZERO,
                    offset: 14,
                },
            )?;
            require(
                instructions,
                spec,
                base + 0x24,
                K::Sw {
                    rt: Reg::V1,
                    base: Reg::A0,
                    offset: 0x14,
                },
            )?;
        }
        SelectorFamily::Pcomi => {
            require(
                instructions,
                spec,
                base,
                K::Addu {
                    rd: Reg::T0,
                    rs: Reg::A0,
                    rt: Reg::ZERO,
                },
            )?;
            require(
                instructions,
                spec,
                base + 4,
                K::Lw {
                    rt: Reg::V0,
                    base: Reg::T0,
                    offset: 0x14,
                },
            )?;
            require(
                instructions,
                spec,
                base + 0x0C,
                K::Beq {
                    rs: Reg::V0,
                    rt: Reg::ZERO,
                    offset: 17,
                },
            )?;
            require(
                instructions,
                spec,
                base + 0x34,
                K::Sw {
                    rt: Reg::A2,
                    base: Reg::T0,
                    offset: 0x14,
                },
            )?;
        }
    }
    Ok(())
}

fn read_word(raw: &[u8], offset: usize) -> Result<u32, SubgameSeqDescriptorError> {
    raw.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
        .ok_or_else(|| error(format!("SEQ descriptor word 범위 이탈: +0x{offset:X}")))
}

fn verify_source_descriptor(
    spec: SubgameSeqDescriptorSpec,
    map: &ExeMap,
    text: &[u8],
    archive: &VddArchive,
) -> Result<(), SubgameSeqDescriptorError> {
    if archive.entries().len() != spec.archive_entries {
        return Err(error(format!(
            "{} 엔트리 수 불일치: expected={}, actual={}",
            spec.archive,
            spec.archive_entries,
            archive.entries().len()
        )));
    }
    let entry = archive.entries().get(spec.vdd_index).ok_or_else(|| {
        error(format!(
            "{} VDD index 범위 이탈: {}",
            spec.archive, spec.vdd_index
        ))
    })?;
    if entry.name() != spec.asset || entry.size != spec.stored_bytes {
        return Err(error(format!(
            "{}:{} VDD 행 불일치: name={}, stored=0x{:X}",
            spec.archive,
            spec.vdd_index,
            entry.name(),
            entry.size
        )));
    }
    let plain = decompress(archive.read_file(entry)).map_err(|source| {
        error(format!(
            "{}:{} CNX 해제 실패: {source}",
            spec.archive, spec.asset
        ))
    })?;
    if plain.len() != spec.decompressed_bytes as usize {
        return Err(error(format!(
            "{}:{} 해제 길이 불일치: expected=0x{:X}, actual=0x{:X}",
            spec.archive,
            spec.asset,
            spec.decompressed_bytes,
            plain.len()
        )));
    }

    let descriptor_offset = map
        .text_offset(spec.descriptor_runtime)
        .ok_or_else(|| error(format!("{} descriptor가 EXE text 밖임", spec.exe)))?;
    let descriptor = text
        .get(descriptor_offset..descriptor_offset + 36)
        .ok_or_else(|| error(format!("{} descriptor 36바이트 범위 이탈", spec.exe)))?;
    let expected = [
        0x0001_0000,
        spec.asset_name_runtime,
        spec.stored_bytes,
        spec.decompressed_bytes,
        0,
        0,
        0,
        0,
        0,
    ];
    let mut actual = [0u32; 9];
    for (index, word) in actual.iter_mut().enumerate() {
        *word = read_word(descriptor, index * 4)?;
    }
    if actual != expected {
        return Err(error(format!(
            "{}:{} source descriptor 불일치: expected={expected:08X?}, actual={actual:08X?}",
            spec.exe, spec.asset
        )));
    }

    let name_offset = map
        .text_offset(spec.asset_name_runtime)
        .ok_or_else(|| error(format!("{} 이름표가 EXE text 밖임", spec.exe)))?;
    let name_end = name_offset
        .checked_add(spec.asset.len() + 1)
        .ok_or_else(|| error("SEQ descriptor 이름표 범위 overflow"))?;
    let name_literal = text
        .get(name_offset..name_end)
        .ok_or_else(|| error(format!("{} 이름표 범위 이탈", spec.exe)))?;
    if name_literal[..spec.asset.len()] != *spec.asset.as_bytes()
        || name_literal[spec.asset.len()] != 0
    {
        return Err(error(format!(
            "{} 이름표 불일치: runtime=0x{:08X}, expected={}\\0",
            spec.exe, spec.asset_name_runtime, spec.asset
        )));
    }
    Ok(())
}

fn audit_subgame_seq_descriptors<'a>(
    specs: &[SubgameSeqDescriptorSpec],
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<SubgameSeqDescriptorAnalysis, SubgameSeqDescriptorError> {
    if specs.is_empty() {
        return Err(error("SEQ descriptor spec이 없음"));
    }
    let executable_map = executables
        .into_iter()
        .map(|(map, text)| (map.name, (map, text)))
        .collect::<BTreeMap<_, _>>();
    let archive_map = archives.into_iter().collect::<BTreeMap<_, _>>();
    let mut seen_executables = BTreeSet::new();
    let mut seen_archives = BTreeSet::new();
    let mut rows = Vec::with_capacity(specs.len());

    for &spec in specs {
        if !seen_executables.insert(spec.exe) || !seen_archives.insert(spec.archive) {
            return Err(error(format!(
                "SEQ descriptor spec EXE/archive 중복: {} / {}",
                spec.exe, spec.archive
            )));
        }
        let &(map, text) = executable_map
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} 검증 입력이 없음", spec.exe)))?;
        if text.len() != map.text_len as usize {
            return Err(error(format!(
                "{} text 길이 불일치: expected={}, actual={}",
                spec.exe,
                map.text_len,
                text.len()
            )));
        }
        let archive = archive_map
            .get(spec.archive)
            .ok_or_else(|| error(format!("{} 검증 입력이 없음", spec.archive)))?;
        let instructions = linear::sweep(map, text)
            .ok_or_else(|| error(format!("{} text 길이가 4바이트 정렬이 아님", spec.exe)))?;
        let (record_type_read_runtime, record_resource_read_runtime, resource_x36_runtime) =
            match spec.family {
                SelectorFamily::Shared => verify_shared_selector(&instructions, spec)?,
                SelectorFamily::Pcomi => verify_pcomi_selector(&instructions, spec)?,
            };
        verify_object_cache(&instructions, spec)?;
        verify_source_descriptor(spec, map, text, archive)?;
        rows.push(SubgameSeqDescriptorRow {
            exe: spec.exe,
            archive: spec.archive,
            asset: spec.asset,
            vdd_index: spec.vdd_index,
            selector_runtime: spec.selector_runtime,
            record_type_read_runtime,
            record_resource_read_runtime,
            table_root_runtime: spec.table_root_runtime,
            resource_x36_runtime,
            descriptor_object_runtime: spec.descriptor_object_runtime,
            descriptor_runtime: spec.descriptor_runtime,
            asset_name_runtime: spec.asset_name_runtime,
            stored_bytes: spec.stored_bytes,
            decompressed_bytes: spec.decompressed_bytes,
        });
    }

    if executable_map.len() != specs.len() || archive_map.len() != specs.len() {
        return Err(error(format!(
            "SEQ descriptor 입력 집합 불일치: specs={}, EXE={}, archives={}",
            specs.len(),
            executable_map.len(),
            archive_map.len()
        )));
    }
    Ok(SubgameSeqDescriptorAnalysis { rows })
}

/// 지원 원본의 세 EXE selector와 대표 source descriptor를 검증한다.
pub fn verify_box_subgame_seq_descriptors<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<SubgameSeqDescriptorAnalysis, SubgameSeqDescriptorError> {
    let analysis =
        audit_subgame_seq_descriptors(&BOX_SUBGAME_SEQ_DESCRIPTOR_SPECS, executables, archives)?;
    if analysis.rows.len() != BOX_SUBGAME_SEQ_DESCRIPTOR_ROWS {
        return Err(error(format!(
            "SEQ descriptor 행 수 불일치: expected={}, actual={}",
            BOX_SUBGAME_SEQ_DESCRIPTOR_ROWS,
            analysis.rows.len()
        )));
    }
    let report = render_subgame_seq_descriptors_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_DESCRIPTOR_REPORT_SHA256 {
        return Err(error(format!(
            "SEQ descriptor report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_DESCRIPTOR_REPORT_SHA256),
            hex(&digest)
        )));
    }
    Ok(analysis)
}

/// 검증 결과를 결정적 3행 TSV로 직렬화한다.
pub fn render_subgame_seq_descriptors_tsv(analysis: &SubgameSeqDescriptorAnalysis) -> String {
    let mut output = String::from(
        "schema_version\texe\tarchive\tasset\tvdd_index\tselector_runtime\t\
         record_type_read_runtime\trecord_resource_read_runtime\ttable_root_runtime\t\
         resource_x36_runtime\tdescriptor_object_runtime\tdescriptor_runtime\t\
         asset_name_runtime\tstored_bytes\tdecompressed_bytes\tstatic_status\n",
    );
    for row in &analysis.rows {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t\
             0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\t\
             generic-selector-and-source-descriptor-only",
            SUBGAME_SEQ_DESCRIPTOR_SCHEMA_VERSION,
            row.exe,
            row.archive,
            row.asset,
            row.vdd_index,
            row.selector_runtime,
            row.record_type_read_runtime,
            row.record_resource_read_runtime,
            row.table_root_runtime,
            row.resource_x36_runtime,
            row.descriptor_object_runtime,
            row.descriptor_runtime,
            row.asset_name_runtime,
            row.stored_bytes,
            row.decompressed_bytes,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_descriptor_tests.rs"]
mod subgame_seq_descriptor_tests;
