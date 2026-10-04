//! 세 서브게임 generic text 합성 record와 간접 target slot의 raw RAM 관측.
//!
//! 지원 원본의 callback-load code window가 RAM과 일치할 때만 EXE를 식별한
//! 뒤 합성 OP21/mode2 record와 mutable pointer, initialized text 밖 target
//! slot을 읽는다. 0인 한 표본은 그 시점에 target이 관측되지 않았다는 뜻일
//! 뿐, 다른 장면의 간접 caller 부재나 constructor 실행 불가를 주장하지 않는다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::disasm::{
    SubgameSeqTextBridgeSpec, BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS,
    SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES,
};

pub const SUBGAME_SEQ_TEXT_BRIDGE_RAM_SCHEMA_VERSION: &str = "subgame-seq-text-bridge-ram-v2";
pub const PS1_MAIN_RAM_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqTextBridgeOutsideSlotObservation {
    pub runtime: u32,
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextBridgeRamObservation {
    pub exe: &'static str,
    pub ram_sha256: [u8; 32],
    pub constructor_runtime: u32,
    pub synthetic_record_runtime: u32,
    pub synthetic_pointer_runtime: u32,
    pub synthetic_pointer: u32,
    pub synthetic_record_occurrences: usize,
    pub constructor_pointer_occurrences: usize,
    pub source_code_windows_verified: usize,
    pub outside_text_slots: Vec<SubgameSeqTextBridgeOutsideSlotObservation>,
}

impl SubgameSeqTextBridgeRamObservation {
    pub const fn observation_status(&self) -> &'static str {
        if self.synthetic_pointer == 0 {
            "source-not-observed-at-synthetic-record"
        } else {
            "source-pointer-observed-needs-binding"
        }
    }

    pub fn outside_text_status(&self) -> &'static str {
        if self
            .outside_text_slots
            .iter()
            .any(|slot| slot.value == self.constructor_runtime)
        {
            "constructor-target-observed-needs-writer-binding"
        } else if self.outside_text_slots.iter().any(|slot| slot.value != 0) {
            "outside-target-observed-needs-writer-binding"
        } else {
            "outside-target-not-observed-at-this-ram"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubgameSeqTextBridgeRamError {
    WrongRamLength { expected: usize, actual: usize },
    MissingPuyo2Spec,
    MissingExecutableSource(&'static str),
    SourceTextLengthMismatch(&'static str),
    SourceWindowOutOfRange { exe: &'static str, runtime: u32 },
    NoMatchingExecutable,
    AmbiguousMatchingExecutables(Vec<&'static str>),
    CoordinateOutOfRange(u32),
    ImmutableSyntheticRecordMismatch { runtime: u32 },
}

impl fmt::Display for SubgameSeqTextBridgeRamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongRamLength { expected, actual } => write!(
                f,
                "subgame text bridge RAM 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::MissingPuyo2Spec => f.write_str("PUYO2 text bridge spec이 없음"),
            Self::MissingExecutableSource(exe) => {
                write!(f, "subgame text bridge 원본 EXE 누락: {exe}")
            }
            Self::SourceTextLengthMismatch(exe) => {
                write!(f, "subgame text bridge 원본 EXE 길이 불일치: {exe}")
            }
            Self::SourceWindowOutOfRange { exe, runtime } => write!(
                f,
                "subgame text bridge 원본 code window 범위 이탈: {exe} 0x{runtime:08X}"
            ),
            Self::NoMatchingExecutable => {
                f.write_str("RAM과 callback-load code window가 일치하는 EXE가 없음")
            }
            Self::AmbiguousMatchingExecutables(executables) => write!(
                f,
                "RAM과 callback-load code window가 여러 EXE에 일치: {executables:?}"
            ),
            Self::CoordinateOutOfRange(runtime) => write!(
                f,
                "subgame text bridge RAM 주소가 main RAM 밖임: 0x{runtime:08X}"
            ),
            Self::ImmutableSyntheticRecordMismatch { runtime } => write!(
                f,
                "PUYO2 합성 OP21/mode2 record 불변부 불일치: 0x{runtime:08X}"
            ),
        }
    }
}

impl Error for SubgameSeqTextBridgeRamError {}

fn ram_offset(runtime: u32, bytes: usize) -> Result<usize, SubgameSeqTextBridgeRamError> {
    let physical = runtime & 0x1FFF_FFFF;
    let offset = usize::try_from(physical)
        .map_err(|_| SubgameSeqTextBridgeRamError::CoordinateOutOfRange(runtime))?;
    let end = offset
        .checked_add(bytes)
        .ok_or(SubgameSeqTextBridgeRamError::CoordinateOutOfRange(runtime))?;
    if end > PS1_MAIN_RAM_BYTES {
        return Err(SubgameSeqTextBridgeRamError::CoordinateOutOfRange(runtime));
    }
    Ok(offset)
}

fn count_bytes(raw: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || needle.len() > raw.len() {
        return 0;
    }
    raw.windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

fn count_synthetic_records(raw: &[u8]) -> usize {
    raw.windows(SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES.len())
        .filter(|window| {
            window[..4] == SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES[..4]
                && window[8..] == SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES[8..]
        })
        .count()
}

fn source_windows_match(
    ram: &[u8],
    map: &ExeMap,
    text: &[u8],
    spec: SubgameSeqTextBridgeSpec,
) -> Result<bool, SubgameSeqTextBridgeRamError> {
    if text.len() != map.text_len as usize {
        return Err(SubgameSeqTextBridgeRamError::SourceTextLengthMismatch(
            spec.exe,
        ));
    }
    for source in spec.outside_text_sources {
        let end_runtime = source.call_runtime.checked_add(8).ok_or(
            SubgameSeqTextBridgeRamError::SourceWindowOutOfRange {
                exe: spec.exe,
                runtime: source.call_runtime,
            },
        )?;
        let start = map.text_offset(source.read_runtime).ok_or(
            SubgameSeqTextBridgeRamError::SourceWindowOutOfRange {
                exe: spec.exe,
                runtime: source.read_runtime,
            },
        )?;
        let end = map.text_offset(end_runtime).ok_or(
            SubgameSeqTextBridgeRamError::SourceWindowOutOfRange {
                exe: spec.exe,
                runtime: end_runtime,
            },
        )?;
        let source_window =
            text.get(start..end)
                .ok_or(SubgameSeqTextBridgeRamError::SourceWindowOutOfRange {
                    exe: spec.exe,
                    runtime: end_runtime,
                })?;
        let ram_start = ram_offset(source.read_runtime, source_window.len())?;
        if ram[ram_start..ram_start + source_window.len()] != *source_window {
            return Ok(false);
        }
    }
    Ok(true)
}

fn inspect_for_spec(
    ram: &[u8],
    spec: SubgameSeqTextBridgeSpec,
    source_code_windows_verified: usize,
) -> Result<SubgameSeqTextBridgeRamObservation, SubgameSeqTextBridgeRamError> {
    let record_offset = ram_offset(
        spec.synthetic_record_runtime,
        SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES.len(),
    )?;
    let record =
        &ram[record_offset..record_offset + SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES.len()];
    if record[..4] != SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES[..4]
        || record[8..] != SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES[8..]
    {
        return Err(
            SubgameSeqTextBridgeRamError::ImmutableSyntheticRecordMismatch {
                runtime: spec.synthetic_record_runtime,
            },
        );
    }
    let synthetic_pointer = u32::from_le_bytes(record[4..8].try_into().expect("4바이트 word"));
    let target = spec.constructor_runtime.to_le_bytes();
    let alias = (spec.constructor_runtime | 0x2000_0000).to_le_bytes();
    let outside_text_slots = spec
        .outside_text_sources
        .iter()
        .map(|source| {
            let offset = ram_offset(source.source_runtime, 4)?;
            let value = u32::from_le_bytes(
                ram[offset..offset + 4]
                    .try_into()
                    .expect("검증한 4바이트 word"),
            );
            Ok(SubgameSeqTextBridgeOutsideSlotObservation {
                runtime: source.source_runtime,
                value,
            })
        })
        .collect::<Result<Vec<_>, SubgameSeqTextBridgeRamError>>()?;

    Ok(SubgameSeqTextBridgeRamObservation {
        exe: spec.exe,
        ram_sha256: Sha256::digest(ram).into(),
        constructor_runtime: spec.constructor_runtime,
        synthetic_record_runtime: spec.synthetic_record_runtime,
        synthetic_pointer_runtime: spec.synthetic_record_runtime + 4,
        synthetic_pointer,
        synthetic_record_occurrences: count_synthetic_records(ram),
        constructor_pointer_occurrences: count_bytes(ram, &target) + count_bytes(ram, &alias),
        source_code_windows_verified,
        outside_text_slots,
    })
}

/// 지원 원본 code window로 EXE를 식별하고 generic text 관련 RAM 상태를 읽는다.
pub fn inspect_box_subgame_seq_text_bridge_ram<'a>(
    ram: &[u8],
    executables: impl IntoIterator<Item = (&'a ExeMap, &'a [u8])>,
) -> Result<SubgameSeqTextBridgeRamObservation, SubgameSeqTextBridgeRamError> {
    if ram.len() != PS1_MAIN_RAM_BYTES {
        return Err(SubgameSeqTextBridgeRamError::WrongRamLength {
            expected: PS1_MAIN_RAM_BYTES,
            actual: ram.len(),
        });
    }
    let executable_map = executables
        .into_iter()
        .map(|(map, text)| (map.name, (map, text)))
        .collect::<BTreeMap<_, _>>();
    let mut matches = Vec::new();
    for &spec in &BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS {
        let &(map, text) = executable_map.get(spec.exe).ok_or(
            SubgameSeqTextBridgeRamError::MissingExecutableSource(spec.exe),
        )?;
        if source_windows_match(ram, map, text, spec)? {
            matches.push(spec);
        }
    }
    let spec = match matches.as_slice() {
        [spec] => *spec,
        [] => return Err(SubgameSeqTextBridgeRamError::NoMatchingExecutable),
        many => {
            return Err(SubgameSeqTextBridgeRamError::AmbiguousMatchingExecutables(
                many.iter().map(|spec| spec.exe).collect(),
            ))
        }
    };
    inspect_for_spec(ram, spec, spec.outside_text_sources.len())
}

/// raw PUYO2 main RAM에서 generic text 합성 record의 관측 상태를 읽는다.
pub fn inspect_puyo2_subgame_seq_text_bridge_ram(
    ram: &[u8],
) -> Result<SubgameSeqTextBridgeRamObservation, SubgameSeqTextBridgeRamError> {
    if ram.len() != PS1_MAIN_RAM_BYTES {
        return Err(SubgameSeqTextBridgeRamError::WrongRamLength {
            expected: PS1_MAIN_RAM_BYTES,
            actual: ram.len(),
        });
    }
    let spec = BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS
        .iter()
        .find(|spec| spec.exe == "PUYO2.EXE")
        .ok_or(SubgameSeqTextBridgeRamError::MissingPuyo2Spec)?;
    inspect_for_spec(ram, *spec, 0)
}

pub fn render_subgame_seq_text_bridge_ram_tsv(
    observation: &SubgameSeqTextBridgeRamObservation,
) -> String {
    let mut output = String::from(
        "schema_version\texe\tram_sha256\tconstructor_runtime\tsynthetic_record_runtime\t\
         synthetic_pointer_runtime\tsynthetic_pointer\tsynthetic_record_occurrences\t\
         constructor_pointer_occurrences\tsource_code_windows_verified\t\
         outside_text_slot_runtime\toutside_text_slot_value\toutside_text_nonzero_slots\t\
         outside_text_constructor_targets\tobservation_status\toutside_text_status\t\
         claim_boundary\n",
    );
    let outside_text_nonzero_slots = observation
        .outside_text_slots
        .iter()
        .filter(|slot| slot.value != 0)
        .count();
    let outside_text_constructor_targets = observation
        .outside_text_slots
        .iter()
        .filter(|slot| slot.value == observation.constructor_runtime)
        .count();
    writeln!(
        output,
        "{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tobserved-ram-only-not-universal-nonexecution",
        SUBGAME_SEQ_TEXT_BRIDGE_RAM_SCHEMA_VERSION,
        observation.exe,
        hex(&observation.ram_sha256),
        observation.constructor_runtime,
        observation.synthetic_record_runtime,
        observation.synthetic_pointer_runtime,
        observation.synthetic_pointer,
        observation.synthetic_record_occurrences,
        observation.constructor_pointer_occurrences,
        observation.source_code_windows_verified,
        join_runtime(
            observation
                .outside_text_slots
                .iter()
                .map(|slot| slot.runtime)
        ),
        join_runtime(
            observation
                .outside_text_slots
                .iter()
                .map(|slot| slot.value)
        ),
        outside_text_nonzero_slots,
        outside_text_constructor_targets,
        observation.observation_status(),
        observation.outside_text_status(),
    )
    .expect("String 쓰기는 실패하지 않는다");
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn join_runtime(values: impl IntoIterator<Item = u32>) -> String {
    values
        .into_iter()
        .map(|runtime| format!("0x{runtime:08X}"))
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
#[path = "subgame_seq_text_bridge_tests.rs"]
mod tests;
