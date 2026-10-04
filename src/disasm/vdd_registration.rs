//! VDD descriptor block 등록의 공용 정적 경계.
//!
//! command `0x29`는 resource type과 VDD descriptor block을 type table에
//! 등록하고 같은 block을 이름 기반 loader에 넘긴다. 이 모듈은 그 기계어와
//! archive descriptor만 검증한다. 개별 resource가 이후 선택·적재·표시된다는
//! 주장은 호출자가 별도로 세워야 한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;

pub const VDD_REGISTRATION_OPCODE: u8 = 0x29;
pub const VDD_DESCRIPTOR_STRIDE: u32 = 36;

/// 지원 원본에서 확인한 VDD 등록 loader의 compiler family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VddRegistrationFamily {
    Shared,
    Pcomi,
}

/// VDD block 하나의 등록 record·handler·loader 사양.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VddRegistrationSourceSpec {
    pub exe: &'static str,
    pub resource_type: u8,
    pub archive: &'static str,
    pub registration_record_runtime: u32,
    pub registration_handler_runtime: u32,
    pub loader_runtime: u32,
    pub table_root_runtime: u32,
    pub archive_descriptor_runtime: u32,
    pub archive_name_runtime: u32,
    pub family: VddRegistrationFamily,
}

/// VDD 등록 원본 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VddRegistrationError(String);

impl fmt::Display for VddRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for VddRegistrationError {}

fn error(message: impl Into<String>) -> VddRegistrationError {
    VddRegistrationError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, VddRegistrationError> {
    instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| error(format!("VDD 등록 명령 주소 없음: 0x{address:08X}")))
}

fn require(
    instructions: &[Instruction],
    spec: VddRegistrationSourceSpec,
    address: u32,
    expected: K,
) -> Result<(), VddRegistrationError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "{} VDD 등록 명령 불일치: address=0x{address:08X}, expected={expected:?}, actual={:?}",
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

fn read_word(raw: &[u8], offset: usize) -> Result<u32, VddRegistrationError> {
    raw.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
        .ok_or_else(|| error(format!("VDD 등록 word 범위 이탈: +0x{offset:X}")))
}

fn verify_registration_record(
    spec: VddRegistrationSourceSpec,
    map: &ExeMap,
    text: &[u8],
) -> Result<(), VddRegistrationError> {
    let offset = map
        .text_offset(spec.registration_record_runtime)
        .ok_or_else(|| error(format!("{} 등록 record가 EXE text 밖임", spec.exe)))?;
    let raw = text
        .get(offset..offset + 8)
        .ok_or_else(|| error(format!("{} 등록 record 8바이트 범위 이탈", spec.exe)))?;
    let expected_header = u32::from_le_bytes([VDD_REGISTRATION_OPCODE, spec.resource_type, 0, 0]);
    if read_word(raw, 0)? != expected_header
        || read_word(raw, 4)? != spec.archive_descriptor_runtime
    {
        return Err(error(format!(
            "{} VDD type {} 등록 record 불일치: runtime=0x{:08X}",
            spec.exe, spec.resource_type, spec.registration_record_runtime
        )));
    }
    Ok(())
}

fn verify_archive_descriptor(
    spec: VddRegistrationSourceSpec,
    map: &ExeMap,
    text: &[u8],
) -> Result<(), VddRegistrationError> {
    let offset = map
        .text_offset(spec.archive_descriptor_runtime)
        .ok_or_else(|| error(format!("{} VDD descriptor가 EXE text 밖임", spec.exe)))?;
    let raw = text
        .get(offset..offset + VDD_DESCRIPTOR_STRIDE as usize)
        .ok_or_else(|| error(format!("{} VDD descriptor 36바이트 범위 이탈", spec.exe)))?;
    let expected = [0x0080_0000, spec.archive_name_runtime, 0, 0, 0, 0, 0, 0, 0];
    let mut actual = [0u32; 9];
    for (index, word) in actual.iter_mut().enumerate() {
        *word = read_word(raw, index * 4)?;
    }
    if actual != expected {
        return Err(error(format!(
            "{} VDD descriptor 불일치: expected={expected:08X?}, actual={actual:08X?}",
            spec.exe
        )));
    }

    let name_offset = map
        .text_offset(spec.archive_name_runtime)
        .ok_or_else(|| error(format!("{} VDD 이름표가 EXE text 밖임", spec.exe)))?;
    let expected_name = format!("{}\0", spec.archive);
    let actual_name = text
        .get(name_offset..name_offset + expected_name.len())
        .ok_or_else(|| error(format!("{} VDD 이름표 범위 이탈", spec.exe)))?;
    if actual_name != expected_name.as_bytes() {
        return Err(error(format!(
            "{} VDD 이름표 불일치: runtime=0x{:08X}, expected={}\\0",
            spec.exe, spec.archive_name_runtime, spec.archive
        )));
    }
    Ok(())
}

