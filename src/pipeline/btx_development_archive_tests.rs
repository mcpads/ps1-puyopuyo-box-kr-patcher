use super::*;
use crate::compress::compress;

const SECTOR_BYTES: usize = 2048;

const SPEC: BtxDevelopmentArchiveSpec = BtxDevelopmentArchiveSpec {
    iso_path: "/TEST_GRP.VDD;1",
    entry_name: "TARGET.BTX",
    expected_cnx_subtype: *b"bi_",
    patch_id: "test-target",
    actor: "pipeline::btx_development_archive_tests",
};

fn fixture(target_cnx: &[u8]) -> Vec<u8> {
    let mut data = vec![0xA5; 4 * SECTOR_BYTES];
    data[..4].copy_from_slice(&2u32.to_le_bytes());
    data[4..20].fill(0);
    data[4..14].copy_from_slice(b"TARGET.BTX");
    data[20..24].copy_from_slice(&1u32.to_le_bytes());
    data[24..28].copy_from_slice(&(target_cnx.len() as u32).to_le_bytes());
    data[28..44].fill(0);
    data[28..37].copy_from_slice(b"OTHER.BIN");
    data[44..48].copy_from_slice(&3u32.to_le_bytes());
    data[48..52].copy_from_slice(&5u32.to_le_bytes());
    data[SECTOR_BYTES..SECTOR_BYTES + target_cnx.len()].copy_from_slice(target_cnx);
    data[3 * SECTOR_BYTES..3 * SECTOR_BYTES + 5].copy_from_slice(b"other");
    data
}

#[test]
fn 교체archive는_target_readback과_비대상보존을_각각검증한다() {
    let original_cnx = compress(b"original", *b"bi_").unwrap();
    let replacement_plain = b"patched";
    let replacement_cnx = compress(replacement_plain, *b"bi_").unwrap();
    let original_raw = fixture(&original_cnx);
    let original = VddArchive::parse(&original_raw).unwrap();
    let replacement = original
        .replace_in_place(&BTreeMap::from([(
            SPEC.entry_name.to_string(),
            replacement_cnx.clone(),
        )]))
        .unwrap();

    assert_eq!(
        verify_replacement_archive(
            SPEC,
            &original,
            &replacement,
            &replacement_cnx,
            replacement_plain,
        )
        .unwrap(),
        (true, true)
    );

    let drift = original
        .replace_in_place(&BTreeMap::from([
            (SPEC.entry_name.to_string(), replacement_cnx.clone()),
            ("OTHER.BIN".to_string(), b"drift".to_vec()),
        ]))
        .unwrap();
    assert!(verify_replacement_archive(
        SPEC,
        &original,
        &drift,
        &replacement_cnx,
        replacement_plain,
    )
    .is_err());
}
