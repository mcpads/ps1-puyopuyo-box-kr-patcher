use super::*;
use crate::disc::sector::regenerate_form1;
use crate::patch::WritePlan;

fn form1_sector(seed: u8) -> Vec<u8> {
    let mut raw = vec![0u8; SECTOR_LEN];
    raw[..12].copy_from_slice(&[
        0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00,
    ]);
    raw[15] = 2;
    raw[16..24].copy_from_slice(&[0, 0, 8, 0, 0, 0, 8, 0]);
    for (index, byte) in raw[FORM1_DATA_OFFSET..FORM1_DATA_OFFSET + FORM1_DATA_LEN]
        .iter_mut()
        .enumerate()
    {
        *byte = seed.wrapping_add(index as u8);
    }
    regenerate_form1(&mut raw).unwrap();
    raw
}

fn image() -> DiscImage {
    let mut raw = form1_sector(3);
    raw.extend(form1_sector(17));
    DiscImage::from_bytes(raw).unwrap()
}

#[test]
fn 데이터와_edc_ecc를_최종_plan으로_함께_적용한다() {
    let image = image();
    let offset = (FORM1_DATA_OFFSET + 100) as u64;
    let expected = image.raw_bytes()[offset as usize..offset as usize + 4].to_vec();
    let write = ExpectedWrite {
        id: "text".to_string(),
        actor: "test".to_string(),
        purpose: "대사 교체".to_string(),
        offset,
        expected_original: expected,
        replacement: vec![9, 8, 7, 6],
        intent: WriteIntent::Data,
    };
    let plan = WritePlan::new()
        .region(ImageRegion::new(
            "text",
            write.range(),
            RegionKind::Data,
            "테스트 데이터",
        ))
        .write(write);

    let output = apply_form1_write_plan(&image, &plan).unwrap();
    assert_eq!(&output[offset as usize..offset as usize + 4], &[9, 8, 7, 6]);
    assert_ne!(
        &output[FORM1_INTEGRITY_OFFSET..FORM1_INTEGRITY_OFFSET + FORM1_INTEGRITY_LEN],
        &image.raw_bytes()[FORM1_INTEGRITY_OFFSET..FORM1_INTEGRITY_OFFSET + FORM1_INTEGRITY_LEN]
    );
    assert_eq!(
        &output[SECTOR_LEN..2 * SECTOR_LEN],
        &image.raw_bytes()[SECTOR_LEN..2 * SECTOR_LEN]
    );
}

#[test]
fn 섹터_header나_edc를_논리_data로_쓰지_못한다() {
    let image = image();
    for offset in [0u64, FORM1_INTEGRITY_OFFSET as u64] {
        let write = ExpectedWrite {
            id: format!("bad-{offset}"),
            actor: "test".to_string(),
            purpose: "경계 침범".to_string(),
            offset,
            expected_original: vec![image.raw_bytes()[offset as usize]],
            replacement: vec![image.raw_bytes()[offset as usize] ^ 1],
            intent: WriteIntent::Data,
        };
        let plan = WritePlan::new()
            .region(ImageRegion::new(
                "bad",
                write.range(),
                RegionKind::Data,
                "거부 테스트",
            ))
            .write(write);
        assert!(matches!(
            apply_form1_write_plan(&image, &plan),
            Err(DiscWriteError::WriteOutsideForm1Data { .. })
        ));
    }
}
