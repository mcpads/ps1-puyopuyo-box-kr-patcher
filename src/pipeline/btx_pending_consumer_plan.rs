//! fixed-cell 승격 전 일본어 양성 BTX의 정적 소유권과 runtime 조사 순서.
//!
//! 같은 해제 픽셀을 공유해도 물리 사본의 소비는 EXE별로 따로 증명해야 한다.
//! 이 모듈은 승격 provenance의 정확한 `archive/name`을 BTX descriptor 분석과
//! 교차하고, 고유 finding을 먼저 덮는 대표 실행과 남은 사본 실행을 나눈다.
//! 현재 큐가 비어도 provenance를 재현한다. descriptor 정적 연결은 runtime
//! 적재나 화면 표시 증거가 아니다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::disasm::{
    BtxDescriptorSelection, BtxDescriptorSelectionAnalysis, BtxNameAnalysis, BtxNameLink,
    BOX_BTX_NAME_CONSUMERS,
};
use crate::graphics::text_candidates::{
    verify_pending_text_catalog_specs, PendingTextAssetSpec, PendingTextError,
    BOX_PROMOTED_CELL_GRID_PROVENANCE,
};

pub const BTX_PENDING_CONSUMER_PLAN_SCHEMA_VERSION: &str = "btx-pending-consumer-plan-v2";
pub const BOX_BTX_PENDING_CONSUMER_REPORT_SHA256: [u8; 32] = [
    0x30, 0x77, 0xAE, 0x0E, 0xF2, 0x53, 0xC4, 0xDD, 0xEA, 0xAF, 0x3D, 0xB3, 0x41, 0x9F, 0x54, 0x0F,
    0x86, 0x43, 0x36, 0x58, 0x8F, 0xEE, 0x93, 0xEF, 0x99, 0xF5, 0x13, 0x5F, 0x86, 0xC5, 0x2D, 0xA4,
];

const BOX_PENDING_PHYSICAL_ROWS: usize = 4;
const BOX_PENDING_EXECUTION_BOUNDARIES: usize = 2;
const BOX_PENDING_REPRESENTATIVE_BOUNDARIES: usize = 1;