fn verify_handler(
    instructions: &[Instruction],
    spec: VddRegistrationSourceSpec,
) -> Result<(), VddRegistrationError> {
    let start = spec.registration_handler_runtime;
    let (table_high, table_low) = high_low(spec.table_root_runtime);
    let mut checks = vec![(
        start + 0x0C,
        K::Lui {
            rt: Reg::V1,
            imm: table_high,
        },
    )];
    match spec.family {
        VddRegistrationFamily::Shared => checks.extend([
            (
                start + 0x14,
                K::Lw {
                    rt: Reg::A0,
                    base: Reg::S0,
                    offset: 0x1C,
                },
            ),
            (
                start + 0x18,
                K::Addiu {
                    rt: Reg::V1,
                    rs: Reg::V1,
                    imm: table_low,
                },
            ),
            (
                start + 0x1C,
                K::Lbu {
                    rt: Reg::V0,
                    base: Reg::A0,
                    offset: 1,
                },
            ),
            (
                start + 0x20,
                K::Lw {
                    rt: Reg::A0,
                    base: Reg::A0,
                    offset: 4,
                },
            ),
            (
                start + 0x24,
                K::Sll {
                    rd: Reg::V0,
                    rt: Reg::V0,
                    sa: 2,
                },
            ),
            (
                start + 0x28,
                K::Addu {
                    rd: Reg::V0,
                    rs: Reg::V0,
                    rt: Reg::V1,
                },
            ),
            (
                start + 0x2C,
                K::Sw {
                    rt: Reg::A0,
                    base: Reg::V0,
                    offset: 0,
                },
            ),
            (
                start + 0x38,
                K::Lbu {
                    rt: Reg::A0,
                    base: Reg::V0,
                    offset: 1,
                },
            ),
            (
                start + 0x3C,
                K::Lw {
                    rt: Reg::A1,
                    base: Reg::V0,
                    offset: 4,
                },
            ),
            (
                start + 0x40,
                K::Jal {
                    target: spec.loader_runtime,
                },
            ),
            (
                start + 0x74,
                K::Addiu {
                    rt: Reg::V0,
                    rs: Reg::V0,
                    imm: 8,
                },
            ),
            (
                start + 0x78,
                K::Sw {
                    rt: Reg::V0,
                    base: Reg::S0,
                    offset: 0x1C,
                },
            ),
        ]),
        VddRegistrationFamily::Pcomi => checks.extend([
            (
                start + 0x14,
                K::Lw {
                    rt: Reg::A1,
                    base: Reg::S0,
                    offset: 0x1C,
                },
            ),
            (
                start + 0x18,
                K::Addiu {
                    rt: Reg::V1,
                    rs: Reg::V1,
                    imm: table_low,
                },
            ),
            (
                start + 0x1C,
                K::Lbu {
                    rt: Reg::V0,
                    base: Reg::A1,
                    offset: 1,
                },
            ),
            (
                start + 0x20,
                K::Lw {
                    rt: Reg::A0,
                    base: Reg::A1,
                    offset: 4,
                },
            ),
            (
                start + 0x24,
                K::Sll {
                    rd: Reg::V0,
                    rt: Reg::V0,
                    sa: 2,
                },
            ),
            (
                start + 0x28,
                K::Addu {
                    rd: Reg::V0,
                    rs: Reg::V0,
                    rt: Reg::V1,
                },
            ),
            (
                start + 0x2C,
                K::Sw {
                    rt: Reg::A0,
                    base: Reg::V0,
                    offset: 0,
                },
            ),
            (
                start + 0x38,
                K::Lbu {
                    rt: Reg::A0,
                    base: Reg::V1,
                    offset: 1,
                },
            ),
            (
                start + 0x3C,
                K::Lw {
                    rt: Reg::A1,
                    base: Reg::V1,
                    offset: 4,
                },
            ),
            (
                start + 0x40,
                K::Jal {
                    target: spec.loader_runtime,
                },
            ),
            (
                start + 0x70,
                K::Addiu {
                    rt: Reg::V0,
                    rs: Reg::V1,
                    imm: 8,
                },
            ),
            (
                start + 0x74,
                K::Sw {
                    rt: Reg::V0,
                    base: Reg::S0,
                    offset: 0x1C,
                },
            ),
        ]),
    }
    for (address, expected) in checks {
        require(instructions, spec, address, expected)?;
    }
    Ok(())
}

