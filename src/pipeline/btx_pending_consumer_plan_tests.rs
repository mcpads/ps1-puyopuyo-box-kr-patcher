use super::*;

#[test]
fn 최소_경계_중_더_많은_물리_행을_덮는_집합을_고른다() {
    let boundaries = BTreeMap::from([
        ("/A", BTreeSet::from(["F1".to_string()])),
        ("/B", BTreeSet::from(["F1".to_string()])),
        ("/C", BTreeSet::from(["F2".to_string(), "F3".to_string()])),
        ("/D", BTreeSet::from(["F2".to_string()])),
        ("/E", BTreeSet::from(["F3".to_string()])),
    ]);
    assert_eq!(
        choose_representative_archives(&boundaries),
        Some(BTreeSet::from(["/A", "/C"]))
    );
}

#[test]
fn 대표_단계와_사본_단계는_runtime_미검증_gate를_분리한다() {
    assert_eq!(
        BtxPendingRuntimePhase::Representative.next_gate(),
        "fresh-entry-exact-descriptor-read-breakpoint-then-screen-and-gpu"
    );
    assert_eq!(
        BtxPendingRuntimePhase::PhysicalCopyFollowup.next_gate(),
        "after-representative-run-same-exact-physical-copy-gate"
    );
}

#[test]
fn descriptor_선택은_exe와_archive가_모두_맞아야_한다() {
    let selection = BtxDescriptorSelection {
        exe: "TEST.EXE",
        archive: "/TEST/TEST_GRP.VDD;1",
        resource_multiply_runtime: 1,
        descriptor_select_runtime: 2,
        cache_check_runtime: 3,
        descriptor_copy_runtime: 4,
        descriptor_name_field_read_runtime: 5,
        descriptor_copy_end_runtime: 6,
        descriptor_copy_source_register: crate::isa::Reg::V1,
        working_descriptor_runtime: 7,
        entry_reached: true,
    };
    let analysis = BtxDescriptorSelectionAnalysis {
        selections: vec![selection],
    };
    assert!(exact_selection(&analysis, "TEST.EXE", "/TEST/TEST_GRP.VDD;1").is_ok());
    assert!(exact_selection(&analysis, "TEST.EXE", "/WRONG/GRP.VDD;1").is_err());
}
