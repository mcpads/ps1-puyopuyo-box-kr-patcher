//! BIOS `Krom2RawAdd` B(0x51) 정적 호출 경계.
//!
//! 스텁 존재, direct `jal` 호출, 엔트리 재귀 추적 포함 여부를 서로 분리한다.
//! 마지막 조건까지 통과해도 정상 플레이에서 글리프가 실제 소비됐다는
//! runtime 증거는 아니다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::{normalize_kseg, ExeMap, EXE_MAPS};
use crate::isa::{FlowKind, Instruction, InstructionKind as K, Reg};

use super::{linear, trace};

const CALLSITE_WORDS_BEFORE: usize = 24;
const CALLSITE_WORDS_AFTER: usize = 8;
const CALLSITE_WINDOW_WORDS: usize = CALLSITE_WORDS_BEFORE + 1 + CALLSITE_WORDS_AFTER;
const WRAPPER_ENTRY_SEARCH_WORDS: usize = 64;

/// 고정 지원 원본에서 발견한 B(0x51) 스텁과 direct caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiosFontRouteSpec {
    pub exe: &'static str,
    pub stub: u32,
    pub direct_caller: u32,
    pub wrapper_entry: u32,
    pub wrapper_caller: u32,
}

pub const BOX_BIOS_FONT_ROUTES: [BiosFontRouteSpec; 3] = [
    BiosFontRouteSpec {
        exe: "PCOMI.EXE",
        stub: 0x8002_FA78,
        direct_caller: 0x8001_B804,
        wrapper_entry: 0x8001_B7C4,
        wrapper_caller: 0x8001_BC58,
    },
    BiosFontRouteSpec {
        exe: "PUYO1.EXE",
        stub: 0x8006_BF2C,
        direct_caller: 0x8002_7DD4,
        wrapper_entry: 0x8002_7D94,
        wrapper_caller: 0x8002_824C,
    },
    BiosFontRouteSpec {
        exe: "PUYO2.EXE",
        stub: 0x8006_B0F8,
        direct_caller: 0x8002_62DC,
        wrapper_entry: 0x8002_629C,
        wrapper_caller: 0x8002_6754,
    },
];

/// EXE 하나의 B(0x51) 정적 경로 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiosFontRouteSummary {
    pub exe: &'static str,
    pub stubs: Vec<u32>,
    pub direct_callers: Vec<u32>,
    pub wrapper_entry: Option<u32>,
    pub wrapper_callers: Vec<u32>,
    pub entry_reached_callers: Vec<u32>,
    pub entry_reached_stubs: Vec<u32>,
    pub entry_reached_wrapper_callers: Vec<u32>,
    pub callsite_shape_sha256: Option<[u8; 32]>,
}

/// 두 direct caller 주변 구조 window의 위치별 일치 수.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiosFontCallsiteSimilarity {
    pub left: &'static str,
    pub right: &'static str,
    pub matched_words: usize,
    pub window_words: usize,
}

/// 11개 EXE의 B(0x51) 정적 분석 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiosFontAnalysis {
    pub routes: Vec<BiosFontRouteSummary>,
    pub callsite_similarities: Vec<BiosFontCallsiteSimilarity>,
}

