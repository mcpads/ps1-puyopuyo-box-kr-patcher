//! 비-QUEST text control의 file pointer와 생성 scratch 경계.
//!
//! `subgame_seq_control`이 고정한 `FF01/08/12/15` 전이를 실제 EXE와 324개
//! SEQ 원본에 다시 교차한다. 정렬된 control 모양은 후보일 뿐 record로
//! 승인하지 않으며, file 내부 target으로 닫히는 pointer만 따로 센다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::{parse_subgame_seq_overlay_footer, SubgameSeqLayoutKind};

use super::{linear, SubgameControlAnalysis, SubgameControlBehavior};

pub const SUBGAME_SEQ_CONTROL_TARGET_SCHEMA_VERSION: &str = "subgame-seq-control-targets-v1";
pub const BOX_SUBGAME_SEQ_CONTROL_TARGET_ROWS: usize = 12;
pub const BOX_SUBGAME_SEQ_CONTROL_TARGET_SEQUENCE_FILES: usize = 324;
pub const BOX_SUBGAME_SEQ_CONTROL_TARGET_OVERLAYS: usize = 207;
pub const BOX_SUBGAME_SEQ_CONTROL_TARGET_RAW: usize = 117;
pub const BOX_SUBGAME_SEQ_CONTROL_TARGET_SEQUENCE_CANDIDATES: usize = 26;
pub const BOX_SUBGAME_SEQ_CONTROL_TARGET_EXE_CANDIDATES: usize = 12;
pub const BOX_SUBGAME_SEQ_CONTROL_TARGET_INTERNAL_POINTERS: usize = 0;
pub const BOX_SUBGAME_SEQ_FF12_GENERATED_BYTES: usize = 22;
pub const BOX_SUBGAME_SEQ_FF15_GUARD_EXCLUSIVE_HALFWORDS: usize = 385;
pub const BOX_SUBGAME_SEQ_FF15_MAX_GENERATED_BYTES: usize = 384 * 2;

