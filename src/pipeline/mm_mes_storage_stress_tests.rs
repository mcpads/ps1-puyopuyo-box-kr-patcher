use super::*;

#[test]
fn storage_stress는_서로_다른_renderable_한글로_512_cell을_채운다() {
    let (mappings, scanned, skipped, renderable) = stress_mappings().unwrap();
    assert_eq!(mappings.len(), MM_MES_STORAGE_CELLS);
    assert_eq!(
        mappings
            .iter()
            .filter_map(|mapping| mapping.glyph)
            .collect::<BTreeSet<_>>()
            .len(),
        MM_MES_STORAGE_CELLS
    );
    assert_eq!(scanned, renderable + skipped);
    assert!(renderable > mappings.len());
    assert_eq!(mappings.first().unwrap().cell, 0);
    assert_eq!(
        usize::from(mappings.last().unwrap().cell),
        MM_MES_STORAGE_CELLS - 1
    );
}
