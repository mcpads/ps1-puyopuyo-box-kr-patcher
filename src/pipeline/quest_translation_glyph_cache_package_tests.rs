use super::*;

#[test]
fn package_digest형식은_소문자sha256만_허용한다() {
    assert!(is_sha256_hex(&"a".repeat(64)));
    assert!(!is_sha256_hex(&"A".repeat(64)));
    assert!(!is_sha256_hex(&"g".repeat(64)));
    assert!(!is_sha256_hex(&"a".repeat(63)));
}
