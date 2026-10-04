//! 고정 BTX atlas의 resource·cell lookup 정적 경계.
//!
//! object의 인접한 두 u16 field를 읽어 `resource × 36`으로 descriptor를
//! 고르고, descriptor `+0x1C`의 table에서 `cell × 12` record를 고르는
//! typed R3000A dataflow를 찾는다. 이 core 뒤의 GPU primitive 생성은
//! renderer 후보를 강하게 뒷받침하지만, 정상 플레이의 실제 draw hit는
//! runtime 증거로 따로 판정한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::{ExeMap, EXE_MAPS};
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{linear, trace};

const SEARCH_WINDOW_WORDS: usize = 32;
const PAIR_WINDOW_WORDS: usize = 12;
const FUNCTION_ENTRY_SEARCH_WORDS: usize = 128;

/// 공통 lookup 뒤 primitive 생성 경로의 두 정적 계열.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BtxRendererFamily {
    /// 짧은 renderer 경로.
    ShortRenderer,
    /// 더 큰 stack frame과 분기 구조를 가진 확장 renderer 경로.
    ExtendedRenderer,
}

impl BtxRendererFamily {
    pub const fn name(self) -> &'static str {
        match self {
            Self::ShortRenderer => "short-renderer",
            Self::ExtendedRenderer => "extended-renderer",
        }
    }

    const fn digest_byte(self) -> u8 {
        match self {
            Self::ShortRenderer => 1,
            Self::ExtendedRenderer => 2,
        }
    }
}

/// 지원 원본에서 검토한 EXE별 renderer core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BtxRendererCoreSpec {
    pub exe: &'static str,
    pub family: BtxRendererFamily,
    pub function: u32,
    pub resource_load: u32,
    pub cell_load: u32,
}

/// 두 renderer 계열은 11개 EXE 각각의 별도 함수다.
pub const BOX_BTX_RENDERER_CORES: [BtxRendererCoreSpec; 22] = [
    renderer(
        "SLPS_031.14",
        BtxRendererFamily::ShortRenderer,
        0x8012_90A0,
        0x8012_90D8,
        0x8012_90F0,
    ),
    renderer(
        "SLPS_031.14",
        BtxRendererFamily::ExtendedRenderer,
        0x8012_9388,
        0x8012_93C8,
        0x8012_93E0,
    ),
    renderer(
        "GOCHA1.EXE",
        BtxRendererFamily::ShortRenderer,
        0x8002_B504,
        0x8002_B53C,
        0x8002_B554,
    ),
    renderer(
        "GOCHA1.EXE",
        BtxRendererFamily::ExtendedRenderer,
        0x8002_B7EC,
        0x8002_B82C,
        0x8002_B844,
    ),
    renderer(
        "GOCHA2.EXE",
        BtxRendererFamily::ShortRenderer,
        0x8002_72B0,
        0x8002_72E8,
        0x8002_7300,
    ),
    renderer(
        "GOCHA2.EXE",
        BtxRendererFamily::ExtendedRenderer,
        0x8002_7598,
        0x8002_75D8,
        0x8002_75F0,
    ),
    renderer(
        "HORI2.EXE",
        BtxRendererFamily::ShortRenderer,
        0x8002_72D0,
        0x8002_7308,
        0x8002_7320,
    ),
    renderer(
        "HORI2.EXE",
        BtxRendererFamily::ExtendedRenderer,
        0x8002_75B8,
        0x8002_75F8,
        0x8002_7610,
    ),
    renderer(
        "HORI4.EXE",
        BtxRendererFamily::ShortRenderer,
        0x8002_75A8,
        0x8002_75E0,
        0x8002_75F8,
    ),
    renderer(
        "HORI4.EXE",
        BtxRendererFamily::ExtendedRenderer,
        0x8002_7890,
        0x8002_78D0,
        0x8002_78E8,
    ),
    renderer(
        "KOUSIKI.EXE",
        BtxRendererFamily::ShortRenderer,
        0x8001_FD90,
        0x8001_FDC8,
        0x8001_FDE0,
    ),
    renderer(
        "KOUSIKI.EXE",
        BtxRendererFamily::ExtendedRenderer,
        0x8002_0078,
        0x8002_00B8,
        0x8002_00D0,
    ),
    renderer(
        "MINPUYO.EXE",
        BtxRendererFamily::ShortRenderer,
        0x8002_75F4,
        0x8002_762C,
        0x8002_7644,
    ),
    renderer(
        "MINPUYO.EXE",
        BtxRendererFamily::ExtendedRenderer,
        0x8002_78DC,
        0x8002_791C,
        0x8002_7934,
    ),
    renderer(
        "PCOMI.EXE",
        BtxRendererFamily::ShortRenderer,
        0x8001_8FE4,
        0x8001_9010,
        0x8001_9014,
    ),
    renderer(
        "PCOMI.EXE",
        BtxRendererFamily::ExtendedRenderer,
        0x8001_92F0,
        0x8001_9328,
        0x8001_932C,
    ),
    renderer(
        "PUYO1.EXE",
        BtxRendererFamily::ShortRenderer,
        0x8002_4AB4,
        0x8002_4AE8,
        0x8002_4B00,
    ),
    renderer(
        "PUYO1.EXE",
        BtxRendererFamily::ExtendedRenderer,
        0x8002_4DC0,
        0x8002_4E00,
        0x8002_4E18,
    ),
    renderer(
        "PUYO2.EXE",
        BtxRendererFamily::ShortRenderer,
        0x8002_3AF0,
        0x8002_3B24,
        0x8002_3B3C,
    ),
    renderer(
        "PUYO2.EXE",
        BtxRendererFamily::ExtendedRenderer,
        0x8002_3DFC,
        0x8002_3E3C,
        0x8002_3E54,
    ),
    renderer(
        "QUEST.EXE",
        BtxRendererFamily::ShortRenderer,
        0x8002_384C,
        0x8002_3884,
        0x8002_389C,
    ),
    renderer(
        "QUEST.EXE",
        BtxRendererFamily::ExtendedRenderer,
        0x8002_3B34,
        0x8002_3B74,
        0x8002_3B8C,
    ),
];

