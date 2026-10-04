//! 단일 extent ISO 파일의 배치와 dual-endian 갱신 계획.
//!
//! 기존 reservation 분할, 검증된 Form1 volume tail allocation, parser가
//! 보존한 directory record 갱신을 소유한다. executable payload의 이동·변경은
//! byte-identical copy와 typed ISA 경로에 남기고, 여기서는 data 파일만 tail
//! payload·record·원본 무변경 경계의 원자적 계획으로 묶는다.

use std::error::Error;
use std::fmt;

use crate::coords::LogicalFile;
use crate::patch::{
    plan_logical_write, ExpectedWrite, ImageRegion, UnchangedBoundary, WriteIntent, WritePlan,
};
use crate::source::{FORM1_DATA_LEN, SECTOR_LEN};

use super::{DiscImage, Extent, ImageError, IsoEntry, VolumeTail};

const PSX_EXE_MAGIC: &[u8; 8] = b"PS-X EXE";

#[derive(Debug)]
pub enum IsoRelocationError {
    EmptySourceExtent,
    EmptyAllocation {
        index: usize,
    },
    AllocationExceedsSource {
        required: u64,
        available: u64,
    },
    RelocatedLbaOverflow,
    ExtentCount {
        path: String,
        actual: usize,
    },
    RecordLocationCount {
        path: String,
        actual: usize,
    },
    RecordTooShort {
        path: String,
        actual: usize,
    },
    Image {
        lba: u32,
        source: ImageError,
    },
    OriginalRecordMismatch {
        path: String,
    },
    ReplacementLength {
        path: String,
        expected: u32,
        actual: usize,
    },
    DirectoryUnsupported(String),
    ExecutablePayloadUnsupported(String),
    AllocationImageMismatch,
    OriginalExtentRangeOverflow(String),
    Plan(String),
}

impl fmt::Display for IsoRelocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySourceExtent => write!(f, "분할할 ISO extent 길이가 0임"),
            Self::EmptyAllocation { index } => {
                write!(f, "ISO extent 분할 요청 길이가 0임: index={index}")
            }
            Self::AllocationExceedsSource {
                required,
                available,
            } => write!(
                f,
                "ISO extent 분할 용량 부족: required={required}, available={available}"
            ),
            Self::RelocatedLbaOverflow => write!(f, "ISO extent 분할 LBA 계산 overflow"),
            Self::ExtentCount { path, actual } => {
                write!(f, "{path}: relocation 대상 extent 수 불일치: {actual}")
            }
            Self::RecordLocationCount { path, actual } => write!(
                f,
                "{path}: relocation 대상 directory record 위치 수 불일치: {actual}"
            ),
            Self::RecordTooShort { path, actual } => {
                write!(
                    f,
                    "{path}: directory record가 LBA/길이 field보다 짧음: {actual}"
                )
            }
            Self::Image { lba, source } => {
                write!(f, "ISO relocation LBA {lba} 읽기 실패: {source}")
            }
            Self::OriginalRecordMismatch { path } => {
                write!(f, "{path}: directory record의 원본 extent field 불일치")
            }
            Self::ReplacementLength {
                path,
                expected,
                actual,
            } => write!(
                f,
                "{path}: relocation payload 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::DirectoryUnsupported(path) => {
                write!(f, "{path}: directory는 data-file relocation 대상이 아님")
            }
            Self::ExecutablePayloadUnsupported(path) => write!(
                f,
                "{path}: PS-X EXE는 data-file relocation 경로로 쓸 수 없음"
            ),
            Self::AllocationImageMismatch => {
                write!(f, "volume tail allocation과 relocation 원본 이미지가 다름")
            }
            Self::OriginalExtentRangeOverflow(path) => {
                write!(f, "{path}: 원본 extent raw 무변경 범위 overflow")
            }
            Self::Plan(message) => write!(f, "ISO relocation Expected Write 실패: {message}"),
        }
    }
}

impl Error for IsoRelocationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Image { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// 한 원본 extent 앞쪽에 배치한 파일들과 뒤에 남긴 extent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtentPartition {
    allocations: Vec<Extent>,
    remainder: Extent,
}

/// 검증된 Form1 volume tail 안에 배정한 한 파일 extent.
#[derive(Debug, Clone, Copy)]
pub struct VolumeTailAllocation<'a> {
    image: &'a DiscImage,
    extent: Extent,
}