const BOX_PENDING_REPRESENTATIVE_ARCHIVES: [&str; 1] = ["/PUYO2/PP2_GRP.VDD;1"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtxPendingRuntimePhase {
    /// 고유 finding을 최소 실행 집합으로 먼저 검증한다.
    Representative,
    /// 같은 픽셀 내용의 나머지 물리 사본을 EXE별로 검증한다.
    PhysicalCopyFollowup,
}

impl BtxPendingRuntimePhase {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Representative => "representative",
            Self::PhysicalCopyFollowup => "physical-copy-followup",
        }
    }

    pub const fn next_gate(self) -> &'static str {
        match self {
            Self::Representative => {
                "fresh-entry-exact-descriptor-read-breakpoint-then-screen-and-gpu"
            }
            Self::PhysicalCopyFollowup => "after-representative-run-same-exact-physical-copy-gate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxPendingConsumerRow {
    pub phase: BtxPendingRuntimePhase,
    pub finding_id: String,
    pub exe: &'static str,
    pub archive: &'static str,
    pub asset_name: String,
    pub descriptor_block_static_evidence: &'static str,
    pub descriptor_entry_runtime: u32,
    pub descriptor_name_or_lba_field_runtime: u32,
    pub descriptor_runtime_start: u32,
    pub descriptor_runtime_end: u32,
    pub descriptor_block_entry_ref_sites: Vec<u32>,
    pub resource_multiply_runtime: u32,
    pub descriptor_select_runtime: u32,
    pub selection_cache_check_runtime: u32,
    pub selection_copy_runtime: u32,
    pub selection_name_field_read_pc: u32,
    pub selection_copy_end_runtime: u32,
    pub selection_copy_source_register: crate::isa::Reg,
    pub working_descriptor_runtime: u32,
    pub selection_entry_reached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxPendingConsumerPlan {
    pub rows: Vec<BtxPendingConsumerRow>,
    pub findings: usize,
    pub execution_boundaries: usize,
    pub representative_boundaries: usize,
    pub physical_copy_followup_boundaries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BtxPendingConsumerPlanError {
    PendingCatalog(PendingTextError),
    DuplicateArchiveOwner(&'static str),
    MissingArchiveOwner(String),
    MissingPhysicalLink {
        exe: &'static str,
        archive: String,
        asset_name: String,
    },
    DuplicatePhysicalLink {
        exe: &'static str,
        archive: String,
        asset_name: String,
        count: usize,
    },
    MissingEntryDescriptorReference {
        exe: &'static str,
        archive: String,
        asset_name: String,
    },
    MissingDescriptorSelection {
        exe: &'static str,
        archive: String,
    },
    DuplicateDescriptorSelection {
        exe: &'static str,
        archive: String,
        count: usize,
    },
    DescriptorEntryUnderflow {
        exe: &'static str,
        archive: String,
        asset_name: String,
    },
    NoRepresentativeCover,
    BoxCount {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    BoxRepresentativeArchives {
        expected: Vec<&'static str>,
        actual: Vec<&'static str>,
    },
    BoxReportDigestMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

impl fmt::Display for BtxPendingConsumerPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PendingCatalog(source) => write!(f, "pending BTX catalog 실패: {source}"),
            Self::DuplicateArchiveOwner(archive) => {
                write!(f, "BTX archive의 EXE owner가 중복됨: {archive}")
            }
            Self::MissingArchiveOwner(archive) => {
                write!(f, "pending BTX archive의 EXE owner가 없음: {archive}")
            }
            Self::MissingPhysicalLink {
                exe,
                archive,
                asset_name,
            } => write!(
                f,
                "pending BTX의 정적 이름표 연결이 없음: {exe}:{archive}:{asset_name}"
            ),
            Self::DuplicatePhysicalLink {
                exe,
                archive,
                asset_name,
                count,
            } => write!(
                f,
                "pending BTX의 정적 이름표 연결이 중복됨: {exe}:{archive}:{asset_name}: {count}"
            ),
            Self::MissingEntryDescriptorReference {
                exe,
                archive,
                asset_name,
            } => write!(
                f,
                "pending BTX descriptor block의 entry trace 참조가 없음: \
                 {exe}:{archive}:{asset_name}"
            ),
            Self::MissingDescriptorSelection { exe, archive } => write!(
                f,
                "pending BTX의 개별 descriptor 선택 경계가 없음: {exe}:{archive}"
            ),
            Self::DuplicateDescriptorSelection {
                exe,
                archive,
                count,
            } => write!(
                f,
                "pending BTX의 개별 descriptor 선택 경계가 중복됨: \
                 {exe}:{archive}: {count}"
            ),
            Self::DescriptorEntryUnderflow {
                exe,
                archive,
                asset_name,
            } => write!(
                f,
                "pending BTX descriptor +4 field에서 행 시작을 계산할 수 없음: \
                 {exe}:{archive}:{asset_name}"
            ),
            Self::NoRepresentativeCover => {
                f.write_str("pending BTX 고유 finding을 덮는 실행 경계 집합이 없음")
            }
            Self::BoxCount {
                field,
                expected,
                actual,
            } => write!(
                f,
                "지원 원본 pending BTX {field} 불일치: expected={expected}, actual={actual}"
            ),
            Self::BoxRepresentativeArchives { expected, actual } => write!(
                f,
                "지원 원본 pending BTX 대표 archive 불일치: \
                 expected={}, actual={}",
                expected.join(","),
                actual.join(",")
            ),
            Self::BoxReportDigestMismatch { expected, actual } => write!(
                f,
                "지원 원본 pending BTX consumer report SHA-256 불일치: \
                 expected={expected:02x?}, actual={actual:02x?}"
            ),
        }
    }
}

impl Error for BtxPendingConsumerPlanError {}

impl From<PendingTextError> for BtxPendingConsumerPlanError {
    fn from(source: PendingTextError) -> Self {
        Self::PendingCatalog(source)
    }
}

fn exact_link<'a>(
    analysis: &'a BtxNameAnalysis,
    exe: &'static str,
    archive: &str,
    asset_name: &str,
) -> Result<&'a BtxNameLink, BtxPendingConsumerPlanError> {
    let links = analysis
        .links
        .iter()
        .filter(|link| link.exe == exe && link.archive == archive && link.asset_name == asset_name)
        .collect::<Vec<_>>();
    match links.as_slice() {
        [] => Err(BtxPendingConsumerPlanError::MissingPhysicalLink {
            exe,
            archive: archive.to_string(),
            asset_name: asset_name.to_string(),
        }),
        [link] => Ok(*link),
        _ => Err(BtxPendingConsumerPlanError::DuplicatePhysicalLink {
            exe,
            archive: archive.to_string(),
            asset_name: asset_name.to_string(),
            count: links.len(),
        }),
    }
}

