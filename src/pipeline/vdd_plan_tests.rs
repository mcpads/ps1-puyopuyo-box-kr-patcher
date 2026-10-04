use super::*;

#[test]
fn 동일한_논리_span은_expected_write로_세지_않는다() {
    let mut plan = WritePlan::new();
    add_logical_span(
        &mut plan,
        LogicalFile::new(20, 2048),
        0,
        &[1, 2, 3],
        &[1, 2, 3],
        "same",
        "test",
        "동일 byte",
        WriteIntent::Data,
    )
    .unwrap();

    assert!(plan.regions.is_empty());
    assert!(plan.writes.is_empty());
}
