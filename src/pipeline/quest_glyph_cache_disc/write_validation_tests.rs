use super::*;
use crate::source::FORM1_DATA_LEN;

const TEST_LOAD_ADDR: u32 = 0x8001_0000;
const TEST_TEXT_LEN: usize = 0x100;
const TEST_RESIDENT_SPAN_LEN: usize = 12;

fn test_map() -> ExeMap {
    ExeMap {
        name: "TEST.EXE",
        load_addr: TEST_LOAD_ADDR,
        text_len: TEST_TEXT_LEN as u32,
        entry_pc: TEST_LOAD_ADDR,
        file: LogicalFile::new(1, (PSX_EXE_HEADER_LEN as usize + TEST_TEXT_LEN) as u32),
    }
}

fn install_resident_source(text: &mut [u8], offset: usize) {
    text[offset..offset + TEST_RESIDENT_SPAN_LEN].copy_from_slice(&[
        0x21, 0, 0, 0, 0x82, 0xA0, 0x82, 0xA2, 0xFF, 0x01, 0xFF, 0x03,
    ]);
}

fn resident_write(
    map: &ExeMap,
    text: &[u8],
    offset: usize,
    unit_id: &str,
    page_id: u8,
) -> QuestResidentTextWrite {
    let source = text[offset..offset + TEST_RESIDENT_SPAN_LEN].to_vec();
    let mut output = source.clone();
    output[..4].copy_from_slice(&[0x21, page_id, 0, 0]);
    output[4] ^= 0x10;
    let record_runtime = map.text_runtime_addr(offset).unwrap();
    QuestResidentTextWrite {
        unit_id: unit_id.to_owned(),
        record_runtime,
        end_runtime: record_runtime + TEST_RESIDENT_SPAN_LEN as u32,
        record_text_offset: offset,
        end_text_offset: offset + TEST_RESIDENT_SPAN_LEN,
        page_id,
        source_sha256: sha256_hex(&source),
        output_sha256: sha256_hex(&output),
        source,
        output,
        glyph_cells: 2,
        status: QuestResidentTextCompositionStatus::ManualDraftNeedsGeometry,
        source_exact: true,
        controls_exact: true,
        terminal_exact: true,
        japanese_glyphs_remaining: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    }
}

fn item_acquisition_write(
    map: &ExeMap,
    text: &[u8],
    offset: usize,
    unit_id: &str,
    page_id: u8,
) -> QuestItemAcquisitionTextWrite {
    let source = text[offset..offset + 7].to_vec();
    let output = vec![30, 0, 31, 0, 0, 0, 0];
    QuestItemAcquisitionTextWrite {
        unit_id: unit_id.to_owned(),
        role: crate::disasm::QuestItemAcquisitionMessageRole::CurrencyAcquiredSuffix,
        runtime: map.text_runtime_addr(offset).unwrap(),
        text_offset: offset,
        page_id,
        source_text: "を入手".to_owned(),
        selected_text: "가나".to_owned(),
        source_sha256: sha256_hex(&source),
        output_sha256: sha256_hex(&output),
        source,
        output,
        glyph_cells: 2,
        status: QuestItemAcquisitionTextCompositionStatus::ManualDraftNeedsRuntimeGeometry,
    }
}

#[test]
fn exe본문길이는_form1_sector단위로_올림된다() {
    assert_eq!(align_up(1, FORM1_DATA_LEN as u32).unwrap(), 2048);
    assert_eq!(align_up(2048, FORM1_DATA_LEN as u32).unwrap(), 2048);
    assert_eq!(align_up(2049, FORM1_DATA_LEN as u32).unwrap(), 4096);
}

#[test]
fn 원본text검사는_선언한_site와resident차이만_허용한다() {
    let original = [0, 1, 2, 3, 4, 5];
    let mut relocated = original;
    relocated[2] = 9;
    relocated[4] = 8;
    assert!(original_text_except_ranges_exact(
        &original,
        &relocated,
        &[(2, 3), (4, 5)]
    ));
    assert!(!original_text_except_ranges_exact(
        &original,
        &relocated,
        &[(2, 3)]
    ));
}

#[test]
fn resident_write는_원본좌표순으로검산하고_0_reservation에만놓인다() {
    let map = test_map();
    let mut text = vec![0; TEST_TEXT_LEN];
    install_resident_source(&mut text, 0x20);
    install_resident_source(&mut text, 0x40);
    let writes = [
        resident_write(&map, &text, 0x40, "later", 8),
        resident_write(&map, &text, 0x20, "earlier", 7),
    ];

    let validated = validate_resident_text_writes(&map, &text, &[(0x80, 0x88)], &writes).unwrap();
    assert_eq!(
        validated
            .iter()
            .map(|span| span.write.unit_id.as_str())
            .collect::<Vec<_>>(),
        ["earlier", "later"]
    );
    validate_resident_relocation_destinations(
        &validated,
        0x1000,
        &[0; 0x1000],
        (0x900, 0x920),
        (0x920, 0x940),
    )
    .unwrap();
}

