use super::*;

const HEADER_LEN: usize = 32;
const PSX_MAIN_RAM_LEN: usize = 2 * 1024 * 1024;

fn variable(name: &str, value: &[u8]) -> Vec<u8> {
    let mut raw = Vec::new();
    raw.push(u8::try_from(name.len()).expect("짧은 이름"));
    raw.extend_from_slice(name.as_bytes());
    raw.extend_from_slice(&u32::try_from(value.len()).expect("작은 값").to_le_bytes());
    raw.extend_from_slice(value);
    raw
}

fn section(name: &str, data: &[u8]) -> Vec<u8> {
    let mut raw = vec![0u8; 32];
    raw[..name.len()].copy_from_slice(name.as_bytes());
    raw.extend_from_slice(
        &u32::try_from(data.len())
            .expect("작은 section")
            .to_le_bytes(),
    );
    raw.extend_from_slice(data);
    raw
}

fn state_with_main_ram(main_ram: &[u8]) -> Vec<u8> {
    let main = variable("MainRAM.data8", main_ram);
    let main_section = section("MAIN", &main);
    let mut raw = vec![0u8; HEADER_LEN];
    raw[..8].copy_from_slice(b"MDFNSVST");
    raw[16..20].copy_from_slice(&0x0010_3201u32.to_le_bytes());
    raw.extend_from_slice(&main_section);
    let len = u32::try_from(raw.len()).expect("작은 fixture");
    raw[20..24].copy_from_slice(&len.to_le_bytes());
    raw
}

#[test]
fn 상주_range는_exact_bytes와_미사용_zero경계를_모두_요구한다() {
    let mut main_ram = vec![0u8; PSX_MAIN_RAM_LEN];
    main_ram[0x100..0x104].copy_from_slice(&[1, 2, 3, 4]);
    let raw = state_with_main_ram(&main_ram);
    let state = MednafenPsxState::parse(&raw).expect("PSX state");

    assert_eq!(
        exact_state_range(&state, 0x8000_0100, &[1, 2, 3, 4], "test").expect("exact"),
        digest(&[1, 2, 3, 4])
    );
    zero_state_range(&state, 0x8000_0200, 8, "suffix").expect("zero suffix");

    let exact_error = exact_state_range(&state, 0x8000_0100, &[1, 2, 3, 5], "test")
        .expect_err("한 byte drift를 거부해야 함");
    let exact_error = exact_error.to_string();
    assert!(exact_error.contains("first_offset=0x3"));
    assert!(exact_error.contains("expected=0x05"));
    assert!(exact_error.contains("actual=0x04"));
    assert!(exact_error.contains("mismatch_bytes=1"));

    main_ram[0x203] = 1;
    let raw = state_with_main_ram(&main_ram);
    let state = MednafenPsxState::parse(&raw).expect("수정 PSX state");
    let zero_error = zero_state_range(&state, 0x8000_0200, 8, "suffix")
        .expect_err("미사용 영역 drift를 거부해야 함");
    assert!(zero_error.to_string().contains("offset=0x3"));
}

#[test]
fn 화면식별_png는_정확한_ihdr크기만_받는다() {
    let mut png = vec![0u8; 24];
    png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    png[12..16].copy_from_slice(b"IHDR");
    png[16..20].copy_from_slice(&PNG_WIDTH.to_be_bytes());
    png[20..24].copy_from_slice(&PNG_HEIGHT.to_be_bytes());
    assert_eq!(png_dimensions(&png), Some((PNG_WIDTH, PNG_HEIGHT)));
    png[20..24].copy_from_slice(&239u32.to_be_bytes());
    assert_eq!(png_dimensions(&png), Some((PNG_WIDTH, 239)));
    assert_eq!(png_dimensions(b"not png"), None);
}

#[test]
fn 상주_scratch는_package_layout에서_받은_범위만_바꾼다() {
    let mut package = vec![0xAA; 12];
    replace_resident_scratch(&mut package, 5, 3, &[1, 2, 3]).expect("동적 scratch 교체");
    assert_eq!(
        package,
        [0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 1, 2, 3, 0xAA, 0xAA, 0xAA, 0xAA]
    );

    let error = replace_resident_scratch(&mut package, 5, 4, &[1, 2, 3])
        .expect_err("scratch와 cell 길이가 다르면 거부해야 함");
    assert!(error.to_string().contains("길이 불일치"));
}