impl PartialEq for VolumeTailAllocation<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.image, other.image) && self.extent == other.extent
    }
}

impl Eq for VolumeTailAllocation<'_> {}

impl VolumeTailAllocation<'_> {
    /// directory record에 쓸 논리 LBA와 파일 길이.
    pub fn extent(&self) -> Extent {
        self.extent
    }
}

/// 한 volume tail을 sector 단위로 나눈 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeTailLayout<'a> {
    allocations: Vec<VolumeTailAllocation<'a>>,
    remainder: Option<VolumeTail<'a>>,
}

impl<'a> VolumeTailLayout<'a> {
    /// 요청 순서대로 배정한 파일 extent.
    pub fn allocations(&self) -> &[VolumeTailAllocation<'a>] {
        &self.allocations
    }

    /// 파일 allocation 뒤에 남은 Form1 volume tail.
    pub fn remainder(&self) -> Option<VolumeTail<'a>> {
        self.remainder
    }
}

impl ExtentPartition {
    /// 요청 순서대로 배치한 파일 extent.
    pub fn allocations(&self) -> &[Extent] {
        &self.allocations
    }

    /// sector 정렬 padding까지 제외하고 뒤에 남은 논리 extent.
    pub fn remainder(&self) -> Extent {
        self.remainder
    }
}

/// 한 extent 앞쪽에 파일을 sector 단위로 배치하고 비어 있지 않은 나머지를 남긴다.
///
/// 각 요청 길이는 ISO directory record에 쓸 논리 길이다. 다음 파일의 LBA는
/// 그 길이를 Form1 sector로 올림한 뒤 계산한다. 원본 논리 길이 밖의 sector
/// padding을 가용 용량으로 세지 않으며, 마지막에 길이 0인 extent도 만들지 않는다.
pub fn partition_extent_with_remainder(
    source: Extent,
    allocation_lengths: &[u32],
) -> Result<ExtentPartition, IsoRelocationError> {
    let (allocations, remainder) = partition_extent(source, allocation_lengths, true)?;
    let remainder = remainder.ok_or(IsoRelocationError::AllocationExceedsSource {
        required: u64::from(source.len).saturating_add(1),
        available: u64::from(source.len),
    })?;
    Ok(ExtentPartition {
        allocations,
        remainder,
    })
}

/// 검증된 Form1 volume tail에 파일을 sector 단위로 연속 배정한다.
///
/// tail 전체를 쓰는 것은 허용하며, 그 경우 remainder는 `None`이다. 반환된
/// allocation만 [`add_single_extent_data_file_relocation`]이 받는다.
pub fn allocate_volume_tail<'a>(
    tail: VolumeTail<'a>,
    allocation_lengths: &[u32],
) -> Result<VolumeTailLayout<'a>, IsoRelocationError> {
    let (allocations, remainder) = partition_extent(tail.extent(), allocation_lengths, false)?;
    Ok(VolumeTailLayout {
        allocations: allocations
            .into_iter()
            .map(|extent| VolumeTailAllocation {
                image: tail.image(),
                extent,
            })
            .collect(),
        remainder: remainder.map(|extent| VolumeTail::from_extent(tail.image(), extent)),
    })
}

fn partition_extent(
    source: Extent,
    allocation_lengths: &[u32],
    require_remainder: bool,
) -> Result<(Vec<Extent>, Option<Extent>), IsoRelocationError> {
    if source.len == 0 {
        return Err(IsoRelocationError::EmptySourceExtent);
    }

    let available = u64::from(source.len);
    let mut consumed = 0u64;
    let mut next_lba = source.lba;
    let mut allocations = Vec::with_capacity(allocation_lengths.len());
    for (index, &len) in allocation_lengths.iter().enumerate() {
        if len == 0 {
            return Err(IsoRelocationError::EmptyAllocation { index });
        }
        let sectors = len.div_ceil(FORM1_DATA_LEN as u32);
        let footprint = u64::from(sectors) * FORM1_DATA_LEN as u64;
        consumed =
            consumed
                .checked_add(footprint)
                .ok_or(IsoRelocationError::AllocationExceedsSource {
                    required: u64::MAX,
                    available,
                })?;
        if consumed > available || (require_remainder && consumed == available) {
            return Err(IsoRelocationError::AllocationExceedsSource {
                required: if consumed == available {
                    consumed.saturating_add(1)
                } else {
                    consumed
                },
                available,
            });
        }
        allocations.push(Extent { lba: next_lba, len });
        next_lba = next_lba
            .checked_add(sectors)
            .ok_or(IsoRelocationError::RelocatedLbaOverflow)?;
    }

    let remainder = if consumed == available {
        None
    } else {
        let remainder_len = u32::try_from(available - consumed).map_err(|_| {
            IsoRelocationError::AllocationExceedsSource {
                required: consumed,
                available,
            }
        })?;
        Some(Extent {
            lba: next_lba,
            len: remainder_len,
        })
    };
    Ok((allocations, remainder))
}

