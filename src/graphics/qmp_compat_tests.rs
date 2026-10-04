use super::QmpMap;

#[test]
fn 기존_graphics_qmp_공개경로는_중립qmp_type을_노출한다() {
    let mut raw = b"PBXQMAP\0".to_vec();
    raw.extend_from_slice(&20u32.to_le_bytes());
    raw.extend_from_slice(&1u32.to_le_bytes());
    raw.extend_from_slice(&1u32.to_le_bytes());
    raw.extend_from_slice(&7u16.to_le_bytes());
    let map: crate::qmp::QmpMap = QmpMap::parse(&raw).unwrap();
    assert_eq!(map.cells(), [7]);
}
