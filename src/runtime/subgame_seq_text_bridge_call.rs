//! 비-QUEST text bridge의 runtime-field 간접 호출 한 건을 원본과 결속한다.
//!
//! breakpoint event의 hit-time register, 같은 frozen 시점의 2 MiB RAM과 화면을
//! 함께 받는다. 원본 EXE의 load→`jalr` window가 RAM과 같고, target register와
//! object field readback이 일치할 때만 제5인자 stack word를 보고한다. 한 hit는
//! 해당 frame의 target만 닫으며 writer나 다른 caller·frame을 승인하지 않는다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::coords::{psx_main_ram_offset, ExeMap, PSX_MAIN_RAM_LEN};
use crate::disasm::subgame_seq_text_bridge::SubgameSeqTextJalrRuntimeFieldSource;
use crate::disasm::{SubgameSeqTextBridgeSpec, BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS};
use crate::isa::Reg;

use super::inspect_box_subgame_seq_text_bridge_ram;

pub const SUBGAME_SEQ_TEXT_BRIDGE_CALL_SCHEMA_VERSION: &str = "subgame-seq-text-bridge-call-v1";

const SCREEN_WIDTH: u32 = 350;
const SCREEN_HEIGHT: u32 = 240;
const FIFTH_ARGUMENT_STACK_OFFSET: u32 = 16;

/// 한 runtime-field `jalr` hit의 source-bound 관측값.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextBridgeCallObservation {
    pub exe: &'static str,
    pub ram_sha256: [u8; 32],
    pub event_sha256: [u8; 32],
    pub screen_sha256: [u8; 32],
    pub source_window_sha256: [u8; 32],
    pub source_code_windows_verified: usize,
    pub call_runtime: u32,
    pub load_runtime: u32,
    pub target_register: Reg,
    pub base_register: Reg,
    pub field_offset: i16,
    pub base_runtime: u32,
    pub field_runtime: u32,
    pub target_runtime: u32,
    pub stack_runtime: u32,
    pub fifth_argument: u32,
    pub constructor_runtime: u32,
    pub dispatcher_runtime: u32,
    pub target_classification: &'static str,
}

impl SubgameSeqTextBridgeCallObservation {
    pub const fn constructor_target_observed(&self) -> bool {
        self.target_runtime == self.constructor_runtime
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextBridgeCallError(String);

impl fmt::Display for SubgameSeqTextBridgeCallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqTextBridgeCallError {}

fn error(message: impl Into<String>) -> SubgameSeqTextBridgeCallError {
    SubgameSeqTextBridgeCallError(message.into())
}

fn digest(raw: &[u8]) -> [u8; 32] {
    Sha256::digest(raw).into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn register_name(register: Reg) -> Result<&'static str, SubgameSeqTextBridgeCallError> {
    const NAMES: [&str; 32] = [
        "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5", "t6",
        "t7", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1", "gp", "sp",
        "fp", "ra",
    ];
    NAMES
        .get(usize::from(register.0))
        .copied()
        .ok_or_else(|| error(format!("text bridge call register 범위 이탈: {register}")))
}

fn event_u32(
    event: &Value,
    path: &[&str],
    label: &str,
) -> Result<u32, SubgameSeqTextBridgeCallError> {
    let value = path
        .iter()
        .try_fold(event, |value, key| value.get(*key))
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("text bridge call event에 {label}가 없음")))?;
    u32::try_from(value).map_err(|_| error(format!("text bridge call event {label} 범위 이탈")))
}

fn event_register(
    event: &Value,
    register: Reg,
    role: &str,
) -> Result<u32, SubgameSeqTextBridgeCallError> {
    let name = register_name(register)?;
    event_u32(event, &["registers", name], &format!("{role} {register}"))
}

fn parse_single_event(raw: &[u8]) -> Result<Value, SubgameSeqTextBridgeCallError> {
    let document: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("text bridge call event JSON 해석 실패: {source}")))?;
    let dropped = document
        .get("dropped")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("text bridge call event dropped가 없음"))?;
    if dropped != 0 {
        return Err(error(format!("text bridge call event dropped={dropped}")));
    }
    let events = document
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| error("text bridge call events가 없음"))?;
    let [event] = events.as_slice() else {
        return Err(error(format!(
            "text bridge call event 수 불일치: expected=1, actual={}",
            events.len()
        )));
    };
    Ok(event.clone())
}

