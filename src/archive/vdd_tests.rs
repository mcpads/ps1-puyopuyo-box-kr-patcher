use super::*;

fn fixture() -> Vec<u8> {
    let mut data = vec![0xA5; 4 * SECTOR_LEN];
    data[..4].copy_from_slice(&2u32.to_le_bytes());

    data[4..12].copy_from_slice(b"TEST.BTX");
    data[20..24].copy_from_slice(&1u32.to_le_bytes());
    data[24..28].copy_from_slice(&5u32.to_le_bytes());

    data[28..36].copy_from_slice(b"DATA.SEQ");
    data[44..48].copy_from_slice(&3u32.to_le_bytes());
    data[48..52].copy_from_slice(&7u32.to_le_bytes());

    // 이름의 NUL 패딩은 정규 형식이고, 그 밖의 표-데이터 간격은 보호한다.
    data[12..20].fill(0);
    data[36..44].fill(0);
    data[SECTOR_LEN..SECTOR_LEN + 5].copy_from_slice(b"hello");
    data[3 * SECTOR_LEN..3 * SECTOR_LEN + 7].copy_from_slice(b"goodbye");
    data
}

#[test]
fn parses_entries_and_reads_files() {
    let archive = VddArchive::parse(&fixture()).unwrap();
    assert_eq!(archive.entry_count(), 2);
    assert_eq!(archive.entries()[0].name(), "TEST.BTX");
    assert_eq!(archive.entries()[0].byte_offset(), SECTOR_LEN);
    assert_eq!(archive.find("data.seq").unwrap().size, 7);
    assert_eq!(archive.read_file(&archive.entries()[1]), b"goodbye");
}

#[test]
fn lossless_serialization_preserves_unknown_padding_and_tail() {
    let original = fixture();
    let archive = VddArchive::parse(&original).unwrap();
    assert_eq!(archive.serialize_lossless(), original);
    assert_eq!(archive.rebuild(&BTreeMap::new()).unwrap(), original);
}

#[test]
fn rebuild_replaces_one_file_and_preserves_the_other() {
    let archive = VddArchive::parse(&fixture()).unwrap();
    let replacements = BTreeMap::from([("data.seq".to_string(), vec![0xBB; 3_000])]);
    let rebuilt = archive.rebuild(&replacements).unwrap();
    let reparsed = VddArchive::parse(&rebuilt).unwrap();

    assert_eq!(reparsed.read_file(&reparsed.entries()[0]), b"hello");
    assert_eq!(reparsed.entries()[1].size, 3_000);
    assert!(reparsed
        .read_file(&reparsed.entries()[1])
        .iter()
        .all(|&byte| byte == 0xBB));
}

#[test]
fn 제자리_교체는_offset과_전체_크기와_미해석_padding을_보존한다() {
    let original = fixture();
    let archive = VddArchive::parse(&original).unwrap();
    let replacements = BTreeMap::from([("test.btx".to_string(), b"hi".to_vec())]);
    let rebuilt = archive.replace_in_place(&replacements).unwrap();
    let reparsed = VddArchive::parse(&rebuilt).unwrap();

    assert_eq!(rebuilt.len(), original.len());
    assert_eq!(reparsed.entries()[0].sector_offset, 1);
    assert_eq!(reparsed.entries()[1].sector_offset, 3);
    assert_eq!(reparsed.read_file(&reparsed.entries()[0]), b"hi");
    assert_eq!(reparsed.read_file(&reparsed.entries()[1]), b"goodbye");
    assert_eq!(
        &rebuilt[SECTOR_LEN + 2..3 * SECTOR_LEN],
        &original[SECTOR_LEN + 2..3 * SECTOR_LEN]
    );
}

#[test]
fn 제자리_교체는_다음_entry를_넘으면_실패한다() {
    let archive = VddArchive::parse(&fixture()).unwrap();
    let replacements = BTreeMap::from([("TEST.BTX".to_string(), vec![0x55; 2 * SECTOR_LEN + 1])]);
    assert_eq!(
        archive.replace_in_place(&replacements),
        Err(VddError::ReplacementDoesNotFit {
            name: "TEST.BTX".to_string(),
            replacement: 2 * SECTOR_LEN + 1,
            capacity: 2 * SECTOR_LEN,
        })
    );
}

