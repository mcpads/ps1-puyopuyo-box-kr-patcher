//! BTX palette index를 browser 검토용 무손실 명도 BMP로 직렬화한다.
//!
//! 실제 CLUT 색을 재현하지 않으며 palette index와 모든 픽셀만 보존한다.
//! 호출자가 준 layout도 consumer 확정값이 아니라 검토 기준선일 수 있다.

use std::error::Error;
use std::fmt;

use crate::btx::{decode, BtxError, Layout, PixelFormat};

const BMP_HEADER_BYTES: usize = 14 + 40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BtxFamilyPreviewError {
    Btx(BtxError),
    SizeOverflow,
    DimensionOutOfRange { width: usize, height: usize },
}

impl fmt::Display for BtxFamilyPreviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Btx(error) => write!(formatter, "BTX preview decode 실패: {error}"),
            Self::SizeOverflow => write!(formatter, "BTX preview BMP 크기 overflow"),
            Self::DimensionOutOfRange { width, height } => write!(
                formatter,
                "BTX preview BMP 차원이 i32 범위를 넘음: {width}x{height}"
            ),
        }
    }
}

impl Error for BtxFamilyPreviewError {}

impl From<BtxError> for BtxFamilyPreviewError {
    fn from(error: BtxError) -> Self {
        Self::Btx(error)
    }
}

fn write_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn write_i32(output: &mut Vec<u8>, value: i32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn intensity(format: PixelFormat, index: u8) -> u8 {
    match format {
        PixelFormat::Indexed4 => index * 17,
        PixelFormat::Indexed8 => index,
    }
}

/// BTX palette index를 24-bit grayscale BMP로 만든다.
pub fn render_btx_family_preview_bmp(
    raw: &[u8],
    layout: Layout,
) -> Result<Vec<u8>, BtxFamilyPreviewError> {
    let width =
        i32::try_from(layout.width).map_err(|_| BtxFamilyPreviewError::DimensionOutOfRange {
            width: layout.width,
            height: layout.height,
        })?;
    let height =
        i32::try_from(layout.height).map_err(|_| BtxFamilyPreviewError::DimensionOutOfRange {
            width: layout.width,
            height: layout.height,
        })?;
    let pixels = decode(raw, layout)?;
    let unpadded_row = layout
        .width
        .checked_mul(3)
        .ok_or(BtxFamilyPreviewError::SizeOverflow)?;
    let row_bytes = unpadded_row
        .checked_add(3)
        .map(|bytes| bytes & !3)
        .ok_or(BtxFamilyPreviewError::SizeOverflow)?;
    let pixel_bytes = row_bytes
        .checked_mul(layout.height)
        .ok_or(BtxFamilyPreviewError::SizeOverflow)?;
    let file_bytes = BMP_HEADER_BYTES
        .checked_add(pixel_bytes)
        .ok_or(BtxFamilyPreviewError::SizeOverflow)?;
    let file_bytes = u32::try_from(file_bytes).map_err(|_| BtxFamilyPreviewError::SizeOverflow)?;
    let pixel_bytes_u32 =
        u32::try_from(pixel_bytes).map_err(|_| BtxFamilyPreviewError::SizeOverflow)?;

    let mut output = Vec::with_capacity(file_bytes as usize);
    output.extend_from_slice(b"BM");
    write_u32(&mut output, file_bytes);
    write_u16(&mut output, 0);
    write_u16(&mut output, 0);
    write_u32(&mut output, BMP_HEADER_BYTES as u32);
    write_u32(&mut output, 40);
    write_i32(&mut output, width);
    write_i32(&mut output, height);
    write_u16(&mut output, 1);
    write_u16(&mut output, 24);
    write_u32(&mut output, 0);
    write_u32(&mut output, pixel_bytes_u32);
    write_i32(&mut output, 2_835);
    write_i32(&mut output, 2_835);
    write_u32(&mut output, 0);
    write_u32(&mut output, 0);

    let padding = vec![0u8; row_bytes - unpadded_row];
    for row in pixels.chunks_exact(layout.width).rev() {
        for &index in row {
            let gray = intensity(layout.format, index);
            output.extend_from_slice(&[gray, gray, gray]);
        }
        output.extend_from_slice(&padding);
    }
    debug_assert_eq!(output.len(), file_bytes as usize);
    Ok(output)
}

#[cfg(test)]
#[path = "btx_family_preview_tests.rs"]
mod tests;
