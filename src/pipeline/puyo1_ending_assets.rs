//! PUYO1 ending에서 관측한 resource ID와 원본 BTX descriptor의 연결.
//!
//! runtime 관측은 별도 증거가 소유한다. 이 모듈은 그때 기록한 resource
//! ID가 지원 원본의 어느 BTX를 가리키는지만 source gate에서 검증한다.
//! 같은 픽셀의 `P2_END*` 사본은 PUYO1 ending 선택 집합에 합치지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::disasm::BtxNameLink;

const PUYO1_EXE: &str = "PUYO1.EXE";
const PP1_GRP: &str = "/PUYO1/PP1_GRP.VDD;1";
const PUYO1_DESCRIPTOR_RUNTIME_START: u32 = 0x8006_7E48;
const BTX_DESCRIPTOR_BYTES: u32 = 36;
const BTX_NAME_POINTER_FIELD_OFFSET: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExpectedEndingAsset {
    resource: u16,
    asset_name: &'static str,
}

const RUNTIME_SELECTED_ENDING_ASSETS: [ExpectedEndingAsset; 9] = [
    ExpectedEndingAsset {
        resource: 83,
        asset_name: "P1_ED1.BTX",
    },
    ExpectedEndingAsset {
        resource: 84,
        asset_name: "P1_ED2.BTX",
    },
    ExpectedEndingAsset {
        resource: 85,
        asset_name: "P1_ED3.BTX",
    },
    ExpectedEndingAsset {
        resource: 86,
        asset_name: "P1_STF1.BTX",
    },
    ExpectedEndingAsset {
        resource: 87,
        asset_name: "P1_STF2.BTX",
    },
    ExpectedEndingAsset {
        resource: 88,
        asset_name: "P1_STF3.BTX",
    },
    ExpectedEndingAsset {
        resource: 107,
        asset_name: "CAST_BG.BTX",
    },
    ExpectedEndingAsset {
        resource: 108,
        asset_name: "KAO.BTX",
    },
    ExpectedEndingAsset {
        resource: 109,
        asset_name: "CHR.BTX",
    },
];

const PROTECTED_PUYO2_ENDING_COPIES: [ExpectedEndingAsset; 3] = [
    ExpectedEndingAsset {
        resource: 158,
        asset_name: "P2_END10.BTX",
    },
    ExpectedEndingAsset {
        resource: 163,
        asset_name: "P2_END15.BTX",
    },
    ExpectedEndingAsset {
        resource: 164,
        asset_name: "P2_END16.BTX",
    },
];

/// 지원 원본 descriptor에서 확정한 resource와 BTX 이름.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo1EndingAssetBinding {
    pub resource: u16,
    pub asset_name: &'static str,
    pub descriptor_runtime: u32,
}

/// PUYO1 ending 관측 집합과 보호할 PUYO2 사본의 원본 연결.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo1EndingAssetMap {
    pub descriptor_runtime_start: u32,
    pub runtime_selected_assets: Vec<Puyo1EndingAssetBinding>,
    pub protected_puyo2_copies: Vec<Puyo1EndingAssetBinding>,
}

/// PUYO1 ending resource 원본 연결 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo1EndingAssetMapError(String);

impl fmt::Display for Puyo1EndingAssetMapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Puyo1EndingAssetMapError {}

fn error(message: impl Into<String>) -> Puyo1EndingAssetMapError {
    Puyo1EndingAssetMapError(message.into())
}

fn resolve_binding(
    links: &[BtxNameLink],
    expected: ExpectedEndingAsset,
) -> Result<Puyo1EndingAssetBinding, Puyo1EndingAssetMapError> {
    let matches = links
        .iter()
        .filter(|link| {
            link.exe == PUYO1_EXE
                && link.archive == PP1_GRP
                && link.asset_name == expected.asset_name
        })
        .collect::<Vec<_>>();
    let [link] = matches.as_slice() else {
        return Err(error(format!(
            "PUYO1 ending BTX 연결 수 불일치: asset={}, count={}",
            expected.asset_name,
            matches.len()
        )));
    };
    if link.descriptor_runtime_start != PUYO1_DESCRIPTOR_RUNTIME_START {
        return Err(error(format!(
            "PUYO1 ending descriptor 시작 주소 불일치: asset={}, expected=0x{:08X}, actual=0x{:08X}",
            expected.asset_name,
            PUYO1_DESCRIPTOR_RUNTIME_START,
            link.descriptor_runtime_start
        )));
    }
    let descriptor_runtime = link
        .descriptor_pointer_runtime
        .checked_sub(BTX_NAME_POINTER_FIELD_OFFSET)
        .ok_or_else(|| error(format!("{} descriptor 주소 underflow", expected.asset_name)))?;
    let delta = descriptor_runtime
        .checked_sub(PUYO1_DESCRIPTOR_RUNTIME_START)
        .ok_or_else(|| {
            error(format!(
                "{} descriptor가 PUYO1 block 앞임",
                expected.asset_name
            ))
        })?;
    if delta % BTX_DESCRIPTOR_BYTES != 0 {
        return Err(error(format!(
            "PUYO1 ending descriptor가 36바이트 격자 밖임: asset={}, runtime=0x{descriptor_runtime:08X}",
            expected.asset_name
        )));
    }
    let actual_resource = u16::try_from(delta / BTX_DESCRIPTOR_BYTES)
        .map_err(|_| error(format!("{} resource가 u16 밖임", expected.asset_name)))?;
    if actual_resource != expected.resource {
        return Err(error(format!(
            "PUYO1 ending resource 불일치: asset={}, expected={}, actual={actual_resource}",
            expected.asset_name, expected.resource
        )));
    }
    Ok(Puyo1EndingAssetBinding {
        resource: actual_resource,
        asset_name: expected.asset_name,
        descriptor_runtime,
    })
}

/// runtime에서 관측한 PUYO1 ending resource와 보호 사본을 원본에 연결한다.
pub fn verify_puyo1_ending_asset_map(
    links: &[BtxNameLink],
) -> Result<Puyo1EndingAssetMap, Puyo1EndingAssetMapError> {
    let runtime_selected_assets = RUNTIME_SELECTED_ENDING_ASSETS
        .iter()
        .copied()
        .map(|expected| resolve_binding(links, expected))
        .collect::<Result<Vec<_>, _>>()?;
    let protected_puyo2_copies = PROTECTED_PUYO2_ENDING_COPIES
        .iter()
        .copied()
        .map(|expected| resolve_binding(links, expected))
        .collect::<Result<Vec<_>, _>>()?;

    let resources = runtime_selected_assets
        .iter()
        .chain(&protected_puyo2_copies)
        .map(|binding| binding.resource)
        .collect::<BTreeSet<_>>();
    if resources.len() != runtime_selected_assets.len() + protected_puyo2_copies.len() {
        return Err(error("PUYO1 ending resource ID가 서로 겹침"));
    }

    Ok(Puyo1EndingAssetMap {
        descriptor_runtime_start: PUYO1_DESCRIPTOR_RUNTIME_START,
        runtime_selected_assets,
        protected_puyo2_copies,
    })
}

#[cfg(test)]
#[path = "puyo1_ending_assets_tests.rs"]
mod tests;
