use super::*;

#[test]
fn cel_stem은_같은_btx를_고르고_sket만_별칭이다() {
    assert_eq!(
        texture_name_for_cel("ARCH.CEL"),
        Ok(("ARCH.BTX".to_string(), false))
    );
    assert_eq!(
        texture_name_for_cel("sket.cel"),
        Ok(("SKE.BTX".to_string(), true))
    );
}

#[test]
fn cel_확장자가_아닌_이름은_거부한다() {
    assert!(matches!(
        texture_name_for_cel("G_MESS.BTX"),
        Err(TuuCelTextureError::UnsupportedCelName(_))
    ));
}
