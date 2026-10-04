use super::*;

#[test]
fn 같은거리에서는_행우선으로앞선donor를고른다() {
    let pixels = [10, 200, 200, 200, 20];
    let repair = [false, true, true, true, false];
    let donors = [true, false, false, false, true];

    let restored =
        restore_masked_pixels_from_nearest_donors(&pixels, 5, 1, &repair, &donors).unwrap();

    assert_eq!(restored, [10, 10, 10, 20, 20]);
}

#[test]
fn 복원대상밖pixel은donor가아니어도그대로보존한다() {
    let pixels = [10, 99, 200, 88, 20];
    let repair = [false, false, true, false, false];
    let donors = [true, false, false, false, true];

    let restored =
        restore_masked_pixels_from_nearest_donors(&pixels, 5, 1, &repair, &donors).unwrap();

    assert_eq!(restored, [10, 99, 10, 88, 20]);
}

#[test]
fn 복원대상과donor가겹치면실패한다() {
    let pixels = [10, 200, 20];
    let repair = [false, true, false];
    let donors = [true, true, false];

    assert!(matches!(
        restore_masked_pixels_from_nearest_donors(&pixels, 3, 1, &repair, &donors),
        Err(SourceBackgroundError::RepairPixelIsDonor { x: 1, y: 0 })
    ));
}
