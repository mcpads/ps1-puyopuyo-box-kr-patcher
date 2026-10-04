//! 루트 `32M.BIN`의 저장 공간 경계.
//!
//! 이 파일은 이미 ISO 9660에 할당된 단일 Form1 extent다. 따라서 같은
//! 크기 안의 payload는 directory record 성장 없이 저장할 수 있다.
//! 현재 EXE가 이를 소비한다는 뜻은 아니며, 직접 filename literal 부재와
//! loader 미확정은 별도 경계로 남긴다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::source::FORM1_DATA_LEN;

use super::{DiscImage, Extent, ImageError, Iso9660, IsoError};

pub const BOX_32M_PATH: &str = "/32M.BIN;1";
const BOX_32M_LBA: u32 = 43_778;
const BOX_32M_LEN: u64 = 33_554_432;
const BOX_32M_SECTORS: u32 = 16_384;
const BOX_32M_END_LBA: u32 = BOX_32M_LBA + BOX_32M_SECTORS;
const BOX_VOLUME_TAIL_SECTORS: u32 = 150;
const BOX_32M_SHA256: &str = "83ee47245398adee79bd9c0a8bc57b821e92aba10f5f9ade8a5d1fae4d8c4302";
const FILENAME_LITERAL: &[u8] = b"32M.BIN";

/// 지원 원본의 예약 파일 요약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Box32mSummary {
    pub lba: u32,
    pub bytes: u64,
    pub form1_sectors: u32,
    pub end_lba: u32,
    pub volume_tail_sectors: u32,
    pub nonzero_bytes: usize,
    pub executable_filename_literals: usize,
}

/// 예약 파일 검증 오류.
#[derive(Debug)]
pub enum Box32mError {
    Iso(IsoError),
    Image {
        lba: u32,
        source: ImageError,
    },
    ExtentCount(usize),
    ExtentMismatch {
        expected_lba: u32,
        actual_lba: u32,
        expected_len: u32,
        actual_len: u32,
    },
    Overlap {
        path: String,
        first_lba: u32,
        end_lba: u32,
    },
    DigestMismatch {
        expected: String,
        actual: String,
    },
    NonZeroData {
        offset: usize,
        value: u8,
    },
    FilenameLiteral {
        executable: String,
        occurrences: usize,
    },
    VolumeTailMismatch {
        expected: Extent,
        actual: Option<Extent>,
    },
}

impl fmt::Display for Box32mError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Iso(source) => write!(f, "{BOX_32M_PATH}: {source}"),
            Self::Image { lba, source } => write!(f, "{BOX_32M_PATH} LBA {lba}: {source}"),
            Self::ExtentCount(actual) => {
                write!(
                    f,
                    "{BOX_32M_PATH} extent 수 불일치: expected=1, actual={actual}"
                )
            }
            Self::ExtentMismatch {
                expected_lba,
                actual_lba,
                expected_len,
                actual_len,
            } => write!(
                f,
                "{BOX_32M_PATH} extent 불일치: expected=({expected_lba},{expected_len}), \
                 actual=({actual_lba},{actual_len})"
            ),
            Self::Overlap {
                path,
                first_lba,
                end_lba,
            } => write!(
                f,
                "{BOX_32M_PATH}와 다른 ISO extent가 겹침: {path} \
                 [{first_lba},{end_lba})"
            ),
            Self::DigestMismatch { expected, actual } => write!(
                f,
                "{BOX_32M_PATH} SHA-256 불일치: expected={expected}, actual={actual}"
            ),
            Self::NonZeroData { offset, value } => write!(
                f,
                "{BOX_32M_PATH} 0이 아닌 원본 데이터: offset=0x{offset:X}, value=0x{value:02X}"
            ),
            Self::FilenameLiteral {
                executable,
                occurrences,
            } => write!(
                f,
                "{executable}에 32M.BIN direct filename literal이 있음: {occurrences}개"
            ),
            Self::VolumeTailMismatch { expected, actual } => {
                write!(
                    f,
                    "{BOX_32M_PATH} 뒤 volume tail 불일치: expected={expected:?}, actual={actual:?}"
                )
            }
        }
    }
}

