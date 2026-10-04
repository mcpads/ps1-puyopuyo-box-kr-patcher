//! 미분류 고유 BTX의 전수 특징과 사람 검토용 family 후보.
//!
//! BTX에는 폭·높이가 없으므로 이 모듈의 `review_width`/`review_height`는
//! 소비 layout이 아니다. 모든 픽셀을 보존하는 8px 정렬 후보 중 256px에
//! 가장 가까운 폭을 골라 비교 특징만 계산한다. 자동 lane과 family 역시
//! 번역 대상 판정이 아니며, 대표 PNG/HITL 검토 순서를 정하는 후보다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha1::Sha1;
use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::{decompress, is_cnx, parse_header};

use super::btx_classification::BOX_BTX_CLASSIFICATION_SAMPLES;
use super::text_candidates::BOX_PENDING_TEXT_ASSETS;
use super::text_catalog::BOX_TEXT_ASSETS;
use crate::btx::PixelFormat;

#[path = "btx_candidate_features.rs"]
mod candidate_features;
#[path = "btx_candidate_reports.rs"]
mod candidate_reports;

use candidate_features::{
    candidate_lane, cell_features, decode_pixels, family_lane, normalized_name_stem,
    occupied_features, owner_scope, palette_histogram, review_layout,
};
pub use candidate_reports::{
    render_btx_candidate_families_tsv, render_btx_family_candidate_features_tsv,
};

pub const BTX_FAMILY_CANDIDATE_SCHEMA_VERSION: &str = "btx-family-candidates-v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BtxVisualPositiveVariantSpec {
    archive: &'static str,
    name: &'static str,
    format: PixelFormat,
    decompressed_sha256: &'static str,
}

/// 사람 판정으로 양성이지만 아직 block catalog로 승격하지 않은 변형.
///
/// `FIL` 사본과 내용이 다른 `RANK_S` 1종, `TITLEA` 2종, `MMENU` 2종이다.
/// family 후보 435개에서 제외하되 제품 번역 입력으로 자동 승격하지 않는다.
const BOX_VISUAL_POSITIVE_UNSPLIT_VARIANTS: [BtxVisualPositiveVariantSpec; 6] = [
    BtxVisualPositiveVariantSpec {
        archive: "/PUYO1/PP1_GRP.VDD;1",
        name: "RANK_S.BTX",
        format: PixelFormat::Indexed4,
        decompressed_sha256: "a2ba537b52ff08df57c45feb919d964689350b80086e40b5dcd89830129cea6a",
    },
    BtxVisualPositiveVariantSpec {
        archive: "/PUYO2/PP2_GRP.VDD;1",
        name: "RANK_S.BTX",
        format: PixelFormat::Indexed4,
        decompressed_sha256: "a2ba537b52ff08df57c45feb919d964689350b80086e40b5dcd89830129cea6a",
    },
    BtxVisualPositiveVariantSpec {
        archive: "/PUYO1/PP1_GRP.VDD;1",
        name: "TITLEA.BTX",
        format: PixelFormat::Indexed8,
        decompressed_sha256: "55b5bab905c06ef599069f27131ed09523e306ab34dd3c1c0a0f7674851cf0d6",
    },
    BtxVisualPositiveVariantSpec {
        archive: "/PUYO2/PP2_GRP.VDD;1",
        name: "TITLEA.BTX",
        format: PixelFormat::Indexed8,
        decompressed_sha256: "e38d3790acccc0c4aabbfd17517541f0e1a6588eb77c381295f8d22b21d248ae",
    },
    BtxVisualPositiveVariantSpec {
        archive: "/PUYO1/PP1_GRP.VDD;1",
        name: "MMENU.BTX",
        format: PixelFormat::Indexed8,
        decompressed_sha256: "0cff391ad50419079a26cda89f940b541581060d5f7410698205ff039a749604",
    },
    BtxVisualPositiveVariantSpec {
        archive: "/PUYO2/PP2_GRP.VDD;1",
        name: "MMENU.BTX",
        format: PixelFormat::Indexed8,
        decompressed_sha256: "fb431fd016b0e622e5d9c8bb8cfd229679ebddf2ed1d0b2f45ffdd3b56f2a571",
    },
];

