//! `TUU_SEQ` CEL과 `TUU_GRP` 캐릭터 BTX atlas의 전수 대응.
//!
//! CEL 값은 같은 stem의 256×128 indexed4 atlas를 8×8 tile 512개로 나눈
//! index 범위와 정확히 맞는다. `SKET.CEL`만 `SKE.BTX` 별칭을 쓴다.
//! 512×256 `G_MESS.BTX`는 이 대응 집합이 아니므로 별도 결과 메시지
//! consumer가 소유한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::archive::VddArchive;
use crate::compress::{decompress, parse_header, CnxError};

use super::tuu_cel::TuuCelTable;

pub const TUU_GRP_PATH: &str = "/GOCHA1/TUU_GRP.VDD;1";
const CHARACTER_ATLAS_PLAIN_BYTES: usize = 16_384;
const CHARACTER_ATLAS_WIDTH: usize = 256;
const CHARACTER_ATLAS_HEIGHT: usize = 128;
const TILE_WIDTH: usize = 8;
const TILE_HEIGHT: usize = 8;
const CHARACTER_TILES: usize = 512;
const G_MESS_RESOURCE_ID: usize = 80;
const G_MESS_PLAIN_BYTES: usize = 65_536;
const G_MESS_TILES: usize = 2_048;

/// TUU CEL과 캐릭터 texture의 닫힌 전수 대응 요약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuuCelTextureSummary {
    pub pairs: usize,
    pub exact_stem_pairs: usize,
    pub alias_pairs: usize,
    pub character_atlas_width: usize,
    pub character_atlas_height: usize,
    pub character_tiles: usize,
    pub unique_cel_values: usize,
    pub maximum_cel_value: u16,
    pub g_mess_resource_id: usize,
    pub g_mess_tiles: usize,
}

pub const BOX_EXPECTED_TUU_CEL_TEXTURE_SUMMARY: TuuCelTextureSummary = TuuCelTextureSummary {
    pairs: 49,
    exact_stem_pairs: 48,
    alias_pairs: 1,
    character_atlas_width: CHARACTER_ATLAS_WIDTH,
    character_atlas_height: CHARACTER_ATLAS_HEIGHT,
    character_tiles: CHARACTER_TILES,
    unique_cel_values: CHARACTER_TILES,
    maximum_cel_value: 0x01FF,
    g_mess_resource_id: G_MESS_RESOURCE_ID,
    g_mess_tiles: G_MESS_TILES,
};

/// TUU CEL·texture 대응 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuuCelTextureError {
    UnsupportedCelName(String),
    MissingTexture(String),
    DuplicateTexture(String),
    Cnx {
        name: String,
        source: CnxError,
    },
    Subtype {
        name: String,
        actual: [u8; 3],
    },
    Cel {
        name: String,
        message: String,
    },
    TextureSize {
        name: String,
        expected: usize,
        actual: usize,
    },
    TileIndexOutOfRange {
        name: String,
        value: u16,
        tiles: usize,
    },
    GMessMissing,
    GMessResource {
        expected: usize,
        actual: usize,
    },
    GMessSize {
        expected: usize,
        actual: usize,
    },
    SummaryMismatch {
        expected: Box<TuuCelTextureSummary>,
        actual: Box<TuuCelTextureSummary>,
    },
}

impl fmt::Display for TuuCelTextureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedCelName(name) => write!(f, "TUU CEL 이름 형식 불일치: {name}"),
            Self::MissingTexture(name) => write!(f, "TUU CEL 대응 BTX가 없음: {name}"),
            Self::DuplicateTexture(name) => write!(f, "TUU CEL 대응 BTX가 중복됨: {name}"),
            Self::Cnx { name, source } => write!(f, "{name}: {source}"),
            Self::Subtype { name, actual } => write!(
                f,
                "{name}: CNX subtype이 bi_가 아님: {}",
                String::from_utf8_lossy(actual)
            ),
            Self::Cel { name, message } => write!(f, "{name}: {message}"),
            Self::TextureSize {
                name,
                expected,
                actual,
            } => write!(
                f,
                "{name}: 캐릭터 atlas 크기 불일치: expected={expected}, actual={actual}"
            ),
            Self::TileIndexOutOfRange { name, value, tiles } => write!(
                f,
                "{name}: CEL tile index 범위 이탈: value={value}, tiles={tiles}"
            ),
            Self::GMessMissing => write!(f, "TUU_GRP에 G_MESS.BTX가 없음"),
            Self::GMessResource { expected, actual } => write!(
                f,
                "G_MESS resource ID 불일치: expected={expected}, actual={actual}"
            ),
            Self::GMessSize { expected, actual } => write!(
                f,
                "G_MESS 해제 크기 불일치: expected={expected}, actual={actual}"
            ),
            Self::SummaryMismatch { expected, actual } => write!(
                f,
                "TUU CEL·texture 전수 요약 불일치: expected={expected:?}, actual={actual:?}"
            ),
        }
    }
}