const fn renderer(
    exe: &'static str,
    family: BtxRendererFamily,
    function: u32,
    resource_load: u32,
    cell_load: u32,
) -> BtxRendererCoreSpec {
    BtxRendererCoreSpec {
        exe,
        family,
        function,
        resource_load,
        cell_load,
    }
}

/// resource·cell lookup 하나.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BtxCellConsumer {
    pub exe: &'static str,
    pub function_candidate: Option<u32>,
    pub object_register: Reg,
    pub resource_field_offset: i16,
    pub cell_field_offset: i16,
    pub resource_load: u32,
    pub cell_load: u32,
    pub descriptor_table_load: u32,
    pub descriptor_entry: u32,
    pub cell_table_load: u32,
    pub cell_entry: u32,
    pub core_entry_reached: bool,
    pub renderer_family: Option<BtxRendererFamily>,
}

/// EXE별 검출 분모.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxCellConsumerSummary {
    pub exe: &'static str,
    pub consumers: usize,
    pub entry_reached_consumers: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BtxCellExpectation {
    exe: &'static str,
    consumers: usize,
    entry_reached_consumers: usize,
}

const BOX_BTX_CELL_EXPECTATIONS: [BtxCellExpectation; 11] = [
    expectation("SLPS_031.14", 2, 2),
    expectation("GOCHA1.EXE", 3, 2),
    expectation("GOCHA2.EXE", 2, 2),
    expectation("HORI2.EXE", 2, 2),
    expectation("HORI4.EXE", 2, 2),
    expectation("KOUSIKI.EXE", 2, 2),
    expectation("MINPUYO.EXE", 2, 2),
    expectation("PCOMI.EXE", 5, 4),
    expectation("PUYO1.EXE", 6, 4),
    expectation("PUYO2.EXE", 6, 4),
    expectation("QUEST.EXE", 8, 3),
];

const fn expectation(
    exe: &'static str,
    consumers: usize,
    entry_reached_consumers: usize,
) -> BtxCellExpectation {
    BtxCellExpectation {
        exe,
        consumers,
        entry_reached_consumers,
    }
}

/// 11개 EXE 분석 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxCellAnalysis {
    pub summaries: Vec<BtxCellConsumerSummary>,
    pub consumers: Vec<BtxCellConsumer>,
    pub analysis_sha256: [u8; 32],
}

