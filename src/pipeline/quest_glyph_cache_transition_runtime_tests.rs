use super::*;

#[test]
fn entry_state는_exact_hook_abi를_읽는다() {
    let body_runtime = 0x8011_8500;
    let raw = format!(
        r#"{{"g0.PC":{},"g0.a0":2148290104,"g0.gp":{},"g0.ra":{},"g0.sp":2149580656}}"#,
        body_runtime, QUEST_HOOK_GP_RUNTIME, QUEST_BODY_RETURN_RUNTIME
    );
    let entry = parse_entry_state(raw.as_bytes(), body_runtime).expect("exact entry state");
    assert_eq!(entry.object_runtime, 0x800C_4E38);
    assert_eq!(entry.gp, QUEST_HOOK_GP_RUNTIME);
    assert_eq!(entry.ra, QUEST_BODY_RETURN_RUNTIME);
    assert_eq!(entry.sp, 0x801F_FF70);
}

#[test]
fn entry_state의_다른_pc를_거부한다() {
    let body_runtime = 0x8011_8500;
    let raw = format!(
        r#"{{"g0.PC":0,"g0.a0":2148290104,"g0.gp":{},"g0.ra":{},"g0.sp":2149580656}}"#,
        QUEST_HOOK_GP_RUNTIME, QUEST_BODY_RETURN_RUNTIME
    );
    let error = parse_entry_state(raw.as_bytes(), body_runtime).expect_err("다른 PC는 실패");
    assert!(error.to_string().contains("entry register 계약 불일치"));
}

#[test]
fn png_shape는_정확한_quest_capture만_받는다() {
    let mut png = vec![0; 24];
    png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    png[12..16].copy_from_slice(b"IHDR");
    png[16..20].copy_from_slice(&PNG_WIDTH.to_be_bytes());
    png[20..24].copy_from_slice(&PNG_HEIGHT.to_be_bytes());
    assert!(png_shape_exact(&png));
    png[20..24].copy_from_slice(&239u32.to_be_bytes());
    assert!(!png_shape_exact(&png));
}

#[test]
fn lifecycle_event_분모는_page별_upload수에서_계산한다() {
    assert_eq!(
        expected_lifecycle_event_count(&[2, 0, 3, 4]).expect("event count"),
        22
    );
}

#[test]
fn 네_record_runtime은_town00_base에서_계산한다() {
    assert_eq!(
        record_runtime(QUEST_GLYPH_CACHE_UPLOAD_POC_RECORD_OFFSET).expect("first"),
        0x8019_BC3C
    );
    assert_eq!(
        record_runtime(QUEST_GLYPH_CACHE_TRANSITION_SECOND_RECORD_OFFSET).expect("second"),
        0x8019_BD4C
    );
    assert_eq!(
        record_runtime(QUEST_GLYPH_CACHE_TRANSITION_THIRD_RECORD_OFFSET).expect("third"),
        0x8019_D09C
    );
    assert_eq!(
        record_runtime(QUEST_GLYPH_CACHE_TRANSITION_RESTORE_RECORD_OFFSET).expect("restore"),
        0x8019_D160
    );
}
