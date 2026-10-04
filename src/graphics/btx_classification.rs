//! 번역 대상이 아닌 것으로 판정한 BTX 대표 표본의 원본 계약.
//!
//! 사람의 화면 판정 자체를 자동화하지는 않는다. 대신 그 판정이 귀속된
//! 정확한 해제 원문, layout, palette와 물리 사본을 지원 원본에서 다시
//! 검증해 다른 자산이나 잘못 감긴 preview에 결론을 이식하지 못하게 한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::text_candidates::PendingTextAssetSpec;
use super::text_catalog::TextAssetSpec;
use crate::btx::{decode, BtxError, Layout, PixelFormat};

/// 전체 자산을 올바른 layout으로 검토한 사람 판정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtxObservedClass {
    /// 글리프나 baked 문구가 아니라 sprite·effect cell만 관측됐다.
    NonTextSpriteCells,
}

/// 하나의 고유 해제 내용과 그 물리 사본에 귀속한 분류 계약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxClassificationSpec<'a> {
    pub sample_id: &'a str,
    pub archive_paths: &'a [&'a str],
    pub name: &'a str,
    pub layout: Layout,
    pub expected_decompressed_sha256: &'a str,
    pub expected_palette_indices: &'a [u8],
    pub expected_nonzero_pixels: usize,
    pub observed_class: BtxObservedClass,
    /// 사람이 전체 자산을 판정할 때 사용한 근거를 짧게 보존한다.
    pub observation: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxClassificationCatalogReport {
    pub samples: usize,
    pub declared_physical_assets: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxClassificationAssetReport {
    pub pixels: usize,
    pub nonzero_pixels: usize,
    pub palette_indices: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BtxClassificationError {
    Btx(BtxError),
    EmptySampleId,
    DuplicateSampleId(String),
    EmptyArchivePaths {
        sample_id: String,
    },
    EmptyArchivePath {
        sample_id: String,
    },
    DuplicatePhysicalAsset {
        archive_path: String,
        name: String,
    },
    TextPhysicalAssetConflict {
        archive_path: String,
        name: String,
    },
    EmptyName {
        sample_id: String,
    },
    EmptyObservation {
        sample_id: String,
    },
    DigestMismatch {
        sample_id: String,
        expected: String,
        actual: String,
    },
    PaletteMismatch {
        sample_id: String,
        expected: Vec<u8>,
        actual: Vec<u8>,
    },
    NonzeroPixelMismatch {
        sample_id: String,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for BtxClassificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Btx(source) => write!(f, "BTX 분류 layout decode 실패: {source}"),
            Self::EmptySampleId => f.write_str("BTX 분류 sample ID가 비어 있음"),
            Self::DuplicateSampleId(id) => write!(f, "BTX 분류 sample ID 중복: {id}"),
            Self::EmptyArchivePaths { sample_id } => {
                write!(f, "{sample_id}: 물리 archive 경로가 없음")
            }
            Self::EmptyArchivePath { sample_id } => {
                write!(f, "{sample_id}: 빈 archive 경로")
            }
            Self::DuplicatePhysicalAsset { archive_path, name } => {
                write!(f, "BTX 분류 물리 자산 중복: {archive_path}:{name}")
            }
            Self::TextPhysicalAssetConflict { archive_path, name } => write!(
                f,
                "비텍스트 분류 자산이 text catalog 또는 pending에도 있음: \
                 {archive_path}:{name}"
            ),
            Self::EmptyName { sample_id } => write!(f, "{sample_id}: 빈 자산 이름"),
            Self::EmptyObservation { sample_id } => write!(f, "{sample_id}: 빈 관측 근거"),
            Self::DigestMismatch {
                sample_id,
                expected,
                actual,
            } => write!(
                f,
                "{sample_id}: 해제 원문 SHA-256 불일치: expected={expected}, actual={actual}"
            ),
            Self::PaletteMismatch {
                sample_id,
                expected,
                actual,
            } => write!(
                f,
                "{sample_id}: palette index 집합 불일치: expected={expected:?}, actual={actual:?}"
            ),
            Self::NonzeroPixelMismatch {
                sample_id,
                expected,
                actual,
            } => write!(
                f,
                "{sample_id}: nonzero pixel 수 불일치: expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for BtxClassificationError {}

impl From<BtxError> for BtxClassificationError {
    fn from(source: BtxError) -> Self {
        Self::Btx(source)
    }
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

/// 분류 표본 ID와 물리 자산의 유일성을 검증한다.
pub fn verify_btx_classification_specs(
    specs: &[BtxClassificationSpec<'_>],
) -> Result<BtxClassificationCatalogReport, BtxClassificationError> {
    let mut ids = BTreeSet::new();
    let mut physical_assets = BTreeSet::new();
    let mut declared_physical_assets = 0usize;
    for spec in specs {
        if spec.sample_id.is_empty() {
            return Err(BtxClassificationError::EmptySampleId);
        }
        if !ids.insert(spec.sample_id) {
            return Err(BtxClassificationError::DuplicateSampleId(
                spec.sample_id.to_string(),
            ));
        }
        if spec.archive_paths.is_empty() {
            return Err(BtxClassificationError::EmptyArchivePaths {
                sample_id: spec.sample_id.to_string(),
            });
        }
        if spec.name.is_empty() {
            return Err(BtxClassificationError::EmptyName {
                sample_id: spec.sample_id.to_string(),
            });
        }
        if spec.observation.is_empty() {
            return Err(BtxClassificationError::EmptyObservation {
                sample_id: spec.sample_id.to_string(),
            });
        }
        for &archive_path in spec.archive_paths {
            if archive_path.is_empty() {
                return Err(BtxClassificationError::EmptyArchivePath {
                    sample_id: spec.sample_id.to_string(),
                });
            }
            if !physical_assets.insert((archive_path, spec.name)) {
                return Err(BtxClassificationError::DuplicatePhysicalAsset {
                    archive_path: archive_path.to_string(),
                    name: spec.name.to_string(),
                });
            }
            declared_physical_assets += 1;
        }
    }
    Ok(BtxClassificationCatalogReport {
        samples: specs.len(),
        declared_physical_assets,
    })
}

/// 비텍스트 표본이 양성 text catalog·pending과 겹치지 않는지 검증한다.
pub fn verify_btx_classification_boundary(
    classified: &[BtxClassificationSpec<'_>],
    text_assets: &[TextAssetSpec<'_>],
    pending_assets: &[PendingTextAssetSpec<'_>],
) -> Result<(), BtxClassificationError> {
    verify_btx_classification_specs(classified)?;
    let physical_assets = classified
        .iter()
        .flat_map(|spec| {
            spec.archive_paths
                .iter()
                .map(move |archive| (*archive, spec.name))
        })
        .collect::<BTreeSet<_>>();
    for (archive_path, name) in text_assets
        .iter()
        .flat_map(|spec| {
            spec.archive_paths
                .iter()
                .map(move |archive| (*archive, spec.name))
        })
        .chain(pending_assets.iter().flat_map(|spec| {
            spec.archive_paths
                .iter()
                .map(move |archive| (*archive, spec.name))
        }))
    {
        if physical_assets.contains(&(archive_path, name)) {
            return Err(BtxClassificationError::TextPhysicalAssetConflict {
                archive_path: archive_path.to_string(),
                name: name.to_string(),
            });
        }
    }
    Ok(())
}

/// 한 물리 사본이 사람이 판정한 정확한 원본인지 검증한다.
pub fn verify_btx_classification_asset(
    raw: &[u8],
    spec: BtxClassificationSpec<'_>,
) -> Result<BtxClassificationAssetReport, BtxClassificationError> {
    let actual_digest = digest(raw);
    if actual_digest != spec.expected_decompressed_sha256 {
        return Err(BtxClassificationError::DigestMismatch {
            sample_id: spec.sample_id.to_string(),
            expected: spec.expected_decompressed_sha256.to_string(),
            actual: actual_digest,
        });
    }
    let pixels = decode(raw, spec.layout)?;
    let actual_palette = pixels
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if actual_palette != spec.expected_palette_indices {
        return Err(BtxClassificationError::PaletteMismatch {
            sample_id: spec.sample_id.to_string(),
            expected: spec.expected_palette_indices.to_vec(),
            actual: actual_palette,
        });
    }
    let nonzero_pixels = pixels.iter().filter(|pixel| **pixel != 0).count();
    if nonzero_pixels != spec.expected_nonzero_pixels {
        return Err(BtxClassificationError::NonzeroPixelMismatch {
            sample_id: spec.sample_id.to_string(),
            expected: spec.expected_nonzero_pixels,
            actual: nonzero_pixels,
        });
    }
    Ok(BtxClassificationAssetReport {
        pixels: pixels.len(),
        nonzero_pixels,
        palette_indices: actual_palette.len(),
    })
}

const P2_INFO2_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const P2_INFO2_PALETTE_INDICES: [u8; 14] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 14, 15];

/// PUYO2 info family에서 전체 layout을 확인한 첫 non-text 표본.
pub const P2_INFO2_CLASSIFICATION: BtxClassificationSpec<'static> = BtxClassificationSpec {
    sample_id: "BTXC-0001",
    archive_paths: &P2_INFO2_ARCHIVES,
    name: "P2_INFO2.BTX",
    layout: Layout {
        width: 144,
        height: 176,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "2b4def8dceb716d125de8e24fbc06cc2334e3560a525f2648aac1b07688630cb",
    expected_palette_indices: &P2_INFO2_PALETTE_INDICES,
    expected_nonzero_pixels: 5_469,
    observed_class: BtxObservedClass::NonTextSpriteCells,
    observation: "PUYO2 exact 144x176 layout 전체에 뿌요 표정과 effect cell만 관측",
};

pub const BOX_BTX_CLASSIFICATION_SAMPLES: [BtxClassificationSpec<'static>; 1] =
    [P2_INFO2_CLASSIFICATION];

#[cfg(test)]
#[path = "btx_classification_tests.rs"]
mod tests;
