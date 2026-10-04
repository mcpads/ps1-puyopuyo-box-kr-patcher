use super::*;

#[test]
fn glyph_slot은_21열_12x12_좌표로_계산한다() {
    assert_eq!(glyph_rect(0).unwrap(), (0, 0, 12, 12));
    assert_eq!(glyph_rect(20).unwrap(), (240, 0, 12, 12));
    assert_eq!(glyph_rect(21).unwrap(), (0, 12, 12, 12));
    assert_eq!(glyph_rect(250).unwrap(), (228, 132, 12, 12));
    assert!(glyph_rect(252).is_err());
}

#[test]
fn code_table은_중복_code나_문자를_허용하지_않는다() {
    let mut raw = Vec::with_capacity(GLYPH_CODE_TABLE_BYTES);
    for index in 0..GLYPH_MAPPINGS {
        let code = 0x20u16 + u16::try_from(index).unwrap();
        raw.extend_from_slice(&code.to_le_bytes());
    }
    assert!(decode_code_table(&raw).is_err());

    let duplicated = [0x82, 0xA0].repeat(GLYPH_MAPPINGS);
    assert!(decode_code_table(&duplicated).is_err());
}

#[test]
fn 주소분할은_addiu의_부호확장을_보존한다() {
    assert_eq!(split_address(0x8009_4E24), (0x8009, 0x4E24));
    assert_eq!(split_address(0x8007_B0D4), (0x8008, -0x4F2C));
}
