//! 완성형 한글 음절의 받침에 따른 한국어 조사 선택.
//!
//! 번역문이나 문장 구조를 만들지 않는다. 호출자가 사람이 정한 조사 역할과
//! 발음 끝음절을 넘기면, 현대 한글 완성형의 종성만 판정한다.

use std::error::Error;
use std::fmt;

/// 조사 선택에 현대 한글 완성형이 아닌 문자를 사용한 오류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KoreanParticleError {
    character: char,
}

impl fmt::Display for KoreanParticleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "조사 선택의 발음 끝음절이 현대 한글 완성형이 아님: {:?}",
            self.character
        )
    }
}

impl Error for KoreanParticleError {}

fn has_batchim(character: char) -> Result<bool, KoreanParticleError> {
    if !('가'..='힣').contains(&character) {
        return Err(KoreanParticleError { character });
    }
    Ok(!((character as u32 - '가' as u32).is_multiple_of(28)))
}

/// 발음 끝음절에 맞는 주격 조사 `이/가`를 고른다.
pub fn select_subject_particle(pronunciation_tail: char) -> Result<char, KoreanParticleError> {
    Ok(if has_batchim(pronunciation_tail)? {
        '이'
    } else {
        '가'
    })
}

/// 발음 끝음절에 맞는 목적격 조사 `을/를`을 고른다.
pub fn select_object_particle(pronunciation_tail: char) -> Result<char, KoreanParticleError> {
    Ok(if has_batchim(pronunciation_tail)? {
        '을'
    } else {
        '를'
    })
}

#[cfg(test)]
#[path = "korean_particle_tests.rs"]
mod tests;
