//! PUYO2 `MMENU.SEQ` module root의 pre-call opcode 실행기 계약.
//!
//! overlay 안의 record와 자산 chain은 `mm_mes_route`가 소유한다. 이 모듈은
//! 그 앞 `E8`의 source table, 초기 dispatch table, 등록 함수와 dispatcher의
//! no-advance 동작만 검증한다. runtime table의 후속 교체와 실제 root 도달은
//! 증명하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::disasm::xref::{find_address_loads, DEFAULT_WINDOW};
use crate::isa::{decode, decode_slice, InstructionKind as K, Reg};

const OPCODE_TABLE_RUNTIME: u32 = 0x8008_B740;
const EXTENDED_OPCODE_SOURCE_RUNTIME: u32 = 0x8008_B360;
const EXTENDED_OPCODE_FIRST: u8 = 0xE0;
const PRE_CALL_OPCODE: u8 = 0xE8;
const PRE_CALL_DEFAULT_HANDLER_RUNTIME: u32 = 0x8001_7208;
const PRE_CALL_INITIAL_DISPATCH_HANDLER_RUNTIME: u32 = 0x8002_28C4;
const OPCODE_REGISTRATION_TARGET_RUNTIME: u32 = 0x8002_2890;
const OPCODE_DISPATCHER_RUNTIME: u32 = 0x8002_2824;
const REGISTERED_OPCODE_FIRST: u8 = 0x60;
const REGISTERED_OPCODE_LAST: u8 = 0xFF;
const OPCODES_PER_REGISTRATION_BATCH: u8 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpcodeRegistrationBatchSpec {
    call_runtime: u32,
    source_table_runtime: u32,
    source_table_load_runtime: u32,
    first_opcode: u8,
    first_opcode_load_runtime: u32,
}

