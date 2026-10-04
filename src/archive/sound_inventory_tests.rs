use super::*;

const SECTOR: usize = 2048;

fn archive(entries: &[(&str, &[u8])]) -> VddArchive {
    let data_sectors: usize = entries
        .iter()
        .map(|(_, data)| data.len().div_ceil(SECTOR))
        .sum();
    let mut raw = vec![0u8; (1 + data_sectors) * SECTOR];
    raw[..4].copy_from_slice(&(entries.len() as u32).to_le_bytes());
    let mut sector_offset = 1usize;
    for (index, (name, data)) in entries.iter().enumerate() {
        let table = 4 + index * 24;
        raw[table..table + name.len()].copy_from_slice(name.as_bytes());
        raw[table + 16..table + 20].copy_from_slice(&(sector_offset as u32).to_le_bytes());
        raw[table + 20..table + 24].copy_from_slice(&(data.len() as u32).to_le_bytes());
        let offset = sector_offset * SECTOR;
        raw[offset..offset + data.len()].copy_from_slice(data);
        sector_offset += data.len().div_ceil(SECTOR);
    }
    VddArchive::parse(&raw).unwrap()
}

fn vab_pair() -> (Vec<u8>, Vec<u8>) {
    let header_bytes = 0x20 + 128 * 0x10 + 16 * 0x20 + 256 * 2;
    let body = vec![0x5a; 16];
    let mut header = vec![0; header_bytes];
    header[..4].copy_from_slice(b"pBAV");
    header[4..8].copy_from_slice(&7u32.to_le_bytes());
    header[0x0c..0x10].copy_from_slice(&((header_bytes + body.len()) as u32).to_le_bytes());
    header[0x12..0x14].copy_from_slice(&1u16.to_le_bytes());
    header[0x14..0x16].copy_from_slice(&1u16.to_le_bytes());
    header[0x16..0x18].copy_from_slice(&1u16.to_le_bytes());
    header[0x20] = 1;
    let tone_table = 0x20 + 128 * 0x10;
    header[tone_table + 0x16..tone_table + 0x18].copy_from_slice(&1u16.to_le_bytes());
    let wave_table = tone_table + 16 * 0x20;
    header[wave_table + 2..wave_table + 4].copy_from_slice(&2u16.to_le_bytes());
    (header, body)
}

#[test]
fn vab_pair와_sound_sequence만_센다() {
    let (header, body) = vab_pair();
    let archive = archive(&[
        ("BANK.VH", &header),
        ("BANK.VB", &body),
        ("MUSIC.SEQ", b"pQES\x00"),
    ]);
    let summary = audit_sound_archives([("/TEST_SND.VDD;1", &archive)]).unwrap();

    assert_eq!(
        summary,
        SoundInventorySummary {
            archives: 1,
            entries: 3,
            vab_headers: 1,
            vab_bodies: 1,
            sound_sequences: 1,
        }
    );
}

#[test]
fn 확장자만_seq인_script와_magic_drift를_거부한다() {
    let script = archive(&[("STORY.SEQ", b"CNX\x02")]);
    assert!(matches!(
        audit_sound_archives([("SND", &script)]),
        Err(SoundInventoryError::InvalidMagic { .. })
    ));

    let bad_header = archive(&[("BANK.VH", b"NOPE"), ("BANK.VB", b"\0")]);
    assert!(matches!(
        audit_sound_archives([("SND", &bad_header)]),
        Err(SoundInventoryError::InvalidMagic { .. })
    ));
}

#[test]
fn vab_stem_짝과_지원_확장자를_요구한다() {
    let (header, body) = vab_pair();
    let unpaired = archive(&[("LEFT.VH", &header), ("RIGHT.VB", &body)]);
    assert!(matches!(
        audit_sound_archives([("SND", &unpaired)]),
        Err(SoundInventoryError::UnpairedVab { .. })
    ));

    let unknown = archive(&[("WORDS.TXT", b"text")]);
    assert!(matches!(
        audit_sound_archives([("SND", &unknown)]),
        Err(SoundInventoryError::UnsupportedEntry { .. })
    ));
}

#[test]
fn 빈_입력과_archive_id_중복을_거부한다() {
    assert_eq!(
        audit_sound_archives(std::iter::empty()),
        Err(SoundInventoryError::EmptyInput)
    );
    let (header, body) = vab_pair();
    let archive = archive(&[("BANK.VH", &header), ("BANK.VB", &body)]);
    assert!(matches!(
        audit_sound_archives([("SND", &archive), ("SND", &archive)]),
        Err(SoundInventoryError::DuplicateArchiveId(_))
    ));
}

#[test]
fn 짝이_맞아도_vab_내부_drift를_거부한다() {
    let (mut header, body) = vab_pair();
    header[0x0c..0x10].copy_from_slice(&0u32.to_le_bytes());
    let archive = archive(&[("BANK.VH", &header), ("BANK.VB", &body)]);
    assert!(matches!(
        audit_sound_archives([("SND", &archive)]),
        Err(SoundInventoryError::InvalidVab { .. })
    ));
}
