//! CNX v2 LZ77 압축·해제.
//!
//! BOX 원본의 CNX 스트림은 16바이트 헤더와 2비트 opcode 네 개를 담은
//! control byte로 구성된다. 원본 압축기는 명령이 2 KiB 경계를 넘지 않도록
//! opcode 0과 `0xCD` 패딩을 넣는다. 새 압축 결과도 같은 경계를 지킨다.
//!
//! 이 모듈의 자기 라운드트립은 새 스트림의 형식 유효성만 증명한다. 원본
//! 바이트 재현과 실제 게임 소비 호환성은 별도 게이트다.

use std::error::Error;
use std::fmt;

const MAGIC: &[u8; 3] = b"CNX";
const VERSION: u8 = 0x02;
const HEADER_LEN: usize = 16;
const FORMAT_BYTE: u8 = 0x10;
const STREAM_PAGE_LEN: usize = 0x800;
const WINDOW_LEN: usize = 0x800;
const WINDOW_MASK: usize = WINDOW_LEN - 1;
const MAX_MATCH_LEN: usize = 35;
const MIN_MATCH_LEN: usize = 4;
const HASH_BITS: usize = 14;
const HASH_LEN: usize = 1 << HASH_BITS;
const MAX_CHAIN_LEN: usize = 128;

/// CNX v2 헤더.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CnxHeader {
    /// 콘텐츠 서브타입. BOX 원본에서는 `bin`, `bi_`, `bit`, `cel`이다.
    pub subtype: [u8; 3],
    /// BOX 원본에서 전부 `0x10`인 형식 바이트.
    pub format_byte: u8,
    /// 16바이트 헤더를 제외한 압축 스트림 크기.
    pub compressed_size: u32,
    /// 해제 결과 크기.
    pub decompressed_size: u32,
}

/// CNX 경계 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CnxError {
    TooShort {
        actual: usize,
        needed: usize,
    },
    InvalidMagic,
    UnsupportedVersion(u8),
    UnsupportedFormatByte(u8),
    LengthOverflow {
        field: &'static str,
        value: usize,
    },
    EncoderBoundExceeded {
        bound: usize,
        actual: usize,
    },
    BodyLengthMismatch {
        declared: usize,
        actual: usize,
    },
    UnexpectedEnd {
        offset: usize,
        context: &'static str,
    },
    MissingTerminator,
    TrailingBodyData {
        terminator_end: usize,
        body_end: usize,
    },
    OutputTooLarge {
        declared: usize,
        attempted: usize,
    },
    OutputSizeMismatch {
        declared: usize,
        actual: usize,
    },
}

impl fmt::Display for CnxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { actual, needed } => {
                write!(f, "CNX 데이터 부족: {actual}B, 최소 {needed}B 필요")
            }
            Self::InvalidMagic => write!(f, "CNX 매직 불일치"),
            Self::UnsupportedVersion(version) => {
                write!(f, "지원하지 않는 CNX 버전: 0x{version:02X}")
            }
            Self::UnsupportedFormatByte(value) => {
                write!(f, "지원하지 않는 CNX 형식 바이트: 0x{value:02X}")
            }
            Self::LengthOverflow { field, value } => {
                write!(f, "CNX {field}가 u32 범위를 초과: {value}")
            }
            Self::EncoderBoundExceeded { bound, actual } => write!(
                f,
                "CNX encoder 출력이 보수 상한을 넘음: bound={bound}, actual={actual}"
            ),
            Self::BodyLengthMismatch { declared, actual } => write!(
                f,
                "CNX 압축 본문 크기 불일치: 선언 {declared}B, 실제 {actual}B"
            ),
            Self::UnexpectedEnd { offset, context } => {
                write!(f, "CNX 오프셋 0x{offset:X}에서 {context} 데이터가 잘림")
            }
            Self::MissingTerminator => write!(f, "CNX 종결 control byte가 없음"),
            Self::TrailingBodyData {
                terminator_end,
                body_end,
            } => write!(
                f,
                "CNX 종결 뒤 미등록 본문 데이터: 0x{terminator_end:X}..0x{body_end:X}"
            ),
            Self::OutputTooLarge {
                declared,
                attempted,
            } => write!(
                f,
                "CNX 해제 결과가 선언 크기를 초과: 선언 {declared}B, 시도 {attempted}B"
            ),
            Self::OutputSizeMismatch { declared, actual } => {
                write!(f, "CNX 해제 크기 불일치: 선언 {declared}B, 실제 {actual}B")
            }
        }
    }
}

impl Error for CnxError {}

