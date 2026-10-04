//! Mednafen PSX native save state의 `MAIN/MainRAM.data8` 읽기.
//!
//! Mednafen `state.cpp`의 일반 save-state container와 PSX `StateAction`이
//! 기록하는 이름·크기를 그대로 검증한다. state를 수정하거나 emulator에
//! 다시 적재하는 기능은 제공하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

const HEADER_LEN: usize = 32;
const SECTION_HEADER_LEN: usize = 36;
const MAGIC: &[u8; 8] = b"MDFNSVST";
const MINIMUM_STATE_VERSION: u32 = 0x900;
const MAIN_SECTION: &str = "MAIN";
const MAIN_RAM_VARIABLE: &str = "MainRAM.data8";
const PSX_MAIN_RAM_LEN: usize = 2 * 1024 * 1024;

/// 검증된 Mednafen PSX state의 읽기 전용 view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MednafenPsxState<'a> {
    state_version: u32,
    section_count: usize,
    main_ram: &'a [u8],
}

impl<'a> MednafenPsxState<'a> {
    pub fn parse(raw: &'a [u8]) -> Result<Self, MednafenStateError> {
        let header = raw
            .get(..HEADER_LEN)
            .ok_or(MednafenStateError::TruncatedHeader {
                available: raw.len(),
            })?;
        if &header[..MAGIC.len()] != MAGIC {
            return Err(MednafenStateError::WrongMagic);
        }

        let state_version = read_u32(header, 16).expect("32바이트 header 범위를 확인함");
        if state_version < MINIMUM_STATE_VERSION {
            return Err(MednafenStateError::UnsupportedVersion(state_version));
        }
        let encoded_total = read_u32(header, 20).expect("32바이트 header 범위를 확인함");
        if encoded_total & 0x8000_0000 != 0 {
            return Err(MednafenStateError::BigEndianState);
        }
        let total_len =
            usize::try_from(encoded_total).map_err(|_| MednafenStateError::CoordinateOverflow)?;
        if total_len != raw.len() {
            return Err(MednafenStateError::LengthMismatch {
                declared: total_len,
                actual: raw.len(),
            });
        }

        let width = usize::try_from(read_u32(header, 24).expect("header width 범위"))
            .map_err(|_| MednafenStateError::CoordinateOverflow)?;
        let height = usize::try_from(read_u32(header, 28).expect("header height 범위"))
            .map_err(|_| MednafenStateError::CoordinateOverflow)?;
        let preview_len = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(3))
            .ok_or(MednafenStateError::CoordinateOverflow)?;
        let mut cursor = HEADER_LEN
            .checked_add(preview_len)
            .ok_or(MednafenStateError::CoordinateOverflow)?;
        if cursor > total_len {
            return Err(MednafenStateError::PreviewOutOfRange {
                width,
                height,
                total_len,
            });
        }

        let mut section_names = BTreeSet::new();
        let mut main_section = None;
        while cursor < total_len {
            let header_end = cursor
                .checked_add(SECTION_HEADER_LEN)
                .ok_or(MednafenStateError::CoordinateOverflow)?;
            let section_header =
                raw.get(cursor..header_end)
                    .ok_or(MednafenStateError::TruncatedSectionHeader {
                        offset: cursor,
                        available: total_len.saturating_sub(cursor),
                    })?;
            let name = parse_fixed_name(&section_header[..32], cursor)?;
            if !section_names.insert(name) {
                return Err(MednafenStateError::DuplicateSection(name.to_string()));
            }
            let size = usize::try_from(
                read_u32(section_header, 32).expect("section header size 범위를 확인함"),
            )
            .map_err(|_| MednafenStateError::CoordinateOverflow)?;
            let data_start = header_end;
            let data_end = data_start
                .checked_add(size)
                .ok_or(MednafenStateError::CoordinateOverflow)?;
            let data =
                raw.get(data_start..data_end)
                    .ok_or(MednafenStateError::SectionOutOfRange {
                        name: name.to_string(),
                        declared: size,
                        available: total_len.saturating_sub(data_start),
                    })?;
            if name == MAIN_SECTION {
                main_section = Some(data);
            }
            cursor = data_end;
        }

