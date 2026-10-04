//! Expected Write — 최종 이미지의 모든 변경을 사전에 선언하고 검증한다.
//!
//! 규약:
//!
//! - 모든 쓰기는 **불변 원본**을 기준으로 검증한 뒤 한 번에 적용한다.
//! - 쓰기마다 원본 바이트 전제를 선언하고, 다르면 실패한다.
//! - 쓰기끼리 겹치면 실패한다. 같은 범위를 의도적으로 만들려면 적용
//!   순서에 맡기지 말고 **하나의 쓰기로 먼저 합성**한다.
//! - 보호 영역 침범, 영역 밖 쓰기, 의도와 영역 종류 불일치는 실패한다.
//! - 기계어 영역 쓰기는 typed [`crate::isa::InstructionKind`] 원천에서 공통
//!   `psx-r3000a` checked assembler를 거친 [`crate::isa::verify_typed_source`]로
//!   조립·검산한
//!   [`VerifiedMachineCode`]로만 가능하다. raw byte 검산 결과는 제품
//!   기계어 쓰기로 승인하지 않는다.
//! - 적용 후 원본과 다른 모든 바이트는 등록된 Expected Write로 설명돼야
//!   한다. 설명되지 않는 diff가 1바이트라도 있으면 실패한다.
//! - 실패하면 산출물을 남기지 않는다.
//!
//! # 크기 변경
//!
//! 이 계층은 **같은 크기 제자리 교체**만 다룬다. 텍스트가 늘어 파일이
//! 커지는 경우는 ISO 재배치(LBA·extent·디렉터리 갱신)라는 다른 경계이며,
//! 그 결과로 나온 이미지를 다시 이 계층이 검증한다.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use crate::coords::LogicalFile;
use crate::isa::VerifiedMachineCode;
use crate::source::FORM1_DATA_LEN;

use super::region::{intersects, ImageRegion, RegionKind, UnchangedBoundary};

/// [`plan_logical_copy`]만 만들 수 있는 불변 원본 copy 증명.
///
/// field를 비공개로 두어 일반 pipeline이 `FileRelocation` 의도를 직접
/// 조립하고 임의 replacement를 넣지 못하게 한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRelocationProof {
    source_offset: u64,
    source_id: String,
}

/// [`plan_logical_machine_code`]만 만들 수 있는 sector 분할 기계어 증명.
///
/// 각 조각은 같은 typed source의 정확한 일부이며, [`WritePlan`]은 같은
/// group의 모든 조각이 빠짐없이 연속된 source 전체를 덮는지 검증한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineCodeSegmentProof {
    verified: Box<VerifiedMachineCode>,
    group_id: String,
    verified_offset: usize,
    segment_index: usize,
    segment_count: usize,
}

/// 쓰기 의도. 영역 종류와 일치해야 한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteIntent {
    Data,
    Metadata,
    /// 검산을 통과한 기계어. 생성 근거를 함께 담는다.
    MachineCode {
        verified: Box<VerifiedMachineCode>,
    },
    /// Mode2 sector 경계에서 나뉜 하나의 typed 기계어 source.
    MachineCodeSegment(MachineCodeSegmentProof),
    /// 불변 원본의 다른 raw 위치에서 그대로 복사한 논리 파일 byte.
    ///
    /// 이 의도는 실행 파일의 ISO extent 재배치처럼 runtime 주소와 내용은
    /// 바꾸지 않고 디스크 위치만 바꾸는 경우에만 쓴다. 검증기는 source
    /// 위치의 기준선 byte와 replacement가 정확히 같은지 확인한다.
    FileRelocation(FileRelocationProof),
}

impl WriteIntent {
    pub fn region_kind(&self) -> RegionKind {
        match self {
            Self::Data => RegionKind::Data,
            Self::Metadata => RegionKind::Metadata,
            Self::MachineCode { .. } => RegionKind::MachineCode,
            Self::MachineCodeSegment(_) => RegionKind::MachineCode,
            Self::FileRelocation(_) => RegionKind::FileRelocation,
        }
    }

    pub fn label(&self) -> &'static str {
        self.region_kind().label()
    }
}