fn runtime_slice<'a>(
    ram: &'a [u8],
    runtime: u32,
    len: usize,
    label: &str,
) -> Result<&'a [u8], SubgameSeqTextBridgeCallError> {
    let offset = psx_main_ram_offset(runtime, len).ok_or_else(|| {
        error(format!(
            "text bridge call {label} PS1 main RAM 범위 이탈: 0x{runtime:08X}+{len}"
        ))
    })?;
    Ok(&ram[offset..offset + len])
}

fn runtime_u32(
    ram: &[u8],
    runtime: u32,
    label: &str,
) -> Result<u32, SubgameSeqTextBridgeCallError> {
    let raw: [u8; 4] = runtime_slice(ram, runtime, 4, label)?
        .try_into()
        .expect("길이를 검증했다");
    Ok(u32::from_le_bytes(raw))
}

fn add_signed_offset(
    base: u32,
    offset: i16,
    label: &str,
) -> Result<u32, SubgameSeqTextBridgeCallError> {
    if offset >= 0 {
        base.checked_add(u32::from(offset.unsigned_abs()))
    } else {
        base.checked_sub(u32::from(offset.unsigned_abs()))
    }
    .ok_or_else(|| error(format!("text bridge call {label} 주소 overflow")))
}

fn png_shape_exact(raw: &[u8]) -> bool {
    raw.get(..8) == Some(b"\x89PNG\r\n\x1a\n")
        && raw.get(12..16) == Some(b"IHDR")
        && raw
            .get(16..20)
            .and_then(|value| value.try_into().ok())
            .map(u32::from_be_bytes)
            == Some(SCREEN_WIDTH)
        && raw
            .get(20..24)
            .and_then(|value| value.try_into().ok())
            .map(u32::from_be_bytes)
            == Some(SCREEN_HEIGHT)
}

fn source_window<'a>(
    map: &ExeMap,
    text: &'a [u8],
    source: SubgameSeqTextJalrRuntimeFieldSource,
) -> Result<&'a [u8], SubgameSeqTextBridgeCallError> {
    let end_runtime = source
        .call_runtime
        .checked_add(8)
        .ok_or_else(|| error("text bridge call source window 끝 주소 overflow"))?;
    let start = map.text_offset(source.load_runtime).ok_or_else(|| {
        error(format!(
            "{} text bridge call source 시작 범위 이탈: 0x{:08X}",
            map.name, source.load_runtime
        ))
    })?;
    let end = map.text_offset(end_runtime).ok_or_else(|| {
        error(format!(
            "{} text bridge call source 끝 범위 이탈: 0x{end_runtime:08X}",
            map.name
        ))
    })?;
    text.get(start..end).ok_or_else(|| {
        error(format!(
            "{} text bridge call source slice 범위 이탈",
            map.name
        ))
    })
}

fn target_classification(
    spec: SubgameSeqTextBridgeSpec,
    map: &ExeMap,
    target: u32,
) -> &'static str {
    if target == spec.constructor_runtime {
        "constructor-target"
    } else if target == spec.dispatcher_runtime {
        "text-dispatcher-target"
    } else if target == 0 {
        "zero-target"
    } else if map.contains(target) {
        "other-exe-text-target"
    } else {
        "outside-exe-target"
    }
}

