use super::*;

#[test]
fn title_remainders_do_not_authorize_neighboring_blocks_or_copyright() {
    assert!(writable("TITLEA.BTX", 175, 60));
    assert!(writable("TITLEA.BTX", 168, 47));
    assert!(!writable("TITLEA.BTX", 168, 23));
    assert!(!writable("TITLEA.BTX", 176, 47));
    assert!(writable("TITLEA.BTX", 180, 60));
    assert!(!writable("TITLEA.BTX", 181, 60));
    assert!(!writable("TITLEA.BTX", 176, 50));
    assert!(!writable("TITLEA.BTX", 80, 160));
    assert!(writable("P2_OP13.BTX", 87, 119));
    assert!(!writable("P2_OP13.BTX", 88, 48));
    assert!(!writable("P2_OP13.BTX", 112, 0));
}

#[test]
fn malformed_or_changed_generated_input_is_rejected() {
    let digest = format!("{:x}", Sha256::digest([0u8, 15]));
    assert_eq!(pixels("000f\n", 2, &digest).unwrap(), [0, 15]);
    assert!(pixels("000e", 2, &digest).is_err());
    assert!(pixels("000f00", 2, &digest).is_err());
    assert!(pixels("zz0f", 2, &digest).is_err());
}
