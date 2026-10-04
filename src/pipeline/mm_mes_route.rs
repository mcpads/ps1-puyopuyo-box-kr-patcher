//! PUYO2 `MMENU.SEQ`의 `MM_MES.BTX` lazy-init 정적 경로.
//!
//! `MMENU.SEQ`의 8개 연속 block은 전역 word `0x57`의 각 bit가 이미
//! 설정됐으면 해당 block을 건너뛰고, 아니면 bit를 설정한 뒤 자산 setup을
//! 수행한다. overlay header가 가리키는 module root의 direct `OP04` 호출과
//! PUYO2의 typed MIPS handler도 함께 검증한다. `MMENU.SEQ` 자체를 고르는
//! 외부 loader, module root의 runtime 실행, pixel draw와 화면 표시는 증명하지
//! 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::isa::{decode, InstructionKind as K, Reg};
use crate::script::parse_subgame_seq_overlay_footer;

use super::mmenu_pre_call::verify_puyo2_mmenu_pre_call;

const PP2_ARCHIVE: &str = "/PUYO2/PP2_SEQ.VDD;1";
const MMENU_SEQUENCE: &str = "MMENU.SEQ";
const MMENU_BYTES: usize = 22_344;
const MMENU_SHA256: &str = "512da8b5a1b391f01217e9494dea3f332a10d8a7def85ed0be6b460d7a2968bf";
const MMENU_LOAD_BASE: u32 = 0x801A_6270;
const MMENU_INITIALIZED_LEN: u32 = 0x571C;
const MMENU_BSS_ADDR: u32 = 0x801A_B98C;
const MMENU_NEXT_ADDR: u32 = 0x801A_B9BC;
const MMENU_MODULE_KIND: u32 = 3;
const MMENU_MODULE_ROOT_POINTER_FIELD_OFFSET: usize = 0x0004;
const MMENU_MODULE_ROOT_OFFSET: usize = 0x5554;
const MMENU_MODULE_ROOT_RUNTIME: u32 = 0x801A_B7C4;
const MMENU_ROOT_OFFSET: usize = 0x21CC;
const MMENU_ROOT_RUNTIME: u32 = 0x801A_843C;
const MMENU_ROOT_POINTER_FIELDS: [usize; 2] = [0x2418, 0x5560];
const MMENU_ROOT_CALL_RECORD_OFFSET: usize = 0x555C;
const MMENU_ROOT_CALL_RECORD_RUNTIME: u32 = 0x801A_B7CC;
const LAZY_INIT_BLOCK_BYTES: usize = 0x48;
const GLOBAL_WORD_INDEX: u8 = 0x57;
const GLOBAL_WORD_TABLE_RUNTIME: u32 = 0x800B_6A70;
const GLOBAL_WORD_RUNTIME: u32 = 0x800B_6BCC;
const MM_MES_RESOURCE_ID: u16 = 165;
const MM_MES_SETUP_RUNTIME: u32 = 0x801A_849C;
const PP2_SEQ_FILES: usize = 151;
const PP2_TYPE1_SELECTOR_RECORDS: usize = 352;
const PP2_TYPE1_SELECTOR_RESOURCES: usize = 192;
const PP2_EXE_TYPE1_SELECTOR_RECORDS: usize = 1;

const PP2_OPCODE_TABLE_RUNTIME: u32 = 0x8008_B740;
const OP04_HANDLER_RUNTIME: u32 = 0x8002_299C;
const OP0F_HANDLER_RUNTIME: u32 = 0x8002_04AC;
const OP12_HANDLER_RUNTIME: u32 = 0x8002_0DBC;
const OP0F_OPERATION_TABLE_RUNTIME: u32 = 0x8001_04E4;
const OP12_COMPARE_TABLE_RUNTIME: u32 = 0x8001_05A4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LazyInitSpec {
    mask: u8,
    resource_id: u16,
}

