use super::*;

#[test]
fn 간격_교체는_같은_길이의_검산된_basic_block이다() {
    let original = original_code().unwrap();
    let replacement = replacement_code().unwrap();
    assert_eq!(original.bytes().len(), 48);
    assert_eq!(replacement.bytes().len(), 48);
    assert_ne!(original.bytes(), replacement.bytes());
    crate::isa::verify_bytes(IsaProfile::DECLARED, START, replacement.bytes()).unwrap();
}
