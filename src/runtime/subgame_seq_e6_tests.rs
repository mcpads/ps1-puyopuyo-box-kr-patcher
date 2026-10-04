use super::*;

type SourceFile = (u16, String, Vec<u8>);

fn write_u16(raw: &mut [u8], offset: usize, value: u16) {
    raw[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(raw: &mut [u8], offset: usize, value: u32) {
    raw[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn source_raw(resource: u16) -> Vec<u8> {
    let initialized_bytes = 0x40 + usize::from(resource - PUYO2_E6_RESOURCE_FIRST) * 4;
    let mut raw = vec![0; initialized_bytes + OVERLAY_FOOTER_BYTES];
    raw[0..2].copy_from_slice(&resource.to_le_bytes());
    raw[4..8].copy_from_slice(&E6_COMMAND);
    let footer = initialized_bytes;
    let bss_runtime = PUYO2_E6_OVERLAY_RUNTIME + initialized_bytes as u32;
    for (index, value) in [
        0,
        0,
        0,
        PUYO2_E6_OVERLAY_RUNTIME,
        initialized_bytes as u32,
        bss_runtime,
        0x2C,
        bss_runtime + 0x30,
        0,
        0x0000_8000,
        0x0080_0000,
    ]
    .into_iter()
    .enumerate()
    {
        write_u32(&mut raw, footer + index * 4, value);
    }
    raw
}

fn source_files() -> Vec<SourceFile> {
    (PUYO2_E6_RESOURCE_FIRST..=PUYO2_E6_RESOURCE_LAST)
        .map(|resource| {
            (
                resource,
                format!("E6_{resource:04X}.SEQ"),
                source_raw(resource),
            )
        })
        .collect()
}

fn runtime_sources<'a>(
    files: &'a [SourceFile],
) -> impl Iterator<Item = Puyo2E6RuntimeSource<'a>> + 'a {
    files.iter().map(|(resource, sequence, raw)| {
        let initialized_bytes = raw.len() - OVERLAY_FOOTER_BYTES;
        Puyo2E6RuntimeSource {
            resource: *resource,
            sequence,
            raw,
            sequence_sha256: Sha256::digest(raw).into(),
            load_base: PUYO2_E6_OVERLAY_RUNTIME,
            initialized_bytes,
            bss_bytes: 0x2C,
        }
    })
}

fn used_cells() -> Vec<usize> {
    (0..209).chain(std::iter::once(246)).collect()
}

fn fixture() -> (Vec<u8>, Vec<SourceFile>) {
    let files = source_files();
    let selected = &files[3];
    let initialized_bytes = selected.2.len() - OVERLAY_FOOTER_BYTES;
    let mut ram = vec![0; PSX_MAIN_RAM_LEN];
    let overlay_offset = psx_main_ram_offset(PUYO2_E6_OVERLAY_RUNTIME, initialized_bytes).unwrap();
    ram[overlay_offset..overlay_offset + initialized_bytes]
        .copy_from_slice(&selected.2[..initialized_bytes]);
    let descriptor_runtime =
        PUYO2_E6_DESCRIPTOR_BASE_RUNTIME + u32::from(selected.0) * PUYO2_E6_DESCRIPTOR_STRIDE;
    let descriptor_offset = psx_main_ram_offset(
        descriptor_runtime + PUYO2_E6_DESCRIPTOR_DESTINATION_FIELD,
        4,
    )
    .unwrap();
    write_u32(&mut ram, descriptor_offset, PUYO2_E6_OVERLAY_RUNTIME);

    let pool_offset = psx_main_ram_offset(
        PUYO2_E6_OBJECT_POOL_RUNTIME,
        PUYO2_E6_OBJECT_SLOTS * PUYO2_E6_OBJECT_BYTES,
    )
    .unwrap();
    let glyph_slot = 12;
    let glyph = pool_offset + glyph_slot * PUYO2_E6_OBJECT_BYTES;
    write_u32(
        &mut ram,
        glyph + PUYO2_E6_OBJECT_CALLBACK_FIELD,
        PUYO2_E6_GLYPH_CALLBACK_RUNTIME,
    );
    ram[glyph + PUYO2_E6_OBJECT_TYPE_FIELD] = PUYO2_E6_OBJECT_TYPE;
    write_u32(&mut ram, glyph + PUYO2_E6_OBJECT_X_FIELD, 0x1000);
    write_u32(&mut ram, glyph + PUYO2_E6_OBJECT_Y_FIELD, 0x2000);
    write_u16(&mut ram, glyph + PUYO2_E6_OBJECT_CELL_FIELD, 64);
    write_u16(
        &mut ram,
        glyph + PUYO2_E6_OBJECT_RESOURCE_FIELD,
        PUYO2_E6_GLYPH_RESOURCE,
    );
    write_u16(&mut ram, glyph + PUYO2_E6_OBJECT_ACTIVE_FIELD, 1);

    let script_slot = 30;
    let script = pool_offset + script_slot * PUYO2_E6_OBJECT_BYTES;
    write_u32(
        &mut ram,
        script + PUYO2_E6_OBJECT_CURRENT_RECORD_FIELD,
        PUYO2_E6_OVERLAY_RUNTIME + 4,
    );
    (ram, files)
}

#[test]
fn exact_overlay_descriptor_glyph_current_record를_분리해_관측한다() {
    let (ram, files) = fixture();
    let observation = inspect_puyo2_e6_ram(&ram, runtime_sources(&files), &used_cells()).unwrap();
    assert_eq!(observation.source_sequences, PUYO2_E6_SEQUENCE_COUNT);
    assert_eq!(observation.exact_overlay_matches.len(), 1);
    assert_eq!(observation.exact_overlay_matches[0].resource, 0x0042);
    assert!(observation.exact_overlay_matches[0].descriptor_resident);
    assert_eq!(observation.pool_callback_candidates, 1);
    assert_eq!(observation.exact_glyph_candidates, 1);
    assert_eq!(observation.glyph_candidates[0].slot, 12);
    assert!(observation.glyph_candidates[0].contract_matches);
    assert_eq!(observation.current_record_candidates.len(), 1);
    assert_eq!(observation.current_record_candidates[0].slot, 30);
    assert_eq!(observation.current_record_candidates[0].command_offset, 4);
    assert_eq!(
        observation.observation_status(),
        "e6-overlay-resident-current-record-and-glyph-object-observed-screen-unverified"
    );
    let report = render_puyo2_e6_ram_tsv(&observation);
    assert!(report.contains("raw-ram-snapshot-only-pc-hit-gpu-upload-draw-screen-unverified"));
    assert!(report.contains("contract=true"));
}

#[test]
fn descriptor_destination이_다르면_resident로_승격하지_않는다() {
    let (mut ram, files) = fixture();
    let selected_resource = 0x0042u16;
    let descriptor_runtime = PUYO2_E6_DESCRIPTOR_BASE_RUNTIME
        + u32::from(selected_resource) * PUYO2_E6_DESCRIPTOR_STRIDE;
    let descriptor_offset = psx_main_ram_offset(
        descriptor_runtime + PUYO2_E6_DESCRIPTOR_DESTINATION_FIELD,
        4,
    )
    .unwrap();
    write_u32(&mut ram, descriptor_offset, 0);
    let observation = inspect_puyo2_e6_ram(&ram, runtime_sources(&files), &used_cells()).unwrap();
    assert_eq!(
        observation.observation_status(),
        "e6-overlay-exact-descriptor-residency-unconfirmed"
    );
}

#[test]
fn callback이_맞아도_cell계약이_다르면_exact로_세지_않는다() {
    let (mut ram, files) = fixture();
    let pool_offset = psx_main_ram_offset(
        PUYO2_E6_OBJECT_POOL_RUNTIME,
        PUYO2_E6_OBJECT_SLOTS * PUYO2_E6_OBJECT_BYTES,
    )
    .unwrap();
    write_u16(
        &mut ram,
        pool_offset + 12 * PUYO2_E6_OBJECT_BYTES + PUYO2_E6_OBJECT_CELL_FIELD,
        245,
    );
    let observation = inspect_puyo2_e6_ram(&ram, runtime_sources(&files), &used_cells()).unwrap();
    assert_eq!(observation.pool_callback_candidates, 1);
    assert_eq!(observation.exact_glyph_candidates, 0);
    assert_eq!(
        observation.observation_status(),
        "e6-overlay-resident-glyph-object-not-observed"
    );
}

#[test]
fn ram_source_cell분모_drift를_실패폐쇄한다() {
    let (ram, mut files) = fixture();
    assert!(matches!(
        inspect_puyo2_e6_ram(
            &ram[..ram.len() - 1],
            runtime_sources(&files),
            &used_cells()
        ),
        Err(Puyo2E6RamError::WrongRamLength { .. })
    ));
    let stale_digest: [u8; 32] = Sha256::digest(&files[0].2).into();
    files[0].2[0] ^= 1;
    let stale_sources = files
        .iter()
        .enumerate()
        .map(|(index, (resource, sequence, raw))| Puyo2E6RuntimeSource {
            resource: *resource,
            sequence,
            raw,
            sequence_sha256: if index == 0 {
                stale_digest
            } else {
                Sha256::digest(raw).into()
            },
            load_base: PUYO2_E6_OVERLAY_RUNTIME,
            initialized_bytes: raw.len() - OVERLAY_FOOTER_BYTES,
            bss_bytes: 0x2C,
        });
    assert!(matches!(
        inspect_puyo2_e6_ram(&ram, stale_sources, &used_cells()),
        Err(Puyo2E6RamError::SourceDigestMismatch(_))
    ));
    assert!(matches!(
        inspect_puyo2_e6_ram(&ram, runtime_sources(&source_files()), &[0, 246]),
        Err(Puyo2E6RamError::WrongUsedCellCount { .. })
    ));
}
