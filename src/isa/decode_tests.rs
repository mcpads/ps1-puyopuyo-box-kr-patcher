use super::*;
use crate::isa::instruction::{GteCommand, InstructionKind as K, Reg};

#[test]
fn decodes_nop_as_sll_zero() {
    assert_eq!(
        decode_word(0x8001_0000, 0x0000_0000),
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0
        }
    );
}

#[test]
fn decodes_common_prologue() {
    // addiu $sp, $sp, -32
    assert_eq!(
        decode_word(0x8001_0000, 0x27BD_FFE0),
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -32
        }
    );
    // sw $ra, 28($sp)
    assert_eq!(
        decode_word(0x8001_0004, 0xAFBF_001C),
        K::Sw {
            rt: Reg::RA,
            base: Reg::SP,
            offset: 28
        }
    );
}

#[test]
fn decodes_jr_ra() {
    assert_eq!(decode_word(0x8001_0000, 0x03E0_0008), K::Jr { rs: Reg::RA });
}

#[test]
fn jump_target_takes_high_nibble_from_next_address() {
    // j 0x80010080 at 0x8000FFFC — 상위 4비트는 다음 명령 주소에서 온다.
    let target = match decode_word(0x8000_FFFC, 0x0800_0020) {
        K::J { target } => target,
        other => panic!("j가 아님: {other:?}"),
    };
    assert_eq!(target, 0x8000_0080);
}

#[test]
fn decodes_lui_ori_pair() {
    assert_eq!(
        decode_word(0x8001_0000, 0x3C08_8001),
        K::Lui {
            rt: Reg::T0,
            imm: 0x8001
        }
    );
    assert_eq!(
        decode_word(0x8001_0004, 0x3508_1234),
        K::Ori {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 0x1234
        }
    );
}

#[test]
fn decodes_bios_call_stub_shape() {
    // li $t2, 0xB0 ; jr $t2 ; li $t1, 0x51  (Krom2RawAdd 스텁 형태)
    assert_eq!(
        decode_word(0x8001_0000, 0x3C0A_0000),
        K::Lui {
            rt: Reg::T2,
            imm: 0
        }
    );
    assert_eq!(decode_word(0x8001_0008, 0x0140_0008), K::Jr { rs: Reg::T2 });
    assert_eq!(
        decode_word(0x8001_000C, 0x2409_0051),
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: 0x51
        }
    );
}

#[test]
fn decodes_rfe() {
    assert_eq!(decode_word(0x8001_0000, 0x4200_0010), K::Rfe);
}

#[test]
fn decodes_gte_command_preserving_raw() {
    // rtps: op=0x12, CO 비트, funct=0x01
    let word = (0x12u32 << 26) | (1 << 25) | 0x01;
    assert_eq!(
        decode_word(0x8001_0000, word),
        K::Gte {
            command: GteCommand::Rtps,
            raw: word
        }
    );
}

#[test]
fn unknown_gte_funct_stays_as_raw_unknown_word() {
    let word = (0x12u32 << 26) | (1 << 25) | 0x02; // 프로필에 없는 funct
    assert_eq!(decode_word(0x8001_0000, word), K::Unknown { word });
}

#[test]
fn reserved_field_nonzero_falls_back_to_unknown() {
    // add에 sa 비트가 서 있으면 프로필 밖으로 남긴다.
    let add = 0x0000_0020u32 | (1 << 6);
    assert_eq!(decode_word(0x8001_0000, add), K::Unknown { word: add });
}

#[test]
fn decode_slice_rejects_unaligned_input() {
    assert!(decode_slice(0x8001_0000, &[0, 0, 0]).is_none());
    assert!(decode_slice(0x8001_0002, &[0; 4]).is_none());
    assert!(decode_slice(0xFFFF_FFFC, &[0; 8]).is_none());
}

#[test]
fn decode_slice_assigns_sequential_addresses() {
    let bytes = [0u8; 12];
    let out = decode_slice(0x8001_0000, &bytes).expect("정렬된 입력");
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].addr, 0x8001_0000);
    assert_eq!(out[1].addr, 0x8001_0004);
    assert_eq!(out[2].addr, 0x8001_0008);
}