impl Error for TuuCelTextureError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cnx { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn texture_name_for_cel(name: &str) -> Result<(String, bool), TuuCelTextureError> {
    let canonical = name.to_ascii_uppercase();
    let stem = canonical
        .strip_suffix(".CEL")
        .ok_or_else(|| TuuCelTextureError::UnsupportedCelName(name.to_string()))?;
    if stem.is_empty() {
        return Err(TuuCelTextureError::UnsupportedCelName(name.to_string()));
    }
    if stem == "SKET" {
        Ok(("SKE.BTX".to_string(), true))
    } else {
        Ok((format!("{stem}.BTX"), false))
    }
}

fn decompress_checked(
    archive: &VddArchive,
    name: &str,
    expected_subtype: &[u8; 3],
) -> Result<Vec<u8>, TuuCelTextureError> {
    let entry = archive
        .find(name)
        .ok_or_else(|| TuuCelTextureError::MissingTexture(name.to_string()))?;
    let compressed = archive.read_file(entry);
    let header = parse_header(compressed).map_err(|source| TuuCelTextureError::Cnx {
        name: name.to_string(),
        source,
    })?;
    if &header.subtype != expected_subtype {
        return Err(TuuCelTextureError::Subtype {
            name: name.to_string(),
            actual: header.subtype,
        });
    }
    decompress(compressed).map_err(|source| TuuCelTextureError::Cnx {
        name: name.to_string(),
        source,
    })
}

/// 지원 원본의 49개 CEL을 같은 캐릭터 BTX tile domain과 전수 대조한다.
pub fn verify_box_tuu_cel_texture_pairing(
    seq_archive: &VddArchive,
    grp_archive: &VddArchive,
) -> Result<TuuCelTextureSummary, TuuCelTextureError> {
    let mut paired_textures = BTreeSet::new();
    let mut all_values = BTreeSet::new();
    let mut pairs = 0usize;
    let mut alias_pairs = 0usize;

    for cel_entry in seq_archive
        .entries()
        .iter()
        .filter(|entry| entry.name().to_ascii_uppercase().ends_with(".CEL"))
    {
        let cel_name = cel_entry.name().to_ascii_uppercase();
        let (texture_name, alias) = texture_name_for_cel(&cel_name)?;
        if !paired_textures.insert(texture_name.clone()) {
            return Err(TuuCelTextureError::DuplicateTexture(texture_name));
        }

        let cel_raw = decompress_checked(seq_archive, &cel_name, b"cel")?;
        let table = TuuCelTable::parse(&cel_raw).map_err(|source| TuuCelTextureError::Cel {
            name: cel_name.clone(),
            message: source.to_string(),
        })?;
        let texture = decompress_checked(grp_archive, &texture_name, b"bi_")?;
        if texture.len() != CHARACTER_ATLAS_PLAIN_BYTES {
            return Err(TuuCelTextureError::TextureSize {
                name: texture_name,
                expected: CHARACTER_ATLAS_PLAIN_BYTES,
                actual: texture.len(),
            });
        }
        let tiles = texture.len() * 2 / (TILE_WIDTH * TILE_HEIGHT);
        if tiles != CHARACTER_TILES {
            return Err(TuuCelTextureError::TextureSize {
                name: texture_name,
                expected: CHARACTER_ATLAS_PLAIN_BYTES,
                actual: texture.len(),
            });
        }
        for &value in table.records().iter().flat_map(|record| record.values()) {
            if usize::from(value) >= tiles {
                return Err(TuuCelTextureError::TileIndexOutOfRange {
                    name: cel_name,
                    value,
                    tiles,
                });
            }
            all_values.insert(value);
        }
        pairs += 1;
        alias_pairs += usize::from(alias);
    }

    let g_mess = grp_archive
        .entries()
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.name().eq_ignore_ascii_case("G_MESS.BTX"))
        .ok_or(TuuCelTextureError::GMessMissing)?;
    let g_mess_resource_id = g_mess.0 + 1;
    if g_mess_resource_id != G_MESS_RESOURCE_ID {
        return Err(TuuCelTextureError::GMessResource {
            expected: G_MESS_RESOURCE_ID,
            actual: g_mess_resource_id,
        });
    }
    if paired_textures.contains("G_MESS.BTX") {
        return Err(TuuCelTextureError::DuplicateTexture(
            "G_MESS.BTX".to_string(),
        ));
    }
    let g_mess_plain = decompress_checked(grp_archive, "G_MESS.BTX", b"bi_")?;
    if g_mess_plain.len() != G_MESS_PLAIN_BYTES {
        return Err(TuuCelTextureError::GMessSize {
            expected: G_MESS_PLAIN_BYTES,
            actual: g_mess_plain.len(),
        });
    }

    let summary = TuuCelTextureSummary {
        pairs,
        exact_stem_pairs: pairs - alias_pairs,
        alias_pairs,
        character_atlas_width: CHARACTER_ATLAS_WIDTH,
        character_atlas_height: CHARACTER_ATLAS_HEIGHT,
        character_tiles: CHARACTER_TILES,
        unique_cel_values: all_values.len(),
        maximum_cel_value: all_values.last().copied().unwrap_or(0),
        g_mess_resource_id,
        g_mess_tiles: g_mess_plain.len() * 2 / (TILE_WIDTH * TILE_HEIGHT),
    };
    if summary != BOX_EXPECTED_TUU_CEL_TEXTURE_SUMMARY {
        return Err(TuuCelTextureError::SummaryMismatch {
            expected: Box::new(BOX_EXPECTED_TUU_CEL_TEXTURE_SUMMARY),
            actual: Box::new(summary),
        });
    }
    Ok(summary)
}

#[cfg(test)]
#[path = "tuu_cel_texture_tests.rs"]
mod tests;