/// 분석 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BtxCellError {
    EmptyInput,
    DuplicateExecutable(&'static str),
    TextLengthMismatch {
        exe: &'static str,
        expected: usize,
        actual: usize,
    },
    UnalignedText(&'static str),
    BoxExecutableCount {
        expected: usize,
        actual: usize,
    },
    MissingBoxExecutable(&'static str),
    AnalysisDigestMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    RendererCoreMismatch {
        exe: &'static str,
        family: BtxRendererFamily,
        function: u32,
    },
    SummaryMismatch {
        exe: &'static str,
        expected_consumers: usize,
        actual_consumers: usize,
        expected_entry_reached: usize,
        actual_entry_reached: usize,
    },
}

impl fmt::Display for BtxCellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "BTX cell consumer 분석할 EXE가 없음"),
            Self::DuplicateExecutable(exe) => write!(f, "BTX cell consumer 분석 EXE 중복: {exe}"),
            Self::TextLengthMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe} text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText(exe) => write!(f, "{exe} text 길이가 4바이트 정렬이 아님"),
            Self::BoxExecutableCount { expected, actual } => write!(
                f,
                "BOX BTX cell consumer EXE 분모 불일치: expected={expected}, actual={actual}"
            ),
            Self::MissingBoxExecutable(exe) => {
                write!(f, "BOX BTX cell consumer 분석 입력 누락: {exe}")
            }
            Self::AnalysisDigestMismatch { expected, actual } => write!(
                f,
                "BOX BTX cell consumer 분석 digest 불일치: expected={}, actual={}",
                hex_digest(expected),
                hex_digest(actual)
            ),
            Self::RendererCoreMismatch {
                exe,
                family,
                function,
            } => write!(
                f,
                "{exe} {} renderer core 불일치: function=0x{function:08X}",
                family.name()
            ),
            Self::SummaryMismatch {
                exe,
                expected_consumers,
                actual_consumers,
                expected_entry_reached,
                actual_entry_reached,
            } => write!(
                f,
                "{exe} BTX cell consumer 분모 불일치: consumers \
                 expected={expected_consumers}, actual={actual_consumers}; entry-reached \
                 expected={expected_entry_reached}, actual={actual_entry_reached}"
            ),
        }
    }
}

impl Error for BtxCellError {}

/// 지원 원본 분석 digest. 최초 전수 검출을 검토한 뒤 고정한다.
pub const BOX_BTX_CELL_ANALYSIS_SHA256: [u8; 32] = [
    0x70, 0x5E, 0x30, 0xBE, 0x66, 0xAD, 0xAC, 0x2A, 0x1A, 0xB0, 0x6A, 0x9A, 0x9E, 0x52, 0x04, 0xE5,
    0x44, 0xB0, 0x7E, 0x5F, 0xE8, 0x2B, 0x5C, 0xAD, 0x77, 0x52, 0xEB, 0x06, 0xFF, 0xBF, 0x81, 0xE3,
];

/// 입력 EXE 각각에서 lookup core를 전수 검출한다.
pub fn audit_btx_cell_consumers<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
) -> Result<BtxCellAnalysis, BtxCellError> {
    let mut seen = BTreeSet::new();
    let mut summaries = Vec::new();
    let mut consumers = Vec::new();

    for (map, text) in executables {
        if !seen.insert(map.name) {
            return Err(BtxCellError::DuplicateExecutable(map.name));
        }
        if text.len() != map.text_len as usize {
            return Err(BtxCellError::TextLengthMismatch {
                exe: map.name,
                expected: map.text_len as usize,
                actual: text.len(),
            });
        }
        let instructions = linear::sweep(map, text).ok_or(BtxCellError::UnalignedText(map.name))?;
        let entry_trace = trace(map, text, &[]);
        let reached = entry_trace.instructions.keys().copied().collect();
        let mut found = find_consumers(map.name, &instructions, &reached);
        found.sort();
        found.dedup();
        summaries.push(BtxCellConsumerSummary {
            exe: map.name,
            consumers: found.len(),
            entry_reached_consumers: found
                .iter()
                .filter(|consumer| consumer.core_entry_reached)
                .count(),
        });
        consumers.extend(found);
    }
    if summaries.is_empty() {
        return Err(BtxCellError::EmptyInput);
    }
    consumers.sort();
    let analysis_sha256 = digest_consumers(&consumers);
    Ok(BtxCellAnalysis {
        summaries,
        consumers,
        analysis_sha256,
    })
}

