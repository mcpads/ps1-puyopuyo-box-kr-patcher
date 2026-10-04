//! 공식전 카드의 고정 셀 문구를 원본 stream 공간 안에서 조판한다.
//!
//! 줄바꿈만 이동할 수 있고 나머지 control의 위치·인자는 보존한다.
//! 결과는 개발 빌드에서 실행 검증할 삽입 후보다. 내부 진입과 공유 atlas의
//! 전체 소비 검증·사람 승인은 이 조판 성공만으로 완료하지 않는다.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MANIFESTS: [&str; 3] = [
    include_str!("../../assets/translations/official-card/messages.tsv"),
    include_str!("../../assets/translations/official-card/pocket-messages.tsv"),
    include_str!("../../assets/translations/official-card/pocket-restored.tsv"),
];
const SOURCE_SHA256: &str = "54c0b713e7148f7ef3f15df4e6c7fd2d496e331f03c3cd9e113c927a6d4e53ab";
const SOURCE_GLYPHS: &str = concat!(
    "あいえかきくけさしすせたつてなに",
    "のはへまよらりるれろをんがだづで",
    "ぷっアカクタチトフマメモリロデド",
    "ブェォッ作成選見差込失敗上書空不",
    "足必要中抜BOXー。？／１２スセ"
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialCardStream {
    pub offset: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    Control(u16, Vec<u16>),
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn source_parts(bytes: &[u8]) -> Result<Vec<Part>, String> {
    if !bytes.len().is_multiple_of(2) {
        return Err("카드 stream word 정렬 오류".into());
    }
    let words: Vec<_> = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    let mut parts = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let code = words[i];
        i += 1;
        if code < 0xff00 {
            let ch = if code == 106 {
                Some('口')
            } else if code >= 112 {
                "ポケテションコミダウ８「！」終了"
                    .chars()
                    .nth(usize::from(code - 112))
            } else {
                code.checked_sub(24)
                    .and_then(|n| SOURCE_GLYPHS.chars().nth(usize::from(n)))
            }
            .ok_or_else(|| format!("카드 원문 미등록 셀: {code}"))?;
            if let Some(Part::Text(text)) = parts.last_mut() {
                text.push(ch);
            } else {
                parts.push(Part::Text(ch.to_string()));
            }
            continue;
        }
        // 판독한 일반 카드·PocketStation 문구의 control만 허용한다.
        let arity = match code {
            0xff00 | 0xff02 => 0,
            0xff11 | 0xff13 => 2,
            0xff16 => 1,
            _ => return Err(format!("카드 미등록 control: {code:04X}")),
        };
        let args = words
            .get(i..i + arity)
            .ok_or("카드 control 인자 잘림")?
            .to_vec();
        i += arity;
        if code == 0xff11 && args != [16, 16] {
            return Err("카드 셀 증분 불일치".into());
        }
        parts.push(Part::Control(code, args));
        if code == 0xff00 {
            if i != words.len() {
                return Err("카드 종료 뒤 미등록 데이터".into());
            }
            return Ok(parts);
        }
    }
    Err("카드 종료 control 누락".into())
}

fn visible(parts: &[Part]) -> String {
    let mut text = String::new();
    for part in parts {
        match part {
            Part::Text(s) => text.push_str(s),
            Part::Control(0xff02, _) => text.push_str("\\n"),
            Part::Control(0xff13, a) => text.push_str(&format!("<FF13:{},{}>", a[0], a[1])),
            _ => (),
        }
    }
    text
}

fn rewrite(
    bytes: &[u8],
    source: &str,
    ko: &str,
    glyphs: &BTreeMap<char, u16>,
) -> Result<Vec<u8>, String> {
    let parts = source_parts(bytes)?;
    if visible(&parts) != source {
        return Err("카드 판독 원문 불일치".into());
    }
    let mut rest = ko;
    let mut runs = Vec::new();
    let mut capacities = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        match part {
            Part::Text(s) => {
                let end = rest.find(['\\', '<']).unwrap_or(rest.len());
                let text = &rest[..end];
                if text.is_empty() {
                    return Err("카드 번역 run 누락".into());
                }
                let cells: Vec<_> = text
                    .chars()
                    .map(|ch| {
                        glyphs
                            .get(&ch)
                            .copied()
                            .ok_or_else(|| format!("카드 번역 글리프 누락: {ch}"))
                    })
                    .collect::<Result<_, _>>()?;
                if cells.len() > 14 {
                    return Err("카드 줄 너비 14셀 초과".into());
                }
                rest = &rest[end..];
                runs.push((index, cells));
                capacities.push(s.chars().count());
            }
            Part::Control(code, args) if matches!(code, 0xff02 | 0xff13) => {
                let token = if *code == 0xff02 {
                    "\\n".into()
                } else {
                    format!("<FF13:{},{}>", args[0], args[1])
                };
                rest = rest
                    .strip_prefix(&token)
                    .ok_or("카드 번역 제어·좌표 불일치")?;
            }
            _ => (),
        }
    }
    if !rest.is_empty() {
        return Err("카드 번역 잔여 문자".into());
    }
    let blank = *glyphs.get(&' ').ok_or("카드 공백 글리프 누락")?;
    let mut output = Vec::new();
    let mut group = Vec::new();
    // FF02 사이에서만 공간을 재배분한다. FF13 앞 선택 문구는 원래 셀 수를
    // 채워 다음 선택지의 상대 x 좌표와 control의 저장 위치를 유지한다.
    for (index, part) in parts.iter().enumerate() {
        if matches!(part, Part::Text(_) | Part::Control(0xff02, _)) {
            group.push(index);
            continue;
        }
        let positions: Vec<_> = runs
            .iter()
            .enumerate()
            .filter_map(|(r, (p, _))| group.contains(p).then_some(r))
            .collect();
        let capacity: usize = positions.iter().map(|&r| capacities[r]).sum();
        let used: usize = positions.iter().map(|&r| runs[r].1.len()).sum();
        let mut spare = capacity
            .checked_sub(used)
            .ok_or("카드 control 사이 저장 공간 초과")?;
        for &r in &positions {
            let add = spare.min(14 - runs[r].1.len());
            runs[r].1.extend(std::iter::repeat_n(blank, add));
            spare -= add;
        }
        if spare != 0 {
            return Err("카드 공백 조판 공간 부족".into());
        }
        for p in group.drain(..) {
            if let Some((_, cells)) = runs.iter().find(|(i, _)| *i == p) {
                output.extend(cells);
            } else {
                output.push(0xff02);
            }
        }
        if let Part::Control(code, args) = part {
            output.push(*code);
            output.extend(args);
        }
    }
    let output: Vec<_> = output.iter().flat_map(|w| w.to_le_bytes()).collect();
    if output.len() != bytes.len() {
        return Err("카드 stream 크기 불일치".into());
    }
    Ok(output)
}

