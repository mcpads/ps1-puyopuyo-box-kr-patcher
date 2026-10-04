use super::*;

#[test]
fn exact_mmenu가_아닌입력은_surface합성전에_실패한다() {
    let result = patch_mmenu_development_surface(&vec![0; 640 * 256]);

    assert!(result.is_err());
}
