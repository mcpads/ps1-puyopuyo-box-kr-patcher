use super::*;

fn write_u32(raw: &mut [u8], offset: usize, value: u32) {
    raw[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u16(raw: &mut [u8], offset: usize, value: u16) {
    raw[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_runtime_u32(raw: &mut [u8], runtime: u32, value: u32) {
    let offset = psx_main_ram_offset(runtime, 4).unwrap();
    write_u32(raw, offset, value);
}

fn fixture() -> (Vec<u8>, Vec<u8>) {
    let mut initialized = vec![0; PUYO2_DRAC_INITIALIZED_BYTES];
    for (index, &runtime) in PUYO2_DRAC_COMMAND_RECORDS.iter().enumerate() {
        let offset = (runtime - PUYO2_DRAC_OVERLAY_RUNTIME) as usize;
        initialized[offset..offset + 4].copy_from_slice(&[0xE0, u8::from(index == 2), 0, 0]);
        write_u32(
            &mut initialized,
            offset + 4,
            0x801B_6CFA + index as u32 * 0x20,
        );
    }
    let mut ram = vec![0; PSX_MAIN_RAM_LEN];
    write_runtime_u32(
        &mut ram,
        PUYO2_DRAC_MAIN_RESOURCE_GLOBAL_RUNTIME,
        u32::from(PUYO2_DRAC_GLYPH_RESOURCE),
    );
    write_runtime_u32(&mut ram, PUYO2_DRAC_AUXILIARY_RESOURCE_GLOBAL_RUNTIME, 136);
    let overlay_offset =
        psx_main_ram_offset(PUYO2_DRAC_OVERLAY_RUNTIME, PUYO2_DRAC_INITIALIZED_BYTES).unwrap();
    ram[overlay_offset..overlay_offset + initialized.len()].copy_from_slice(&initialized);

    let message_object_offset = 0x10_0000usize;
    write_u32(
        &mut ram,
        message_object_offset + PUYO2_DRAC_MESSAGE_CALLBACK_FIELD,
        PUYO2_DRAC_MESSAGE_UPDATE_RUNTIME,
    );
    write_u32(&mut ram, message_object_offset + 0x50, 5);
    write_u32(&mut ram, message_object_offset + 0x54, 2);
    write_u32(&mut ram, message_object_offset + 0x58, 9);
    write_u32(&mut ram, message_object_offset + 0x98, 20);
    write_u32(&mut ram, message_object_offset + 0xA4, 12);
    write_u32(&mut ram, message_object_offset + 0xA8, 1);
    write_u32(&mut ram, message_object_offset + 0xAC, 0);

    let script_object_offset = 0x11_0000usize;
    write_u32(
        &mut ram,
        script_object_offset + PUYO2_DRAC_SCRIPT_CURRENT_RECORD_FIELD,
        PUYO2_DRAC_COMMAND_RECORDS[2],
    );

    let glyph_object_offset = 0x12_0000usize;
    write_u32(
        &mut ram,
        glyph_object_offset + PUYO2_DRAC_GLYPH_CALLBACK_FIELD,
        PUYO2_DRAC_GLYPH_UPDATE_RUNTIME,
    );
    write_u32(&mut ram, glyph_object_offset + 0x50, 0x1000);
    write_u32(&mut ram, glyph_object_offset + 0x54, 0x2000);
    write_u16(
        &mut ram,
        glyph_object_offset + PUYO2_DRAC_GLYPH_CELL_FIELD,
        64,
    );
    write_u16(
        &mut ram,
        glyph_object_offset + PUYO2_DRAC_GLYPH_RESOURCE_FIELD,
        PUYO2_DRAC_GLYPH_RESOURCE,
    );
    write_u16(
        &mut ram,
        glyph_object_offset + PUYO2_DRAC_GLYPH_ACTIVE_FIELD,
        1,
    );
    ram[glyph_object_offset + PUYO2_DRAC_GLYPH_KIND_FIELD] = 1;
    (ram, initialized)
}

#[test]
fn overlay와_exact_e0_message_object를_분리해_관측한다() {
    let (ram, initialized) = fixture();
    let observation = inspect_verified_puyo2_drac_ram(&ram, &initialized).unwrap();
    assert!(observation.overlay_initialized_exact);
    assert_eq!(observation.callback_occurrences, 1);
    assert_eq!(observation.exact_record_candidates, 1);
    assert_eq!(observation.message_candidates.len(), 1);
    assert_eq!(observation.glyph_callback_occurrences, 1);
    assert_eq!(observation.exact_main_glyph_candidates, 1);
    assert_eq!(observation.exact_same_named_glyph_candidates, 0);
    assert_eq!(observation.main_resource_global, 140);
    assert_eq!(observation.auxiliary_resource_global, 136);
    assert_eq!(observation.glyph_candidates.len(), 1);
    assert_eq!(observation.current_record_candidates.len(), 1);
    assert_eq!(
        observation.message_candidates[0].object_runtime,
        0x8010_0000
    );
    assert_eq!(
        observation.current_record_candidates[0].object_runtime,
        0x8011_0000
    );
    assert_eq!(observation.current_record_candidates[0].record_index, 3);
    assert_eq!(observation.current_record_candidates[0].variant, 1);
    assert_eq!(observation.message_candidates[0].column, 5);
    assert_eq!(observation.glyph_candidates[0].object_runtime, 0x8012_0000);
    assert_eq!(observation.glyph_candidates[0].cell, 64);
    assert_eq!(observation.glyph_candidates[0].resource, 140);
    assert!(observation.glyph_candidates[0].main_contract_matches);
    assert!(!observation.glyph_candidates[0].same_named_contract_matches);
    assert_eq!(
        observation.observation_status(),
        "drac-overlay-message-object-and-e0-record-observed-screen-unverified"
    );
    assert!(render_puyo2_drac_ram_tsv(&observation)
        .contains("raw-ram-only-screen-clut-vram-draw-unverified"));
    assert!(render_puyo2_drac_ram_tsv(&observation).contains("main-contract=true"));
}

#[test]
fn glyph_callback은_resource_cell계약과_관측을_분리한다() {
    let (mut ram, initialized) = fixture();
    write_u16(
        &mut ram,
        0x12_0000 + PUYO2_DRAC_GLYPH_RESOURCE_FIELD,
        PUYO2_DRAC_GLYPH_RESOURCE + 1,
    );
    let observation = inspect_verified_puyo2_drac_ram(&ram, &initialized).unwrap();
    assert_eq!(observation.glyph_callback_occurrences, 1);
    assert_eq!(observation.glyph_candidates.len(), 1);
    assert_eq!(observation.exact_main_glyph_candidates, 0);
    assert_eq!(observation.exact_same_named_glyph_candidates, 1);
    assert!(!observation.glyph_candidates[0].main_contract_matches);
    assert!(observation.glyph_candidates[0].same_named_contract_matches);
}

#[test]
fn 두_object중_한쪽만_있으면_부분관측으로_남긴다() {
    let (mut ram, initialized) = fixture();
    write_u32(&mut ram, 0x10_0000 + PUYO2_DRAC_MESSAGE_CALLBACK_FIELD, 0);
    let observation = inspect_verified_puyo2_drac_ram(&ram, &initialized).unwrap();
    assert!(observation.message_candidates.is_empty());
    assert_eq!(observation.current_record_candidates.len(), 1);
    assert_eq!(
        observation.observation_status(),
        "drac-overlay-loaded-partial-runtime-observation"
    );
}

#[test]
fn overlay가_다르면_callback이_있어도_loaded로_승격하지_않는다() {
    let (mut ram, initialized) = fixture();
    let overlay_offset = psx_main_ram_offset(PUYO2_DRAC_OVERLAY_RUNTIME, 1).unwrap();
    ram[overlay_offset] ^= 1;
    let observation = inspect_verified_puyo2_drac_ram(&ram, &initialized).unwrap();
    assert!(!observation.overlay_initialized_exact);
    assert_eq!(observation.observation_status(), "drac-overlay-not-loaded");
}

#[test]
fn 길이와_source_digest는_실패_폐쇄다() {
    let (ram, initialized) = fixture();
    assert!(matches!(
        inspect_verified_puyo2_drac_ram(&[], &initialized),
        Err(Puyo2DracRamError::WrongRamLength { .. })
    ));
    assert!(matches!(
        inspect_puyo2_drac_ram(&ram, &[0; PUYO2_DRAC_SEQUENCE_BYTES]),
        Err(Puyo2DracRamError::SequenceDigestMismatch)
    ));
}
