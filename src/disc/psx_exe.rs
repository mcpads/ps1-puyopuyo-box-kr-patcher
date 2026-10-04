//! PS-X EXE 헤더와 적재 사상 검증.

use std::error::Error;
use std::fmt;

use crate::coords::{ExeMap, PSX_EXE_HEADER_LEN};

const MAGIC: &[u8; 8] = b"PS-X EXE";
const HEADER_LEN: usize = PSX_EXE_HEADER_LEN as usize;

/// PS-X EXE 파싱·사상 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsxExeError {
    Truncated(usize),
    InvalidMagic,
    EmptyText,
    FileLengthMismatch {
        expected: usize,
        actual: usize,
    },
    StackOverflow,
    MapMismatch {
        exe: &'static str,
        field: &'static str,
        expected: u64,
        actual: u64,
    },
}

impl fmt::Display for PsxExeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated(actual) => {
                write!(
                    f,
                    "PS-X EXE 헤더가 잘림: expected>={HEADER_LEN}, actual={actual}"
                )
            }
            Self::InvalidMagic => write!(f, "PS-X EXE magic 불일치"),
            Self::EmptyText => write!(f, "PS-X EXE text 크기가 0"),
            Self::FileLengthMismatch { expected, actual } => write!(
                f,
                "PS-X EXE 파일 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::StackOverflow => write!(f, "PS-X EXE 초기 stack 계산 overflow"),
            Self::MapMismatch {
                exe,
                field,
                expected,
                actual,
            } => write!(
                f,
                "{exe} PS-X EXE {field} 불일치: expected=0x{expected:X}, actual=0x{actual:X}"
            ),
        }
    }
}

impl Error for PsxExeError {}

/// 한 PS-X EXE와 그 text 본문.
#[derive(Debug)]
pub struct PsxExe<'a> {
    raw: &'a [u8],
    entry_pc: u32,
    initial_gp: u32,
    load_addr: u32,
    text_len: u32,
    data_addr: u32,
    data_len: u32,
    bss_addr: u32,
    bss_len: u32,
    stack_base: u32,
    stack_len: u32,
}

impl<'a> PsxExe<'a> {
    /// 헤더와 선언 text 길이를 검증한다.
    pub fn parse(raw: &'a [u8]) -> Result<Self, PsxExeError> {
        if raw.len() < HEADER_LEN {
            return Err(PsxExeError::Truncated(raw.len()));
        }
        if &raw[..MAGIC.len()] != MAGIC {
            return Err(PsxExeError::InvalidMagic);
        }
        let text_len = read_u32(raw, 0x1c);
        if text_len == 0 {
            return Err(PsxExeError::EmptyText);
        }
        let expected =
            HEADER_LEN
                .checked_add(text_len as usize)
                .ok_or(PsxExeError::FileLengthMismatch {
                    expected: usize::MAX,
                    actual: raw.len(),
                })?;
        if raw.len() != expected {
            return Err(PsxExeError::FileLengthMismatch {
                expected,
                actual: raw.len(),
            });
        }
        Ok(Self {
            raw,
            entry_pc: read_u32(raw, 0x10),
            initial_gp: read_u32(raw, 0x14),
            load_addr: read_u32(raw, 0x18),
            text_len,
            data_addr: read_u32(raw, 0x20),
            data_len: read_u32(raw, 0x24),
            bss_addr: read_u32(raw, 0x28),
            bss_len: read_u32(raw, 0x2c),
            stack_base: read_u32(raw, 0x30),
            stack_len: read_u32(raw, 0x34),
        })
    }

    pub fn entry_pc(&self) -> u32 {
        self.entry_pc
    }

    pub fn initial_gp(&self) -> u32 {
        self.initial_gp
    }

    pub fn load_addr(&self) -> u32 {
        self.load_addr
    }

    pub fn text_len(&self) -> u32 {
        self.text_len
    }

    pub fn data_addr(&self) -> u32 {
        self.data_addr
    }

    pub fn data_len(&self) -> u32 {
        self.data_len
    }

    pub fn bss_addr(&self) -> u32 {
        self.bss_addr
    }

    pub fn bss_len(&self) -> u32 {
        self.bss_len
    }

    pub fn stack_base(&self) -> u32 {
        self.stack_base
    }

    pub fn stack_len(&self) -> u32 {
        self.stack_len
    }

    /// header의 stack base + size.
    pub fn initial_sp(&self) -> Result<u32, PsxExeError> {
        self.stack_base
            .checked_add(self.stack_len)
            .ok_or(PsxExeError::StackOverflow)
    }

    /// `0x800`바이트 헤더 뒤의 선언 text.
    pub fn text(&self) -> &'a [u8] {
        &self.raw[HEADER_LEN..]
    }

    /// 이 EXE에만 선언한 좌표 사상과 대조한다.
    pub fn verify_map(&self, map: &'static ExeMap) -> Result<(), PsxExeError> {
        require_field(
            map,
            "파일 길이",
            u64::from(map.file.len),
            self.raw.len() as u64,
        )?;
        require_field(
            map,
            "entry PC",
            u64::from(map.entry_pc),
            u64::from(self.entry_pc),
        )?;
        require_field(
            map,
            "load 주소",
            u64::from(map.load_addr),
            u64::from(self.load_addr),
        )?;
        require_field(
            map,
            "text 길이",
            u64::from(map.text_len),
            u64::from(self.text_len),
        )
    }
}

fn read_u32(raw: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap())
}

fn require_field(
    map: &'static ExeMap,
    field: &'static str,
    expected: u64,
    actual: u64,
) -> Result<(), PsxExeError> {
    if expected != actual {
        return Err(PsxExeError::MapMismatch {
            exe: map.name,
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "psx_exe_tests.rs"]
mod tests;