/// B(0x51) 정적 분석 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BiosFontError {
    EmptyInput,
    DuplicateExecutable(&'static str),
    TextLengthMismatch {
        exe: &'static str,
        expected: usize,
        actual: usize,
    },
    UnalignedText(&'static str),
    CallsiteWindowOutsideText {
        exe: &'static str,
        caller: u32,
    },
    BoxExecutableCount {
        expected: usize,
        actual: usize,
    },
    MissingBoxExecutable(&'static str),
    UnexpectedRoute {
        exe: &'static str,
        stubs: Vec<u32>,
        callers: Vec<u32>,
    },
    EntryTraceMismatch {
        exe: &'static str,
        expected_caller: u32,
        expected_stub: u32,
    },
    WrapperMismatch {
        exe: &'static str,
        expected_entry: u32,
        actual_entry: Option<u32>,
        expected_caller: u32,
        actual_callers: Vec<u32>,
    },
    WrapperEntryTraceMismatch {
        exe: &'static str,
        expected_caller: u32,
    },
}

impl fmt::Display for BiosFontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "BIOS font 분석할 EXE가 없음"),
            Self::DuplicateExecutable(exe) => write!(f, "BIOS font 분석 EXE 중복: {exe}"),
            Self::TextLengthMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe} text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText(exe) => write!(f, "{exe} text 길이가 4바이트 정렬이 아님"),
            Self::CallsiteWindowOutsideText { exe, caller } => write!(
                f,
                "{exe} B(0x51) caller window가 text 범위 밖임: 0x{caller:08X}"
            ),
            Self::BoxExecutableCount { expected, actual } => write!(
                f,
                "BOX BIOS font 분석 EXE 분모 불일치: expected={expected}, actual={actual}"
            ),
            Self::MissingBoxExecutable(exe) => write!(f, "BOX BIOS font 분석 입력 누락: {exe}"),
            Self::UnexpectedRoute {
                exe,
                stubs,
                callers,
            } => write!(
                f,
                "{exe} B(0x51) 정적 경로 불일치: stubs={stubs:08X?}, callers={callers:08X?}"
            ),
            Self::EntryTraceMismatch {
                exe,
                expected_caller,
                expected_stub,
            } => write!(
                f,
                "{exe} B(0x51) caller/stub가 엔트리 추적 하한에 함께 없음: \
                 caller=0x{expected_caller:08X}, stub=0x{expected_stub:08X}"
            ),
            Self::WrapperMismatch {
                exe,
                expected_entry,
                actual_entry,
                expected_caller,
                actual_callers,
            } => write!(
                f,
                "{exe} B(0x51) wrapper 경로 불일치: expected entry=0x{expected_entry:08X}, \
                 actual entry={actual_entry:08X?}, expected caller=0x{expected_caller:08X}, \
                 actual callers={actual_callers:08X?}"
            ),
            Self::WrapperEntryTraceMismatch {
                exe,
                expected_caller,
            } => write!(
                f,
                "{exe} B(0x51) wrapper caller가 엔트리 추적 하한에 없음: \
                 caller=0x{expected_caller:08X}"
            ),
        }
    }
}

impl Error for BiosFontError {}

#[derive(Debug)]
struct RouteWork {
    summary: BiosFontRouteSummary,
    callsite_shape: Option<Vec<u32>>,
}

/// 입력 EXE에서 B(0x51) 스텁과 direct caller를 전수 탐색한다.
pub fn audit_bios_font_routes<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
) -> Result<BiosFontAnalysis, BiosFontError> {
    let mut seen = BTreeSet::new();
    let mut work = Vec::new();

    for (map, text) in executables {
        if !seen.insert(map.name) {
            return Err(BiosFontError::DuplicateExecutable(map.name));
        }
        if text.len() != map.text_len as usize {
            return Err(BiosFontError::TextLengthMismatch {
                exe: map.name,
                expected: map.text_len as usize,
                actual: text.len(),
            });
        }
        let instructions =
            linear::sweep(map, text).ok_or(BiosFontError::UnalignedText(map.name))?;
        let stubs = find_krom2rawadd_stubs(&instructions);
        let direct_callers = find_direct_callers(&instructions, &stubs);
        let wrapper_entry = match direct_callers.as_slice() {
            [caller] => find_enclosing_function_entry(&instructions, *caller),
            _ => None,
        };
        let wrapper_callers = wrapper_entry
            .map(|entry| find_direct_callers(&instructions, &[entry]))
            .unwrap_or_default();
        let entry_trace = trace(map, text, &[]);
        let entry_reached_callers = direct_callers
            .iter()
            .copied()
            .filter(|caller| entry_trace.instructions.contains_key(caller))
            .collect();
        let entry_reached_stubs = stubs
            .iter()
            .copied()
            .filter(|stub| entry_trace.instructions.contains_key(stub))
            .collect();
        let entry_reached_wrapper_callers = wrapper_callers
            .iter()
            .copied()
            .filter(|caller| entry_trace.instructions.contains_key(caller))
            .collect();

        let callsite_shape = if let [caller] = direct_callers.as_slice() {
            Some(callsite_shape(map, text, *caller)?)
        } else {
            None
        };
        let callsite_shape_sha256 = callsite_shape.as_ref().map(|shape| {
            let mut digest = Sha256::new();
            for word in shape {
                digest.update(word.to_le_bytes());
            }
            digest.finalize().into()
        });
        work.push(RouteWork {
            summary: BiosFontRouteSummary {
                exe: map.name,
                stubs,
                direct_callers,
                wrapper_entry,
                wrapper_callers,
                entry_reached_callers,
                entry_reached_stubs,
                entry_reached_wrapper_callers,
                callsite_shape_sha256,
            },
            callsite_shape,
        });
    }
    if work.is_empty() {
        return Err(BiosFontError::EmptyInput);
    }

    let routed: Vec<_> = work
        .iter()
        .filter_map(|route| {
            route
                .callsite_shape
                .as_ref()
                .map(|shape| (route.summary.exe, shape))
        })
        .collect();
    let mut callsite_similarities = Vec::new();
    for left_index in 0..routed.len() {
        for right_index in left_index + 1..routed.len() {
            let (left, left_shape) = routed[left_index];
            let (right, right_shape) = routed[right_index];
            let matched_words = left_shape
                .iter()
                .zip(right_shape)
                .filter(|(left_word, right_word)| left_word == right_word)
                .count();
            callsite_similarities.push(BiosFontCallsiteSimilarity {
                left,
                right,
                matched_words,
                window_words: CALLSITE_WINDOW_WORDS,
            });
        }
    }

    Ok(BiosFontAnalysis {
        routes: work.into_iter().map(|route| route.summary).collect(),
        callsite_similarities,
    })
}

