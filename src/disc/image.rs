//! 불변 원본 BIN 식별과 raw 섹터 접근.

use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use sha1::{Digest, Sha1};

use crate::source::{BIN_LEN, BIN_SHA1, SECTOR_LEN, VOLUME_SECTORS};

use super::sector::{verified_data, verified_form1_data, SectorError, SectorKind};

/// 원본 raw 트랙의 form 분포.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectorCounts {
    pub form1: u32,
    pub form2: u32,
}

/// 디스크 이미지 오류.
#[derive(Debug)]
pub enum ImageError {
    Io(std::io::Error),
    WrongLength {
        expected: u64,
        actual: u64,
    },
    UnalignedLength(usize),
    WrongSectorCount {
        expected: u32,
        actual: u32,
    },
    Sha1Mismatch {
        expected: &'static str,
        actual: String,
    },
    LbaOutOfRange {
        lba: u32,
        sectors: u32,
    },
    Sector {
        lba: u32,
        source: SectorError,
    },
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "디스크 이미지 I/O 실패: {error}"),
            Self::WrongLength { expected, actual } => {
                write!(
                    f,
                    "원본 BIN 크기 불일치: expected={expected}, actual={actual}"
                )
            }
            Self::UnalignedLength(actual) => {
                write!(
                    f,
                    "BIN 크기가 {SECTOR_LEN}바이트 섹터 경계가 아님: {actual}"
                )
            }
            Self::WrongSectorCount { expected, actual } => write!(
                f,
                "원본 BIN 섹터 수 불일치: expected={expected}, actual={actual}"
            ),
            Self::Sha1Mismatch { expected, actual } => {
                write!(
                    f,
                    "원본 BIN SHA-1 불일치: expected={expected}, actual={actual}"
                )
            }
            Self::LbaOutOfRange { lba, sectors } => {
                write!(f, "LBA 범위 이탈: lba={lba}, sectors={sectors}")
            }
            Self::Sector { lba, source } => write!(f, "LBA {lba} 검증 실패: {source}"),
        }
    }
}

impl Error for ImageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Sector { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ImageError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// raw Mode2/2352 이미지.
///
/// 지원 원본에서 만드는 공개 경로는 [`VerifiedSourceImage::open`]뿐이다.
/// 작은 합성 이미지는 crate 내부 테스트에서만 만들 수 있다.
#[derive(Debug)]
pub struct DiscImage {
    bytes: Vec<u8>,
    sectors: u32,
}

impl DiscImage {
    pub(crate) fn from_bytes(bytes: Vec<u8>) -> Result<Self, ImageError> {
        if !bytes.len().is_multiple_of(SECTOR_LEN) {
            return Err(ImageError::UnalignedLength(bytes.len()));
        }
        let sectors = u32::try_from(bytes.len() / SECTOR_LEN)
            .map_err(|_| ImageError::UnalignedLength(bytes.len()))?;
        Ok(Self { bytes, sectors })
    }

    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// 이미지의 raw 섹터 수.
    pub fn sector_count(&self) -> u32 {
        self.sectors
    }

    /// Expected Write가 기준으로 삼는 raw 이미지 전체.
    pub fn raw_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// 한 raw 2,352바이트 섹터를 돌려준다.
    pub fn raw_sector(&self, lba: u32) -> Result<&[u8], ImageError> {
        if lba >= self.sectors {
            return Err(ImageError::LbaOutOfRange {
                lba,
                sectors: self.sectors,
            });
        }
        let start = lba as usize * SECTOR_LEN;
        Ok(&self.bytes[start..start + SECTOR_LEN])
    }

    /// EDC/ECC까지 검증한 Form1 사용자 데이터 한 섹터를 돌려준다.
    pub fn form1_data(&self, lba: u32) -> Result<&[u8], ImageError> {
        verified_form1_data(self.raw_sector(lba)?)
            .map_err(|source| ImageError::Sector { lba, source })
    }

    /// 이미지의 모든 섹터를 form별 EDC/ECC 규칙으로 검증한다.
    pub fn verify_all_sectors(&self) -> Result<SectorCounts, ImageError> {
        let mut counts = SectorCounts { form1: 0, form2: 0 };
        for lba in 0..self.sectors {
            let sector = verified_data(self.raw_sector(lba)?)
                .map_err(|source| ImageError::Sector { lba, source })?;
            match sector.kind {
                SectorKind::Form1 => counts.form1 += 1,
                SectorKind::Form2 => counts.form2 += 1,
            }
        }
        Ok(counts)
    }
}

/// 크기와 SHA-1이 정확히 일치하는 불변 지원 원본.
#[derive(Debug)]
pub struct VerifiedSourceImage {
    image: DiscImage,
}

impl VerifiedSourceImage {
    /// 파일 전체를 읽고 고정 지원 원본의 크기·SHA-1·섹터 수를 확인한다.
    pub fn open(path: &Path) -> Result<Self, ImageError> {
        let metadata = fs::metadata(path)?;
        if metadata.len() != BIN_LEN {
            return Err(ImageError::WrongLength {
                expected: BIN_LEN,
                actual: metadata.len(),
            });
        }
        let bytes = fs::read(path)?;
        let actual_sha1 = format!("{:x}", Sha1::digest(&bytes));
        if actual_sha1 != BIN_SHA1 {
            return Err(ImageError::Sha1Mismatch {
                expected: BIN_SHA1,
                actual: actual_sha1,
            });
        }
        let image = DiscImage::from_bytes(bytes)?;
        if image.sector_count() != VOLUME_SECTORS {
            return Err(ImageError::WrongSectorCount {
                expected: VOLUME_SECTORS,
                actual: image.sector_count(),
            });
        }
        Ok(Self { image })
    }

    /// 검증한 원본의 raw 디스크 뷰.
    pub fn image(&self) -> &DiscImage {
        &self.image
    }
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
