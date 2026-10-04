use super::*;
use crate::coords::{ExeMap, LogicalFile};
use crate::isa::{encode_all, InstructionKind as K, Reg};

fn test_map(text_len: u32) -> ExeMap {
    ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(100, 0x800 + text_len),
    }
}

#[test]
fn sweep_assigns_load_relative_addresses() {
    let text = encode_all(&[
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -16,
        },
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let map = test_map(text.len() as u32);
    let out = sweep(&map, &text).expect("정렬된 텍스트");
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].addr, 0x8001_0000);
    assert_eq!(out[2].addr, 0x8001_0008);
}

#[test]
fn sweep_rejects_unaligned_text() {
    let map = test_map(3);
    assert!(sweep(&map, &[0, 0, 0]).is_none());
}

#[test]
fn find_selects_matching_instructions() {
    let text = encode_all(&[
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Jr { rs: Reg::T2 },
    ]);
    let map = test_map(text.len() as u32);
    let out = sweep(&map, &text).unwrap();
    let jrs = find(&out, |i| matches!(i.kind, K::Jr { .. }));
    assert_eq!(jrs.len(), 2);
}

#[test]
fn unknown_ratio_flags_data_as_probably_not_code() {
    // 텍스트 문자열은 코드로 해석되지 않는 워드를 많이 만든다.
    let data = b"\xff\xff\xff\xff\xff\xff\xff\xff";
    let map = test_map(data.len() as u32);
    let out = sweep(&map, data).unwrap();
    assert!(unknown_ratio(&out) > 0.9);

    let code = encode_all(&[K::Jr { rs: Reg::RA }]);
    let map = test_map(code.len() as u32);
    assert_eq!(unknown_ratio(&sweep(&map, &code).unwrap()), 0.0);
}
