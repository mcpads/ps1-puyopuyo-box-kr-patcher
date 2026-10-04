//! 기계어 검산 — 조립 / 연속 해석 / 재조립의 3자 일치.
//!
//! 템플릿은 **어셈블리 원천과 선언한 대상 ISA 프로필의 조립·해석·재조립
//! 검산이 없는 기계어 생성·이동**을 자동 실패로 규정한다. 이 모듈이 그
//! 판정을 소유한다.
//!
//! ```text
//!   typed 원천 ──공통 checked assemble──▶ bytes A
//!                                         │ decode
//!                                         ▼
//!                                 Vec<Instruction>   (연속·완전·프로필 내부)
//!                                         │ encode
//!                                         ▼
//!                                      bytes C   ── A == C 이어야 승인
//! ```
//!
//! 세 조건이 모두 성립할 때만 기계어 쓰기가 승인된다. 부분 통과는 승인이
//! 아니다.

use super::decode::decode_slice;
use super::encode::encode;
use super::instruction::{Instruction, InstructionKind};
use super::profile::IsaProfile;
use super::typed_source::lower_instruction;
use typed_isa_core::StaticSemantics;

/// 검산 실패 사유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// 길이가 4바이트 경계에 맞지 않는다.
    Unaligned { len: usize },
    /// 검산 대상이 비어 있다. 빈 입력을 통과로 처리하지 않는다.
    Empty,
    /// 선언 프로필로 해석되지 않는 워드가 있다.
    OutsideProfile {
        profile: &'static str,
        addr: u32,
        word: u32,
    },
    /// 재조립 결과가 원본 바이트와 다르다.
    ReassemblyMismatch {
        addr: u32,
        original: u32,
        reassembled: u32,
    },
    /// 조립 결과가 어셈블리 원천이 선언한 명령 수와 다르다.
    LengthMismatch { expected: usize, actual: usize },
    /// 로컬 조사 표현을 제품용 공통 ISA 의미 표현으로 내릴 수 없다.
    TypedSourceOutsideProfile {
        profile: &'static str,
        index: usize,
        instruction: InstructionKind,
    },
    /// 공통 checked assembler가 배치·필드·제어 흐름 검산을 거부했다.
    CheckedAssembly {
        profile: &'static str,
        source: psx_r3000a::AssemblerError,
    },
    /// 공통 프로필이 배치된 typed 명령의 정적 의미를 만들지 못했다.
    StaticSemantics {
        profile: &'static str,
        addr: u32,
        source: psx_r3000a::SemanticError,
    },
    /// 공통 assembler 결과를 로컬 조사 표현으로 다시 읽은 의미가 다르다.
    TypedSourceSemanticMismatch {
        addr: u32,
        expected: InstructionKind,
        actual: InstructionKind,
    },
    /// 기계어 생성의 권위 있는 typed source ID가 비어 있다.
    EmptySourceId,
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unaligned { len } => {
                write!(f, "길이 {len}바이트는 4바이트 경계가 아니다")
            }
            Self::Empty => f.write_str("검산 대상이 비어 있다"),
            Self::OutsideProfile {
                profile,
                addr,
                word,
            } => write!(
                f,
                "0x{addr:08X}의 워드 0x{word:08X}가 프로필 {profile} 밖이다"
            ),
            Self::ReassemblyMismatch {
                addr,
                original,
                reassembled,
            } => write!(
                f,
                "0x{addr:08X} 재조립 불일치: 원본 0x{original:08X} != 재조립 0x{reassembled:08X}"
            ),
            Self::LengthMismatch { expected, actual } => {
                write!(f, "명령 수 불일치: 선언 {expected}, 실제 {actual}")
            }
            Self::TypedSourceOutsideProfile {
                profile,
                index,
                instruction,
            } => write!(
                f,
                "typed source #{index} {instruction:?}를 공통 프로필 {profile}로 내릴 수 없다"
            ),
            Self::CheckedAssembly { profile, source } => {
                write!(f, "공통 프로필 {profile} checked assembly 실패: {source}")
            }
            Self::StaticSemantics {
                profile,
                addr,
                source,
            } => write!(
                f,
                "0x{addr:08X} 공통 프로필 {profile} 정적 의미 검산 실패: {source}"
            ),
            Self::TypedSourceSemanticMismatch {
                addr,
                expected,
                actual,
            } => write!(
                f,
                "0x{addr:08X} typed source 의미 불일치: 선언 {expected:?}, 실제 {actual:?}"
            ),
            Self::EmptySourceId => f.write_str("typed 기계어 source ID가 비어 있다"),
        }
    }
}

impl std::error::Error for VerifyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CheckedAssembly { source, .. } => Some(source),
            Self::StaticSemantics { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// 검산을 통과한 기계어 구간.
///
/// 원본 분석용 [`verify_bytes`]와 제품 생성용 [`verify_typed_source`]가
/// 모두 이 타입을 만들지만, 후자만 `typed_source_id`와 공통
/// `checked_profile_id`를 가진다. 쓰기 계획은 두 provenance를 모두 요구한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedMachineCode {
    profile: IsaProfile,
    base_addr: u32,
    bytes: Vec<u8>,
    instructions: Vec<Instruction>,
    typed_source_id: Option<String>,
    checked_profile_id: Option<&'static str>,
}

