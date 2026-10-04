use super::*;

#[test]
fn extent_겹침은_맞닿은_경계를_겹침으로_세지_않는다() {
    assert!(ranges_overlap(10, 20, 15, 25));
    assert!(ranges_overlap(10, 20, 9, 11));
    assert!(!ranges_overlap(10, 20, 20, 30));
    assert!(!ranges_overlap(10, 20, 0, 10));
}

#[test]
fn direct_filename_literal을_겹치지_않게_센다() {
    assert_eq!(count_occurrences(b"32M.BIN\0xx32M.BIN;1", b"32M.BIN"), 2);
    assert_eq!(count_occurrences(b"32M.BI", b"32M.BIN"), 0);
    assert_eq!(count_occurrences(b"anything", b""), 0);
}
