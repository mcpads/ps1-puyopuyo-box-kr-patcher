use super::*;

#[test]
fn 다른원본은_픽셀쓰기전에_거부한다() {
    assert!(patch_boxparts_surface(&vec![0; 256 * 256]).is_err());
}
