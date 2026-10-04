use super::*;

#[test]
fn stream은_고정_span만_바꾼다() {
    let mut original = vec![0xAA; MENU06_STREAM_END_OFFSET + 32];
    let source = vec![0x11; MENU06_STREAM_END_OFFSET - MENU06_STREAM_OFFSET];
    original[MENU06_STREAM_OFFSET..MENU06_STREAM_END_OFFSET].copy_from_slice(&source);
    let replacement = vec![0x22; source.len()];
    let output = replace_stream(&original, &source, &replacement).unwrap();
    assert_eq!(
        &output[MENU06_STREAM_OFFSET..MENU06_STREAM_END_OFFSET],
        replacement
    );
    assert_eq!(
        &output[..MENU06_STREAM_OFFSET],
        &original[..MENU06_STREAM_OFFSET]
    );
    assert_eq!(
        &output[MENU06_STREAM_END_OFFSET..],
        &original[MENU06_STREAM_END_OFFSET..]
    );
}

#[test]
fn source_drift와_길이변경을_거부한다() {
    let original = vec![0; MENU06_STREAM_END_OFFSET + 1];
    let span = MENU06_STREAM_END_OFFSET - MENU06_STREAM_OFFSET;
    assert!(replace_stream(&original, &vec![1; span], &vec![0; span]).is_err());
    assert!(replace_stream(&original, &vec![0; span], &vec![0; span - 1]).is_err());
}
