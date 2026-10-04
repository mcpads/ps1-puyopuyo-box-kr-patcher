//! `BOXOPT.BTX` 겹침 selector의 수동 조판 관계.
//!
//! 문구 자체는 번역 manifest가 소유한다. 이 파일은 어떤 번역 문자열의 어느
//! 문자 구간을 공용 pixel 영역에 놓는지만 선언한다. 공백을 물리 간격으로
//! 바꾸는 선택도 자동 감기가 아니라 이 명시적 조판의 일부다.

use crate::graphics::text_catalog::Rect;
use crate::graphics::text_profile::HorizontalAlign;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TranslationSlice {
    pub block_id: &'static str,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TextRun {
    pub rect: Rect,
    pub text: TranslationSlice,
    pub align: HorizontalAlign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SliceEquality {
    pub left: TranslationSlice,
    pub right: TranslationSlice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SliceLiteral {
    pub slice: TranslationSlice,
    pub expected: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SharedSelectorLayout {
    pub target_block_ids: &'static [&'static str],
    pub preserved_block_ids: &'static [&'static str],
    pub runs: &'static [TextRun],
    pub equalities: &'static [SliceEquality],
    pub literals: &'static [SliceLiteral],
}

const fn slice(block_id: &'static str, start: usize, end: usize) -> TranslationSlice {
    TranslationSlice {
        block_id,
        start,
        end,
    }
}

const fn run(rect: Rect, text: TranslationSlice, align: HorizontalAlign) -> TextRun {
    TextRun { rect, text, align }
}

const COM_TARGETS: [&str; 1] = ["BTXB-0286"];
const COM_PRESERVED: [&str; 1] = ["BTXB-0284"];
const COM_RUNS: [TextRun; 1] = [run(
    Rect {
        x: 208,
        y: 208,
        width: 48,
        height: 16,
    },
    slice("BTXB-0286", 4, 7),
    HorizontalAlign::Left { padding: 0 },
)];
const COM_LITERALS: [SliceLiteral; 1] = [SliceLiteral {
    slice: slice("BTXB-0286", 0, 4),
    expected: "COM ",
}];

const NORMAL_TARGETS: [&str; 2] = ["BTXB-0293", "BTXB-0299"];
const NORMAL_RUNS: [TextRun; 1] = [run(
    Rect {
        x: 0,
        y: 0,
        width: 64,
        height: 16,
    },
    slice("BTXB-0293", 0, 2),
    HorizontalAlign::Center,
)];
const NORMAL_EQUALITIES: [SliceEquality; 1] = [SliceEquality {
    left: slice("BTXB-0293", 0, 2),
    right: slice("BTXB-0299", 0, 2),
}];

const SCORE_TARGETS: [&str; 2] = ["BTXB-0295", "BTXB-0311"];
const SCORE_RUNS: [TextRun; 3] = [
    run(
        Rect {
            x: 64,
            y: 64,
            width: 112,
            height: 16,
        },
        slice("BTXB-0311", 0, 4),
        HorizontalAlign::Right { padding: 0 },
    ),
    run(
        Rect {
            x: 176,
            y: 64,
            width: 32,
            height: 16,
        },
        slice("BTXB-0311", 5, 7),
        HorizontalAlign::Right { padding: 0 },
    ),
    run(
        Rect {
            x: 208,
            y: 64,
            width: 32,
            height: 16,
        },
        slice("BTXB-0295", 2, 4),
        HorizontalAlign::Left { padding: 0 },
    ),
];
const SCORE_EQUALITIES: [SliceEquality; 1] = [SliceEquality {
    left: slice("BTXB-0295", 0, 2),
    right: slice("BTXB-0311", 5, 7),
}];

const EDIT_TARGETS: [&str; 2] = ["BTXB-0298", "BTXB-0302"];
const EDIT_RUNS: [TextRun; 2] = [
    run(
        Rect {
            x: 0,
            y: 80,
            width: 80,
            height: 16,
        },
        slice("BTXB-0302", 0, 2),
        HorizontalAlign::Right { padding: 0 },
    ),
    run(
        Rect {
            x: 80,
            y: 80,
            width: 32,
            height: 16,
        },
        slice("BTXB-0302", 3, 5),
        HorizontalAlign::Left { padding: 0 },
    ),
];
const EDIT_EQUALITIES: [SliceEquality; 1] = [SliceEquality {
    left: slice("BTXB-0298", 0, 2),
    right: slice("BTXB-0302", 0, 2),
}];

const CHAIN_TARGETS: [&str; 2] = ["BTXB-0305", "BTXB-0306"];
const CHAIN_RUNS: [TextRun; 2] = [
    run(
        Rect {
            x: 128,
            y: 0,
            width: 16,
            height: 16,
        },
        slice("BTXB-0306", 0, 1),
        HorizontalAlign::Right { padding: 0 },
    ),
    run(
        Rect {
            x: 144,
            y: 0,
            width: 80,
            height: 16,
        },
        slice("BTXB-0306", 1, 6),
        HorizontalAlign::Left { padding: 0 },
    ),
];
const CHAIN_EQUALITIES: [SliceEquality; 1] = [SliceEquality {
    left: slice("BTXB-0305", 0, 5),
    right: slice("BTXB-0306", 1, 6),
}];

const FREEZE_TARGETS: [&str; 2] = ["BTXB-0314", "BTXB-0323"];
const FREEZE_RUNS: [TextRun; 2] = [
    run(
        Rect {
            x: 0,
            y: 96,
            width: 80,
            height: 16,
        },
        slice("BTXB-0323", 0, 6),
        HorizontalAlign::Right { padding: 0 },
    ),
    run(
        Rect {
            x: 80,
            y: 96,
            width: 16,
            height: 16,
        },
        slice("BTXB-0314", 0, 1),
        HorizontalAlign::Left { padding: 0 },
    ),
];

pub(super) const SHARED_SELECTOR_LAYOUTS: [SharedSelectorLayout; 6] = [
    SharedSelectorLayout {
        target_block_ids: &COM_TARGETS,
        preserved_block_ids: &COM_PRESERVED,
        runs: &COM_RUNS,
        equalities: &[],
        literals: &COM_LITERALS,
    },
    SharedSelectorLayout {
        target_block_ids: &NORMAL_TARGETS,
        preserved_block_ids: &[],
        runs: &NORMAL_RUNS,
        equalities: &NORMAL_EQUALITIES,
        literals: &[],
    },
    SharedSelectorLayout {
        target_block_ids: &SCORE_TARGETS,
        preserved_block_ids: &[],
        runs: &SCORE_RUNS,
        equalities: &SCORE_EQUALITIES,
        literals: &[],
    },
    SharedSelectorLayout {
        target_block_ids: &EDIT_TARGETS,
        preserved_block_ids: &[],
        runs: &EDIT_RUNS,
        equalities: &EDIT_EQUALITIES,
        literals: &[],
    },
    SharedSelectorLayout {
        target_block_ids: &CHAIN_TARGETS,
        preserved_block_ids: &[],
        runs: &CHAIN_RUNS,
        equalities: &CHAIN_EQUALITIES,
        literals: &[],
    },
    SharedSelectorLayout {
        target_block_ids: &FREEZE_TARGETS,
        preserved_block_ids: &[],
        runs: &FREEZE_RUNS,
        equalities: &[],
        literals: &[],
    },
];
