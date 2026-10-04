//! 기존 `graphics::qmp` 공개 경로의 호환 re-export.
//!
//! QMP 구조와 원본 inventory 검증의 정본 소유자는 [`crate::qmp`]이다.

pub use crate::qmp::*;

#[cfg(test)]
#[path = "qmp_compat_tests.rs"]
mod tests;