/// 원본과 작성한 일반 카드·PocketStation 문구를 검증하고, 별도 글리프 공급에 사용할 조판 후보를 만든다.
/// 셀 매핑은 24..127 안에서 중복이 없어야 하며 공백도 16px 셀로 공급해야 한다.
pub fn compile_official_card_streams(
    source: &[u8],
    glyphs: &BTreeMap<char, u16>,
    reference_sequence: &[u8],
    reference_atlas: &[u8],
) -> Result<Vec<OfficialCardStream>, String> {
    if source.len() != 11308 || hash(source) != SOURCE_SHA256 {
        return Err("MCRD1OF 원본 identity 불일치".into());
    }
    // 126·127의 뜻은 공식전의 빈 픽셀에서 추정하지 않고, 같은 디스크의
    // 동일 문구와 동일 셀 배치가 있는 PCOMI 원본 두 자산에 결속한다.
    if reference_sequence.len() != 6632
        || hash(reference_sequence)
            != "256869d567c017a2c8d54cc615ce64189f7a6ac2f83060c3af6ef9940f4851b9"
        || reference_atlas.len() != 16384
        || hash(reference_atlas)
            != "4f9498956b6fa24091944d2dc8ca46091719a9feb0ce2c273dcc3d687f0b04e1"
        || reference_sequence[0xfbe..0x100c] != source[0x1222..0x1270]
    {
        return Err("카드 종료문 PCOMI 원본 참조 불일치".into());
    }
    let mut used = BTreeSet::new();
    for &cell in glyphs.values() {
        if !(24..128).contains(&cell) || !used.insert(cell) {
            return Err("카드 글리프 셀 범위·중복 오류".into());
        }
    }
    let mut result = Vec::new();
    let mut ranges = Vec::new();
    for (manifest_index, line) in MANIFESTS
        .iter()
        .enumerate()
        .flat_map(|(i, m)| m.lines().skip(1).map(move |line| (i, line)))
    {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 6
            || fields[5]
                != if manifest_index == 2 {
                    "reference-restored-draft"
                } else {
                    "draft"
                }
        {
            return Err("카드 manifest 형식·상태 오류".into());
        }
        let offset = usize::from_str_radix(fields[0], 16).map_err(|e| e.to_string())?;
        let end = usize::from_str_radix(fields[1], 16).map_err(|e| e.to_string())?;
        if offset >= end || ranges.iter().any(|&(a, z)| offset < z && a < end) {
            return Err("카드 stream 중복·범위 오류".into());
        }
        let original = source.get(offset..end).ok_or("카드 stream 범위 이탈")?;
        if hash(original) != fields[2] {
            return Err("카드 stream 원본 SHA-256 불일치".into());
        }
        result.push(OfficialCardStream {
            offset,
            bytes: rewrite(original, fields[3], fields[4], glyphs)
                .map_err(|e| format!("카드 +{offset:04X}: {e}"))?,
        });
        ranges.push((offset, end));
    }
    Ok(result)
}

/// 세 번역 표가 함께 사용하는 고정 셀 문자표를 만든다.
pub fn official_card_glyph_mapping() -> Result<BTreeMap<char, u16>, String> {
    let manifests = MANIFESTS;
    let mut chars = BTreeSet::from([' ']);
    for line in manifests
        .iter()
        .flat_map(|manifest| manifest.lines().skip(1))
    {
        let ko = line.split('\t').nth(4).ok_or("번역 열 누락")?;
        let mut in_tag = false;
        let mut iter = ko.chars();
        while let Some(ch) = iter.next() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                '\\' if !in_tag => {
                    if iter.next() != Some('n') {
                        return Err("미등록 escape".into());
                    }
                }
                _ if !in_tag => {
                    chars.insert(ch);
                }
                _ => (),
            }
        }
    }
    let mapping: BTreeMap<_, _> = chars
        .into_iter()
        .enumerate()
        .map(|(i, ch)| (ch, 24 + i as u16))
        .collect();
    Ok(mapping)
}

#[cfg(test)]
#[path = "official_card_text_tests.rs"]
mod tests;
