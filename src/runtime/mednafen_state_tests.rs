use super::*;

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

fn fixture() -> Vec<u8> {
    let mut main_ram = vec![0u8; PSX_MAIN_RAM_LEN];
    main_ram[0xDB7C0..0xDB7C4].copy_from_slice(&3u32.to_le_bytes());
    let mut main = variable(MAIN_RAM_VARIABLE, &main_ram);
    main.extend_from_slice(&variable("SysControl.Regs", &[0u8; 36]));
    let main_section = section(MAIN_SECTION, &main);
    let cpu_section = section("CPU", &variable("PC", &0x8001_0000u32.to_le_bytes()));

    let mut raw = vec![0u8; HEADER_LEN];
    raw[..MAGIC.len()].copy_from_slice(MAGIC);
    raw[16..20].copy_from_slice(&0x0010_3201u32.to_le_bytes());
    raw.extend_from_slice(&main_section);
    raw.extend_from_slice(&cpu_section);
    let len = u32::try_from(raw.len()).expect("작은 fixture");
    raw[20..24].copy_from_slice(&len.to_le_bytes());
    raw
}

#[test]
fn psx_main_ram의_세_mirror에서_같은_u32를_읽는다() {
    let raw = fixture();
    let state = MednafenPsxState::parse(&raw).expect("PSX state");
    assert_eq!(state.state_version(), 0x0010_3201);
    assert_eq!(state.section_count(), 2);
    assert_eq!(state.main_ram_bytes(), PSX_MAIN_RAM_LEN);
    assert_eq!(state.read_u32_le(0x000D_B7C0), Ok(3));
    assert_eq!(state.read_u32_le(0x800D_B7C0), Ok(3));
    assert_eq!(state.read_u32_le(0xA00D_B7C0), Ok(3));
    assert_eq!(state.read_u16_le(0x800D_B7C0), Ok(3));
    assert_eq!(state.read_u8(0x800D_B7C0), Ok(3));
    assert_eq!(state.read_bytes(0x800D_B7C0, 4), Ok(&[3, 0, 0, 0][..]));
    assert_eq!(state.read_bytes(0xA00D_B7C0, 4), Ok(&[3, 0, 0, 0][..]));
    assert!(matches!(
        state.read_bytes(0x801F_FFFF, 2),
        Err(MednafenStateError::AddressOutOfRange { .. })
    ));
}

#[test]
fn 정렬_word검색은_kseg0주소와_중복을_모두_보존한다() {
    let mut raw = fixture();
    let state = MednafenPsxState::parse(&raw).expect("PSX state");
    assert_eq!(state.find_aligned_u32_le(3), vec![0x800D_B7C0]);

    let needle = 0x1122_3344u32.to_le_bytes();
    let main_variable_start = HEADER_LEN + SECTION_HEADER_LEN + 1 + MAIN_RAM_VARIABLE.len() + 4;
    raw[main_variable_start + 0x100..main_variable_start + 0x104].copy_from_slice(&needle);
    raw[main_variable_start + 0x108..main_variable_start + 0x10C].copy_from_slice(&needle);
    let state = MednafenPsxState::parse(&raw).expect("수정 fixture");
    assert_eq!(
        state.find_aligned_u32_le(0x1122_3344),
        vec![0x8000_0100, 0x8000_0108]
    );
}

#[test]
fn state_전체길이와_main_ram_크기_drift를_거부한다() {
    let mut raw = fixture();
    let wrong_len = u32::try_from(raw.len() - 1).expect("작은 fixture");
    raw[20..24].copy_from_slice(&wrong_len.to_le_bytes());
    assert!(matches!(
        MednafenPsxState::parse(&raw),
        Err(MednafenStateError::LengthMismatch { .. })
    ));

    let main = variable(MAIN_RAM_VARIABLE, &[0u8; 4]);
    let main_section = section(MAIN_SECTION, &main);
    let mut raw = vec![0u8; HEADER_LEN];
    raw[..MAGIC.len()].copy_from_slice(MAGIC);
    raw[16..20].copy_from_slice(&0x0010_3201u32.to_le_bytes());
    raw.extend_from_slice(&main_section);
    let len = u32::try_from(raw.len()).expect("작은 fixture");
    raw[20..24].copy_from_slice(&len.to_le_bytes());
    assert_eq!(
        MednafenPsxState::parse(&raw),
        Err(MednafenStateError::WrongMainRamSize {
            expected: PSX_MAIN_RAM_LEN,
            actual: 4,
        })
    );
}

#[test]
fn section과_variable_중복을_거부한다() {
    let mut raw = fixture();
    let main_start = HEADER_LEN;
    let main_size = read_u32(&raw, main_start + 32).expect("MAIN size") as usize;
    let main_end = main_start + SECTION_HEADER_LEN + main_size;
    let duplicate = raw[main_start..main_end].to_vec();
    raw.extend_from_slice(&duplicate);
    let len = u32::try_from(raw.len()).expect("작은 fixture");
    raw[20..24].copy_from_slice(&len.to_le_bytes());
    assert_eq!(
        MednafenPsxState::parse(&raw),
        Err(MednafenStateError::DuplicateSection("MAIN".to_string()))
    );
}

#[test]
fn main_ram밖_주소와_끝을_넘는_u32를_거부한다() {
    let raw = fixture();
    let state = MednafenPsxState::parse(&raw).expect("PSX state");
    assert_eq!(
        state.read_u32_le(0x8020_0000),
        Err(MednafenStateError::AddressOutOfRange {
            runtime_address: 0x8020_0000,
        })
    );
    assert_eq!(
        state.read_u32_le(0x801F_FFFE),
        Err(MednafenStateError::AddressOutOfRange {
            runtime_address: 0x801F_FFFE,
        })
    );
    assert_eq!(
        state.read_u8(0x8020_0000),
        Err(MednafenStateError::AddressOutOfRange {
            runtime_address: 0x8020_0000,
        })
    );
}
