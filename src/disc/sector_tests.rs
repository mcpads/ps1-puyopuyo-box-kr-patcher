use super::*;

fn sample_sector() -> Vec<u8> {
    let mut raw = vec![0u8; SECTOR_LEN];
    raw[..SYNC.len()].copy_from_slice(&SYNC);
    raw[12..16].copy_from_slice(&[0x00, 0x02, 0x00, 0x02]);
    raw[16..24].copy_from_slice(&[0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08, 0x00]);
    for (index, byte) in raw[FORM1_DATA_OFFSET..FORM1_EDC_OFFSET]
        .iter_mut()
        .enumerate()
    {
        *byte = (index * 37 + 11) as u8;
    }
    raw
}

#[test]
fn 고정_vector와_같은_edc_ecc를_만든다() {
    let mut raw = sample_sector();
    regenerate_form1(&mut raw).unwrap();

    assert_eq!(
        &raw[FORM1_EDC_OFFSET..ECC_P_OFFSET],
        &[0x90, 0xf3, 0x38, 0x5e]
    );
    assert_eq!(
        &raw[ECC_P_OFFSET..ECC_P_OFFSET + 16],
        &[
            0xae, 0x9d, 0x35, 0x63, 0x08, 0x6e, 0x74, 0x46, 0x25, 0x42, 0xd8, 0x9e, 0x32, 0x92,
            0x0e, 0xa1,
        ]
    );
    assert_eq!(
        &raw[ECC_Q_OFFSET..ECC_Q_OFFSET + 16],
        &[
            0x7c, 0x09, 0x2f, 0xe7, 0x02, 0x8a, 0xb9, 0xc8, 0x7a, 0x05, 0xc6, 0x99, 0x88, 0xef,
            0xdf, 0x82,
        ]
    );
    assert_eq!(
        verified_form1_data(&raw).unwrap(),
        &sample_sector()[FORM1_DATA_OFFSET..FORM1_EDC_OFFSET]
    );
}

#[test]
fn 재생성은_idempotent이고_손상을_검출한다() {
    let mut raw = sample_sector();
    regenerate_form1(&mut raw).unwrap();
    let once = raw.clone();
    regenerate_form1(&mut raw).unwrap();
    assert_eq!(raw, once);

    raw[FORM1_DATA_OFFSET + 100] ^= 1;
    assert_eq!(
        verified_form1_data(&raw),
        Err(SectorError::Form1EdcMismatch)
    );
}

#[test]
fn 구조_오류와_form2를_거부한다() {
    assert_eq!(
        verified_form1_data(&[0; 10]),
        Err(SectorError::WrongLength {
            expected: SECTOR_LEN,
            actual: 10,
        })
    );

    let mut raw = sample_sector();
    raw[0] = 1;
    assert_eq!(regenerate_form1(&mut raw), Err(SectorError::InvalidSync));

    let mut raw = sample_sector();
    raw[MODE_OFFSET] = 1;
    assert_eq!(
        regenerate_form1(&mut raw),
        Err(SectorError::UnsupportedMode(1))
    );

    let mut raw = sample_sector();
    raw[SUBHEADER_B.start] ^= 1;
    assert_eq!(
        regenerate_form1(&mut raw),
        Err(SectorError::SubheaderMismatch)
    );

    let mut raw = sample_sector();
    raw[SUBMODE_OFFSET] |= FORM2_BIT;
    raw[SUBHEADER_B.start + 2] |= FORM2_BIT;
    assert_eq!(regenerate_form1(&mut raw), Err(SectorError::ExpectedForm1));
}

#[test]
fn form2_edc를_별도_범위로_검증한다() {
    let mut raw = sample_sector();
    raw[SUBMODE_OFFSET] |= FORM2_BIT;
    raw[SUBHEADER_B.start + 2] |= FORM2_BIT;
    for (index, byte) in raw[FORM1_DATA_OFFSET..FORM2_EDC_OFFSET]
        .iter_mut()
        .enumerate()
    {
        *byte = (index * 17 + 3) as u8;
    }
    regenerate_form2(&mut raw).unwrap();

    assert_eq!(&raw[FORM2_EDC_OFFSET..ECC_END], &[0x7e, 0xff, 0xf1, 0x11]);
    assert_eq!(verified_data(&raw).unwrap().kind, SectorKind::Form2);
    assert_eq!(verified_form2_data(&raw).unwrap().len(), FORM2_DATA_LEN);
    assert_eq!(verified_form1_data(&raw), Err(SectorError::ExpectedForm1));

    raw[FORM1_DATA_OFFSET + 100] ^= 1;
    assert_eq!(
        verified_form2_data(&raw),
        Err(SectorError::Form2EdcMismatch)
    );
}
