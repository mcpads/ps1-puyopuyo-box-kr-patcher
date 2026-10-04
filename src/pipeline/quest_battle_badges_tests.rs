use super::*;

#[test]
fn 공통_네_셀만_복사하고_상쇄는_quest_좌표로_옮긴다() {
    let mut quest = vec![7; 256 * 128];
    let mut translated = vec![9; 256 * 126];
    for row in translated[88 * 256..104 * 256].chunks_exact_mut(256) {
        row[48..80].fill(11);
    }
    copy_badge_pixels(&mut quest, &translated);
    for y in 0..128 {
        for x in 0..256 {
            let expected = if (x < 96 && y < 48)
                || ((96..192).contains(&x) && y < 56)
                || (x < 80 && (64..88).contains(&y))
            {
                9
            } else if (160..192).contains(&x) && (88..104).contains(&y) {
                11
            } else {
                7
            };
            assert_eq!(quest[y * 256 + x], expected, "({x}, {y})");
        }
    }
}

#[test]
fn 크기가_같아도_다른_quest_원본은_합성하지_않는다() {
    assert!(compose(&vec![0; 16384], &vec![0; 16128]).is_err());
}
