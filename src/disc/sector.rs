//! Mode2/2352 Form1 섹터의 구조와 EDC/ECC 검증.
//!
//! 논리 사용자 데이터는 raw 이미지에서 연속하지 않는다. 이 모듈은 섹터
//! framing과 무결성 필드를 검증한 뒤에만 2,048바이트 Form1 데이터를
//! 노출하고, 수정 섹터의 EDC/ECC를 같은 규칙으로 다시 만든다.

use std::error::Error;
use std::fmt;
use std::sync::OnceLock;

use crate::source::{FORM1_DATA_LEN, FORM1_DATA_OFFSET, SECTOR_LEN};

const SYNC: [u8; 12] = [
    0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00,
];
const MODE_OFFSET: usize = 0x0f;
const SUBHEADER_A: std::ops::Range<usize> = 0x10..0x14;
const SUBHEADER_B: std::ops::Range<usize> = 0x14..0x18;
const SUBMODE_OFFSET: usize = 0x12;
const FORM2_BIT: u8 = 0x20;
const FORM1_EDC_OFFSET: usize = 0x818;
const ECC_P_OFFSET: usize = 0x81c;
const ECC_Q_OFFSET: usize = 0x8c8;
const ECC_END: usize = 0x930;
const FORM2_DATA_LEN: usize = 2324;
const FORM2_EDC_OFFSET: usize = 0x92c;

/// Form1 사용자 데이터 뒤의 EDC·ECC 연속 범위 시작.
pub const FORM1_INTEGRITY_OFFSET: usize = FORM1_EDC_OFFSET;

/// Form1 EDC·ECC 연속 범위 길이.
pub const FORM1_INTEGRITY_LEN: usize = ECC_END - FORM1_EDC_OFFSET;

/// Mode2 섹터 form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorKind {
    Form1,
    Form2,
}

/// form별 무결성을 확인한 사용자 데이터.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedSectorData<'a> {
    pub kind: SectorKind,
    pub data: &'a [u8],
}

/// raw 섹터 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectorError {
    WrongLength { expected: usize, actual: usize },
    InvalidSync,
    UnsupportedMode(u8),
    SubheaderMismatch,
    ExpectedForm1,
    ExpectedForm2,
    Form1EdcMismatch,
    Form2EdcMismatch,
    EccPMismatch,
    EccQMismatch,
}

impl fmt::Display for SectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength { expected, actual } => {
                write!(
                    f,
                    "raw 섹터 크기 불일치: expected={expected}, actual={actual}"
                )
            }
            Self::InvalidSync => write!(f, "raw 섹터 sync pattern 불일치"),
            Self::UnsupportedMode(mode) => {
                write!(f, "지원하지 않는 CD 섹터 mode: {mode}")
            }
            Self::SubheaderMismatch => write!(f, "Mode2 subheader 복제본 불일치"),
            Self::ExpectedForm1 => write!(f, "Mode2 Form2를 Form1으로 읽을 수 없음"),
            Self::ExpectedForm2 => write!(f, "Mode2 Form1을 Form2로 읽을 수 없음"),
            Self::Form1EdcMismatch => write!(f, "Mode2 Form1 EDC 불일치"),
            Self::Form2EdcMismatch => write!(f, "Mode2 Form2 EDC 불일치"),
            Self::EccPMismatch => write!(f, "Mode2 Form1 ECC P 불일치"),
            Self::EccQMismatch => write!(f, "Mode2 Form1 ECC Q 불일치"),
        }
    }
}

impl Error for SectorError {}

#[derive(Debug)]
struct Tables {
    forward: [u8; 256],
    backward: [u8; 256],
    edc: [u32; 256],
}

#[derive(Debug)]
struct ExpectedIntegrity {
    edc: [u8; 4],
    p: Vec<u8>,
    q: Vec<u8>,
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut forward = [0u8; 256];
        let mut backward = [0u8; 256];
        let mut edc = [0u32; 256];
        for value in 0..256usize {
            let doubled =
                (((value as u32) << 1) ^ if value & 0x80 != 0 { 0x11d } else { 0 }) & 0xff;
            forward[value] = doubled as u8;
            backward[value ^ doubled as usize] = value as u8;

            let mut remainder = value as u32;
            for _ in 0..8 {
                remainder = if remainder & 1 != 0 {
                    (remainder >> 1) ^ 0xd801_8001
                } else {
                    remainder >> 1
                };
            }
            edc[value] = remainder;
        }
        Tables {
            forward,
            backward,
            edc,
        }
    })
}

/// framing을 검증하고 Mode2 form을 판정한다.
pub fn classify(raw: &[u8]) -> Result<SectorKind, SectorError> {
    if raw.len() != SECTOR_LEN {
        return Err(SectorError::WrongLength {
            expected: SECTOR_LEN,
            actual: raw.len(),
        });
    }
    if raw[..SYNC.len()] != SYNC {
        return Err(SectorError::InvalidSync);
    }
    if raw[MODE_OFFSET] != 2 {
        return Err(SectorError::UnsupportedMode(raw[MODE_OFFSET]));
    }
    if raw[SUBHEADER_A] != raw[SUBHEADER_B] {
        return Err(SectorError::SubheaderMismatch);
    }
    Ok(if raw[SUBMODE_OFFSET] & FORM2_BIT == 0 {
        SectorKind::Form1
    } else {
        SectorKind::Form2
    })
}

fn compute_edc(data: &[u8], table: &[u32; 256]) -> u32 {
    let mut value = 0u32;
    for &byte in data {
        value = (value >> 8) ^ table[((value ^ u32::from(byte)) & 0xff) as usize];
    }
    value
}

