use super::*;

#[test]
fn banner_pixels_preserve_every_byte_outside_the_declared_mask() {
    let original = vec![7; WIDTH * 256];
    let changed = apply_rows(
        &original,
        "y\tx\texpected_hex\treplacement_hex\n28\t70\t0707\t0809\n",
    )
    .unwrap();
    for (index, (&before, &after)) in original.iter().zip(&changed).enumerate() {
        if (28 * WIDTH + 70..28 * WIDTH + 72).contains(&index) {
            assert_ne!(before, after);
        } else {
            assert_eq!(before, after);
        }
    }
}

#[test]
fn banner_rejects_source_mismatch_overlap_and_cake_or_flower_writes() {
    let original = vec![7; WIDTH * 256];
    for rows in [
        "28\t70\t00\t08",
        "28\t70\t07\t08\n28\t70\t07\t09",
        "34\t194\t07\t08",
        "27\t70\t07\t08",
        "28\t69\t07\t08",
        "28\t195\t0707\t0809",
    ] {
        assert!(apply_rows(
            &original,
            &format!("y\tx\texpected_hex\treplacement_hex\n{rows}\n")
        )
        .is_err());
    }
}
