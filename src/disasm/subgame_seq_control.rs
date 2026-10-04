//! 비-QUEST text control의 stream 전이와 인자 폭.
//!
//! `subgame_seq_text_consumer`가 고정한 `FF00`–`FF2F` handler를 작은
//! register dataflow로 다시 읽는다. GP shadow stream pointer에서 시작한
//! 값만 추적해 순차 전진, 절대 pointer branch/call, return, object commit을
//! 분리한다. handler의 다른 게임 상태 의미는 이 모듈이 주장하지 않는다.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use crate::coords::ExeMap;
use crate::isa::{FlowKind, Instruction, InstructionKind as K, Reg};

use super::{linear, SubgameTextConsumerAnalysis};

const CONTROL_COUNT: usize = 48;
const OBJECT_STREAM_OFFSET: i16 = 0x008C;
const OBJECT_RETURN_OFFSET: i16 = 0x0090;
const MAX_HANDLER_INSTRUCTIONS: usize = 4096;

/// control 실행 뒤 text stream이 선택되는 방식.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SubgameControlBehavior {
    /// 추가 인자 없이 현재 stream을 유지한다.
    NoAdvance,
    /// inline 인자를 소비하고 그 다음 token으로 진행한다.
    Sequential { argument_bytes: u8 },
    /// inline 32-bit pointer로 분기한다.
    BranchAbsolute { pointer_bytes: u8 },
    /// 복귀 pointer를 object에 저장하고 inline 32-bit pointer로 분기한다.
    CallAbsolute { pointer_bytes: u8 },
    /// object에 저장된 복귀 pointer를 복원한다.
    Return,
    /// 고정 header를 읽은 뒤 조건에 따라 다른 stream으로 전환한다.
    DynamicSelect { header_bytes: u8 },
    /// 현재 stream을 object `+0x8C`에 commit한다.
    CommitObject,
}

impl SubgameControlBehavior {
    pub const fn argument_bytes(self) -> u8 {
        match self {
            Self::NoAdvance | Self::Return | Self::CommitObject => 0,
            Self::Sequential { argument_bytes } => argument_bytes,
            Self::BranchAbsolute { pointer_bytes } | Self::CallAbsolute { pointer_bytes } => {
                pointer_bytes
            }
            Self::DynamicSelect { header_bytes } => header_bytes,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::NoAdvance => "no-advance",
            Self::Sequential { .. } => "sequential",
            Self::BranchAbsolute { .. } => "branch-absolute",
            Self::CallAbsolute { .. } => "call-absolute",
            Self::Return => "return",
            Self::DynamicSelect { .. } => "dynamic-select",
            Self::CommitObject => "commit-object",
        }
    }
}

const NO: SubgameControlBehavior = SubgameControlBehavior::NoAdvance;
const SEQ2: SubgameControlBehavior = SubgameControlBehavior::Sequential { argument_bytes: 2 };
const SEQ4: SubgameControlBehavior = SubgameControlBehavior::Sequential { argument_bytes: 4 };

/// 세 EXE에서 공통으로 검증해야 하는 code별 stream 계약.
pub const BOX_SUBGAME_CONTROL_BEHAVIORS: [SubgameControlBehavior; CONTROL_COUNT] = [
    NO,
    SubgameControlBehavior::BranchAbsolute { pointer_bytes: 4 },
    NO,
    SEQ2,
    SEQ2,
    SEQ2,
    SEQ4,
    SEQ2,
    SubgameControlBehavior::CallAbsolute { pointer_bytes: 4 },
    SubgameControlBehavior::Return,
    SubgameControlBehavior::CommitObject,
    NO,
    NO,
    NO,
    SEQ2,
    SEQ2,
    SEQ2,
    SEQ4,
    SubgameControlBehavior::DynamicSelect { header_bytes: 10 },
    SEQ4,
    SEQ2,
    SubgameControlBehavior::DynamicSelect { header_bytes: 8 },
    NO,
    SEQ2,
    SEQ2,
    NO,
    NO,
    SEQ4,
    SEQ2,
    NO,
    NO,
    NO,
    NO,
    SEQ4,
    SEQ4,
    NO,
    NO,
    NO,
    NO,
    NO,
    NO,
    NO,
    NO,
    NO,
    NO,
    NO,
    NO,
    NO,
];

/// EXE·code별 검산 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameControlSite {
    pub exe: &'static str,
    pub code: u8,
    pub handler_runtime: u32,
    pub behavior: SubgameControlBehavior,
    pub reached_instructions: usize,
    pub trap_paths: usize,
}

