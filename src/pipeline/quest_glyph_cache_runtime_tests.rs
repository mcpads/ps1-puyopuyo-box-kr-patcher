use super::*;

#[test]
fn runtime_slice는_kseg주소를_main_ram_offset으로_검증한다() {
    let mut ram = vec![0_u8; PSX_MAIN_RAM_LEN];
    let offset = psx_main_ram_offset(QUEST_PAGE_CARRIER_START_RUNTIME, 4).unwrap();
    ram[offset..offset + 4].copy_from_slice(&[1, 2, 3, 4]);

    assert_eq!(
        runtime_slice(&ram, QUEST_PAGE_CARRIER_START_RUNTIME, 4, "test").unwrap(),
        [1, 2, 3, 4]
    );
    assert!(runtime_slice(
        &ram[..offset + 3],
        QUEST_PAGE_CARRIER_START_RUNTIME,
        4,
        "test"
    )
    .unwrap_err()
    .to_string()
    .contains("잘림"));
    assert!(runtime_slice(&ram, 0x8020_0000, 1, "test").is_err());
}
