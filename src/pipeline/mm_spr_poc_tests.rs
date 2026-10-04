use super::*;

#[test]
fn control과_한글_산출물_이름을_구분한다() {
    assert_eq!(MmSprPocMode::Control.label(), "control");
    assert_eq!(MmSprPocMode::Korean.label(), "ko");
    assert_eq!(
        MmSprPocScope::FirstDescription.label(),
        "shell-first-description"
    );
    assert_eq!(
        MmSprPocScope::ApprovedDescriptions.label(),
        "shell-approved-descriptions"
    );
}