/// 현재 encoder가 `input_len` 바이트를 만들 때의 header 포함 보수 크기 상한.
///
/// opcode payload와 control byte는 입력 바이트당 2바이트보다 작다. 2 KiB page
/// padding은 앞에서 소비한 page payload보다 작으므로 이를 다시 두 배로 잡고,
/// 빈 입력·첫 page·종결 여유 2 KiB를 더한다. 실제 압축률 예측값이 아니라
/// relocation reservation을 입력 내용과 무관하게 검산하기 위한 상한이다.
pub const fn cnx_output_size_upper_bound(input_len: usize) -> Option<usize> {
    match input_len.checked_mul(4) {
        Some(payload) => payload.checked_add(STREAM_PAGE_LEN),
        None => None,
    }
}

/// CNX v2 시그니처인가.
pub fn is_cnx(data: &[u8]) -> bool {
    data.len() >= 4 && &data[..3] == MAGIC && data[3] == VERSION
}

/// 16바이트 CNX v2 헤더를 파싱한다.
pub fn parse_header(data: &[u8]) -> Result<CnxHeader, CnxError> {
    if data.len() < HEADER_LEN {
        return Err(CnxError::TooShort {
            actual: data.len(),
            needed: HEADER_LEN,
        });
    }
    if &data[..3] != MAGIC {
        return Err(CnxError::InvalidMagic);
    }
    if data[3] != VERSION {
        return Err(CnxError::UnsupportedVersion(data[3]));
    }
    if data[7] != FORMAT_BYTE {
        return Err(CnxError::UnsupportedFormatByte(data[7]));
    }
    Ok(CnxHeader {
        subtype: [data[4], data[5], data[6]],
        format_byte: data[7],
        compressed_size: u32::from_be_bytes([data[8], data[9], data[10], data[11]]),
        decompressed_size: u32::from_be_bytes([data[12], data[13], data[14], data[15]]),
    })
}

fn require(data: &[u8], offset: usize, len: usize, context: &'static str) -> Result<(), CnxError> {
    if offset.checked_add(len).is_none_or(|end| end > data.len()) {
        return Err(CnxError::UnexpectedEnd { offset, context });
    }
    Ok(())
}

fn push_checked(output: &mut Vec<u8>, byte: u8, declared: usize) -> Result<(), CnxError> {
    let attempted = output.len() + 1;
    if attempted > declared {
        return Err(CnxError::OutputTooLarge {
            declared,
            attempted,
        });
    }
    output.push(byte);
    Ok(())
}

/// CNX v2 스트림을 해제한다.
///
/// 선언 본문보다 짧거나 긴 입력, 잘린 명령, 종결 누락, 출력 크기 불일치는
/// 부분 결과 대신 오류를 반환한다.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, CnxError> {
    let header = parse_header(data)?;
    let body_len = header.compressed_size as usize;
    let body_end = HEADER_LEN
        .checked_add(body_len)
        .ok_or(CnxError::LengthOverflow {
            field: "압축 크기",
            value: body_len,
        })?;
    if data.len() != body_end {
        return Err(CnxError::BodyLengthMismatch {
            declared: body_len,
            actual: data.len().saturating_sub(HEADER_LEN),
        });
    }

    let declared = header.decompressed_size as usize;
    let mut output = Vec::with_capacity(declared);
    let mut ring = [0u8; WINDOW_LEN];
    let mut ring_pos = 0usize;
    let mut source = HEADER_LEN;
    let mut terminated = false;

    while source < body_end {
        let control = data[source];
        source += 1;
        if control == 0 {
            terminated = true;
            break;
        }

        let mut fields = control;
        for _ in 0..4 {
            match fields & 0x03 {
                0 => {
                    require(data, source, 1, "패딩 길이")?;
                    let skip = data[source] as usize;
                    source += 1;
                    require(data, source, skip, "패딩")?;
                    source += skip;
                    break;
                }
                1 => {
                    require(data, source, 1, "리터럴")?;
                    let byte = data[source];
                    source += 1;
                    push_checked(&mut output, byte, declared)?;
                    ring[ring_pos] = byte;
                    ring_pos = (ring_pos + 1) & WINDOW_MASK;
                }
                2 => {
                    require(data, source, 2, "윈도 참조")?;
                    let pair = u16::from_be_bytes([data[source], data[source + 1]]);
                    source += 2;
                    let distance = usize::from(pair >> 5) + 1;
                    let len = usize::from(pair & 0x1F) + MIN_MATCH_LEN;
                    for _ in 0..len {
                        let byte = ring[ring_pos.wrapping_sub(distance) & WINDOW_MASK];
                        push_checked(&mut output, byte, declared)?;
                        ring[ring_pos] = byte;
                        ring_pos = (ring_pos + 1) & WINDOW_MASK;
                    }
                }
                3 => {
                    require(data, source, 1, "리터럴 런 길이")?;
                    let len = data[source] as usize;
                    source += 1;
                    require(data, source, len, "리터럴 런")?;
                    for &byte in &data[source..source + len] {
                        push_checked(&mut output, byte, declared)?;
                        ring[ring_pos] = byte;
                        ring_pos = (ring_pos + 1) & WINDOW_MASK;
                    }
                    source += len;
                }
                _ => unreachable!(),
            }
            fields >>= 2;
        }
    }

    if !terminated {
        return Err(CnxError::MissingTerminator);
    }
    if source != body_end {
        return Err(CnxError::TrailingBodyData {
            terminator_end: source,
            body_end,
        });
    }
    if output.len() != declared {
        return Err(CnxError::OutputSizeMismatch {
            declared,
            actual: output.len(),
        });
    }
    Ok(output)
}