#[test]
fn resident_write는_stale_sha와원본byte불일치를거부한다() {
    let map = test_map();
    let mut text = vec![0; TEST_TEXT_LEN];
    install_resident_source(&mut text, 0x20);
    let mut write = resident_write(&map, &text, 0x20, "stale", 7);
    write.source_sha256 = "0".repeat(64);
    let error =
        validate_resident_text_writes(&map, &text, &[(0x80, 0x88)], &[write.clone()]).unwrap_err();
    assert!(error.to_string().contains("SHA-256"));

    write.source_sha256 = sha256_hex(&write.source);
    text[0x24] ^= 1;
    let error = validate_resident_text_writes(&map, &text, &[(0x80, 0x88)], &[write]).unwrap_err();
    assert!(error.to_string().contains("원본 byte 불일치"));
}

#[test]
fn resident_write는_좌표drift와hook_site침범을거부한다() {
    let map = test_map();
    let mut text = vec![0; TEST_TEXT_LEN];
    install_resident_source(&mut text, 0x20);
    install_resident_source(&mut text, 0x80);

    let mut drifted = resident_write(&map, &text, 0x20, "drifted", 7);
    drifted.record_runtime += 1;
    drifted.end_runtime += 1;
    let error =
        validate_resident_text_writes(&map, &text, &[(0x80, 0x88)], &[drifted]).unwrap_err();
    assert!(error.to_string().contains("runtime/text 좌표 불일치"));

    let hook_overlap = resident_write(&map, &text, 0x80, "hook-overlap", 8);
    let error =
        validate_resident_text_writes(&map, &text, &[(0x60, 0x68), (0x80, 0x88)], &[hook_overlap])
            .unwrap_err();
    assert!(error.to_string().contains("hook site를 침범"));
}

#[test]
fn resident_write는_중복과겹침을거부한다() {
    let map = test_map();
    let mut text = vec![0; TEST_TEXT_LEN];
    install_resident_source(&mut text, 0x20);
    install_resident_source(&mut text, 0x40);
    let duplicate = [
        resident_write(&map, &text, 0x20, "same", 7),
        resident_write(&map, &text, 0x40, "same", 8),
    ];
    let error =
        validate_resident_text_writes(&map, &text, &[(0x80, 0x88)], &duplicate).unwrap_err();
    assert!(error.to_string().contains("unit ID 중복"));

    let mut overlapping_text = vec![0; TEST_TEXT_LEN];
    install_resident_source(&mut overlapping_text, 0x20);
    install_resident_source(&mut overlapping_text, 0x28);
    let overlapping = [
        resident_write(&map, &overlapping_text, 0x20, "first", 7),
        resident_write(&map, &overlapping_text, 0x28, "second", 8),
    ];
    let error =
        validate_resident_text_writes(&map, &overlapping_text, &[(0x80, 0x88)], &overlapping)
            .unwrap_err();
    assert!(error.to_string().contains("span 겹침"));
}

#[test]
fn resident_destination은_payload경계와_nonzero_reservation을거부한다() {
    let map = test_map();
    let mut text = vec![0; TEST_TEXT_LEN];
    install_resident_source(&mut text, 0x20);
    let writes = [resident_write(&map, &text, 0x20, "resident", 7)];
    let validated = validate_resident_text_writes(&map, &text, &[(0x80, 0x88)], &writes).unwrap();
    let span = &validated[0];

    let mut nonzero_reservation = vec![0; 0x1000];
    nonzero_reservation[span.file_start as usize] = 1;
    let error = validate_resident_relocation_destinations(
        &validated,
        0x1000,
        &nonzero_reservation,
        (0x900, 0x920),
        (0x920, 0x940),
    )
    .unwrap_err();
    assert!(error.to_string().contains("0 reservation이 아님"));

    let error = validate_resident_relocation_destinations(
        &validated,
        0x1000,
        &[0; 0x1000],
        (span.file_start, span.file_end),
        (0x920, 0x940),
    )
    .unwrap_err();
    assert!(error.to_string().contains("payload 경계를 침범"));

    let error = validate_resident_relocation_destinations(
        &validated,
        u32::try_from(span.file_end - 1).unwrap(),
        &[0; 0x1000],
        (0x900, 0x920),
        (0x920, 0x940),
    )
    .unwrap_err();
    assert!(error.to_string().contains("payload 경계를 침범"));
}

#[test]
fn item_acquisition_write는_pair내_zero와_명시적terminal을_구분한다() {
    let map = test_map();
    let mut text = vec![0; TEST_TEXT_LEN];
    text[0x20..0x27].copy_from_slice(&[0x82, 0xA0, 0x82, 0xA2, 0x82, 0xA4, 0]);
    let writes = [item_acquisition_write(&map, &text, 0x20, "fixed", 7)];

    let validated = validate_item_acquisition_text_writes(&map, &text, &[], &writes).unwrap();
    assert_eq!(validated[0].write.output, [30, 0, 31, 0, 0, 0, 0]);
    validate_item_acquisition_relocation_destinations(
        &validated,
        0x1000,
        &[0; 0x1000],
        (0x900, 0x920),
        (0x920, 0x940),
    )
    .unwrap();
}

