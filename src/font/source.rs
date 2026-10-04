//! 버전이 고정된 글꼴 원천.
//!
//! Maplestory Light 파일은 NEXON의 공식 배포본을 수정하지 않은 채 포함한다.
//! 정확한 출처와 배포 조건은 `assets/fonts/README.md`가 소유한다.

use std::error::Error;
use std::fmt;

use fontdue::{Font, FontSettings};
use sha1::{Digest, Sha1};
use sha2::Sha256;

/// profile이 검증된 Maplestory Light를 참조할 때 쓰는 안정 ID.
pub const MAPLESTORY_LIGHT_FONT_ID: &str = "maplestory-light";

/// 포함한 Maplestory Light TTF의 바이트 길이.
pub const MAPLESTORY_LIGHT_LEN: usize = 1_357_656;

/// 포함한 Maplestory Light TTF의 SHA-1.
pub const MAPLESTORY_LIGHT_SHA1: &str = "578ab49cd5dd25e3a13a2ca34f55019123d09f4d";

/// 자매 프로젝트와 대조한 Maplestory Light TTF의 SHA-256.
pub const MAPLESTORY_LIGHT_SHA256: &str =
    "6d51d8e576f77b01914095aa1f69f9d37c16d93fe940d748962867f218442ba9";

/// bundle·embedded font 배포물에 함께 유지할 저작권 안내.
pub const MAPLESTORY_LIGHT_NOTICE: &str =
    "MapleStory Light font copyright NEXON Korea Corporation.";

const MAPLESTORY_LIGHT_BYTES: &[u8] = include_bytes!("../../assets/fonts/MaplestoryLight.ttf");

/// 글꼴 원천 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontSourceError {
    WrongLength {
        expected: usize,
        actual: usize,
    },
    Sha1Mismatch {
        expected: &'static str,
        actual: String,
    },
    Sha256Mismatch {
        expected: &'static str,
        actual: String,
    },
    InvalidFont(String),
}

impl fmt::Display for FontSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength { expected, actual } => {
                write!(
                    f,
                    "Maplestory Light 길이 불일치: expected={expected}, actual={actual}"
                )
            }
            Self::Sha1Mismatch { expected, actual } => write!(
                f,
                "Maplestory Light SHA-1 불일치: expected={expected}, actual={actual}"
            ),
            Self::Sha256Mismatch { expected, actual } => write!(
                f,
                "Maplestory Light SHA-256 불일치: expected={expected}, actual={actual}"
            ),
            Self::InvalidFont(error) => {
                write!(f, "Maplestory Light 파싱 실패: {error}")
            }
        }
    }
}

impl Error for FontSourceError {}

/// 정확한 Maplestory Light 배포본만 `fontdue` 입력으로 승인한다.
pub fn load_maplestory_light(bytes: &[u8]) -> Result<Font, FontSourceError> {
    if bytes.len() != MAPLESTORY_LIGHT_LEN {
        return Err(FontSourceError::WrongLength {
            expected: MAPLESTORY_LIGHT_LEN,
            actual: bytes.len(),
        });
    }
    let actual = format!("{:x}", Sha1::digest(bytes));
    if actual != MAPLESTORY_LIGHT_SHA1 {
        return Err(FontSourceError::Sha1Mismatch {
            expected: MAPLESTORY_LIGHT_SHA1,
            actual,
        });
    }
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != MAPLESTORY_LIGHT_SHA256 {
        return Err(FontSourceError::Sha256Mismatch {
            expected: MAPLESTORY_LIGHT_SHA256,
            actual,
        });
    }

    Font::from_bytes(bytes, FontSettings::default())
        .map_err(|error| FontSourceError::InvalidFont(error.to_string()))
}

/// 저장소에 포함한 검증된 Maplestory Light를 로드한다.
pub fn bundled_maplestory_light() -> Result<Font, FontSourceError> {
    load_maplestory_light(MAPLESTORY_LIGHT_BYTES)
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
