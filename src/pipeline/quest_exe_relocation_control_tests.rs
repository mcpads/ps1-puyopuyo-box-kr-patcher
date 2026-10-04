use super::*;

#[test]
fn extent_overlap은_즉시_실패한다() {
    let intervals = vec![(10, 20, "/A".to_string()), (19, 30, "/B".to_string())];
    assert!(count_extent_overlaps(&intervals).is_err());
}

#[test]
fn 맞닿은_extent는_겹치지_않는다() {
    let intervals = vec![(10, 20, "/A".to_string()), (20, 30, "/B".to_string())];
    assert_eq!(count_extent_overlaps(&intervals).unwrap(), 0);
}