fn compute_ecc(
    source: &[u8],
    major_count: usize,
    minor_count: usize,
    major_multiplier: usize,
    minor_increment: usize,
    table: &Tables,
) -> Vec<u8> {
    let matrix_size = major_count * minor_count;
    debug_assert!(source.len() >= matrix_size);
    let mut output = vec![0u8; major_count * 2];
    for major in 0..major_count {
        let mut index = (major >> 1) * major_multiplier + (major & 1);
        let mut parity_a = 0u8;
        let mut parity_b = 0u8;
        for _ in 0..minor_count {
            let value = source[index];
            index += minor_increment;
            if index >= matrix_size {
                index -= matrix_size;
            }
            parity_a ^= value;
            parity_b ^= value;
            parity_a = table.forward[parity_a as usize];
        }
        parity_a = table.backward[(table.forward[parity_a as usize] ^ parity_b) as usize];
        output[major] = parity_a;
        output[major + major_count] = parity_a ^ parity_b;
    }
    output
}

fn expected_integrity(raw: &[u8]) -> Result<ExpectedIntegrity, SectorError> {
    if classify(raw)? != SectorKind::Form1 {
        return Err(SectorError::ExpectedForm1);
    }
    let table = tables();
    let mut work = [0u8; SECTOR_LEN];
    work.copy_from_slice(raw);

    let edc = compute_edc(&work[0x10..FORM1_EDC_OFFSET], &table.edc).to_le_bytes();
    work[FORM1_EDC_OFFSET..ECC_P_OFFSET].copy_from_slice(&edc);
    work[0x0c..0x10].fill(0);
    work[ECC_P_OFFSET..ECC_END].fill(0);

    let p = compute_ecc(&work[0x0c..], 86, 24, 2, 86, table);
    work[ECC_P_OFFSET..ECC_Q_OFFSET].copy_from_slice(&p);
    let q = compute_ecc(&work[0x0c..], 52, 43, 86, 88, table);
    Ok(ExpectedIntegrity { edc, p, q })
}

/// Mode2 Form1 구조와 EDC/ECC를 검증하고 논리 사용자 데이터를 돌려준다.
pub fn verified_form1_data(raw: &[u8]) -> Result<&[u8], SectorError> {
    let expected = expected_integrity(raw)?;
    if raw[FORM1_EDC_OFFSET..ECC_P_OFFSET] != expected.edc {
        return Err(SectorError::Form1EdcMismatch);
    }
    if raw[ECC_P_OFFSET..ECC_Q_OFFSET] != expected.p {
        return Err(SectorError::EccPMismatch);
    }
    if raw[ECC_Q_OFFSET..ECC_END] != expected.q {
        return Err(SectorError::EccQMismatch);
    }
    Ok(&raw[FORM1_DATA_OFFSET..FORM1_DATA_OFFSET + FORM1_DATA_LEN])
}

/// Mode2 Form1 섹터의 EDC/ECC만 재계산한다.
///
/// sync, mode, subheader와 Form 종류는 원본 표현을 검증하며 바꾸지 않는다.
pub fn regenerate_form1(raw: &mut [u8]) -> Result<(), SectorError> {
    let expected = expected_integrity(raw)?;
    raw[FORM1_EDC_OFFSET..ECC_P_OFFSET].copy_from_slice(&expected.edc);
    raw[ECC_P_OFFSET..ECC_Q_OFFSET].copy_from_slice(&expected.p);
    raw[ECC_Q_OFFSET..ECC_END].copy_from_slice(&expected.q);
    Ok(())
}

fn expected_form2_edc(raw: &[u8]) -> Result<[u8; 4], SectorError> {
    if classify(raw)? != SectorKind::Form2 {
        return Err(SectorError::ExpectedForm2);
    }
    Ok(compute_edc(&raw[0x10..FORM2_EDC_OFFSET], &tables().edc).to_le_bytes())
}

/// Mode2 Form2 구조와 EDC를 검증하고 2,324바이트 사용자 데이터를 돌려준다.
pub fn verified_form2_data(raw: &[u8]) -> Result<&[u8], SectorError> {
    let expected = expected_form2_edc(raw)?;
    if raw[FORM2_EDC_OFFSET..ECC_END] != expected {
        return Err(SectorError::Form2EdcMismatch);
    }
    Ok(&raw[FORM1_DATA_OFFSET..FORM1_DATA_OFFSET + FORM2_DATA_LEN])
}

/// 섹터 form을 판정하고 해당 무결성 검증 뒤 사용자 데이터를 돌려준다.
pub fn verified_data(raw: &[u8]) -> Result<VerifiedSectorData<'_>, SectorError> {
    let kind = classify(raw)?;
    let data = match kind {
        SectorKind::Form1 => verified_form1_data(raw)?,
        SectorKind::Form2 => verified_form2_data(raw)?,
    };
    Ok(VerifiedSectorData { kind, data })
}

/// Mode2 Form2 섹터의 EDC만 재계산한다.
///
/// Form2에는 P/Q ECC가 없으며 나머지 표현은 바꾸지 않는다.
pub fn regenerate_form2(raw: &mut [u8]) -> Result<(), SectorError> {
    let expected = expected_form2_edc(raw)?;
    raw[FORM2_EDC_OFFSET..ECC_END].copy_from_slice(&expected);
    Ok(())
}

#[cfg(test)]
#[path = "sector_tests.rs"]
mod tests;
