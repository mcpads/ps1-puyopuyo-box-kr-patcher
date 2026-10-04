//! 아직 runtime layout·block 분할이 닫히지 않은 BTX 양성 자산 계약.
//!
//! 이 목록은 번역 가능한 block catalog가 아니다. 사람의 시각 관측에서
//! 일본어 픽셀을 확인했지만 실제 화면의 저장→상주→소비 연결이나 block
//! 경계가 남은 자산만 보존한다. `review_layout`의 증거 수준은 시각적 행
//! 감기 후보, SEQ의 exact resource/cell header, raw VRAM whole-asset
//! exact를 `layout_basis`로 구분한다. 승격 뒤에도 정적 consumer 분석을
//! 재현해야 하는 자산은 별도 provenance에 남기고 현재 pending 큐와 섞지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::text_catalog::{TextAssetSpec, TextStorageSpec};
use crate::btx::{decode, BtxError, Layout, PixelFormat};

/// review layout의 증거 수준.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewLayoutBasis {
    /// 픽셀 연속성을 검토하기 위한 행 감기 후보. runtime 폭이 아니다.
    VisualCandidate,
    /// exact resource의 canonical SEQ setup과 plain 길이가 함께 확정한 폭.
    StaticConsumerExact,
    /// 원본 전체가 raw VRAM에서 exact 직사각형으로 확인된 runtime 폭.
    RuntimeWholeAssetExact,
    /// 같은 frozen 관측점에서 whole-asset VRAM exact와 실제 화면을 확인한 폭.
    RuntimeVisibleWholeAssetExact,
}

impl ReviewLayoutBasis {
    pub const fn label(self) -> &'static str {
        match self {
            Self::VisualCandidate => "visual-candidate",
            Self::StaticConsumerExact => "static-consumer-exact",
            Self::RuntimeWholeAssetExact => "runtime-whole-asset-exact",
            Self::RuntimeVisibleWholeAssetExact => "runtime-visible-whole-asset-exact",
        }
    }

    pub const fn pending_state(self) -> &'static str {
        match self {
            Self::VisualCandidate => "needs-runtime-and-block-split",
            Self::StaticConsumerExact => "needs-runtime-and-block-split",
            Self::RuntimeWholeAssetExact => "needs-screen-and-block-split",
            Self::RuntimeVisibleWholeAssetExact => "needs-block-split",
        }
    }
}

/// block catalog로 승격하기 전의 일본어 양성 BTX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingTextAssetSpec<'a> {
    pub finding_id: &'a str,
    pub archive_paths: &'a [&'a str],
    pub name: &'a str,
    pub review_layout: Layout,
    pub layout_basis: ReviewLayoutBasis,
    pub expected_decompressed_sha256: &'a str,
    /// 사람이 판독한 예시 또는 아직 남은 판독 경계.
    pub observed_text: &'a str,
}

/// pending catalog의 정적 분모.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingTextCatalogReport {
    pub assets: usize,
    pub declared_physical_assets: usize,
}

