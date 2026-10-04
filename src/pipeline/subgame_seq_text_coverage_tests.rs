use super::*;

fn owners(value: &str) -> Vec<String> {
    vec![value.to_string()]
}

#[test]
fn scene_message는_repertoire_overlap과_보호_copy보다_우선한다() {
    assert_eq!(
        classify(0, &[], &[], &owners("message"), &[], &owners("min")),
        Ok(SubgameSeqTextCoverageDisposition::SceneMessage)
    );
    assert_eq!(
        classify(
            0,
            &[],
            &[],
            &owners("message"),
            &owners("repertoire"),
            &owners("min")
        ),
        Ok(SubgameSeqTextCoverageDisposition::SceneMessageInsideRepertoire)
    );
}

#[test]
fn 확인된_owner가_없으면_u16_정렬을_별도_판정한다() {
    assert_eq!(
        classify(3, &[], &[], &[], &[], &[]),
        Ok(SubgameSeqTextCoverageDisposition::MisalignedForConfirmedU16Consumers)
    );
    assert_eq!(
        classify(2, &[], &[], &[], &[], &[]),
        Ok(SubgameSeqTextCoverageDisposition::AlignedUnresolved)
    );
}

#[test]
fn e6와_scene_owner의_중첩은_실패한다() {
    assert!(classify(0, &owners("e6"), &[], &owners("message"), &[], &[]).is_err());
}

#[test]
fn custom_record_owner는_정렬_미확정보다_우선하며_중첩은_실패한다() {
    assert_eq!(
        classify(2, &[], &owners("custom"), &[], &[], &[]),
        Ok(SubgameSeqTextCoverageDisposition::CustomByteRecord)
    );
    assert!(classify(2, &owners("e6"), &owners("custom"), &[], &[], &[]).is_err());
}
