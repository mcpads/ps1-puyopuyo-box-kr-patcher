//! `P2_INFO4/5` distinct-Hangul 256-cell 전체 디스크 저장 PoC.
//!
//! fixed-cell assembler가 현대 한글 전 구간에서 고른 서로 다른 256자를
//! PUYO1/PUYO2의 네 물리 BTX에 적용한다. 고정 컨테이너를 넘으면 검증된
//! 32M 예약영역으로 성장 VDD를 재배치하고 Expected Write·Mode2/Form1
//! EDC/ECC·최종 readback을 한 번에 검증한다. 이 입력은 번역 승인이나
//! runtime 표시를 대신하지 않는다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::disc::VerifiedSourceImage;
use crate::graphics::text_catalog::{TextAssetSpec, P2_INFO4_TEXT_ASSET, P2_INFO5_TEXT_ASSET};

mod asset_preparation;
mod disc_build;

pub const P2_INFO_PAGE_STRESS_DISC_SCHEMA_VERSION: &str = "p2-info-page-stress-disc-v1";
pub const BOX_P2_INFO_PAGE_STRESS_DISC_ASSETS: usize = 4;
pub const BOX_P2_INFO_PAGE_STRESS_DISC_VDDS: usize = 2;

pub const BOX_P2_INFO_PAGE_STRESS_DISC_PLANNED_WRITES: usize = 2_410;
pub const BOX_P2_INFO_PAGE_STRESS_DISC_METADATA_WRITES: usize = 3;
pub const BOX_P2_INFO_PAGE_STRESS_DISC_DATA_WRITES: usize = 2_407;
pub const BOX_P2_INFO_PAGE_STRESS_DISC_TOUCHED_SECTORS: usize = 2_410;
pub const BOX_P2_INFO_PAGE_STRESS_DISC_FINAL_EXPECTED_WRITES: usize = 4_820;
pub const BOX_P2_INFO_PAGE_STRESS_DISC_IMAGE_SHA256: &str =
    "ca4504622294574a14bcffc93375f7bd59a8a9700dbcec7049d0873991509682";
pub const BOX_P2_INFO_PAGE_STRESS_PUYO1_RELOCATED_LBA: u32 = 43_778;
pub const BOX_P2_INFO_PAGE_STRESS_PUYO1_RELOCATED_BYTES: u32 = 2_201_600;
pub const BOX_P2_INFO_PAGE_STRESS_PUYO2_RELOCATED_LBA: u32 = 44_853;
pub const BOX_P2_INFO_PAGE_STRESS_PUYO2_RELOCATED_BYTES: u32 = 2_727_936;
pub const BOX_P2_INFO_PAGE_STRESS_RELOCATED_32M_LBA: u32 = 46_185;
pub const BOX_P2_INFO_PAGE_STRESS_RELOCATED_32M_BYTES: u32 = 28_624_896;
pub const BOX_P2_INFO_PAGE_STRESS_CNX_UPPER_BOUND_BYTES: usize = 133_120;
pub const BOX_P2_INFO_PAGE_STRESS_VDD_UPPER_BOUND_BYTES: u64 = 5_453_824;
pub const BOX_P2_INFO_PAGE_STRESS_RESERVATION_HEADROOM_BYTES: u64 = 28_100_608;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P2InfoPageVddRewriteMethod {
    InPlace,
    RepackInContainer,
    RelocateToReservation,
}

