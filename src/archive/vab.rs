//! PlayStation split VAB header/body의 검증된 구조.
//!
//! `.VH`는 고정 header와 128개 program 표, 실제 program 수만큼의 tone
//! block, 256개 wave 크기 표를 가진다. tone block은 program 번호가 아니라
//! tone 수가 0이 아닌 program 표 항목의 순서로 저장된다. `.VB`의 sample은
//! wave 크기 표의 8바이트 단위를 차례로 합친 범위다.

use std::error::Error;
use std::fmt;
use std::ops::Range;

const VAB_MAGIC: &[u8; 4] = b"pBAV";
const VAB_VERSION: u32 = 7;
const HEADER_BYTES: usize = 0x20;
const PROGRAM_COUNT: usize = 128;
const PROGRAM_BYTES: usize = 0x10;
const TONES_PER_PROGRAM: usize = 16;
const TONE_BYTES: usize = 0x20;
const WAVE_SIZE_COUNT: usize = 256;
const WAVE_SIZE_BYTES: usize = 2;
const SAMPLE_SIZE_UNIT: usize = 8;
const MIN_HEADER_BYTES: usize =
    HEADER_BYTES + PROGRAM_COUNT * PROGRAM_BYTES + WAVE_SIZE_COUNT * WAVE_SIZE_BYTES;

/// VAB sample body 안의 한 VAG 범위.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VabSample {
    pub number: u16,
    pub body_range: Range<usize>,
}

/// 한 program tone이 참조하는 sample과 음정 속성.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VabTone {
    pub number: u8,
    pub program_number: u8,
    pub sample_number: u16,
    pub volume: u8,
    pub pan: u8,
    pub center: u8,
    pub shift: u8,
}

/// 128개 program 번호 공간에서 실제 tone을 가진 program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VabProgram {
    pub number: u8,
    pub volume: u8,
    pub priority: u8,
    pub mode: u8,
    pub pan: u8,
    pub tones: Vec<VabTone>,
}

/// `.VH/.VB` 한 쌍에서 검증한 VAB bank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VabBank {
    pub version: u32,
    pub bank_id: u32,
    pub declared_bytes: usize,
    pub master_volume: u8,
    pub master_pan: u8,
    pub programs: Vec<VabProgram>,
    pub samples: Vec<VabSample>,
}

