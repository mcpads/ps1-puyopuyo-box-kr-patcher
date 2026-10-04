//! 일반 raster BTX의 top-down semantic family 제안.
//!
//! 화면 역할이 비슷한 자산을 함께 검토하기 위한 version 관리 제안일 뿐,
//! font·effect profile이나 제품 번역 입력을 자동 승인하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

pub static BOX_BTX_RASTER_FAMILY_PROPOSALS: &[BtxRasterFamilyProposal] = &[
    BtxRasterFamilyProposal {
        id: "shell-mode-navigation",
        purpose: "셸 mode 선택과 주메뉴 navigation",
        asset_names: &["BOXCUBE.BTX", "BOXPARTS.BTX", "MMENU.BTX"],
    },
    BtxRasterFamilyProposal {
        id: "shell-title-branding",
        purpose: "셸 title과 branding",
        asset_names: &["TITLEA.BTX"],
    },
    BtxRasterFamilyProposal {
        id: "shell-options",
        purpose: "셸 option label과 설명",
        asset_names: &["MOPT.BTX"],
    },
    BtxRasterFamilyProposal {
        id: "shell-record-ranking",
        purpose: "memory card와 ranking UI",
        asset_names: &["MCRD01.BTX", "RANK_S.BTX"],
    },
    BtxRasterFamilyProposal {
        id: "puyo-mode-menu",
        purpose: "classic Puyo mode menu와 선택 안내",
        asset_names: &["MENU01.BTX", "MENU201.BTX", "MENU202.BTX", "MENU203.BTX"],
    },
    BtxRasterFamilyProposal {
        id: "puyo-help-rules",
        purpose: "classic Puyo 도움말과 rule 설명",
        asset_names: &["P2_INFO1.BTX", "P2_INFO3.BTX", "P2RULED1.BTX"],
    },
    BtxRasterFamilyProposal {
        id: "puyo-status-branding",
        purpose: "subgame 상태 label과 opening logo",
        asset_names: &[
            "BATAN2.BTX",
            "P2_OP1.BTX",
            "P2_OP13.BTX",
            "PCOMI01.BTX",
            "PCOMI02.BTX",
            "RPG_SYS1.BTX",
        ],
    },
    BtxRasterFamilyProposal {
        id: "puyo-ending-message",
        purpose: "classic Puyo ending prompt와 banner",
        asset_names: &["P1_OVER3.BTX", "P2_END15.BTX", "P2_END16.BTX"],
    },
    BtxRasterFamilyProposal {
        id: "puyo-ending-credits",
        purpose: "classic Puyo ending credit 인명",
        asset_names: &["P2_END10.BTX"],
    },
    BtxRasterFamilyProposal {
        id: "gocha-result-messages",
        purpose: "GOCHA 결과 화면 문장",
        asset_names: &["G_MESS.BTX"],
    },
    BtxRasterFamilyProposal {
        id: "subgame-rule-options",
        purpose: "서브게임 대전 규칙과 option label",
        asset_names: &["BOXOPT.BTX"],
    },
    BtxRasterFamilyProposal {
        id: "subgame-mode-sign",
        purpose: "서브게임 mode 선택 표지판 surface",
        asset_names: &["KANBAN.BTX"],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxRasterFamilyProposal {
    pub id: &'static str,
    pub purpose: &'static str,
    pub asset_names: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxRasterFamilyProposalReport {
    pub families: usize,
    pub assets: usize,
    pub blocks: usize,
    pub automatic_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxRasterFamilyProposalError(String);

impl fmt::Display for BtxRasterFamilyProposalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for BtxRasterFamilyProposalError {}

fn error(message: impl Into<String>) -> BtxRasterFamilyProposalError {
    BtxRasterFamilyProposalError(message.into())
}

pub fn proposed_family_for_asset(
    asset_name: &str,
) -> Result<&'static BtxRasterFamilyProposal, BtxRasterFamilyProposalError> {
    let mut matches = BOX_BTX_RASTER_FAMILY_PROPOSALS
        .iter()
        .filter(|proposal| proposal.asset_names.contains(&asset_name));
    let proposal = matches
        .next()
        .ok_or_else(|| error(format!("semantic family가 없는 raster BTX: {asset_name}")))?;
    if matches.next().is_some() {
        return Err(error(format!(
            "여러 semantic family에 중복된 raster BTX: {asset_name}"
        )));
    }
    Ok(proposal)
}

/// 현재 member가 family 제안에 정확히 한 번씩 들어가는지 검증한다.
pub fn verify_btx_raster_family_proposals<'a>(
    members: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<BtxRasterFamilyProposalReport, BtxRasterFamilyProposalError> {
    let mut family_ids = BTreeSet::new();
    let mut declared_assets = BTreeSet::new();
    for proposal in BOX_BTX_RASTER_FAMILY_PROPOSALS {
        if proposal.id.is_empty() || proposal.purpose.is_empty() || proposal.asset_names.is_empty()
        {
            return Err(error("비어 있는 BTX semantic family 제안이 있음"));
        }
        if !family_ids.insert(proposal.id) {
            return Err(error(format!(
                "BTX semantic family ID 중복: {}",
                proposal.id
            )));
        }
        for asset_name in proposal.asset_names {
            if !declared_assets.insert(*asset_name) {
                return Err(error(format!(
                    "BTX semantic family 자산 중복: {asset_name}"
                )));
            }
        }
    }

    let mut member_ids = BTreeSet::new();
    let mut member_assets = BTreeSet::new();
    let mut family_blocks = BTreeMap::<&str, usize>::new();
    for (asset_name, block_id) in members {
        if !member_ids.insert(block_id) {
            return Err(error(format!(
                "raster family member block 중복: {block_id}"
            )));
        }
        member_assets.insert(asset_name);
        let proposal = proposed_family_for_asset(asset_name)?;
        let count = family_blocks.entry(proposal.id).or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| error(format!("{} member 합계 overflow", proposal.id)))?;
    }

    if declared_assets != member_assets
        || family_blocks.len() != BOX_BTX_RASTER_FAMILY_PROPOSALS.len()
    {
        return Err(error(format!(
            "BTX semantic family 전체 분할 불일치: declared/member assets={}/{}, \
             declared/member families={}/{}, member blocks={}",
            declared_assets.len(),
            member_assets.len(),
            BOX_BTX_RASTER_FAMILY_PROPOSALS.len(),
            family_blocks.len(),
            member_ids.len()
        )));
    }

    Ok(BtxRasterFamilyProposalReport {
        families: BOX_BTX_RASTER_FAMILY_PROPOSALS.len(),
        assets: declared_assets.len(),
        blocks: member_ids.len(),
        automatic_approval: false,
    })
}

#[cfg(test)]
#[path = "btx_raster_family_tests.rs"]
mod tests;