/// 지원 원본의 세 route와 나머지 8개 무스텁 경계를 검증한다.
pub fn verify_box_bios_font_routes<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
) -> Result<BiosFontAnalysis, BiosFontError> {
    let analysis = audit_bios_font_routes(executables)?;
    if analysis.routes.len() != EXE_MAPS.len() {
        return Err(BiosFontError::BoxExecutableCount {
            expected: EXE_MAPS.len(),
            actual: analysis.routes.len(),
        });
    }

    let specs: BTreeMap<_, _> = BOX_BIOS_FONT_ROUTES
        .iter()
        .map(|spec| (spec.exe, *spec))
        .collect();
    for map in EXE_MAPS {
        let route = analysis
            .routes
            .iter()
            .find(|route| route.exe == map.name)
            .ok_or(BiosFontError::MissingBoxExecutable(map.name))?;
        if let Some(spec) = specs.get(map.name) {
            if route.stubs != [spec.stub] || route.direct_callers != [spec.direct_caller] {
                return Err(BiosFontError::UnexpectedRoute {
                    exe: map.name,
                    stubs: route.stubs.clone(),
                    callers: route.direct_callers.clone(),
                });
            }
            if route.entry_reached_callers != [spec.direct_caller]
                || route.entry_reached_stubs != [spec.stub]
            {
                return Err(BiosFontError::EntryTraceMismatch {
                    exe: map.name,
                    expected_caller: spec.direct_caller,
                    expected_stub: spec.stub,
                });
            }
            if route.wrapper_entry != Some(spec.wrapper_entry)
                || route.wrapper_callers != [spec.wrapper_caller]
            {
                return Err(BiosFontError::WrapperMismatch {
                    exe: map.name,
                    expected_entry: spec.wrapper_entry,
                    actual_entry: route.wrapper_entry,
                    expected_caller: spec.wrapper_caller,
                    actual_callers: route.wrapper_callers.clone(),
                });
            }
            if route.entry_reached_wrapper_callers != [spec.wrapper_caller] {
                return Err(BiosFontError::WrapperEntryTraceMismatch {
                    exe: map.name,
                    expected_caller: spec.wrapper_caller,
                });
            }
        } else if !route.stubs.is_empty() || !route.direct_callers.is_empty() {
            return Err(BiosFontError::UnexpectedRoute {
                exe: map.name,
                stubs: route.stubs.clone(),
                callers: route.direct_callers.clone(),
            });
        }
    }
    Ok(analysis)
}