#[derive(Debug)]
enum CompOp {
    Single(u8),
    Literal(Vec<u8>),
    Match { distance: u16, len: u16 },
}

impl CompOp {
    fn opcode(&self) -> u8 {
        match self {
            Self::Single(_) => 1,
            Self::Match { .. } => 2,
            Self::Literal(_) => 3,
        }
    }

    fn encoded(&self) -> Vec<u8> {
        match self {
            Self::Single(byte) => vec![*byte],
            Self::Literal(bytes) => {
                let mut encoded = Vec::with_capacity(bytes.len() + 1);
                encoded.push(bytes.len() as u8);
                encoded.extend_from_slice(bytes);
                encoded
            }
            Self::Match { distance, len } => {
                let pair = ((*distance - 1) << 5) | (*len - MIN_MATCH_LEN as u16);
                pair.to_be_bytes().to_vec()
            }
        }
    }
}

struct MatchFinder {
    heads: Vec<u32>,
    chains: Vec<u32>,
}

impl MatchFinder {
    fn new(data_len: usize) -> Self {
        Self {
            heads: vec![u32::MAX; HASH_LEN],
            chains: vec![u32::MAX; data_len],
        }
    }

    fn hash3(data: &[u8], pos: usize) -> usize {
        let a = usize::from(data[pos]);
        let b = usize::from(data[pos + 1]);
        let c = usize::from(data[pos + 2]);
        ((a << 10) ^ (b << 5) ^ c) & (HASH_LEN - 1)
    }

    fn insert(&mut self, data: &[u8], pos: usize) {
        if pos + 2 >= data.len() {
            return;
        }
        let hash = Self::hash3(data, pos);
        self.chains[pos] = self.heads[hash];
        self.heads[hash] = pos as u32;
    }

    fn find_best(&self, data: &[u8], pos: usize) -> Option<(u16, u16)> {
        let remaining = data.len() - pos;
        if remaining < MIN_MATCH_LEN || pos == 0 || pos + 2 >= data.len() {
            return None;
        }

        let hash = Self::hash3(data, pos);
        let max_len = remaining.min(MAX_MATCH_LEN);
        let min_pos = pos.saturating_sub(WINDOW_LEN);
        let mut candidate = self.heads[hash];
        let mut steps = 0usize;
        let mut best = None;
        let mut best_len = 0usize;

        while candidate != u32::MAX {
            let candidate_pos = candidate as usize;
            if candidate_pos < min_pos {
                break;
            }
            if candidate_pos >= pos {
                candidate = self.chains[candidate_pos];
                continue;
            }

            let distance = pos - candidate_pos;
            let mut len = 0usize;
            for i in 0..max_len {
                let source_pos = candidate_pos + (i % distance);
                if data[source_pos] != data[pos + i] {
                    break;
                }
                len += 1;
            }
            if len >= MIN_MATCH_LEN && len > best_len {
                best = Some((distance as u16, len as u16));
                best_len = len;
                if len == max_len {
                    break;
                }
            }

            candidate = self.chains[candidate_pos];
            steps += 1;
            if steps >= MAX_CHAIN_LEN {
                break;
            }
        }
        best
    }
}

