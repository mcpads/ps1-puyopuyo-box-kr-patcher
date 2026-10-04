//! 엄격한 Shift-JIS decode·encode.
//!
//! `encoding_rs`의 Shift_JIS는 디코딩 오류를 U+FFFD로, 인코딩 누락을
//! numeric character reference로 대체할 수 있다. 제품 경로는
//! `had_errors`를 검사해 어느 대체 결과도 반환하지 않는다.

use std::error::Error;
use std::fmt;

use encoding_rs::SHIFT_JIS;

/// Shift-JIS 변환 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SjisError {
    InvalidBytes,
    UnmappableCharacters(Vec<char>),
}

impl fmt::Display for SjisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBytes => write!(f, "유효하지 않은 Shift-JIS 바이트"),
            Self::UnmappableCharacters(characters) => {
                write!(f, "Shift-JIS mapping이 없는 문자: ")?;
                for (index, character) in characters.iter().enumerate() {
                    if index != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{character} (U+{:04X})", *character as u32)?;
                }
                Ok(())
            }
        }
    }
}

impl Error for SjisError {}

/// 오류 대체 문자를 허용하지 않고 Shift-JIS를 Unicode로 바꾼다.
pub fn decode_sjis(raw: &[u8]) -> Result<String, SjisError> {
    let (decoded, _, had_errors) = SHIFT_JIS.decode(raw);
    if had_errors {
        return Err(SjisError::InvalidBytes);
    }
    Ok(decoded.into_owned())
}

/// mapping 누락을 numeric character reference로 대체하지 않고 실패한다.
///
/// 오류에는 누락 문자를 입력 순서의 고유 집합으로 담으며, 부분 인코딩
/// 바이트는 반환하지 않는다.
pub fn encode_sjis(text: &str) -> Result<Vec<u8>, SjisError> {
    let (encoded, _, had_errors) = SHIFT_JIS.encode(text);
    if !had_errors {
        return Ok(encoded.into_owned());
    }

    let mut unmappable = Vec::new();
    for character in text.chars() {
        let mut utf8 = [0u8; 4];
        let character_text = character.encode_utf8(&mut utf8);
        let (_, _, character_error) = SHIFT_JIS.encode(character_text);
        if character_error && !unmappable.contains(&character) {
            unmappable.push(character);
        }
    }
    Err(SjisError::UnmappableCharacters(unmappable))
}

#[cfg(test)]
#[path = "sjis_tests.rs"]
mod sjis_tests;