fn verify_loader(
    instructions: &[Instruction],
    spec: VddRegistrationSourceSpec,
) -> Result<(), VddRegistrationError> {
    let start = spec.loader_runtime;
    let (table_high, table_low) = high_low(spec.table_root_runtime);
    match spec.family {
        VddRegistrationFamily::Shared => {
            let checks = [
                (
                    start + 8,
                    K::Addu {
                        rd: Reg::FP,
                        rs: Reg::A1,
                        rt: Reg::ZERO,
                    },
                ),
                (
                    start + 0x34,
                    K::Lhu {
                        rt: Reg::A3,
                        base: Reg::FP,
                        offset: 2,
                    },
                ),
                (
                    start + 0x38,
                    K::Lw {
                        rt: Reg::V0,
                        base: Reg::FP,
                        offset: 4,
                    },
                ),
                (
                    start + 0xA0,
                    K::Addiu {
                        rt: Reg::V1,
                        rs: Reg::FP,
                        imm: VDD_DESCRIPTOR_STRIDE as i16,
                    },
                ),
                (
                    start + 0xA4,
                    K::Lhu {
                        rt: Reg::V0,
                        base: Reg::V1,
                        offset: 2,
                    },
                ),
                (
                    start + 0x204,
                    K::Lui {
                        rt: Reg::V1,
                        imm: table_high,
                    },
                ),
                (
                    start + 0x20C,
                    K::Addiu {
                        rt: Reg::V1,
                        rs: Reg::V1,
                        imm: table_low,
                    },
                ),
                (
                    start + 0x210,
                    K::Sll {
                        rd: Reg::V0,
                        rt: Reg::A3,
                        sa: 2,
                    },
                ),
                (
                    start + 0x214,
                    K::Addu {
                        rd: Reg::V0,
                        rs: Reg::V0,
                        rt: Reg::V1,
                    },
                ),
                (
                    start + 0x218,
                    K::Sw {
                        rt: Reg::FP,
                        base: Reg::V0,
                        offset: 0,
                    },
                ),
            ];
            for (address, expected) in checks {
                require(instructions, spec, address, expected)?;
            }
        }
        VddRegistrationFamily::Pcomi => {
            let checks = [
                (
                    start + 0x0C,
                    K::Addu {
                        rd: Reg::S6,
                        rs: Reg::A1,
                        rt: Reg::ZERO,
                    },
                ),
                (
                    start + 0x30,
                    K::Lw {
                        rt: Reg::V0,
                        base: Reg::S6,
                        offset: 4,
                    },
                ),
                (
                    start + 0x34,
                    K::Lhu {
                        rt: Reg::S7,
                        base: Reg::S6,
                        offset: 2,
                    },
                ),
                (
                    start + 0x94,
                    K::Addiu {
                        rt: Reg::A0,
                        rs: Reg::S6,
                        imm: VDD_DESCRIPTOR_STRIDE as i16,
                    },
                ),
                (
                    start + 0x98,
                    K::Lhu {
                        rt: Reg::V0,
                        base: Reg::A0,
                        offset: 2,
                    },
                ),
                (
                    start + 0x244,
                    K::Lui {
                        rt: Reg::V1,
                        imm: table_high,
                    },
                ),
                (
                    start + 0x248,
                    K::Addiu {
                        rt: Reg::V0,
                        rs: Reg::V1,
                        imm: table_low,
                    },
                ),
                (
                    start + 0x24C,
                    K::Addu {
                        rd: Reg::V0,
                        rs: Reg::S5,
                        rt: Reg::V0,
                    },
                ),
                (
                    start + 0x250,
                    K::Sw {
                        rt: Reg::S6,
                        base: Reg::V0,
                        offset: 0,
                    },
                ),
            ];
            for (address, expected) in checks {
                require(instructions, spec, address, expected)?;
            }
        }
    }
    Ok(())
}

/// 한 EXE의 VDD 등록 record·handler·loader와 archive descriptor를 검증한다.
pub fn verify_vdd_registration_source(
    map: &ExeMap,
    text: &[u8],
    spec: VddRegistrationSourceSpec,
) -> Result<(), VddRegistrationError> {
    if map.name != spec.exe {
        return Err(error(format!(
            "VDD 등록 EXE 불일치: expected={}, actual={}",
            spec.exe, map.name
        )));
    }
    if text.len() != map.text_len as usize {
        return Err(error(format!(
            "{} text 길이 불일치: expected={}, actual={}",
            spec.exe,
            map.text_len,
            text.len()
        )));
    }
    verify_registration_record(spec, map, text)?;
    verify_archive_descriptor(spec, map, text)?;
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error(format!("{} text 길이가 4바이트 정렬이 아님", spec.exe)))?;
    verify_handler(&instructions, spec)?;
    verify_loader(&instructions, spec)
}

#[cfg(test)]
#[path = "vdd_registration_tests.rs"]
mod tests;