impl P2InfoPageVddRewriteMethod {
    pub const fn label(self) -> &'static str {
        match self {
            Self::InPlace => "in-place",
            Self::RepackInContainer => "repack-in-container",
            Self::RelocateToReservation => "relocate-to-32m-reservation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2InfoPageStressAssetReport {
    pub scope: &'static str,
    pub archive: &'static str,
    pub asset: &'static str,
    pub cnx_before: usize,
    pub cnx_after: usize,
    pub changed_cells: usize,
    pub changed_pixels: usize,
    pub changed_plain_bytes: usize,
    pub output_plain_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2InfoPageStressVddReport {
    pub scope: &'static str,
    pub archive: &'static str,
    pub method: P2InfoPageVddRewriteMethod,
    pub changed_vdd_bytes: usize,
    pub output_vdd_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2InfoPageStressRelocationReport {
    pub scope: &'static str,
    pub archive: &'static str,
    pub original_lba: u32,
    pub original_bytes: u32,
    pub relocated_lba: u32,
    pub relocated_bytes: u32,
    pub relocated_sectors: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2InfoPageStressDiscReport {
    pub schema_version: &'static str,
    pub scanned_hangul: usize,
    pub skipped_hangul: usize,
    pub renderable_hangul: usize,
    pub distinct_hangul: usize,
    pub assets: Vec<P2InfoPageStressAssetReport>,
    pub vdds: Vec<P2InfoPageStressVddReport>,
    pub relocations: Vec<P2InfoPageStressRelocationReport>,
    pub relocated_32m_lba: u32,
    pub relocated_32m_bytes: u32,
    pub conservative_cnx_upper_bound_bytes: usize,
    pub conservative_vdd_upper_bound_bytes: u64,
    pub reservation_headroom_bytes: u64,
    pub extent_overlaps: usize,
    pub planned_writes: usize,
    pub metadata_writes: usize,
    pub data_writes: usize,
    pub touched_sectors: usize,
    pub final_expected_writes: usize,
    pub image_sha256: String,
    pub product_eligible: bool,
    pub runtime_verified: bool,
}

#[derive(Debug)]
pub struct P2InfoPageStressDiscBuild {
    pub image: Vec<u8>,
    pub report: P2InfoPageStressDiscReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2InfoPageStressDiscError(String);

impl fmt::Display for P2InfoPageStressDiscError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for P2InfoPageStressDiscError {}

fn error(message: impl Into<String>) -> P2InfoPageStressDiscError {
    P2InfoPageStressDiscError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TargetSpec {
    scope: &'static str,
    archive: &'static str,
    asset: &'static str,
}

const TARGETS: [TargetSpec; BOX_P2_INFO_PAGE_STRESS_DISC_ASSETS] = [
    TargetSpec {
        scope: "PUYO1",
        archive: "/PUYO1/PP1_GRP.VDD;1",
        asset: "P2_INFO4.BTX",
    },
    TargetSpec {
        scope: "PUYO1",
        archive: "/PUYO1/PP1_GRP.VDD;1",
        asset: "P2_INFO5.BTX",
    },
    TargetSpec {
        scope: "PUYO2",
        archive: "/PUYO2/PP2_GRP.VDD;1",
        asset: "P2_INFO4.BTX",
    },
    TargetSpec {
        scope: "PUYO2",
        archive: "/PUYO2/PP2_GRP.VDD;1",
        asset: "P2_INFO5.BTX",
    },
];

const EXPECTED_ASSET_ROWS: [(&str, &str, usize, usize, usize, usize, &str); 4] = [
    (
        "PUYO1",
        "P2_INFO4.BTX",
        6_407,
        7_096,
        24_303,
        16_346,
        "dc42fd0c7615202a1e03f95e141f070406adc64108813218e0c628b93e1b1915",
    ),
    (
        "PUYO1",
        "P2_INFO5.BTX",
        3_960,
        7_096,
        49_259,
        26_277,
        "dc42fd0c7615202a1e03f95e141f070406adc64108813218e0c628b93e1b1915",
    ),
    (
        "PUYO2",
        "P2_INFO4.BTX",
        6_407,
        7_096,
        24_303,
        16_346,
        "dc42fd0c7615202a1e03f95e141f070406adc64108813218e0c628b93e1b1915",
    ),
    (
        "PUYO2",
        "P2_INFO5.BTX",
        3_960,
        7_096,
        49_259,
        26_277,
        "dc42fd0c7615202a1e03f95e141f070406adc64108813218e0c628b93e1b1915",
    ),
];

const EXPECTED_VDD_ROWS: [(&str, P2InfoPageVddRewriteMethod, usize, &str); 2] = [
    (
        "PUYO1",
        P2InfoPageVddRewriteMethod::RelocateToReservation,
        611_372,
        "1ce3b76153c4bf298281ec8ac1d5c04f252b66c72d261802d3a1237a79a303f7",
    ),
    (
        "PUYO2",
        P2InfoPageVddRewriteMethod::RelocateToReservation,
        2_198_805,
        "b6907c80005ad95b5cedd44be6fc5e0d8138b424d58b42f25bed5468a02f7097",
    ),
];

fn text_asset(name: &str) -> Result<TextAssetSpec<'static>, P2InfoPageStressDiscError> {
    match name {
        "P2_INFO4.BTX" => Ok(P2_INFO4_TEXT_ASSET),
        "P2_INFO5.BTX" => Ok(P2_INFO5_TEXT_ASSET),
        _ => Err(error(format!("P2 INFO stress 미등록 자산: {name}"))),
    }
}

/// 지원 원본에서 서로 다른 256 한글을 네 INFO page에 넣은 raw BIN을 만든다.
pub fn build_p2_info_page_stress_disc(
    source: &VerifiedSourceImage,
) -> Result<P2InfoPageStressDiscBuild, P2InfoPageStressDiscError> {
    disc_build::build(source)
}
