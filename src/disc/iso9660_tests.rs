use super::*;
use crate::disc::sector::regenerate_form1;
use crate::source::{FORM1_DATA_OFFSET, SECTOR_LEN};

const SECTORS: u32 = 26;
const ROOT_LBA: u32 = 20;
const SUBDIR_LBA: u32 = 23;

fn put_u16_both(raw: &mut [u8], offset: usize, value: u16) {
    raw[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    raw[offset + 2..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn put_u32_both(raw: &mut [u8], offset: usize, value: u32) {
    raw[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    raw[offset + 4..offset + 8].copy_from_slice(&value.to_be_bytes());
}

fn record(identifier: &[u8], lba: u32, len: u32, flags: u8) -> Vec<u8> {
    let record_len = (33 + identifier.len()).next_multiple_of(2);
    let mut raw = vec![0u8; record_len];
    raw[0] = record_len as u8;
    put_u32_both(&mut raw, 2, lba);
    put_u32_both(&mut raw, 10, len);
    raw[25] = flags;
    put_u16_both(&mut raw, 28, 1);
    raw[32] = identifier.len() as u8;
    raw[33..33 + identifier.len()].copy_from_slice(identifier);
    raw
}

fn base_image() -> Vec<u8> {
    let mut image = vec![0u8; SECTORS as usize * SECTOR_LEN];
    for lba in 0..SECTORS {
        let start = lba as usize * SECTOR_LEN;
        let raw = &mut image[start..start + SECTOR_LEN];
        raw[..12].copy_from_slice(&[
            0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00,
        ]);
        raw[15] = 2;
        raw[16..24].copy_from_slice(&[0, 0, 8, 0, 0, 0, 8, 0]);
        regenerate_form1(raw).unwrap();
    }
    image
}

fn write_user_sector(image: &mut [u8], lba: u32, data: &[u8]) {
    assert!(data.len() <= FORM1_DATA_LEN);
    let start = lba as usize * SECTOR_LEN;
    let raw = &mut image[start..start + SECTOR_LEN];
    raw[FORM1_DATA_OFFSET..FORM1_DATA_OFFSET + data.len()].copy_from_slice(data);
    regenerate_form1(raw).unwrap();
}

pub(crate) fn synthetic_iso() -> Vec<u8> {
    let mut image = base_image();
    let root = record(&[0], ROOT_LBA, FORM1_DATA_LEN as u32, DIRECTORY_FLAG);

    let mut pvd = vec![0u8; FORM1_DATA_LEN];
    pvd[0] = 1;
    pvd[1..6].copy_from_slice(b"CD001");
    pvd[6] = 1;
    pvd[40..72].fill(b' ');
    pvd[40..47].copy_from_slice(b"TESTVOL");
    put_u32_both(&mut pvd, 80, SECTORS);
    put_u16_both(&mut pvd, 128, FORM1_DATA_LEN as u16);
    pvd[156..156 + root.len()].copy_from_slice(&root);
    write_user_sector(&mut image, PVD_LBA, &pvd);

    let mut terminator = vec![0u8; FORM1_DATA_LEN];
    terminator[0] = 255;
    terminator[1..6].copy_from_slice(b"CD001");
    terminator[6] = 1;
    write_user_sector(&mut image, PVD_LBA + 1, &terminator);

    let root_records = [
        record(&[0], ROOT_LBA, FORM1_DATA_LEN as u32, DIRECTORY_FLAG),
        record(&[1], ROOT_LBA, FORM1_DATA_LEN as u32, DIRECTORY_FLAG),
        record(b"SPLIT.BIN;1", 21, 3, MULTI_EXTENT_FLAG),
        record(b"SPLIT.BIN;1", 22, 2, 0),
        record(b"DIR", SUBDIR_LBA, FORM1_DATA_LEN as u32, DIRECTORY_FLAG),
    ];
    let root_data: Vec<u8> = root_records.into_iter().flatten().collect();
    write_user_sector(&mut image, ROOT_LBA, &root_data);

    let subdir_records = [
        record(&[0], SUBDIR_LBA, FORM1_DATA_LEN as u32, DIRECTORY_FLAG),
        record(&[1], ROOT_LBA, FORM1_DATA_LEN as u32, DIRECTORY_FLAG),
        record(b"HELLO.TXT;1", 24, 5, 0),
    ];
    let subdir_data: Vec<u8> = subdir_records.into_iter().flatten().collect();
    write_user_sector(&mut image, SUBDIR_LBA, &subdir_data);
    write_user_sector(&mut image, 21, b"ABC");
    write_user_sector(&mut image, 22, b"DE");
    write_user_sector(&mut image, 24, b"hello");
    image
}

#[test]
fn 디렉터리와_multi_extent를_정확히_읽는다() {
    let image = DiscImage::from_bytes(synthetic_iso()).unwrap();
    let iso = Iso9660::parse(&image).unwrap();

    assert_eq!(iso.volume_id(), "TESTVOL");
    assert_eq!(iso.volume_sectors(), SECTORS);
    assert_eq!(
        iso.entries().iter().map(IsoEntry::path).collect::<Vec<_>>(),
        ["/DIR", "/DIR/HELLO.TXT;1", "/SPLIT.BIN;1"]
    );
    assert!(iso.find("/DIR").unwrap().is_directory());
    assert_eq!(iso.read_file("/SPLIT.BIN;1").unwrap(), b"ABCDE");
    assert_eq!(iso.read_file("/DIR/HELLO.TXT;1").unwrap(), b"hello");
    assert_eq!(
        iso.find("/SPLIT.BIN;1").unwrap().record_locations().len(),
        2
    );
    let hello_record = iso.find("/DIR/HELLO.TXT;1").unwrap().record_locations()[0];
    assert_eq!(hello_record.lba, SUBDIR_LBA);
    assert!(hello_record.user_data_offset < FORM1_DATA_LEN);
}

#[test]
fn 기대한_단일_extent에서만_파일을_읽는다() {
    let image = DiscImage::from_bytes(synthetic_iso()).unwrap();
    let iso = Iso9660::parse(&image).unwrap();
    let expected = Extent { lba: 24, len: 5 };

    let entry = iso
        .verify_single_extent_file("/DIR/HELLO.TXT;1", expected)
        .unwrap();
    assert_eq!(entry.extents(), [expected]);
    assert_eq!(
        iso.read_file_at_extent("/DIR/HELLO.TXT;1", expected)
            .unwrap(),
        b"hello"
    );
}

#[test]
fn 다른_extent와_multi_extent를_단일_extent_readback으로_받아들이지_않는다() {
    let image = DiscImage::from_bytes(synthetic_iso()).unwrap();
    let iso = Iso9660::parse(&image).unwrap();
    let expected = Extent { lba: 24, len: 5 };

    assert!(matches!(
        iso.read_file_at_extent("/DIR/HELLO.TXT;1", Extent { lba: 25, len: 5 }),
        Err(IsoError::UnexpectedFileExtents {
            path,
            expected: Extent { lba: 25, len: 5 },
            actual,
        }) if path == "/DIR/HELLO.TXT;1" && actual == [expected]
    ));
    assert!(matches!(
        iso.verify_single_extent_file("/SPLIT.BIN;1", Extent { lba: 21, len: 5 }),
        Err(IsoError::UnexpectedFileExtents { path, actual, .. })
            if path == "/SPLIT.BIN;1"
                && actual == [Extent { lba: 21, len: 3 }, Extent { lba: 22, len: 2 }]
    ));
}

#[test]
fn 모든_allocation_뒤의_form1_volume_tail만_반환한다() {
    let image = DiscImage::from_bytes(synthetic_iso()).unwrap();
    let iso = Iso9660::parse(&image).unwrap();
    assert_eq!(
        iso.form1_volume_tail().unwrap().unwrap().extent(),
        Extent {
            lba: 25,
            len: FORM1_DATA_LEN as u32,
        }
    );

    let mut bytes = synthetic_iso();
    let pvd = PVD_LBA as usize * SECTOR_LEN + FORM1_DATA_OFFSET;
    put_u32_both(&mut bytes, pvd + 132, 1);
    bytes[pvd + 140..pvd + 144].copy_from_slice(&25u32.to_le_bytes());
    bytes[pvd + 148..pvd + 152].copy_from_slice(&25u32.to_be_bytes());
    let sector_start = PVD_LBA as usize * SECTOR_LEN;
    regenerate_form1(&mut bytes[sector_start..sector_start + SECTOR_LEN]).unwrap();
    let image = DiscImage::from_bytes(bytes).unwrap();
    let iso = Iso9660::parse(&image).unwrap();
    assert_eq!(iso.form1_volume_tail().unwrap(), None);
}

#[test]
fn volume_밖의_파일_extent를_parse에서_거부한다() {
    let mut bytes = synthetic_iso();
    let image = DiscImage::from_bytes(bytes.clone()).unwrap();
    let iso = Iso9660::parse(&image).unwrap();
    let location = iso.find("/DIR/HELLO.TXT;1").unwrap().record_locations()[0];
    let record = location.lba as usize * SECTOR_LEN + FORM1_DATA_OFFSET + location.user_data_offset;
    put_u32_both(&mut bytes, record + 2, SECTORS);
    let sector_start = location.lba as usize * SECTOR_LEN;
    regenerate_form1(&mut bytes[sector_start..sector_start + SECTOR_LEN]).unwrap();

    let image = DiscImage::from_bytes(bytes).unwrap();
    assert!(matches!(
        Iso9660::parse(&image),
        Err(IsoError::ExtentOutsideVolume {
            owner,
            extent: Extent { lba: SECTORS, len: 5 },
            volume_sectors: SECTORS,
        }) if owner == "/DIR/HELLO.TXT;1"
    ));
}

#[test]
fn volume_descriptor_terminator의_signature_drift를_거부한다() {
    let mut bytes = synthetic_iso();
    let terminator = (PVD_LBA as usize + 1) * SECTOR_LEN + FORM1_DATA_OFFSET;
    bytes[terminator + 1] = b'X';
    let sector_start = (PVD_LBA as usize + 1) * SECTOR_LEN;
    regenerate_form1(&mut bytes[sector_start..sector_start + SECTOR_LEN]).unwrap();

    let image = DiscImage::from_bytes(bytes).unwrap();
    assert!(matches!(
        Iso9660::parse(&image),
        Err(IsoError::InvalidVolumeDescriptor(lba)) if lba == PVD_LBA + 1
    ));
}

#[test]
fn dual_endian_불일치와_디렉터리_읽기를_거부한다() {
    let mut bytes = synthetic_iso();
    let pvd = PVD_LBA as usize * SECTOR_LEN + FORM1_DATA_OFFSET;
    bytes[pvd + 84] ^= 1;
    let sector_start = PVD_LBA as usize * SECTOR_LEN;
    regenerate_form1(&mut bytes[sector_start..sector_start + SECTOR_LEN]).unwrap();
    let image = DiscImage::from_bytes(bytes).unwrap();
    assert!(matches!(
        Iso9660::parse(&image),
        Err(IsoError::DualEndianMismatch {
            field: "volume space size",
            ..
        })
    ));

    let image = DiscImage::from_bytes(synthetic_iso()).unwrap();
    let iso = Iso9660::parse(&image).unwrap();
    assert!(matches!(
        iso.read_file("/DIR"),
        Err(IsoError::IsDirectory(path)) if path == "/DIR"
    ));
}

#[test]
fn 종결되지_않거나_이름이_바뀐_multi_extent를_거부한다() {
    let first = DirectoryRecord {
        identifier: Identifier::Named("A.BIN;1".to_string()),
        extent: Extent { lba: 1, len: 1 },
        is_directory: false,
        continues: true,
        location: DirectoryRecordLocation {
            lba: ROOT_LBA,
            user_data_offset: 0,
            record_len: 34,
        },
    };
    assert!(matches!(
        group_records(vec![first.clone()], "/"),
        Err(IsoError::UnterminatedMultiExtent(path)) if path == "/A.BIN;1"
    ));

    let second = DirectoryRecord {
        identifier: Identifier::Named("B.BIN;1".to_string()),
        extent: Extent { lba: 2, len: 1 },
        is_directory: false,
        continues: false,
        location: DirectoryRecordLocation {
            lba: ROOT_LBA,
            user_data_offset: 34,
            record_len: 34,
        },
    };
    assert!(matches!(
        group_records(vec![first, second], "/"),
        Err(IsoError::MultiExtentMismatch(path)) if path == "/A.BIN;1"
    ));
}
