use super::*;

#[test]
fn 지원_분류는_literal_15개를_중복없이_닫는다() {
    verify_partition(&LITERAL_COMBINES, &RESOURCE_X36_SELECTORS).unwrap();
}

#[test]
fn 미분류_literal이나_중복은_거부한다() {
    let mut missing = LITERAL_COMBINES.to_vec();
    missing.push(0x8006_0000);
    assert!(verify_partition(&missing, &RESOURCE_X36_SELECTORS).is_err());

    let mut duplicated = LITERAL_COMBINES.to_vec();
    duplicated.push(LITERAL_COMBINES[0]);
    assert!(verify_partition(&duplicated, &RESOURCE_X36_SELECTORS).is_err());
}
