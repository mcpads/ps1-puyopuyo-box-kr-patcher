use super::{encode, Layout, PixelFormat};

#[test]
fn 기존_graphics_btx_공개경로는_중립codec과_같은type을_노출한다() {
    let layout: crate::btx::Layout = Layout {
        width: 2,
        height: 1,
        format: PixelFormat::Indexed4,
    };
    assert_eq!(encode(&[1, 2], layout).unwrap(), [0x21]);
}
