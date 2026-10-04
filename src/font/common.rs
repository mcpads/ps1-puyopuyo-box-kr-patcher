//! 9개 서브게임 `COM_FONT.BTX`의 물리 variant 경계.
//!
//! 같은 이름을 공유하지만 지원 원본에는 해제 크기 2종, 고유 픽셀 내용
//! 6종이 있다. 이 모듈은 각 GRP에 귀속된 원본과 256px 폭 해석을 고정한다.
//! rectangle의 소비 의미는 별도 runtime cell table 분석이 소유한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::btx::{decode, encode, Layout, PixelFormat};
use crate::compress::{decompress, parse_header};

const COM_FONT_NAME: &str = "COM_FONT.BTX";

/// 지원 원본 `COM_FONT.BTX` 하나의 물리 사양.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonFontSpec {
    pub archive: &'static str,
    pub width: usize,
    pub height: usize,
    pub plain_bytes: usize,
    pub plain_sha256: &'static str,
    pub nonzero_pixels: usize,
    pub occupied_max_x: usize,
    pub occupied_max_y: usize,
    pub palette_indices: &'static [u8],
}

pub const BOX_COMMON_FONT_SPECS: [CommonFontSpec; 9] = [
    CommonFontSpec {
        archive: "/GOCHA1/TUU_GRP.VDD;1",
        width: 256,
        height: 80,
        plain_bytes: 10_240,
        plain_sha256: "281c53c48ec5d8c0e1f6d733895a6f2efcb196e5f64d56d1e8f4dc205d0ef371",
        nonzero_pixels: 8_720,
        occupied_max_x: 255,
        occupied_max_y: 79,
        palette_indices: &[0, 1, 2, 3, 4, 5, 6, 7, 15],
    },
    CommonFontSpec {
        archive: "/GOCHA2/GOTY_GRP.VDD;1",
        width: 256,
        height: 80,
        plain_bytes: 10_240,
        plain_sha256: "f7f051cbe48a12ec857d7fea728c5fdfaad3500030a83628a73e5428f45c07b9",
        nonzero_pixels: 8_646,
        occupied_max_x: 255,
        occupied_max_y: 79,
        palette_indices: &[0, 1, 2, 3, 4, 5, 6, 7, 15],
    },
    CommonFontSpec {
        archive: "/HORI2/HORI2GRP.VDD;1",
        width: 256,
        height: 80,
        plain_bytes: 10_240,
        plain_sha256: "f7f051cbe48a12ec857d7fea728c5fdfaad3500030a83628a73e5428f45c07b9",
        nonzero_pixels: 8_646,
        occupied_max_x: 255,
        occupied_max_y: 79,
        palette_indices: &[0, 1, 2, 3, 4, 5, 6, 7, 15],
    },
    CommonFontSpec {
        archive: "/HORI4/HORI4GRP.VDD;1",
        width: 256,
        height: 80,
        plain_bytes: 10_240,
        plain_sha256: "3153f09f7572ea7da67875afb6ac0f4d454fac816e05d307c1f18513319f63f7",
        nonzero_pixels: 7_990,
        occupied_max_x: 255,
        occupied_max_y: 75,
        palette_indices: &[0, 1, 2, 3, 4, 5, 6, 7, 15],
    },
    CommonFontSpec {
        archive: "/KOUSIKI/KOU_GRP.VDD;1",
        width: 256,
        height: 128,
        plain_bytes: 16_384,
        plain_sha256: "3abecb9c2c83f36dc1f2823fe1f9a5e8f8ac48a2fe04d8ef9348654ea8e759ab",
        nonzero_pixels: 11_983,
        occupied_max_x: 255,
        occupied_max_y: 127,
        palette_indices: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 14, 15],
    },
    CommonFontSpec {
        archive: "/MINPUYO/MIN_GRP.VDD;1",
        width: 256,
        height: 80,
        plain_bytes: 10_240,
        plain_sha256: "3153f09f7572ea7da67875afb6ac0f4d454fac816e05d307c1f18513319f63f7",
        nonzero_pixels: 7_990,
        occupied_max_x: 255,
        occupied_max_y: 75,
        palette_indices: &[0, 1, 2, 3, 4, 5, 6, 7, 15],
    },
    CommonFontSpec {
        archive: "/PUYO1/PP1_GRP.VDD;1",
        width: 256,
        height: 128,
        plain_bytes: 16_384,
        plain_sha256: "40f24bb09efd4ccd5e76de092a34315717c61ec68c93c8b82ec28a55bcfe8e78",
        nonzero_pixels: 11_975,
        occupied_max_x: 255,
        occupied_max_y: 127,
        palette_indices: &[0, 1, 2, 3, 4, 5, 6, 8, 10, 11, 14, 15],
    },
    CommonFontSpec {
        archive: "/PUYO2/PP2_GRP.VDD;1",
        width: 256,
        height: 128,
        plain_bytes: 16_384,
        plain_sha256: "3abecb9c2c83f36dc1f2823fe1f9a5e8f8ac48a2fe04d8ef9348654ea8e759ab",
        nonzero_pixels: 11_983,
        occupied_max_x: 255,
        occupied_max_y: 127,
        palette_indices: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 14, 15],
    },
    CommonFontSpec {
        archive: "/QUEST/RPG_GRP.VDD;1",
        width: 256,
        height: 128,
        plain_bytes: 16_384,
        plain_sha256: "a70e9dba84e78baefb408d793bf360645e712219b677390d549255553387b452",
        nonzero_pixels: 11_772,
        occupied_max_x: 255,
        occupied_max_y: 127,
        palette_indices: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 11, 15],
    },
];

