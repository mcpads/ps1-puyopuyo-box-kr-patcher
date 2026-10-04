use super::*;

#[test]
fn 선택_행은_group과_terminator와_zero_tail을_구분한다() {
    let values = [0i32, 1, -1, 2, 3, -1, -999, 0, 0, 0];
    let mut raw = vec![0u8; SELECTION_ROW_BYTES];
    for (index, value) in values.into_iter().enumerate() {
        raw[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    assert_eq!(
        parse_selection_row(&raw, 4),
        Ok(vec![vec![0, 1], vec![2, 3]])
    );
}

#[test]
fn 선택_행의_범위와_tail_drift를_거부한다() {
    let mut out_of_range = vec![0u8; SELECTION_ROW_BYTES];
    out_of_range[..4].copy_from_slice(&4i32.to_le_bytes());
    out_of_range[4..8].copy_from_slice(&(-1i32).to_le_bytes());
    out_of_range[8..12].copy_from_slice(&(-999i32).to_le_bytes());
    assert!(parse_selection_row(&out_of_range, 4).is_err());

    let mut tail = vec![0u8; SELECTION_ROW_BYTES];
    tail[..4].copy_from_slice(&0i32.to_le_bytes());
    tail[4..8].copy_from_slice(&(-1i32).to_le_bytes());
    tail[8..12].copy_from_slice(&(-999i32).to_le_bytes());
    tail[12..16].copy_from_slice(&1i32.to_le_bytes());
    assert!(parse_selection_row(&tail, 4).is_err());
}