/// 원본 EXE source와 한 hit의 event·RAM·screen을 함께 검증한다.
pub fn inspect_box_subgame_seq_text_bridge_call<'a>(
    ram: &[u8],
    events_json: &[u8],
    screen_png: &[u8],
    executables: impl IntoIterator<Item = (&'a ExeMap, &'a [u8])>,
) -> Result<SubgameSeqTextBridgeCallObservation, SubgameSeqTextBridgeCallError> {
    if ram.len() != PSX_MAIN_RAM_LEN {
        return Err(error(format!(
            "text bridge call RAM 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
            ram.len()
        )));
    }
    if !png_shape_exact(screen_png) {
        return Err(error(format!(
            "text bridge call 화면 PNG가 {SCREEN_WIDTH}x{SCREEN_HEIGHT}가 아님"
        )));
    }

    let executables = executables.into_iter().collect::<Vec<_>>();
    let ram_observation = inspect_box_subgame_seq_text_bridge_ram(
        ram,
        executables.iter().map(|(map, text)| (*map, *text)),
    )
    .map_err(|source| error(format!("text bridge call EXE/RAM 식별 실패: {source}")))?;
    let spec = BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS
        .iter()
        .copied()
        .find(|spec| spec.exe == ram_observation.exe)
        .ok_or_else(|| error(format!("{} text bridge spec 누락", ram_observation.exe)))?;
    let &(map, text) = executables
        .iter()
        .find(|(map, _)| map.name == ram_observation.exe)
        .ok_or_else(|| error(format!("{} 원본 EXE 누락", ram_observation.exe)))?;

    let event = parse_single_event(events_json)?;
    let call_runtime = event_u32(&event, &["pc"], "PC")?;
    let source = spec
        .computed_memory_sources
        .iter()
        .copied()
        .find(|source| source.call_runtime == call_runtime)
        .ok_or_else(|| {
            error(format!(
                "{} runtime-field call site가 아님: 0x{call_runtime:08X}",
                spec.exe
            ))
        })?;

    let source_window = source_window(map, text, source)?;
    let runtime_window = runtime_slice(
        ram,
        source.load_runtime,
        source_window.len(),
        "source window",
    )?;
    if runtime_window != source_window {
        return Err(error(format!(
            "{} runtime-field source window가 지원 원본과 다름: 0x{:08X}..0x{:08X}",
            spec.exe,
            source.load_runtime,
            source.call_runtime + 8
        )));
    }

    let target_runtime = event_register(&event, source.target_reg, "target")?;
    let base_runtime = event_register(&event, source.base_reg, "base")?;
    let field_runtime = add_signed_offset(base_runtime, source.field_offset, "field")?;
    let field_target = runtime_u32(ram, field_runtime, "target field")?;
    if field_target != target_runtime {
        return Err(error(format!(
            "{} hit-time target register와 field readback 불일치: register=0x{target_runtime:08X}, field=0x{field_target:08X}",
            spec.exe
        )));
    }
    let stack_pointer = event_register(&event, Reg::SP, "stack")?;
    let stack_runtime = stack_pointer
        .checked_add(FIFTH_ARGUMENT_STACK_OFFSET)
        .ok_or_else(|| error("text bridge call 제5인자 stack 주소 overflow"))?;
    let fifth_argument = runtime_u32(ram, stack_runtime, "제5인자 stack word")?;

    Ok(SubgameSeqTextBridgeCallObservation {
        exe: spec.exe,
        ram_sha256: digest(ram),
        event_sha256: digest(events_json),
        screen_sha256: digest(screen_png),
        source_window_sha256: digest(source_window),
        source_code_windows_verified: ram_observation.source_code_windows_verified,
        call_runtime,
        load_runtime: source.load_runtime,
        target_register: source.target_reg,
        base_register: source.base_reg,
        field_offset: source.field_offset,
        base_runtime,
        field_runtime,
        target_runtime,
        stack_runtime,
        fifth_argument,
        constructor_runtime: spec.constructor_runtime,
        dispatcher_runtime: spec.dispatcher_runtime,
        target_classification: target_classification(spec, map, target_runtime),
    })
}

pub fn render_subgame_seq_text_bridge_calls_tsv(
    observations: &[SubgameSeqTextBridgeCallObservation],
) -> String {
    let mut output = String::from(
        "schema_version\texe\tram_sha256\tevent_sha256\tscreen_sha256\t\
         source_window_sha256\tsource_code_windows_verified\tcall_runtime\tload_runtime\t\
         target_register\tbase_register\tfield_offset\tbase_runtime\tfield_runtime\t\
         target_runtime\tstack_runtime\tfifth_argument\tconstructor_runtime\tdispatcher_runtime\t\
         target_classification\tconstructor_target_observed\truntime_verified\trelease_eligible\t\
         claim_boundary\n",
    );
    for observation in observations {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{:+#06X}\t\
             0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\t\
             true\tfalse\tsingle-hit-source-bound-target-and-stack-arg5-not-writer-or-universal-callers",
            SUBGAME_SEQ_TEXT_BRIDGE_CALL_SCHEMA_VERSION,
            observation.exe,
            hex(&observation.ram_sha256),
            hex(&observation.event_sha256),
            hex(&observation.screen_sha256),
            hex(&observation.source_window_sha256),
            observation.source_code_windows_verified,
            observation.call_runtime,
            observation.load_runtime,
            register_name(observation.target_register).expect("검증한 target register"),
            register_name(observation.base_register).expect("검증한 base register"),
            observation.field_offset,
            observation.base_runtime,
            observation.field_runtime,
            observation.target_runtime,
            observation.stack_runtime,
            observation.fifth_argument,
            observation.constructor_runtime,
            observation.dispatcher_runtime,
            observation.target_classification,
            observation.constructor_target_observed(),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

#[cfg(test)]
#[path = "subgame_seq_text_bridge_call_tests.rs"]
mod tests;
