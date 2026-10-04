use super::*;

fn fixture() -> (ExeMap, Vec<u8>) {
    let map = *crate::coords::exe_map("QUEST.EXE").unwrap();
    let mut text = vec![0; map.text_len as usize];
    let start = map.text_offset(SITE_RUNTIME).unwrap();
    // j 0x8002b8cc / nop의 불변 원본.
    text[start..start + 8].copy_from_slice(&[0x33, 0xae, 0x00, 0x08, 0, 0, 0, 0]);
    (map, text)
}

#[test]
fn sprt의_ot_연결만_대체하고_delay_slot을_보존한다() {
    let (map, text) = fixture();
    let code = assemble(&map, &text, 0x8010_0000).unwrap();
    assert_eq!(code.bytes(), &[0x00, 0x00, 0x04, 0x08, 0, 0, 0, 0]);
}

#[test]
fn 다른_분기나_delay_slot이면_설치하지_않는다() {
    let (map, text) = fixture();
    let start = map.text_offset(SITE_RUNTIME).unwrap();
    for at in [start, start + 4] {
        let mut changed = text.clone();
        changed[at] ^= 1;
        assert!(assemble(&map, &changed, 0x8010_0000).is_err());
    }
}
