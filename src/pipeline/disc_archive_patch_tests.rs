use super::*;

const ORIGINAL: &[u8] = b"original";
const FIRST: &[u8] = b"first";
const SECOND: &[u8] = b"second";

fn patch(iso_path: &'static str, replacement_vdd: &'static [u8]) -> DiscArchiveEntryPatch<'static> {
    DiscArchiveEntryPatch {
        iso_path,
        entry_name: "TARGET.BTX",
        original_vdd: ORIGINAL,
        replacement_vdd,
        replacement_cnx_bytes: 1,
        id: "fixture",
        actor: "pipeline::disc_archive_patch_tests",
    }
}

#[test]
fn archive변경이_없으면_원본_하나면_replacement가_기대상태다() {
    assert_eq!(
        expected_archive_after_changes("/TARGET.VDD;1", ORIGINAL, &[], &[]).unwrap(),
        ORIGINAL
    );
    assert_eq!(
        expected_archive_after_changes(
            "/TARGET.VDD;1",
            ORIGINAL,
            &[patch("/target.vdd;1", FIRST)],
            &[],
        )
        .unwrap(),
        FIRST
    );
    assert_eq!(
        expected_archive_after_changes(
            "/TARGET.VDD;1",
            ORIGINAL,
            &[],
            &[DiscArchiveRepack {
                iso_path: "/target.vdd;1",
                original_vdd: ORIGINAL,
                replacement_vdd: SECOND,
                id: "fixture-repack",
                actor: "pipeline::disc_archive_patch_tests",
            }],
        )
        .unwrap(),
        SECOND
    );
}

#[test]
fn 같은_vdd의_독립replacement_둘은_적용순서에_맡기지않는다() {
    let patches = [
        patch("/TARGET.VDD;1", FIRST),
        patch("/target.vdd;1", SECOND),
    ];

    assert!(expected_archive_after_changes("/TARGET.VDD;1", ORIGINAL, &patches, &[]).is_err());
}

#[test]
fn entry_patch와_repack도_같은_vdd에_겹치지않는다() {
    let entry = patch("/TARGET.VDD;1", FIRST);
    let repack = DiscArchiveRepack {
        iso_path: "/target.vdd;1",
        original_vdd: ORIGINAL,
        replacement_vdd: SECOND,
        id: "fixture-repack",
        actor: "pipeline::disc_archive_patch_tests",
    };

    assert!(verify_archive_change_paths(&[entry], &[repack]).is_err());
    assert!(expected_archive_after_changes(
        "/TARGET.VDD;1",
        ORIGINAL,
        &[patch("/TARGET.VDD;1", FIRST)],
        &[repack],
    )
    .is_err());
}
