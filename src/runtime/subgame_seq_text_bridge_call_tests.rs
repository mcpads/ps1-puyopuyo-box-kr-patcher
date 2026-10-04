use serde_json::{json, Map, Value};

use super::*;
use crate::disasm::SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES;

struct Fixture {
    ram: Vec<u8>,
    executables: Vec<(&'static ExeMap, Vec<u8>)>,
    events: Vec<u8>,
    screen: Vec<u8>,
    source: SubgameSeqTextJalrRuntimeFieldSource,
    target: u32,
}

fn write_runtime(ram: &mut [u8], runtime: u32, raw: &[u8]) {
    let offset = psx_main_ram_offset(runtime, raw.len()).unwrap();
    ram[offset..offset + raw.len()].copy_from_slice(raw);
}

fn fixture() -> Fixture {
    let selected = BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS
        .iter()
        .find(|spec| spec.exe == "PUYO2.EXE")
        .unwrap();
    let source = selected.computed_memory_sources[0];
    let mut executables = Vec::new();
    for (index, spec) in BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS.iter().enumerate() {
        let map = crate::coords::exe_map(spec.exe).unwrap();
        let mut text = vec![0; map.text_len as usize];
        for outside in spec.outside_text_sources {
            let start = map.text_offset(outside.read_runtime).unwrap();
            let end = map.text_offset(outside.call_runtime + 8).unwrap();
            text[start..end].fill((index + 1) as u8);
        }
        executables.push((map, text));
    }

    let selected_map = crate::coords::exe_map(selected.exe).unwrap();
    let selected_text = executables
        .iter()
        .find(|(map, _)| map.name == selected.exe)
        .map(|(_, text)| text)
        .unwrap();
    let mut ram = vec![0; PSX_MAIN_RAM_LEN];
    for outside in selected.outside_text_sources {
        let start = selected_map.text_offset(outside.read_runtime).unwrap();
        let end = selected_map.text_offset(outside.call_runtime + 8).unwrap();
        write_runtime(&mut ram, outside.read_runtime, &selected_text[start..end]);
    }
    write_runtime(
        &mut ram,
        selected.synthetic_record_runtime,
        &SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES,
    );
    let source_start = selected_map.text_offset(source.load_runtime).unwrap();
    let source_end = selected_map.text_offset(source.call_runtime + 8).unwrap();
    write_runtime(
        &mut ram,
        source.load_runtime,
        &selected_text[source_start..source_end],
    );

    let target = selected.dispatcher_runtime;
    let base = 0x800A_0000;
    let field = add_signed_offset(base, source.field_offset, "fixture").unwrap();
    let sp = 0x801F_F000;
    write_runtime(&mut ram, field, &target.to_le_bytes());
    write_runtime(&mut ram, sp + 16, &0x8017_C098u32.to_le_bytes());

    let mut registers = Map::new();
    registers.insert(
        register_name(source.target_reg).unwrap().to_string(),
        Value::from(target),
    );
    registers.insert(
        register_name(source.base_reg).unwrap().to_string(),
        Value::from(base),
    );
    registers.insert("sp".to_string(), Value::from(sp));
    let events = serde_json::to_vec(&json!({
        "dropped": 0,
        "events": [{"pc": source.call_runtime, "registers": registers}]
    }))
    .unwrap();

    let mut screen = vec![0; 24];
    screen[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    screen[12..16].copy_from_slice(b"IHDR");
    screen[16..20].copy_from_slice(&SCREEN_WIDTH.to_be_bytes());
    screen[20..24].copy_from_slice(&SCREEN_HEIGHT.to_be_bytes());

    Fixture {
        ram,
        executables,
        events,
        screen,
        source,
        target,
    }
}

fn inspect(
    fixture: &Fixture,
) -> Result<SubgameSeqTextBridgeCallObservation, SubgameSeqTextBridgeCallError> {
    inspect_box_subgame_seq_text_bridge_call(
        &fixture.ram,
        &fixture.events,
        &fixture.screen,
        fixture
            .executables
            .iter()
            .map(|(map, text)| (*map, text.as_slice())),
    )
}

#[test]
fn hit_target과_field와_stack_arg5를_한_관측으로_결속한다() {
    let fixture = fixture();
    let observation = inspect(&fixture).unwrap();
    assert_eq!(observation.exe, "PUYO2.EXE");
    assert_eq!(observation.call_runtime, fixture.source.call_runtime);
    assert_eq!(observation.target_runtime, fixture.target);
    assert_eq!(observation.fifth_argument, 0x8017_C098);
    assert_eq!(observation.target_classification, "text-dispatcher-target");
    assert!(!observation.constructor_target_observed());
    assert!(render_subgame_seq_text_bridge_calls_tsv(&[observation])
        .contains("single-hit-source-bound-target-and-stack-arg5"));
}

#[test]
fn target_register와_field_readback이_다르면_실패한다() {
    let mut fixture = fixture();
    let base = 0x800A_0000;
    let field = add_signed_offset(base, fixture.source.field_offset, "fixture").unwrap();
    write_runtime(&mut fixture.ram, field, &0x8001_0000u32.to_le_bytes());
    assert!(inspect(&fixture)
        .unwrap_err()
        .to_string()
        .contains("target register와 field readback 불일치"));
}

#[test]
fn hit_call_window가_원본과_다르면_실패한다() {
    let mut fixture = fixture();
    let offset = psx_main_ram_offset(fixture.source.load_runtime, 1).unwrap();
    fixture.ram[offset] ^= 1;
    assert!(inspect(&fixture)
        .unwrap_err()
        .to_string()
        .contains("source window가 지원 원본과 다름"));
}

#[test]
fn dropped_event와_여러_event는_부분_보고하지_않는다() {
    let mut dropped_fixture = fixture();
    dropped_fixture.events = serde_json::to_vec(&json!({"dropped": 1, "events": []})).unwrap();
    assert!(inspect(&dropped_fixture)
        .unwrap_err()
        .to_string()
        .contains("dropped=1"));

    let mut multiple_fixture = fixture();
    let event = json!({"pc": multiple_fixture.source.call_runtime, "registers": {}});
    multiple_fixture.events = serde_json::to_vec(&json!({
        "dropped": 0,
        "events": [event.clone(), event]
    }))
    .unwrap();
    assert!(inspect(&multiple_fixture)
        .unwrap_err()
        .to_string()
        .contains("event 수 불일치"));
}