        let main_section = main_section.ok_or(MednafenStateError::MissingMainSection)?;
        let main_ram = parse_main_ram(main_section)?;
        Ok(Self {
            state_version,
            section_count: section_names.len(),
            main_ram,
        })
    }

    pub const fn state_version(&self) -> u32 {
        self.state_version
    }

    pub const fn section_count(&self) -> usize {
        self.section_count
    }

    pub const fn main_ram_bytes(&self) -> usize {
        self.main_ram.len()
    }

    /// PSX Main RAM의 KUSEG/KSEG0/KSEG1 주소에서 u8을 읽는다.
    pub fn read_u8(&self, runtime_address: u32) -> Result<u8, MednafenStateError> {
        let offset = psx_main_ram_offset(runtime_address)
            .ok_or(MednafenStateError::AddressOutOfRange { runtime_address })?;
        self.main_ram
            .get(offset)
            .copied()
            .ok_or(MednafenStateError::AddressOutOfRange { runtime_address })
    }

    /// PSX Main RAM의 KUSEG/KSEG0/KSEG1 주소에서 연속 byte를 읽는다.
    pub fn read_bytes(
        &self,
        runtime_address: u32,
        bytes: usize,
    ) -> Result<&'a [u8], MednafenStateError> {
        let offset = psx_main_ram_offset(runtime_address)
            .ok_or(MednafenStateError::AddressOutOfRange { runtime_address })?;
        let end = offset
            .checked_add(bytes)
            .ok_or(MednafenStateError::AddressOutOfRange { runtime_address })?;
        self.main_ram
            .get(offset..end)
            .ok_or(MednafenStateError::AddressOutOfRange { runtime_address })
    }

    /// PSX Main RAM의 KUSEG/KSEG0/KSEG1 주소에서 little-endian u16을 읽는다.
    pub fn read_u16_le(&self, runtime_address: u32) -> Result<u16, MednafenStateError> {
        let offset = psx_main_ram_offset(runtime_address)
            .ok_or(MednafenStateError::AddressOutOfRange { runtime_address })?;
        let raw = self
            .main_ram
            .get(offset..offset + 2)
            .ok_or(MednafenStateError::AddressOutOfRange { runtime_address })?;
        Ok(u16::from_le_bytes(
            raw.try_into().expect("u16 Main RAM 범위를 확인함"),
        ))
    }

    /// PSX Main RAM의 KUSEG/KSEG0/KSEG1 주소에서 little-endian u32를 읽는다.
    pub fn read_u32_le(&self, runtime_address: u32) -> Result<u32, MednafenStateError> {
        let offset = psx_main_ram_offset(runtime_address)
            .ok_or(MednafenStateError::AddressOutOfRange { runtime_address })?;
        let raw = self
            .main_ram
            .get(offset..offset + 4)
            .ok_or(MednafenStateError::AddressOutOfRange { runtime_address })?;
        Ok(u32::from_le_bytes(
            raw.try_into().expect("u32 Main RAM 범위를 확인함"),
        ))
    }

    /// Main RAM의 4바이트 정렬 word 중 정확히 같은 값을 가진 KSEG0 주소.
    ///
    /// 정적 table과 객체 field를 구분하지는 않는다. 호출자가 주변 구조와
    /// 값 범위를 함께 검사해 후보를 좁혀야 한다.
    pub fn find_aligned_u32_le(&self, value: u32) -> Vec<u32> {
        self.main_ram
            .chunks_exact(4)
            .enumerate()
            .filter_map(|(index, raw)| {
                let actual =
                    u32::from_le_bytes(raw.try_into().expect("4바이트 chunk 범위를 확인함"));
                if actual != value {
                    return None;
                }
                let byte_offset = u32::try_from(index.checked_mul(4)?).ok()?;
                0x8000_0000u32.checked_add(byte_offset)
            })
            .collect()
    }
}

fn read_u32(raw: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        raw.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn parse_fixed_name(raw: &[u8], offset: usize) -> Result<&str, MednafenStateError> {
    let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
    if raw[end..].iter().any(|byte| *byte != 0) {
        return Err(MednafenStateError::NonZeroNamePadding { offset });
    }
    let name =
        std::str::from_utf8(&raw[..end]).map_err(|_| MednafenStateError::InvalidName { offset })?;
    if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(MednafenStateError::InvalidName { offset });
    }
    Ok(name)
}

fn parse_main_ram(main: &[u8]) -> Result<&[u8], MednafenStateError> {
    let mut cursor = 0usize;
    let mut names = BTreeSet::new();
    let mut main_ram = None;
    while cursor < main.len() {
        let name_len = usize::from(*main.get(cursor).ok_or(
            MednafenStateError::TruncatedMainVariable {
                offset: cursor,
                available: 0,
            },
        )?);
        cursor += 1;
        let name_end = cursor
            .checked_add(name_len)
            .ok_or(MednafenStateError::CoordinateOverflow)?;
        let name_raw =
            main.get(cursor..name_end)
                .ok_or(MednafenStateError::TruncatedMainVariable {
                    offset: cursor - 1,
                    available: main.len().saturating_sub(cursor - 1),
                })?;
        let name = std::str::from_utf8(name_raw)
            .map_err(|_| MednafenStateError::InvalidMainVariableName { offset: cursor })?;
        if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(MednafenStateError::InvalidMainVariableName { offset: cursor });
        }
        if !names.insert(name) {
            return Err(MednafenStateError::DuplicateMainVariable(name.to_string()));
        }
        cursor = name_end;
        let size = usize::try_from(read_u32(main, cursor).ok_or(
            MednafenStateError::TruncatedMainVariable {
                offset: cursor,
                available: main.len().saturating_sub(cursor),
            },
        )?)
        .map_err(|_| MednafenStateError::CoordinateOverflow)?;
        cursor += 4;
        let value_end = cursor
            .checked_add(size)
            .ok_or(MednafenStateError::CoordinateOverflow)?;
        let value =
            main.get(cursor..value_end)
                .ok_or(MednafenStateError::MainVariableOutOfRange {
                    name: name.to_string(),
                    declared: size,
                    available: main.len().saturating_sub(cursor),
                })?;
        if name == MAIN_RAM_VARIABLE {
            main_ram = Some(value);
        }
        cursor = value_end;
    }

    let main_ram = main_ram.ok_or(MednafenStateError::MissingMainRam)?;
    if main_ram.len() != PSX_MAIN_RAM_LEN {
        return Err(MednafenStateError::WrongMainRamSize {
            expected: PSX_MAIN_RAM_LEN,
            actual: main_ram.len(),
        });
    }
    Ok(main_ram)
}