#[test]
fn 제자리_교체는_대소문자만_다른_중복_요청을_거부한다() {
    let archive = VddArchive::parse(&fixture()).unwrap();
    let replacements = BTreeMap::from([
        ("TEST.BTX".to_string(), b"a".to_vec()),
        ("test.btx".to_string(), b"b".to_vec()),
    ]);
    assert!(matches!(
        archive.replace_in_place(&replacements),
        Err(VddError::DuplicateReplacement(_))
    ));
}

#[test]
fn 고정_컨테이너_재패킹은_개별_slot_초과를_전체_여유로_흡수한다() {
    let mut original = fixture();
    original[44..48].copy_from_slice(&2u32.to_le_bytes());
    original[48..52].copy_from_slice(&3_000u32.to_le_bytes());
    original[2 * SECTOR_LEN..2 * SECTOR_LEN + 3_000].fill(0xCC);
    let archive = VddArchive::parse(&original).unwrap();
    let replacements = BTreeMap::from([
        ("TEST.BTX".to_string(), vec![0xBB; 3_000]),
        ("DATA.SEQ".to_string(), b"goodbye".to_vec()),
    ]);

    assert!(matches!(
        archive.replace_in_place(&replacements),
        Err(VddError::ReplacementDoesNotFit { .. })
    ));
    let rebuilt = archive.repack_in_container(&replacements).unwrap();
    let reparsed = VddArchive::parse(&rebuilt).unwrap();
    assert_eq!(rebuilt.len(), original.len());
    assert_eq!(reparsed.entries()[0].sector_offset, 1);
    assert_eq!(reparsed.entries()[1].sector_offset, 3);
    assert_eq!(
        reparsed.read_file(&reparsed.entries()[0]),
        vec![0xBB; 3_000]
    );
    assert_eq!(
        reparsed.read_file(&reparsed.entries()[1]),
        b"goodbye".as_slice()
    );
}

#[test]
fn 고정_컨테이너_재패킹은_전체_크기를_넘으면_실패한다() {
    let archive = VddArchive::parse(&fixture()).unwrap();
    let replacements = BTreeMap::from([("TEST.BTX".to_string(), vec![0x55; 4 * SECTOR_LEN])]);
    assert!(matches!(
        archive.repack_in_container(&replacements),
        Err(VddError::RepackedArchiveDoesNotFit { .. })
    ));
}

#[test]
fn rejects_unknown_replacement_names() {
    let archive = VddArchive::parse(&fixture()).unwrap();
    let replacements = BTreeMap::from([("NOPE.SEQ".to_string(), vec![1])]);
    assert!(matches!(
        archive.rebuild(&replacements),
        Err(VddError::UnknownReplacement(_))
    ));
}

#[test]
fn rejects_truncated_tables() {
    let mut data = vec![0; 10];
    data[..4].copy_from_slice(&2u32.to_le_bytes());
    assert!(matches!(
        VddArchive::parse(&data),
        Err(VddError::TruncatedTable { .. })
    ));
}

#[test]
fn rejects_non_ascii_names_and_nonzero_name_padding() {
    let mut invalid = fixture();
    invalid[4] = 0xFF;
    assert!(matches!(
        VddArchive::parse(&invalid),
        Err(VddError::InvalidNameByte { .. })
    ));

    let mut invalid = fixture();
    invalid[13] = b'X';
    assert!(matches!(
        VddArchive::parse(&invalid),
        Err(VddError::NonZeroNamePadding { .. })
    ));
}

#[test]
fn rejects_duplicate_names_case_insensitively() {
    let mut invalid = fixture();
    invalid[28..36].copy_from_slice(b"test.btx");
    assert!(matches!(
        VddArchive::parse(&invalid),
        Err(VddError::DuplicateName(_))
    ));
}

#[test]
fn rejects_entries_before_data_or_outside_the_archive() {
    let mut before_data = fixture();
    before_data[20..24].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        VddArchive::parse(&before_data),
        Err(VddError::EntryBeforeData { .. })
    ));

    let mut outside = fixture();
    outside[20..24].copy_from_slice(&10u32.to_le_bytes());
    assert!(matches!(
        VddArchive::parse(&outside),
        Err(VddError::EntryOutOfRange { .. })
    ));
}

#[test]
fn rejects_overlapping_entries() {
    let mut invalid = fixture();
    invalid[44..48].copy_from_slice(&1u32.to_le_bytes());
    assert!(matches!(
        VddArchive::parse(&invalid),
        Err(VddError::OverlappingEntries { .. })
    ));
}
