use super::*;

#[test]
fn body_entry는_exact_pc_gp와_main_ram_object만_받는다() {
    let raw = serde_json::to_vec(&serde_json::json!({
        "g0.PC": 0x8011_7664u32,
        "g0.gp": QUEST_HOOK_GP_RUNTIME,
        "g0.a0": 0x800C_4E38u32,
    }))
    .expect("JSON fixture");
    assert_eq!(parse_entry_state(&raw, 0x8011_7664), Ok(0x800C_4E38));

    let wrong_pc = serde_json::to_vec(&serde_json::json!({
        "g0.PC": 0x8011_7668u32,
        "g0.gp": QUEST_HOOK_GP_RUNTIME,
        "g0.a0": 0x800C_4E38u32,
    }))
    .expect("JSON fixture");
    assert!(parse_entry_state(&wrong_pc, 0x8011_7664)
        .expect_err("다른 PC를 거부해야 함")
        .to_string()
        .contains("body entry 불일치"));
}

#[test]
fn routed_target은_page와_분리된_little_endian_u16_word다() {
    assert_eq!(routed_target_word([0x21, 0x3E, 0x13, 0x16]), 0x1613);
}

#[test]
fn inline은_다음_본문이고_tail만_word_offset을_가진다() {
    assert!(route_target_matches(
        QuestTranslationSelectionPlacement::InlineCapacityCandidate,
        36,
        [0x21, 36, 0, 0],
        0x0A54,
    ));
    assert!(!route_target_matches(
        QuestTranslationSelectionPlacement::InlineCapacityCandidate,
        36,
        [0x21, 36, 0x95, 0x02],
        0x0A54,
    ));
    assert!(route_target_matches(
        QuestTranslationSelectionPlacement::RelocatedTailCapacityCandidate,
        36,
        [0x21, 36, 0x95, 0x02],
        0x0A54,
    ));
    assert!(route_target_matches(
        QuestTranslationSelectionPlacement::RetainSource,
        62,
        [0x21, 0, 0, 0],
        0x1C64,
    ));
}

#[test]
fn tsv는_synthetic와_natural_판정을_서로_다른_열로_보존한다() {
    let report = QuestDevelopmentRouteRuntimeReport {
        schema_version: QUEST_DEVELOPMENT_ROUTE_RUNTIME_SCHEMA_VERSION,
        image_sha256: "image".to_string(),
        sequence: "TOWN00.SEQ".to_string(),
        sequence_bytes: 4,
        sequence_sha256: "sequence".to_string(),
        stream_id: "stream".to_string(),
        placement: "placement",
        routing_page_id: 62,
        record_page_id: 0,
        record_offset: 4,
        record_runtime: 0x8019_9A74,
        routed_record: [0x21, 0, 0, 0],
        body_offset: 8,
        body_runtime: 0x8019_9A78,
        object_runtime: 0x800C_4E38,
        object_stream_runtime: 0x8019_9A74,
        entry_state_sha256: "state".to_string(),
        entry_ram_sha256: "ram".to_string(),
        visible_png_sha256: "png".to_string(),
        visible_png_width: PNG_WIDTH,
        visible_png_height: PNG_HEIGHT,
        sequence_loaded_exact: true,
        route_record_exact: true,
        body_entry_exact: true,
        object_stream_exact: true,
        visible_capture_shape_exact: true,
        synthetic_pointer_intervention: true,
        synthetic_route_verified: true,
        natural_reach_verified: false,
        translation_approved: false,
        layout_approved: false,
        product_eligible: false,
    };
    let rendered = render_quest_development_route_runtime_tsv(&report);
    let rows = rendered.lines().collect::<Vec<_>>();
    let header = rows[0].split('\t').collect::<Vec<_>>();
    let values = rows[1].split('\t').collect::<Vec<_>>();
    assert_eq!(header.len(), values.len());
    assert_eq!(
        values[header
            .iter()
            .position(|name| *name == "synthetic_route_verified")
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