fn psx_main_ram_offset(runtime_address: u32) -> Option<usize> {
    let physical = match runtime_address {
        0x0000_0000..=0x001F_FFFF => runtime_address,
        0x8000_0000..=0x801F_FFFF | 0xA000_0000..=0xA01F_FFFF => runtime_address & 0x001F_FFFF,
        _ => return None,
    };
    usize::try_from(physical).ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MednafenStateError {
    TruncatedHeader {
        available: usize,
    },
    WrongMagic,
    UnsupportedVersion(u32),
    BigEndianState,
    LengthMismatch {
        declared: usize,
        actual: usize,
    },
    CoordinateOverflow,
    PreviewOutOfRange {
        width: usize,
        height: usize,
        total_len: usize,
    },
    TruncatedSectionHeader {
        offset: usize,
        available: usize,
    },
    InvalidName {
        offset: usize,
    },
    NonZeroNamePadding {
        offset: usize,
    },
    DuplicateSection(String),
    SectionOutOfRange {
        name: String,
        declared: usize,
        available: usize,
    },
    MissingMainSection,
    TruncatedMainVariable {
        offset: usize,
        available: usize,
    },
    InvalidMainVariableName {
        offset: usize,
    },
    DuplicateMainVariable(String),
    MainVariableOutOfRange {
        name: String,
        declared: usize,
        available: usize,
    },
    MissingMainRam,
    WrongMainRamSize {
        expected: usize,
        actual: usize,
    },
    AddressOutOfRange {
        runtime_address: u32,
    },
}

impl fmt::Display for MednafenStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedHeader { available } => {
                write!(f, "Mednafen state header 부족: available={available}")
            }
            Self::WrongMagic => f.write_str("Mednafen state magic 불일치"),
            Self::UnsupportedVersion(version) => {
                write!(f, "Mednafen state version 미지원: 0x{version:X}")
            }
            Self::BigEndianState => f.write_str("big-endian Mednafen state는 지원하지 않음"),
            Self::LengthMismatch { declared, actual } => write!(
                f,
                "Mednafen state 전체 길이 불일치: declared={declared}, actual={actual}"
            ),
            Self::CoordinateOverflow => f.write_str("Mednafen state 좌표 overflow"),
            Self::PreviewOutOfRange {
                width,
                height,
                total_len,
            } => write!(
                f,
                "Mednafen state preview 범위 이탈: {width}x{height}, total={total_len}"
            ),
            Self::TruncatedSectionHeader { offset, available } => write!(
                f,
                "Mednafen section header 부족: offset=0x{offset:X}, available={available}"
            ),
            Self::InvalidName { offset } => {
                write!(f, "Mednafen section 이름 오류: offset=0x{offset:X}")
            }
            Self::NonZeroNamePadding { offset } => write!(
                f,
                "Mednafen section 이름 padding이 0이 아님: offset=0x{offset:X}"
            ),
            Self::DuplicateSection(name) => write!(f, "Mednafen section 중복: {name}"),
            Self::SectionOutOfRange {
                name,
                declared,
                available,
            } => write!(
                f,
                "Mednafen section 범위 이탈: {name}, declared={declared}, available={available}"
            ),
            Self::MissingMainSection => f.write_str("Mednafen MAIN section이 없음"),
            Self::TruncatedMainVariable { offset, available } => write!(
                f,
                "Mednafen MAIN variable 부족: offset=0x{offset:X}, available={available}"
            ),
            Self::InvalidMainVariableName { offset } => {
                write!(f, "Mednafen MAIN variable 이름 오류: offset=0x{offset:X}")
            }
            Self::DuplicateMainVariable(name) => {
                write!(f, "Mednafen MAIN variable 중복: {name}")
            }
            Self::MainVariableOutOfRange {
                name,
                declared,
                available,
            } => write!(
                f,
                "Mednafen MAIN variable 범위 이탈: {name}, declared={declared}, \
                 available={available}"
            ),
            Self::MissingMainRam => f.write_str("Mednafen MAIN/MainRAM.data8이 없음"),
            Self::WrongMainRamSize { expected, actual } => write!(
                f,
                "Mednafen PSX Main RAM 크기 불일치: expected={expected}, actual={actual}"
            ),
            Self::AddressOutOfRange { runtime_address } => {
                write!(f, "PSX Main RAM 주소 범위 이탈: 0x{runtime_address:08X}")
            }
        }
    }
}

impl Error for MednafenStateError {}

#[cfg(test)]
#[path = "mednafen_state_tests.rs"]
mod tests;