/// 지원 원본의 11개 EXE와 검토된 exact 결과를 고정한다.
pub fn verify_box_btx_cell_consumers<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
) -> Result<BtxCellAnalysis, BtxCellError> {
    let analysis = audit_btx_cell_consumers(executables)?;
    if analysis.summaries.len() != EXE_MAPS.len() {
        return Err(BtxCellError::BoxExecutableCount {
            expected: EXE_MAPS.len(),
            actual: analysis.summaries.len(),
        });
    }
    for map in EXE_MAPS {
        if !analysis
            .summaries
            .iter()
            .any(|summary| summary.exe == map.name)
        {
            return Err(BtxCellError::MissingBoxExecutable(map.name));
        }
    }
    for expected in BOX_BTX_CELL_EXPECTATIONS {
        let summary = analysis
            .summaries
            .iter()
            .find(|summary| summary.exe == expected.exe)
            .ok_or(BtxCellError::MissingBoxExecutable(expected.exe))?;
        if summary.consumers != expected.consumers
            || summary.entry_reached_consumers != expected.entry_reached_consumers
        {
            return Err(BtxCellError::SummaryMismatch {
                exe: expected.exe,
                expected_consumers: expected.consumers,
                actual_consumers: summary.consumers,
                expected_entry_reached: expected.entry_reached_consumers,
                actual_entry_reached: summary.entry_reached_consumers,
            });
        }
    }
    if analysis.analysis_sha256 != BOX_BTX_CELL_ANALYSIS_SHA256 {
        return Err(BtxCellError::AnalysisDigestMismatch {
            expected: BOX_BTX_CELL_ANALYSIS_SHA256,
            actual: analysis.analysis_sha256,
        });
    }
    for spec in BOX_BTX_RENDERER_CORES {
        let matched = analysis.consumers.iter().any(|consumer| {
            consumer.exe == spec.exe
                && consumer.function_candidate == Some(spec.function)
                && consumer.resource_load == spec.resource_load
                && consumer.cell_load == spec.cell_load
                && consumer.core_entry_reached
                && consumer.renderer_family == Some(spec.family)
        });
        if !matched {
            return Err(BtxCellError::RendererCoreMismatch {
                exe: spec.exe,
                family: spec.family,
                function: spec.function,
            });
        }
    }
    Ok(analysis)
}

