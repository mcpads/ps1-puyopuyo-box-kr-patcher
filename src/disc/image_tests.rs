use super::*;
use crate::disc::sector::regenerate_form1;
use crate::source::FORM1_DATA_OFFSET;

fn valid_sector() -> Vec<u8> {
    let mut raw = vec![0u8; SECTOR_LEN];
    raw[..12].copy_from_slice(&[
        0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00,
    ]);
    raw[15] = 2;
    raw[16..24].copy_from_slice(&[0, 0, 8, 0, 0, 0, 8, 0]);
    raw[FORM1_DATA_OFFSET] = 0x5a;
    regenerate_form1(&mut raw).unwrap();
    raw
}

#[test]
fn 정렬된_이미지는_lba와_form1_data를_검증한다() {
    let mut bytes = valid_sector();
    bytes.extend_from_slice(&valid_sector());
    let image = DiscImage::from_bytes(bytes).unwrap();

    assert_eq!(image.sector_count(), 2);
    assert_eq!(image.form1_data(1).unwrap()[0], 0x5a);
    assert_eq!(
        image.verify_all_sectors().unwrap(),
        SectorCounts { form1: 2, form2: 0 }
    );
    assert!(matches!(
        image.raw_sector(2),
        Err(ImageError::LbaOutOfRange { lba: 2, sectors: 2 })
    ));
}

#[test]
fn 부분_섹터와_무결성_손상을_거부한다() {
    assert!(matches!(
        DiscImage::from_bytes(vec![0; SECTOR_LEN - 1]),
        Err(ImageError::UnalignedLength(_))
    ));

    let mut raw = valid_sector();
    raw[FORM1_DATA_OFFSET] ^= 1;
    let image = DiscImage::from_bytes(raw).unwrap();
    assert!(matches!(
        image.form1_data(0),
        Err(ImageError::Sector {
            source: SectorError::Form1EdcMismatch,
            ..
        })
    ));
}
