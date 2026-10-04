use super::*;
use crate::disc::iso9660::tests::synthetic_iso;
use crate::disc::{apply_form1_write_plan, Iso9660};

#[test]
fn extent_field는_lba와_길이를_dual_endian으로_쓴다() {
    let raw = encode_extent_fields(Extent {
        lba: 0x1234_5678,
        len: 0x90AB_CDEF,
    });
    assert_eq!(&raw[..4], &0x1234_5678u32.to_le_bytes());
    assert_eq!(&raw[4..8], &0x1234_5678u32.to_be_bytes());
    assert_eq!(&raw[8..12], &0x90AB_CDEFu32.to_le_bytes());
    assert_eq!(&raw[12..], &0x90AB_CDEFu32.to_be_bytes());
}

#[test]
fn 논리길이를_sector로_올림해_앞에서부터_배치한다() {
    let partition = partition_extent_with_remainder(
        Extent {
            lba: 100,
            len: 10_000,
        },
        &[3_000, 1],
    )
    .unwrap();

    assert_eq!(
        partition.allocations(),
        &[
            Extent {
                lba: 100,
                len: 3_000,
            },
            Extent { lba: 102, len: 1 },
        ]
    );
    assert_eq!(
        partition.remainder(),
        Extent {
            lba: 103,
            len: 3_856,
        }
    );
}

#[test]
fn 빈요청은_원본_extent를_그대로_남긴다() {
    let source = Extent {
        lba: 321,
        len: 4_096,
    };
    let partition = partition_extent_with_remainder(source, &[]).unwrap();
    assert!(partition.allocations().is_empty());
    assert_eq!(partition.remainder(), source);
}

#[test]
fn 길이0_요청과_빈나머지를_거부한다() {
    assert!(matches!(
        partition_extent_with_remainder(Extent { lba: 1, len: 0 }, &[]),
        Err(IsoRelocationError::EmptySourceExtent)
    ));
    assert!(matches!(
        partition_extent_with_remainder(Extent { lba: 1, len: 4_096 }, &[0]),
        Err(IsoRelocationError::EmptyAllocation { index: 0 })
    ));
    assert!(matches!(
        partition_extent_with_remainder(Extent { lba: 1, len: 4_096 }, &[4_096]),
        Err(IsoRelocationError::AllocationExceedsSource {
            required: 4_097,
            available: 4_096,
        })
    ));
}

#[test]
fn 다음_extent의_lba가_u32를_넘으면_거부한다() {
    assert!(matches!(
        partition_extent_with_remainder(
            Extent {
                lba: u32::MAX,
                len: 4_096,
            },
            &[1],
        ),
        Err(IsoRelocationError::RelocatedLbaOverflow)
    ));
}

#[test]
fn volume_tail은_전체_sector를_소비하는_파일도_배정한다() {
    let image = DiscImage::from_bytes(synthetic_iso()).unwrap();
    let iso = Iso9660::parse(&image).unwrap();
    let tail = iso.form1_volume_tail().unwrap().unwrap();

    let empty = allocate_volume_tail(tail, &[]).unwrap();
    assert!(empty.allocations().is_empty());
    assert_eq!(empty.remainder(), Some(tail));

    let layout = allocate_volume_tail(tail, &[8]).unwrap();
    assert_eq!(layout.allocations()[0].extent(), Extent { lba: 25, len: 8 });
    assert_eq!(layout.remainder(), None);
    assert!(matches!(
        allocate_volume_tail(tail, &[FORM1_DATA_LEN as u32 + 1]),
        Err(IsoRelocationError::AllocationExceedsSource {
            required: 4_096,
            available: 2_048,
        })
    ));
}

#[test]
fn data_file_relocation은_tail_payload_record와_원본보호를_한_plan에_묶는다() {
    let image = DiscImage::from_bytes(synthetic_iso()).unwrap();
    let iso = Iso9660::parse(&image).unwrap();
    let tail = iso.form1_volume_tail().unwrap().unwrap();
    let layout = allocate_volume_tail(tail, &[8]).unwrap();
    let allocation = layout.allocations()[0];
    let entry = iso.find("/DIR/HELLO.TXT;1").unwrap();
    let original_sector = image.raw_sector(24).unwrap().to_vec();
    let mut plan = WritePlan::new();

    add_single_extent_data_file_relocation(
        &mut plan,
        &image,
        entry,
        allocation,
        b"expanded",
        "hello-growth",
        "test",
        "합성 data 파일을 volume tail로 성장 재배치",
    )
    .unwrap();
    assert_eq!(plan.writes.len(), 2);
    assert_eq!(plan.regions.len(), 2);
    assert_eq!(plan.unchanged.len(), 1);

    let output = apply_form1_write_plan(&image, &plan).unwrap();
    let final_image = DiscImage::from_bytes(output).unwrap();
    let final_iso = Iso9660::parse(&final_image).unwrap();
    assert_eq!(
        final_iso
            .read_file_at_extent("/DIR/HELLO.TXT;1", allocation.extent())
            .unwrap(),
        b"expanded"
    );
    assert_eq!(final_image.raw_sector(24).unwrap(), original_sector);
    assert_eq!(final_iso.form1_volume_tail().unwrap(), None);
}

#[test]
fn data_file_relocation은_길이와_executable_payload를_부분등록전에_거부한다() {
    let image = DiscImage::from_bytes(synthetic_iso()).unwrap();
    let iso = Iso9660::parse(&image).unwrap();
    let tail = iso.form1_volume_tail().unwrap().unwrap();
    let layout = allocate_volume_tail(tail, &[8]).unwrap();
    let allocation = layout.allocations()[0];
    let entry = iso.find("/DIR/HELLO.TXT;1").unwrap();
    let mut plan = WritePlan::new();

    assert!(matches!(
        add_single_extent_data_file_relocation(
            &mut plan,
            &image,
            entry,
            allocation,
            b"short",
            "bad-length",
            "test",
            "길이 거부",
        ),
        Err(IsoRelocationError::ReplacementLength {
            expected: 8,
            actual: 5,
            ..
        })
    ));
    assert!(matches!(
        add_single_extent_data_file_relocation(
            &mut plan,
            &image,
            entry,
            allocation,
            PSX_EXE_MAGIC,
            "bad-exe",
            "test",
            "executable 거부",
        ),
        Err(IsoRelocationError::ExecutablePayloadUnsupported(path))
            if path == "/DIR/HELLO.TXT;1"
    ));
    let other_image = DiscImage::from_bytes(synthetic_iso()).unwrap();
    assert!(matches!(
        add_single_extent_data_file_relocation(
            &mut plan,
            &other_image,
            entry,
            allocation,
            b"expanded",
            "wrong-image",
            "test",
            "다른 이미지 증거 거부",
        ),
        Err(IsoRelocationError::AllocationImageMismatch)
    ));
    assert!(plan.writes.is_empty());
    assert!(plan.regions.is_empty());
    assert!(plan.unchanged.is_empty());
}
