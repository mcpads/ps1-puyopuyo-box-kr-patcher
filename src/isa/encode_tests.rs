use super::*;
use crate::isa::decode::decode_word;
use crate::isa::instruction::{GteCommand, InstructionKind as K, Reg};

/// 결정적 의사난수. 외부 의존 없이 넓은 워드 공간을 훑는다.
fn lcg(seed: &mut u64) -> u32 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*seed >> 32) as u32
}

#[test]
fn decode_encode_is_identity_over_random_words() {
    let mut seed = 0x0BAD_C0DE_DEAD_BEEF;
    for _ in 0..300_000 {
        let word = lcg(&mut seed);
        let kind = decode_word(0x8001_0000, word);
        assert_eq!(
            encode(kind),
            word,
            "재조립 불일치: word=0x{word:08X} kind={kind:?}"
        );
    }
}

#[test]
fn decode_encode_is_identity_over_special_opcode_space() {
    // op=0 (SPECIAL) 전 funct × 대표 레지스터 조합
    for funct in 0u32..64 {
        for rs in [0u32, 1, 8, 31] {
            for rt in [0u32, 2, 9, 30] {
                for rd in [0u32, 3, 16, 29] {
                    for sa in [0u32, 1, 31] {
                        let word = (rs << 21) | (rt << 16) | (rd << 11) | (sa << 6) | funct;
                        let kind = decode_word(0x8001_0000, word);
                        assert_eq!(encode(kind), word, "SPECIAL word=0x{word:08X}");
                    }
                }
            }
        }
    }
}

#[test]
fn decode_encode_is_identity_over_primary_opcode_space() {
    for op in 0u32..64 {
        for rs in [0u32, 1, 29] {
            for rt in [0u32, 1, 16, 17, 31] {
                for imm in [0u32, 1, 0x7FFF, 0x8000, 0xFFFF] {
                    let word = (op << 26) | (rs << 21) | (rt << 16) | imm;
                    let kind = decode_word(0x8001_0000, word);
                    assert_eq!(encode(kind), word, "op=0x{op:02X} word=0x{word:08X}");
                }
            }
        }
    }
}

#[test]
fn encodes_known_words() {
    assert_eq!(
        encode(K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -32
        }),
        0x27BD_FFE0
    );
    assert_eq!(encode(K::Jr { rs: Reg::RA }), 0x03E0_0008);
    assert_eq!(
        encode(K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0
        }),
        0
    );
    assert_eq!(encode(K::Rfe), 0x4200_0010);
}

#[test]
fn encode_all_emits_little_endian() {
    let bytes = encode_all(&[K::Jr { rs: Reg::RA }]);
    assert_eq!(bytes, vec![0x08, 0x00, 0xE0, 0x03]);
}

#[test]
fn encode_gte_roundtrips_through_decode() {
    for cmd in [
        GteCommand::Rtps,
        GteCommand::Nclip,
        GteCommand::Mvmva,
        GteCommand::Rtpt,
        GteCommand::Ncct,
    ] {
        let word = encode_gte(cmd, 0).expect("알려진 커맨드");
        match decode_word(0x8001_0000, word) {
            K::Gte { command, raw } => {
                assert_eq!(command, cmd);
                assert_eq!(raw, word);
            }
            other => panic!("GTE가 아님: {other:?}"),
        }
    }
}

#[test]
fn encode_gte_rejects_unknown_command() {
    assert!(encode_gte(GteCommand::Unknown(0x02), 0).is_none());
}
