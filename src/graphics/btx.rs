//! 기존 `graphics::btx` 공개 경로의 호환 re-export.
//!
//! BTX codec의 정본 소유자는 [`crate::btx`]이다. 저장소 내부 코드는
//! 중립 경로를 직접 참조하고, 이 module은 외부 호출 호환만 보존한다.

pub use crate::btx::*;

#[cfg(test)]
#[path = "btx_compat_tests.rs"]
mod tests;
