//! 표시 문자열 인코딩 경계.
//!
//! 원본 일본어는 Shift-JIS로 엄격하게 decode·encode한다. 한글 custom
//! mapping은 target별 font/code-table 모듈이 소유하며 이 경계를
//! 조용히 우회하지 않는다.

pub mod kana_runs;
pub mod korean_particle;
pub mod sjis;

pub use kana_runs::{scan_kana_runs, KanaRun};
pub use korean_particle::{select_object_particle, select_subject_particle, KoreanParticleError};
pub use sjis::{decode_sjis, encode_sjis, SjisError};
