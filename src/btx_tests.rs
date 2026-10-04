use super::*;

#[test]
fn cnx_subtype이_bpp를_구분한다() {
    assert_eq!(
        PixelFormat::from_cnx_subtype(*b"bi_"),
        Ok(PixelFormat::Indexed4)
    );
    assert_eq!(
        PixelFormat::from_cnx_subtype(*b"bit"),
        Ok(PixelFormat::Indexed8)
    );
    assert_eq!(
        PixelFormat::from_cnx_subtype(*b"bin"),
        Err(BtxError::UnsupportedCnxSubtype(*b"bin"))
    );
}

#[test]
fn indexed4는_low_nibble을_왼쪽에_둔다() {
    let layout = Layout {
        width: 4,
        height: 1,
        format: PixelFormat::Indexed4,
    };
    assert_eq!(decode(&[0x12, 0xAB], layout), Ok(vec![2, 1, 11, 10]));
}

#[test]
fn indexed4는_바이트_동일하게_왕복한다() {
    let original = [0x12, 0x34, 0xAB, 0xCD];
    let layout = Layout {
        width: 4,
        height: 2,
        format: PixelFormat::Indexed4,
    };
    let pixels = decode(&original, layout).unwrap();
    assert_eq!(encode(&pixels, layout), Ok(original.to_vec()));
}

#[test]
fn indexed8은_바이트_동일하게_왕복한다() {
    let original = [0, 1, 127, 255];
    let layout = Layout {
        width: 2,
        height: 2,
        format: PixelFormat::Indexed8,
    };
    let pixels = decode(&original, layout).unwrap();
    assert_eq!(pixels, original);
    assert_eq!(encode(&pixels, layout), Ok(original.to_vec()));
}

#[test]
fn 입력_크기가_layout보다_작거나_크면_실패한다() {
    let layout = Layout {
        width: 4,
        height: 1,
        format: PixelFormat::Indexed4,
    };
    assert_eq!(
        decode(&[0x12], layout),
        Err(BtxError::DataLengthMismatch {
            expected: 2,
            actual: 1,
        })
    );
    assert_eq!(
        decode(&[0x12, 0x34, 0x56], layout),
        Err(BtxError::DataLengthMismatch {
            expected: 2,
            actual: 3,
        })
    );
}

#[test]
fn 홀수_4bpp_폭과_범위_밖_인덱스를_거부한다() {
    let odd = Layout {
        width: 3,
        height: 2,
        format: PixelFormat::Indexed4,
    };
    assert_eq!(odd.byte_len(), Err(BtxError::OddIndexed4Width(3)));

    let layout = Layout {
        width: 2,
        height: 1,
        format: PixelFormat::Indexed4,
    };
    assert_eq!(
        encode(&[1, 16], layout),
        Err(BtxError::PaletteIndexOutOfRange {
            offset: 1,
            value: 16,
        })
    );
}