fn encode_extent_fields(extent: Extent) -> [u8; 16] {
    let mut raw = [0u8; 16];
    raw[..4].copy_from_slice(&extent.lba.to_le_bytes());
    raw[4..8].copy_from_slice(&extent.lba.to_be_bytes());
    raw[8..12].copy_from_slice(&extent.len.to_le_bytes());
    raw[12..16].copy_from_slice(&extent.len.to_be_bytes());
    raw
}

/// 단일 extent 파일의 LBA/길이 record 갱신 하나를 계획한다.
fn plan_single_extent_record_update(
    image: &DiscImage,
    entry: &IsoEntry,
    replacement: Extent,
    id: &str,
    actor: &str,
    purpose: &str,
) -> Result<ExpectedWrite, IsoRelocationError> {
    let [original] = entry.extents() else {
        return Err(IsoRelocationError::ExtentCount {
            path: entry.path().to_string(),
            actual: entry.extents().len(),
        });
    };
    let [location] = entry.record_locations() else {
        return Err(IsoRelocationError::RecordLocationCount {
            path: entry.path().to_string(),
            actual: entry.record_locations().len(),
        });
    };
    if location.record_len < 18 {
        return Err(IsoRelocationError::RecordTooShort {
            path: entry.path().to_string(),
            actual: location.record_len,
        });
    }
    let data = image
        .form1_data(location.lba)
        .map_err(|source| IsoRelocationError::Image {
            lba: location.lba,
            source,
        })?;
    let start = location.user_data_offset + 2;
    let end = start + 16;
    let expected = data
        .get(start..end)
        .ok_or_else(|| IsoRelocationError::RecordTooShort {
            path: entry.path().to_string(),
            actual: location.record_len,
        })?;
    if expected != encode_extent_fields(*original) {
        return Err(IsoRelocationError::OriginalRecordMismatch {
            path: entry.path().to_string(),
        });
    }
    let writes = plan_logical_write(
        LogicalFile::new(location.lba, FORM1_DATA_LEN as u32),
        start as u64,
        expected,
        &encode_extent_fields(replacement),
        id,
        actor,
        purpose,
        |_| WriteIntent::Metadata,
    )
    .ok_or_else(|| IsoRelocationError::Plan(format!("{id}: record 쓰기 분할 실패")))?;
    let [write] = writes.as_slice() else {
        return Err(IsoRelocationError::Plan(format!(
            "{id}: record 쓰기가 {}개로 분할됨",
            writes.len()
        )));
    };
    Ok(write.clone())
}

/// 단일 extent 파일의 dual-endian record 쓰기와 정확한 허용 영역을 함께 등록한다.
pub fn add_single_extent_record_update(
    plan: &mut WritePlan,
    image: &DiscImage,
    entry: &IsoEntry,
    replacement: Extent,
    id: &str,
    actor: &str,
    purpose: &str,
) -> Result<(), IsoRelocationError> {
    let write = plan_single_extent_record_update(image, entry, replacement, id, actor, purpose)?;
    plan.regions.push(ImageRegion::new(
        format!("region:{}", write.id),
        write.range(),
        write.intent.region_kind(),
        "parser가 보존한 ISO directory record의 dual-endian LBA/길이 field",
    ));
    plan.writes.push(write);
    Ok(())
}