const OPCODE_REGISTRATION_BATCH_SPECS: [OpcodeRegistrationBatchSpec; 5] = [
    OpcodeRegistrationBatchSpec {
        call_runtime: 0x8001_7224,
        source_table_runtime: 0x8008_B360,
        source_table_load_runtime: 0x8001_7214,
        first_opcode: 0xE0,
        first_opcode_load_runtime: 0x8001_721C,
    },
    OpcodeRegistrationBatchSpec {
        call_runtime: 0x8005_0704,
        source_table_runtime: 0x8008_D60C,
        source_table_load_runtime: 0x8005_06F8,
        first_opcode: 0x80,
        first_opcode_load_runtime: 0x8005_0700,
    },
    OpcodeRegistrationBatchSpec {
        call_runtime: 0x8005_17BC,
        source_table_runtime: 0x8008_D70C,
        source_table_load_runtime: 0x8005_17AC,
        first_opcode: 0xC0,
        first_opcode_load_runtime: 0x8005_17B4,
    },
    OpcodeRegistrationBatchSpec {
        call_runtime: 0x8005_B010,
        source_table_runtime: 0x8008_D980,
        source_table_load_runtime: 0x8005_B000,
        first_opcode: 0x60,
        first_opcode_load_runtime: 0x8005_B008,
    },
    OpcodeRegistrationBatchSpec {
        call_runtime: 0x8005_B560,
        source_table_runtime: 0x8008_DA00,
        source_table_load_runtime: 0x8005_B550,
        first_opcode: 0xA0,
        first_opcode_load_runtime: 0x8005_B558,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MmenuPreCallAnalysis {
    pub opcode: u8,
    pub source_slot_runtime: u32,
    pub dispatch_slot_runtime: u32,
    pub default_handler_runtime: u32,
    pub initial_dispatch_handler_runtime: u32,
    pub registration_batches: usize,
    pub pre_call_registration_runtime: u32,
    pub registration_target_address_loads: usize,
    pub registration_target_raw_pointers: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MmenuPreCallError(String);

impl fmt::Display for MmenuPreCallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for MmenuPreCallError {}

fn error(message: impl Into<String>) -> MmenuPreCallError {
    MmenuPreCallError(message.into())
}

fn source_slot_runtime(opcode: u8) -> Result<u32, MmenuPreCallError> {
    let index = opcode
        .checked_sub(EXTENDED_OPCODE_FIRST)
        .ok_or_else(|| error("PUYO2 확장 opcode가 E0보다 작음"))?;
    EXTENDED_OPCODE_SOURCE_RUNTIME
        .checked_add(u32::from(index) * 4)
        .ok_or_else(|| error("PUYO2 확장 opcode source slot 주소 overflow"))
}

fn dispatch_slot_runtime(opcode: u8) -> Result<u32, MmenuPreCallError> {
    OPCODE_TABLE_RUNTIME
        .checked_add(u32::from(opcode) * 4)
        .ok_or_else(|| error("PUYO2 opcode dispatch slot 주소 overflow"))
}

fn executable_word(map: &ExeMap, text: &[u8], runtime: u32) -> Result<u32, MmenuPreCallError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "PUYO2 MMENU pre-call runtime 주소가 EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    text.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
        .ok_or_else(|| {
            error(format!(
                "PUYO2 MMENU pre-call word 범위 이탈: 0x{runtime:08X}"
            ))
        })
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), MmenuPreCallError> {
    let actual = decode(runtime, executable_word(map, text, runtime)?);
    if actual.kind != expected {
        return Err(error(format!(
            "PUYO2 MMENU pre-call typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn direct_jal_call_sites(
    base: u32,
    text: &[u8],
    target: u32,
) -> Result<Vec<u32>, MmenuPreCallError> {
    let instructions =
        decode_slice(base, text).ok_or_else(|| error("PUYO2 MMENU pre-call EXE 연속 해석 실패"))?;
    Ok(instructions
        .into_iter()
        .filter_map(|instruction| {
            (instruction.kind == K::Jal { target }).then_some(instruction.addr)
        })
        .collect())
}

fn addiu_address_parts(runtime: u32) -> (u16, i16) {
    let low = runtime as u16 as i16;
    let high = runtime.wrapping_sub(low as i32 as u32) >> 16;
    (high as u16, low)
}

fn verify_registration_batch(
    map: &ExeMap,
    text: &[u8],
    spec: OpcodeRegistrationBatchSpec,
) -> Result<(), MmenuPreCallError> {
    let (source_high, source_low) = addiu_address_parts(spec.source_table_runtime);
    for (runtime, expected) in [
        (
            spec.source_table_load_runtime,
            K::Lui {
                rt: Reg::A0,
                imm: source_high,
            },
        ),
        (
            spec.source_table_load_runtime + 0x04,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: source_low,
            },
        ),
        (
            spec.first_opcode_load_runtime,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: i16::from(spec.first_opcode),
            },
        ),
        (
            spec.call_runtime,
            K::Jal {
                target: OPCODE_REGISTRATION_TARGET_RUNTIME,
            },
        ),
        (
            spec.call_runtime + 0x04,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: i16::from(OPCODES_PER_REGISTRATION_BATCH),
            },
        ),
    ] {
        require_instruction(map, text, runtime, expected)?;
    }
    Ok(())
}

fn verify_registration_batches(map: &ExeMap, text: &[u8]) -> Result<u32, MmenuPreCallError> {
    let actual_call_sites =
        direct_jal_call_sites(map.load_addr, text, OPCODE_REGISTRATION_TARGET_RUNTIME)?;
    let expected_call_sites = OPCODE_REGISTRATION_BATCH_SPECS
        .iter()
        .map(|spec| spec.call_runtime)
        .collect::<Vec<_>>();
    if actual_call_sites != expected_call_sites {
        return Err(error(format!(
            "PUYO2 opcode 등록 direct call census 불일치: expected={expected_call_sites:08X?}, actual={actual_call_sites:08X?}"
        )));
    }

    let mut covered_opcodes = BTreeSet::new();
    let mut pre_call_registration_runtime = None;
    for spec in OPCODE_REGISTRATION_BATCH_SPECS {
        verify_registration_batch(map, text, spec)?;
        let end = spec
            .first_opcode
            .checked_add(OPCODES_PER_REGISTRATION_BATCH - 1)
            .ok_or_else(|| error("PUYO2 opcode 등록 범위 overflow"))?;
        for opcode in spec.first_opcode..=end {
            if !covered_opcodes.insert(opcode) {
                return Err(error(format!(
                    "PUYO2 opcode 등록 범위 겹침: opcode=0x{opcode:02X}"
                )));
            }
        }
        if (spec.first_opcode..=end).contains(&PRE_CALL_OPCODE) {
            let slot_runtime =
                spec.source_table_runtime + u32::from(PRE_CALL_OPCODE - spec.first_opcode) * 4;
            if slot_runtime != source_slot_runtime(PRE_CALL_OPCODE)?
                || pre_call_registration_runtime
                    .replace(spec.call_runtime)
                    .is_some()
            {
                return Err(error(
                    "PUYO2 E8 source 등록 batch가 유일하지 않거나 slot이 다름",
                ));
            }
        }
    }
    let expected_coverage =
        (REGISTERED_OPCODE_FIRST..=REGISTERED_OPCODE_LAST).collect::<BTreeSet<_>>();
    if covered_opcodes != expected_coverage {
        return Err(error(format!(
            "PUYO2 opcode 등록 범위가 0x60..0xFF를 정확히 덮지 않음: actual={covered_opcodes:02X?}"
        )));
    }
    pre_call_registration_runtime.ok_or_else(|| error("PUYO2 E8 source 등록 batch가 없음"))
}

/// 등록 후 `E8` 기본 handler와 dispatcher의 no-advance 동작을 검증한다.
pub(super) fn verify_puyo2_mmenu_pre_call(
    map: &ExeMap,
    text: &[u8],
) -> Result<MmenuPreCallAnalysis, MmenuPreCallError> {
    if map.name != "PUYO2.EXE" || text.len() != map.text_len as usize {
        return Err(error("PUYO2 EXE 이름 또는 text 길이 불일치"));
    }
    let source_slot = source_slot_runtime(PRE_CALL_OPCODE)?;
    let dispatch_slot = dispatch_slot_runtime(PRE_CALL_OPCODE)?;
    let pre_call_registration_runtime = verify_registration_batches(map, text)?;
    let instructions = decode_slice(map.load_addr, text)
        .ok_or_else(|| error("PUYO2 MMENU pre-call EXE 연속 해석 실패"))?;
    let registration_target_address_loads = find_address_loads(&instructions, DEFAULT_WINDOW)
        .into_iter()
        .filter(|load| load.value == OPCODE_REGISTRATION_TARGET_RUNTIME)
        .count();
    let target_bytes = OPCODE_REGISTRATION_TARGET_RUNTIME.to_le_bytes();
    let registration_target_raw_pointers = text
        .windows(4)
        .filter(|bytes| *bytes == target_bytes)
        .count();
    if registration_target_address_loads != 0 || registration_target_raw_pointers != 0 {
        return Err(error(format!(
            "PUYO2 opcode 등록 target의 간접 정적 참조가 생김: address_loads={registration_target_address_loads}, raw_pointers={registration_target_raw_pointers}"
        )));
    }
    let source_handler = executable_word(map, text, source_slot)?;
    let initial_dispatch_handler = executable_word(map, text, dispatch_slot)?;
    if source_handler != PRE_CALL_DEFAULT_HANDLER_RUNTIME
        || initial_dispatch_handler != PRE_CALL_INITIAL_DISPATCH_HANDLER_RUNTIME
    {
        return Err(error(format!(
            "PUYO2 E8 source/초기 dispatch handler 불일치: source=0x{source_handler:08X}, dispatch=0x{initial_dispatch_handler:08X}"
        )));
    }

    for (runtime, expected) in [
        (PRE_CALL_DEFAULT_HANDLER_RUNTIME, K::Jr { rs: Reg::RA }),
        (
            PRE_CALL_DEFAULT_HANDLER_RUNTIME + 4,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
        (
            OPCODE_DISPATCHER_RUNTIME + 0x1C,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::GP,
                offset: 0x0468,
            },
        ),
        (
            OPCODE_DISPATCHER_RUNTIME + 0x20,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x001C,
            },
        ),
        (
            OPCODE_DISPATCHER_RUNTIME + 0x28,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            OPCODE_DISPATCHER_RUNTIME + 0x30,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            OPCODE_DISPATCHER_RUNTIME + 0x34,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::S1,
            },
        ),
        (
            OPCODE_DISPATCHER_RUNTIME + 0x38,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            OPCODE_DISPATCHER_RUNTIME + 0x40,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        (
            OPCODE_DISPATCHER_RUNTIME + 0x48,
            K::Lw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: 0x0468,
            },
        ),
        (
            OPCODE_DISPATCHER_RUNTIME + 0x50,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: -13,
            },
        ),
    ] {
        require_instruction(map, text, runtime, expected)?;
    }

    Ok(MmenuPreCallAnalysis {
        opcode: PRE_CALL_OPCODE,
        source_slot_runtime: source_slot,
        dispatch_slot_runtime: dispatch_slot,
        default_handler_runtime: PRE_CALL_DEFAULT_HANDLER_RUNTIME,
        initial_dispatch_handler_runtime: PRE_CALL_INITIAL_DISPATCH_HANDLER_RUNTIME,
        registration_batches: OPCODE_REGISTRATION_BATCH_SPECS.len(),
        pre_call_registration_runtime,
        registration_target_address_loads,
        registration_target_raw_pointers,
    })
}

#[cfg(test)]
#[path = "mmenu_pre_call_tests.rs"]
mod tests;
