use super::*;

#[test]
fn 세button은_서로다른조판폭과_slot보호경계를가진다() {
    assert_eq!(SOURCE_MASKS.len(), 3);
    assert_eq!(SOURCE_MASKS[0].editable.width, 40);
    assert_eq!(SOURCE_MASKS[1].editable.width, 40);
    assert_eq!(SOURCE_MASKS[2].editable.width, 25);
    assert_eq!(
        SOURCE_MASKS[2].editable.x + SOURCE_MASKS[2].editable.width,
        29
    );
}

#[test]
fn exact_mcrd01이아닌입력은_mask합성전에실패한다() {
    let invalid = vec![0; MCRD01_TEXT_ASSET.layout.byte_len().unwrap()];

    assert!(patch_mcrd01_development_surface(&invalid).is_err());
}
