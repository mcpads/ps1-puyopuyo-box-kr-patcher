//! BTX 무헤더 indexed bitmap의 엄격한 디코드·인코드.
//!
//! BOX 원본에서는 CNX subtype `bi_`가 4bpp, `bit`가 8bpp BTX를 담는다.
//! BTX 자체에는 폭·높이가 없으므로 호출자가 소비자에서 확정한 layout을
//! 제공해야 한다. 크기가 맞지 않으면 일부 픽셀만 반환하지 않고 실패한다.

use std::error::Error;
use std::fmt;

/// BTX 픽셀 인코딩.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// 바이트당 두 픽셀. 하위 nibble이 왼쪽 픽셀이다.
    Indexed4,
    /// 바이트당 한 픽셀.
    Indexed8,
}

impl PixelFormat {
    /// BOX CNX subtype에서 픽셀 인코딩을 판정한다.
    pub fn from_cnx_subtype(subtype: [u8; 3]) -> Result<Self, BtxError> {
        match &subtype {
            b"bi_" => Ok(Self::Indexed4),
            b"bit" => Ok(Self::Indexed8),
            _ => Err(BtxError::UnsupportedCnxSubtype(subtype)),
        }
    }

    /// 픽셀당 bit 수.
    pub const fn bits_per_pixel(self) -> u8 {
        match self {
            Self::Indexed4 => 4,
            Self::Indexed8 => 8,
        }
    }
}

/// 소비자에서 확정한 BTX layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub width: usize,
    pub height: usize,
    pub format: PixelFormat,
}

impl Layout {
    /// layout이 요구하는 원시 바이트 수를 계산한다.
    pub fn byte_len(self) -> Result<usize, BtxError> {
        if self.width == 0 || self.height == 0 {
            return Err(BtxError::EmptyLayout {
                width: self.width,
                height: self.height,
            });
        }
        if self.format == PixelFormat::Indexed4 && !self.width.is_multiple_of(2) {
            return Err(BtxError::OddIndexed4Width(self.width));
        }
        let pixels = self
            .width
            .checked_mul(self.height)
            .ok_or(BtxError::LayoutOverflow {
                width: self.width,
                height: self.height,
            })?;
        Ok(match self.format {
            PixelFormat::Indexed4 => pixels / 2,
            PixelFormat::Indexed8 => pixels,
        })
    }
}

/// BTX 경계 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BtxError {
    UnsupportedCnxSubtype([u8; 3]),
    EmptyLayout { width: usize, height: usize },
    OddIndexed4Width(usize),
    LayoutOverflow { width: usize, height: usize },
    DataLengthMismatch { expected: usize, actual: usize },
    PixelLengthMismatch { expected: usize, actual: usize },
    PaletteIndexOutOfRange { offset: usize, value: u8 },
}

impl fmt::Display for BtxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedCnxSubtype(subtype) => write!(
                f,
                "BTX로 지원하지 않는 CNX subtype: {:02X} {:02X} {:02X}",
                subtype[0], subtype[1], subtype[2]
            ),
            Self::EmptyLayout { width, height } => {
                write!(f, "BTX layout 크기가 0: {width}x{height}")
            }
            Self::OddIndexed4Width(width) => {
                write!(f, "4bpp BTX 폭은 짝수여야 함: {width}")
            }
            Self::LayoutOverflow { width, height } => {
                write!(f, "BTX layout 크기 overflow: {width}x{height}")
            }
            Self::DataLengthMismatch { expected, actual } => {
                write!(
                    f,
                    "BTX 원시 크기 불일치: expected={expected}, actual={actual}"
                )
            }
            Self::PixelLengthMismatch { expected, actual } => {
                write!(
                    f,
                    "BTX 픽셀 수 불일치: expected={expected}, actual={actual}"
                )
            }
            Self::PaletteIndexOutOfRange { offset, value } => write!(
                f,
                "4bpp BTX 픽셀 인덱스 범위 초과: offset={offset}, value={value}"
            ),
        }
    }
}

impl Error for BtxError {}

/// BTX를 행 우선 팔레트 인덱스로 디코드한다.
pub fn decode(data: &[u8], layout: Layout) -> Result<Vec<u8>, BtxError> {
    let expected = layout.byte_len()?;
    if data.len() != expected {
        return Err(BtxError::DataLengthMismatch {
            expected,
            actual: data.len(),
        });
    }

    match layout.format {
        PixelFormat::Indexed4 => {
            let mut pixels = Vec::with_capacity(expected * 2);
            for &byte in data {
                pixels.push(byte & 0x0F);
                pixels.push(byte >> 4);
            }
            Ok(pixels)
        }
        PixelFormat::Indexed8 => Ok(data.to_vec()),
    }
}

/// 행 우선 팔레트 인덱스를 BTX로 인코드한다.
pub fn encode(pixels: &[u8], layout: Layout) -> Result<Vec<u8>, BtxError> {
    let expected_bytes = layout.byte_len()?;
    let expected_pixels =
        layout
            .width
            .checked_mul(layout.height)
            .ok_or(BtxError::LayoutOverflow {
                width: layout.width,
                height: layout.height,
            })?;
    if pixels.len() != expected_pixels {
        return Err(BtxError::PixelLengthMismatch {
            expected: expected_pixels,
            actual: pixels.len(),
        });
    }

    match layout.format {
        PixelFormat::Indexed4 => {
            let mut data = Vec::with_capacity(expected_bytes);
            for (pair_offset, pair) in pixels.chunks_exact(2).enumerate() {
                for (within_pair, &value) in pair.iter().enumerate() {
                    if value > 0x0F {
                        return Err(BtxError::PaletteIndexOutOfRange {
                            offset: pair_offset * 2 + within_pair,
                            value,
                        });
                    }
                }
                data.push(pair[0] | (pair[1] << 4));
            }
            Ok(data)
        }
        PixelFormat::Indexed8 => Ok(pixels.to_vec()),
    }
}

#[cfg(test)]
#[path = "btx_tests.rs"]
mod tests;
