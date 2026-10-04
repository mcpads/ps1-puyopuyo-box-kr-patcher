//! 다섯 서브게임 `BOXOPT.BTX`의 sparse selector 소비 경계.
//!
//! 공용 sprite renderer는 object `+0x6C`의 resource와 `+0x6A`의 selector를
//! 읽고, descriptor `+0x1C`가 가리키는 12-byte rectangle 표를 소비한다.
//! 이 모듈은 각 EXE를 별도 사양으로 검산하고, 다섯 setup이 같은 96-slot
//! asset-backed prefix를 공급하는지 확인한다. 정적 연결은 실제 실행을
//! 대신하지 않으며, GOCHA1 runtime 관측은 문서와 emucap 원장이 소유한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::btx::PixelFormat;
use crate::coords::ExeMap;
use crate::disasm::{BtxDescriptorSelectionAnalysis, BtxNameAnalysis, BtxNameLink};
use crate::graphics::boxopt::{
    parse_boxopt_selector_table, BoxoptSelector, BoxoptSelectorError,
    BOXOPT_ASSET_BACKED_SELECTORS, BOXOPT_EMPTY_SELECTORS, BOXOPT_OVERLAP_GROUPS,
    BOXOPT_PHRASE_SELECTORS, BOXOPT_SELECTOR_SLOTS, BOXOPT_SELECTOR_TABLE_BYTES,
    BOXOPT_SELECTOR_TABLE_SHA256, BOXOPT_TRANSLATION_TARGET_SELECTORS,
};
use crate::isa::{decode, InstructionKind as K, Reg};

use super::btx_pending_static_cells::{
    descriptor_entry_runtime, resource_id, scan_setup_records, verify_initial_descriptor,
    BtxPendingStaticCellError, SetupScanContext,
};

const BOXOPT_ASSET_NAME: &str = "BOXOPT.BTX";
const BOXOPT_DECOMPRESSED_BYTES: u32 = 768 * 256 / 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RendererSiteSpec {
    resource_load_runtime: u32,
    object_register: Reg,
    descriptor_register: Reg,
    record_register: Reg,
    cell_register: Reg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoxoptStaticSpec {
    exe: &'static str,
    archive: &'static str,
    resource_id: u16,
    descriptor_runtime: u32,
    setup_runtime: u32,
    table_runtime: u32,
    descriptor_root_pointer_runtime: u32,
    renderer_sites: [RendererSiteSpec; 2],
}

const fn renderer_pair(first: u32, second: u32, second_object: Reg) -> [RendererSiteSpec; 2] {
    [
        RendererSiteSpec {
            resource_load_runtime: first,
            object_register: Reg::S5,
            descriptor_register: Reg::S4,
            record_register: Reg::S6,
            cell_register: Reg::S3,
        },
        RendererSiteSpec {
            resource_load_runtime: second,
            object_register: second_object,
            descriptor_register: Reg::FP,
            record_register: Reg::T1,
            cell_register: Reg::A3,
        },
    ]
}

const BOXOPT_SPECS: [BoxoptStaticSpec; 5] = [
    BoxoptStaticSpec {
        exe: "GOCHA1.EXE",
        archive: "/GOCHA1/TUU_GRP.VDD;1",
        resource_id: 1,
        descriptor_runtime: 0x8007_0B4C,
        setup_runtime: 0x8006_EDB0,
        table_runtime: 0x8006_DC14,
        descriptor_root_pointer_runtime: 0x800B_DE68,
        renderer_sites: renderer_pair(0x8002_B53C, 0x8002_B82C, Reg::S3),
    },
    BoxoptStaticSpec {
        exe: "GOCHA2.EXE",
        archive: "/GOCHA2/GOTY_GRP.VDD;1",
        resource_id: 9,
        descriptor_runtime: 0x8005_ABA0,
        setup_runtime: 0x8005_A300,
        table_runtime: 0x8005_9164,
        descriptor_root_pointer_runtime: 0x800A_3910,
        renderer_sites: renderer_pair(0x8002_72E8, 0x8002_75D8, Reg::S1),
    },
    BoxoptStaticSpec {
        exe: "HORI2.EXE",
        archive: "/HORI2/HORI2GRP.VDD;1",
        resource_id: 9,
        descriptor_runtime: 0x8005_A958,
        setup_runtime: 0x8005_A0D8,
        table_runtime: 0x8005_8F3C,
        descriptor_root_pointer_runtime: 0x800A_3FC0,
        renderer_sites: renderer_pair(0x8002_7308, 0x8002_75F8, Reg::S1),
    },
    BoxoptStaticSpec {
        exe: "HORI4.EXE",
        archive: "/HORI4/HORI4GRP.VDD;1",
        resource_id: 9,
        descriptor_runtime: 0x8005_AC0C,
        setup_runtime: 0x8005_A384,
        table_runtime: 0x8005_91E8,
        descriptor_root_pointer_runtime: 0x800A_4268,
        renderer_sites: renderer_pair(0x8002_75E0, 0x8002_78D0, Reg::S1),
    },
    BoxoptStaticSpec {
        exe: "MINPUYO.EXE",
        archive: "/MINPUYO/MIN_GRP.VDD;1",
        resource_id: 9,
        descriptor_runtime: 0x8005_B208,
        setup_runtime: 0x8005_A968,
        table_runtime: 0x8005_97CC,
        descriptor_root_pointer_runtime: 0x800A_3F38,
        renderer_sites: renderer_pair(0x8002_762C, 0x8002_791C, Reg::S1),
    },
];

/// 한 EXE에서 정적으로 닫힌 BOXOPT setup과 공용 renderer 소비 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxoptStaticOwner {
    pub exe: &'static str,
    pub archive: &'static str,
    pub resource_id: u16,
    pub descriptor_runtime: u32,
    pub descriptor_selection_runtime: u32,
    pub setup_runtime: u32,
    pub table_runtime: u32,
    pub renderer_resource_loads: [u32; 2],
    pub renderer_selector_loads: [u32; 2],
    pub renderer_table_loads: [u32; 2],
}

