//! 가나를 포함한 연속 2바이트 Shift-JIS 후보 run.
//!
//! 이 휴리스틱은 코드표·token·consumer를 모르는 저장층의 조사 하한만
//! 만든다. 엄격한 Shift-JIS decode, 실제 실행 도달, 번역 모집단 판정은
//! 호출자가 별도로 보존해야 한다.

/// 연속 2바이트 후보 run 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KanaRun<'a> {
    pub offset: usize,
    pub raw: &'a [u8],
    pub hiragana: usize,
    pub katakana: usize,
}

/// 기존 조사와 같은 가나 포함 Shift-JIS run을 찾는다.
pub fn scan_kana_runs(raw: &[u8], min_code_units: usize) -> Vec<KanaRun<'_>> {
    let mut out = Vec::new();
    let mut run_start = None;
    let mut code_units = 0usize;
    let mut hiragana = 0usize;
    let mut katakana = 0usize;
    let mut index = 0usize;

    while index + 1 < raw.len() {
        let hi = raw[index];
        let lo = raw[index + 1];
        if valid_heuristic_pair(hi, lo) {
            run_start.get_or_insert(index);
            code_units += 1;
            let code = u16::from_be_bytes([hi, lo]);
            hiragana += usize::from((0x829F..=0x82F1).contains(&code));
            katakana += usize::from((0x8340..=0x8396).contains(&code));
            index += 2;
        } else {
            finish_run(
                raw,
                index,
                min_code_units,
                &mut out,
                &mut run_start,
                &mut code_units,
                &mut hiragana,
                &mut katakana,
            );
            index += 1;
        }
    }
    finish_run(
        raw,
        index,
        min_code_units,
        &mut out,
        &mut run_start,
        &mut code_units,
        &mut hiragana,
        &mut katakana,
    );
    out
}

#[allow(clippy::too_many_arguments)]
fn finish_run<'a>(
    raw: &'a [u8],
    end: usize,
    min_code_units: usize,
    out: &mut Vec<KanaRun<'a>>,
    run_start: &mut Option<usize>,
    code_units: &mut usize,
    hiragana: &mut usize,
    katakana: &mut usize,
) {
    if let Some(start) = *run_start {
        if *code_units >= min_code_units && (*hiragana != 0 || *katakana != 0) {
            out.push(KanaRun {
                offset: start,
                raw: &raw[start..end],
                hiragana: *hiragana,
                katakana: *katakana,
            });
        }
    }
    *run_start = None;
    *code_units = 0;
    *hiragana = 0;
    *katakana = 0;
}

fn valid_heuristic_pair(hi: u8, lo: u8) -> bool {
    ((0x81..=0x9F).contains(&hi) || (0xE0..=0xEA).contains(&hi))
        && ((0x40..=0x7E).contains(&lo) || (0x80..=0xFC).contains(&lo))
}

#[cfg(test)]
#[path = "kana_runs_tests.rs"]
mod kana_runs_tests;
