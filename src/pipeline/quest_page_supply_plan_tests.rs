use super::*;

#[test]
fn carrier_용량_차이는_부호를_보존한다() {
    assert_eq!(remaining(67_592, 65_536).unwrap(), 2_056);
    assert_eq!(remaining(67_592, 98_304).unwrap(), -30_712);
    assert_eq!(remaining(67_592, 96_224).unwrap(), -28_632);
    assert_eq!(remaining(67_592, 66_186).unwrap(), 1_406);
}

#[test]
fn 한글_고유_집합만_집계한다() {
    assert_eq!(unique_hangul("드라코「ABC 드라코!").len(), 3);
    assert!(unique_hangul("「ABC!」").is_empty());
}
