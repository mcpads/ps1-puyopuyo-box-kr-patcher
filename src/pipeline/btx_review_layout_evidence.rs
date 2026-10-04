//! 사람 판독이 불충분한 BTX family의 정적 layout 근거를 모은다.
//!
//! 판독용 행 감기 폭은 소비자 layout이 아니다. 이 모듈은 현재 검토 원장에서
//! `review-insufficient`인 family만 골라 각 물리 사본의 이름표와 초기
//! descriptor를 다시 검증하고, 자기 EXE와 대응하는 SEQ overlay의
//! `OP28/sub12` setup과 알려진 일곱 selector 형식을 찾는다. descriptor의
//! 해제 크기와 맞는 setup만 layout 근거로 세고, selector는 record type 0인
//! GRP 선택과 같은 숫자의 다른 resource 공간을 분리한다. 어느 결과도 텍스트
//! 여부나 runtime 소비 증거로 올리지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::btx::PixelFormat;
use crate::coords::ExeMap;
use crate::disasm::{trace, BtxDescriptorSelectionAnalysis, BtxNameAnalysis, BtxNameLink};
use crate::graphics::btx_family_candidates::{
    BtxCandidateFamily, BtxFamilyCandidate, BtxFamilyCandidateAnalysis, BtxPhysicalCandidateCopy,
};
use crate::review::{BtxFamilyReviewAnalysis, BtxFamilyReviewDecisionKind};
use crate::script::subgame_selector_records::{
    scan_subgame_selector_records, SubgameSelectorRecordForm,
};
use crate::script::{SubgameSeqLayoutAnalysis, SubgameSeqLayoutKind};

