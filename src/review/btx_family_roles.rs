//! 사람 관측이 양성인 BTX family를 기술 역할과 근거 수준에 연결한다.
//!
//! `family_id`는 전체 후보 정렬에 따라 바뀔 수 있으므로 입력 계약으로 쓰지
//! 않는다. 역할 사양은 의미가 안정적인 `family_key`에 결속하고, 현재
//! source-derived ID와 구성원 수는 감사할 때 다시 계산한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::btx::{Layout, PixelFormat};
use crate::graphics::btx_family_candidates::{
    BtxCandidateFamily, BtxFamilyCandidate, BtxFamilyCandidateAnalysis,
};

use super::{BtxFamilyReviewAnalysis, BtxFamilyReviewDecision, BtxFamilyReviewDecisionKind};

/// 사람이 일본어 픽셀을 관측한 family의 자산 역할.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtxFamilyAssetRole {
    GlyphAtlas,
    MixedGlyphUiAtlas,
    UiSurface,
}

impl BtxFamilyAssetRole {
    pub const fn label(self) -> &'static str {
        match self {
            Self::GlyphAtlas => "glyph-atlas",
            Self::MixedGlyphUiAtlas => "mixed-glyph-ui-atlas",
            Self::UiSurface => "ui-surface",
        }
    }
}

/// 역할 판정이 기대는 가장 강한 현재 근거.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtxFamilyRoleBasis {
    RuntimeConsumerConfirmed,
    SourceStructureConfirmed,
    HumanVisualCandidate,
}

impl BtxFamilyRoleBasis {
    pub const fn label(self) -> &'static str {
        match self {
            Self::RuntimeConsumerConfirmed => "runtime-consumer-confirmed",
            Self::SourceStructureConfirmed => "source-structure-confirmed",
            Self::HumanVisualCandidate => "human-visual-candidate",
        }
    }
}

/// 안정적인 family key에 결속한 역할 사양.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxFamilyRoleSpec {
    pub family_key: &'static str,
    pub role: BtxFamilyAssetRole,
    pub basis: BtxFamilyRoleBasis,
    /// `HumanVisualCandidate`에서는 소비 layout이 아니라 판독용 후보다.
    pub common_layout: Option<Layout>,
}

/// 현재 source-derived family와 역할 사양을 결합한 행.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyRoleRow {
    pub family_id: String,
    pub family_key: String,
    pub role: BtxFamilyAssetRole,
    pub basis: BtxFamilyRoleBasis,
    pub common_layout: Option<Layout>,
    pub unique_contents: usize,
    pub physical_copies: usize,
}

/// 모든 양성 family 역할을 빠짐없이 결속한 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyRoleAnalysis {
    pub rows: Vec<BtxFamilyRoleRow>,
    pub families: usize,
    pub unique_contents: usize,
    pub physical_copies: usize,
    pub runtime_consumer_confirmed: usize,
    pub source_structure_confirmed: usize,
    pub human_visual_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyRoleError(String);

impl fmt::Display for BtxFamilyRoleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "BTX family 역할 감사 실패: {}", self.0)
    }
}

impl Error for BtxFamilyRoleError {}

fn error(message: impl Into<String>) -> BtxFamilyRoleError {
    BtxFamilyRoleError(message.into())
}

fn family_members<'a>(
    family: &BtxCandidateFamily,
    candidates: &'a [BtxFamilyCandidate],
) -> Result<Vec<&'a BtxFamilyCandidate>, BtxFamilyRoleError> {
    let members = candidates
        .iter()
        .filter(|candidate| candidate.family_key == family.family_key)
        .collect::<Vec<_>>();
    if members.len() != family.members {
        return Err(error(format!(
            "{} 구성원 수 불일치: family={}, candidates={}",
            family.family_key,
            family.members,
            members.len()
        )));
    }
    if members
        .iter()
        .any(|candidate| candidate.family_id != family.family_id)
    {
        return Err(error(format!(
            "{} 구성원의 현재 family ID 불일치",
            family.family_key
        )));
    }
    let physical_copies = members
        .iter()
        .map(|candidate| candidate.physical_copies.len())
        .sum::<usize>();
    if physical_copies != family.physical_copies {
        return Err(error(format!(
            "{} 물리 사본 수 불일치: family={}, candidates={physical_copies}",
            family.family_key, family.physical_copies
        )));
    }
    Ok(members)
}