/// 이름표와 descriptor에서 이미 검증한 물리 BTX의 정적 근거.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxConsumerEvidence<'a> {
    pub archive: &'a str,
    pub name: &'a str,
    pub static_evidence: &'a str,
    pub static_reference_sites: usize,
    pub descriptor_runtime: u32,
}

/// 자동 후보가 들어갈 사람 검토 lane. 확정 분류가 아니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BtxReviewLane {
    CharacterBackgroundEffectCandidate,
    UiFontTextCandidate,
    Ambiguous,
}

impl BtxReviewLane {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CharacterBackgroundEffectCandidate => "character-background-effect-candidate",
            Self::UiFontTextCandidate => "ui-font-text-candidate",
            Self::Ambiguous => "ambiguous",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BtxCandidateConfidence {
    High,
    Medium,
    Low,
}

impl BtxCandidateConfidence {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxPhysicalCandidateCopy {
    pub archive: String,
    pub name: String,
    pub resource_index: usize,
    pub static_evidence: String,
    pub static_reference_sites: usize,
    pub descriptor_runtime: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxOccupiedBox {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// 고유 해제 내용 하나의 source-derived 비교 특징.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyCandidate {
    pub candidate_id: String,
    pub family_id: String,
    pub family_key: String,
    pub review_lane: BtxReviewLane,
    pub confidence: BtxCandidateConfidence,
    pub candidate_reason: &'static str,
    pub decompressed_sha1: String,
    pub decompressed_sha256: String,
    pub format: PixelFormat,
    pub decompressed_bytes: usize,
    pub pixel_count: usize,
    pub dimension_candidates: usize,
    pub review_width: usize,
    pub review_height: usize,
    pub palette_histogram: Vec<usize>,
    pub candidate_background_index: u8,
    pub occupied_box: Option<BtxOccupiedBox>,
    pub horizontal_occupied_bands: usize,
    pub vertical_occupied_bands: usize,
    pub complete_16x16_cells: usize,
    pub unique_16x16_cells: usize,
    pub repeated_16x16_cells: usize,
    pub background_16x16_cells: usize,
    pub partial_16x16_edges: bool,
    pub physical_copies: Vec<BtxPhysicalCandidateCopy>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxCandidateFamily {
    pub family_id: String,
    pub family_key: String,
    pub review_lane: BtxReviewLane,
    pub confidence: BtxCandidateConfidence,
    pub members: usize,
    pub physical_copies: usize,
    pub representative_candidate_id: String,
    pub representative_archive: String,
    pub representative_name: String,
    pub representative_preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxUnclassifiedKnownContentAlias {
    pub archive: String,
    pub name: String,
    pub known_content_class: &'static str,
    pub decompressed_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyCandidateAnalysis {
    pub physical_btx: usize,
    pub unique_contents: usize,
    pub known_text_contents: usize,
    pub known_pending_contents: usize,
    pub known_visual_positive_contents: usize,
    pub known_nontext_contents: usize,
    pub declared_known_physical_btx: usize,
    pub known_content_physical_btx: usize,
    pub unclassified_known_content_aliases: Vec<BtxUnclassifiedKnownContentAlias>,
    pub candidates: Vec<BtxFamilyCandidate>,
    pub families: Vec<BtxCandidateFamily>,
    pub automatic_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyCandidateError(String);

impl fmt::Display for BtxFamilyCandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for BtxFamilyCandidateError {}

fn error(message: impl Into<String>) -> BtxFamilyCandidateError {
    BtxFamilyCandidateError(message.into())
}

fn format_label(format: PixelFormat) -> &'static str {
    match format {
        PixelFormat::Indexed4 => "indexed4",
        PixelFormat::Indexed8 => "indexed8",
    }
}

fn content_key(format: PixelFormat, sha256: &str) -> String {
    format!("{}:{sha256}", format_label(format))
}

fn known_content_sets() -> Result<[BTreeSet<String>; 4], BtxFamilyCandidateError> {
    let text = BOX_TEXT_ASSETS
        .iter()
        .map(|asset| content_key(asset.layout.format, asset.expected_decompressed_sha256))
        .collect::<BTreeSet<_>>();
    let pending = BOX_PENDING_TEXT_ASSETS
        .iter()
        .map(|asset| {
            content_key(
                asset.review_layout.format,
                asset.expected_decompressed_sha256,
            )
        })
        .collect::<BTreeSet<_>>();
    let nontext = BOX_BTX_CLASSIFICATION_SAMPLES
        .iter()
        .map(|asset| content_key(asset.layout.format, asset.expected_decompressed_sha256))
        .collect::<BTreeSet<_>>();
    let visual_positive = BOX_VISUAL_POSITIVE_UNSPLIT_VARIANTS
        .iter()
        .map(|asset| content_key(asset.format, asset.decompressed_sha256))
        .collect::<BTreeSet<_>>();
    for (left_name, left, right_name, right) in [
        ("text", &text, "pending", &pending),
        ("text", &text, "visual-positive", &visual_positive),
        ("text", &text, "nontext", &nontext),
        ("pending", &pending, "visual-positive", &visual_positive),
        ("pending", &pending, "nontext", &nontext),
        ("visual-positive", &visual_positive, "nontext", &nontext),
    ] {
        if let Some(key) = left.intersection(right).next() {
            return Err(error(format!(
                "BTX known content 경계 중복: {left_name}/{right_name} {key}"
            )));
        }
    }
    Ok([text, pending, visual_positive, nontext])
}

#[derive(Debug)]
struct UniqueContent {
    format: PixelFormat,
    raw: Vec<u8>,
    sha1: String,
    sha256: String,
    copies: Vec<(String, String, usize)>,
}

/// 지원 원본에서 현재 미분류 고유 BTX의 비교 특징과 family를 만든다.
pub fn verify_box_btx_family_candidates<'a>(
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
    evidence: impl IntoIterator<Item = BtxConsumerEvidence<'a>>,
) -> Result<BtxFamilyCandidateAnalysis, BtxFamilyCandidateError> {
    let [known_text, known_pending, known_visual_positive, known_nontext] = known_content_sets()?;
    let mut evidence_by_asset = BTreeMap::new();
    for row in evidence {
        if row.archive.is_empty() || row.name.is_empty() || row.static_evidence.is_empty() {
            return Err(error("빈 BTX consumer evidence 필드"));
        }
        if evidence_by_asset
            .insert((row.archive.to_string(), row.name.to_string()), row)
            .is_some()
        {
            return Err(error(format!(
                "BTX consumer evidence 중복: {}:{}",
                row.archive, row.name
            )));
        }
    }

    let mut physical_btx = 0usize;
    let mut unique = BTreeMap::<String, UniqueContent>::new();
    let mut seen_assets = BTreeSet::new();
    let mut asset_content_keys = BTreeMap::new();
    for (archive_id, archive) in archives {
        for (resource_index, entry) in archive.entries().iter().enumerate() {
            if !entry.name().to_ascii_uppercase().ends_with(".BTX") {
                continue;
            }
            let compressed = archive.read_file(entry);
            if !is_cnx(compressed) {
                return Err(error(format!(
                    "{archive_id}:{}: BTX가 CNX v2가 아님",
                    entry.name()
                )));
            }
            let header = parse_header(compressed)
                .map_err(|source| error(format!("{archive_id}:{}: {source}", entry.name())))?;
            let format = PixelFormat::from_cnx_subtype(header.subtype)
                .map_err(|source| error(format!("{archive_id}:{}: {source}", entry.name())))?;
            let raw = decompress(compressed)
                .map_err(|source| error(format!("{archive_id}:{}: {source}", entry.name())))?;
            let sha1 = format!("{:x}", Sha1::digest(&raw));
            let sha256 = format!("{:x}", Sha256::digest(&raw));
            let key = content_key(format, &sha256);
            asset_content_keys.insert(
                (archive_id.to_string(), entry.name().to_string()),
                key.clone(),
            );
            unique
                .entry(key)
                .and_modify(|content| {
                    content.copies.push((
                        archive_id.to_string(),
                        entry.name().to_string(),
                        resource_index,
                    ));
                })
                .or_insert_with(|| UniqueContent {
                    format,
                    raw,
                    sha1,
                    sha256,
                    copies: vec![(
                        archive_id.to_string(),
                        entry.name().to_string(),
                        resource_index,
                    )],
                });
            seen_assets.insert((archive_id.to_string(), entry.name().to_string()));
            physical_btx += 1;
        }
    }
    if seen_assets != evidence_by_asset.keys().cloned().collect::<BTreeSet<_>>() {
        return Err(error(format!(
            "BTX physical asset와 consumer evidence 분모 불일치: assets={}, evidence={}",
            seen_assets.len(),
            evidence_by_asset.len()
        )));
    }
    for spec in BOX_VISUAL_POSITIVE_UNSPLIT_VARIANTS {
        let expected = content_key(spec.format, spec.decompressed_sha256);
        let actual = asset_content_keys
            .get(&(spec.archive.to_string(), spec.name.to_string()))
            .ok_or_else(|| {
                error(format!(
                    "시각 양성·미분할 BTX가 지원 원본에 없음: {}:{}",
                    spec.archive, spec.name
                ))
            })?;
        if actual != &expected {
            return Err(error(format!(
                "시각 양성·미분할 BTX 원문 불일치: {}:{} expected={}, actual={actual}",
                spec.archive, spec.name, expected
            )));
        }
    }
    for (label, known) in [
        ("text", &known_text),
        ("pending", &known_pending),
        ("visual-positive", &known_visual_positive),
        ("nontext", &known_nontext),
    ] {
        if let Some(missing) = known.iter().find(|key| !unique.contains_key(*key)) {
            return Err(error(format!(
                "지원 원본에 {label} known BTX content가 없음: {missing}"
            )));
        }
    }

    let known_keys = known_text
        .iter()
        .chain(&known_pending)
        .chain(&known_visual_positive)
        .chain(&known_nontext)
        .cloned()
        .collect::<BTreeSet<_>>();
    let known_content_physical_btx = unique
        .iter()
        .filter(|(key, _)| known_keys.contains(*key))
        .map(|(_, content)| content.copies.len())
        .sum();
    let declared_known_assets = BOX_TEXT_ASSETS
        .iter()
        .flat_map(|asset| {
            asset
                .archive_paths
                .iter()
                .map(move |archive| ((*archive).to_string(), asset.name.to_string()))
        })
        .chain(BOX_PENDING_TEXT_ASSETS.iter().flat_map(|asset| {
            asset
                .archive_paths
                .iter()
                .map(move |archive| ((*archive).to_string(), asset.name.to_string()))
        }))
        .chain(
            BOX_VISUAL_POSITIVE_UNSPLIT_VARIANTS
                .iter()
                .map(|asset| (asset.archive.to_string(), asset.name.to_string())),
        )
        .chain(BOX_BTX_CLASSIFICATION_SAMPLES.iter().flat_map(|asset| {
            asset
                .archive_paths
                .iter()
                .map(move |archive| ((*archive).to_string(), asset.name.to_string()))
        }))
        .collect::<BTreeSet<_>>();
    if !declared_known_assets.is_subset(&seen_assets) {
        return Err(error(format!(
            "선언된 known BTX가 지원 원본의 물리 자산에 없음: declared={}, source_subset={}",
            declared_known_assets.len(),
            declared_known_assets.is_subset(&seen_assets)
        )));
    }
    let unclassified_known_content_aliases = asset_content_keys
        .iter()
        .filter(|(asset, key)| known_keys.contains(*key) && !declared_known_assets.contains(*asset))
        .map(|((archive, name), key)| {
            let known_content_class = if known_text.contains(key) {
                "catalog-text-content"
            } else if known_pending.contains(key) {
                "pending-text-content"
            } else if known_visual_positive.contains(key) {
                "visual-positive-content"
            } else {
                "classified-nontext-content"
            };
            BtxUnclassifiedKnownContentAlias {
                archive: archive.clone(),
                name: name.clone(),
                known_content_class,
                decompressed_sha256: key
                    .split_once(':')
                    .map(|(_, digest)| digest.to_string())
                    .expect("검증된 content key"),
            }
        })
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    for (key, content) in unique.iter().filter(|(key, _)| !known_keys.contains(*key)) {
        let pixels = decode_pixels(&content.raw, content.format);
        let (review_width, review_height, dimension_candidates) = review_layout(pixels.len())?;
        let histogram = palette_histogram(&pixels, content.format);
        let candidate_background_index = histogram
            .iter()
            .enumerate()
            .max_by_key(|(index, count)| (**count, std::cmp::Reverse(*index)))
            .map(|(index, _)| index as u8)
            .ok_or_else(|| error(format!("{key}: palette histogram이 비어 있음")))?;
        let (occupied_box, horizontal_bands, vertical_bands) = occupied_features(
            &pixels,
            review_width,
            review_height,
            candidate_background_index,
        );
        let (cells, unique_cells, repeated_cells, background_cells, partial_edges) = cell_features(
            &pixels,
            review_width,
            review_height,
            candidate_background_index,
        );
        let mut copies = content
            .copies
            .iter()
            .map(|(archive, name, resource_index)| {
                let static_row = evidence_by_asset
                    .get(&(archive.clone(), name.clone()))
                    .ok_or_else(|| {
                        error(format!("BTX consumer evidence 없음: {archive}:{name}"))
                    })?;
                Ok(BtxPhysicalCandidateCopy {
                    archive: archive.clone(),
                    name: name.clone(),
                    resource_index: *resource_index,
                    static_evidence: static_row.static_evidence.to_string(),
                    static_reference_sites: static_row.static_reference_sites,
                    descriptor_runtime: static_row.descriptor_runtime,
                })
            })
            .collect::<Result<Vec<_>, BtxFamilyCandidateError>>()?;
        copies
            .sort_by(|left, right| (&left.archive, &left.name).cmp(&(&right.archive, &right.name)));
        let representative = copies
            .first()
            .ok_or_else(|| error(format!("{key}: 물리 BTX 사본이 없음")))?;
        let stem = normalized_name_stem(&representative.name);
        let family_key = format!(
            "{}:{}:{}",
            owner_scope(&representative.archive),
            stem,
            format_label(content.format)
        );
        let (review_lane, confidence, candidate_reason) = candidate_lane(
            &stem,
            content.format,
            content.raw.len(),
            review_width,
            review_height,
        );
        candidates.push(BtxFamilyCandidate {
            candidate_id: String::new(),
            family_id: String::new(),
            family_key,
            review_lane,
            confidence,
            candidate_reason,
            decompressed_sha1: content.sha1.clone(),
            decompressed_sha256: content.sha256.clone(),
            format: content.format,
            decompressed_bytes: content.raw.len(),
            pixel_count: pixels.len(),
            dimension_candidates,
            review_width,
            review_height,
            palette_histogram: histogram,
            candidate_background_index,
            occupied_box,
            horizontal_occupied_bands: horizontal_bands,
            vertical_occupied_bands: vertical_bands,
            complete_16x16_cells: cells,
            unique_16x16_cells: unique_cells,
            repeated_16x16_cells: repeated_cells,
            background_16x16_cells: background_cells,
            partial_16x16_edges: partial_edges,
            physical_copies: copies,
        });
    }
    candidates.sort_by(|left, right| {
        let left_copy = &left.physical_copies[0];
        let right_copy = &right.physical_copies[0];
        (
            &left.family_key,
            &left_copy.archive,
            &left_copy.name,
            &left.decompressed_sha256,
        )
            .cmp(&(
                &right.family_key,
                &right_copy.archive,
                &right_copy.name,
                &right.decompressed_sha256,
            ))
    });

    let family_keys = candidates
        .iter()
        .map(|candidate| candidate.family_key.clone())
        .collect::<BTreeSet<_>>();
    let family_ids = family_keys
        .iter()
        .enumerate()
        .map(|(index, key)| (key.clone(), format!("BTXFAM-{:04}", index + 1)))
        .collect::<BTreeMap<_, _>>();
    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.candidate_id = format!("BTXU-{:04}", index + 1);
        candidate.family_id = family_ids
            .get(&candidate.family_key)
            .expect("검증된 family key")
            .clone();
    }

    let mut families = Vec::new();
    for (family_key, family_id) in family_ids {
        let members = candidates
            .iter()
            .filter(|candidate| candidate.family_key == family_key)
            .collect::<Vec<_>>();
        let (review_lane, confidence) = family_lane(&members);
        let representative = members
            .iter()
            .copied()
            .max_by_key(|candidate| {
                (
                    candidate
                        .physical_copies
                        .iter()
                        .map(|copy| copy.static_reference_sites)
                        .sum::<usize>(),
                    candidate.physical_copies.len(),
                    std::cmp::Reverse(candidate.candidate_id.as_str()),
                )
            })
            .expect("한 개 이상의 family member");
        let representative_copy = &representative.physical_copies[0];
        families.push(BtxCandidateFamily {
            family_id,
            family_key,
            review_lane,
            confidence,
            members: members.len(),
            physical_copies: members
                .iter()
                .map(|member| member.physical_copies.len())
                .sum(),
            representative_candidate_id: representative.candidate_id.clone(),
            representative_archive: representative_copy.archive.clone(),
            representative_name: representative_copy.name.clone(),
            representative_preview: format!(
                "previews/{}-{}.png",
                format_label(representative.format),
                representative.decompressed_sha1
            ),
        });
    }

    if candidates.len() + known_keys.len() != unique.len() {
        return Err(error(format!(
            "BTX 고유 content 분할 불일치: candidates={}, unique={}, known={}",
            candidates.len(),
            unique.len(),
            known_keys.len()
        )));
    }
    let candidate_physical_btx = candidates
        .iter()
        .map(|candidate| candidate.physical_copies.len())
        .sum::<usize>();
    if candidate_physical_btx + known_content_physical_btx != physical_btx
        || (!candidates.is_empty() && families.is_empty())
    {
        return Err(error(format!(
            "BTX physical/family 분할 불일치: physical={}, candidates={}, known={}, \
             known-content-aliases={}, families={}",
            physical_btx,
            candidate_physical_btx,
            known_content_physical_btx,
            unclassified_known_content_aliases.len(),
            families.len(),
        )));
    }
    Ok(BtxFamilyCandidateAnalysis {
        physical_btx,
        unique_contents: unique.len(),
        known_text_contents: known_text.len(),
        known_pending_contents: known_pending.len(),
        known_visual_positive_contents: known_visual_positive.len(),
        known_nontext_contents: known_nontext.len(),
        declared_known_physical_btx: declared_known_assets.len(),
        known_content_physical_btx,
        unclassified_known_content_aliases,
        candidates,
        families,
        automatic_approval: false,
    })
}

#[cfg(test)]
#[path = "btx_family_candidates_tests.rs"]
mod tests;