impl VerifiedMachineCode {
    pub fn profile(&self) -> IsaProfile {
        self.profile
    }
    pub fn base_addr(&self) -> u32 {
        self.base_addr
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }
    pub fn len_words(&self) -> usize {
        self.instructions.len()
    }
    /// 제품 기계어를 생성한 권위 있는 typed source ID.
    ///
    /// 원본 분석용 [`verify_bytes`]와 preassembled 검산에는 값이 없으며,
    /// [`verify_typed_source`]로 만든 결과에만 있다.
    pub fn typed_source_id(&self) -> Option<&str> {
        self.typed_source_id.as_deref()
    }
    /// 제품용 typed source를 조립·검산한 공통 ISA 프로필 ID.
    pub fn checked_profile_id(&self) -> Option<&'static str> {
        self.checked_profile_id
    }
}

/// 바이트열이 선언 프로필에서 연속 해석되고 그대로 재조립되는지 검산한다.
///
/// 검사 순서는 실패를 조기에 드러내도록 고정한다.
///
/// 1. 정렬·비어 있지 않음
/// 2. 전 워드가 프로필 안에서 해석됨 (`Unknown` 0개)
/// 3. 재조립 결과가 원본과 워드 단위로 동일
pub fn verify_bytes(
    profile: IsaProfile,
    base_addr: u32,
    bytes: &[u8],
) -> Result<VerifiedMachineCode, VerifyError> {
    if bytes.is_empty() {
        return Err(VerifyError::Empty);
    }
    let instructions =
        decode_slice(base_addr, bytes).ok_or(VerifyError::Unaligned { len: bytes.len() })?;

    for ins in &instructions {
        if !profile.contains(&ins.kind) {
            return Err(VerifyError::OutsideProfile {
                profile: profile.name(),
                addr: ins.addr,
                word: ins.word,
            });
        }
    }

    for ins in &instructions {
        let reassembled = encode(ins.kind);
        if reassembled != ins.word {
            return Err(VerifyError::ReassemblyMismatch {
                addr: ins.addr,
                original: ins.word,
                reassembled,
            });
        }
    }

    Ok(VerifiedMachineCode {
        profile,
        base_addr,
        bytes: bytes.to_vec(),
        instructions,
        typed_source_id: None,
        checked_profile_id: None,
    })
}

/// typed [`InstructionKind`] 원천을 공통 checked assembler로 조립하고, 각
/// 배치 명령의 정적 의미와 프로젝트의 원본 보존 분석 adapter에서
/// 의미·바이트가 일치하는지 검산한다.
///
/// 제품 기계어의 권위 있는 원천은 raw byte 배열이 아니라 이 함수에 전달한
/// typed 명령 목록이다. [`crate::patch::WritePlan`]은 이 경로로 생성한
/// [`VerifiedMachineCode`]만 기계어 Expected Write로 승인한다.
pub fn verify_typed_source(
    profile: IsaProfile,
    base_addr: u32,
    source_id: &str,
    source: &[InstructionKind],
) -> Result<VerifiedMachineCode, VerifyError> {
    if source_id.trim().is_empty() {
        return Err(VerifyError::EmptySourceId);
    }
    if base_addr & 3 != 0 {
        return Err(VerifyError::CheckedAssembly {
            profile: psx_r3000a::PROFILE_ID,
            source: psx_r3000a::AssemblerError::UnalignedOrigin { origin: base_addr },
        });
    }

    let mut checked_source = Vec::with_capacity(source.len());
    for (index, &instruction) in source.iter().enumerate() {
        let offset = u32::try_from(index)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or(VerifyError::CheckedAssembly {
                profile: psx_r3000a::PROFILE_ID,
                source: psx_r3000a::AssemblerError::AddressOverflow {
                    origin: base_addr,
                    instruction_index: index,
                },
            })?;
        let address = base_addr
            .checked_add(offset)
            .ok_or(VerifyError::CheckedAssembly {
                profile: psx_r3000a::PROFILE_ID,
                source: psx_r3000a::AssemblerError::AddressOverflow {
                    origin: base_addr,
                    instruction_index: index,
                },
            })?;
        let checked = lower_instruction(instruction, address).ok_or(
            VerifyError::TypedSourceOutsideProfile {
                profile: psx_r3000a::PROFILE_ID,
                index,
                instruction,
            },
        )?;
        checked_source.push(checked);
    }

    let mut assembler = psx_r3000a::Assembler::new();
    assembler.emit_all(checked_source);
    let assembled =
        assembler
            .assemble(base_addr)
            .map_err(|source| VerifyError::CheckedAssembly {
                profile: psx_r3000a::PROFILE_ID,
                source,
            })?;
    for span in assembled.instruction_spans() {
        psx_r3000a::PsxR3000A::semantics(&span.instruction, &span.location).map_err(|source| {
            VerifyError::StaticSemantics {
                profile: psx_r3000a::PROFILE_ID,
                addr: span.location,
                source,
            }
        })?;
    }
    let mut verified = verify_bytes(profile, base_addr, assembled.bytes())?;
    if verified.len_words() != source.len() {
        return Err(VerifyError::LengthMismatch {
            expected: source.len(),
            actual: verified.len_words(),
        });
    }
    for (&expected, actual) in source.iter().zip(verified.instructions()) {
        if expected != actual.kind {
            return Err(VerifyError::TypedSourceSemanticMismatch {
                addr: actual.addr,
                expected,
                actual: actual.kind,
            });
        }
    }
    verified.typed_source_id = Some(source_id.to_owned());
    verified.checked_profile_id = Some(psx_r3000a::PROFILE_ID);
    Ok(verified)
}

#[cfg(test)]
#[path = "verify_tests.rs"]
mod verify_tests;