/// pending BTX 계약 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingTextError {
    Btx(BtxError),
    EmptyFindingId,
    DuplicateFindingId(String),
    EmptyArchivePaths {
        finding_id: String,
    },
    EmptyArchivePath {
        finding_id: String,
    },
    DuplicateArchivePath {
        finding_id: String,
        archive_path: String,
    },
    DuplicatePhysicalAsset {
        archive_path: String,
        name: String,
    },
    ApprovedPhysicalAssetConflict {
        archive_path: String,
        name: String,
    },
    PromotedPhysicalAssetMissing {
        archive_path: String,
        name: String,
    },
    PromotedContractMismatch {
        archive_path: String,
        name: String,
    },
    PromotedStorageMismatch {
        name: String,
    },
    PromotedRasterStorageMismatch {
        name: String,
    },
    EmptyAssetName {
        finding_id: String,
    },
    EmptyObservedText {
        finding_id: String,
    },
    DecompressedDigestMismatch {
        finding_id: String,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for PendingTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Btx(source) => write!(f, "BTX review layout decode 실패: {source}"),
            Self::EmptyFindingId => f.write_str("pending BTX finding ID가 비어 있음"),
            Self::DuplicateFindingId(id) => write!(f, "pending BTX finding ID 중복: {id}"),
            Self::EmptyArchivePaths { finding_id } => {
                write!(f, "{finding_id}: 물리 archive 경로가 없음")
            }
            Self::EmptyArchivePath { finding_id } => {
                write!(f, "{finding_id}: 빈 archive 경로")
            }
            Self::DuplicateArchivePath {
                finding_id,
                archive_path,
            } => write!(f, "{finding_id}: archive 경로 중복: {archive_path}"),
            Self::DuplicatePhysicalAsset { archive_path, name } => {
                write!(f, "pending BTX 물리 자산 중복: {archive_path}:{name}")
            }
            Self::ApprovedPhysicalAssetConflict { archive_path, name } => write!(
                f,
                "pending BTX가 승인 catalog에도 존재함: {archive_path}:{name}"
            ),
            Self::PromotedPhysicalAssetMissing { archive_path, name } => write!(
                f,
                "승격 provenance의 물리 자산이 승인 catalog에 없음: {archive_path}:{name}"
            ),
            Self::PromotedContractMismatch { archive_path, name } => write!(
                f,
                "승격 provenance와 승인 catalog의 layout/digest 불일치: \
                 {archive_path}:{name}"
            ),
            Self::PromotedStorageMismatch { name } => {
                write!(f, "승격 provenance 자산이 fixed cell grid가 아님: {name}")
            }
            Self::PromotedRasterStorageMismatch { name } => {
                write!(f, "승격 provenance 자산이 raster block이 아님: {name}")
            }
            Self::EmptyAssetName { finding_id } => {
                write!(f, "{finding_id}: 자산 이름이 비어 있음")
            }
            Self::EmptyObservedText { finding_id } => {
                write!(f, "{finding_id}: 관측 텍스트 요약이 비어 있음")
            }
            Self::DecompressedDigestMismatch {
                finding_id,
                expected,
                actual,
            } => write!(
                f,
                "{finding_id}: 해제 원문 SHA-256 불일치: expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for PendingTextError {}

impl From<BtxError> for PendingTextError {
    fn from(source: BtxError) -> Self {
        Self::Btx(source)
    }
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

/// pending catalog의 ID와 물리 자산 분모를 검증한다.
pub fn verify_pending_text_catalog_specs(
    specs: &[PendingTextAssetSpec<'_>],
) -> Result<PendingTextCatalogReport, PendingTextError> {
    let mut finding_ids = BTreeSet::new();
    let mut physical_assets = BTreeSet::new();
    let mut declared_physical_assets = 0usize;
    for spec in specs {
        if spec.finding_id.is_empty() {
            return Err(PendingTextError::EmptyFindingId);
        }
        if !finding_ids.insert(spec.finding_id) {
            return Err(PendingTextError::DuplicateFindingId(
                spec.finding_id.to_string(),
            ));
        }
        if spec.archive_paths.is_empty() {
            return Err(PendingTextError::EmptyArchivePaths {
                finding_id: spec.finding_id.to_string(),
            });
        }
        if spec.name.is_empty() {
            return Err(PendingTextError::EmptyAssetName {
                finding_id: spec.finding_id.to_string(),
            });
        }
        if spec.observed_text.is_empty() {
            return Err(PendingTextError::EmptyObservedText {
                finding_id: spec.finding_id.to_string(),
            });
        }
        for &archive_path in spec.archive_paths {
            if archive_path.is_empty() {
                return Err(PendingTextError::EmptyArchivePath {
                    finding_id: spec.finding_id.to_string(),
                });
            }
            if !physical_assets.insert((archive_path, spec.name)) {
                return Err(PendingTextError::DuplicatePhysicalAsset {
                    archive_path: archive_path.to_string(),
                    name: spec.name.to_string(),
                });
            }
            declared_physical_assets += 1;
        }
    }

    Ok(PendingTextCatalogReport {
        assets: specs.len(),
        declared_physical_assets,
    })
}

/// block catalog와 pending 큐의 물리 자산이 서로 겹치지 않는지 검증한다.
pub fn verify_pending_text_catalog_boundary(
    pending_specs: &[PendingTextAssetSpec<'_>],
    approved_specs: &[TextAssetSpec<'_>],
) -> Result<(), PendingTextError> {
    verify_pending_text_catalog_specs(pending_specs)?;

    let approved_physical_assets = approved_specs
        .iter()
        .flat_map(|spec| {
            spec.archive_paths
                .iter()
                .map(move |archive_path| (*archive_path, spec.name))
        })
        .collect::<BTreeSet<_>>();
    for spec in pending_specs {
        for &archive_path in spec.archive_paths {
            if approved_physical_assets.contains(&(archive_path, spec.name)) {
                return Err(PendingTextError::ApprovedPhysicalAssetConflict {
                    archive_path: archive_path.to_string(),
                    name: spec.name.to_string(),
                });
            }
        }
    }
    Ok(())
}

/// block 분할 전 provenance가 현재 fixed-cell 승인 계약과 이어지는지 검증한다.
pub fn verify_promoted_text_catalog_provenance(
    provenance: &[PendingTextAssetSpec<'_>],
    approved: &[TextAssetSpec<'_>],
) -> Result<(), PendingTextError> {
    verify_pending_text_catalog_specs(provenance)?;
    for source in provenance {
        let promoted = approved
            .iter()
            .find(|candidate| {
                candidate.name == source.name
                    && candidate.archive_paths.len() == source.archive_paths.len()
                    && source
                        .archive_paths
                        .iter()
                        .all(|archive| candidate.archive_paths.contains(archive))
            })
            .ok_or_else(|| PendingTextError::PromotedPhysicalAssetMissing {
                archive_path: source.archive_paths.join(","),
                name: source.name.to_string(),
            })?;
        if promoted.layout != source.review_layout
            || promoted.expected_decompressed_sha256 != source.expected_decompressed_sha256
        {
            return Err(PendingTextError::PromotedContractMismatch {
                archive_path: source.archive_paths.join(","),
                name: source.name.to_string(),
            });
        }
        if !matches!(promoted.storage, TextStorageSpec::FixedCellGrid(_)) {
            return Err(PendingTextError::PromotedStorageMismatch {
                name: source.name.to_string(),
            });
        }
    }
    Ok(())
}

/// block 분할 전 provenance가 현재 raster-block 승인 계약과 이어지는지 검증한다.
pub fn verify_promoted_raster_text_catalog_provenance(
    provenance: &[PendingTextAssetSpec<'_>],
    approved: &[TextAssetSpec<'_>],
) -> Result<(), PendingTextError> {
    verify_pending_text_catalog_specs(provenance)?;
    for source in provenance {
        let promoted = approved
            .iter()
            .find(|candidate| {
                candidate.name == source.name
                    && candidate.archive_paths.len() == source.archive_paths.len()
                    && source
                        .archive_paths
                        .iter()
                        .all(|archive| candidate.archive_paths.contains(archive))
            })
            .ok_or_else(|| PendingTextError::PromotedPhysicalAssetMissing {
                archive_path: source.archive_paths.join(","),
                name: source.name.to_string(),
            })?;
        if promoted.layout != source.review_layout
            || promoted.expected_decompressed_sha256 != source.expected_decompressed_sha256
        {
            return Err(PendingTextError::PromotedContractMismatch {
                archive_path: source.archive_paths.join(","),
                name: source.name.to_string(),
            });
        }
        if !matches!(promoted.storage, TextStorageSpec::RasterBlocks) {
            return Err(PendingTextError::PromotedRasterStorageMismatch {
                name: source.name.to_string(),
            });
        }
    }
    Ok(())
}

/// 한 물리 사본의 해제 원문과 review layout을 검증한다.
pub fn verify_pending_text_asset(
    data: &[u8],
    spec: PendingTextAssetSpec<'_>,
) -> Result<(), PendingTextError> {
    let mut archive_paths = BTreeSet::new();
    for &archive_path in spec.archive_paths {
        if archive_path.is_empty() {
            return Err(PendingTextError::EmptyArchivePath {
                finding_id: spec.finding_id.to_string(),
            });
        }
        if !archive_paths.insert(archive_path) {
            return Err(PendingTextError::DuplicateArchivePath {
                finding_id: spec.finding_id.to_string(),
                archive_path: archive_path.to_string(),
            });
        }
    }
    let actual = digest(data);
    if actual != spec.expected_decompressed_sha256 {
        return Err(PendingTextError::DecompressedDigestMismatch {
            finding_id: spec.finding_id.to_string(),
            expected: spec.expected_decompressed_sha256.to_string(),
            actual,
        });
    }
    decode(data, spec.review_layout)?;
    Ok(())
}

fn escape_tsv(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            _ => escaped.push(character),
        }
    }
    escaped
}

/// block 분할 전 양성 자산을 사람 검토용 TSV로 내보낸다.
pub fn render_pending_text_review_tsv() -> Result<String, PendingTextError> {
    render_pending_text_review_tsv_from(&BOX_PENDING_TEXT_ASSETS)
}

fn render_pending_text_review_tsv_from(
    specs: &[PendingTextAssetSpec<'_>],
) -> Result<String, PendingTextError> {
    verify_pending_text_catalog_specs(specs)?;
    let mut specs = specs.iter().collect::<Vec<_>>();
    specs.sort_by_key(|spec| spec.finding_id);

    let mut output = String::from(
        "schema_version\tfinding_id\tarchive_paths\tasset_name\tasset_sha256\t\
         pixel_format\treview_width\treview_height\tlayout_basis\tstate\tobserved_text\n",
    );
    for spec in specs {
        let format = match spec.review_layout.format {
            PixelFormat::Indexed4 => "indexed4",
            PixelFormat::Indexed8 => "indexed8",
        };
        let fields = [
            "btx-pending-review-v1".to_string(),
            spec.finding_id.to_string(),
            escape_tsv(&spec.archive_paths.join("\n")),
            spec.name.to_string(),
            spec.expected_decompressed_sha256.to_string(),
            format.to_string(),
            spec.review_layout.width.to_string(),
            spec.review_layout.height.to_string(),
            spec.layout_basis.label().to_string(),
            spec.layout_basis.pending_state().to_string(),
            escape_tsv(spec.observed_text),
        ];
        output.push_str(&fields.join("\t"));
        output.push('\n');
    }
    Ok(output)
}

const P2_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const GOCHA1_ARCHIVES: [&str; 1] = ["/GOCHA1/TUU_GRP.VDD;1"];
const SUBGAME_KANBAN_ARCHIVES: [&str; 5] = [
    "/GOCHA1/TUU_GRP.VDD;1",
    "/GOCHA2/GOTY_GRP.VDD;1",
    "/HORI2/HORI2GRP.VDD;1",
    "/HORI4/HORI4GRP.VDD;1",
    "/MINPUYO/MIN_GRP.VDD;1",
];
const PUYO1_ARCHIVES: [&str; 1] = ["/PUYO1/PP1_GRP.VDD;1"];
const PUYO2_ARCHIVES: [&str; 1] = ["/PUYO2/PP2_GRP.VDD;1"];
/// fixed-cell catalog 승격 전 정적 consumer 분석을 재현하는 provenance.
///
/// 현재 pending 큐가 아니며 `BOX_TEXT_ASSETS`와 exact 계약으로 교차한다.
pub const BOX_PROMOTED_CELL_GRID_PROVENANCE: [PendingTextAssetSpec<'static>; 2] = [
    PendingTextAssetSpec {
        finding_id: "BTXF-016",
        archive_paths: &P2_ARCHIVES,
        name: "P2_INFO4.BTX",
        review_layout: Layout {
            width: 256,
            height: 256,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::StaticConsumerExact,
        expected_decompressed_sha256:
            "e033d4211bf2e7ebb58e5540a252fb69ff29ad62890f32609bc1552de1273f09",
        observed_text: "落ちてくるぷよはレバーで左右にうごかせる。 (본문 계속)",
    },
    PendingTextAssetSpec {
        finding_id: "BTXF-017",
        archive_paths: &P2_ARCHIVES,
        name: "P2_INFO5.BTX",
        review_layout: Layout {
            width: 256,
            height: 256,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::StaticConsumerExact,
        expected_decompressed_sha256:
            "16ada14af66f89e2788ad825c805f2d14be59445e138d53950520d184bb912ed",
        observed_text: "相手からのおじゃまぷよはこちらからのおじゃまぷよで打ち消すことができるぞ。",
    },
];

/// PUYO1/PUYO2 공용 menu family의 정적 consumer exact 양성 표본.
///
/// block 승격 뒤에도 EXE별 resource와 SEQ setup을 다시 검산하기 위해 현재
/// pending 배열과 별도의 provenance로 보존한다.
pub const BOX_MENU_STATIC_TEXT_PROVENANCE: [PendingTextAssetSpec<'static>; 5] = [
    PendingTextAssetSpec {
        finding_id: "BTXF-023",
        archive_paths: &P2_ARCHIVES,
        name: "MENU01.BTX",
        review_layout: Layout {
            width: 512,
            height: 256,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::StaticConsumerExact,
        expected_decompressed_sha256:
            "b9cbb55b12018a2a0f6ff1ba41058e8442676b60a54a21282edb339e8715b746",
        observed_text: "モード・レベルを選択して下さい。／ひとりで・ふたりで",
    },
    PendingTextAssetSpec {
        finding_id: "BTXF-024",
        archive_paths: &P2_ARCHIVES,
        name: "MENU06.BTX",
        review_layout: Layout {
            width: 512,
            height: 256,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::StaticConsumerExact,
        expected_decompressed_sha256:
            "7a621068d7341bbb6a09f4905fe5650fe228f5a6d908fbaab495376e656f01d5",
        observed_text: "あいうえお…／相殺・連鎖 등 일본어 glyph bank",
    },
    PendingTextAssetSpec {
        finding_id: "BTXF-025",
        archive_paths: &P2_ARCHIVES,
        name: "MENU201.BTX",
        review_layout: Layout {
            width: 512,
            height: 256,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::StaticConsumerExact,
        expected_decompressed_sha256:
            "7f9741908762cd6a26d8fcc31f3d9f8551aa1e5bbdc790f3e149a9e9c690e3a1",
        observed_text: "ひみつ／きほんそうさ／ゲームオーバー와 일본어 glyph bank",
    },
    PendingTextAssetSpec {
        finding_id: "BTXF-026",
        archive_paths: &P2_ARCHIVES,
        name: "MENU202.BTX",
        review_layout: Layout {
            width: 768,
            height: 256,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::StaticConsumerExact,
        expected_decompressed_sha256:
            "004e32789dee11bc7e27fcee47a12e820edf1ffd341b37632d376fc7a16c18d4",
        observed_text: "ひとりで／ふたりで button label과 モードを選んでください",
    },
    PendingTextAssetSpec {
        finding_id: "BTXF-027",
        archive_paths: &P2_ARCHIVES,
        name: "MENU203.BTX",
        review_layout: Layout {
            width: 256,
            height: 256,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::StaticConsumerExact,
        expected_decompressed_sha256:
            "cf16ac48e9ab4d01fec3164491ab7897ce1ce54226e22c72bb5482d6e1051b80",
        observed_text: "ぷよぷよのルール／得点／連鎖",
    },
];

/// raster block catalog로 승격한 MENU01/MENU201/MENU202/MENU203의 정적 consumer provenance.
pub const BOX_PROMOTED_MENU_RASTER_PROVENANCE: [PendingTextAssetSpec<'static>; 4] = [
    BOX_MENU_STATIC_TEXT_PROVENANCE[0],
    BOX_MENU_STATIC_TEXT_PROVENANCE[2],
    BOX_MENU_STATIC_TEXT_PROVENANCE[3],
    BOX_MENU_STATIC_TEXT_PROVENANCE[4],
];

/// PP1/PP2의 새 일본어 양성 표면과 사본별 정적 setup 상충을 보존하는 provenance.
///
/// `MM_MES` 두 사본은 공통 baked-text surface지만 `NEXT` 상태 한 곳의
/// 픽셀이 다르므로 finding을 분리한다.
/// `P1_OVER3`는 PUYO1의 같은 frozen 화면·VRAM 표본으로 runtime-visible이며,
/// 같은 픽셀인 PUYO2 사본의 소비는 정적 setup까지만 확정한다.
/// 나머지 네 자산은 픽셀이 같지만, ending 세 자산은 PP2의 exact setup과
/// PP1의 256×176 setup이 상충하므로 전체 finding을 static exact로 승격하지
/// 않는다. `pipeline::btx_text_surface_cells`가 이 차이를 원본에서 검산한다.
pub const BOX_PUYO_TEXT_SURFACE_PROVENANCE: [PendingTextAssetSpec<'static>; 6] = [
    PendingTextAssetSpec {
        finding_id: "BTXF-029",
        archive_paths: &PUYO1_ARCHIVES,
        name: "MM_MES.BTX",
        review_layout: Layout {
            width: 512,
            height: 256,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::VisualCandidate,
        expected_decompressed_sha256:
            "b8c3fdabc55c3abe09938f53e6482e8d58ef972ef36c2c08cde8418ff9721f70",
        observed_text: "16개 가로 band에 이어진 rule·option baked-text surface",
    },
    PendingTextAssetSpec {
        finding_id: "BTXF-030",
        archive_paths: &PUYO2_ARCHIVES,
        name: "MM_MES.BTX",
        review_layout: Layout {
            width: 512,
            height: 256,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::VisualCandidate,
        expected_decompressed_sha256:
            "8b8ef104858a692f2f9a1127e7d9ac792b1aaa577a28d02cced1c6e67525044c",
        observed_text: "PP1과 공통인 baked-text surface와 NEXT 상태 변형",
    },
    PendingTextAssetSpec {
        finding_id: "BTXF-031",
        archive_paths: &P2_ARCHIVES,
        name: "P1_OVER3.BTX",
        review_layout: Layout {
            width: 256,
            height: 192,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::RuntimeVisibleWholeAssetExact,
        expected_decompressed_sha256:
            "6cd68f067a8c9494f84843fc30c77315e0f89078a66c456280a355d86a9a8a57",
        observed_text: "コンティニューするかい？와 0–10 숫자",
    },
    PendingTextAssetSpec {
        finding_id: "BTXF-032",
        archive_paths: &P2_ARCHIVES,
        name: "P2_END10.BTX",
        review_layout: Layout {
            width: 256,
            height: 160,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::StaticConsumerExact,
        expected_decompressed_sha256:
            "fe6a21b7e41e645d96ed6f4087688d5daa4d7e67d30f7f07d97d8ffa58019126",
        observed_text: "MOO仁井谷／じぇみに広野／秋山泰俊S／松島剛史／木戸福三郎 등 ending credit",
    },
    PendingTextAssetSpec {
        finding_id: "BTXF-033",
        archive_paths: &P2_ARCHIVES,
        name: "P2_END15.BTX",
        review_layout: Layout {
            width: 256,
            height: 224,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::RuntimeVisibleWholeAssetExact,
        expected_decompressed_sha256:
            "937d1cb1cf68ce74aa066b9e0e53ace465436f3ad40c4965c95f7a49ac545888",
        observed_text: "てっぺん／おしまい ending 그림 문구",
    },
    PendingTextAssetSpec {
        finding_id: "BTXF-034",
        archive_paths: &P2_ARCHIVES,
        name: "P2_END16.BTX",
        review_layout: Layout {
            width: 256,
            height: 256,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::RuntimeVisibleWholeAssetExact,
        expected_decompressed_sha256:
            "5368d0cf79735a4ccd5a5be3f59ac86a78442bc41b2a980879c45d2214a93a3b",
        observed_text: "めざせ頂上サタン様 조립 문구와 별도 탑 그림",
    },
];

/// raster block catalog로 승격한 `P1_OVER3`의 사본별 정적·runtime provenance.
pub const BOX_PROMOTED_P1_OVER3_PROVENANCE: [PendingTextAssetSpec<'static>; 1] =
    [BOX_PUYO_TEXT_SURFACE_PROVENANCE[2]];

/// raster block catalog로 승격한 ending 자산의 사본별 정적·runtime provenance.
pub const BOX_PROMOTED_ENDING_RASTER_PROVENANCE: [PendingTextAssetSpec<'static>; 3] = [
    BOX_PUYO_TEXT_SURFACE_PROVENANCE[3],
    BOX_PUYO_TEXT_SURFACE_PROVENANCE[4],
    BOX_PUYO_TEXT_SURFACE_PROVENANCE[5],
];

/// raster block catalog로 승격한 GOCHA1 결과 메시지의 정적 provenance.
pub const BOX_PROMOTED_G_MESS_PROVENANCE: [PendingTextAssetSpec<'static>; 1] =
    [PendingTextAssetSpec {
        finding_id: "BTXF-028",
        archive_paths: &GOCHA1_ARCHIVES,
        name: "G_MESS.BTX",
        review_layout: Layout {
            width: 512,
            height: 256,
            format: PixelFormat::Indexed4,
        },
        layout_basis: ReviewLayoutBasis::StaticConsumerExact,
        expected_decompressed_sha256:
            "e80411b0142228a8af7baf1166eb29412bdb03aca1afd49c0df3d015cb8d57da",
        observed_text: "がーんっ！／せっかくクリアしたのにこれだけ？／ヤッホー！ 등 2열 대사",
    }];

/// raster block catalog로 승격한 서브게임 mode 표지판의 runtime provenance.
///
/// GOCHA1에서 whole-asset residency와 cell 2 draw를 확인했다. 다른 네 사본은
/// 해제 픽셀과 layout이 같지만 각 EXE의 실행·upload·draw를 뜻하지 않는다.
pub const BOX_PROMOTED_KANBAN_PROVENANCE: [PendingTextAssetSpec<'static>; 1] =
    [PendingTextAssetSpec {
        finding_id: "BTXF-035",
        archive_paths: &SUBGAME_KANBAN_ARCHIVES,
        name: "KANBAN.BTX",
        review_layout: Layout {
            width: 200,
            height: 256,
            format: PixelFormat::Indexed8,
        },
        layout_basis: ReviewLayoutBasis::RuntimeVisibleWholeAssetExact,
        expected_decompressed_sha256:
            "7d220251fd0db49a62f7823330a9dcc9fec9dada86d40c9338af53267bff8587",
        observed_text: "とれーにんぐ／ほりあてぷよぷよ／ごちゃまぜぷよぷよ／ばくだんぷよぷよ通",
    }];

/// 시각 양성이지만 아직 block catalog로 승격하지 못한 비캐릭터 BTX.
pub const BOX_PENDING_TEXT_ASSETS: [PendingTextAssetSpec<'static>; 3] = [
    BOX_MENU_STATIC_TEXT_PROVENANCE[1],
    BOX_PUYO_TEXT_SURFACE_PROVENANCE[0],
    BOX_PUYO_TEXT_SURFACE_PROVENANCE[1],
];

#[cfg(test)]
#[path = "text_candidates_tests.rs"]
mod tests;