/// 9개 물리 사본 검증 요약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonFontSummary {
    pub variants: usize,
    pub unique_contents: usize,
    pub short_variants: usize,
    pub tall_variants: usize,
    pub total_nonzero_pixels: usize,
}

/// `COM_FONT.BTX` 원본 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonFontError {
    DuplicateArchive(String),
    MissingArchive(&'static str),
    MissingAsset(&'static str),
    InvalidCnx {
        archive: &'static str,
        detail: String,
    },
    WrongSubtype {
        archive: &'static str,
        actual: [u8; 3],
    },
    PlainLength {
        archive: &'static str,
        expected: usize,
        actual: usize,
    },
    Digest {
        archive: &'static str,
        expected: String,
        actual: String,
    },
    Btx {
        archive: &'static str,
        detail: String,
    },
    Roundtrip(&'static str),
    PixelSummary {
        archive: &'static str,
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    Palette {
        archive: &'static str,
        expected: Vec<u8>,
        actual: Vec<u8>,
    },
    UniqueContentCount {
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for CommonFontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateArchive(archive) => write!(f, "COM_FONT GRP 입력 중복: {archive}"),
            Self::MissingArchive(archive) => write!(f, "COM_FONT GRP 입력 누락: {archive}"),
            Self::MissingAsset(archive) => write!(f, "{archive}:{COM_FONT_NAME} 누락"),
            Self::InvalidCnx { archive, detail } => {
                write!(f, "{archive}:{COM_FONT_NAME} CNX 오류: {detail}")
            }
            Self::WrongSubtype { archive, actual } => write!(
                f,
                "{archive}:{COM_FONT_NAME} subtype 불일치: actual={actual:02X?}"
            ),
            Self::PlainLength {
                archive,
                expected,
                actual,
            } => write!(
                f,
                "{archive}:{COM_FONT_NAME} 해제 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::Digest {
                archive,
                expected,
                actual,
            } => write!(
                f,
                "{archive}:{COM_FONT_NAME} SHA-256 불일치: expected={expected}, actual={actual}"
            ),
            Self::Btx { archive, detail } => {
                write!(f, "{archive}:{COM_FONT_NAME} BTX 오류: {detail}")
            }
            Self::Roundtrip(archive) => {
                write!(f, "{archive}:{COM_FONT_NAME} indexed4 왕복 불일치")
            }
            Self::PixelSummary {
                archive,
                field,
                expected,
                actual,
            } => write!(
                f,
                "{archive}:{COM_FONT_NAME} {field} 불일치: expected={expected}, actual={actual}"
            ),
            Self::Palette {
                archive,
                expected,
                actual,
            } => write!(
                f,
                "{archive}:{COM_FONT_NAME} palette index 불일치: \
                 expected={expected:?}, actual={actual:?}"
            ),
            Self::UniqueContentCount { expected, actual } => write!(
                f,
                "COM_FONT 고유 내용 수 불일치: expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for CommonFontError {}

/// 11개 GRP 입력에서 지원 원본의 `COM_FONT.BTX` 9개를 검증한다.
pub fn verify_box_common_fonts<'a>(
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<CommonFontSummary, CommonFontError> {
    let mut by_id = BTreeMap::new();
    for (archive_id, archive) in archives {
        if by_id.insert(archive_id, archive).is_some() {
            return Err(CommonFontError::DuplicateArchive(archive_id.to_string()));
        }
    }

    let mut unique_contents = BTreeSet::new();
    let mut short_variants = 0;
    let mut tall_variants = 0;
    let mut total_nonzero_pixels = 0;
    for spec in BOX_COMMON_FONT_SPECS {
        let archive = by_id
            .get(spec.archive)
            .ok_or(CommonFontError::MissingArchive(spec.archive))?;
        verified_common_font_pixels(spec, archive)?;

        unique_contents.insert(spec.plain_sha256);
        match spec.height {
            80 => short_variants += 1,
            128 => tall_variants += 1,
            _ => unreachable!("고정 COM_FONT 사양 높이"),
        }
        total_nonzero_pixels += spec.nonzero_pixels;
    }
    if unique_contents.len() != 6 {
        return Err(CommonFontError::UniqueContentCount {
            expected: 6,
            actual: unique_contents.len(),
        });
    }

    Ok(CommonFontSummary {
        variants: BOX_COMMON_FONT_SPECS.len(),
        unique_contents: unique_contents.len(),
        short_variants,
        tall_variants,
        total_nonzero_pixels,
    })
}

pub(crate) fn verified_common_font_pixels(
    spec: CommonFontSpec,
    archive: &VddArchive,
) -> Result<Vec<u8>, CommonFontError> {
    let entry = archive
        .find(COM_FONT_NAME)
        .ok_or(CommonFontError::MissingAsset(spec.archive))?;
    let cnx = archive.read_file(entry);
    let header = parse_header(cnx).map_err(|error| CommonFontError::InvalidCnx {
        archive: spec.archive,
        detail: error.to_string(),
    })?;
    if &header.subtype != b"bi_" {
        return Err(CommonFontError::WrongSubtype {
            archive: spec.archive,
            actual: header.subtype,
        });
    }
    let plain = decompress(cnx).map_err(|error| CommonFontError::InvalidCnx {
        archive: spec.archive,
        detail: error.to_string(),
    })?;
    verify_variant(spec, &plain)
}

fn verify_variant(spec: CommonFontSpec, plain: &[u8]) -> Result<Vec<u8>, CommonFontError> {
    if plain.len() != spec.plain_bytes {
        return Err(CommonFontError::PlainLength {
            archive: spec.archive,
            expected: spec.plain_bytes,
            actual: plain.len(),
        });
    }
    let actual_digest = format!("{:x}", Sha256::digest(plain));
    if actual_digest != spec.plain_sha256 {
        return Err(CommonFontError::Digest {
            archive: spec.archive,
            expected: spec.plain_sha256.to_string(),
            actual: actual_digest,
        });
    }

    let layout = Layout {
        width: spec.width,
        height: spec.height,
        format: PixelFormat::Indexed4,
    };
    let pixels = decode(plain, layout).map_err(|error| CommonFontError::Btx {
        archive: spec.archive,
        detail: error.to_string(),
    })?;
    let encoded = encode(&pixels, layout).map_err(|error| CommonFontError::Btx {
        archive: spec.archive,
        detail: error.to_string(),
    })?;
    if encoded != plain {
        return Err(CommonFontError::Roundtrip(spec.archive));
    }

    let nonzero_pixels = pixels.iter().filter(|pixel| **pixel != 0).count();
    require_pixel_summary(
        spec,
        "nonzero pixel 수",
        spec.nonzero_pixels,
        nonzero_pixels,
    )?;
    let occupied_max_x = pixels
        .iter()
        .enumerate()
        .filter(|(_, pixel)| **pixel != 0)
        .map(|(index, _)| index % spec.width)
        .max()
        .unwrap_or(0);
    require_pixel_summary(spec, "occupied max x", spec.occupied_max_x, occupied_max_x)?;
    let occupied_max_y = pixels
        .iter()
        .enumerate()
        .filter(|(_, pixel)| **pixel != 0)
        .map(|(index, _)| index / spec.width)
        .max()
        .unwrap_or(0);
    require_pixel_summary(spec, "occupied max y", spec.occupied_max_y, occupied_max_y)?;
    let actual_palette: Vec<_> = pixels
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if actual_palette != spec.palette_indices {
        return Err(CommonFontError::Palette {
            archive: spec.archive,
            expected: spec.palette_indices.to_vec(),
            actual: actual_palette,
        });
    }
    Ok(pixels)
}

fn require_pixel_summary(
    spec: CommonFontSpec,
    field: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), CommonFontError> {
    if expected != actual {
        return Err(CommonFontError::PixelSummary {
            archive: spec.archive,
            field,
            expected,
            actual,
        });
    }
    Ok(())
}