#[test]
fn item_acquisition_write는_보호범위와_nonzero_padding을_거부한다() {
    let map = test_map();
    let mut text = vec![0; TEST_TEXT_LEN];
    text[0x20..0x27].copy_from_slice(&[0x82, 0xA0, 0x82, 0xA2, 0x82, 0xA4, 0]);
    let mut write = item_acquisition_write(&map, &text, 0x20, "fixed", 7);
    let error =
        validate_item_acquisition_text_writes(&map, &text, &[(0x24, 0x28)], &[write.clone()])
            .unwrap_err();
    assert!(error.to_string().contains("보호 경계 불일치"));

    write.output[6] = 1;
    write.output_sha256 = sha256_hex(&write.output);
    let error = validate_item_acquisition_text_writes(&map, &text, &[], &[write]).unwrap_err();
    assert!(error.to_string().contains("NUL/padding 불일치"));
}

#[test]
fn supplemental기계어는_원본byte와주소가_맞을때만_site가된다() {
    let map = test_map();
    let mut text = vec![0; TEST_TEXT_LEN];
    let runtime = TEST_LOAD_ADDR + 0x20;
    let original = verify_bytes(IsaProfile::DECLARED, runtime, &[0, 0, 0, 0]).unwrap();
    let replacement =
        verify_bytes(IsaProfile::DECLARED, runtime, &0x2402_0001u32.to_le_bytes()).unwrap();
    let patch = QuestGlyphCacheMachineCodePatch {
        original: &original,
        replacement: &replacement,
        id: "supplemental-fixture",
        purpose: "fixture",
    };
    let mut sites = Vec::new();
    let patches = [patch];
    append_supplemental_machine_sites(&map, &text, &mut sites, &patches).unwrap();
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].code.bytes(), replacement.bytes());

    text[0x20] = 1;
    let stale = QuestGlyphCacheMachineCodePatch {
        original: &original,
        replacement: &replacement,
        id: "stale-fixture",
        purpose: "fixture",
    };
    let stale_patches = [stale];
    assert!(
        append_supplemental_machine_sites(&map, &text, &mut Vec::new(), &stale_patches).is_err()
    );
}

#[test]
fn supplemental_data는_source_digest와좌표를검증해_0_reservation에만쓴다() {
    let map = test_map();
    let mut text = vec![0; TEST_TEXT_LEN];
    text[0x20..0x28].copy_from_slice(&[0x82, 0xA0, 0, 0, 0, 0, 0, 0]);
    let replacement = [30, 0, 31, 0, 0, 0, 0, 0];
    let original_sha256 = sha256_hex(&text[0x20..0x28]);
    let replacement_sha256 = sha256_hex(&replacement);
    let patches = [QuestGlyphCacheDataPatch {
        runtime: map.text_runtime_addr(0x20).unwrap(),
        original: &text[0x20..0x28],
        replacement: &replacement,
        original_sha256: &original_sha256,
        replacement_sha256: &replacement_sha256,
        required_page_id: Some(7),
        id: "ui-literal",
        purpose: "fixture",
    }];

    let validated = validate_data_patches(&map, &text, &[(0x80, 0x88)], &patches).unwrap();
    assert_eq!(validated[0].text_start, 0x20);
    validate_data_patch_relocation_destinations(
        &validated,
        0x1000,
        &[0; 0x1000],
        (0x900, 0x920),
        (0x920, 0x940),
    )
    .unwrap();
}

#[test]
fn supplemental_data는_stale_digest와보호범위침범을거부한다() {
    let map = test_map();
    let mut text = vec![0; TEST_TEXT_LEN];
    text[0x20..0x28].copy_from_slice(&[0x82, 0xA0, 0, 0, 0, 0, 0, 0]);
    let replacement = [30, 0, 31, 0, 0, 0, 0, 0];
    let replacement_sha256 = sha256_hex(&replacement);
    let stale = "0".repeat(64);
    let stale_patches = [QuestGlyphCacheDataPatch {
        runtime: map.text_runtime_addr(0x20).unwrap(),
        original: &text[0x20..0x28],
        replacement: &replacement,
        original_sha256: &stale,
        replacement_sha256: &replacement_sha256,
        required_page_id: Some(7),
        id: "stale-ui-literal",
        purpose: "fixture",
    }];
    assert!(validate_data_patches(&map, &text, &[], &stale_patches).is_err());

    let original_sha256 = sha256_hex(&text[0x20..0x28]);
    let protected_patches = [QuestGlyphCacheDataPatch {
        runtime: map.text_runtime_addr(0x20).unwrap(),
        original: &text[0x20..0x28],
        replacement: &replacement,
        original_sha256: &original_sha256,
        replacement_sha256: &replacement_sha256,
        required_page_id: Some(7),
        id: "protected-ui-literal",
        purpose: "fixture",
    }];
    let error =
        validate_data_patches(&map, &text, &[(0x24, 0x2C)], &protected_patches).unwrap_err();
    assert!(error.to_string().contains("보호 경계 불일치"));
}
