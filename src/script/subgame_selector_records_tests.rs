use super::*;

#[test]
fn 알려진_일곱_selector형식의_type과_resource를_해석한다() {
    let raw = [
        0x28, 0x01, 0x01, 0xA1, 62, 0, // OP28/sub01
        0x28, 0x03, 0x00, 0xA3, 165, 0, // OP28/sub03
        0x28, 0x04, 0x01, 0xA4, 62, 0, // OP28/sub04
        0x28, 0x10, 0xB0, 0x00, 165, 0, // OP28/sub10
        0x28, 0x12, 0xB2, 0x01, 62, 0, // OP28/sub12
        0x39, 0x00, 165, 0, 0xFE, 0xFF, // main OP39
        0xC3, 0xFE, 0xFF, 0x01, 62, 0, 0x00, 0x00, // main OPC3
    ];

    let records = scan_subgame_selector_records(&raw);

    assert_eq!(records.len(), 7);
    assert_eq!(records[0].form, SubgameSelectorRecordForm::Op28Sub01);
    assert_eq!(records[0].flags, Some(0xA1));
    assert_eq!(records[0].record_type, 1);
    assert_eq!(records[0].resource_id, 62);
    assert_eq!(records[4].form, SubgameSelectorRecordForm::Op28Sub12);
    assert_eq!(records[4].flags, Some(0xB2));
    assert_eq!(records[5].form, SubgameSelectorRecordForm::MainOp39);
    assert_eq!(records[5].flags, None);
    assert_eq!(records[5].record_type, 0);
    assert_eq!(records[5].resource_id, 165);
    assert_eq!(records[6].form, SubgameSelectorRecordForm::MainOpC3);
    assert_eq!(records[6].record_type, 1);
    assert_eq!(records[6].resource_id, 62);
}

#[test]
fn 잘린_record머리는_selector로_반환하지_않는다() {
    assert!(scan_subgame_selector_records(&[0x28, 0x12, 0x97, 0x00, 0xAF, 0x00]).is_empty());
    assert!(scan_subgame_selector_records(&[0xC3, 0xFF, 0xFF, 0x00, 0xAF, 0x00]).is_empty());
}

#[test]
fn byte단위_후보의_정렬판정은_호출자에게_남긴다() {
    let raw = [0xFF, 0x28, 0x12, 0x97, 0x00, 0xAF, 0x00, 0x00, 0x00];

    let records = scan_subgame_selector_records(&raw);

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].offset, 1);
    assert!(!records[0].offset.is_multiple_of(4));
}
