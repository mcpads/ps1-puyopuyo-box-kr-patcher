use super::*;
use crate::compress::{compress, decompress};

use super::super::btx_development_archive::{
    BtxDevelopmentArchiveSpec, BtxDevelopmentEntryStorageReport, PreparedBtxDevelopmentEntry,
};

const SECTOR_BYTES: usize = 2048;

const FIRST_SPEC: BtxDevelopmentArchiveSpec = BtxDevelopmentArchiveSpec {
    iso_path: "/TEST_GRP.VDD;1",
    entry_name: "TARGET.BTX",
    expected_cnx_subtype: *b"bi_",
    patch_id: "test-target",
    actor: "pipeline::cnx_development_archive_replacement_tests",
};

const SECOND_SPEC: BtxDevelopmentArchiveSpec = BtxDevelopmentArchiveSpec {
    iso_path: FIRST_SPEC.iso_path,
    entry_name: "SECOND.BTX",
    expected_cnx_subtype: *b"bi_",
    patch_id: "test-second",
    actor: "pipeline::cnx_development_archive_replacement_tests",
};

fn composition_fixture(first_cnx: &[u8], second_cnx: &[u8]) -> Vec<u8> {
    let mut data = vec![0xA5; 7 * SECTOR_BYTES];
    data[..4].copy_from_slice(&3u32.to_le_bytes());
    for (index, (name, sector, size)) in [
        ("TARGET.BTX", 1u32, first_cnx.len()),
        ("SECOND.BTX", 3u32, second_cnx.len()),
        ("OTHER.BIN", 5u32, 5usize),
    ]
    .into_iter()
    .enumerate()
    {
        let base = 4 + index * 24;
        data[base..base + 16].fill(0);
        data[base..base + name.len()].copy_from_slice(name.as_bytes());
        data[base + 16..base + 20].copy_from_slice(&sector.to_le_bytes());
        data[base + 20..base + 24].copy_from_slice(&(size as u32).to_le_bytes());
    }
    data[SECTOR_BYTES..SECTOR_BYTES + first_cnx.len()].copy_from_slice(first_cnx);
    data[3 * SECTOR_BYTES..3 * SECTOR_BYTES + second_cnx.len()].copy_from_slice(second_cnx);
    data[5 * SECTOR_BYTES..5 * SECTOR_BYTES + 5].copy_from_slice(b"other");
    data
}

fn prepared_entry(
    spec: BtxDevelopmentArchiveSpec,
    original_vdd: &[u8],
    patched_plain: &[u8],
) -> PreparedBtxDevelopmentEntry {
    let archive = VddArchive::parse(original_vdd).unwrap();
    let original_entry = archive.find(spec.entry_name).unwrap();
    let original_cnx = archive.read_file(original_entry);
    let replacement_cnx = compress(patched_plain, spec.expected_cnx_subtype).unwrap();
    PreparedBtxDevelopmentEntry {
        spec,
        original_vdd: original_vdd.to_vec(),
        replacement_cnx: replacement_cnx.clone(),
        patched_plain: patched_plain.to_vec(),
        storage: BtxDevelopmentEntryStorageReport {
            original_cnx_bytes: original_cnx.len(),
            replacement_cnx_bytes: replacement_cnx.len(),
            plain_bytes: patched_plain.len(),
            cnx_roundtrip_exact: true,
        },
    }
}

#[test]
fn 같은_vdd의_여러_btx는_불변원본에서_한번_재패킹한다() {
    let first_source = compress(b"first-source", *b"bi_").unwrap();
    let second_source = compress(b"second-source", *b"bi_").unwrap();
    let original_raw = composition_fixture(&first_source, &second_source);
    let first = prepared_entry(FIRST_SPEC, &original_raw, b"first-output");
    let second = prepared_entry(SECOND_SPEC, &original_raw, b"second-output");

    let repack = repack_cnx_development_entries(
        FIRST_SPEC.iso_path,
        "test-composition",
        "pipeline::cnx_development_archive_replacement_tests",
        &[&first, &second],
    )
    .unwrap();

    assert_eq!(repack.report.replacement_entries, 2);
    assert_eq!(
        repack.report.placement,
        CnxDevelopmentArchivePlacement::RepackInContainer
    );
    assert!(repack.report.target_entries_exact);
    assert!(repack.report.non_target_entries_exact);
    assert!(repack.report.moved_entry_offsets > 0);
    let final_archive = VddArchive::parse(&repack.replacement_vdd).unwrap();
    assert_eq!(
        decompress(final_archive.read_file(final_archive.find("TARGET.BTX").unwrap())).unwrap(),
        b"first-output"
    );
    assert_eq!(
        decompress(final_archive.read_file(final_archive.find("SECOND.BTX").unwrap())).unwrap(),
        b"second-output"
    );
    assert_eq!(
        final_archive.read_file(final_archive.find("OTHER.BIN").unwrap()),
        b"other"
    );
}

#[test]
fn 현재_cnx가_container를_넘을때만_성장_vdd를_relocation기여로_만든다() {
    let first_source = compress(b"first-source", *b"bi_").unwrap();
    let second_source = compress(b"second-source", *b"bi_").unwrap();
    let original_raw = composition_fixture(&first_source, &second_source);
    let incompressible = |seed: u32| {
        let mut state = seed;
        (0..12_000)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect::<Vec<_>>()
    };
    let first = prepared_entry(FIRST_SPEC, &original_raw, &incompressible(0x1234_5678));
    let second = prepared_entry(SECOND_SPEC, &original_raw, &incompressible(0x8765_4321));

    let replacement = prepare_cnx_development_archive_replacement(
        FIRST_SPEC.iso_path,
        "test-growing-composition",
        "pipeline::cnx_development_archive_replacement_tests",
        &[&first, &second],
    )
    .unwrap();

    assert_eq!(
        replacement.report.placement,
        CnxDevelopmentArchivePlacement::RelocateToReservation
    );
    assert!(replacement.report.vdd_bytes > replacement.report.original_vdd_bytes);
    assert!(matches!(
        replacement.disc_contribution(),
        CnxDevelopmentDiscContribution::RelocatedFile(_)
    ));
    assert!(replacement.report.target_entries_exact);
    assert!(replacement.report.non_target_entries_exact);
}

#[test]
fn 같은_entry를_두번_합성하거나_서로다른_원본을_섞지않는다() {
    let first_source = compress(b"first-source", *b"bi_").unwrap();
    let second_source = compress(b"second-source", *b"bi_").unwrap();
    let original_raw = composition_fixture(&first_source, &second_source);
    let first = prepared_entry(FIRST_SPEC, &original_raw, b"first-output");
    assert!(repack_cnx_development_entries(
        FIRST_SPEC.iso_path,
        "test-composition",
        "pipeline::cnx_development_archive_replacement_tests",
        &[&first, &first],
    )
    .is_err());

    let mut drifted_raw = original_raw.clone();
    *drifted_raw.last_mut().unwrap() ^= 1;
    let second = prepared_entry(SECOND_SPEC, &drifted_raw, b"second-output");
    assert!(repack_cnx_development_entries(
        FIRST_SPEC.iso_path,
        "test-composition",
        "pipeline::cnx_development_archive_replacement_tests",
        &[&first, &second],
    )
    .is_err());
}