fn verify_common_layout(
    spec: BtxFamilyRoleSpec,
    members: &[&BtxFamilyCandidate],
) -> Result<(), BtxFamilyRoleError> {
    let Some(layout) = spec.common_layout else {
        return Ok(());
    };
    let expected_bytes = layout
        .byte_len()
        .map_err(|source| error(format!("{} layout 오류: {source}", spec.family_key)))?;
    for member in members {
        if member.format != layout.format
            || member.pixel_count != layout.width * layout.height
            || member.decompressed_bytes != expected_bytes
        {
            return Err(error(format!(
                "{} common layout이 {} 원문과 맞지 않음: {:?} {}x{}, pixels={}, bytes={}",
                spec.family_key,
                member.candidate_id,
                layout.format,
                layout.width,
                layout.height,
                member.pixel_count,
                member.decompressed_bytes
            )));
        }
    }
    Ok(())
}

/// 사람 양성 판정과 역할 사양이 정확히 일치하는지 감사한다.
pub fn audit_btx_family_roles(
    candidates: &BtxFamilyCandidateAnalysis,
    decisions: &[BtxFamilyReviewDecision],
    specs: &[BtxFamilyRoleSpec],
) -> Result<BtxFamilyRoleAnalysis, BtxFamilyRoleError> {
    if specs.is_empty() {
        return Err(error("역할 사양이 비어 있음"));
    }

    let mut families_by_key = BTreeMap::new();
    for family in &candidates.families {
        if families_by_key
            .insert(family.family_key.as_str(), family)
            .is_some()
        {
            return Err(error(format!(
                "source-derived family key 중복: {}",
                family.family_key
            )));
        }
    }
    let decisions_by_key = decisions
        .iter()
        .map(|decision| (decision.family_key.as_str(), decision))
        .collect::<BTreeMap<_, _>>();
    if decisions_by_key.len() != decisions.len() {
        return Err(error("사람 관측 family key 중복"));
    }

    let positive_keys = decisions
        .iter()
        .filter(|decision| decision.decision == BtxFamilyReviewDecisionKind::JapanesePixelsObserved)
        .map(|decision| decision.family_key.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen_keys = BTreeSet::new();
    let mut routed_positive_keys = BTreeSet::new();
    let mut rows = Vec::with_capacity(specs.len());
    for spec in specs {
        if !seen_keys.insert(spec.family_key) {
            return Err(error(format!(
                "역할 사양 family key 중복: {}",
                spec.family_key
            )));
        }
        let family = families_by_key.get(spec.family_key).ok_or_else(|| {
            error(format!(
                "역할 사양의 현재 family가 없음: {}",
                spec.family_key
            ))
        })?;
        let decision = decisions_by_key
            .get(family.family_key.as_str())
            .ok_or_else(|| {
                error(format!(
                    "{} 현재 family의 사람 관측이 없음: {}",
                    spec.family_key, family.family_id
                ))
            })?;
        if decision.decision != BtxFamilyReviewDecisionKind::JapanesePixelsObserved {
            return Err(error(format!(
                "{} 역할 사양이 일본어 픽셀 양성 family에 결속되지 않음: {}",
                spec.family_key,
                decision.decision.label()
            )));
        }
        let members = family_members(family, &candidates.candidates)?;
        verify_common_layout(*spec, &members)?;
        routed_positive_keys.insert(family.family_key.as_str());
        rows.push(BtxFamilyRoleRow {
            family_id: family.family_id.clone(),
            family_key: family.family_key.clone(),
            role: spec.role,
            basis: spec.basis,
            common_layout: spec.common_layout,
            unique_contents: family.members,
            physical_copies: family.physical_copies,
        });
    }

    if routed_positive_keys != positive_keys {
        let missing = positive_keys
            .difference(&routed_positive_keys)
            .copied()
            .collect::<Vec<_>>();
        return Err(error(format!(
            "사람 양성 family의 역할 사양이 누락됨: {}",
            missing.join(",")
        )));
    }

    rows.sort_by(|left, right| left.family_key.cmp(&right.family_key));
    let runtime_consumer_confirmed = rows
        .iter()
        .filter(|row| row.basis == BtxFamilyRoleBasis::RuntimeConsumerConfirmed)
        .count();
    let source_structure_confirmed = rows
        .iter()
        .filter(|row| row.basis == BtxFamilyRoleBasis::SourceStructureConfirmed)
        .count();
    let human_visual_candidates = rows
        .iter()
        .filter(|row| row.basis == BtxFamilyRoleBasis::HumanVisualCandidate)
        .count();
    Ok(BtxFamilyRoleAnalysis {
        families: rows.len(),
        unique_contents: rows.iter().map(|row| row.unique_contents).sum(),
        physical_copies: rows.iter().map(|row| row.physical_copies).sum(),
        runtime_consumer_confirmed,
        source_structure_confirmed,
        human_visual_candidates,
        rows,
    })
}

pub const BOX_BTX_FAMILY_ROLE_SPECS: &[BtxFamilyRoleSpec] = &[
    BtxFamilyRoleSpec {
        family_key: "classic-puyo:COM_FONT:indexed4",
        role: BtxFamilyAssetRole::MixedGlyphUiAtlas,
        basis: BtxFamilyRoleBasis::SourceStructureConfirmed,
        common_layout: Some(Layout {
            width: 256,
            height: 128,
            format: PixelFormat::Indexed4,
        }),
    },
    BtxFamilyRoleSpec {
        family_key: "classic-puyo:DEMOFONT:indexed4",
        role: BtxFamilyAssetRole::GlyphAtlas,
        basis: BtxFamilyRoleBasis::RuntimeConsumerConfirmed,
        common_layout: Some(Layout {
            width: 256,
            height: 512,
            format: PixelFormat::Indexed4,
        }),
    },
    BtxFamilyRoleSpec {
        family_key: "quest:COM_FONT:indexed4",
        role: BtxFamilyAssetRole::MixedGlyphUiAtlas,
        basis: BtxFamilyRoleBasis::SourceStructureConfirmed,
        common_layout: Some(Layout {
            width: 256,
            height: 128,
            format: PixelFormat::Indexed4,
        }),
    },
    BtxFamilyRoleSpec {
        family_key: "quest:RPG_FNT:indexed4",
        role: BtxFamilyAssetRole::GlyphAtlas,
        basis: BtxFamilyRoleBasis::RuntimeConsumerConfirmed,
        common_layout: Some(Layout {
            width: 256,
            height: 256,
            format: PixelFormat::Indexed4,
        }),
    },
    BtxFamilyRoleSpec {
        family_key: "subgame:COM_FONT:indexed4",
        role: BtxFamilyAssetRole::MixedGlyphUiAtlas,
        basis: BtxFamilyRoleBasis::SourceStructureConfirmed,
        common_layout: None,
    },
    BtxFamilyRoleSpec {
        family_key: "subgame:SP_FONT:indexed4",
        role: BtxFamilyAssetRole::MixedGlyphUiAtlas,
        basis: BtxFamilyRoleBasis::HumanVisualCandidate,
        common_layout: Some(Layout {
            width: 256,
            height: 208,
            format: PixelFormat::Indexed4,
        }),
    },
];

/// 지원 원본의 사람 양성 family 역할을 검증한다.
pub fn verify_box_btx_family_roles(
    candidates: &BtxFamilyCandidateAnalysis,
    review: &BtxFamilyReviewAnalysis,
) -> Result<BtxFamilyRoleAnalysis, BtxFamilyRoleError> {
    audit_btx_family_roles(candidates, &review.decisions, BOX_BTX_FAMILY_ROLE_SPECS)
}

#[cfg(test)]
#[path = "btx_family_roles_tests.rs"]
mod tests;
