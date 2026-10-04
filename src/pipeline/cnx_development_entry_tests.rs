use super::*;

const SECTOR_BYTES: usize = 2048;
const SPEC: CnxDevelopmentEntrySpec = CnxDevelopmentEntrySpec {
    iso_path: "/TEST_SEQ.VDD;1",
    entry_name: "TARGET.SEQ",
    expected_cnx_subtype: *b"bin",
    patch_id: "test-target",
    actor: "pipeline::cnx_development_entry_tests",
};

fn fixture(subtype: [u8; 3]) -> Vec<u8> {
    let target = compress(b"original", subtype).unwrap();
    let mut data = vec![0xA5; 4 * SECTOR_BYTES];
    data[..4].copy_from_slice(&2u32.to_le_bytes());
    data[4..20].fill(0);
    data[4..14].copy_from_slice(b"TARGET.SEQ");
    data[20..24].copy_from_slice(&1u32.to_le_bytes());
    data[24..28].copy_from_slice(&(target.len() as u32).to_le_bytes());
    data[28..44].fill(0);
    data[28..37].copy_from_slice(b"OTHER.BIN");
    data[44..48].copy_from_slice(&3u32.to_le_bytes());
    data[48..52].copy_from_slice(&5u32.to_le_bytes());
    data[SECTOR_BYTES..SECTOR_BYTES + target.len()].copy_from_slice(&target);
    data[3 * SECTOR_BYTES..3 * SECTOR_BYTES + 5].copy_from_slice(b"other");
    data
}

#[test]
fn cnx_entry는_같은길이변경을재압축하고_원본vdd를보존한다() {
    let original = fixture(*b"bin");
    let (prepared, marker) =
        prepare_cnx_development_entry_from_vdd(SPEC, original.clone(), |plain| {
            assert_eq!(plain, b"original");
            Ok((b"patched!".to_vec(), "patched"))
        })
        .unwrap();

    assert_eq!(marker, "patched");
    assert_eq!(prepared.original_vdd, original);
    assert_eq!(prepared.patched_plain(), b"patched!");
    assert_eq!(decompress(&prepared.replacement_cnx).unwrap(), b"patched!");
}

#[test]
fn cnx_entry는_무변경_길이변경_subtype불일치를거부한다() {
    let unchanged = prepare_cnx_development_entry_from_vdd(SPEC, fixture(*b"bin"), |plain| {
        Ok((plain.to_vec(), ()))
    });
    assert!(unchanged.unwrap_err().to_string().contains("원본과 같음"));

    let resized = prepare_cnx_development_entry_from_vdd(SPEC, fixture(*b"bin"), |_| {
        Ok((b"short".to_vec(), ()))
    });
    assert!(resized
        .unwrap_err()
        .to_string()
        .contains("plain 크기가 바뀜"));

    let wrong_subtype = prepare_cnx_development_entry_from_vdd(SPEC, fixture(*b"bi_"), |_| {
        Ok((b"patched!".to_vec(), ()))
    });
    assert!(wrong_subtype
        .unwrap_err()
        .to_string()
        .contains("CNX subtype 불일치"));
}