/// 검토 가능한 결정적 TSV.
pub fn render_btx_cell_consumers_tsv(analysis: &BtxCellAnalysis) -> String {
    let mut out = String::from(
        "exe\trenderer_family\tfunction_candidate\tobject_register\tresource_field_offset\
         \tcell_field_offset\tresource_load\tcell_load\tdescriptor_table_load\
         \tdescriptor_entry\tcell_table_load\tcell_entry\tcore_entry_reached\n",
    );
    for consumer in &analysis.consumers {
        let function = consumer
            .function_candidate
            .map(|address| format!("0x{address:08X}"))
            .unwrap_or_default();
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t0x{:04X}\t0x{:04X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\
             \t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}",
            consumer.exe,
            consumer
                .renderer_family
                .map(BtxRendererFamily::name)
                .unwrap_or_default(),
            function,
            consumer.object_register,
            consumer.resource_field_offset as u16,
            consumer.cell_field_offset as u16,
            consumer.resource_load,
            consumer.cell_load,
            consumer.descriptor_table_load,
            consumer.descriptor_entry,
            consumer.cell_table_load,
            consumer.cell_entry,
            consumer.core_entry_reached,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    out
}

fn find_consumers(
    exe: &'static str,
    instructions: &[Instruction],
    reached: &BTreeSet<u32>,
) -> Vec<BtxCellConsumer> {
    let mut out = Vec::new();
    for (resource_index, resource_instruction) in instructions.iter().enumerate() {
        let K::Lhu {
            rt: resource_id,
            base: object,
            offset: resource_offset,
        } = resource_instruction.kind
        else {
            continue;
        };
        let Some(cell_offset) = resource_offset.checked_sub(2) else {
            continue;
        };
        let pair_end = (resource_index + PAIR_WINDOW_WORDS + 1).min(instructions.len());
        let pair_start = resource_index.saturating_sub(PAIR_WINDOW_WORDS);
        for cell_index in pair_start..pair_end {
            let K::Lhu {
                rt: cell_id,
                base: cell_object,
                offset: actual_cell_offset,
            } = instructions[cell_index].kind
            else {
                continue;
            };
            if cell_object != object || actual_cell_offset != cell_offset {
                continue;
            }
            if let Some(consumer) = match_core(
                exe,
                instructions,
                reached,
                resource_index,
                cell_index,
                object,
                resource_id,
                cell_id,
                resource_offset,
                cell_offset,
            ) {
                out.push(consumer);
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn match_core(
    exe: &'static str,
    instructions: &[Instruction],
    reached: &BTreeSet<u32>,
    resource_index: usize,
    cell_index: usize,
    object: Reg,
    resource_id: Reg,
    cell_id: Reg,
    resource_offset: i16,
    cell_offset: i16,
) -> Option<BtxCellConsumer> {
    let first_load = resource_index.min(cell_index);
    let start = first_load.saturating_sub(PAIR_WINDOW_WORDS);
    let end = (resource_index.max(cell_index) + SEARCH_WINDOW_WORDS).min(instructions.len());
    let window = &instructions[start..end];

    let resource_x8 = find_after(window, resource_index - start, |kind| match *kind {
        K::Sll { rd, rt, sa: 3 } if rt == resource_id => Some(rd),
        _ => None,
    })?;
    let (resource_x9_index, resource_x9) =
        find_after(window, resource_x8.0 + 1, |kind| match *kind {
            K::Addu { rd, rs, rt } if operands_are(rs, rt, resource_x8.1, resource_id) => Some(rd),
            _ => None,
        })?;
    let (resource_x36_index, resource_x36) =
        find_after(window, resource_x9_index + 1, |kind| match *kind {
            K::Sll { rd, rt, sa: 2 } if rt == resource_x9 => Some(rd),
            _ => None,
        })?;

    let (cell_x2_index, cell_x2) = find_after(window, cell_index - start, |kind| match *kind {
        K::Sll { rd, rt, sa: 1 } if rt == cell_id => Some(rd),
        _ => None,
    })?;
    let (cell_x3_index, cell_x3) = find_after(window, cell_x2_index + 1, |kind| match *kind {
        K::Addu { rd, rs, rt } if operands_are(rs, rt, cell_x2, cell_id) => Some(rd),
        _ => None,
    })?;
    let (cell_x12_index, cell_x12) = find_after(window, cell_x3_index + 1, |kind| match *kind {
        K::Sll { rd, rt, sa: 2 } if rt == cell_x3 => Some(rd),
        _ => None,
    })?;

    for table_index in resource_index - start..window.len() {
        let K::Lw {
            rt: descriptor_table,
            base: table_address,
            ..
        } = window[table_index].kind
        else {
            continue;
        };
        let has_lui = window[..table_index].iter().rev().take(12).any(
            |instruction| matches!(instruction.kind, K::Lui { rt, .. } if rt == table_address),
        );
        if !has_lui {
            continue;
        }
        let Some((descriptor_index, descriptor)) =
            find_after(window, table_index + 1, |kind| match *kind {
                K::Addu { rd, rs, rt } if operands_are(rs, rt, descriptor_table, resource_x36) => {
                    Some(rd)
                }
                _ => None,
            })
        else {
            continue;
        };
        let Some((cell_table_index, cell_table)) =
            find_after(window, descriptor_index + 1, |kind| match *kind {
                K::Lw {
                    rt,
                    base,
                    offset: 0x001C,
                } if base == descriptor => Some(rt),
                _ => None,
            })
        else {
            continue;
        };
        let final_start = cell_table_index.max(cell_x12_index) + 1;
        let Some((cell_entry_index, _)) = find_after(window, final_start, |kind| match *kind {
            K::Addu { rd, rs, rt } if operands_are(rs, rt, cell_table, cell_x12) => Some(rd),
            _ => None,
        }) else {
            continue;
        };
        let dataflow_is_unclobbered = [
            (resource_index - start, resource_x8.0, resource_id),
            (resource_x8.0, resource_x9_index, resource_x8.1),
            (resource_x8.0, resource_x9_index, resource_id),
            (resource_x9_index, resource_x36_index, resource_x9),
            (resource_x36_index, descriptor_index, resource_x36),
            (table_index, descriptor_index, descriptor_table),
            (cell_index - start, cell_x2_index, cell_id),
            (cell_x2_index, cell_x3_index, cell_x2),
            (cell_x2_index, cell_x3_index, cell_id),
            (cell_x3_index, cell_x12_index, cell_x3),
            (descriptor_index, cell_table_index, descriptor),
            (cell_table_index, cell_entry_index, cell_table),
            (cell_x12_index, cell_entry_index, cell_x12),
        ]
        .iter()
        .all(|&(producer, consumer, register)| {
            register_not_clobbered(window, producer, consumer, register)
        });
        if !dataflow_is_unclobbered {
            continue;
        }
        let core_addresses = [
            instructions[resource_index].addr,
            instructions[cell_index].addr,
            window[resource_x8.0].addr,
            window[resource_x9_index].addr,
            window[resource_x36_index].addr,
            window[table_index].addr,
            window[descriptor_index].addr,
            window[cell_x2_index].addr,
            window[cell_x3_index].addr,
            window[cell_x12_index].addr,
            window[cell_table_index].addr,
            window[cell_entry_index].addr,
        ];
        return Some(BtxCellConsumer {
            exe,
            function_candidate: enclosing_function_entry(instructions, first_load),
            object_register: object,
            resource_field_offset: resource_offset,
            cell_field_offset: cell_offset,
            resource_load: instructions[resource_index].addr,
            cell_load: instructions[cell_index].addr,
            descriptor_table_load: window[table_index].addr,
            descriptor_entry: window[descriptor_index].addr,
            cell_table_load: window[cell_table_index].addr,
            cell_entry: window[cell_entry_index].addr,
            core_entry_reached: core_addresses
                .iter()
                .all(|address| reached.contains(address)),
            renderer_family: BOX_BTX_RENDERER_CORES
                .iter()
                .find(|spec| {
                    spec.exe == exe
                        && spec.function
                            == enclosing_function_entry(instructions, first_load)
                                .unwrap_or_default()
                        && spec.resource_load == instructions[resource_index].addr
                        && spec.cell_load == instructions[cell_index].addr
                })
                .map(|spec| spec.family),
        });
    }
    None
}

fn find_after<T>(
    instructions: &[Instruction],
    start: usize,
    predicate: impl Fn(&K) -> Option<T>,
) -> Option<(usize, T)> {
    instructions
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, instruction)| predicate(&instruction.kind).map(|value| (index, value)))
}

fn operands_are(left: Reg, right: Reg, expected_left: Reg, expected_right: Reg) -> bool {
    (left == expected_left && right == expected_right)
        || (left == expected_right && right == expected_left)
}

fn register_not_clobbered(
    instructions: &[Instruction],
    producer: usize,
    consumer: usize,
    register: Reg,
) -> bool {
    producer < consumer
        && instructions[producer + 1..consumer]
            .iter()
            .all(|instruction| instruction.dest_reg() != Some(register))
}

fn enclosing_function_entry(instructions: &[Instruction], index: usize) -> Option<u32> {
    instructions[..=index]
        .iter()
        .rev()
        .take(FUNCTION_ENTRY_SEARCH_WORDS)
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

fn digest_consumers(consumers: &[BtxCellConsumer]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for consumer in consumers {
        digest.update(consumer.exe.as_bytes());
        digest.update([0]);
        digest.update(
            consumer
                .function_candidate
                .unwrap_or_default()
                .to_le_bytes(),
        );
        digest.update([consumer.object_register.0]);
        digest.update(consumer.resource_field_offset.to_le_bytes());
        digest.update(consumer.cell_field_offset.to_le_bytes());
        digest.update(consumer.resource_load.to_le_bytes());
        digest.update(consumer.cell_load.to_le_bytes());
        digest.update(consumer.descriptor_table_load.to_le_bytes());
        digest.update(consumer.descriptor_entry.to_le_bytes());
        digest.update(consumer.cell_table_load.to_le_bytes());
        digest.update(consumer.cell_entry.to_le_bytes());
        digest.update([u8::from(consumer.core_entry_reached)]);
        digest.update([consumer
            .renderer_family
            .map(BtxRendererFamily::digest_byte)
            .unwrap_or_default()]);
    }
    digest.finalize().into()
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "btx_cell_tests.rs"]
mod btx_cell_tests;
