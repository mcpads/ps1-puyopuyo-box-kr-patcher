use super::*;

#[test]
fn 다른_원본에는_생성판을_적용하지_않는다() {
    for (asset, patch) in [
        (
            MENU202_TEXT_ASSET,
            patch_menu202_surface as fn(&[u8]) -> Result<_, _>,
        ),
        (
            KANBAN_TEXT_ASSET,
            patch_kanban_surface as fn(&[u8]) -> Result<_, _>,
        ),
    ] {
        let wrong = vec![0; asset.layout.byte_len().unwrap()];
        assert!(patch(&wrong).is_err());
        assert!(patch(&wrong[..wrong.len() - 1]).is_err());
    }
}