const LAZY_INIT_SPECS: [LazyInitSpec; 8] = [
    LazyInitSpec {
        mask: 0x01,
        resource_id: 162,
    },
    LazyInitSpec {
        mask: 0x02,
        resource_id: 165,
    },
    LazyInitSpec {
        mask: 0x04,
        resource_id: 166,
    },
    LazyInitSpec {
        mask: 0x08,
        resource_id: 163,
    },
    LazyInitSpec {
        mask: 0x10,
        resource_id: 164,
    },
    LazyInitSpec {
        mask: 0x20,
        resource_id: 167,
    },
    LazyInitSpec {
        mask: 0x40,
        resource_id: 160,
    },
    LazyInitSpec {
        mask: 0x80,
        resource_id: 161,
    },
];

/// `MMENU.SEQ` 안에서 검증한 one-time setup block 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmMesLazyInitBlock {
    pub record_offset: usize,
    pub record_runtime: u32,
    pub mask: u8,
    pub resource_id: u16,
    pub setup_runtime: u32,
    pub skip_runtime: u32,
}

/// 지원 원본에서 닫힌 `MM_MES` lazy-init 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmMesRouteAnalysis {
    pub sequence_bytes: usize,
    pub load_base: u32,
    pub module_root_pointer_field_offset: usize,
    pub module_root_runtime: u32,
    pub root_call_record_offset: usize,
    pub root_call_record_runtime: u32,
    pub pre_call_opcode: u8,
    pub pre_call_source_slot_runtime: u32,
    pub pre_call_dispatch_slot_runtime: u32,
    pub pre_call_default_handler_runtime: u32,
    pub pre_call_initial_dispatch_handler_runtime: u32,
    pub pre_call_registration_batches: usize,
    pub pre_call_registration_runtime: u32,
    pub pre_call_registration_target_address_loads: usize,
    pub pre_call_registration_target_raw_pointers: usize,
    pub root_runtime: u32,
    pub root_pointer_field_offsets: Vec<usize>,
    pub global_word_index: u8,
    pub global_word_runtime: u32,
    pub blocks: Vec<MmMesLazyInitBlock>,
    pub mm_mes_resource_id: u16,
    pub mm_mes_setup_runtime: u32,
    pub pp2_seq_files: usize,
    pub direct_sequence_selector_records: usize,
    pub direct_sequence_selector_resources: usize,
    pub direct_sequence_fallback_records: usize,
    pub direct_sequence_selections: usize,
    pub direct_executable_selector_records: usize,
    pub direct_executable_fallback_records: usize,
    pub direct_executable_selections: usize,
}

/// `MM_MES` lazy-init 정적 경로 source gate 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmMesRouteError(String);

impl fmt::Display for MmMesRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for MmMesRouteError {}

fn error(message: impl Into<String>) -> MmMesRouteError {
    MmMesRouteError(message.into())
}

fn read_u32(raw: &[u8], offset: usize, label: &str) -> Result<u32, MmMesRouteError> {
    raw.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
        .ok_or_else(|| error(format!("{label} word 범위 이탈: +0x{offset:X}")))
}