/// 다섯 물리 사본에 공통인 selector 표와 소비자 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxoptStaticAnalysis {
    pub owners: Vec<BoxoptStaticOwner>,
    pub selectors: Vec<BoxoptSelector>,
    pub selector_table_sha256: &'static str,
    pub selector_slots: usize,
    pub asset_backed_selectors: usize,
    pub empty_selectors: usize,
    pub phrase_selectors: usize,
    pub translation_target_selectors: usize,
    pub overlap_groups: usize,
    pub phrase_selector_pixels: usize,
    pub phrase_surface_union_pixels: usize,
    pub first_non_asset_selector: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxoptStaticError(String);

impl fmt::Display for BoxoptStaticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "BOXOPT 정적 소비 감사 실패: {}", self.0)
    }
}

impl Error for BoxoptStaticError {}

impl From<BoxoptSelectorError> for BoxoptStaticError {
    fn from(source: BoxoptSelectorError) -> Self {
        Self(source.to_string())
    }
}

impl From<BtxPendingStaticCellError> for BoxoptStaticError {
    fn from(source: BtxPendingStaticCellError) -> Self {
        Self(source.to_string())
    }
}

fn error(message: impl Into<String>) -> BoxoptStaticError {
    BoxoptStaticError(message.into())
}

fn exact_name_link(
    names: &BtxNameAnalysis,
    spec: BoxoptStaticSpec,
) -> Result<&BtxNameLink, BoxoptStaticError> {
    let links = names
        .links
        .iter()
        .filter(|link| {
            link.exe == spec.exe
                && link.archive == spec.archive
                && link.asset_name == BOXOPT_ASSET_NAME
        })
        .collect::<Vec<_>>();
    match links.as_slice() {
        [link] => Ok(*link),
        _ => Err(error(format!(
            "{}:{} BOXOPT 이름표 연결 수가 1이 아님: {}",
            spec.exe,
            spec.archive,
            links.len()
        ))),
    }
}

