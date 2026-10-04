use super::*;

fn fixture(pointer: u32) -> Vec<u8> {
    let mut ram = vec![0; PS1_MAIN_RAM_BYTES];
    let spec = BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS
        .iter()
        .find(|spec| spec.exe == "PUYO2.EXE")
        .unwrap();
    let offset = (spec.synthetic_record_runtime & 0x1F_FFFF) as usize;
    ram[offset..offset + SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES.len()]
        .copy_from_slice(&SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES);
    ram[offset + 4..offset + 8].copy_from_slice(&pointer.to_le_bytes());
    ram
}

#[test]
fn zero_pointer는_미관측으로_보고한다() {
    let observation = inspect_puyo2_subgame_seq_text_bridge_ram(&fixture(0)).unwrap();
    assert_eq!(observation.synthetic_pointer, 0);
    assert_eq!(observation.synthetic_record_occurrences, 1);
    assert_eq!(observation.constructor_pointer_occurrences, 0);
    assert_eq!(observation.outside_text_slots.len(), 2);
    assert_eq!(
        observation.outside_text_status(),
        "outside-target-not-observed-at-this-ram"
    );
    assert_eq!(
        observation.observation_status(),
        "source-not-observed-at-synthetic-record"
    );
}

#[test]
fn nonzero_pointer도_source_binding을_승인하지_않는다() {
    let pointer = 0x801B_6F00;
    let observation = inspect_puyo2_subgame_seq_text_bridge_ram(&fixture(pointer)).unwrap();
    assert_eq!(observation.synthetic_pointer, pointer);
    let report = render_subgame_seq_text_bridge_ram_tsv(&observation);
    assert!(report.contains("source-pointer-observed-needs-binding"));
    assert!(report.contains("observed-ram-only-not-universal-nonexecution"));
}

#[test]
fn outside_slot의_constructor값도_writer_binding전에는_승인하지_않는다() {
    let mut ram = fixture(0);
    let spec = BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS
        .iter()
        .find(|spec| spec.exe == "PUYO2.EXE")
        .unwrap();
    let offset = (spec.outside_text_sources[0].source_runtime & 0x1F_FFFF) as usize;
    ram[offset..offset + 4].copy_from_slice(&spec.constructor_runtime.to_le_bytes());
    let observation = inspect_puyo2_subgame_seq_text_bridge_ram(&ram).unwrap();
    assert_eq!(
        observation.outside_text_status(),
        "constructor-target-observed-needs-writer-binding"
    );
}

#[test]
fn 원본_code_window로_세_exe중_하나를_식별한다() {
    let mut executables = Vec::new();
    for (index, spec) in BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS.iter().enumerate() {
        let map = crate::coords::exe_map(spec.exe).unwrap();
        let mut text = vec![0; map.text_len as usize];
        for source in spec.outside_text_sources {
            let start = map.text_offset(source.read_runtime).unwrap();
            let end = map.text_offset(source.call_runtime + 8).unwrap();
            text[start..end].fill((index + 1) as u8);
        }
        executables.push((map, text));
    }

    let selected = BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS
        .iter()
        .find(|spec| spec.exe == "PCOMI.EXE")
        .unwrap();
    let selected_map = crate::coords::exe_map(selected.exe).unwrap();
    let selected_text = executables
        .iter()
        .find(|(map, _)| map.name == selected.exe)
        .map(|(_, text)| text)
        .unwrap();
    let mut ram = vec![0; PS1_MAIN_RAM_BYTES];
    for source in selected.outside_text_sources {
        let start = selected_map.text_offset(source.read_runtime).unwrap();
        let end = selected_map.text_offset(source.call_runtime + 8).unwrap();
        let ram_start = (source.read_runtime & 0x1F_FFFF) as usize;
        ram[ram_start..ram_start + end - start].copy_from_slice(&selected_text[start..end]);
    }
    let record_offset = (selected.synthetic_record_runtime & 0x1F_FFFF) as usize;
    ram[record_offset..record_offset + SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES.len()]
        .copy_from_slice(&SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES);

    let observation = inspect_box_subgame_seq_text_bridge_ram(
        &ram,
        executables
            .iter()
            .map(|(map, text)| (*map, text.as_slice())),
    )
    .unwrap();
    assert_eq!(observation.exe, "PCOMI.EXE");
    assert_eq!(observation.source_code_windows_verified, 2);
    assert_eq!(observation.outside_text_slots.len(), 2);
}

#[test]
fn 크기와_record_불변부는_실패_폐쇄다() {
    assert!(matches!(
        inspect_puyo2_subgame_seq_text_bridge_ram(&[]),
        Err(SubgameSeqTextBridgeRamError::WrongRamLength { .. })
    ));
    let mut ram = fixture(0);
    let offset = (0x8008_BBE0u32 & 0x1F_FFFF) as usize;
    ram[offset] ^= 1;
    assert!(matches!(
        inspect_puyo2_subgame_seq_text_bridge_ram(&ram),
        Err(SubgameSeqTextBridgeRamError::ImmutableSyntheticRecordMismatch { .. })
    ));
}
