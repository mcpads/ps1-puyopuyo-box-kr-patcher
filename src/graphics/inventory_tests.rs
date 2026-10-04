use super::*;

use crate::compress::compress;

const SECTOR_LEN: usize = 2048;

fn archive(files: &[(&str, Vec<u8>)]) -> VddArchive {
    let mut raw = vec![0u8; (files.len() + 1) * SECTOR_LEN];
    raw[..4].copy_from_slice(&(files.len() as u32).to_le_bytes());
    for (index, (name, data)) in files.iter().enumerate() {
        assert!(name.len() <= 16);
        let table = 4 + index * 24;
        raw[table..table + name.len()].copy_from_slice(name.as_bytes());
        raw[table + 16..table + 20].copy_from_slice(&((index + 1) as u32).to_le_bytes());
        raw[table + 20..table + 24].copy_from_slice(&(data.len() as u32).to_le_bytes());
        let start = (index + 1) * SECTOR_LEN;
        raw[start..start + data.len()].copy_from_slice(data);
    }
    VddArchive::parse(&raw).unwrap()
}

#[test]
fn 여러_grp의_구조_분모와_고유_내용을_센다() {
    let four = compress(&[1, 2, 3, 4], *b"bi_").unwrap();
    let eight = compress(&[1, 2, 3, 4], *b"bit").unwrap();
    let first = archive(&[
        ("A.BTX", four.clone()),
        ("B.BTX", four),
        ("OTHER.BX", vec![0xAA]),
    ]);
    let second = archive(&[("C.BTX", eight)]);

    assert_eq!(
        audit_archives([("first", &first), ("second", &second)]),
        Ok(InventorySummary {
            archives: 2,
            entries: 4,
            physical_btx: 3,
            indexed4: 2,
            indexed8: 1,
            unique_content: 2,
            other_entries: 1,
        })
    );
}

#[test]
fn raw_btx와_알_수_없는_subtype을_거부한다() {
    let raw = archive(&[("RAW.BTX", vec![1, 2, 3, 4])]);
    assert!(matches!(
        audit_archives([("raw", &raw)]),
        Err(InventoryError::NonCnxBtx { .. })
    ));

    let unknown = archive(&[("UNKNOWN.BTX", compress(&[1], *b"bin").unwrap())]);
    assert!(matches!(
        audit_archives([("unknown", &unknown)]),
        Err(InventoryError::UnsupportedFormat {
            subtype: [b'b', b'i', b'n'],
            ..
        })
    ));
}

#[test]
fn 빈_입력과_중복_archive_id를_거부한다() {
    let no_archives: [(&str, &VddArchive); 0] = [];
    assert_eq!(audit_archives(no_archives), Err(InventoryError::EmptyInput));

    let sample = archive(&[("A.BTX", compress(&[1], *b"bit").unwrap())]);
    assert!(matches!(
        audit_archives([("same", &sample), ("same", &sample)]),
        Err(InventoryError::DuplicateArchiveId(id)) if id == "same"
    ));
}