fn read_extent(image: &DiscImage, extent: Extent) -> Result<Vec<u8>, IsoRelocationError> {
    let capacity = usize::try_from(extent.len).map_err(|_| {
        IsoRelocationError::Plan("relocation extent 길이 변환 overflow".to_string())
    })?;
    let mut output = Vec::with_capacity(capacity);
    let mut remaining = capacity;
    let mut lba = extent.lba;
    while remaining > 0 {
        let data = image
            .form1_data(lba)
            .map_err(|source| IsoRelocationError::Image { lba, source })?;
        let take = remaining.min(FORM1_DATA_LEN);
        output.extend_from_slice(&data[..take]);
        remaining -= take;
        lba = lba
            .checked_add(1)
            .ok_or(IsoRelocationError::RelocatedLbaOverflow)?;
    }
    Ok(output)
}

/// Form1 volume tail allocation에 data 파일 payload와 directory record를 함께 등록한다.
///
/// PS-X EXE는 이 경로에서 거부한다. executable relocation은 byte-identical
/// [`crate::patch::plan_logical_copy`]와 typed ISA 쓰기를 별도로 합성해야 한다.
/// 성공하기 전에는 plan을 부분 변경하지 않으며, 원본 파일의 raw extent는
/// 무변경 경계로 등록한다.
#[allow(clippy::too_many_arguments)]
pub fn add_single_extent_data_file_relocation(
    plan: &mut WritePlan,
    image: &DiscImage,
    entry: &IsoEntry,
    allocation: VolumeTailAllocation<'_>,
    replacement: &[u8],
    id: &str,
    actor: &str,
    purpose: &str,
) -> Result<(), IsoRelocationError> {
    if !std::ptr::eq(allocation.image, image) {
        return Err(IsoRelocationError::AllocationImageMismatch);
    }
    if entry.is_directory() {
        return Err(IsoRelocationError::DirectoryUnsupported(
            entry.path().to_string(),
        ));
    }
    let destination = allocation.extent();
    if replacement.len() != destination.len as usize {
        return Err(IsoRelocationError::ReplacementLength {
            path: entry.path().to_string(),
            expected: destination.len,
            actual: replacement.len(),
        });
    }
    let [original] = entry.extents() else {
        return Err(IsoRelocationError::ExtentCount {
            path: entry.path().to_string(),
            actual: entry.extents().len(),
        });
    };
    let original_payload = read_extent(image, *original)?;
    if original_payload.starts_with(PSX_EXE_MAGIC) || replacement.starts_with(PSX_EXE_MAGIC) {
        return Err(IsoRelocationError::ExecutablePayloadUnsupported(
            entry.path().to_string(),
        ));
    }
    let expected_destination = read_extent(image, destination)?;
    let payload_writes = if expected_destination == replacement {
        Vec::new()
    } else {
        plan_logical_write(
            LogicalFile::new(destination.lba, destination.len),
            0,
            &expected_destination,
            replacement,
            &format!("{id}:payload"),
            actor,
            purpose,
            |_| WriteIntent::Data,
        )
        .ok_or_else(|| IsoRelocationError::Plan(format!("{id}: payload 쓰기 분할 실패")))?
    };
    let record_write = plan_single_extent_record_update(
        image,
        entry,
        destination,
        &format!("{id}:directory-record"),
        actor,
        purpose,
    )?;
    let original_sectors = original.len.div_ceil(FORM1_DATA_LEN as u32);
    let original_end_lba = original
        .lba
        .checked_add(original_sectors)
        .ok_or_else(|| IsoRelocationError::OriginalExtentRangeOverflow(entry.path().to_string()))?;
    let original_start = u64::from(original.lba) * SECTOR_LEN as u64;
    let original_end = u64::from(original_end_lba) * SECTOR_LEN as u64;

    for write in &payload_writes {
        plan.regions.push(ImageRegion::new(
            format!("region:{}", write.id),
            write.range(),
            write.intent.region_kind(),
            "검증된 Form1 volume tail allocation의 data-file payload",
        ));
    }
    plan.regions.push(ImageRegion::new(
        format!("region:{}", record_write.id),
        record_write.range(),
        record_write.intent.region_kind(),
        "parser가 보존한 ISO directory record의 dual-endian LBA/길이 field",
    ));
    plan.writes.extend(payload_writes);
    plan.writes.push(record_write);
    plan.unchanged.push(UnchangedBoundary::new(
        format!("{id}:original-extent"),
        original_start..original_end,
        "volume tail relocation 뒤 원본 ISO 파일 raw extent byte-identical 보존",
    ));
    Ok(())
}

#[cfg(test)]
#[path = "relocation_tests.rs"]
mod tests;