fn generate_ops(data: &[u8]) -> Vec<CompOp> {
    let mut finder = MatchFinder::new(data.len());
    let mut ops = Vec::new();
    let mut pos = 0usize;

    while pos < data.len() {
        let best = finder.find_best(data, pos);
        if let Some((distance, len)) = best {
            let mut use_literal = false;
            let lazy_possible = usize::from(len) < MAX_MATCH_LEN && pos + 1 < data.len();
            if lazy_possible {
                finder.insert(data, pos);
                if let Some((_, next_len)) = finder.find_best(data, pos + 1) {
                    use_literal = next_len > len + 1;
                }
            }

            if use_literal {
                ops.push(CompOp::Single(data[pos]));
                pos += 1;
                continue;
            }

            let insert_start = if lazy_possible { pos + 1 } else { pos };
            for insert_pos in insert_start..pos + usize::from(len) {
                finder.insert(data, insert_pos);
            }
            ops.push(CompOp::Match { distance, len });
            pos += usize::from(len);
        } else {
            finder.insert(data, pos);
            ops.push(CompOp::Single(data[pos]));
            pos += 1;
        }
    }
    ops
}

fn merge_literals(ops: Vec<CompOp>) -> Vec<CompOp> {
    let mut merged = Vec::with_capacity(ops.len());
    let mut singles = Vec::new();

    let flush = |singles: &mut Vec<u8>, merged: &mut Vec<CompOp>| {
        if singles.len() >= 5 {
            for chunk in singles.chunks(255) {
                merged.push(CompOp::Literal(chunk.to_vec()));
            }
        } else {
            merged.extend(singles.iter().copied().map(CompOp::Single));
        }
        singles.clear();
    };

    for op in ops {
        match op {
            CompOp::Single(byte) => singles.push(byte),
            other => {
                flush(&mut singles, &mut merged);
                merged.push(other);
            }
        }
    }
    flush(&mut singles, &mut merged);
    merged
}

fn pack_ops(ops: &[CompOp]) -> Vec<u8> {
    let mut body = Vec::new();
    let mut op_index = 0usize;

    while op_index < ops.len() {
        let control_offset = body.len();
        body.push(0);
        let mut slot = 0usize;

        while slot < 4 && op_index < ops.len() {
            let encoded = ops[op_index].encoded();
            let absolute = HEADER_LEN + body.len();
            let remaining = STREAM_PAGE_LEN - (absolute % STREAM_PAGE_LEN);
            let reserve_padding_command = usize::from(slot < 3);
            if encoded.len() + reserve_padding_command > remaining {
                for unused_slot in slot + 1..4 {
                    body[control_offset] |= 3 << (unused_slot * 2);
                }
                body.push((remaining - 1) as u8);
                body.extend(std::iter::repeat_n(0xCD, remaining - 1));
                break;
            }

            body[control_offset] |= ops[op_index].opcode() << (slot * 2);
            body.extend_from_slice(&encoded);
            op_index += 1;
            slot += 1;
        }

        if op_index == ops.len() && slot < 4 {
            // 남은 첫 opcode 0이 현재 control group을 끝낸다.
            for unused_slot in slot + 1..4 {
                body[control_offset] |= 3 << (unused_slot * 2);
            }
            body.push(0);
        }
    }

    // control byte 0은 전체 스트림 종결이다.
    body.push(0);
    body
}

/// 입력을 CNX v2로 압축한다.
///
/// 새 스트림은 BOX 원본과 같은 `0x10` 형식 바이트와 2 KiB 경계 패딩을
/// 사용한다. 반환값을 실제 빌드에 쓰기 전에는 대상 소비 호환성 게이트를
/// 별도로 통과해야 한다.
pub fn compress(data: &[u8], subtype: [u8; 3]) -> Result<Vec<u8>, CnxError> {
    let decompressed_size = u32::try_from(data.len()).map_err(|_| CnxError::LengthOverflow {
        field: "해제 크기",
        value: data.len(),
    })?;
    let body = pack_ops(&merge_literals(generate_ops(data)));
    let compressed_size = u32::try_from(body.len()).map_err(|_| CnxError::LengthOverflow {
        field: "압축 크기",
        value: body.len(),
    })?;

    let mut result = Vec::with_capacity(HEADER_LEN + body.len());
    result.extend_from_slice(MAGIC);
    result.push(VERSION);
    result.extend_from_slice(&subtype);
    result.push(FORMAT_BYTE);
    result.extend_from_slice(&compressed_size.to_be_bytes());
    result.extend_from_slice(&decompressed_size.to_be_bytes());
    result.extend_from_slice(&body);
    let bound = cnx_output_size_upper_bound(data.len()).ok_or(CnxError::LengthOverflow {
        field: "encoder 보수 상한",
        value: data.len(),
    })?;
    if result.len() > bound {
        return Err(CnxError::EncoderBoundExceeded {
            bound,
            actual: result.len(),
        });
    }
    Ok(result)
}

#[cfg(test)]
#[path = "cnx_tests.rs"]
mod tests;