/// 세 EXE × 48 control 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameControlAnalysis {
    pub sites: Vec<SubgameControlSite>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameControlError(String);

impl fmt::Display for SubgameControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameControlError {}

fn error(message: impl Into<String>) -> SubgameControlError {
    SubgameControlError(message.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expr {
    Unknown,
    Zero,
    Stream(i32),
    Argument { offset: i32, bytes: u8 },
    ArgumentHigh { offset: i32, bytes: u8 },
    Absolute { argument_bytes: u8 },
    SavedReturn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegisterState {
    registers: [Expr; 32],
    stream_memory: Expr,
}

impl RegisterState {
    fn initial() -> Self {
        let mut registers = [Expr::Unknown; 32];
        registers[Reg::ZERO.0 as usize] = Expr::Zero;
        Self {
            registers,
            stream_memory: Expr::Stream(0),
        }
    }

    fn get(&self, register: Reg) -> Expr {
        self.registers[register.0 as usize]
    }

    fn set(&mut self, register: Reg, value: Expr) {
        if register != Reg::ZERO {
            self.registers[register.0 as usize] = value;
        }
    }

    fn merge(&self, incoming: &Self) -> Self {
        let mut merged = self.clone();
        for (slot, incoming) in merged.registers.iter_mut().zip(incoming.registers) {
            if *slot != incoming {
                *slot = Expr::Unknown;
            }
        }
        if merged.stream_memory != incoming.stream_memory {
            merged.stream_memory = Expr::Unknown;
        }
        merged.registers[Reg::ZERO.0 as usize] = Expr::Zero;
        merged
    }

    fn clobber_after_call(&mut self) {
        for register in [
            Reg::V0,
            Reg::V1,
            Reg::A0,
            Reg::A1,
            Reg::A2,
            Reg::A3,
            Reg::T0,
            Reg::T1,
            Reg::T2,
            Reg::T3,
            Reg(12),
            Reg(13),
            Reg(14),
            Reg(15),
            Reg(24),
            Reg(25),
        ] {
            self.set(register, Expr::Unknown);
        }
    }
}

#[derive(Debug, Default)]
struct Observations {
    stream_offsets: BTreeSet<i32>,
    absolute_argument_bytes: BTreeSet<u8>,
    return_restores: usize,
    saved_return_offsets: BTreeSet<i32>,
    object_commit_offsets: BTreeSet<i32>,
    unknown_stream_stores: BTreeSet<u32>,
    trap_paths: usize,
}

fn argument_end(offset: i32, bytes: u8) -> Option<u8> {
    let end = offset.checked_add(bytes as i32)?;
    u8::try_from(end).ok()
}

fn combine_absolute(left: Expr, right: Expr) -> Option<Expr> {
    let (low_offset, low_bytes, high_offset, high_bytes) = match (left, right) {
        (
            Expr::Argument {
                offset: low_offset,
                bytes: low_bytes,
            },
            Expr::ArgumentHigh {
                offset: high_offset,
                bytes: high_bytes,
            },
        )
        | (
            Expr::ArgumentHigh {
                offset: high_offset,
                bytes: high_bytes,
            },
            Expr::Argument {
                offset: low_offset,
                bytes: low_bytes,
            },
        ) => (low_offset, low_bytes, high_offset, high_bytes),
        _ => return None,
    };
    let bytes = argument_end(low_offset, low_bytes)?.max(argument_end(high_offset, high_bytes)?);
    Some(Expr::Absolute {
        argument_bytes: bytes,
    })
}

fn execute(
    instruction: &Instruction,
    state: &mut RegisterState,
    observations: &mut Observations,
    shadow_gp_offset: i16,
) {
    match instruction.kind {
        K::Lw { rt, base, offset } if base == Reg::GP && offset == shadow_gp_offset => {
            state.set(rt, state.stream_memory);
        }
        K::Lw { rt, base, offset } if base != Reg::GP && offset == OBJECT_RETURN_OFFSET => {
            state.set(rt, Expr::SavedReturn);
        }
        K::Lhu { rt, base, offset } => {
            let value = match state.get(base) {
                Expr::Stream(base_offset) => Expr::Argument {
                    offset: base_offset + offset as i32,
                    bytes: 2,
                },
                _ => Expr::Unknown,
            };
            state.set(rt, value);
        }
        K::Lbu { rt, base, offset } => {
            let value = match state.get(base) {
                Expr::Stream(base_offset) => Expr::Argument {
                    offset: base_offset + offset as i32,
                    bytes: 1,
                },
                _ => Expr::Unknown,
            };
            state.set(rt, value);
        }
        K::Addiu { rt, rs, imm } => {
            let value = match state.get(rs) {
                Expr::Stream(offset) => Expr::Stream(offset + imm as i32),
                Expr::Zero if imm == 0 => Expr::Zero,
                _ => Expr::Unknown,
            };
            state.set(rt, value);
        }
        K::Sll { rd, rt, sa: 16 } => {
            let value = match state.get(rt) {
                Expr::Argument { offset, bytes } => Expr::ArgumentHigh { offset, bytes },
                Expr::Zero => Expr::Zero,
                _ => Expr::Unknown,
            };
            state.set(rd, value);
        }
        K::Addu { rd, rs, rt } => {
            let left = state.get(rs);
            let right = state.get(rt);
            let value = match (left, right) {
                (Expr::Zero, value) | (value, Expr::Zero) => value,
                _ => combine_absolute(left, right).unwrap_or(Expr::Unknown),
            };
            state.set(rd, value);
        }
        K::Sw { rt, base, offset } if base == Reg::GP && offset == shadow_gp_offset => {
            let value = state.get(rt);
            match value {
                Expr::Stream(offset) => {
                    observations.stream_offsets.insert(offset);
                }
                Expr::Absolute { argument_bytes } => {
                    observations.absolute_argument_bytes.insert(argument_bytes);
                }
                Expr::SavedReturn => observations.return_restores += 1,
                _ => {
                    observations.unknown_stream_stores.insert(instruction.addr);
                }
            }
            state.stream_memory = value;
        }
        K::Sw { rt, base, offset } if base != Reg::GP && offset == OBJECT_RETURN_OFFSET => {
            if let Expr::Stream(offset) = state.get(rt) {
                observations.saved_return_offsets.insert(offset);
            }
        }
        K::Sw { rt, base, offset } if base != Reg::GP && offset == OBJECT_STREAM_OFFSET => {
            if let Expr::Stream(offset) = state.get(rt) {
                observations.object_commit_offsets.insert(offset);
            }
        }
        kind => {
            if let Some(destination) =
                Instruction::new(instruction.addr, instruction.word, kind).dest_reg()
            {
                state.set(destination, Expr::Unknown);
            }
        }
    }
}

fn enqueue(
    address: u32,
    state: RegisterState,
    states: &mut BTreeMap<u32, RegisterState>,
    queue: &mut VecDeque<u32>,
) {
    if let Some(previous) = states.get(&address) {
        let merged = previous.merge(&state);
        if &merged == previous {
            return;
        }
        states.insert(address, merged);
    } else {
        states.insert(address, state);
    }
    queue.push_back(address);
}

fn instruction_at(
    instructions: &BTreeMap<u32, Instruction>,
    address: u32,
) -> Result<Instruction, SubgameControlError> {
    instructions
        .get(&address)
        .copied()
        .ok_or_else(|| error(format!("control handler 명령 주소 없음: 0x{address:08X}")))
}

fn execute_delay(
    instructions: &BTreeMap<u32, Instruction>,
    control: Instruction,
    state: &mut RegisterState,
    observations: &mut Observations,
    shadow_gp_offset: i16,
) -> Result<(), SubgameControlError> {
    let delay = instruction_at(instructions, control.addr + 4)?;
    if delay.flow_kind().has_delay_slot() {
        return Err(error(format!(
            "control handler delay slot에 제어 명령이 있음: 0x{:08X}",
            delay.addr
        )));
    }
    execute(&delay, state, observations, shadow_gp_offset);
    Ok(())
}

fn analyze_handler(
    instructions: &BTreeMap<u32, Instruction>,
    entry: u32,
    shadow_gp_offset: i16,
) -> Result<(SubgameControlBehavior, usize, usize), SubgameControlError> {
    let mut states = BTreeMap::new();
    let mut queue = VecDeque::new();
    let mut observations = Observations::default();
    let mut visited_addresses = BTreeSet::new();
    enqueue(entry, RegisterState::initial(), &mut states, &mut queue);

    while let Some(address) = queue.pop_front() {
        if visited_addresses.len() > MAX_HANDLER_INSTRUCTIONS {
            return Err(error(format!(
                "control handler local trace가 상한을 넘음: entry=0x{entry:08X}"
            )));
        }
        visited_addresses.insert(address);
        let instruction = instruction_at(instructions, address)?;
        let mut state = states
            .get(&address)
            .cloned()
            .ok_or_else(|| error("control handler state 누락"))?;
        execute(
            &instruction,
            &mut state,
            &mut observations,
            shadow_gp_offset,
        );
        match instruction.flow_kind() {
            FlowKind::Sequential => enqueue(address + 4, state, &mut states, &mut queue),
            FlowKind::Branch { taken } => {
                execute_delay(
                    instructions,
                    instruction,
                    &mut state,
                    &mut observations,
                    shadow_gp_offset,
                )?;
                enqueue(taken, state.clone(), &mut states, &mut queue);
                enqueue(address + 8, state, &mut states, &mut queue);
            }
            FlowKind::Jump { target } => {
                execute_delay(
                    instructions,
                    instruction,
                    &mut state,
                    &mut observations,
                    shadow_gp_offset,
                )?;
                enqueue(target, state, &mut states, &mut queue);
            }
            FlowKind::Call { .. } => {
                execute_delay(
                    instructions,
                    instruction,
                    &mut state,
                    &mut observations,
                    shadow_gp_offset,
                )?;
                state.clobber_after_call();
                enqueue(address + 8, state, &mut states, &mut queue);
            }
            FlowKind::Return => {
                execute_delay(
                    instructions,
                    instruction,
                    &mut state,
                    &mut observations,
                    shadow_gp_offset,
                )?;
            }
            FlowKind::JumpRegister { rs } => {
                return Err(error(format!(
                    "control handler 안의 미해석 간접 분기: entry=0x{entry:08X}, \
                     pc=0x{address:08X}, register={rs}"
                )));
            }
            FlowKind::Trap => {
                observations.trap_paths += 1;
            }
        }
    }

    let behavior = if !observations.unknown_stream_stores.is_empty() {
        let Some(maximum) = observations.stream_offsets.iter().copied().max() else {
            return Err(error(format!(
                "control handler dynamic stream 전이에 고정 header 전진이 없음: \
                 entry=0x{entry:08X}, stores={:08X?}",
                observations.unknown_stream_stores
            )));
        };
        SubgameControlBehavior::DynamicSelect {
            header_bytes: u8::try_from(maximum).map_err(|_| {
                error(format!(
                    "control handler dynamic header 폭이 u8 밖임: entry=0x{entry:08X}, \
                     bytes={maximum}"
                ))
            })?,
        }
    } else if !observations.absolute_argument_bytes.is_empty() {
        let mut widths = observations.absolute_argument_bytes.iter().copied();
        let Some(argument_bytes) = widths.next() else {
            unreachable!("absolute_argument_bytes가 비어 있지 않음을 검사했다");
        };
        if widths.next().is_some() {
            return Err(error(format!(
                "control handler absolute pointer 폭이 여러 개임: entry=0x{entry:08X}, \
                 widths={:?}",
                observations.absolute_argument_bytes
            )));
        }
        match observations
            .saved_return_offsets
            .iter()
            .copied()
            .collect::<Vec<_>>()
            .as_slice()
        {
            [] => SubgameControlBehavior::BranchAbsolute {
                pointer_bytes: argument_bytes,
            },
            [saved] if *saved == argument_bytes as i32 => SubgameControlBehavior::CallAbsolute {
                pointer_bytes: argument_bytes,
            },
            saved => {
                return Err(error(format!(
                    "control handler call 복귀 offset 불일치: entry=0x{entry:08X}, \
                     argument={argument_bytes}, saved={saved:?}"
                )));
            }
        }
    } else if observations.return_restores != 0 {
        if observations.return_restores != 1 {
            return Err(error(format!(
                "control handler return restore 수 불일치: entry=0x{entry:08X}, \
                 restores={}",
                observations.return_restores
            )));
        }
        SubgameControlBehavior::Return
    } else if let Some(maximum) = observations.stream_offsets.iter().copied().max() {
        if maximum <= 0 || maximum % 2 != 0 {
            return Err(error(format!(
                "control handler 순차 전진 폭 불일치: entry=0x{entry:08X}, offsets={:?}",
                observations.stream_offsets
            )));
        }
        SubgameControlBehavior::Sequential {
            argument_bytes: u8::try_from(maximum).map_err(|_| {
                error(format!(
                    "control handler 순차 전진 폭이 u8 밖임: entry=0x{entry:08X}, bytes={maximum}"
                ))
            })?,
        }
    } else if observations.object_commit_offsets == [0].into_iter().collect() {
        SubgameControlBehavior::CommitObject
    } else if observations.object_commit_offsets.is_empty()
        && observations.saved_return_offsets.is_empty()
    {
        SubgameControlBehavior::NoAdvance
    } else {
        return Err(error(format!(
            "control handler stream 전이 분류 실패: entry=0x{entry:08X}, \
             saved={:?}, commit={:?}",
            observations.saved_return_offsets, observations.object_commit_offsets
        )));
    };
    Ok((behavior, visited_addresses.len(), observations.trap_paths))
}

/// 세 EXE의 `FF00`–`FF2F` stream 계약을 각각 검산한다.
pub fn verify_box_subgame_seq_controls<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    consumer_analysis: &SubgameTextConsumerAnalysis,
) -> Result<SubgameControlAnalysis, SubgameControlError> {
    let mut inputs = BTreeMap::new();
    for (map, text) in executables {
        if inputs.insert(map.name, (map, text)).is_some() {
            return Err(error(format!(
                "subgame control EXE 입력 중복: {}",
                map.name
            )));
        }
    }
    if consumer_analysis.control_tables.len() != 3 || consumer_analysis.consumers.len() != 6 {
        return Err(error("subgame control 입력 consumer 분모 불일치"));
    }

    let mut sites = Vec::with_capacity(3 * CONTROL_COUNT);
    for table in &consumer_analysis.control_tables {
        let &(map, text) = inputs
            .get(table.exe)
            .ok_or_else(|| error(format!("subgame control EXE 입력 누락: {}", table.exe)))?;
        if table.handlers.len() != CONTROL_COUNT {
            return Err(error(format!(
                "{} control handler 수 불일치: expected={CONTROL_COUNT}, actual={}",
                table.exe,
                table.handlers.len()
            )));
        }
        let shadow_offsets = consumer_analysis
            .consumers
            .iter()
            .filter(|consumer| consumer.exe == table.exe)
            .map(|consumer| consumer.shadow_gp_offset)
            .collect::<BTreeSet<_>>();
        let mut offsets = shadow_offsets.iter().copied();
        let Some(shadow_gp_offset) = offsets.next() else {
            return Err(error(format!(
                "{} consumer GP shadow offset이 하나가 아님: {shadow_offsets:?}",
                table.exe
            )));
        };
        if offsets.next().is_some() {
            return Err(error(format!(
                "{} consumer GP shadow offset이 하나가 아님: {shadow_offsets:?}",
                table.exe
            )));
        }
        let swept = linear::sweep(map, text)
            .ok_or_else(|| error(format!("{} text가 4바이트 정렬이 아님", table.exe)))?;
        let instructions = swept
            .into_iter()
            .map(|instruction| (instruction.addr, instruction))
            .collect::<BTreeMap<_, _>>();
        let mut cache = BTreeMap::new();
        for (code, handler_runtime) in table.handlers.iter().copied().enumerate() {
            let (behavior, reached_instructions, trap_paths) = if let Some(cached) =
                cache.get(&handler_runtime)
            {
                *cached
            } else {
                let analyzed = analyze_handler(&instructions, handler_runtime, shadow_gp_offset)?;
                cache.insert(handler_runtime, analyzed);
                analyzed
            };
            let expected = BOX_SUBGAME_CONTROL_BEHAVIORS[code];
            if behavior != expected {
                return Err(error(format!(
                    "{} FF{code:02X} stream 계약 불일치: handler=0x{handler_runtime:08X}, \
                     expected={expected:?}, actual={behavior:?}",
                    table.exe
                )));
            }
            sites.push(SubgameControlSite {
                exe: table.exe,
                code: code as u8,
                handler_runtime,
                behavior,
                reached_instructions,
                trap_paths,
            });
        }
    }
    if sites.len() != 3 * CONTROL_COUNT {
        return Err(error(format!(
            "지원 원본 subgame control site 수 불일치: expected={}, actual={}",
            3 * CONTROL_COUNT,
            sites.len()
        )));
    }
    Ok(SubgameControlAnalysis { sites })
}

/// 세 EXE × 48 code의 handler·인자 폭·전이를 TSV로 만든다.
pub fn render_subgame_seq_controls_tsv(analysis: &SubgameControlAnalysis) -> String {
    let mut out = String::from(
        "exe\tcode\thandler_runtime\tbehavior\targument_bytes\treached_instructions\t\
         trap_paths\tpatch_contract\n",
    );
    for site in &analysis.sites {
        writeln!(
            out,
            "{}\tFF{:02X}\t0x{:08X}\t{}\t{}\t{}\t{}\tpreserve-control-and-arguments",
            site.exe,
            site.code,
            site.handler_runtime,
            site.behavior.label(),
            site.behavior.argument_bytes(),
            site.reached_instructions,
            site.trap_paths,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    out
}

#[cfg(test)]
#[path = "subgame_seq_control_tests.rs"]
mod subgame_seq_control_tests;