use super::btx_pending_static_cells::{
    descriptor_entry_runtime, resource_id, scan_setup_records, verify_initial_descriptor,
    BtxPendingStaticCellError, SetupScanContext,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BtxReviewSetupStorage {
    Executable,
    SequenceOverlay,
}

impl BtxReviewSetupStorage {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Executable => "executable",
            Self::SequenceOverlay => "sequence-overlay",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtxReviewLayoutResolution {
    ExactAcrossCopies,
    RowPaddedAcrossCopies,
    PartialAcrossCopies,
    MultipleLayouts,
    NoCompatibleSetup,
}

impl BtxReviewLayoutResolution {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ExactAcrossCopies => "exact-across-copies",
            Self::RowPaddedAcrossCopies => "row-padded-across-copies",
            Self::PartialAcrossCopies => "partial-across-copies",
            Self::MultipleLayouts => "multiple-layouts",
            Self::NoCompatibleSetup => "no-compatible-setup",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BtxDescriptorStorageLayout {
    pub width: u16,
    pub stored_height: u16,
    pub atlas_height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxReviewSetupEvidence {
    pub storage: BtxReviewSetupStorage,
    pub source: String,
    pub record_runtime: u32,
    pub table_runtime: u32,
    pub atlas_width: u16,
    pub atlas_height: u16,
    pub review_layout_matches: bool,
    pub descriptor_plain_layout_matches: bool,
    pub descriptor_storage_height: Option<u16>,
    pub descriptor_padding_rows: Option<u16>,
}

/// 같은 resource 번호를 담은 알려진 selector record의 정적 위치.
///
/// `record_type == 0`만 현재 GRP descriptor table을 고른다. 다른 type에서
/// 숫자가 같은 record는 다른 자원 공간의 일치이므로 그래픽 소비 근거가
/// 아니다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxReviewSelectorEvidence {
    pub storage: BtxReviewSetupStorage,
    pub source: String,
    pub record_runtime: u32,
    pub form: SubgameSelectorRecordForm,
    pub flags: Option<u8>,
    pub record_type: u8,
    /// EXE entry 재귀 추적에서 이 시작 주소가 실제 명령으로 해석됐는가.
    /// SEQ overlay는 이 모듈이 entry trace를 소유하지 않으므로 `None`이다.
    pub entry_trace_instruction: Option<bool>,
}

impl BtxReviewSelectorEvidence {
    pub fn is_record_candidate(&self) -> bool {
        self.entry_trace_instruction != Some(true)
    }

    pub fn selects_graphics_archive(&self) -> bool {
        self.is_record_candidate() && self.record_type == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxReviewLayoutCopyEvidence {
    pub family_key: String,
    pub candidate_id: String,
    pub decompressed_sha256: String,
    pub format: PixelFormat,
    pub decompressed_bytes: usize,
    pub review_width: usize,
    pub review_height: usize,
    pub exe: &'static str,
    pub archive: String,
    pub asset_name: String,
    pub vdd_entry_index: usize,
    pub descriptor_resource_id: u16,
    pub descriptor_runtime: u32,
    pub descriptor_selection_runtime: Option<u32>,
    pub setups: Vec<BtxReviewSetupEvidence>,
    pub selectors: Vec<BtxReviewSelectorEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxReviewLayoutCandidateEvidence {
    pub family_key: String,
    pub candidate_id: String,
    pub decompressed_sha256: String,
    pub physical_copies: usize,
    pub copies_with_setup: usize,
    pub copies_with_descriptor_sized_setup: usize,
    pub copies_with_descriptor_compatible_setup: usize,
    pub copies_with_graphics_selector: usize,
    pub graphics_selector_records: usize,
    pub other_type_numeric_matches: usize,
    pub entry_trace_code_matches: usize,
    pub descriptor_sized_layouts: Vec<(u16, u16)>,
    pub descriptor_storage_layouts: Vec<BtxDescriptorStorageLayout>,
    pub resolution: BtxReviewLayoutResolution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxReviewLayoutEvidenceAnalysis {
    pub copies: Vec<BtxReviewLayoutCopyEvidence>,
    pub candidates: Vec<BtxReviewLayoutCandidateEvidence>,
    pub families: usize,
    pub unique_contents: usize,
    pub physical_copies: usize,
    pub copies_with_setup: usize,
    pub copies_with_descriptor_sized_setup: usize,
    pub copies_with_descriptor_compatible_setup: usize,
    pub copies_with_graphics_selector: usize,
    pub graphics_selector_records: usize,
    pub other_type_numeric_matches: usize,
    pub entry_trace_code_matches: usize,
    pub exact_candidates: usize,
    pub row_padded_candidates: usize,
    pub partial_candidates: usize,
    pub multiple_layout_candidates: usize,
    pub candidates_without_compatible_setup: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxReviewLayoutEvidenceError(String);

impl fmt::Display for BtxReviewLayoutEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "BTX review layout 근거 감사 실패: {}", self.0)
    }
}

impl Error for BtxReviewLayoutEvidenceError {}

impl From<BtxPendingStaticCellError> for BtxReviewLayoutEvidenceError {
    fn from(source: BtxPendingStaticCellError) -> Self {
        Self(source.to_string())
    }
}

fn error(message: impl Into<String>) -> BtxReviewLayoutEvidenceError {
    BtxReviewLayoutEvidenceError(message.into())
}

fn exact_name_link<'a>(
    names: &'a BtxNameAnalysis,
    copy: &BtxPhysicalCandidateCopy,
) -> Result<&'a BtxNameLink, BtxReviewLayoutEvidenceError> {
    let links = names
        .links
        .iter()
        .filter(|link| link.archive == copy.archive && link.asset_name == copy.name)
        .collect::<Vec<_>>();
    match links.as_slice() {
        [link] => Ok(*link),
        _ => Err(error(format!(
            "물리 BTX 이름표 연결 수가 1이 아님: {}:{}: {}",
            copy.archive,
            copy.name,
            links.len()
        ))),
    }
}

fn review_insufficient_members<'a>(
    candidates: &'a BtxFamilyCandidateAnalysis,
    review: &BtxFamilyReviewAnalysis,
) -> Result<Vec<(&'a BtxCandidateFamily, &'a BtxFamilyCandidate)>, BtxReviewLayoutEvidenceError> {
    let insufficient_keys = review
        .decisions
        .iter()
        .filter(|decision| decision.decision == BtxFamilyReviewDecisionKind::ReviewInsufficient)
        .map(|decision| decision.family_key.as_str())
        .collect::<BTreeSet<_>>();
    if insufficient_keys.len() != review.review_insufficient {
        return Err(error(format!(
            "review-insufficient 집계 불일치: decisions={}, summary={}",
            insufficient_keys.len(),
            review.review_insufficient
        )));
    }

    let mut targets = Vec::new();
    for family_key in insufficient_keys {
        let families = candidates
            .families
            .iter()
            .filter(|family| family.family_key == family_key)
            .collect::<Vec<_>>();
        let [family] = families.as_slice() else {
            return Err(error(format!(
                "review-insufficient family의 현재 후보가 하나가 아님: {family_key}: {}",
                families.len()
            )));
        };
        let members = candidates
            .candidates
            .iter()
            .filter(|candidate| candidate.family_key == family_key)
            .collect::<Vec<_>>();
        let physical_copies = members
            .iter()
            .map(|candidate| candidate.physical_copies.len())
            .sum::<usize>();
        if members.len() != family.members || physical_copies != family.physical_copies {
            return Err(error(format!(
                "{family_key} family 분모 불일치: family={}/{}, candidates={}/{physical_copies}",
                family.members,
                family.physical_copies,
                members.len()
            )));
        }
        targets.extend(members.into_iter().map(|member| (*family, member)));
    }
    targets.sort_by(|left, right| {
        left.0
            .family_key
            .cmp(&right.0.family_key)
            .then_with(|| left.1.decompressed_sha256.cmp(&right.1.decompressed_sha256))
    });
    Ok(targets)
}

fn paired_sequence_archive(grp_archive: &str, available: &BTreeSet<&str>) -> Option<String> {
    let prefix = grp_archive.strip_suffix("GRP.VDD;1")?;
    let sequence = format!("{prefix}SEQ.VDD;1");
    available.contains(sequence.as_str()).then_some(sequence)
}

fn descriptor_storage_layout(
    format: PixelFormat,
    descriptor_plain_bytes: usize,
    atlas_width: u16,
    atlas_height: u16,
) -> Option<BtxDescriptorStorageLayout> {
    let pixels_per_byte = match format {
        PixelFormat::Indexed4 => 2usize,
        PixelFormat::Indexed8 => 1usize,
    };
    let pixels = descriptor_plain_bytes.checked_mul(pixels_per_byte)?;
    let width = usize::from(atlas_width);
    if width == 0 || !pixels.is_multiple_of(width) {
        return None;
    }
    let stored_height = u16::try_from(pixels / width).ok()?;
    (stored_height != 0 && stored_height <= atlas_height).then_some(BtxDescriptorStorageLayout {
        width: atlas_width,
        stored_height,
        atlas_height,
    })
}

fn setup_evidence(
    storage: BtxReviewSetupStorage,
    source: String,
    setup: super::btx_pending_static_cells::BtxPendingStaticSetup,
    format: PixelFormat,
    descriptor_plain_bytes: usize,
) -> BtxReviewSetupEvidence {
    let storage_layout = descriptor_storage_layout(
        format,
        descriptor_plain_bytes,
        setup.atlas_width,
        setup.atlas_height,
    );
    BtxReviewSetupEvidence {
        storage,
        source,
        record_runtime: setup.record_runtime,
        table_runtime: setup.table_runtime,
        atlas_width: setup.atlas_width,
        atlas_height: setup.atlas_height,
        review_layout_matches: setup.review_layout_matches,
        descriptor_plain_layout_matches: setup.descriptor_plain_layout_matches,
        descriptor_storage_height: storage_layout.map(|layout| layout.stored_height),
        descriptor_padding_rows: storage_layout
            .map(|layout| layout.atlas_height - layout.stored_height),
    }
}

fn scan_selector_records(
    storage: BtxReviewSetupStorage,
    source: &str,
    raw: &[u8],
    initialized_len: usize,
    load_base: u32,
    resource_id: u16,
    entry_trace: Option<&BTreeSet<u32>>,
) -> Result<Vec<BtxReviewSelectorEvidence>, BtxReviewLayoutEvidenceError> {
    let scan_len = initialized_len.min(raw.len());
    let mut records = Vec::new();
    for record in scan_subgame_selector_records(&raw[..scan_len]) {
        if !record.offset.is_multiple_of(4) || record.resource_id != resource_id {
            continue;
        }
        let offset = u32::try_from(record.offset)
            .map_err(|_| error(format!("{source} selector 위치가 u32를 넘음")))?;
        let record_runtime = load_base
            .checked_add(offset)
            .ok_or_else(|| error(format!("{source} selector runtime 주소 overflow")))?;
        records.push(BtxReviewSelectorEvidence {
            storage,
            source: source.to_owned(),
            record_runtime,
            form: record.form,
            flags: record.flags,
            record_type: record.record_type,
            entry_trace_instruction: entry_trace
                .map(|instructions| instructions.contains(&record_runtime)),
        });
    }
    Ok(records)
}

fn summarize_candidate_layouts(
    candidate: &BtxFamilyCandidate,
    copies: &[BtxReviewLayoutCopyEvidence],
) -> Result<BtxReviewLayoutCandidateEvidence, BtxReviewLayoutEvidenceError> {
    if copies.len() != candidate.physical_copies.len()
        || copies.iter().any(|copy| {
            copy.candidate_id != candidate.candidate_id
                || copy.decompressed_sha256 != candidate.decompressed_sha256
        })
    {
        return Err(error(format!(
            "{} 물리 사본 요약 입력 불일치",
            candidate.candidate_id
        )));
    }
    let copies_with_setup = copies.iter().filter(|copy| !copy.setups.is_empty()).count();
    let copies_with_descriptor_sized_setup = copies
        .iter()
        .filter(|copy| {
            copy.setups
                .iter()
                .any(|setup| setup.descriptor_plain_layout_matches)
        })
        .count();
    let copies_with_descriptor_compatible_setup = copies
        .iter()
        .filter(|copy| {
            copy.setups
                .iter()
                .any(|setup| setup.descriptor_storage_height.is_some())
        })
        .count();
    let copies_with_graphics_selector = copies
        .iter()
        .filter(|copy| {
            copy.selectors
                .iter()
                .any(BtxReviewSelectorEvidence::selects_graphics_archive)
        })
        .count();
    let graphics_selector_records = copies
        .iter()
        .flat_map(|copy| &copy.selectors)
        .filter(|selector| selector.selects_graphics_archive())
        .count();
    let other_type_numeric_matches = copies
        .iter()
        .flat_map(|copy| &copy.selectors)
        .filter(|selector| selector.is_record_candidate() && !selector.selects_graphics_archive())
        .count();
    let entry_trace_code_matches = copies
        .iter()
        .flat_map(|copy| &copy.selectors)
        .filter(|selector| !selector.is_record_candidate())
        .count();
    let descriptor_sized_layouts = copies
        .iter()
        .flat_map(|copy| &copy.setups)
        .filter(|setup| setup.descriptor_plain_layout_matches)
        .map(|setup| (setup.atlas_width, setup.atlas_height))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let descriptor_storage_layouts = copies
        .iter()
        .flat_map(|copy| &copy.setups)
        .filter_map(|setup| {
            setup
                .descriptor_storage_height
                .map(|stored_height| BtxDescriptorStorageLayout {
                    width: setup.atlas_width,
                    stored_height,
                    atlas_height: setup.atlas_height,
                })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let resolution = if descriptor_storage_layouts.len() > 1 {
        BtxReviewLayoutResolution::MultipleLayouts
    } else if copies_with_descriptor_compatible_setup == copies.len() && !copies.is_empty() {
        if descriptor_storage_layouts
            .iter()
            .any(|layout| layout.stored_height != layout.atlas_height)
        {
            BtxReviewLayoutResolution::RowPaddedAcrossCopies
        } else {
            BtxReviewLayoutResolution::ExactAcrossCopies
        }
    } else if copies_with_descriptor_compatible_setup != 0 {
        BtxReviewLayoutResolution::PartialAcrossCopies
    } else {
        BtxReviewLayoutResolution::NoCompatibleSetup
    };
    Ok(BtxReviewLayoutCandidateEvidence {
        family_key: candidate.family_key.clone(),
        candidate_id: candidate.candidate_id.clone(),
        decompressed_sha256: candidate.decompressed_sha256.clone(),
        physical_copies: copies.len(),
        copies_with_setup,
        copies_with_descriptor_sized_setup,
        copies_with_descriptor_compatible_setup,
        copies_with_graphics_selector,
        graphics_selector_records,
        other_type_numeric_matches,
        entry_trace_code_matches,
        descriptor_sized_layouts,
        descriptor_storage_layouts,
        resolution,
    })
}

/// 현재 판단 불충분 family의 descriptor-sized 정적 setup layout을 감사한다.
pub fn audit_btx_review_layout_evidence<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sequence_sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    names: &BtxNameAnalysis,
    selections: &BtxDescriptorSelectionAnalysis,
    candidates: &BtxFamilyCandidateAnalysis,
    review: &BtxFamilyReviewAnalysis,
) -> Result<BtxReviewLayoutEvidenceAnalysis, BtxReviewLayoutEvidenceError> {
    let executable_map = executables
        .into_iter()
        .map(|(map, raw)| (map.name, (map, raw)))
        .collect::<BTreeMap<_, _>>();
    let sequence_source_map = sequence_sources
        .into_iter()
        .map(|(archive, sequence, raw)| ((archive, sequence), raw))
        .collect::<BTreeMap<_, _>>();
    if sequence_source_map.len() != layouts.rows.len() {
        return Err(error(format!(
            "SEQ layout/source 분모 불일치: layouts={}, sources={}",
            layouts.rows.len(),
            sequence_source_map.len()
        )));
    }
    for layout in &layouts.rows {
        if !sequence_source_map.contains_key(&(layout.archive.as_str(), layout.sequence.as_str())) {
            return Err(error(format!(
                "SEQ layout 원본 누락: {}:{}",
                layout.archive, layout.sequence
            )));
        }
    }
    let sequence_archives = layouts
        .rows
        .iter()
        .map(|layout| layout.archive.as_str())
        .collect::<BTreeSet<_>>();
    let executable_entry_traces = executable_map
        .iter()
        .map(|(&exe, &(map, raw))| {
            let instructions = trace(map, raw, &[])
                .instructions
                .keys()
                .copied()
                .collect::<BTreeSet<_>>();
            (exe, instructions)
        })
        .collect::<BTreeMap<_, _>>();

    let targets = review_insufficient_members(candidates, review)?;
    let mut copies = Vec::new();
    let mut candidate_rows = Vec::new();
    for (_, candidate) in targets {
        let candidate_copy_start = copies.len();
        for copy in &candidate.physical_copies {
            let link = exact_name_link(names, copy)?;
            let &(map, executable) = executable_map
                .get(link.exe)
                .ok_or_else(|| error(format!("{} EXE 입력 누락", link.exe)))?;
            let descriptor_runtime = descriptor_entry_runtime(link)?;
            let descriptor_resource_id = resource_id(link, descriptor_runtime)?;
            let (_, descriptor_plain_bytes, initial_cell_table, initial_cell_count) =
                verify_initial_descriptor(map, executable, link, descriptor_runtime)?;
            if usize::try_from(descriptor_plain_bytes).ok() != Some(candidate.decompressed_bytes) {
                return Err(error(format!(
                    "{}:{} descriptor plain과 후보 원문 크기 불일치: descriptor={descriptor_plain_bytes}, candidate={}",
                    copy.archive, copy.name, candidate.decompressed_bytes
                )));
            }
            if initial_cell_table != 0 || initial_cell_count != 0 {
                return Err(error(format!(
                    "{}:{} 초기 descriptor cell field가 0이 아님",
                    copy.archive, copy.name
                )));
            }

            let matching_selections = selections
                .selections
                .iter()
                .filter(|selection| selection.exe == link.exe && selection.archive == copy.archive)
                .collect::<Vec<_>>();
            if matching_selections.len() > 1 {
                return Err(error(format!(
                    "{}:{} descriptor 선택 경계가 중복됨",
                    link.exe, copy.archive
                )));
            }
            let descriptor_selection_runtime = matching_selections
                .first()
                .map(|selection| selection.descriptor_select_runtime);

            let scan_context = SetupScanContext {
                initialized_len: executable.len(),
                load_base: map.load_addr,
                resource_id: descriptor_resource_id,
                review_width: candidate.review_width,
                review_height: candidate.review_height,
                format: candidate.format,
                descriptor_plain_bytes,
            };
            let mut setups = scan_setup_records(link.exe, executable, scan_context)?
                .into_iter()
                .map(|setup| {
                    setup_evidence(
                        BtxReviewSetupStorage::Executable,
                        link.exe.to_owned(),
                        setup,
                        candidate.format,
                        candidate.decompressed_bytes,
                    )
                })
                .collect::<Vec<_>>();
            let mut selectors = scan_selector_records(
                BtxReviewSetupStorage::Executable,
                link.exe,
                executable,
                executable.len(),
                map.load_addr,
                descriptor_resource_id,
                executable_entry_traces.get(link.exe),
            )?;

            if let Some(sequence_archive) =
                paired_sequence_archive(&copy.archive, &sequence_archives)
            {
                for layout in layouts.rows.iter().filter(|layout| {
                    layout.archive == sequence_archive
                        && layout.kind == SubgameSeqLayoutKind::Overlay
                }) {
                    let footer = layout
                        .footer
                        .ok_or_else(|| error("overlay layout의 footer 누락"))?;
                    let raw = sequence_source_map
                        .get(&(layout.archive.as_str(), layout.sequence.as_str()))
                        .ok_or_else(|| {
                            error(format!(
                                "SEQ layout 원본 누락: {}:{}",
                                layout.archive, layout.sequence
                            ))
                        })?;
                    setups.extend(
                        scan_setup_records(
                            &layout.sequence,
                            raw,
                            SetupScanContext {
                                initialized_len: usize::try_from(footer.initialized_len)
                                    .map_err(|_| error("SEQ initialized 길이 overflow"))?,
                                load_base: footer.load_base,
                                ..scan_context
                            },
                        )?
                        .into_iter()
                        .map(|setup| {
                            setup_evidence(
                                BtxReviewSetupStorage::SequenceOverlay,
                                format!("{}:{}", layout.archive, layout.sequence),
                                setup,
                                candidate.format,
                                candidate.decompressed_bytes,
                            )
                        }),
                    );
                    selectors.extend(scan_selector_records(
                        BtxReviewSetupStorage::SequenceOverlay,
                        &format!("{}:{}", layout.archive, layout.sequence),
                        raw,
                        usize::try_from(footer.initialized_len)
                            .map_err(|_| error("SEQ initialized 길이 overflow"))?,
                        footer.load_base,
                        descriptor_resource_id,
                        None,
                    )?);
                }
            }
            setups.sort_by(|left, right| {
                left.storage
                    .cmp(&right.storage)
                    .then_with(|| left.source.cmp(&right.source))
                    .then_with(|| left.record_runtime.cmp(&right.record_runtime))
            });
            selectors.sort_by(|left, right| {
                left.storage
                    .cmp(&right.storage)
                    .then_with(|| left.source.cmp(&right.source))
                    .then_with(|| left.record_runtime.cmp(&right.record_runtime))
                    .then_with(|| left.form.cmp(&right.form))
                    .then_with(|| left.record_type.cmp(&right.record_type))
            });
            copies.push(BtxReviewLayoutCopyEvidence {
                family_key: candidate.family_key.clone(),
                candidate_id: candidate.candidate_id.clone(),
                decompressed_sha256: candidate.decompressed_sha256.clone(),
                format: candidate.format,
                decompressed_bytes: candidate.decompressed_bytes,
                review_width: candidate.review_width,
                review_height: candidate.review_height,
                exe: link.exe,
                archive: copy.archive.clone(),
                asset_name: copy.name.clone(),
                vdd_entry_index: copy.resource_index,
                descriptor_resource_id,
                descriptor_runtime,
                descriptor_selection_runtime,
                setups,
                selectors,
            });
        }
        let candidate_copies = &copies[candidate_copy_start..];
        candidate_rows.push(summarize_candidate_layouts(candidate, candidate_copies)?);
    }

    copies.sort_by(|left, right| {
        left.family_key
            .cmp(&right.family_key)
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
            .then_with(|| left.archive.cmp(&right.archive))
    });
    candidate_rows.sort_by(|left, right| {
        left.family_key
            .cmp(&right.family_key)
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let families = candidate_rows
        .iter()
        .map(|candidate| candidate.family_key.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let copies_with_setup = copies.iter().filter(|copy| !copy.setups.is_empty()).count();
    let copies_with_descriptor_sized_setup = copies
        .iter()
        .filter(|copy| {
            copy.setups
                .iter()
                .any(|setup| setup.descriptor_plain_layout_matches)
        })
        .count();
    let copies_with_descriptor_compatible_setup = copies
        .iter()
        .filter(|copy| {
            copy.setups
                .iter()
                .any(|setup| setup.descriptor_storage_height.is_some())
        })
        .count();
    let copies_with_graphics_selector = copies
        .iter()
        .filter(|copy| {
            copy.selectors
                .iter()
                .any(BtxReviewSelectorEvidence::selects_graphics_archive)
        })
        .count();
    let graphics_selector_records = copies
        .iter()
        .flat_map(|copy| &copy.selectors)
        .filter(|selector| selector.selects_graphics_archive())
        .count();
    let other_type_numeric_matches = copies
        .iter()
        .flat_map(|copy| &copy.selectors)
        .filter(|selector| selector.is_record_candidate() && !selector.selects_graphics_archive())
        .count();
    let entry_trace_code_matches = copies
        .iter()
        .flat_map(|copy| &copy.selectors)
        .filter(|selector| !selector.is_record_candidate())
        .count();
    let count_resolution = |resolution| {
        candidate_rows
            .iter()
            .filter(|candidate| candidate.resolution == resolution)
            .count()
    };
    Ok(BtxReviewLayoutEvidenceAnalysis {
        families,
        unique_contents: candidate_rows.len(),
        physical_copies: copies.len(),
        copies_with_setup,
        copies_with_descriptor_sized_setup,
        copies_with_descriptor_compatible_setup,
        copies_with_graphics_selector,
        graphics_selector_records,
        other_type_numeric_matches,
        entry_trace_code_matches,
        exact_candidates: count_resolution(BtxReviewLayoutResolution::ExactAcrossCopies),
        row_padded_candidates: count_resolution(BtxReviewLayoutResolution::RowPaddedAcrossCopies),
        partial_candidates: count_resolution(BtxReviewLayoutResolution::PartialAcrossCopies),
        multiple_layout_candidates: count_resolution(BtxReviewLayoutResolution::MultipleLayouts),
        candidates_without_compatible_setup: count_resolution(
            BtxReviewLayoutResolution::NoCompatibleSetup,
        ),
        copies,
        candidates: candidate_rows,
    })
}

#[cfg(test)]
#[path = "btx_review_layout_evidence_tests.rs"]
mod tests;
