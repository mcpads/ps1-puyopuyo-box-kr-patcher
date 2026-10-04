//! E6 glyph 분석에서 공유하는 실행 주소와 typed 명령 검증.

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K};

use super::{error, SubgameSeqE6GlyphError};

pub(super) fn runtime_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    runtime: u32,
    len: usize,
) -> Result<&'a [u8], SubgameSeqE6GlyphError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "{} E6 glyph runtime이 EXE 밖임: 0x{runtime:08X}",
            map.name
        ))
    })?;
    text.get(offset..offset + len).ok_or_else(|| {
        error(format!(
            "{} E6 glyph runtime 범위 이탈: 0x{runtime:08X}",
            map.name
        ))
    })
}

pub(super) fn read_u16(raw: &[u8], offset: usize) -> Result<u16, SubgameSeqE6GlyphError> {
    raw.get(offset..offset + 2)
        .map(|bytes| u16::from_le_bytes(bytes.try_into().expect("2바이트 halfword")))
        .ok_or_else(|| error(format!("E6 glyph halfword 범위 이탈: +0x{offset:X}")))
}

pub(super) fn read_i16(raw: &[u8], offset: usize) -> Result<i16, SubgameSeqE6GlyphError> {
    read_u16(raw, offset).map(|value| value as i16)
}

pub(super) fn read_u32(raw: &[u8], offset: usize) -> Result<u32, SubgameSeqE6GlyphError> {
    raw.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
        .ok_or_else(|| error(format!("E6 glyph word 범위 이탈: +0x{offset:X}")))
}

pub(super) fn require(
    instructions: &[Instruction],
    map: &ExeMap,
    runtime: u32,
    expected: K,
) -> Result<(), SubgameSeqE6GlyphError> {
    let offset = map
        .text_offset(runtime)
        .ok_or_else(|| error(format!("{} E6 glyph 명령이 EXE 밖임", map.name)))?;
    let actual = instructions
        .get(offset / 4)
        .filter(|instruction| instruction.addr == runtime)
        .ok_or_else(|| error(format!("{} E6 glyph 명령 누락: 0x{runtime:08X}", map.name)))?;
    if actual.kind != expected {
        return Err(error(format!(
            "{} E6 glyph 명령 불일치: 0x{runtime:08X}, expected={expected:?}, actual={:?}",
            map.name, actual.kind
        )));
    }
    Ok(())
}

pub(super) fn high_low(address: u32) -> (u16, i16) {
    (
        ((address.wrapping_add(0x8000)) >> 16) as u16,
        address as u16 as i16,
    )
}