fn aligned_pointer_offsets(raw: &[u8], initialized_len: usize, target: u32) -> Vec<usize> {
    raw[..initialized_len]
        .chunks_exact(4)
        .enumerate()
        .filter_map(|(index, bytes)| {
            (u32::from_le_bytes(bytes.try_into().expect("4바이트 word")) == target)
                .then_some(index * 4)
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectType1SelectorRecord {
    offset: usize,
    resource_id: u16,
    reserved: u16,
}

/// byte 2의 flags 값은 제한하지 않고 `OP28/sub10/type1` shape를 넓게 센다.
/// 정렬되지 않은 우연 일치도 숨기지 않으며, 정렬·reserved 계약은 호출자가
/// 전체 census에서 별도로 검증한다.
fn direct_type1_selector_records(raw: &[u8]) -> Vec<DirectType1SelectorRecord> {
    raw.windows(8)
        .enumerate()
        .filter_map(|(offset, bytes)| {
            (bytes[0] == 0x28 && bytes[1] == 0x10 && bytes[3] == 0x01).then_some(
                DirectType1SelectorRecord {
                    offset,
                    resource_id: u16::from_le_bytes([bytes[4], bytes[5]]),
                    reserved: u16::from_le_bytes([bytes[6], bytes[7]]),
                },
            )
        })
        .collect()
}

fn runtime_at(offset: usize) -> Result<u32, MmMesRouteError> {
    MMENU_LOAD_BASE
        .checked_add(u32::try_from(offset).map_err(|_| error("MMENU offset이 u32를 넘음"))?)
        .ok_or_else(|| error("MMENU runtime 주소 overflow"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MmMesModuleEntry {
    pointer_field_offset: usize,
    module_root_runtime: u32,
    root_call_record_offset: usize,
    root_call_record_runtime: u32,
}

/// overlay header의 module root와 그 안의 direct `OP04` root call을 묶는다.
/// 중간 `E8`의 handler 의미는 EXE 검증에서 별도로 닫고 runtime 도달은
/// 이 정적 계약에 포함하지 않는다.
fn verify_module_entry(raw: &[u8]) -> Result<MmMesModuleEntry, MmMesRouteError> {
    if read_u32(raw, 0, "MMENU module kind")? != MMENU_MODULE_KIND {
        return Err(error("MMENU module kind 불일치"));
    }
    if read_u32(
        raw,
        MMENU_MODULE_ROOT_POINTER_FIELD_OFFSET,
        "MMENU module root pointer",
    )? != MMENU_MODULE_ROOT_RUNTIME
    {
        return Err(error("MMENU module root pointer 불일치"));
    }
    let pointer_fields = aligned_pointer_offsets(
        raw,
        MMENU_INITIALIZED_LEN as usize,
        MMENU_MODULE_ROOT_RUNTIME,
    );
    if pointer_fields != [MMENU_MODULE_ROOT_POINTER_FIELD_OFFSET] {
        return Err(error(format!(
            "MMENU module root pointer field census 불일치: {pointer_fields:X?}"
        )));
    }
    let module_prefix = raw
        .get(MMENU_MODULE_ROOT_OFFSET..MMENU_MODULE_ROOT_OFFSET + 8)
        .ok_or_else(|| error("MMENU module root prefix 범위 이탈"))?;
    if module_prefix != [0x0F, 0x40, GLOBAL_WORD_INDEX, 0x00, 0xE8, 0, 0, 0] {
        return Err(error(format!(
            "MMENU module root prefix 불일치: {module_prefix:02X?}"
        )));
    }
    let call = raw
        .get(MMENU_ROOT_CALL_RECORD_OFFSET..MMENU_ROOT_CALL_RECORD_OFFSET + 8)
        .ok_or_else(|| error("MMENU root call record 범위 이탈"))?;
    if call[..4] != [0x04, 0, 0, 0]
        || u32::from_le_bytes(call[4..8].try_into().expect("root pointer word"))
            != MMENU_ROOT_RUNTIME
    {
        return Err(error(format!(
            "MMENU direct OP04 root call 불일치: {call:02X?}"
        )));
    }
    Ok(MmMesModuleEntry {
        pointer_field_offset: MMENU_MODULE_ROOT_POINTER_FIELD_OFFSET,
        module_root_runtime: MMENU_MODULE_ROOT_RUNTIME,
        root_call_record_offset: MMENU_ROOT_CALL_RECORD_OFFSET,
        root_call_record_runtime: MMENU_ROOT_CALL_RECORD_RUNTIME,
    })
}

fn verify_lazy_init_chain(raw: &[u8]) -> Result<Vec<MmMesLazyInitBlock>, MmMesRouteError> {
    let mut blocks = Vec::with_capacity(LAZY_INIT_SPECS.len());
    for (index, spec) in LAZY_INIT_SPECS.iter().enumerate() {
        let record_offset = MMENU_ROOT_OFFSET + index * LAZY_INIT_BLOCK_BYTES;
        let block = raw
            .get(record_offset..record_offset + LAZY_INIT_BLOCK_BYTES)
            .ok_or_else(|| error(format!("MMENU lazy-init block {index} 범위 이탈")))?;
        let expected_condition = [0x12, 0x46, GLOBAL_WORD_INDEX, spec.mask];
        if block[..4] != expected_condition {
            return Err(error(format!(
                "MMENU lazy-init block {index} 조건 record 불일치: actual={:02X?}",
                &block[..4]
            )));
        }
        let expected_skip = runtime_at(record_offset + LAZY_INIT_BLOCK_BYTES)?;
        let actual_skip = read_u32(block, 4, "MMENU lazy-init skip")?;
        if actual_skip != expected_skip {
            return Err(error(format!(
                "MMENU lazy-init block {index} skip 불일치: expected=0x{expected_skip:08X}, actual=0x{actual_skip:08X}"
            )));
        }
        let expected_set = [0x0F, 0x47, GLOBAL_WORD_INDEX, spec.mask];
        if block[8..12] != expected_set {
            return Err(error(format!(
                "MMENU lazy-init block {index} bit set record 불일치: actual={:02X?}",
                &block[8..12]
            )));
        }
        let setup = &block[0x18..0x28];
        if setup[..4] != [0x28, 0x12, 0x97, 0x00]
            || u16::from_le_bytes(setup[4..6].try_into().expect("resource word"))
                != spec.resource_id
        {
            return Err(error(format!(
                "MMENU lazy-init block {index} resource setup 불일치: expected={}, actual={:02X?}",
                spec.resource_id, setup
            )));
        }
        blocks.push(MmMesLazyInitBlock {
            record_offset,
            record_runtime: runtime_at(record_offset)?,
            mask: spec.mask,
            resource_id: spec.resource_id,
            setup_runtime: runtime_at(record_offset + 0x18)?,
            skip_runtime: actual_skip,
        });
    }
    Ok(blocks)
}

fn executable_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    runtime: u32,
    len: usize,
) -> Result<&'a [u8], MmMesRouteError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "PUYO2 MM_MES route runtime 주소가 EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    text.get(offset..offset + len).ok_or_else(|| {
        error(format!(
            "PUYO2 MM_MES route EXE text 범위 이탈: 0x{runtime:08X}+{len}"
        ))
    })
}

fn executable_word(map: &ExeMap, text: &[u8], runtime: u32) -> Result<u32, MmMesRouteError> {
    Ok(u32::from_le_bytes(
        executable_slice(map, text, runtime, 4)?
            .try_into()
            .expect("4바이트 word"),
    ))
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), MmMesRouteError> {
    let actual = decode(runtime, executable_word(map, text, runtime)?);
    if actual.kind != expected {
        return Err(error(format!(
            "PUYO2 MM_MES route typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn verify_handler_words(map: &ExeMap, text: &[u8]) -> Result<(), MmMesRouteError> {
    if executable_word(map, text, PP2_OPCODE_TABLE_RUNTIME + 0x04 * 4)? != OP04_HANDLER_RUNTIME
        || executable_word(map, text, PP2_OPCODE_TABLE_RUNTIME + 0x0F * 4)? != OP0F_HANDLER_RUNTIME
        || executable_word(map, text, PP2_OPCODE_TABLE_RUNTIME + 0x12 * 4)? != OP12_HANDLER_RUNTIME
    {
        return Err(error("PUYO2 OP04/OP0F/OP12 handler table pointer 불일치"));
    }
    if executable_word(map, text, OP12_COMPARE_TABLE_RUNTIME + 6 * 4)? != 0x8002_0F44 {
        return Err(error("PUYO2 OP12 비교 index 6 AND handler pointer 불일치"));
    }
    if executable_word(map, text, OP0F_OPERATION_TABLE_RUNTIME + 7 * 4)? != 0x8002_066C {
        return Err(error("PUYO2 OP0F 연산 index 7 OR handler pointer 불일치"));
    }
    Ok(())
}

/// `04 00 00 00 <pointer>`가 현재 record 다음 주소를 return stack에 넣고
/// record `+4`의 pointer로 분기하는 PUYO2 고유 object layout을 고정한다.
fn verify_op04_call(map: &ExeMap, text: &[u8]) -> Result<(), MmMesRouteError> {
    for (runtime, expected) in [
        (
            0x8002_299C,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0x40,
            },
        ),
        (
            0x8002_29A0,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8002_29A4,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
        ),
        (
            0x8002_29A8,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::A0,
            },
        ),
        (
            0x8002_29AC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
        (
            0x8002_29B0,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x28,
            },
        ),
        (
            0x8002_29B4,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x40,
            },
        ),
        (
            0x8002_29B8,
            K::Lw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8002_29BC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
        (
            0x8002_29C0,
            K::Sb {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x40,
            },
        ),
        (
            0x8002_29C4,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 4,
            },
        ),
        (0x8002_29C8, K::Jr { rs: Reg::RA }),
        (
            0x8002_29CC,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
    ] {
        require_instruction(map, text, runtime, expected)?;
    }
    Ok(())
}

/// `12 46 index mask pointer`가 `global[index] & mask != 0`이면 pointer로
/// 건너뛰고, 아니면 8바이트 뒤로 진행하는 경로를 typed 명령으로 고정한다.
fn verify_op12_skip_if_set(map: &ExeMap, text: &[u8]) -> Result<(), MmMesRouteError> {
    for (runtime, expected) in [
        (
            0x8002_0DBC,
            K::Lw {
                rt: Reg::A3,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8002_0DC4,
            K::Lbu {
                rt: Reg::T1,
                base: Reg::A3,
                offset: 1,
            },
        ),
        (
            0x8002_0E08,
            K::Lui {
                rt: Reg::V1,
                imm: 0x800B,
            },
        ),
        (
            0x8002_0E0C,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::A3,
                offset: 2,
            },
        ),
        (
            0x8002_0E10,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x6A70,
            },
        ),
        (
            0x8002_0E18,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8002_0E34,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8002_0E38,
            K::Lb {
                rt: Reg::A2,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8002_0E84,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8002_0E8C,
            K::Lb {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 3,
            },
        ),
        (
            0x8002_0E94,
            K::Andi {
                rt: Reg::V1,
                rs: Reg::T1,
                imm: 0x000F,
            },
        ),
        (
            0x8002_0F44,
            K::And {
                rd: Reg::V0,
                rs: Reg::A2,
                rt: Reg::A1,
            },
        ),
        (
            0x8002_0F4C,
            K::Sltu {
                rd: Reg::T0,
                rs: Reg::ZERO,
                rt: Reg::V0,
            },
        ),
        (
            0x8002_0F90,
            K::Beq {
                rs: Reg::T0,
                rt: Reg::ZERO,
                offset: 6,
            },
        ),
        (
            0x8002_0F98,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8002_0FA0,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8002_0FA8,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8002_0FB4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 4,
            },
        ),
        (
            0x8002_0FBC,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
    ] {
        require_instruction(map, text, runtime, expected)?;
    }
    Ok(())
}

/// `0F 47 index mask`가 `global[index] = (global[index] | mask) & 0xFF`를
/// 수행한 뒤 4바이트 진행하는 경로를 typed 명령으로 고정한다.
fn verify_op0f_set_bit(map: &ExeMap, text: &[u8]) -> Result<(), MmMesRouteError> {
    for (runtime, expected) in [
        (
            0x8002_04C8,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S2,
                offset: 0x1C,
            },
        ),
        (
            0x8002_04D0,
            K::Lbu {
                rt: Reg::S3,
                base: Reg::A0,
                offset: 1,
            },
        ),
        (
            0x8002_04D8,
            K::Andi {
                rt: Reg::V1,
                rs: Reg::S3,
                imm: 0x0030,
            },
        ),
        (
            0x8002_04EC,
            K::Beq {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 10,
            },
        ),
        (
            0x8002_0518,
            K::Lbu {
                rt: Reg::S1,
                base: Reg::A0,
                offset: 3,
            },
        ),
        (
            0x8002_0564,
            K::Andi {
                rt: Reg::V1,
                rs: Reg::S3,
                imm: 0x00C0,
            },
        ),
        (
            0x8002_057C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x40,
            },
        ),
        (
            0x8002_0580,
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 8,
            },
        ),
        (
            0x8002_05A8,
            K::Lui {
                rt: Reg::V1,
                imm: 0x800B,
            },
        ),
        (
            0x8002_05AC,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 2,
            },
        ),
        (
            0x8002_05B0,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x6A70,
            },
        ),
        (
            0x8002_05B4,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8002_05B8,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8002_05BC,
            K::Lw {
                rt: Reg::S0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8002_0600,
            K::Andi {
                rt: Reg::V1,
                rs: Reg::S3,
                imm: 0x000F,
            },
        ),
        (
            0x8002_0670,
            K::Or {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::S1,
            },
        ),
        (
            0x8002_06EC,
            K::Andi {
                rt: Reg::V1,
                rs: Reg::S3,
                imm: 0x00C0,
            },
        ),
        (
            0x8002_0704,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x40,
            },
        ),
        (
            0x8002_0708,
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 8,
            },
        ),
        (
            0x8002_072C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S2,
                offset: 0x1C,
            },
        ),
        (
            0x8002_0734,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 2,
            },
        ),
        (
            0x8002_0738,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x6A70,
            },
        ),
        (
            0x8002_073C,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8002_0740,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8002_0744,
            K::Andi {
                rt: Reg::V1,
                rs: Reg::S0,
                imm: 0x00FF,
            },
        ),
        (
            0x8002_074C,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8002_0788,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 4,
            },
        ),
        (
            0x8002_078C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S2,
                offset: 0x1C,
            },
        ),
    ] {
        require_instruction(map, text, runtime, expected)?;
    }
    Ok(())
}

