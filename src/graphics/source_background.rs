//! 원문 글자 mask에 가린 raster 배경을 결정적으로 복원한다.
//!
//! 호출자는 지울 pixel과 배경 donor pixel을 각각 판정한다. 이 모듈은 그
//! 판정을 추론하지 않고, mask 안의 각 pixel을 Manhattan 거리상 가장 가까운
//! donor 값으로 되돌린다. 같은 거리에서는 행 우선으로 앞선 donor를 고른다.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::error::Error;
use std::fmt;

/// nearest-donor 배경 복원 입력 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceBackgroundError {
    EmptyLayout {
        width: usize,
        height: usize,
    },
    LayoutOverflow {
        width: usize,
        height: usize,
    },
    InputLengthMismatch {
        expected: usize,
        pixels: usize,
        repair_mask: usize,
        donor_mask: usize,
    },
    EmptyRepairMask,
    RepairPixelIsDonor {
        x: usize,
        y: usize,
    },
    EmptyDonorMask,
    DonorDidNotReachRepairPixel {
        x: usize,
        y: usize,
    },
}

impl fmt::Display for SourceBackgroundError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLayout { width, height } => {
                write!(formatter, "배경 복원 layout 크기가 0: {width}x{height}")
            }
            Self::LayoutOverflow { width, height } => {
                write!(formatter, "배경 복원 layout 크기 overflow: {width}x{height}")
            }
            Self::InputLengthMismatch {
                expected,
                pixels,
                repair_mask,
                donor_mask,
            } => write!(
                formatter,
                "배경 복원 입력 크기 불일치: expected={expected}, pixels={pixels}, repair_mask={repair_mask}, donor_mask={donor_mask}"
            ),
            Self::EmptyRepairMask => formatter.write_str("배경 복원 대상 mask가 비어 있음"),
            Self::RepairPixelIsDonor { x, y } => {
                write!(formatter, "배경 복원 대상이 donor에도 포함됨: ({x},{y})")
            }
            Self::EmptyDonorMask => formatter.write_str("배경 복원 donor mask가 비어 있음"),
            Self::DonorDidNotReachRepairPixel { x, y } => {
                write!(formatter, "배경 donor가 복원 대상에 도달하지 못함: ({x},{y})")
            }
        }
    }
}

impl Error for SourceBackgroundError {}

/// mask 안 pixel을 가장 가까운 명시적 donor의 원본 값으로 복원한다.
pub fn restore_masked_pixels_from_nearest_donors(
    pixels: &[u8],
    width: usize,
    height: usize,
    repair_mask: &[bool],
    donor_mask: &[bool],
) -> Result<Vec<u8>, SourceBackgroundError> {
    if width == 0 || height == 0 {
        return Err(SourceBackgroundError::EmptyLayout { width, height });
    }
    let expected = width
        .checked_mul(height)
        .ok_or(SourceBackgroundError::LayoutOverflow { width, height })?;
    if pixels.len() != expected || repair_mask.len() != expected || donor_mask.len() != expected {
        return Err(SourceBackgroundError::InputLengthMismatch {
            expected,
            pixels: pixels.len(),
            repair_mask: repair_mask.len(),
            donor_mask: donor_mask.len(),
        });
    }
    if !repair_mask.iter().any(|selected| *selected) {
        return Err(SourceBackgroundError::EmptyRepairMask);
    }
    if let Some(offset) = repair_mask
        .iter()
        .zip(donor_mask)
        .position(|(repair, donor)| *repair && *donor)
    {
        return Err(SourceBackgroundError::RepairPixelIsDonor {
            x: offset % width,
            y: offset / width,
        });
    }

    let mut best = vec![(usize::MAX, usize::MAX); expected];
    let mut queue = BinaryHeap::new();
    for (offset, &is_donor) in donor_mask.iter().enumerate() {
        if is_donor {
            best[offset] = (0, offset);
            queue.push(Reverse((0usize, offset, offset)));
        }
    }
    if queue.is_empty() {
        return Err(SourceBackgroundError::EmptyDonorMask);
    }

    while let Some(Reverse((distance, owner, offset))) = queue.pop() {
        if best[offset] != (distance, owner) {
            continue;
        }
        let x = offset % width;
        let y = offset / width;
        let neighbors = [
            x.checked_sub(1).map(|x| y * width + x),
            (x + 1 < width).then_some(y * width + x + 1),
            y.checked_sub(1).map(|y| y * width + x),
            (y + 1 < height).then_some((y + 1) * width + x),
        ];
        let candidate = (distance + 1, owner);
        for neighbor in neighbors.into_iter().flatten() {
            if candidate < best[neighbor] {
                best[neighbor] = candidate;
                queue.push(Reverse((candidate.0, candidate.1, neighbor)));
            }
        }
    }

    let mut restored = pixels.to_vec();
    for (offset, &repair) in repair_mask.iter().enumerate() {
        if !repair {
            continue;
        }
        let owner = best[offset].1;
        if owner == usize::MAX {
            return Err(SourceBackgroundError::DonorDidNotReachRepairPixel {
                x: offset % width,
                y: offset / width,
            });
        }
        restored[offset] = pixels[owner];
    }
    Ok(restored)
}

#[cfg(test)]
#[path = "source_background_tests.rs"]
mod tests;
