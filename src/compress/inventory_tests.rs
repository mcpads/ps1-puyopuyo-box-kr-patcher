use super::*;

use crate::compress::compress;

const SECTOR_LEN: usize = 2048;

fn archive(files: &[(&str, Vec<u8>)]) -> VddArchive {
    let mut raw = vec![0u8; (files.len() + 1) * SECTOR_LEN];
    raw[..4].copy_from_slice(&(files.len() as u32).to_le_bytes());
    for (index, (name, data)) in files.iter().enumerate() {
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
fn 네_subtype과_raw를_분리해_센다() {
    let sample = archive(&[
        ("A.SEQ", compress(b"seq", *b"bin").unwrap()),
        ("B.BTX", compress(b"four", *b"bi_").unwrap()),
        ("C.BTX", compress(b"eight", *b"bit").unwrap()),
        ("D.CEL", compress(b"cell", *b"cel").unwrap()),
        ("E.BT", vec![0; 8]),
    ]);
    assert_eq!(
        audit_archives([("sample", &sample)]),
        Ok(CnxInventorySummary {
            archives: 1,
            entries: 5,
            cnx: 4,
            bin: 1,
            bi_: 1,
            bit: 1,
            cel: 1,
            raw: 1,
        })
    );
}

#[test]
fn 미등록_subtype과_확장자_불일치를_거부한다() {
    let unknown = archive(&[("A.SEQ", compress(b"x", *b"wat").unwrap())]);
    assert!(matches!(
        audit_archives([("unknown", &unknown)]),
        Err(CnxInventoryError::UnknownSubtype { .. })
    ));

    let mismatch = archive(&[("A.BTX", compress(b"x", *b"bin").unwrap())]);
    assert!(matches!(
        audit_archives([("mismatch", &mismatch)]),
        Err(CnxInventoryError::ExtensionMismatch { .. })
    ));
}

#[test]
fn box_계약은_미등록_archive와_누락을_거부한다() {
    let sample = archive(&[("A.SEQ", compress(b"x", *b"bin").unwrap())]);
    assert_eq!(
        verify_box_inventory([("/UNKNOWN.VDD;1", &sample)]),
        Err(CnxInventoryError::UnexpectedArchive(
            "/UNKNOWN.VDD;1".to_string()
        ))
    );
    let none: [(&str, &VddArchive); 0] = [];
    assert_eq!(
        verify_box_inventory(none),
        Err(CnxInventoryError::MissingArchive("/FIL_GRP.VDD;1"))
    );
}