/// 최종 이미지에 적용할 쓰기 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedWrite {
    pub id: String,
    /// 이 쓰기를 만든 주체 (빌드 단계·모듈).
    pub actor: String,
    /// 무엇을 위해 쓰는가.
    pub purpose: String,
    /// 이미지 바이트 오프셋.
    pub offset: u64,
    /// 원본이 이 위치에 가지고 있어야 할 바이트.
    pub expected_original: Vec<u8>,
    /// 새로 쓸 바이트. `expected_original`과 길이가 같아야 한다.
    pub replacement: Vec<u8>,
    pub intent: WriteIntent,
}

impl ExpectedWrite {
    pub fn range(&self) -> Range<u64> {
        self.offset..self.offset + self.replacement.len() as u64
    }
}

/// 계획 전체.
#[derive(Debug, Clone, Default)]
pub struct WritePlan {
    pub regions: Vec<ImageRegion>,
    pub writes: Vec<ExpectedWrite>,
    pub unchanged: Vec<UnchangedBoundary>,
}

/// 검증 실패 사유. 전부 빌드 실패다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    EmptyField {
        what: &'static str,
        id: String,
    },
    DuplicateRegionId(String),
    RegionOutOfBounds {
        id: String,
        end: u64,
        image_len: u64,
    },
    RegionsOverlap {
        a: String,
        b: String,
    },
    DuplicateWriteId(String),
    LengthMismatch {
        id: String,
        original: usize,
        replacement: usize,
    },
    EmptyWrite(String),
    WriteOutOfBounds {
        id: String,
        end: u64,
        image_len: u64,
    },
    OriginalBytesMismatch {
        id: String,
        offset: u64,
    },
    WritesOverlap {
        a: String,
        a_actor: String,
        b: String,
        b_actor: String,
    },
    NotInExactlyOneRegion {
        id: String,
        matched: usize,
    },
    ProtectedRegionWrite {
        id: String,
        region: String,
    },
    IntentRegionMismatch {
        id: String,
        intent: &'static str,
        region: String,
        kind: &'static str,
    },
    MachineCodeBytesMismatch {
        id: String,
    },
    MachineCodeSourceMissing {
        id: String,
    },
    MachineCodeProfileMissing {
        id: String,
    },
    MachineCodeSegmentRangeInvalid {
        id: String,
    },
    MachineCodeSegmentSetInvalid {
        group_id: String,
    },
    RelocationSourceIdEmpty {
        id: String,
    },
    RelocationSourceOutOfBounds {
        id: String,
        end: u64,
        image_len: u64,
    },
    RelocationSourceMismatch {
        id: String,
        source_offset: u64,
    },
    UnchangedBoundaryViolated {
        id: String,
        offset: u64,
    },
    UntrackedDiff {
        offset: u64,
    },
    OutputLengthChanged {
        expected: usize,
        actual: usize,
    },
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyField { what, id } => write!(f, "{id}: {what}가 비어 있다"),
            Self::DuplicateRegionId(id) => write!(f, "영역 id 중복: {id}"),
            Self::RegionOutOfBounds { id, end, image_len } => {
                write!(f, "영역 {id}의 끝 {end}가 이미지 크기 {image_len} 밖이다")
            }
            Self::RegionsOverlap { a, b } => write!(f, "영역 {a}와 {b}가 겹친다"),
            Self::DuplicateWriteId(id) => write!(f, "Expected Write id 중복: {id}"),
            Self::LengthMismatch {
                id,
                original,
                replacement,
            } => write!(
                f,
                "{id}: 원본 전제 {original}B와 교체 {replacement}B의 길이가 다르다"
            ),
            Self::EmptyWrite(id) => write!(f, "{id}: 빈 쓰기"),
            Self::WriteOutOfBounds { id, end, image_len } => {
                write!(f, "{id}의 끝 {end}가 이미지 크기 {image_len} 밖이다")
            }
            Self::OriginalBytesMismatch { id, offset } => {
                write!(f, "{id}: 오프셋 {offset}의 원본 바이트 전제가 어긋난다")
            }
            Self::WritesOverlap {
                a,
                a_actor,
                b,
                b_actor,
            } => write!(f, "{a}({a_actor})와 {b}({b_actor})의 쓰기 범위가 겹친다"),
            Self::NotInExactlyOneRegion { id, matched } => write!(
                f,
                "{id}가 정확히 하나의 영역에 들어가지 않는다 (일치 {matched}개)"
            ),
            Self::ProtectedRegionWrite { id, region } => {
                write!(f, "{id}가 보호 영역 {region}을 침범한다")
            }
            Self::IntentRegionMismatch {
                id,
                intent,
                region,
                kind,
            } => write!(
                f,
                "{id}의 의도 {intent}가 영역 {region}의 종류 {kind}와 다르다"
            ),
            Self::MachineCodeBytesMismatch { id } => {
                write!(f, "{id}: 교체 바이트가 검산된 기계어와 다르다")
            }
            Self::MachineCodeSourceMissing { id } => {
                write!(f, "{id}: typed 기계어 source 없이 raw byte만 검산했다")
            }
            Self::MachineCodeProfileMissing { id } => write!(
                f,
                "{id}: 공통 PSX checked assembler profile provenance가 없다"
            ),
            Self::MachineCodeSegmentRangeInvalid { id } => {
                write!(f, "{id}: typed 기계어 source 조각 범위가 유효하지 않다")
            }
            Self::MachineCodeSegmentSetInvalid { group_id } => write!(
                f,
                "{group_id}: sector 분할 typed 기계어 조각 집합이 source 전체와 일치하지 않는다"
            ),
            Self::RelocationSourceIdEmpty { id } => {
                write!(f, "{id}: file relocation 원본 식별자가 비어 있다")
            }
            Self::RelocationSourceOutOfBounds { id, end, image_len } => write!(
                f,
                "{id}: file relocation 원본 끝 {end}가 이미지 크기 {image_len} 밖이다"
            ),
            Self::RelocationSourceMismatch { id, source_offset } => write!(
                f,
                "{id}: replacement가 불변 원본 offset {source_offset}의 byte와 다르다"
            ),
            Self::UnchangedBoundaryViolated { id, offset } => {
                write!(f, "무변경 경계 {id}의 오프셋 {offset}가 바뀌었다")
            }
            Self::UntrackedDiff { offset } => {
                write!(f, "오프셋 {offset}에 등록되지 않은 변경이 있다")
            }
            Self::OutputLengthChanged { expected, actual } => {
                write!(f, "출력 길이가 바뀌었다: {expected} → {actual}")
            }
        }
    }
}

