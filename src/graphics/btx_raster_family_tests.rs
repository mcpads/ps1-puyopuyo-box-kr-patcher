use super::*;

#[test]
fn 미등록자산을조용히family에넣지않는다() {
    let error = proposed_family_for_asset("UNKNOWN.BTX").unwrap_err();
    assert!(error.to_string().contains("semantic family가 없는"));
}
