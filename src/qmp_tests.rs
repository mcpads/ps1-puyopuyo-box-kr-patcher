use super::*;

fn fixture(width: u32, height: u32, cells: &[u16]) -> Vec<u8> {
    let mut raw = Vec::new();
    raw.extend_from_slice(MAGIC);
    raw.extend_from_slice(&(QMP_HEADER_LEN as u32).to_le_bytes());
    raw.extend_from_slice(&width.to_le_bytes());
    raw.extend_from_slice(&height.to_le_bytes());
    for &cell in cells {
        raw.extend_from_slice(&cell.to_le_bytes());
    }
    raw
}

#[test]
fn header와_u16_tile_grid를_바이트_동일하게_왕복한다() {
    let raw = fixture(3, 2, &[0, 1, 0x8081, 0x8082, 0x8083, 9]);
    let map = QmpMap::parse(&raw).unwrap();

    assert_eq!(map.width(), 3);
    assert_eq!(map.height(), 2);
    assert_eq!(map.cells(), &[0, 1, 0x8081, 0x8082, 0x8083, 9]);
    assert_eq!(map.serialize(), raw);
}

#[test]
fn sjis처럼_보이는_바이트도_u16_le_tile_id로_보존한다() {
    let raw = fixture(3, 1, &[0x8081, 0x8082, 0x8083]);
    assert_eq!(&raw[QMP_HEADER_LEN..], b"\x81\x80\x82\x80\x83\x80");
    let map = QmpMap::parse(&raw).unwrap();
    assert_eq!(map.cells(), &[0x8081, 0x8082, 0x8083]);
}

#[test]
fn magic_header_length과_빈_크기를_거부한다() {
    assert_eq!(QmpMap::parse(&[]), Err(QmpError::Truncated(0)));

    let mut bad_magic = fixture(1, 1, &[0]);
    bad_magic[0] ^= 0xFF;
    assert_eq!(QmpMap::parse(&bad_magic), Err(QmpError::InvalidMagic));

    let mut bad_header = fixture(1, 1, &[0]);
    bad_header[8..12].copy_from_slice(&16u32.to_le_bytes());
    assert_eq!(
        QmpMap::parse(&bad_header),
        Err(QmpError::InvalidHeaderLength(16))
    );

    assert_eq!(
        QmpMap::parse(&fixture(0, 1, &[])),
        Err(QmpError::EmptyDimensions {
            width: 0,
            height: 1
        })
    );
}

#[test]
fn 선언_grid보다_짧거나_긴_payload를_거부한다() {
    let short = fixture(2, 2, &[0, 1, 2]);
    assert_eq!(
        QmpMap::parse(&short),
        Err(QmpError::LengthMismatch {
            expected: 28,
            actual: 26,
        })
    );

    let long = fixture(1, 1, &[0, 1]);
    assert_eq!(
        QmpMap::parse(&long),
        Err(QmpError::LengthMismatch {
            expected: 22,
            actual: 24,
        })
    );
}
