use super::*;
use crate::isa::decode::decode;
use crate::isa::instruction::InstructionKind as K;

#[test]
fn nop_is_displayed_as_alias() {
    assert_eq!(
        Asm(K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0
        })
        .to_string(),
        "nop"
    );
}

#[test]
fn alias_display_does_not_change_encoding() {
    // move는 표시 별칭일 뿐 실체가 아니므로 재조립은 addu 그대로다.
    let kind = K::Addu {
        rd: Reg::A0,
        rs: Reg::V0,
        rt: Reg::ZERO,
    };
    assert_eq!(Asm(kind).to_string(), "move    $a0, $v0");
    assert_eq!(crate::isa::encode(kind), 0x0040_2021);
}

#[test]
fn unknown_word_is_shown_as_data() {
    assert_eq!(
        Asm(K::Unknown { word: 0xDEAD_BEEF }).to_string(),
        ".word   0xDEADBEEF"
    );
}

#[test]
fn listing_line_has_address_word_and_asm() {
    let line = listing_line(&decode(0x8001_0000, 0x03E0_0008));
    assert_eq!(line, "80010000  03E00008  jr      $ra");
}
