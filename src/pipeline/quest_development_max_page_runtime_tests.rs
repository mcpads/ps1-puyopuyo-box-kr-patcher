use super::*;

#[test]
fn entry와_return_state는_같은_hook_abi를_보존한다() {
    let entry_raw = serde_json::to_vec(&serde_json::json!({
        "g0.PC": 0x8011_7664u32,
        "g0.a0": 0x800C_4E38u32,
        "g0.gp": QUEST_HOOK_GP_RUNTIME,
        "g0.ra": BODY_RETURN_RUNTIME,
        "g0.sp": 0x801F_FF70u32,
    }))
    .expect("entry JSON");
    let entry = parse_entry_state(&entry_raw, 0x8011_7664).expect("exact entry");
    let return_raw = serde_json::to_vec(&serde_json::json!({
        "g0.PC": BODY_RETURN_RUNTIME,
        "g0.a0": entry.object_runtime,
        "g0.gp": entry.gp,
        "g0.ra": entry.ra,
        "g0.sp": entry.sp,
    }))
    .expect("return JSON");
    verify_return_state(&return_raw, entry).expect("exact return");
}

#[test]
fn return_state의_다른_object를_거부한다() {
    let entry = EntryState {
        object_runtime: 0x800C_4E38,
        gp: QUEST_HOOK_GP_RUNTIME,
        ra: BODY_RETURN_RUNTIME,
        sp: 0x801F_FF70,
    };
    let raw = serde_json::to_vec(&serde_json::json!({
        "g0.PC": BODY_RETURN_RUNTIME,
        "g0.a0": 0x800C_4E3Cu32,
        "g0.gp": entry.gp,
        "g0.ra": entry.ra,
        "g0.sp": entry.sp,
    }))
    .expect("return JSON");
    assert!(verify_return_state(&raw, entry)
        .expect_err("다른 object는 실패")
        .to_string()
        .contains("return register 계약 불일치"));
}

#[test]
fn visible_png는_quest_capture_shape를_읽는다() {
    let mut png = vec![0; 24];
    png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    png[12..16].copy_from_slice(b"IHDR");
    png[16..20].copy_from_slice(&PNG_WIDTH.to_be_bytes());
    png[20..24].copy_from_slice(&PNG_HEIGHT.to_be_bytes());
    assert_eq!(png_dimensions(&png), Some((PNG_WIDTH, PNG_HEIGHT)));
}

#[test]
fn tsv는_synthetic와_natural_경계를_분리한다() {
    let report = QuestDevelopmentMaxPageRuntimeReport {
        schema_version: QUEST_DEVELOPMENT_MAX_PAGE_RUNTIME_SCHEMA_VERSION,
        image_sha256: "image".to_string(),
        stream_id: "stream".to_string(),
        sequence: "SHEZ.SEQ".to_string(),
        placement: "inline-capacity-candidate",
        natural_record_runtime: 0x8019_A440,
        injection_record_runtime: 0x8019_BC3C,
        payload_bytes: 192,
        payload_sha256: "payload".to_string(),
        previous_page_id: 62,
        target_page_id: 31,
        max_page_glyphs: 162,
        object_runtime: 0x800C_4E38,
        entry_state_sha256: "entry-state".to_string(),
        entry_ram_sha256: "entry-ram".to_string(),
        return_state_sha256: "return-state".to_string(),
        return_ram_sha256: "return-ram".to_string(),
        return_gpu_sha256: "return-gpu".to_string(),
        lifecycle_events_sha256: "events".to_string(),
        visible_png_sha256: "visible".to_string(),
        visible_png_width: PNG_WIDTH,
        visible_png_height: PNG_HEIGHT,
        load_image_calls: 162,
        draw_sync_calls: 162,
        event_count: 325,
        dropped_events: 0,
        disc_sequence_exact: true,
        injected_town_sequence_exact: true,
        body_entry_exact: true,
        body_return_exact: true,
        page_transition_exact: true,
        resident_inputs_exact: true,
        uploaded_cells_exact: true,
        lifecycle_exact: true,
        visible_capture_shape_exact: true,
        synthetic_stream_intervention: true,
        synthetic_previous_page_intervention: true,
        runtime_verified: true,
        natural_reach_verified: false,
        translation_approved: false,
        layout_approved: false,
        product_eligible: false,
    };
    let rendered = render_quest_development_max_page_runtime_tsv(&report);
    let rows = rendered.lines().collect::<Vec<_>>();
    let header = rows[0].split('\t').collect::<Vec<_>>();
    let values = rows[1].split('\t').collect::<Vec<_>>();
    assert_eq!(header.len(), values.len());
    assert_eq!(
        values[header
            .iter()
            .position(|name| *name == "synthetic_stream_intervention")
            .expect("synthetic 열")],
        "true"
    );
    assert_eq!(
        values[header
            .iter()
            .position(|name| *name == "natural_reach_verified")
            .expect("natural 열")],
        "false"
    );
}
