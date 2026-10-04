use super::*;
use crate::isa::encode::encode_all;
use crate::isa::instruction::{GteCommand, InstructionKind as K, Reg};

const P: IsaProfile = IsaProfile::DECLARED;

fn hook_stub() -> Vec<K> {
    vec![
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -16,
        },
        K::Sw {
            rt: Reg::RA,
            base: Reg::SP,
            offset: 12,
        },
        K::Jal {
            target: 0x8001_2340,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        }, // 딜레이 슬롯
        K::Lw {
            rt: Reg::RA,
            base: Reg::SP,
            offset: 12,
        },
        K::Jr { rs: Reg::RA },
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: 16,
        },
    ]
}

#[test]
fn accepts_verified_stub() {
    let bytes = encode_all(&hook_stub());
    let v = verify_bytes(P, 0x8001_0000, &bytes).expect("검산 통과해야 함");
    assert_eq!(v.len_words(), 7);
    assert_eq!(v.bytes(), bytes.as_slice());
    assert_eq!(v.base_addr(), 0x8001_0000);
    assert_eq!(v.profile(), P);
    assert_eq!(v.typed_source_id(), None);
    assert_eq!(v.checked_profile_id(), None);
}

#[test]
fn typed_source만_제품기계어_source_id를_가진다() {
    let source = hook_stub();
    let v = verify_typed_source(P, 0x8001_0000, "hooks/quest-font-v1", &source)
        .expect("typed source 검산 통과");
    assert_eq!(v.bytes(), encode_all(&source));
    assert_eq!(v.typed_source_id(), Some("hooks/quest-font-v1"));
    assert_eq!(v.checked_profile_id(), Some(psx_r3000a::PROFILE_ID));
    assert_eq!(
        verify_typed_source(P, 0x8001_0000, " ", &source),
        Err(VerifyError::EmptySourceId)
    );
}

#[test]
fn typed_source의_분기_immediate를_배치_target으로_옮긴다() {
    let source = [
        K::Beq {
            rs: Reg::T0,
            rt: Reg::T1,
            offset: 1,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 1,
        },
    ];
    let verified = verify_typed_source(P, 0x8001_0000, "tests/placed-branch", &source)
        .expect("배치 분기 검산");

    assert_eq!(verified.bytes(), encode_all(&source));
    assert_eq!(verified.instructions()[0].kind, source[0]);
}

#[test]
fn typed_source는_delay_slot_없는_제어흐름을_거부한다() {
    assert_eq!(
        verify_typed_source(
            P,
            0x8001_0000,
            "tests/missing-delay-slot",
            &[K::Jr { rs: Reg::RA }]
        ),
        Err(VerifyError::CheckedAssembly {
            profile: psx_r3000a::PROFILE_ID,
            source: psx_r3000a::AssemblerError::MissingDelaySlot {
                address: 0x8001_0000,
            },
        })
    );
}

#[test]
fn typed_source는_분석용_unknown을_거부한다() {
    let instruction = K::Unknown { word: 0xFFFF_FFFF };
    assert_eq!(
        verify_typed_source(P, 0x8001_0000, "tests/unknown", &[instruction]),
        Err(VerifyError::TypedSourceOutsideProfile {
            profile: psx_r3000a::PROFILE_ID,
            index: 0,
            instruction,
        })
    );
}

#[test]
fn typed_source의_gte_raw와_command가_같아야_한다() {
    let raw = crate::isa::encode_gte(GteCommand::Rtps, 0).unwrap();
    let valid = K::Gte {
        command: GteCommand::Rtps,
        raw,
    };
    let verified = verify_typed_source(P, 0x8001_0000, "tests/gte", &[valid]).unwrap();
    assert_eq!(verified.bytes(), raw.to_le_bytes());

    let mismatched = K::Gte {
        command: GteCommand::Nclip,
        raw,
    };
    assert!(matches!(
        verify_typed_source(P, 0x8001_0000, "tests/gte-mismatch", &[mismatched]),
        Err(VerifyError::TypedSourceOutsideProfile { index: 0, .. })
    ));
}

#[test]
fn rejects_empty_input() {
    assert_eq!(verify_bytes(P, 0x8001_0000, &[]), Err(VerifyError::Empty));
}

#[test]
fn rejects_unaligned_input() {
    assert_eq!(
        verify_bytes(P, 0x8001_0000, &[0, 0, 0]),
        Err(VerifyError::Unaligned { len: 3 })
    );
}

#[test]
fn rejects_word_outside_profile() {
    // op=0x3F는 프로필에 없다.
    let word = 0xFFFF_FFFFu32;
    let err = verify_bytes(P, 0x8001_0000, &word.to_le_bytes()).unwrap_err();
    match err {
        VerifyError::OutsideProfile { addr, word: w, .. } => {
            assert_eq!(addr, 0x8001_0000);
            assert_eq!(w, word);
        }
        other => panic!("프로필 밖 판정이 아님: {other:?}"),
    }
}

#[test]
fn rejects_unknown_word_in_the_middle_of_valid_code() {
    let mut bytes = encode_all(&hook_stub());
    // 4번째 워드를 프로필 밖 값으로 오염시킨다.
    bytes[12..16].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    let err = verify_bytes(P, 0x8001_0000, &bytes).unwrap_err();
    assert!(matches!(
        err,
        VerifyError::OutsideProfile {
            addr: 0x8001_000C,
            ..
        }
    ));
}

#[test]
fn rejects_data_masquerading_as_code() {
    // 문자열 바이트열은 프로필 안에서 연속 해석되지 않아야 한다.
    let data = b"KRPATCH\0";
    assert!(verify_bytes(P, 0x8001_0000, data).is_err());
}

#[test]
fn verified_instructions_expose_delay_slot_requirement() {
    let bytes = encode_all(&hook_stub());
    let v = verify_bytes(P, 0x8001_0000, &bytes).unwrap();
    // jal은 딜레이 슬롯을 요구한다 — 훅 이동 검산의 전제.
    let jal = &v.instructions()[2];
    assert!(jal.flow_kind().has_delay_slot());
    // jr $ra는 리턴으로 분류된다.
    assert!(matches!(
        v.instructions()[5].flow_kind(),
        crate::isa::FlowKind::Return
    ));
}