impl std::error::Error for PlanError {}

impl WritePlan {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn region(mut self, r: ImageRegion) -> Self {
        self.regions.push(r);
        self
    }

    pub fn write(mut self, w: ExpectedWrite) -> Self {
        self.writes.push(w);
        self
    }

    pub fn unchanged(mut self, b: UnchangedBoundary) -> Self {
        self.unchanged.push(b);
        self
    }

    /// 원본을 기준으로 계획 전체를 검증한다. 적용은 하지 않는다.
    pub fn validate(&self, baseline: &[u8]) -> Result<(), PlanError> {
        let image_len = baseline.len() as u64;

        let mut region_ids = BTreeSet::new();
        for (i, region) in self.regions.iter().enumerate() {
            if region.id.is_empty() {
                return Err(PlanError::EmptyField {
                    what: "영역 id",
                    id: format!("#{i}"),
                });
            }
            if region.reason.is_empty() {
                return Err(PlanError::EmptyField {
                    what: "영역 근거",
                    id: region.id.clone(),
                });
            }
            if !region_ids.insert(region.id.as_str()) {
                return Err(PlanError::DuplicateRegionId(region.id.clone()));
            }
            if region.range.start >= region.range.end || region.range.end > image_len {
                return Err(PlanError::RegionOutOfBounds {
                    id: region.id.clone(),
                    end: region.range.end,
                    image_len,
                });
            }
            for other in self.regions.iter().take(i) {
                if intersects(&region.range, &other.range) {
                    return Err(PlanError::RegionsOverlap {
                        a: region.id.clone(),
                        b: other.id.clone(),
                    });
                }
            }
        }

        let mut write_ids = BTreeSet::new();
        for (i, w) in self.writes.iter().enumerate() {
            if w.id.is_empty() {
                return Err(PlanError::EmptyField {
                    what: "쓰기 id",
                    id: format!("#{i}"),
                });
            }
            for (what, v) in [("actor", &w.actor), ("purpose", &w.purpose)] {
                if v.is_empty() {
                    return Err(PlanError::EmptyField {
                        what,
                        id: w.id.clone(),
                    });
                }
            }
            if !write_ids.insert(w.id.as_str()) {
                return Err(PlanError::DuplicateWriteId(w.id.clone()));
            }
            if w.replacement.is_empty() {
                return Err(PlanError::EmptyWrite(w.id.clone()));
            }
            if w.expected_original.len() != w.replacement.len() {
                return Err(PlanError::LengthMismatch {
                    id: w.id.clone(),
                    original: w.expected_original.len(),
                    replacement: w.replacement.len(),
                });
            }

            let range = w.range();
            if range.end > image_len {
                return Err(PlanError::WriteOutOfBounds {
                    id: w.id.clone(),
                    end: range.end,
                    image_len,
                });
            }
            let actual = &baseline[range.start as usize..range.end as usize];
            if actual != w.expected_original.as_slice() {
                return Err(PlanError::OriginalBytesMismatch {
                    id: w.id.clone(),
                    offset: w.offset,
                });
            }

            for other in self.writes.iter().take(i) {
                if intersects(&range, &other.range()) {
                    return Err(PlanError::WritesOverlap {
                        a: w.id.clone(),
                        a_actor: w.actor.clone(),
                        b: other.id.clone(),
                        b_actor: other.actor.clone(),
                    });
                }
            }

            let containing: Vec<&ImageRegion> =
                self.regions.iter().filter(|r| r.contains(&range)).collect();
            if containing.len() != 1 {
                return Err(PlanError::NotInExactlyOneRegion {
                    id: w.id.clone(),
                    matched: containing.len(),
                });
            }
            let region = containing[0];
            if region.kind == RegionKind::Protected {
                return Err(PlanError::ProtectedRegionWrite {
                    id: w.id.clone(),
                    region: region.id.clone(),
                });
            }
            if w.intent.region_kind() != region.kind {
                return Err(PlanError::IntentRegionMismatch {
                    id: w.id.clone(),
                    intent: w.intent.label(),
                    region: region.id.clone(),
                    kind: region.kind.label(),
                });
            }

            // 기계어 쓰기는 typed source에서 생성됐고 검산된 바이트와
            // 정확히 같아야 한다. raw byte 검산이나 검산 뒤 바꿔치기를
            // 모두 막는다.
            if let WriteIntent::MachineCode { verified } = &w.intent {
                if verified.typed_source_id().is_none() {
                    return Err(PlanError::MachineCodeSourceMissing { id: w.id.clone() });
                }
                if verified.checked_profile_id() != Some(psx_r3000a::PROFILE_ID) {
                    return Err(PlanError::MachineCodeProfileMissing { id: w.id.clone() });
                }
                if verified.bytes() != w.replacement.as_slice() {
                    return Err(PlanError::MachineCodeBytesMismatch { id: w.id.clone() });
                }
            }
            if let WriteIntent::MachineCodeSegment(proof) = &w.intent {
                if proof.verified.typed_source_id().is_none() {
                    return Err(PlanError::MachineCodeSourceMissing { id: w.id.clone() });
                }
                if proof.verified.checked_profile_id() != Some(psx_r3000a::PROFILE_ID) {
                    return Err(PlanError::MachineCodeProfileMissing { id: w.id.clone() });
                }
                let end = proof
                    .verified_offset
                    .checked_add(w.replacement.len())
                    .ok_or_else(|| PlanError::MachineCodeSegmentRangeInvalid {
                        id: w.id.clone(),
                    })?;
                let expected = proof
                    .verified
                    .bytes()
                    .get(proof.verified_offset..end)
                    .ok_or_else(|| PlanError::MachineCodeSegmentRangeInvalid {
                        id: w.id.clone(),
                    })?;
                if expected != w.replacement {
                    return Err(PlanError::MachineCodeBytesMismatch { id: w.id.clone() });
                }
            }
            if let WriteIntent::FileRelocation(proof) = &w.intent {
                if proof.source_id.trim().is_empty() {
                    return Err(PlanError::RelocationSourceIdEmpty { id: w.id.clone() });
                }
                let source_end = proof
                    .source_offset
                    .checked_add(w.replacement.len() as u64)
                    .ok_or_else(|| PlanError::RelocationSourceOutOfBounds {
                        id: w.id.clone(),
                        end: u64::MAX,
                        image_len,
                    })?;
                if source_end > image_len {
                    return Err(PlanError::RelocationSourceOutOfBounds {
                        id: w.id.clone(),
                        end: source_end,
                        image_len,
                    });
                }
                if baseline[proof.source_offset as usize..source_end as usize]
                    != *w.replacement.as_slice()
                {
                    return Err(PlanError::RelocationSourceMismatch {
                        id: w.id.clone(),
                        source_offset: proof.source_offset,
                    });
                }
            }
        }

        let mut machine_code_groups =
            BTreeMap::<&str, Vec<(&ExpectedWrite, &MachineCodeSegmentProof)>>::new();
        for write in &self.writes {
            if let WriteIntent::MachineCodeSegment(proof) = &write.intent {
                machine_code_groups
                    .entry(proof.group_id.as_str())
                    .or_default()
                    .push((write, proof));
            }
        }
        for (group_id, mut segments) in machine_code_groups {
            segments.sort_by_key(|(_, proof)| proof.segment_index);
            let Some((_, first)) = segments.first() else {
                continue;
            };
            let valid_header = !group_id.is_empty()
                && first.segment_count == segments.len()
                && segments.iter().all(|(_, proof)| {
                    proof.segment_count == first.segment_count
                        && proof.verified == first.verified
                        && proof.group_id == first.group_id
                });
            let mut cursor = 0usize;
            let valid_segments = segments.iter().enumerate().all(|(index, (write, proof))| {
                if proof.segment_index != index || proof.verified_offset != cursor {
                    return false;
                }
                cursor = match cursor.checked_add(write.replacement.len()) {
                    Some(next) => next,
                    None => return false,
                };
                true
            });
            if !valid_header || !valid_segments || cursor != first.verified.bytes().len() {
                return Err(PlanError::MachineCodeSegmentSetInvalid {
                    group_id: group_id.to_string(),
                });
            }
        }

        // 무변경 경계가 어떤 쓰기와도 겹치지 않아야 한다.
        for b in &self.unchanged {
            for w in &self.writes {
                if intersects(&b.range, &w.range()) {
                    return Err(PlanError::UnchangedBoundaryViolated {
                        id: b.id.clone(),
                        offset: w.offset,
                    });
                }
            }
        }

        Ok(())
    }

