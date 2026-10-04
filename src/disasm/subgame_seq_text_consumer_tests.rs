use crate::coords::LogicalFile;
use crate::isa::{encode_all, InstructionKind as K};

use super::*;

fn map() -> ExeMap {
    ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x1000,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, 0x1800),
    }
}

#[test]
fn sign_addiu로_control_table_주소를_복원한다() {
    assert_eq!(loaded_address(0x8009, -25220), 0x8008_9D7C);
    assert_eq!(loaded_address(0x8005, 2276), 0x8005_08E4);
}

#[test]
fn control_table은_자기_exe_밖_handler를_거부한다() {
    let map = map();
    let mut text = vec![0u8; map.text_len as usize];
    let table_runtime = 0x8001_0800;
    let offset = map.text_offset(table_runtime).unwrap();
    for chunk in text[offset..offset + CONTROL_TABLE_ENTRIES * 4].chunks_exact_mut(4) {
        chunk.copy_from_slice(&0x8001_0100u32.to_le_bytes());
    }
    text[offset..offset + 4].copy_from_slice(&0x8002_0000u32.to_le_bytes());
    let spec = SubgameTextConsumerSpec {
        exe: "TEST.EXE",
        archive: "TEST.VDD",
        control_table_runtime: table_runtime,
        glyph_cache_runtime: 0,
        bios_wrapper_runtime: 0,
        bios_wrapper_call_runtime: 0,
        renderer_short_runtime: 0,
        renderer_extended_runtime: 0,
        constructor_runtime: 0,
        constructor_stream_load_runtime: 0,
        constructor_stream_store_runtime: 0,
        control_table_sha256: [0; 32],
        control_unique_handlers: 0,
        sites: [BOX_SUBGAME_TEXT_CONSUMERS[0].sites[0]; 2],
    };
    assert!(verify_control_table(&spec, &map, &text)
        .unwrap_err()
        .to_string()
        .contains("자기 EXE text 밖"));
}

#[test]
fn require는_typed_instruction_불일치를_거부한다() {
    let map = map();
    let bytes = encode_all(&[K::Addiu {
        rt: Reg::V0,
        rs: Reg::V0,
        imm: 2,
    }]);
    let instructions = linear::sweep(&ExeMap { text_len: 4, ..map }, &bytes).unwrap();
    let error = require(
        &instructions,
        "TEST.EXE",
        0x8001_0000,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 4,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("명령 불일치"));
}

#[test]
fn tsv는_consumer와_table을_분리한다() {
    let analysis = SubgameTextConsumerAnalysis {
        consumers: vec![SubgameTextConsumer {
            exe: "TEST.EXE",
            archive: "TEST.VDD",
            role: "object-loop",
            entry_runtime: 1,
            source_kind: SubgameTextSourceKind::ObjectField,
            source_runtime: 2,
            token_load_runtime: 3,
            pointer_advance_runtime: 4,
            pointer_store_runtime: 5,
            shadow_gp_offset: 6,
            threshold_compare_runtime: 7,
            control_table_runtime: 8,
            indirect_call_runtime: 9,
            glyph_cache_call_runtime: 10,
            renderer_short_call_runtime: 11,
            renderer_extended_call_runtime: 12,
            entry_reached: false,
        }],
        control_tables: vec![SubgameTextControlTable {
            exe: "TEST.EXE",
            runtime: 8,
            entries: 48,
            defined_entries: 3,
            zero_entries: 45,
            unique_handlers: 3,
            handlers: vec![8; 48],
            sha256: [0; 32],
        }],
    };
    let report = render_subgame_text_consumers_tsv(&analysis);
    assert!(report.starts_with("kind\texe\tarchive"));
    assert!(report.contains("consumer\tTEST.EXE"));
    assert!(report.contains("control-table\tTEST.EXE"));
}
