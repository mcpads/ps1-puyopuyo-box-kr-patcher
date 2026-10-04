//! 선형 스윕 디스어셈블.
//!
//! 텍스트 세그먼트를 4바이트 경계에서 처음부터 끝까지 해석한다. 코드와
//! 데이터를 구분하지 않으므로 **데이터 구간은 의미 없는 명령으로 보인다.**
//! 도달 가능성을 주장하려면 [`super::recursive`]를 쓴다.
//!
//! 쓰임: 특정 비트 패턴(BIOS 스텁, 특정 상수 로드)이 어디에 있는지 훑을 때.

use crate::coords::ExeMap;
use crate::isa::{decode, Instruction};

/// 텍스트 세그먼트 전체를 선형 해석한다.
///
/// `text`는 PSX-EXE 헤더(`0x800`)를 제외한 텍스트 바이트다. 길이가 4의
/// 배수가 아니면 `None` — 부분 결과를 정상 산출물로 돌려주지 않는다.
pub fn sweep(map: &ExeMap, text: &[u8]) -> Option<Vec<Instruction>> {
    if !text.len().is_multiple_of(4) {
        return None;
    }
    Some(
        text.chunks_exact(4)
            .enumerate()
            .map(|(i, c)| {
                let addr = map.load_addr.wrapping_add((i * 4) as u32);
                decode(addr, u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            })
            .collect(),
    )
}

/// 선형 해석 결과에서 술어를 만족하는 명령을 고른다.
pub fn find<F>(instructions: &[Instruction], pred: F) -> Vec<&Instruction>
where
    F: Fn(&Instruction) -> bool,
{
    instructions.iter().filter(|i| pred(i)).collect()
}

/// 프로필로 해석되지 않은 워드의 비율.
///
/// 높으면 그 구간이 코드가 아니라 데이터일 가능성이 크다. 판정이 아니라
/// 힌트다.
pub fn unknown_ratio(instructions: &[Instruction]) -> f64 {
    if instructions.is_empty() {
        return 0.0;
    }
    let unknown = instructions.iter().filter(|i| !i.kind.is_decoded()).count();
    unknown as f64 / instructions.len() as f64
}

#[cfg(test)]
#[path = "linear_tests.rs"]
mod linear_tests;
