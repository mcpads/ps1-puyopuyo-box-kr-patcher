use crate::coords::LogicalFile;

use super::*;

fn quest_map() -> ExeMap {
    ExeMap {
        name: "QUEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x000A_E800,
        entry_pc: 0x800A_A6E8,
        file: LogicalFile::new(33_538, 716_800),
    }
}

#[test]
fn 최대_provisional_page는_504_sector_exe를_요구한다() {
    let (text_bytes, file_bytes, sectors) = required_sizes(&quest_map(), 10_582).unwrap();
    assert_eq!(text_bytes, 0x000F_B800);
    assert_eq!(file_bytes, 1_032_192);
    assert_eq!(sectors, 504);
}

#[test]
fn 최소_provisional_page도_원본보다_150_sector_늘어난다() {
    let (_, file_bytes, sectors) = required_sizes(&quest_map(), 3_052).unwrap();
    assert_eq!(file_bytes, 1_024_000);
    assert_eq!(sectors, 500);
    assert_eq!(sectors - quest_map().file.sector_count(), 150);
}

#[test]
fn 원본은_350_sector다() {
    assert_eq!(quest_map().file.sector_count(), 350);
}

#[test]
fn relocation_control_크기는_source_gate_상수와_같다() {
    assert_eq!(QUEST_RELOCATION_CONTROL_TEXT_BYTES, 0x000F_B800);
    assert_eq!(QUEST_RELOCATION_CONTROL_FILE_BYTES, 1_032_192);
    assert_eq!(QUEST_RELOCATION_CONTROL_SECTORS, 504);
}
