//! `MMENU.BTX` label별 exact 물리 사본과 수동 개발 조판.

use super::{error, MmenuSurfaceError};
use crate::graphics::btx_raster_translation_manifest::{
    GeneralRasterTranslationEntry, GeneralRasterTranslationStatus,
};
use crate::graphics::text_catalog::{Rect, TextBlockSpec, MMENU_TEXT_ASSET};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReplicaRole {
    Base,
    Transition,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ReplicaSpec {
    pub role: ReplicaRole,
    pub rect: Rect,
    pub expected_sha256: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LabelSpec {
    pub block_id: &'static str,
    pub display_text: &'static str,
    pub replicas: [ReplicaSpec; 3],
}

pub(super) const LABELS: [LabelSpec; 6] = [
    LabelSpec {
        block_id: "BTXB-0042",
        display_text: "뿌요\n뿌요",
        replicas: [
            base_replica(
                48,
                40,
                72,
                48,
                "811c14c32b8b7c91c459aa61c2145e62116f3b385a402ce9d34b94cba5067544",
            ),
            transition_replica(
                322,
                16,
                72,
                48,
                "49b4f567a8b9b0354f55502c4660f7eeb14edd75b6daf7a03277af8c41b797ed",
            ),
            transition_replica(
                2,
                200,
                72,
                48,
                "d302e8f19701ba76ef4af36be194dd1e42d761262d8a5def51ca5f7c0dcfb8ee",
            ),
        ],
    },
    LabelSpec {
        block_id: "BTXB-0043",
        display_text: "뿌요뿌요2",
        replicas: [
            base_replica(
                128,
                40,
                64,
                48,
                "6f66b11651850514bfaed0c5fd263b98589b3198b5f38f18277282223c4c0f1c",
            ),
            transition_replica(
                328,
                104,
                64,
                48,
                "fc3265d89348ff318e8544cf9d305b6c1dd18e772e12087a073f2255706cce0a",
            ),
            transition_replica(
                88,
                192,
                64,
                48,
                "fc3265d89348ff318e8544cf9d305b6c1dd18e772e12087a073f2255706cce0a",
            ),
        ],
    },
    LabelSpec {
        block_id: "BTXB-0044",
        display_text: "옵션",
        replicas: [
            base_replica(
                192,
                48,
                56,
                40,
                "ba3b1f74ba2aa146a4f8b99b06961df7a6b0b865c290df93a49dde4c8f57c837",
            ),
            transition_replica(
                426,
                72,
                56,
                40,
                "ec26d36f5670aaa09c14e8140cd9e958a3e2dadeaa3b21f32fec5f28e60ade72",
            ),
            transition_replica(
                258,
                184,
                56,
                40,
                "f51acbe58c97174ed9770878c84da187d4079639f079739600a854003029df74",
            ),
        ],
    },
    LabelSpec {
        block_id: "BTXB-0045",
        display_text: "공식전",
        replicas: [
            base_replica(
                72,
                104,
                64,
                48,
                "35b852f8434745817fe77c7659935293dc6421ae5a0e5a1f7cfce74796bff2a8",
            ),
            transition_replica(
                428,
                12,
                64,
                48,
                "f182a476138614d2b29127a005d518a404e004634f57ef1548ee08e12e274a53",
            ),
            transition_replica(
                156,
                196,
                64,
                48,
                "f182a476138614d2b29127a005d518a404e004634f57ef1548ee08e12e274a53",
            ),
        ],
    },
    LabelSpec {
        block_id: "BTXB-0046",
        display_text: "뿌요\nBOX",
        replicas: [
            base_replica(
                144,
                104,
                64,
                48,
                "641ef18c2ae2f377be05abfa0f88de70cda74ef0b4d1f06ac5f4752ce477f9e7",
            ),
            transition_replica(
                520,
                12,
                64,
                48,
                "2a7c44454caa14c2718c6689082bf671b59d45e3ba92d59ca74e032696e84646",
            ),
            transition_replica(
                432,
                132,
                64,
                48,
                "2a7c44454caa14c2718c6689082bf671b59d45e3ba92d59ca74e032696e84646",
            ),
        ],
    },
    LabelSpec {
        block_id: "BTXB-0047",
        display_text: "뿌요\n커뮤!",
        replicas: [
            base_replica(
                208,
                104,
                72,
                48,
                "c853cb8966c564fc7ad9fa35f1c5341782751c7271bfe3f8a9c6fa5502242fab",
            ),
            transition_replica(
                430,
                196,
                72,
                48,
                "851448d7807e8a0a64a6628ddc2e86c3d5bb575a70d8a1ba048ba8d3b759dead",
            ),
            transition_replica(
                326,
                196,
                72,
                48,
                "ac7cdf024cffd5604bfbbde5ff02ebe431be6da256fa0978fa4cd6e932e26263",
            ),
        ],
    },
];

const fn base_replica(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    expected_sha256: &'static str,
) -> ReplicaSpec {
    ReplicaSpec {
        role: ReplicaRole::Base,
        rect: Rect {
            x,
            y,
            width,
            height,
        },
        expected_sha256,
    }
}

const fn transition_replica(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    expected_sha256: &'static str,
) -> ReplicaSpec {
    ReplicaSpec {
        role: ReplicaRole::Transition,
        rect: Rect {
            x,
            y,
            width,
            height,
        },
        expected_sha256,
    }
}

fn without_whitespace(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

pub(super) fn verify_translation(
    label: LabelSpec,
    block: TextBlockSpec<'_>,
    entry: &GeneralRasterTranslationEntry,
) -> Result<(), MmenuSurfaceError> {
    if entry.block_id != label.block_id
        || entry.semantic_family != "shell-mode-navigation"
        || entry.archive_paths != "/FIL_GRP.VDD;1"
        || entry.asset_name != MMENU_TEXT_ASSET.name
        || entry.asset_sha256 != MMENU_TEXT_ASSET.expected_decompressed_sha256
        || entry.block_pixels_sha256 != block.expected_pixels_sha256
        || entry.bbox_x != block.bbox.x
        || entry.bbox_y != block.bbox.y
        || entry.bbox_width != block.bbox.width
        || entry.bbox_height != block.bbox.height
        || entry.protected_source != block.source
        || entry.status != GeneralRasterTranslationStatus::DraftHumanReviewRequired
        || entry.category_id.is_some()
        || entry.profile_id.is_some()
        || without_whitespace(&entry.ko) != without_whitespace(label.display_text)
    {
        return Err(error(format!(
            "{}: 첫 메뉴 번역·수동 조판 보호 계약 drift",
            label.block_id
        )));
    }
    Ok(())
}