impl VabBank {
    /// split VAB header와 sample body의 모든 길이·참조를 검증한다.
    pub fn parse(header: &[u8], body: &[u8]) -> Result<Self, VabError> {
        if header.len() < MIN_HEADER_BYTES {
            return Err(VabError::HeaderTooShort(header.len()));
        }
        if &header[..4] != VAB_MAGIC {
            return Err(VabError::InvalidMagic(header[..4].try_into().unwrap()));
        }
        let version = read_u32(header, 4);
        if version != VAB_VERSION {
            return Err(VabError::UnsupportedVersion(version));
        }
        let bank_id = read_u32(header, 8);
        let declared_bytes = read_u32(header, 0x0c) as usize;
        let declared_programs = read_u16(header, 0x12) as usize;
        let declared_tones = read_u16(header, 0x14) as usize;
        let declared_samples = read_u16(header, 0x16) as usize;
        if declared_programs > PROGRAM_COUNT {
            return Err(VabError::ProgramCountOutOfRange(declared_programs));
        }
        if declared_samples >= WAVE_SIZE_COUNT {
            return Err(VabError::SampleCountOutOfRange(declared_samples));
        }
        let expected_header_bytes = HEADER_BYTES
            + PROGRAM_COUNT * PROGRAM_BYTES
            + declared_programs * TONES_PER_PROGRAM * TONE_BYTES
            + WAVE_SIZE_COUNT * WAVE_SIZE_BYTES;
        if header.len() != expected_header_bytes {
            return Err(VabError::HeaderLengthMismatch {
                expected: expected_header_bytes,
                actual: header.len(),
            });
        }

        let active_program_numbers = (0..PROGRAM_COUNT)
            .filter(|number| header[program_offset(*number)] != 0)
            .collect::<Vec<_>>();
        if active_program_numbers.len() != declared_programs {
            return Err(VabError::ActiveProgramCountMismatch {
                declared: declared_programs,
                actual: active_program_numbers.len(),
            });
        }

        let tone_table = HEADER_BYTES + PROGRAM_COUNT * PROGRAM_BYTES;
        let mut programs = Vec::with_capacity(declared_programs);
        let mut actual_tones = 0usize;
        for (compact_number, program_number) in active_program_numbers.into_iter().enumerate() {
            let program_base = program_offset(program_number);
            let tone_count = header[program_base] as usize;
            if tone_count > TONES_PER_PROGRAM {
                return Err(VabError::ToneCountOutOfRange {
                    program: program_number,
                    tones: tone_count,
                });
            }
            actual_tones += tone_count;
            let mut tones = Vec::with_capacity(tone_count);
            for tone_number in 0..tone_count {
                let tone_base =
                    tone_table + (compact_number * TONES_PER_PROGRAM + tone_number) * TONE_BYTES;
                let stored_program = header[tone_base + 0x14] as usize;
                if stored_program != program_number {
                    return Err(VabError::ToneProgramMismatch {
                        program: program_number,
                        tone: tone_number,
                        stored: stored_program,
                    });
                }
                let sample_number = read_u16(header, tone_base + 0x16) as usize;
                if sample_number == 0 || sample_number > declared_samples {
                    return Err(VabError::ToneSampleOutOfRange {
                        program: program_number,
                        tone: tone_number,
                        sample: sample_number,
                        samples: declared_samples,
                    });
                }
                tones.push(VabTone {
                    number: tone_number as u8,
                    program_number: program_number as u8,
                    sample_number: sample_number as u16,
                    volume: header[tone_base + 2],
                    pan: header[tone_base + 3],
                    center: header[tone_base + 4],
                    shift: header[tone_base + 5],
                });
            }
            programs.push(VabProgram {
                number: program_number as u8,
                volume: header[program_base + 1],
                priority: header[program_base + 2],
                mode: header[program_base + 3],
                pan: header[program_base + 4],
                tones,
            });
        }
        if actual_tones != declared_tones {
            return Err(VabError::TotalToneCountMismatch {
                declared: declared_tones,
                actual: actual_tones,
            });
        }

        let wave_table = tone_table + declared_programs * TONES_PER_PROGRAM * TONE_BYTES;
        let mut samples = Vec::with_capacity(declared_samples);
        let mut body_offset = 0usize;
        for sample_number in 1..=declared_samples {
            let size_units =
                read_u16(header, wave_table + sample_number * WAVE_SIZE_BYTES) as usize;
            if size_units == 0 {
                return Err(VabError::EmptySample(sample_number));
            }
            let sample_bytes = size_units
                .checked_mul(SAMPLE_SIZE_UNIT)
                .ok_or(VabError::SizeOverflow)?;
            let end = body_offset
                .checked_add(sample_bytes)
                .ok_or(VabError::SizeOverflow)?;
            samples.push(VabSample {
                number: sample_number as u16,
                body_range: body_offset..end,
            });
            body_offset = end;
        }
        if body_offset != body.len() {
            return Err(VabError::BodyLengthMismatch {
                expected: body_offset,
                actual: body.len(),
            });
        }
        let actual_bytes = header
            .len()
            .checked_add(body.len())
            .ok_or(VabError::SizeOverflow)?;
        if declared_bytes != actual_bytes {
            return Err(VabError::DeclaredLengthMismatch {
                declared: declared_bytes,
                actual: actual_bytes,
            });
        }

        Ok(Self {
            version,
            bank_id,
            declared_bytes,
            master_volume: header[0x18],
            master_pan: header[0x19],
            programs,
            samples,
        })
    }

