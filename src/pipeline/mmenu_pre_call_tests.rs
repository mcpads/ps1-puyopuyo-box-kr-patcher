use super::*;

#[test]
fn e8_source와_dispatch_slot_주소를_각_coord에서_계산한다() {
    assert_eq!(source_slot_runtime(PRE_CALL_OPCODE).unwrap(), 0x8008_B380);
    assert_eq!(dispatch_slot_runtime(PRE_CALL_OPCODE).unwrap(), 0x8008_BAE0);
    assert!(source_slot_runtime(0xDF).is_err());
}

#[test]
fn direct_jal_census는_대상_callsite만_runtime_주소로_돌려준다() {
    let target = 0x8002_2890;
    let target_jal = 0x0C00_0000 | ((target >> 2) & 0x03FF_FFFF);
    let other_jal = 0x0C00_0000 | ((0x8003_0000 >> 2) & 0x03FF_FFFF);
    let text = [0, target_jal, other_jal, target_jal]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();

    assert_eq!(
        direct_jal_call_sites(0x8001_0000, &text, target).unwrap(),
        [0x8001_0004, 0x8001_000C]
    );
}

#[test]
fn addiu_주소_분해는_음수_low를_보정한다() {
    assert_eq!(addiu_address_parts(0x8008_B360), (0x8009, -0x4CA0));
    assert_eq!(addiu_address_parts(0x8008_1234), (0x8008, 0x1234));
}
