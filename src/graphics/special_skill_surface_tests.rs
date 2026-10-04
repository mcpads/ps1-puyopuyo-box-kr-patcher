use super::*;
#[test]
fn all_translated_glyphs_fit_and_leave_window_arrows_and_unallocated_cells_intact() {
    let text = SkillText::load().unwrap();
    let original = vec![0xab; 256 * 208 / 2];
    let out = compose(&original, &text).unwrap();
    let before = decode(&original, SPECIAL_SKILL_ASSET.layout).unwrap();
    let after = decode(&out, SPECIAL_SKILL_ASSET.layout).unwrap();
    for y in 0..208 {
        for x in 0..256 {
            if x >= 252 || y >= 144 || y / 12 * 21 + x / 12 >= text.glyphs.len() {
                assert_eq!(after[y * 256 + x], before[y * 256 + x]);
            } else {
                assert!([0, 15].contains(&after[y * 256 + x]));
            }
        }
    }
    assert!(patch_special_skill_surface(&original).is_err());
}