fn executable_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    runtime: u32,
    len: usize,
) -> Result<&'a [u8], BoxoptStaticError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "{} runtime 주소가 EXE text 밖임: 0x{runtime:08X}",
            map.name
        ))
    })?;
    text.get(offset..offset + len).ok_or_else(|| {
        error(format!(
            "{} EXE text 범위 이탈: 0x{runtime:08X}+{len}",
            map.name
        ))
    })
}

fn read_u32(raw: &[u8]) -> u32 {
    u32::from_le_bytes(raw.try_into().expect("4바이트 word"))
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), BoxoptStaticError> {
    let actual = decode(runtime, read_u32(executable_slice(map, text, runtime, 4)?));
    if actual.kind != expected {
        return Err(error(format!(
            "{} typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            map.name, actual.kind
        )));
    }
    Ok(())
}

fn split_address(runtime: u32) -> (u16, i16) {
    (
        runtime.wrapping_add(0x8000).wrapping_shr(16) as u16,
        runtime as u16 as i16,
    )
}

fn verify_renderer_site(
    map: &ExeMap,
    text: &[u8],
    root_pointer_runtime: u32,
    site: RendererSiteSpec,
) -> Result<(), BoxoptStaticError> {
    let start = site.resource_load_runtime;
    let (root_high, root_low) = split_address(root_pointer_runtime);
    let checks = [
        (
            0x00,
            K::Lhu {
                rt: Reg::V1,
                base: site.object_register,
                offset: 0x6C,
            },
        ),
        (
            0x04,
            K::Lui {
                rt: Reg::A0,
                imm: root_high,
            },
        ),
        (
            0x08,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V1,
                sa: 3,
            },
        ),
        (
            0x0C,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x10,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x14,
            K::Lw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: root_low,
            },
        ),
        (
            0x18,
            K::Lhu {
                rt: Reg::A0,
                base: site.object_register,
                offset: 0x6A,
            },
        ),
        (
            0x1C,
            K::Addu {
                rd: site.descriptor_register,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x20,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A0,
                sa: 1,
            },
        ),
        (
            0x24,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            0x28,
            K::Lw {
                rt: Reg::V1,
                base: site.descriptor_register,
                offset: 0x1C,
            },
        ),
        (
            0x2C,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x30,
            K::Addu {
                rd: site.record_register,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x38,
            K::Addiu {
                rt: site.cell_register,
                rs: site.record_register,
                imm: 12,
            },
        ),
    ];
    for (offset, expected) in checks {
        require_instruction(map, text, start + offset, expected)?;
    }
    Ok(())
}