    /// VAB body의 byte offset을 소유하는 sample을 찾는다.
    pub fn sample_containing(&self, body_offset: usize) -> Option<&VabSample> {
        self.samples
            .iter()
            .find(|sample| sample.body_range.contains(&body_offset))
    }

    /// 지정 sample을 참조하는 모든 program/tone을 반환한다.
    pub fn tones_for_sample(&self, sample_number: u16) -> Vec<&VabTone> {
        self.programs
            .iter()
            .flat_map(|program| &program.tones)
            .filter(|tone| tone.sample_number == sample_number)
            .collect()
    }
}

/// VAB 구조 또는 교차 참조 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VabError {
    HeaderTooShort(usize),
    InvalidMagic([u8; 4]),
    UnsupportedVersion(u32),
    ProgramCountOutOfRange(usize),
    SampleCountOutOfRange(usize),
    HeaderLengthMismatch {
        expected: usize,
        actual: usize,
    },
    ActiveProgramCountMismatch {
        declared: usize,
        actual: usize,
    },
    ToneCountOutOfRange {
        program: usize,
        tones: usize,
    },
    ToneProgramMismatch {
        program: usize,
        tone: usize,
        stored: usize,
    },
    ToneSampleOutOfRange {
        program: usize,
        tone: usize,
        sample: usize,
        samples: usize,
    },
    TotalToneCountMismatch {
        declared: usize,
        actual: usize,
    },
    EmptySample(usize),
    BodyLengthMismatch {
        expected: usize,
        actual: usize,
    },
    DeclaredLengthMismatch {
        declared: usize,
        actual: usize,
    },
    SizeOverflow,
}

impl fmt::Display for VabError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderTooShort(actual) => write!(f, "VAB header 부족: {actual}B"),
            Self::InvalidMagic(actual) => write!(f, "VAB magic 불일치: {actual:02X?}"),
            Self::UnsupportedVersion(version) => write!(f, "VAB version 미지원: {version}"),
            Self::ProgramCountOutOfRange(programs) => {
                write!(f, "VAB program 수 범위 이탈: {programs}")
            }
            Self::SampleCountOutOfRange(samples) => {
                write!(f, "VAB sample 수 범위 이탈: {samples}")
            }
            Self::HeaderLengthMismatch { expected, actual } => write!(
                f,
                "VAB header 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::ActiveProgramCountMismatch { declared, actual } => write!(
                f,
                "VAB 활성 program 수 불일치: declared={declared}, actual={actual}"
            ),
            Self::ToneCountOutOfRange { program, tones } => write!(
                f,
                "VAB program {program} tone 수 범위 이탈: {tones}"
            ),
            Self::ToneProgramMismatch {
                program,
                tone,
                stored,
            } => write!(
                f,
                "VAB tone program 참조 불일치: program={program}, tone={tone}, stored={stored}"
            ),
            Self::ToneSampleOutOfRange {
                program,
                tone,
                sample,
                samples,
            } => write!(
                f,
                "VAB tone sample 참조 범위 이탈: program={program}, tone={tone}, sample={sample}, samples={samples}"
            ),
            Self::TotalToneCountMismatch { declared, actual } => write!(
                f,
                "VAB 전체 tone 수 불일치: declared={declared}, actual={actual}"
            ),
            Self::EmptySample(sample) => write!(f, "VAB sample {sample} 크기가 0"),
            Self::BodyLengthMismatch { expected, actual } => write!(
                f,
                "VAB body 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::DeclaredLengthMismatch { declared, actual } => write!(
                f,
                "VAB 선언 길이 불일치: declared={declared}, actual={actual}"
            ),
            Self::SizeOverflow => write!(f, "VAB 크기 계산 overflow"),
        }
    }
}

impl Error for VabError {}

fn program_offset(number: usize) -> usize {
    HEADER_BYTES + number * PROGRAM_BYTES
}

fn read_u16(raw: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(raw[offset..offset + 2].try_into().unwrap())
}

fn read_u32(raw: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap())
}

#[cfg(test)]
#[path = "vab_tests.rs"]
mod vab_tests;
