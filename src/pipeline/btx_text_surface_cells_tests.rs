use std::collections::BTreeSet;

use super::*;

#[test]
fn pending_puyo_surface_spec은_6개_finding과_10개_물리_사본이다() {
    let specs = specs();
    assert_eq!(specs.len(), EXPECTED_FINDINGS);
    assert_eq!(
        specs
            .iter()
            .map(|spec| spec.finding_id)
            .collect::<BTreeSet<_>>()
            .len(),
        EXPECTED_FINDINGS
    );
    assert_eq!(
        specs
            .iter()
            .map(|spec| spec.archive_paths.len())
            .sum::<usize>(),
        EXPECTED_PHYSICAL_ASSETS
    );
}

#[test]
fn exact_setup과_상충_setup을_모두_명시한다() {
    let exact = ROW_EXPECTATIONS
        .iter()
        .flat_map(|row| row.setups)
        .filter(|setup| setup.exact_layout)
        .count();
    let conflicting = ROW_EXPECTATIONS
        .iter()
        .flat_map(|row| row.setups)
        .filter(|setup| !setup.exact_layout)
        .count();
    assert_eq!(exact, EXPECTED_EXACT_LAYOUTS);
    assert_eq!(conflicting, EXPECTED_SETUP_RECORDS - EXPECTED_EXACT_LAYOUTS);
}