fn verify_handlers(map: &ExeMap, text: &[u8]) -> Result<(), MmMesRouteError> {
    if map.name != "PUYO2.EXE" || text.len() != map.text_len as usize {
        return Err(error("PUYO2 EXE 이름 또는 text 길이 불일치"));
    }
    verify_handler_words(map, text)?;
    verify_op04_call(map, text)?;
    verify_op12_skip_if_set(map, text)?;
    verify_op0f_set_bit(map, text)
}

/// 지원 원본의 `MMENU.SEQ` lazy-init chain과 bounded 상위 선택 census를
/// 검증한다.
pub fn verify_box_mm_mes_route<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
) -> Result<MmMesRouteAnalysis, MmMesRouteError> {
    let pp2_executables = executables
        .into_iter()
        .filter(|(map, _)| map.name == "PUYO2.EXE")
        .collect::<Vec<_>>();
    let [(map, text)] = pp2_executables.as_slice() else {
        return Err(error(format!(
            "PUYO2 EXE 입력 수가 1이 아님: {}",
            pp2_executables.len()
        )));
    };
    verify_handlers(map, text)?;
    let pre_call =
        verify_puyo2_mmenu_pre_call(map, text).map_err(|source| error(source.to_string()))?;

    let mut pp2_names = BTreeSet::new();
    let mut mmenu_sources = Vec::new();
    let mut direct_sequence_selector_records = Vec::new();
    let mut direct_sequence_selector_resources = BTreeSet::new();
    let mut direct_sequence_fallback_records = 0usize;
    let mut direct_sequence_selections = Vec::new();
    for (archive, sequence, raw) in sources {
        if archive != PP2_ARCHIVE {
            continue;
        }
        if !pp2_names.insert(sequence) {
            return Err(error(format!("PP2 SEQ 입력 중복: {sequence}")));
        }
        if sequence == MMENU_SEQUENCE {
            mmenu_sources.push(raw);
        }
        for record in direct_type1_selector_records(raw) {
            if !record.offset.is_multiple_of(4) || record.reserved != 0 {
                return Err(error(format!(
                    "PP2 type-1 selector record 정렬/reserved 불일치: {sequence}+0x{:X}: {record:?}",
                    record.offset
                )));
            }
            direct_sequence_selector_resources.insert(record.resource_id);
            direct_sequence_fallback_records += usize::from(record.resource_id == 0);
            if record.resource_id == 62 {
                direct_sequence_selections.push((sequence.to_string(), record.offset));
            }
            direct_sequence_selector_records.push((sequence.to_string(), record));
        }
    }
    if pp2_names.len() != PP2_SEQ_FILES {
        return Err(error(format!(
            "PP2 SEQ 파일 수 불일치: expected={PP2_SEQ_FILES}, actual={}",
            pp2_names.len()
        )));
    }
    if direct_sequence_selector_records.len() != PP2_TYPE1_SELECTOR_RECORDS
        || direct_sequence_selector_resources.len() != PP2_TYPE1_SELECTOR_RESOURCES
        || direct_sequence_fallback_records != 0
    {
        return Err(error(format!(
            "PP2 type-1 selector census 불일치: records={}, resources={}, fallback={direct_sequence_fallback_records}",
            direct_sequence_selector_records.len(),
            direct_sequence_selector_resources.len()
        )));
    }
    let [raw] = mmenu_sources.as_slice() else {
        return Err(error(format!(
            "PP2 MMENU.SEQ 입력 수가 1이 아님: {}",
            mmenu_sources.len()
        )));
    };
    if raw.len() != MMENU_BYTES || format!("{:x}", Sha256::digest(raw)) != MMENU_SHA256 {
        return Err(error(format!(
            "PP2 MMENU.SEQ 원본 identity 불일치: bytes={}, sha256={:x}",
            raw.len(),
            Sha256::digest(raw)
        )));
    }
    let footer = parse_subgame_seq_overlay_footer(raw)
        .map_err(|source| error(format!("PP2 MMENU.SEQ footer: {source}")))?
        .ok_or_else(|| error("PP2 MMENU.SEQ가 overlay가 아님"))?;
    if footer.load_base != MMENU_LOAD_BASE
        || footer.initialized_len != MMENU_INITIALIZED_LEN
        || footer.bss_addr != MMENU_BSS_ADDR
        || footer.bss_len != 0x2C
        || footer.next_addr != MMENU_NEXT_ADDR
    {
        return Err(error(format!(
            "PP2 MMENU.SEQ footer 계약 불일치: {footer:?}"
        )));
    }
    let module_entry = verify_module_entry(raw)?;
    let pointer_fields = aligned_pointer_offsets(raw, footer.footer_offset, MMENU_ROOT_RUNTIME);
    if pointer_fields != MMENU_ROOT_POINTER_FIELDS {
        return Err(error(format!(
            "PP2 MMENU root pointer fields 불일치: expected={MMENU_ROOT_POINTER_FIELDS:X?}, actual={pointer_fields:X?}"
        )));
    }
    let blocks = verify_lazy_init_chain(raw)?;
    let mm_mes = blocks
        .iter()
        .find(|block| block.resource_id == MM_MES_RESOURCE_ID)
        .copied()
        .ok_or_else(|| error("PP2 MM_MES lazy-init block 누락"))?;
    if mm_mes.mask != 0x02 || mm_mes.setup_runtime != MM_MES_SETUP_RUNTIME {
        return Err(error(format!(
            "PP2 MM_MES lazy-init block 불일치: {mm_mes:?}"
        )));
    }

    if !direct_sequence_selections.is_empty() {
        return Err(error(format!(
            "PP2 resource 62 canonical direct selection이 새로 발견됨: {direct_sequence_selections:?}"
        )));
    }
    let direct_executable_selector_records = direct_type1_selector_records(text);
    let direct_executable_fallback_records = direct_executable_selector_records
        .iter()
        .filter(|record| record.resource_id == 0)
        .count();
    if direct_executable_selector_records.len() != PP2_EXE_TYPE1_SELECTOR_RECORDS
        || direct_executable_selector_records
            .iter()
            .any(|record| !record.offset.is_multiple_of(4) || record.reserved != 0)
        || direct_executable_fallback_records != 0
    {
        return Err(error(format!(
            "PUYO2.EXE type-1 selector census 불일치: {direct_executable_selector_records:?}"
        )));
    }
    let direct_executable_selections = direct_executable_selector_records
        .iter()
        .filter(|record| record.resource_id == 62)
        .collect::<Vec<_>>();
    if !direct_executable_selections.is_empty() {
        return Err(error(format!(
            "PUYO2.EXE resource 62 canonical direct selection이 새로 발견됨: {direct_executable_selections:X?}"
        )));
    }
    let expected_global_runtime = GLOBAL_WORD_TABLE_RUNTIME + u32::from(GLOBAL_WORD_INDEX) * 4;
    if expected_global_runtime != GLOBAL_WORD_RUNTIME {
        return Err(error("PP2 MMENU lazy-init 전역 word 주소 계산 불일치"));
    }

    Ok(MmMesRouteAnalysis {
        sequence_bytes: raw.len(),
        load_base: footer.load_base,
        module_root_pointer_field_offset: module_entry.pointer_field_offset,
        module_root_runtime: module_entry.module_root_runtime,
        root_call_record_offset: module_entry.root_call_record_offset,
        root_call_record_runtime: module_entry.root_call_record_runtime,
        pre_call_opcode: pre_call.opcode,
        pre_call_source_slot_runtime: pre_call.source_slot_runtime,
        pre_call_dispatch_slot_runtime: pre_call.dispatch_slot_runtime,
        pre_call_default_handler_runtime: pre_call.default_handler_runtime,
        pre_call_initial_dispatch_handler_runtime: pre_call.initial_dispatch_handler_runtime,
        pre_call_registration_batches: pre_call.registration_batches,
        pre_call_registration_runtime: pre_call.pre_call_registration_runtime,
        pre_call_registration_target_address_loads: pre_call.registration_target_address_loads,
        pre_call_registration_target_raw_pointers: pre_call.registration_target_raw_pointers,
        root_runtime: MMENU_ROOT_RUNTIME,
        root_pointer_field_offsets: pointer_fields,
        global_word_index: GLOBAL_WORD_INDEX,
        global_word_runtime: GLOBAL_WORD_RUNTIME,
        blocks,
        mm_mes_resource_id: mm_mes.resource_id,
        mm_mes_setup_runtime: mm_mes.setup_runtime,
        pp2_seq_files: pp2_names.len(),
        direct_sequence_selector_records: direct_sequence_selector_records.len(),
        direct_sequence_selector_resources: direct_sequence_selector_resources.len(),
        direct_sequence_fallback_records,
        direct_sequence_selections: direct_sequence_selections.len(),
        direct_executable_selector_records: direct_executable_selector_records.len(),
        direct_executable_fallback_records,
        direct_executable_selections: direct_executable_selections.len(),
    })
}

#[cfg(test)]
#[path = "mm_mes_route_tests.rs"]
mod tests;
