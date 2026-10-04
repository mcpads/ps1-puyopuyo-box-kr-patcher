//! QUEST glyph atlas의 논리 page와 물리 bank 선택 규칙.
//!
//! hook assembly와 별도로 선택 의미를 고정한다. marker의 하위 7비트는
//! 논리 page ID, 상위 비트는 두 번째 물리 bank를 뜻한다. 이 모듈은 VRAM
//! 소유권이나 실제 실행을 승인하지 않는다.

use std::error::Error;
use std::fmt;

pub const QUEST_GLYPH_ATLAS_BANK_COUNT: usize = 2;
pub const QUEST_GLYPH_ATLAS_SECONDARY_MARKER_BIT: u8 = 0x80;
pub const QUEST_GLYPH_ATLAS_LOGICAL_PAGE_MASK: u8 = 0x7F;

/// 두 물리 atlas bank 중 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestGlyphAtlasBank {
    Primary,
    Secondary,
}

impl QuestGlyphAtlasBank {
    pub const fn index(self) -> usize {
        match self {
            Self::Primary => 0,
            Self::Secondary => 1,
        }
    }

    pub const fn state_offset(self) -> i16 {
        match self {
            Self::Primary => 0,
            Self::Secondary => 3,
        }
    }
}

/// 한 hook 진입에서 관측한 bank residency와 live object pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestGlyphAtlasBankState {
    pub logical_pages: [u8; QUEST_GLYPH_ATLAS_BANK_COUNT],
    pub pinned: [bool; QUEST_GLYPH_ATLAS_BANK_COUNT],
}

/// 다음 OP21 page를 처리하는 선택 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestGlyphAtlasBankSelection {
    Original,
    Resident {
        bank: QuestGlyphAtlasBank,
        marker: u8,
    },
    Upload {
        bank: QuestGlyphAtlasBank,
        marker: u8,
    },
    WaitAllBanksPinned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestGlyphAtlasBankPolicyError {
    page: u8,
}

impl fmt::Display for QuestGlyphAtlasBankPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "QUEST glyph atlas 논리 page가 marker 범위를 넘음: {}",
            self.page
        )
    }
}

impl Error for QuestGlyphAtlasBankPolicyError {}

/// 논리 page와 물리 bank를 object marker 한 바이트로 결합한다.
pub fn encode_quest_glyph_atlas_marker(
    page: u8,
    bank: QuestGlyphAtlasBank,
) -> Result<u8, QuestGlyphAtlasBankPolicyError> {
    if page == 0 || page > QUEST_GLYPH_ATLAS_LOGICAL_PAGE_MASK {
        return Err(QuestGlyphAtlasBankPolicyError { page });
    }
    Ok(match bank {
        QuestGlyphAtlasBank::Primary => page,
        QuestGlyphAtlasBank::Secondary => page | QUEST_GLYPH_ATLAS_SECONDARY_MARKER_BIT,
    })
}

/// marker가 가리키는 물리 bank를 반환한다. 0은 원본 route다.
pub const fn quest_glyph_atlas_bank_from_marker(marker: u8) -> Option<QuestGlyphAtlasBank> {
    if marker == 0 {
        None
    } else if marker & QUEST_GLYPH_ATLAS_SECONDARY_MARKER_BIT == 0 {
        Some(QuestGlyphAtlasBank::Primary)
    } else {
        Some(QuestGlyphAtlasBank::Secondary)
    }
}

/// 현재 residency와 live object pin에서 요청 page의 bank를 고른다.
///
/// page state만 남고 해당 bank를 가리키는 live object가 없으면 장면 전환 중
/// VRAM이 다른 자산에 덮였을 수 있다. 이 경우 resident hit로 보지 않고 같은
/// bank에 page를 다시 올린다.
pub fn select_quest_glyph_atlas_bank(
    page: u8,
    state: QuestGlyphAtlasBankState,
) -> Result<QuestGlyphAtlasBankSelection, QuestGlyphAtlasBankPolicyError> {
    if page == 0 {
        return Ok(QuestGlyphAtlasBankSelection::Original);
    }
    if page > QUEST_GLYPH_ATLAS_LOGICAL_PAGE_MASK {
        return Err(QuestGlyphAtlasBankPolicyError { page });
    }

    for bank in [QuestGlyphAtlasBank::Primary, QuestGlyphAtlasBank::Secondary] {
        if state.logical_pages[bank.index()] != page {
            continue;
        }
        let marker = encode_quest_glyph_atlas_marker(page, bank)?;
        return Ok(if state.pinned[bank.index()] {
            QuestGlyphAtlasBankSelection::Resident { bank, marker }
        } else {
            QuestGlyphAtlasBankSelection::Upload { bank, marker }
        });
    }
    for bank in [QuestGlyphAtlasBank::Primary, QuestGlyphAtlasBank::Secondary] {
        if !state.pinned[bank.index()] {
            return Ok(QuestGlyphAtlasBankSelection::Upload {
                bank,
                marker: encode_quest_glyph_atlas_marker(page, bank)?,
            });
        }
    }
    Ok(QuestGlyphAtlasBankSelection::WaitAllBanksPinned)
}

#[cfg(test)]
#[path = "quest_glyph_atlas_bank_policy_tests.rs"]
mod tests;
