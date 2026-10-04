//! Mode2 Form1 Expected Write의 최종 적용.
//!
//! 논리 데이터 계획으로 후보 이미지를 메모리에서 합성한 뒤 수정 섹터의
//! EDC/ECC를 계산한다. 실제 반환 이미지는 데이터와 무결성 필드를 모두
//! 포함한 새 최종 [`WritePlan`]을 불변 원본에 한 번 적용해서만 만든다.

use std::error::Error;
use std::fmt;

use crate::patch::{ExpectedWrite, ImageRegion, PlanError, RegionKind, WriteIntent, WritePlan};
use crate::source::{FORM1_DATA_LEN, FORM1_DATA_OFFSET, SECTOR_LEN};

use super::image::{DiscImage, ImageError};
use super::sector::{
    regenerate_form1, verified_form1_data, SectorError, FORM1_INTEGRITY_LEN, FORM1_INTEGRITY_OFFSET,
};

/// raw 디스크 쓰기 실패.
#[derive(Debug)]
pub enum DiscWriteError {
    Plan(PlanError),
    Image(ImageError),
    Sector { lba: u32, source: SectorError },
    WriteOutsideForm1Data { id: String, offset: u64 },
    FinalCandidateMismatch,
}

impl fmt::Display for DiscWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plan(error) => write!(f, "Expected Write 실패: {error}"),
            Self::Image(error) => write!(f, "최종 디스크 검증 실패: {error}"),
            Self::Sector { lba, source } => {
                write!(f, "LBA {lba} 무결성 재생성 실패: {source}")
            }
            Self::WriteOutsideForm1Data { id, offset } => write!(
                f,
                "{id}: raw offset {offset}가 Mode2 Form1 사용자 데이터 밖임"
            ),
            Self::FinalCandidateMismatch => {
                write!(f, "최종 Expected Write 결과가 사전 합성 후보와 다름")
            }
        }
    }
}

impl Error for DiscWriteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Plan(error) => Some(error),
            Self::Image(error) => Some(error),
            Self::Sector { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<PlanError> for DiscWriteError {
    fn from(value: PlanError) -> Self {
        Self::Plan(value)
    }
}

impl From<ImageError> for DiscWriteError {
    fn from(value: ImageError) -> Self {
        Self::Image(value)
    }
}

fn ensure_form1_data_write(write: &ExpectedWrite) -> Result<(), DiscWriteError> {
    let start = write.offset as usize;
    let end = start + write.replacement.len();
    let sector = start / SECTOR_LEN;
    if end > (sector + 1) * SECTOR_LEN {
        return Err(DiscWriteError::WriteOutsideForm1Data {
            id: write.id.clone(),
            offset: write.offset,
        });
    }
    let within_start = start % SECTOR_LEN;
    let within_end = end % SECTOR_LEN;
    if within_start < FORM1_DATA_OFFSET
        || within_end > FORM1_DATA_OFFSET + FORM1_DATA_LEN
        || within_end == 0
    {
        return Err(DiscWriteError::WriteOutsideForm1Data {
            id: write.id.clone(),
            offset: write.offset,
        });
    }
    Ok(())
}

/// 논리 데이터 계획과 EDC/ECC를 하나의 최종 계획으로 만들어 적용한다.
pub fn apply_form1_write_plan(
    image: &DiscImage,
    data_plan: &WritePlan,
) -> Result<Vec<u8>, DiscWriteError> {
    let baseline = image.raw_bytes();
    data_plan.validate(baseline)?;
    for write in &data_plan.writes {
        ensure_form1_data_write(write)?;
    }

    // 최종 write의 replacement를 계산하기 위한 메모리상 합성 후보다.
    // 이 값 자체를 산출물로 반환하지 않는다.
    let mut candidate = data_plan.apply(baseline)?;
    let touched = data_plan.touched_sectors();
    for &lba in &touched {
        verified_form1_data(image.raw_sector(lba)?)
            .map_err(|source| DiscWriteError::Sector { lba, source })?;
        let start = lba as usize * SECTOR_LEN;
        regenerate_form1(&mut candidate[start..start + SECTOR_LEN])
            .map_err(|source| DiscWriteError::Sector { lba, source })?;
    }

    let mut final_plan = WritePlan::new();
    for boundary in &data_plan.unchanged {
        final_plan = final_plan.unchanged(boundary.clone());
    }
    for write in &data_plan.writes {
        final_plan = final_plan
            .region(ImageRegion::new(
                format!("final:data:{}", write.id),
                write.range(),
                write.intent.region_kind(),
                "검증된 논리 쓰기의 최종 raw 이미지 범위",
            ))
            .write(write.clone());
    }
    for lba in touched {
        let sector_start = lba as usize * SECTOR_LEN;
        let start = sector_start + FORM1_INTEGRITY_OFFSET;
        let end = start + FORM1_INTEGRITY_LEN;
        let id = format!("sector-{lba}-edc-ecc");
        final_plan = final_plan
            .region(ImageRegion::new(
                format!("final:metadata:{id}"),
                start as u64..end as u64,
                RegionKind::Metadata,
                "수정된 Mode2 Form1 섹터의 EDC/ECC",
            ))
            .write(ExpectedWrite {
                id,
                actor: "disc::writer".to_string(),
                purpose: "수정된 Form1 사용자 데이터에 맞춘 EDC/ECC 재생성".to_string(),
                offset: start as u64,
                expected_original: baseline[start..end].to_vec(),
                replacement: candidate[start..end].to_vec(),
                intent: WriteIntent::Metadata,
            });
    }

    let output = final_plan.apply(baseline)?;
    if output != candidate {
        return Err(DiscWriteError::FinalCandidateMismatch);
    }
    drop(candidate);
    let verified = DiscImage::from_bytes(output)?;
    verified.verify_all_sectors()?;
    Ok(verified.into_bytes())
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