fn exact_selection<'a>(
    analysis: &'a BtxDescriptorSelectionAnalysis,
    exe: &'static str,
    archive: &str,
) -> Result<&'a BtxDescriptorSelection, BtxPendingConsumerPlanError> {
    let selections = analysis
        .selections
        .iter()
        .filter(|selection| selection.exe == exe && selection.archive == archive)
        .collect::<Vec<_>>();
    match selections.as_slice() {
        [] => Err(BtxPendingConsumerPlanError::MissingDescriptorSelection {
            exe,
            archive: archive.to_string(),
        }),
        [selection] => Ok(*selection),
        _ => Err(BtxPendingConsumerPlanError::DuplicateDescriptorSelection {
            exe,
            archive: archive.to_string(),
            count: selections.len(),
        }),
    }
}

fn choose_representative_archives(
    boundary_findings: &BTreeMap<&'static str, BTreeSet<String>>,
) -> Option<BTreeSet<&'static str>> {
    let mut archives = boundary_findings.keys().copied().collect::<Vec<_>>();
    archives.sort_by_key(|archive| (*archive != "/PUYO2/PP2_GRP.VDD;1", archive.to_string()));
    if archives.len() >= usize::BITS as usize {
        return None;
    }
    let all_findings = boundary_findings
        .values()
        .flatten()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut best: Option<(usize, usize, usize, BTreeSet<&'static str>)> = None;
    for mask in 1usize..(1usize << archives.len()) {
        let selected = archives
            .iter()
            .enumerate()
            .filter_map(|(index, archive)| ((mask & (1 << index)) != 0).then_some(*archive))
            .collect::<BTreeSet<_>>();
        let covered_findings = selected
            .iter()
            .flat_map(|archive| boundary_findings[archive].iter().cloned())
            .collect::<BTreeSet<_>>();
        if covered_findings != all_findings {
            continue;
        }
        let boundary_count = selected.len();
        let physical_rows = selected
            .iter()
            .map(|archive| boundary_findings[archive].len())
            .sum::<usize>();
        let score = (boundary_count, usize::MAX - physical_rows, mask);
        if best
            .as_ref()
            .is_none_or(|(best_count, best_rows, best_mask, _)| {
                score < (*best_count, *best_rows, *best_mask)
            })
        {
            best = Some((score.0, score.1, score.2, selected));
        }
    }
    best.map(|(_, _, _, selected)| selected)
}