    /// 검증 후 원본에 적용해 최종 이미지를 만든다.
    ///
    /// 검증 실패면 산출물을 만들지 않는다. 적용 후 원본과 다른 모든 바이트가
    /// 등록된 쓰기로 설명되는지 다시 확인한다.
    pub fn apply(&self, baseline: &[u8]) -> Result<Vec<u8>, PlanError> {
        self.validate(baseline)?;

        let mut out = baseline.to_vec();
        for w in &self.writes {
            let r = w.range();
            out[r.start as usize..r.end as usize].copy_from_slice(&w.replacement);
        }

        self.verify_final(baseline, &out)?;
        Ok(out)
    }

    /// 최종 이미지의 모든 diff가 등록된 쓰기로 설명되는지 확인한다.
    ///
    /// 계획 밖에서 이미지를 만졌거나, 쓰기 범위 밖으로 새어 나간 변경을
    /// 잡는다.
    pub fn verify_final(&self, baseline: &[u8], output: &[u8]) -> Result<(), PlanError> {
        if baseline.len() != output.len() {
            return Err(PlanError::OutputLengthChanged {
                expected: baseline.len(),
                actual: output.len(),
            });
        }

        let mut ranges: Vec<Range<u64>> = self.writes.iter().map(|w| w.range()).collect();
        ranges.sort_by_key(|r| r.start);

        let covered = |offset: u64| -> bool {
            ranges
                .binary_search_by(|r| {
                    if offset < r.start {
                        std::cmp::Ordering::Greater
                    } else if offset >= r.end {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
                .is_ok()
        };

        for (i, (a, b)) in baseline.iter().zip(output.iter()).enumerate() {
            if a != b && !covered(i as u64) {
                return Err(PlanError::UntrackedDiff { offset: i as u64 });
            }
        }

        for b in &self.unchanged {
            let s = b.range.start as usize;
            let e = (b.range.end as usize).min(output.len());
            if let Some(off) = (s..e).find(|&i| baseline[i] != output[i]) {
                return Err(PlanError::UnchangedBoundaryViolated {
                    id: b.id.clone(),
                    offset: off as u64,
                });
            }
        }

        Ok(())
    }

    /// 이 계획이 건드리는 섹터 LBA 집합.
    ///
    /// 수정된 섹터만 EDC/ECC를 다시 계산해야 한다. 건드리지 않은 섹터의
    /// 비정상·보호 표현을 정상화하면 안 된다.
    pub fn touched_sectors(&self) -> BTreeSet<u32> {
        use crate::source::SECTOR_LEN;
        let mut out = BTreeSet::new();
        for w in &self.writes {
            let r = w.range();
            let first = r.start / SECTOR_LEN as u64;
            let last = (r.end - 1) / SECTOR_LEN as u64;
            for s in first..=last {
                out.insert(s as u32);
            }
        }
        out
    }
}

/// 논리 파일 안의 연속 구간을 Expected Write 목록으로 만든다.
///
/// **논리적으로 연속인 바이트는 이미지에서 연속이 아니다.** Mode2/2352는
/// 섹터마다 304바이트의 sync·header·subheader·EDC·ECC를 끼우므로, 섹터
/// 경계를 넘는 논리 쓰기는 여러 개의 이미지 쓰기로 쪼개야 한다.
///
/// 이 함수는 그 분할을 대신 해 준다. 직접 오프셋을 계산하면 섹터 경계에서
/// 조용히 보호 필드를 덮어쓰게 된다.
#[allow(clippy::too_many_arguments)]
pub fn plan_logical_write(
    file: LogicalFile,
    file_offset: u64,
    expected_original: &[u8],
    replacement: &[u8],
    id_prefix: &str,
    actor: &str,
    purpose: &str,
    intent_for: impl Fn(usize) -> WriteIntent,
) -> Option<Vec<ExpectedWrite>> {
    if expected_original.len() != replacement.len() || replacement.is_empty() {
        return None;
    }
    file.touched_sectors(file_offset, replacement.len() as u64)?;

    let mut out = Vec::new();
    let mut done = 0usize;
    let total = replacement.len();

    while done < total {
        let cur = file_offset + done as u64;
        let image_off = file.image_offset(cur)?;
        // 이 섹터에서 남은 바이트 수
        let in_sector = FORM1_DATA_LEN as u64 - (cur % FORM1_DATA_LEN as u64);
        let take = in_sector.min((total - done) as u64) as usize;

        out.push(ExpectedWrite {
            id: format!("{id_prefix}#{}", out.len()),
            actor: actor.to_string(),
            purpose: purpose.to_string(),
            offset: image_off,
            expected_original: expected_original[done..done + take].to_vec(),
            replacement: replacement[done..done + take].to_vec(),
            intent: intent_for(out.len()),
        });
        done += take;
    }

    Some(out)
}

/// typed 기계어 하나를 Mode2 sector 경계에 맞춰 Expected Write로 나눈다.
///
/// 반환된 조각은 하나의 group 증명을 공유한다. 일부 조각을 누락하거나
/// 순서·source offset·replacement를 바꾸면 [`WritePlan`] 검증이 실패한다.
#[allow(clippy::too_many_arguments)]
pub fn plan_logical_machine_code(
    file: LogicalFile,
    file_offset: u64,
    expected_original: &[u8],
    verified: &VerifiedMachineCode,
    id_prefix: &str,
    actor: &str,
    purpose: &str,
) -> Option<Vec<ExpectedWrite>> {
    let replacement = verified.bytes();
    if expected_original.len() != replacement.len()
        || replacement.is_empty()
        || id_prefix.trim().is_empty()
        || actor.trim().is_empty()
        || purpose.trim().is_empty()
    {
        return None;
    }
    file.touched_sectors(file_offset, replacement.len() as u64)?;

    let mut spans = Vec::new();
    let mut done = 0usize;
    while done < replacement.len() {
        let logical = file_offset.checked_add(done as u64)?;
        let image_offset = file.image_offset(logical)?;
        let in_sector = FORM1_DATA_LEN as u64 - logical % FORM1_DATA_LEN as u64;
        let take = in_sector.min((replacement.len() - done) as u64) as usize;
        spans.push((done, take, image_offset));
        done = done.checked_add(take)?;
    }

    let group_id = format!("{actor}:{id_prefix}");
    let segment_count = spans.len();
    Some(
        spans
            .into_iter()
            .enumerate()
            .map(
                |(segment_index, (verified_offset, take, offset))| ExpectedWrite {
                    id: format!("{id_prefix}#{segment_index}"),
                    actor: actor.to_string(),
                    purpose: purpose.to_string(),
                    offset,
                    expected_original: expected_original[verified_offset..verified_offset + take]
                        .to_vec(),
                    replacement: replacement[verified_offset..verified_offset + take].to_vec(),
                    intent: WriteIntent::MachineCodeSegment(MachineCodeSegmentProof {
                        verified: Box::new(verified.clone()),
                        group_id: group_id.clone(),
                        verified_offset,
                        segment_index,
                        segment_count,
                    }),
                },
            )
            .collect(),
    )
}

/// 한 논리 파일 구간을 다른 논리 파일 위치로 기준선 그대로 복사한다.
///
/// source와 destination의 Mode2 sector 경계를 모두 따라 분할한다. 생성된
/// 쓰기는 [`WriteIntent::FileRelocation`]이므로 replacement를 임의로 바꾸거나
/// 원본 위치를 속이면 최종 [`WritePlan`] 검증에서 실패한다.
#[allow(clippy::too_many_arguments)]
pub fn plan_logical_copy(
    baseline: &[u8],
    source: LogicalFile,
    source_offset: u64,
    destination: LogicalFile,
    destination_offset: u64,
    len: u64,
    id_prefix: &str,
    actor: &str,
    purpose: &str,
    source_id: &str,
) -> Option<Vec<ExpectedWrite>> {
    source.touched_sectors(source_offset, len)?;
    destination.touched_sectors(destination_offset, len)?;
    if len == 0 || source_id.trim().is_empty() {
        return None;
    }

    let image_len = u64::try_from(baseline.len()).ok()?;
    let mut output = Vec::new();
    let mut done = 0u64;
    while done < len {
        let source_logical = source_offset.checked_add(done)?;
        let destination_logical = destination_offset.checked_add(done)?;
        let source_raw = source.image_offset(source_logical)?;
        let destination_raw = destination.image_offset(destination_logical)?;
        let source_in_sector = FORM1_DATA_LEN as u64 - source_logical % FORM1_DATA_LEN as u64;
        let destination_in_sector =
            FORM1_DATA_LEN as u64 - destination_logical % FORM1_DATA_LEN as u64;
        let take = source_in_sector
            .min(destination_in_sector)
            .min(len.checked_sub(done)?);
        let source_end = source_raw.checked_add(take)?;
        let destination_end = destination_raw.checked_add(take)?;
        if source_end > image_len || destination_end > image_len {
            return None;
        }
        let source_bytes = &baseline[source_raw as usize..source_end as usize];
        let destination_bytes = &baseline[destination_raw as usize..destination_end as usize];
        if source_bytes != destination_bytes {
            output.push(ExpectedWrite {
                id: format!("{id_prefix}#{}", output.len()),
                actor: actor.to_string(),
                purpose: purpose.to_string(),
                offset: destination_raw,
                expected_original: destination_bytes.to_vec(),
                replacement: source_bytes.to_vec(),
                intent: WriteIntent::FileRelocation(FileRelocationProof {
                    source_offset: source_raw,
                    source_id: source_id.to_string(),
                }),
            });
        }
        done = done.checked_add(take)?;
    }
    Some(output)
}

#[cfg(test)]
#[path = "write_plan_tests.rs"]
mod write_plan_tests;