/// 지원 원본에서 생성한 handler·정렬 후보 보고서 digest.
pub const BOX_SUBGAME_SEQ_CONTROL_TARGET_REPORT_SHA256: [u8; 32] = [
    0x87, 0x0F, 0x62, 0x63, 0xA8, 0x40, 0x32, 0x95, 0x60, 0xD9, 0x27, 0x44, 0x30, 0xE1, 0x07, 0x36,
    0x09, 0x73, 0x94, 0xA8, 0xF6, 0x7D, 0xAD, 0xD6, 0xFE, 0xC5, 0xC8, 0xDB, 0x34, 0x09, 0x7E, 0x58,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubgameControlTargetKind {
    InlineAbsolutePointer,
    GeneratedScratch,
}

impl SubgameControlTargetKind {
    fn label(self) -> &'static str {
        match self {
            Self::InlineAbsolutePointer => "inline-absolute-pointer",
            Self::GeneratedScratch => "generated-exe-scratch",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DynamicKeyInstructions {
    target_stores: [(u32, K); 2],
    target_store_count: usize,
    return_store: (u32, K),
    terminator_immediate: (u32, K),
    terminator_store: (u32, K),
    guard: Option<(u32, K)>,
}

#[derive(Debug, Clone, Copy)]
pub struct SubgameControlTargetSpec {
    pub exe: &'static str,
    pub archive: &'static str,
    pub code: u8,
    pub handler_runtime: u32,
    pub handler_end_runtime: u32,
    pub header_bytes: u8,
    pub target_kind: SubgameControlTargetKind,
    pub scratch_runtime: Option<u32>,
    pub generated_bytes: Option<usize>,
    pub handler_sha256: &'static str,
    key: Option<DynamicKeyInstructions>,
}

const fn dynamic_spec(
    identity: (&'static str, &'static str, u8),
    handler: (u32, u32, u8),
    scratch: (u32, usize),
    handler_sha256: &'static str,
    key: DynamicKeyInstructions,
) -> SubgameControlTargetSpec {
    let (exe, archive, code) = identity;
    let (handler_runtime, handler_end_runtime, header_bytes) = handler;
    let (scratch_runtime, generated_bytes) = scratch;
    SubgameControlTargetSpec {
        exe,
        archive,
        code,
        handler_runtime,
        handler_end_runtime,
        header_bytes,
        target_kind: SubgameControlTargetKind::GeneratedScratch,
        scratch_runtime: Some(scratch_runtime),
        generated_bytes: Some(generated_bytes),
        handler_sha256,
        key: Some(key),
    }
}

const fn pointer_spec(
    exe: &'static str,
    archive: &'static str,
    code: u8,
    handler_runtime: u32,
) -> SubgameControlTargetSpec {
    SubgameControlTargetSpec {
        exe,
        archive,
        code,
        handler_runtime,
        handler_end_runtime: handler_runtime,
        header_bytes: 4,
        target_kind: SubgameControlTargetKind::InlineAbsolutePointer,
        scratch_runtime: None,
        generated_bytes: None,
        handler_sha256: "",
        key: None,
    }
}

pub const BOX_SUBGAME_SEQ_CONTROL_TARGET_SPECS: [SubgameControlTargetSpec; 12] = [
    pointer_spec("PCOMI.EXE", "/PCOMI/PC1_SEQ.VDD;1", 0x01, 0x8001_D5DC),
    pointer_spec("PCOMI.EXE", "/PCOMI/PC1_SEQ.VDD;1", 0x08, 0x8001_D7E8),
    dynamic_spec(
        ("PCOMI.EXE", "/PCOMI/PC1_SEQ.VDD;1", 0x12),
        (0x8001_C7A4, 0x8001_CBAC, 10),
        (0x8008_8FD0, BOX_SUBGAME_SEQ_FF12_GENERATED_BYTES),
        "e0d8cb2939f61d254c33a97381eb686c5a4285e04011d223054e271f925bfc42",
        DynamicKeyInstructions {
            target_stores: [
                (
                    0x8001_CA68,
                    K::Sw {
                        rt: Reg::A0,
                        base: Reg::GP,
                        offset: 0x0124,
                    },
                ),
                (
                    0x8001_CAB0,
                    K::Sw {
                        rt: Reg::V0,
                        base: Reg::GP,
                        offset: 0x0124,
                    },
                ),
            ],
            target_store_count: 2,
            return_store: (
                0x8001_CA54,
                K::Sw {
                    rt: Reg::A0,
                    base: Reg::V0,
                    offset: 0,
                },
            ),
            terminator_immediate: (
                0x8001_CA28,
                K::Ori {
                    rt: Reg::V0,
                    rs: Reg::ZERO,
                    imm: 0xFF09,
                },
            ),
            terminator_store: (
                0x8001_CA2C,
                K::Sh {
                    rt: Reg::V0,
                    base: Reg::V1,
                    offset: 20,
                },
            ),
            guard: None,
        },
    ),
    dynamic_spec(
        ("PCOMI.EXE", "/PCOMI/PC1_SEQ.VDD;1", 0x15),
        (0x8001_CBAC, 0x8001_D38C, 8),
        (0x8008_8FF0, BOX_SUBGAME_SEQ_FF15_MAX_GENERATED_BYTES),
        "fdbda0d5803938942462a9af76dd2125eb21a7f42dc545c4fe876ccc1a94adbe",
        DynamicKeyInstructions {
            target_stores: [
                (
                    0x8001_CE1C,
                    K::Sw {
                        rt: Reg::A0,
                        base: Reg::GP,
                        offset: 0x0124,
                    },
                ),
                (
                    0,
                    K::Sll {
                        rd: Reg::ZERO,
                        rt: Reg::ZERO,
                        sa: 0,
                    },
                ),
            ],
            target_store_count: 1,
            return_store: (
                0x8001_CE10,
                K::Sw {
                    rt: Reg::A0,
                    base: Reg::V0,
                    offset: 0x0090,
                },
            ),
            terminator_immediate: (
                0x8001_D358,
                K::Ori {
                    rt: Reg::V0,
                    rs: Reg::ZERO,
                    imm: 0xFF09,
                },
            ),
            terminator_store: (
                0x8001_D35C,
                K::Sh {
                    rt: Reg::V0,
                    base: Reg::A3,
                    offset: 2,
                },
            ),
            guard: Some((
                0x8001_CDE4,
                K::Slti {
                    rt: Reg::V0,
                    rs: Reg::V0,
                    imm: 385,
                },
            )),
        },
    ),
    pointer_spec("PUYO1.EXE", "/PUYO1/PP1_SEQ.VDD;1", 0x01, 0x8002_9BE0),
    pointer_spec("PUYO1.EXE", "/PUYO1/PP1_SEQ.VDD;1", 0x08, 0x8002_9DE4),
    dynamic_spec(
        ("PUYO1.EXE", "/PUYO1/PP1_SEQ.VDD;1", 0x12),
        (0x8002_8D60, 0x8002_91B0, 10),
        (0x800C_77A8, BOX_SUBGAME_SEQ_FF12_GENERATED_BYTES),
        "2c454f359f83d370683656d2069c834df6a326296896b4679f84a184d61125e5",
        DynamicKeyInstructions {
            target_stores: [
                (
                    0x8002_902C,
                    K::Sw {
                        rt: Reg::A0,
                        base: Reg::GP,
                        offset: 0x04B8,
                    },
                ),
                (
                    0x8002_9094,
                    K::Sw {
                        rt: Reg::V1,
                        base: Reg::GP,
                        offset: 0x04B8,
                    },
                ),
            ],
            target_store_count: 2,
            return_store: (
                0x8002_9040,
                K::Sw {
                    rt: Reg::A1,
                    base: Reg::V0,
                    offset: 0x0090,
                },
            ),
            terminator_immediate: (
                0x8002_9004,
                K::Ori {
                    rt: Reg::V0,
                    rs: Reg::ZERO,
                    imm: 0xFF09,
                },
            ),
            terminator_store: (
                0x8002_9008,
                K::Sh {
                    rt: Reg::V0,
                    base: Reg::V1,
                    offset: 0x77BC,
                },
            ),
            guard: None,
        },
    ),
    dynamic_spec(
        ("PUYO1.EXE", "/PUYO1/PP1_SEQ.VDD;1", 0x15),
        (0x8002_91B0, 0x8002_9994, 8),
        (0x800C_77C8, BOX_SUBGAME_SEQ_FF15_MAX_GENERATED_BYTES),
        "b3d67104466b2298a40d8677aeff572b897f6d07af9002d997a79f0876b325d2",
        DynamicKeyInstructions {
            target_stores: [
                (
                    0x8002_93B4,
                    K::Sw {
                        rt: Reg::V0,
                        base: Reg::GP,
                        offset: 0x04B8,
                    },
                ),
                (
                    0,
                    K::Sll {
                        rd: Reg::ZERO,
                        rt: Reg::ZERO,
                        sa: 0,
                    },
                ),
            ],
            target_store_count: 1,
            return_store: (
                0x8002_93C4,
                K::Sw {
                    rt: Reg::A0,
                    base: Reg::V0,
                    offset: 0x0090,
                },
            ),
            terminator_immediate: (
                0x8002_9950,
                K::Ori {
                    rt: Reg::V0,
                    rs: Reg::ZERO,
                    imm: 0xFF09,
                },
            ),
            terminator_store: (
                0x8002_9964,
                K::Sh {
                    rt: Reg::V0,
                    base: Reg::A3,
                    offset: 2,
                },
            ),
            guard: Some((
                0x8002_939C,
                K::Slti {
                    rt: Reg::A0,
                    rs: Reg::A0,
                    imm: 385,
                },
            )),
        },
    ),
    pointer_spec("PUYO2.EXE", "/PUYO2/PP2_SEQ.VDD;1", 0x01, 0x8002_80E8),
    pointer_spec("PUYO2.EXE", "/PUYO2/PP2_SEQ.VDD;1", 0x08, 0x8002_82EC),
    dynamic_spec(
        ("PUYO2.EXE", "/PUYO2/PP2_SEQ.VDD;1", 0x12),
        (0x8002_7268, 0x8002_76B8, 10),
        (0x800C_8198, BOX_SUBGAME_SEQ_FF12_GENERATED_BYTES),
        "fb123cd862fba2dc87e414ca4ea1b6a82bbf233aa180a4ae0666beefadadc1fb",
        DynamicKeyInstructions {
            target_stores: [
                (
                    0x8002_7534,
                    K::Sw {
                        rt: Reg::A0,
                        base: Reg::GP,
                        offset: 0x04B4,
                    },
                ),
                (
                    0x8002_759C,
                    K::Sw {
                        rt: Reg::V1,
                        base: Reg::GP,
                        offset: 0x04B4,
                    },
                ),
            ],
            target_store_count: 2,
            return_store: (
                0x8002_7548,
                K::Sw {
                    rt: Reg::A1,
                    base: Reg::V0,
                    offset: 0x0090,
                },
            ),
            terminator_immediate: (
                0x8002_750C,
                K::Ori {
                    rt: Reg::V0,
                    rs: Reg::ZERO,
                    imm: 0xFF09,
                },
            ),
            terminator_store: (
                0x8002_7510,
                K::Sh {
                    rt: Reg::V0,
                    base: Reg::V1,
                    offset: -32340,
                },
            ),
            guard: None,
        },
    ),
    dynamic_spec(
        ("PUYO2.EXE", "/PUYO2/PP2_SEQ.VDD;1", 0x15),
        (0x8002_76B8, 0x8002_7E9C, 8),
        (0x800C_81B8, BOX_SUBGAME_SEQ_FF15_MAX_GENERATED_BYTES),
        "33796dc6d9600bd4b9b5e2411f70df1fad2a8712b0fb515bac71e2b5fcf42109",
        DynamicKeyInstructions {
            target_stores: [
                (
                    0x8002_78BC,
                    K::Sw {
                        rt: Reg::V0,
                        base: Reg::GP,
                        offset: 0x04B4,
                    },
                ),
                (
                    0,
                    K::Sll {
                        rd: Reg::ZERO,
                        rt: Reg::ZERO,
                        sa: 0,
                    },
                ),
            ],
            target_store_count: 1,
            return_store: (
                0x8002_78CC,
                K::Sw {
                    rt: Reg::A0,
                    base: Reg::V0,
                    offset: 0x0090,
                },
            ),
            terminator_immediate: (
                0x8002_7E58,
                K::Ori {
                    rt: Reg::V0,
                    rs: Reg::ZERO,
                    imm: 0xFF09,
                },
            ),
            terminator_store: (
                0x8002_7E6C,
                K::Sh {
                    rt: Reg::V0,
                    base: Reg::A3,
                    offset: 2,
                },
            ),
            guard: Some((
                0x8002_78A4,
                K::Slti {
                    rt: Reg::A0,
                    rs: Reg::A0,
                    imm: 385,
                },
            )),
        },
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameControlTargetRow {
    pub exe: &'static str,
    pub archive: &'static str,
    pub code: u8,
    pub handler_runtime: u32,
    pub header_bytes: u8,
    pub target_kind: SubgameControlTargetKind,
    pub scratch_runtime: Option<u32>,
    pub generated_bytes: Option<usize>,
    pub handler_sha256: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameControlTargetOccurrence {
    pub container: &'static str,
    pub archive: String,
    pub asset: String,
    pub layout: &'static str,
    pub offset: usize,
    pub code: u8,
    pub header: Vec<u8>,
    pub direct_pointer: Option<u32>,
    pub target_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameControlTargetAnalysis {
    pub rows: Vec<SubgameControlTargetRow>,
    pub occurrences: Vec<SubgameControlTargetOccurrence>,
    pub sequence_files: usize,
    pub overlays: usize,
    pub raw_opaque: usize,
    pub sequence_candidates: usize,
    pub exe_candidates: usize,
    pub internal_pointers: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameControlTargetError(String);

impl fmt::Display for SubgameControlTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameControlTargetError {}

fn error(message: impl Into<String>) -> SubgameControlTargetError {
    SubgameControlTargetError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, SubgameControlTargetError> {
    instructions
        .binary_search_by_key(&address, |instruction| instruction.addr)
        .ok()
        .map(|index| &instructions[index])
        .ok_or_else(|| error(format!("control target 명령 주소 없음: 0x{address:08X}")))
}

fn require(
    instructions: &[Instruction],
    exe: &str,
    address: u32,
    expected: K,
) -> Result<(), SubgameControlTargetError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "{exe} control target 명령 불일치: address=0x{address:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn loaded_addresses(instructions: &[Instruction]) -> BTreeSet<u32> {
    let mut addresses = BTreeSet::new();
    for (index, instruction) in instructions.iter().enumerate() {
        let K::Lui { rt: high_reg, imm } = instruction.kind else {
            continue;
        };
        for candidate in instructions.iter().skip(index + 1).take(8) {
            if let K::Addiu {
                rt: _,
                rs,
                imm: low,
            } = candidate.kind
            {
                if rs == high_reg {
                    addresses.insert(((u32::from(imm)) << 16).wrapping_add_signed(i32::from(low)));
                }
            }
        }
    }
    addresses
}

fn code_header_bytes(code: u8) -> usize {
    match code {
        0x01 | 0x08 => 4,
        0x12 => 10,
        0x15 => 8,
        _ => unreachable!("지원 control만 scan한다"),
    }
}

fn scan_occurrences(
    container: &'static str,
    archive: &str,
    asset: &str,
    layout: &'static str,
    raw: &[u8],
    scan_end: usize,
    target_offset: impl Fn(u32) -> Option<usize>,
) -> Vec<SubgameControlTargetOccurrence> {
    let mut occurrences = Vec::new();
    for offset in (0..scan_end.saturating_sub(2)).step_by(2) {
        let token = u16::from_le_bytes([raw[offset], raw[offset + 1]]);
        let code = match token {
            0xFF01 => 0x01,
            0xFF08 => 0x08,
            0xFF12 => 0x12,
            0xFF15 => 0x15,
            _ => continue,
        };
        let header_bytes = code_header_bytes(code);
        let end = offset + 2 + header_bytes;
        if end > scan_end {
            continue;
        }
        let header = raw[offset + 2..end].to_vec();
        let direct_pointer = matches!(code, 0x01 | 0x08)
            .then(|| u32::from_le_bytes(header.as_slice().try_into().expect("4바이트 header")));
        let resolved = direct_pointer.and_then(&target_offset);
        occurrences.push(SubgameControlTargetOccurrence {
            container,
            archive: archive.to_string(),
            asset: asset.to_string(),
            layout,
            offset,
            code,
            header,
            direct_pointer,
            target_offset: resolved,
        });
    }
    occurrences
}

/// 세 EXE의 동적 target과 324개 SEQ의 정렬 후보를 검증한다.
pub fn verify_box_subgame_seq_control_targets<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    controls: &SubgameControlAnalysis,
) -> Result<SubgameControlTargetAnalysis, SubgameControlTargetError> {
    let executables = executables
        .into_iter()
        .map(|(map, text)| (map.name, (map, text)))
        .collect::<BTreeMap<_, _>>();
    if executables.len() != 3 {
        return Err(error(format!(
            "control target EXE 분모 불일치: expected=3, actual={}",
            executables.len()
        )));
    }

    let mut rows = Vec::with_capacity(BOX_SUBGAME_SEQ_CONTROL_TARGET_ROWS);
    for spec in BOX_SUBGAME_SEQ_CONTROL_TARGET_SPECS {
        let &(map, text) = executables
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} control target EXE 누락", spec.exe)))?;
        let site = controls
            .sites
            .iter()
            .find(|site| site.exe == spec.exe && site.code == spec.code)
            .ok_or_else(|| {
                error(format!(
                    "{} FF{:02X} control site 누락",
                    spec.exe, spec.code
                ))
            })?;
        if site.handler_runtime != spec.handler_runtime
            || site.behavior.argument_bytes() != spec.header_bytes
        {
            return Err(error(format!(
                "{} FF{:02X} handler/header 불일치",
                spec.exe, spec.code
            )));
        }
        match (spec.code, site.behavior) {
            (0x01, SubgameControlBehavior::BranchAbsolute { pointer_bytes: 4 })
            | (0x08, SubgameControlBehavior::CallAbsolute { pointer_bytes: 4 })
            | (0x12, SubgameControlBehavior::DynamicSelect { header_bytes: 10 })
            | (0x15, SubgameControlBehavior::DynamicSelect { header_bytes: 8 }) => {}
            _ => {
                return Err(error(format!(
                    "{} FF{:02X} target kind와 control behavior 불일치",
                    spec.exe, spec.code
                )));
            }
        }

        let handler_sha256 = if let Some(key) = spec.key {
            let start = map
                .text_offset(spec.handler_runtime)
                .ok_or_else(|| error("dynamic handler 시작이 EXE 밖임"))?;
            let end = map
                .text_offset(spec.handler_end_runtime)
                .ok_or_else(|| error("dynamic handler 끝이 EXE 밖임"))?;
            let digest: [u8; 32] = Sha256::digest(
                text.get(start..end)
                    .ok_or_else(|| error("dynamic handler byte 범위 초과"))?,
            )
            .into();
            if hex(&digest) != spec.handler_sha256 {
                return Err(error(format!(
                    "{} FF{:02X} handler SHA-256 불일치: expected={}, actual={}",
                    spec.exe,
                    spec.code,
                    spec.handler_sha256,
                    hex(&digest)
                )));
            }
            let instructions = linear::sweep(map, text)
                .ok_or_else(|| error(format!("{} EXE text 비정렬", spec.exe)))?;
            for &(address, expected) in &key.target_stores[..key.target_store_count] {
                require(&instructions, spec.exe, address, expected)?;
            }
            require(
                &instructions,
                spec.exe,
                key.return_store.0,
                key.return_store.1,
            )?;
            require(
                &instructions,
                spec.exe,
                key.terminator_immediate.0,
                key.terminator_immediate.1,
            )?;
            require(
                &instructions,
                spec.exe,
                key.terminator_store.0,
                key.terminator_store.1,
            )?;
            if let Some((address, expected)) = key.guard {
                require(&instructions, spec.exe, address, expected)?;
            }
            let handler_instructions = instructions
                .iter()
                .copied()
                .filter(|instruction| {
                    instruction.addr >= spec.handler_runtime
                        && instruction.addr < spec.handler_end_runtime
                })
                .collect::<Vec<_>>();
            let scratch = spec
                .scratch_runtime
                .ok_or_else(|| error("dynamic control scratch 누락"))?;
            if !loaded_addresses(&handler_instructions).contains(&scratch) {
                return Err(error(format!(
                    "{} FF{:02X} scratch 주소 조립을 찾지 못함: 0x{scratch:08X}",
                    spec.exe, spec.code
                )));
            }
            let initialized_end = map
                .load_addr
                .checked_add(map.text_len)
                .ok_or_else(|| error("EXE initialized 끝 overflow"))?;
            if scratch < initialized_end {
                return Err(error(format!(
                    "{} FF{:02X} scratch가 initialized EXE 안임",
                    spec.exe, spec.code
                )));
            }
            Some(digest)
        } else {
            None
        };
        rows.push(SubgameControlTargetRow {
            exe: spec.exe,
            archive: spec.archive,
            code: spec.code,
            handler_runtime: spec.handler_runtime,
            header_bytes: spec.header_bytes,
            target_kind: spec.target_kind,
            scratch_runtime: spec.scratch_runtime,
            generated_bytes: spec.generated_bytes,
            handler_sha256,
        });
    }

    let mut occurrences = Vec::new();
    let mut sequence_files = 0usize;
    let mut overlays = 0usize;
    let mut raw_opaque = 0usize;
    let sources = sources.into_iter().collect::<Vec<_>>();
    for &(archive, asset, raw) in &sources {
        sequence_files += 1;
        let footer = parse_subgame_seq_overlay_footer(raw)
            .map_err(|cause| error(format!("{archive}:{asset}: {cause}")))?;
        let (layout, scan_end) = if let Some(footer) = footer {
            overlays += 1;
            (SubgameSeqLayoutKind::Overlay, footer.footer_offset)
        } else {
            raw_opaque += 1;
            (SubgameSeqLayoutKind::RawOpaque, raw.len())
        };
        let map = footer.and_then(|footer| footer.runtime_map());
        occurrences.extend(scan_occurrences(
            "sequence",
            archive,
            asset,
            match layout {
                SubgameSeqLayoutKind::Overlay => "overlay",
                SubgameSeqLayoutKind::RawOpaque => "raw-opaque",
            },
            raw,
            scan_end,
            |pointer| map.and_then(|map| map.file_offset(pointer)),
        ));
    }
    let sequence_candidates = occurrences.len();

    for (&name, &(map, text)) in &executables {
        occurrences.extend(scan_occurrences(
            "executable",
            "",
            name,
            "exe-text",
            text,
            text.len(),
            |pointer| map.text_offset(pointer),
        ));
    }
    occurrences.sort_by(|left, right| {
        (
            left.container,
            left.archive.as_str(),
            left.asset.as_str(),
            left.offset,
            left.code,
        )
            .cmp(&(
                right.container,
                right.archive.as_str(),
                right.asset.as_str(),
                right.offset,
                right.code,
            ))
    });
    let exe_candidates = occurrences.len() - sequence_candidates;
    let internal_pointers = occurrences
        .iter()
        .filter(|occurrence| occurrence.target_offset.is_some())
        .count();

    let sequence_by_code = [0x01, 0x08, 0x12, 0x15].map(|code| {
        occurrences
            .iter()
            .filter(|occurrence| occurrence.container == "sequence" && occurrence.code == code)
            .count()
    });
    let exe_by_code = [0x01, 0x08, 0x12, 0x15].map(|code| {
        occurrences
            .iter()
            .filter(|occurrence| occurrence.container == "executable" && occurrence.code == code)
            .count()
    });
    let actual = [
        rows.len(),
        sequence_files,
        overlays,
        raw_opaque,
        sequence_candidates,
        exe_candidates,
        internal_pointers,
    ];
    let expected = [
        BOX_SUBGAME_SEQ_CONTROL_TARGET_ROWS,
        BOX_SUBGAME_SEQ_CONTROL_TARGET_SEQUENCE_FILES,
        BOX_SUBGAME_SEQ_CONTROL_TARGET_OVERLAYS,
        BOX_SUBGAME_SEQ_CONTROL_TARGET_RAW,
        BOX_SUBGAME_SEQ_CONTROL_TARGET_SEQUENCE_CANDIDATES,
        BOX_SUBGAME_SEQ_CONTROL_TARGET_EXE_CANDIDATES,
        BOX_SUBGAME_SEQ_CONTROL_TARGET_INTERNAL_POINTERS,
    ];
    if actual != expected || sequence_by_code != [0, 24, 0, 2] || exe_by_code != [1, 8, 3, 0] {
        return Err(error(format!(
            "control target 분모 불일치: totals expected={expected:?}, actual={actual:?}; sequence={sequence_by_code:?}, exe={exe_by_code:?}"
        )));
    }

    for spec in BOX_SUBGAME_SEQ_CONTROL_TARGET_SPECS
        .iter()
        .filter(|spec| spec.target_kind == SubgameControlTargetKind::GeneratedScratch)
    {
        let scratch = spec.scratch_runtime.expect("generated scratch");
        let generated = spec.generated_bytes.expect("generated bytes");
        let scratch_end = scratch
            .checked_add(u32::try_from(generated).map_err(|_| error("scratch 길이 초과"))?)
            .ok_or_else(|| error("scratch 끝 overflow"))?;
        for &(archive, asset, raw) in &sources {
            if let Some(map) = parse_subgame_seq_overlay_footer(raw)
                .map_err(|cause| error(format!("{archive}:{asset}: {cause}")))?
                .and_then(|footer| footer.runtime_map())
            {
                let map_end = map
                    .base
                    .checked_add(map.len)
                    .ok_or_else(|| error("overlay 끝 overflow"))?;
                if scratch < map_end && map.base < scratch_end {
                    return Err(error(format!(
                        "{} FF{:02X} scratch가 {archive}:{asset} overlay와 겹침",
                        spec.exe, spec.code
                    )));
                }
            }
        }
    }

    let analysis = SubgameControlTargetAnalysis {
        rows,
        occurrences,
        sequence_files,
        overlays,
        raw_opaque,
        sequence_candidates,
        exe_candidates,
        internal_pointers,
    };
    let report = render_subgame_seq_control_targets_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_CONTROL_TARGET_REPORT_SHA256 {
        return Err(error(format!(
            "control target report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_CONTROL_TARGET_REPORT_SHA256),
            hex(&digest)
        )));
    }
    Ok(analysis)
}

/// handler 경계와 정렬 후보를 같은 TSV에 기록한다.
pub fn render_subgame_seq_control_targets_tsv(analysis: &SubgameControlTargetAnalysis) -> String {
    let mut out = String::from(
        "schema_version\trow_kind\texe\tarchive\tasset\tlayout\toffset\tcode\t\
         handler_runtime\theader_bytes\ttarget_kind\tscratch_runtime\tgenerated_bytes\t\
         handler_sha256\theader_hex\tdirect_pointer\ttarget_offset\tverdict\n",
    );
    for row in &analysis.rows {
        let verdict = match (row.code, row.target_kind) {
            (0x01 | 0x08, SubgameControlTargetKind::InlineAbsolutePointer) => {
                "inline-pointer-contract-no-validated-static-record"
            }
            (0x12, SubgameControlTargetKind::GeneratedScratch) => {
                "generated-10-halfwords-plus-ff09-not-file-target"
            }
            (0x15, SubgameControlTargetKind::GeneratedScratch) => {
                "generated-stream-including-ff09-less-than-385-halfwords-not-file-target"
            }
            _ => "invalid",
        };
        writeln!(
            out,
            "{}\thandler\t{}\t{}\t\t\t\tFF{:02X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t\t\t\t{}",
            SUBGAME_SEQ_CONTROL_TARGET_SCHEMA_VERSION,
            row.exe,
            row.archive,
            row.code,
            row.handler_runtime,
            row.header_bytes,
            row.target_kind.label(),
            row.scratch_runtime
                .map(|runtime| format!("0x{runtime:08X}"))
                .unwrap_or_default(),
            row.generated_bytes
                .map(|bytes| bytes.to_string())
                .unwrap_or_default(),
            row.handler_sha256
                .as_ref()
                .map(|digest| hex(digest))
                .unwrap_or_default(),
            verdict,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    for occurrence in &analysis.occurrences {
        let header = occurrence
            .header
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        writeln!(
            out,
            "{}\tcandidate\t\t{}\t{}\t{}\t0x{:X}\tFF{:02X}\t\t{}\t\t\t\t\t{}\t{}\t{}\taligned-word-candidate-not-proven-control",
            SUBGAME_SEQ_CONTROL_TARGET_SCHEMA_VERSION,
            occurrence.archive,
            occurrence.asset,
            occurrence.layout,
            occurrence.offset,
            occurrence.code,
            code_header_bytes(occurrence.code),
            header,
            occurrence
                .direct_pointer
                .map(|pointer| format!("0x{pointer:08X}"))
                .unwrap_or_default(),
            occurrence
                .target_offset
                .map(|offset| format!("0x{offset:X}"))
                .unwrap_or_default(),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_control_target_tests.rs"]
mod subgame_seq_control_target_tests;