/// pending catalog와 전수 BTX 이름표 분석을 교차해 실행 조사 순서를 만든다.
pub fn audit_btx_pending_consumer_plan(
    specs: &[PendingTextAssetSpec<'_>],
    name_analysis: &BtxNameAnalysis,
    selection_analysis: &BtxDescriptorSelectionAnalysis,
) -> Result<BtxPendingConsumerPlan, BtxPendingConsumerPlanError> {
    let catalog = verify_pending_text_catalog_specs(specs)?;
    let mut archive_owners = BTreeMap::new();
    for owner in BOX_BTX_NAME_CONSUMERS {
        if archive_owners.insert(owner.archive, owner.exe).is_some() {
            return Err(BtxPendingConsumerPlanError::DuplicateArchiveOwner(
                owner.archive,
            ));
        }
    }

    let mut boundary_findings = BTreeMap::<&'static str, BTreeSet<String>>::new();
    for spec in specs {
        for &archive in spec.archive_paths {
            let Some((&owned_archive, &exe)) = archive_owners.get_key_value(archive) else {
                return Err(BtxPendingConsumerPlanError::MissingArchiveOwner(
                    archive.to_string(),
                ));
            };
            let link = exact_link(name_analysis, exe, owned_archive, spec.name)?;
            exact_selection(selection_analysis, exe, owned_archive)?;
            if link.descriptor_entry_reached_ref_sites.is_empty() {
                return Err(
                    BtxPendingConsumerPlanError::MissingEntryDescriptorReference {
                        exe,
                        archive: archive.to_string(),
                        asset_name: spec.name.to_string(),
                    },
                );
            }
            boundary_findings
                .entry(owned_archive)
                .or_default()
                .insert(spec.finding_id.to_string());
        }
    }

    let representative_archives = choose_representative_archives(&boundary_findings)
        .ok_or(BtxPendingConsumerPlanError::NoRepresentativeCover)?;
    let mut rows = Vec::with_capacity(catalog.declared_physical_assets);
    for spec in specs {
        for &archive in spec.archive_paths {
            let (&owned_archive, &exe) = archive_owners
                .get_key_value(archive)
                .expect("owner 누락은 대표 집합 계산 전에 거부했다");
            let link = exact_link(name_analysis, exe, owned_archive, spec.name)?;
            let selection = exact_selection(selection_analysis, exe, owned_archive)?;
            let descriptor_entry_runtime = link.descriptor_pointer_runtime.checked_sub(4).ok_or(
                BtxPendingConsumerPlanError::DescriptorEntryUnderflow {
                    exe,
                    archive: owned_archive.to_string(),
                    asset_name: spec.name.to_string(),
                },
            )?;
            rows.push(BtxPendingConsumerRow {
                phase: if representative_archives.contains(owned_archive) {
                    BtxPendingRuntimePhase::Representative
                } else {
                    BtxPendingRuntimePhase::PhysicalCopyFollowup
                },
                finding_id: spec.finding_id.to_string(),
                exe,
                archive: owned_archive,
                asset_name: spec.name.to_string(),
                descriptor_block_static_evidence: link.static_evidence(),
                descriptor_entry_runtime,
                descriptor_name_or_lba_field_runtime: link.descriptor_pointer_runtime,
                descriptor_runtime_start: link.descriptor_runtime_start,
                descriptor_runtime_end: link.descriptor_runtime_end,
                descriptor_block_entry_ref_sites: link.descriptor_entry_reached_ref_sites.clone(),
                resource_multiply_runtime: selection.resource_multiply_runtime,
                descriptor_select_runtime: selection.descriptor_select_runtime,
                selection_cache_check_runtime: selection.cache_check_runtime,
                selection_copy_runtime: selection.descriptor_copy_runtime,
                selection_name_field_read_pc: selection.descriptor_name_field_read_runtime,
                selection_copy_end_runtime: selection.descriptor_copy_end_runtime,
                selection_copy_source_register: selection.descriptor_copy_source_register,
                working_descriptor_runtime: selection.working_descriptor_runtime,
                selection_entry_reached: selection.entry_reached,
            });
        }
    }
    rows.sort_by(|left, right| {
        left.finding_id
            .cmp(&right.finding_id)
            .then_with(|| left.archive.cmp(right.archive))
    });

    Ok(BtxPendingConsumerPlan {
        rows,
        findings: catalog.assets,
        execution_boundaries: boundary_findings.len(),
        representative_boundaries: representative_archives.len(),
        physical_copy_followup_boundaries: boundary_findings.len() - representative_archives.len(),
    })
}

/// 지원 원본의 2 finding·4 physical·2 EXE/GRP 경계를 고정한다.
pub fn verify_box_btx_pending_consumer_plan(
    name_analysis: &BtxNameAnalysis,
    selection_analysis: &BtxDescriptorSelectionAnalysis,
) -> Result<BtxPendingConsumerPlan, BtxPendingConsumerPlanError> {
    let plan = audit_btx_pending_consumer_plan(
        &BOX_PROMOTED_CELL_GRID_PROVENANCE,
        name_analysis,
        selection_analysis,
    )?;
    let counts = [
        ("finding", 2, plan.findings),
        ("physical row", BOX_PENDING_PHYSICAL_ROWS, plan.rows.len()),
        (
            "execution boundary",
            BOX_PENDING_EXECUTION_BOUNDARIES,
            plan.execution_boundaries,
        ),
        (
            "representative boundary",
            BOX_PENDING_REPRESENTATIVE_BOUNDARIES,
            plan.representative_boundaries,
        ),
        (
            "physical copy followup boundary",
            BOX_PENDING_EXECUTION_BOUNDARIES - BOX_PENDING_REPRESENTATIVE_BOUNDARIES,
            plan.physical_copy_followup_boundaries,
        ),
    ];
    for (field, expected, actual) in counts {
        if actual != expected {
            return Err(BtxPendingConsumerPlanError::BoxCount {
                field,
                expected,
                actual,
            });
        }
    }
    let actual_representatives = plan
        .rows
        .iter()
        .filter(|row| row.phase == BtxPendingRuntimePhase::Representative)
        .map(|row| row.archive)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut expected_representatives = BOX_PENDING_REPRESENTATIVE_ARCHIVES.to_vec();
    expected_representatives.sort_unstable();
    if actual_representatives != expected_representatives {
        return Err(BtxPendingConsumerPlanError::BoxRepresentativeArchives {
            expected: expected_representatives,
            actual: actual_representatives,
        });
    }
    let report = render_btx_pending_consumer_plan_tsv(&plan);
    let actual_digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if actual_digest != BOX_BTX_PENDING_CONSUMER_REPORT_SHA256 {
        return Err(BtxPendingConsumerPlanError::BoxReportDigestMismatch {
            expected: BOX_BTX_PENDING_CONSUMER_REPORT_SHA256,
            actual: actual_digest,
        });
    }
    Ok(plan)
}

fn hex_addresses(addresses: &[u32]) -> String {
    addresses
        .iter()
        .map(|address| format!("0x{address:08X}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// 물리 pending BTX별 정적 owner와 다음 runtime gate의 결정적 TSV.
pub fn render_btx_pending_consumer_plan_tsv(plan: &BtxPendingConsumerPlan) -> String {
    let mut output = String::from(
        "schema_version\tphase\tfinding_id\texe\tarchive\tasset_name\tstatic_ownership\t\
         descriptor_block_static_evidence\tdescriptor_entry_runtime\t\
         descriptor_name_or_lba_field_runtime\tdescriptor_runtime_start\t\
         descriptor_runtime_end\tdescriptor_block_entry_ref_sites\t\
         selection_static_evidence\tresource_multiply_runtime\tdescriptor_select_runtime\t\
         selection_cache_check_runtime\tselection_copy_runtime\t\
         selection_name_field_read_pc\tselection_copy_end_runtime\t\
         selection_copy_source_register\tworking_descriptor_runtime\t\
         selection_entry_reached\truntime_verdict\tnegative_result_limit\tnext_gate\n",
    );
    for row in &plan.rows {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\texact-archive-name-descriptor\t{}\t0x{:08X}\t\
             0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\tresource-x36-cache-guard-copy36\t\
             0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t\
             0x{:08X}\t{}\tunverified\tcache-hit-can-bypass-copy-so-missing-hit-is-not-absence\t{}",
            BTX_PENDING_CONSUMER_PLAN_SCHEMA_VERSION,
            row.phase.label(),
            row.finding_id,
            row.exe,
            row.archive,
            row.asset_name,
            row.descriptor_block_static_evidence,
            row.descriptor_entry_runtime,
            row.descriptor_name_or_lba_field_runtime,
            row.descriptor_runtime_start,
            row.descriptor_runtime_end,
            hex_addresses(&row.descriptor_block_entry_ref_sites),
            row.resource_multiply_runtime,
            row.descriptor_select_runtime,
            row.selection_cache_check_runtime,
            row.selection_copy_runtime,
            row.selection_name_field_read_pc,
            row.selection_copy_end_runtime,
            row.selection_copy_source_register,
            row.working_descriptor_runtime,
            row.selection_entry_reached,
            row.phase.next_gate(),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

#[cfg(test)]
#[path = "btx_pending_consumer_plan_tests.rs"]
mod tests;