/// 다섯 EXE의 BOXOPT setup, 96-slot prefix와 두 renderer를 검증한다.
pub fn verify_box_boxopt_static<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    names: &BtxNameAnalysis,
    selections: &BtxDescriptorSelectionAnalysis,
) -> Result<BoxoptStaticAnalysis, BoxoptStaticError> {
    let mut executable_map = BTreeMap::new();
    for (map, text) in executables {
        if executable_map.insert(map.name, (map, text)).is_some() {
            return Err(error(format!("{} EXE 입력 중복", map.name)));
        }
    }

    let mut owners = Vec::new();
    let mut common_table = None;
    for spec in BOXOPT_SPECS {
        let &(map, text) = executable_map
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} EXE 입력 누락", spec.exe)))?;
        let link = exact_name_link(names, spec)?;
        let descriptor_runtime = descriptor_entry_runtime(link)?;
        let descriptor_resource_id = resource_id(link, descriptor_runtime)?;
        let (_, plain_bytes, initial_cell_table, initial_cell_count) =
            verify_initial_descriptor(map, text, link, descriptor_runtime)?;
        if descriptor_runtime != spec.descriptor_runtime
            || descriptor_resource_id != spec.resource_id
            || plain_bytes != BOXOPT_DECOMPRESSED_BYTES
            || initial_cell_table != 0
            || initial_cell_count != 0
        {
            return Err(error(format!(
                "{} BOXOPT descriptor 사양 불일치: runtime/resource/plain/cell={descriptor_runtime:08X}/{descriptor_resource_id}/{plain_bytes}/{initial_cell_table:08X}/{initial_cell_count}",
                spec.exe
            )));
        }
        let matching_selections = selections
            .selections
            .iter()
            .filter(|selection| selection.exe == spec.exe && selection.archive == spec.archive)
            .collect::<Vec<_>>();
        let [selection] = matching_selections.as_slice() else {
            return Err(error(format!(
                "{} BOXOPT descriptor 선택 경계 수가 1이 아님: {}",
                spec.exe,
                matching_selections.len()
            )));
        };
        let setups = scan_setup_records(
            spec.exe,
            text,
            SetupScanContext {
                initialized_len: text.len(),
                load_base: map.load_addr,
                resource_id: descriptor_resource_id,
                review_width: 768,
                review_height: 256,
                format: PixelFormat::Indexed4,
                descriptor_plain_bytes: plain_bytes,
            },
        )?;
        let [setup] = setups.as_slice() else {
            return Err(error(format!(
                "{} BOXOPT setup 수가 1이 아님: {}",
                spec.exe,
                setups.len()
            )));
        };
        if setup.record_runtime != spec.setup_runtime
            || setup.table_runtime != spec.table_runtime
            || setup.atlas_width != 768
            || setup.atlas_height != 256
            || !setup.review_layout_matches
            || !setup.descriptor_plain_layout_matches
        {
            return Err(error(format!(
                "{} BOXOPT setup 사양 불일치: setup={setup:?}",
                spec.exe
            )));
        }

        let raw = executable_slice(
            map,
            text,
            spec.table_runtime,
            BOXOPT_SELECTOR_TABLE_BYTES + 12,
        )?;
        let table = parse_boxopt_selector_table(raw)?;
        if let Some(common) = &common_table {
            if common != &table {
                return Err(error(format!(
                    "{} BOXOPT selector 표가 공통 원본과 다름",
                    spec.exe
                )));
            }
        } else {
            common_table = Some(table);
        }
        for site in spec.renderer_sites {
            verify_renderer_site(map, text, spec.descriptor_root_pointer_runtime, site)?;
        }
        owners.push(BoxoptStaticOwner {
            exe: spec.exe,
            archive: spec.archive,
            resource_id: spec.resource_id,
            descriptor_runtime: spec.descriptor_runtime,
            descriptor_selection_runtime: selection.descriptor_select_runtime,
            setup_runtime: spec.setup_runtime,
            table_runtime: spec.table_runtime,
            renderer_resource_loads: spec.renderer_sites.map(|site| site.resource_load_runtime),
            renderer_selector_loads: spec
                .renderer_sites
                .map(|site| site.resource_load_runtime + 0x18),
            renderer_table_loads: spec
                .renderer_sites
                .map(|site| site.resource_load_runtime + 0x28),
        });
    }
    owners.sort_by_key(|owner| owner.exe);
    if owners
        .iter()
        .map(|owner| owner.exe)
        .collect::<BTreeSet<_>>()
        .len()
        != BOXOPT_SPECS.len()
    {
        return Err(error("BOXOPT owner EXE가 중복됨"));
    }
    let table = common_table.ok_or_else(|| error("BOXOPT 공통 selector 표가 없음"))?;
    Ok(BoxoptStaticAnalysis {
        owners,
        selectors: table.selectors,
        selector_table_sha256: BOXOPT_SELECTOR_TABLE_SHA256,
        selector_slots: BOXOPT_SELECTOR_SLOTS,
        asset_backed_selectors: BOXOPT_ASSET_BACKED_SELECTORS,
        empty_selectors: BOXOPT_EMPTY_SELECTORS,
        phrase_selectors: BOXOPT_PHRASE_SELECTORS,
        translation_target_selectors: BOXOPT_TRANSLATION_TARGET_SELECTORS,
        overlap_groups: BOXOPT_OVERLAP_GROUPS,
        phrase_selector_pixels: table.phrase_selector_pixels,
        phrase_surface_union_pixels: table.phrase_surface_union_pixels,
        first_non_asset_selector: table.first_non_asset_selector.index,
    })
}

#[cfg(test)]
#[path = "boxopt_static_tests.rs"]
mod tests;
