use super::*;

#[test]
fn 주소분할은_lw_부호확장을_보존한다() {
    assert_eq!(split_address(0x800B_DE68), (0x800C, -0x2198));
    assert_eq!(split_address(0x800A_3910), (0x800A, 0x3910));
}

#[test]
fn 다섯_exe는_각자_별도_renderer와_descriptor를_가진다() {
    assert_eq!(BOXOPT_SPECS.len(), 5);
    assert_eq!(
        BOXOPT_SPECS
            .iter()
            .map(|spec| spec.exe)
            .collect::<BTreeSet<_>>()
            .len(),
        5
    );
    assert!(BOXOPT_SPECS.iter().all(|spec| {
        spec.renderer_sites[0].resource_load_runtime != spec.renderer_sites[1].resource_load_runtime
    }));
}
