//! `*_SND.VDD`의 표시 텍스트 경계.
//!
//! 엔트리 이름의 `.SEQ`만 보고 script로 분류하지 않는다. Sound VDD의
//! `.SEQ`는 `pQES` magic을 가진 PlayStation sequence이며, `.VH/.VB`는
//! 이름 stem이 짝을 이루는 VAB header/sample body다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::{VabBank, VabError, VddArchive};

const VAB_HEADER_MAGIC: &[u8; 4] = b"pBAV";
const SOUND_SEQUENCE_MAGIC: &[u8; 4] = b"pQES";

/// 고정 지원 원본의 Sound VDD 경로.
pub const BOX_SOUND_ARCHIVE_PATHS: [&str; 11] = [
    "/FIL_SND.VDD;1",
    "/GOCHA1/PP1_SND.VDD;1",
    "/GOCHA2/PP1_SND.VDD;1",
    "/HORI2/PP1_SND.VDD;1",
    "/HORI4/PP1_SND.VDD;1",
    "/KOUSIKI/KOU_SND.VDD;1",
    "/MINPUYO/PP1_SND.VDD;1",
    "/PCOMI/PC1_SND.VDD;1",
    "/PUYO1/PP1_SND.VDD;1",
    "/PUYO2/PP2_SND.VDD;1",
    "/QUEST/RPG_SND.VDD;1",
];

/// Sound VDD 구조 분모.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundInventorySummary {
    pub archives: usize,
    pub entries: usize,
    pub vab_headers: usize,
    pub vab_bodies: usize,
    pub sound_sequences: usize,
}

/// 고정 지원 원본에서 기대하는 Sound VDD 구조 분모.
pub const BOX_EXPECTED_SOUND_INVENTORY: SoundInventorySummary = SoundInventorySummary {
    archives: 11,
    entries: 1_903,
    vab_headers: 781,
    vab_bodies: 781,
    sound_sequences: 341,
};

/// Sound VDD 분류 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoundInventoryError {
    EmptyInput,
    EmptyArchiveId,
    DuplicateArchiveId(String),
    UnsupportedEntry {
        archive: String,
        name: String,
    },
    InvalidMagic {
        archive: String,
        name: String,
        expected: [u8; 4],
    },
    InvalidVab {
        archive: String,
        header: String,
        body: String,
        source: VabError,
    },
    UnpairedVab {
        archive: String,
        headers: Vec<String>,
        bodies: Vec<String>,
    },
    MissingBoxArchive(&'static str),
    UnexpectedBoxArchive(String),
    SummaryMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for SoundInventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "Sound VDD 인벤토리 입력이 없음"),
            Self::EmptyArchiveId => write!(f, "Sound VDD 식별자가 비어 있음"),
            Self::DuplicateArchiveId(id) => write!(f, "Sound VDD 식별자 중복: {id}"),
            Self::UnsupportedEntry { archive, name } => {
                write!(f, "{archive}:{name}: 지원 Sound 엔트리 형식이 아님")
            }
            Self::InvalidMagic {
                archive,
                name,
                expected,
            } => write!(
                f,
                "{archive}:{name}: Sound magic 불일치: expected={expected:02X?}"
            ),
            Self::InvalidVab {
                archive,
                header,
                body,
                source,
            } => write!(
                f,
                "{archive}:{header}/{body}: split VAB 검증 실패: {source}"
            ),
            Self::UnpairedVab {
                archive,
                headers,
                bodies,
            } => write!(
                f,
                "{archive}: VAB .VH/.VB stem 짝 불일치: headers={headers:?}, bodies={bodies:?}"
            ),
            Self::MissingBoxArchive(archive) => {
                write!(f, "지원 원본 Sound VDD 누락: {archive}")
            }
            Self::UnexpectedBoxArchive(archive) => {
                write!(f, "지원 원본에 선언하지 않은 Sound VDD: {archive}")
            }
            Self::SummaryMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "Sound VDD {field} 불일치: expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for SoundInventoryError {}