/// route와 callsite 구조 비교를 한 long-form TSV로 만든다.
pub fn render_bios_font_analysis_tsv(analysis: &BiosFontAnalysis) -> String {
    let mut out = String::from(
        "kind\tleft_exe\tright_exe\tstub\tdirect_caller\twrapper_entry\twrapper_caller\
         \tentry_reached_caller\tentry_reached_stub\tentry_reached_wrapper_caller\
         \tmatched_words\twindow_words\tcallsite_shape_sha256\n",
    );
    for route in &analysis.routes {
        let stub = single_hex(&route.stubs);
        let caller = single_hex(&route.direct_callers);
        let wrapper_entry = route
            .wrapper_entry
            .map(|address| format!("0x{address:08X}"))
            .unwrap_or_default();
        let wrapper_caller = single_hex(&route.wrapper_callers);
        let reached_caller = single_hex(&route.entry_reached_callers);
        let reached_stub = single_hex(&route.entry_reached_stubs);
        let reached_wrapper_caller = single_hex(&route.entry_reached_wrapper_callers);
        let digest = route
            .callsite_shape_sha256
            .as_ref()
            .map(hex_digest)
            .unwrap_or_default();
        writeln!(
            out,
            "route\t{}\t\t{stub}\t{caller}\t{wrapper_entry}\t{wrapper_caller}\
             \t{reached_caller}\t{reached_stub}\t{reached_wrapper_caller}\t\t\t{digest}",
            route.exe
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    for similarity in &analysis.callsite_similarities {
        writeln!(
            out,
            "similarity\t{}\t{}\t\t\t\t\t\t\t\t{}\t{}\t",
            similarity.left, similarity.right, similarity.matched_words, similarity.window_words,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    out
}

fn find_krom2rawadd_stubs(instructions: &[Instruction]) -> Vec<u32> {
    instructions
        .windows(3)
        .filter_map(|window| {
            let [load_bios_b, jump_bios_b, function_number] = window else {
                unreachable!("windows(3)은 항상 세 원소다");
            };
            let contiguous = jump_bios_b.addr == load_bios_b.addr + 4
                && function_number.addr == load_bios_b.addr + 8;
            let shape = matches!(
                load_bios_b.kind,
                K::Addiu {
                    rt: Reg::T2,
                    rs: Reg::ZERO,
                    imm: 0x00B0,
                }
            ) && matches!(jump_bios_b.kind, K::Jr { rs: Reg::T2 })
                && matches!(
                    function_number.kind,
                    K::Addiu {
                        rt: Reg::T1,
                        rs: Reg::ZERO,
                        imm: 0x0051,
                    }
                );
            (contiguous && shape).then_some(load_bios_b.addr)
        })
        .collect()
}

fn find_direct_callers(instructions: &[Instruction], stubs: &[u32]) -> Vec<u32> {
    let stubs: BTreeSet<_> = stubs.iter().copied().collect();
    instructions
        .iter()
        .filter_map(|instruction| match instruction.flow_kind() {
            FlowKind::Call { target } if stubs.contains(&normalize_kseg(target)) => {
                Some(instruction.addr)
            }
            _ => None,
        })
        .collect()
}

fn find_enclosing_function_entry(instructions: &[Instruction], caller: u32) -> Option<u32> {
    let caller_index = instructions
        .binary_search_by_key(&caller, |instruction| instruction.addr)
        .ok()?;
    instructions[..=caller_index]
        .iter()
        .rev()
        .take(WRAPPER_ENTRY_SEARCH_WORDS)
        .find_map(|instruction| {
            matches!(
                instruction.kind,
                K::Addiu {
                    rt: Reg::SP,
                    rs: Reg::SP,
                    imm
                } if imm < 0
            )
            .then_some(instruction.addr)
        })
}

fn callsite_shape(map: &ExeMap, text: &[u8], caller: u32) -> Result<Vec<u32>, BiosFontError> {
    let caller_offset =
        map.text_offset(caller)
            .ok_or(BiosFontError::CallsiteWindowOutsideText {
                exe: map.name,
                caller,
            })?;
    let start = caller_offset.checked_sub(CALLSITE_WORDS_BEFORE * 4).ok_or(
        BiosFontError::CallsiteWindowOutsideText {
            exe: map.name,
            caller,
        },
    )?;
    let end = caller_offset
        .checked_add((CALLSITE_WORDS_AFTER + 1) * 4)
        .ok_or(BiosFontError::CallsiteWindowOutsideText {
            exe: map.name,
            caller,
        })?;
    let bytes = text
        .get(start..end)
        .ok_or(BiosFontError::CallsiteWindowOutsideText {
            exe: map.name,
            caller,
        })?;
    Ok(bytes
        .chunks_exact(4)
        .map(|word| {
            normalize_instruction_shape(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        })
        .collect())
}

/// 레지스터 배치는 보존하고 즉시값·직접 target만 제거한 국소 구조 word.
fn normalize_instruction_shape(word: u32) -> u32 {
    match word >> 26 {
        0 => word,
        2 | 3 => word & 0xFC00_0000,
        _ => word & 0xFFFF_0000,
    }
}

fn single_hex(addresses: &[u32]) -> String {
    match addresses {
        [] => String::new(),
        [address] => format!("0x{address:08X}"),
        _ => addresses
            .iter()
            .map(|address| format!("0x{address:08X}"))
            .collect::<Vec<_>>()
            .join(","),
    }
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "bios_font_tests.rs"]
mod bios_font_tests;