impl Error for Box32mError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Iso(source) => Some(source),
            Self::Image { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<IsoError> for Box32mError {
    fn from(value: IsoError) -> Self {
        Self::Iso(value)
    }
}

/// 지원 원본의 `32M.BIN` extent·Form1·전량 0·직접 literal 부재를 검증한다.
pub fn verify_box_32m_reservation<'a>(
    iso: &Iso9660<'_>,
    image: &DiscImage,
    executables: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Result<Box32mSummary, Box32mError> {
    let entry = iso.find(BOX_32M_PATH)?;
    let [extent] = entry.extents() else {
        return Err(Box32mError::ExtentCount(entry.extents().len()));
    };
    if extent.lba != BOX_32M_LBA || extent.len != BOX_32M_LEN as u32 {
        return Err(Box32mError::ExtentMismatch {
            expected_lba: BOX_32M_LBA,
            actual_lba: extent.lba,
            expected_len: BOX_32M_LEN as u32,
            actual_len: extent.len,
        });
    }

    for other in iso.entries() {
        if other.path() == BOX_32M_PATH {
            continue;
        }
        for other_extent in other.extents() {
            let sectors = other_extent.len.div_ceil(FORM1_DATA_LEN as u32);
            let other_end = other_extent.lba.saturating_add(sectors);
            if ranges_overlap(BOX_32M_LBA, BOX_32M_END_LBA, other_extent.lba, other_end) {
                return Err(Box32mError::Overlap {
                    path: other.path().to_string(),
                    first_lba: other_extent.lba,
                    end_lba: other_end,
                });
            }
        }
    }

    for lba in BOX_32M_LBA..BOX_32M_END_LBA {
        image
            .form1_data(lba)
            .map_err(|source| Box32mError::Image { lba, source })?;
    }

    let raw = iso.read_file(BOX_32M_PATH)?;
    let actual_digest = format!("{:x}", Sha256::digest(&raw));
    if actual_digest != BOX_32M_SHA256 {
        return Err(Box32mError::DigestMismatch {
            expected: BOX_32M_SHA256.to_string(),
            actual: actual_digest,
        });
    }
    if let Some((offset, &value)) = raw.iter().enumerate().find(|(_, value)| **value != 0) {
        return Err(Box32mError::NonZeroData { offset, value });
    }

    let mut filename_literals = 0usize;
    for (name, text) in executables {
        let occurrences = count_occurrences(text, FILENAME_LITERAL);
        if occurrences != 0 {
            return Err(Box32mError::FilenameLiteral {
                executable: name.to_string(),
                occurrences,
            });
        }
        filename_literals += occurrences;
    }

    let expected_tail = Extent {
        lba: BOX_32M_END_LBA,
        len: BOX_VOLUME_TAIL_SECTORS * FORM1_DATA_LEN as u32,
    };
    let actual_tail = iso.form1_volume_tail()?.map(|tail| tail.extent());
    if actual_tail != Some(expected_tail) {
        return Err(Box32mError::VolumeTailMismatch {
            expected: expected_tail,
            actual: actual_tail,
        });
    }
    Ok(Box32mSummary {
        lba: BOX_32M_LBA,
        bytes: raw.len() as u64,
        form1_sectors: BOX_32M_SECTORS,
        end_lba: BOX_32M_END_LBA,
        volume_tail_sectors: BOX_VOLUME_TAIL_SECTORS,
        nonzero_bytes: 0,
        executable_filename_literals: filename_literals,
    })
}

fn ranges_overlap(first_start: u32, first_end: u32, second_start: u32, second_end: u32) -> bool {
    first_start < second_end && second_start < first_end
}

fn count_occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || haystack.len() < needle.len() {
        return 0;
    }
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

#[cfg(test)]
#[path = "reserved_tests.rs"]
mod reserved_tests;
