use super::*;
use crate::coords::exe_map;

fn exe(map: &'static ExeMap) -> Vec<u8> {
    let mut raw = vec![0u8; map.file.len as usize];
    raw[..MAGIC.len()].copy_from_slice(MAGIC);
    raw[0x10..0x14].copy_from_slice(&map.entry_pc.to_le_bytes());
    raw[0x14..0x18].copy_from_slice(&0x1234_5678u32.to_le_bytes());
    raw[0x18..0x1c].copy_from_slice(&map.load_addr.to_le_bytes());
    raw[0x1c..0x20].copy_from_slice(&map.text_len.to_le_bytes());
    raw[0x30..0x34].copy_from_slice(&0x801f_0000u32.to_le_bytes());
    raw[0x34..0x38].copy_from_slice(&0x0000_fff0u32.to_le_bytes());
    raw
}

#[test]
fn header와_자기_exe_사상을_검증한다() {
    let map = exe_map("QUEST.EXE").unwrap();
    let raw = exe(map);
    let parsed = PsxExe::parse(&raw).unwrap();

    parsed.verify_map(map).unwrap();
    assert_eq!(parsed.initial_gp(), 0x1234_5678);
    assert_eq!(parsed.initial_sp().unwrap(), 0x801f_fff0);
    assert_eq!(parsed.text().len(), map.text_len as usize);
}

#[test]
fn 다른_exe의_사상을_이식하면_실패한다() {
    let quest = exe_map("QUEST.EXE").unwrap();
    let puyo2 = exe_map("PUYO2.EXE").unwrap();
    let raw = exe(quest);
    let parsed = PsxExe::parse(&raw).unwrap();

    assert!(matches!(
        parsed.verify_map(puyo2),
        Err(PsxExeError::MapMismatch {
            exe: "PUYO2.EXE",
            ..
        })
    ));
}

#[test]
fn magic_길이_text와_stack_overflow를_거부한다() {
    assert_eq!(
        PsxExe::parse(&[0; 10]).unwrap_err(),
        PsxExeError::Truncated(10)
    );

    let map = exe_map("QUEST.EXE").unwrap();
    let mut raw = exe(map);
    raw[0] = 0;
    assert_eq!(PsxExe::parse(&raw).unwrap_err(), PsxExeError::InvalidMagic);

    let mut raw = exe(map);
    raw.pop();
    assert!(matches!(
        PsxExe::parse(&raw),
        Err(PsxExeError::FileLengthMismatch { .. })
    ));

    let mut raw = exe(map);
    raw[0x30..0x34].copy_from_slice(&u32::MAX.to_le_bytes());
    raw[0x34..0x38].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(
        PsxExe::parse(&raw).unwrap().initial_sp(),
        Err(PsxExeError::StackOverflow)
    );
}
