//! MIPS R3000A (PlayStation) 명령 집합.
//!
//! 이 모듈의 책임은 두 가지로 나뉜다.
//!
//! - **조사**: 원본 코드를 읽고 이해한다. 프로필 밖 워드를 `Unknown`으로
//!   남겨도 된다.
//! - **검산**: 패치가 만들거나 옮기는 기계어를 승인한다. `Unknown`이 하나도
//!   없어야 하고 재조립이 원본과 같아야 한다.
//!
//! 두 책임은 공통 `psx-r3000a` 의미 모델을 공유하되 판정 기준이 다르다.
//! 조사는 이 프로젝트가 raw word·`Unknown`을 보존하고, 검산은 공통 crate의
//! 엄격 decode·재조립을 사용한다. 판정은 [`verify`]가 소유한다.

pub mod decode;
pub mod display;
pub mod encode;
pub mod instruction;
pub mod profile;
mod typed_source;
pub mod verify;

pub use decode::{decode, decode_slice, decode_word};
pub use display::{listing_line, Asm};
pub use encode::{encode, encode_all, encode_gte};
pub use instruction::{FlowKind, GteCommand, Instruction, InstructionKind, Reg};
pub use profile::IsaProfile;
pub use verify::{verify_bytes, verify_typed_source, VerifiedMachineCode, VerifyError};

#[cfg(test)]
#[path = "psx_profile_compatibility_tests.rs"]
mod psx_profile_compatibility_tests;