/// Sound VDD의 모든 엔트리를 VAB header/body 또는 PS1 sequence로 분류한다.
pub fn audit_sound_archives<'a>(
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<SoundInventorySummary, SoundInventoryError> {
    let mut archive_ids = BTreeSet::new();
    let mut summary = SoundInventorySummary {
        archives: 0,
        entries: 0,
        vab_headers: 0,
        vab_bodies: 0,
        sound_sequences: 0,
    };

    for (archive_id, archive) in archives {
        if archive_id.trim().is_empty() {
            return Err(SoundInventoryError::EmptyArchiveId);
        }
        if !archive_ids.insert(archive_id.to_string()) {
            return Err(SoundInventoryError::DuplicateArchiveId(
                archive_id.to_string(),
            ));
        }
        let mut headers = BTreeMap::new();
        let mut bodies = BTreeMap::new();
        summary.archives += 1;
        summary.entries += archive.entry_count();

        for entry in archive.entries() {
            let name = entry.name();
            let upper = name.to_ascii_uppercase();
            let data = archive.read_file(entry);
            if let Some(stem) = upper.strip_suffix(".VH") {
                require_magic(archive_id, name, data, VAB_HEADER_MAGIC)?;
                headers.insert(stem.to_string(), (name.to_string(), data));
                summary.vab_headers += 1;
            } else if let Some(stem) = upper.strip_suffix(".VB") {
                bodies.insert(stem.to_string(), (name.to_string(), data));
                summary.vab_bodies += 1;
            } else if upper.ends_with(".SEQ") {
                require_magic(archive_id, name, data, SOUND_SEQUENCE_MAGIC)?;
                summary.sound_sequences += 1;
            } else {
                return Err(SoundInventoryError::UnsupportedEntry {
                    archive: archive_id.to_string(),
                    name: name.to_string(),
                });
            }
        }
        let header_stems = headers.keys().cloned().collect::<BTreeSet<_>>();
        let body_stems = bodies.keys().cloned().collect::<BTreeSet<_>>();
        if header_stems != body_stems {
            return Err(SoundInventoryError::UnpairedVab {
                archive: archive_id.to_string(),
                headers: header_stems.into_iter().collect(),
                bodies: body_stems.into_iter().collect(),
            });
        }
        for (stem, (header_name, header)) in headers {
            let (body_name, body) = bodies.get(&stem).unwrap();
            VabBank::parse(header, body).map_err(|source| SoundInventoryError::InvalidVab {
                archive: archive_id.to_string(),
                header: header_name,
                body: body_name.clone(),
                source,
            })?;
        }
    }

    if summary.archives == 0 {
        return Err(SoundInventoryError::EmptyInput);
    }
    Ok(summary)
}

/// 지원 원본의 11개 Sound VDD 경로와 구조 분모를 정확히 검증한다.
pub fn verify_box_sound_inventory<'a>(
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<SoundInventorySummary, SoundInventoryError> {
    let archives: Vec<_> = archives.into_iter().collect();
    let actual_ids: BTreeSet<_> = archives.iter().map(|(id, _)| *id).collect();
    for expected in BOX_SOUND_ARCHIVE_PATHS {
        if !actual_ids.contains(expected) {
            return Err(SoundInventoryError::MissingBoxArchive(expected));
        }
    }
    let expected_ids: BTreeSet<_> = BOX_SOUND_ARCHIVE_PATHS.into_iter().collect();
    if let Some(unexpected) = actual_ids.difference(&expected_ids).next() {
        return Err(SoundInventoryError::UnexpectedBoxArchive(
            (*unexpected).to_string(),
        ));
    }

    let summary = audit_sound_archives(archives)?;
    require_field(
        "archive 수",
        BOX_EXPECTED_SOUND_INVENTORY.archives,
        summary.archives,
    )?;
    require_field(
        "entry 수",
        BOX_EXPECTED_SOUND_INVENTORY.entries,
        summary.entries,
    )?;
    require_field(
        "VAB header 수",
        BOX_EXPECTED_SOUND_INVENTORY.vab_headers,
        summary.vab_headers,
    )?;
    require_field(
        "VAB body 수",
        BOX_EXPECTED_SOUND_INVENTORY.vab_bodies,
        summary.vab_bodies,
    )?;
    require_field(
        "sound sequence 수",
        BOX_EXPECTED_SOUND_INVENTORY.sound_sequences,
        summary.sound_sequences,
    )?;
    Ok(summary)
}

fn require_magic(
    archive: &str,
    name: &str,
    data: &[u8],
    expected: &[u8; 4],
) -> Result<(), SoundInventoryError> {
    if !data.starts_with(expected) {
        return Err(SoundInventoryError::InvalidMagic {
            archive: archive.to_string(),
            name: name.to_string(),
            expected: *expected,
        });
    }
    Ok(())
}

fn require_field(
    field: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), SoundInventoryError> {
    if expected != actual {
        return Err(SoundInventoryError::SummaryMismatch {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "sound_inventory_tests.rs"]
mod sound_inventory_tests;
