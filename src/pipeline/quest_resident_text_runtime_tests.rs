use super::*;
use serde_json::json;

fn slot(selection_renderer_runtime: u32, input_indicator_runtime: u32) -> QuestFontWindowSlot {
    QuestFontWindowSlot {
        index: 2,
        state_runtime: 0x800E_F000,
        parent_runtime: 0x800B_E000,
        resource_selector: 0,
        window_resource: 0,
        window_cell: 0,
        active: 0,
        origin_x: 0,
        origin_y: 0,
        cursor_x: 0,
        cursor_y: 0,
        cell_width: 12,
        cell_height: 16,
        line_height: 16,
        glyph_spacing: 0,
        line_spacing: 0,
        input_indicator_runtime,
        selection_renderer_runtime,
        geometry: Some(QuestFontWindowGeometry {
            parent_x_fixed12: 80 << 12,
            parent_y_fixed12: 144 << 12,
            parent_x_px: 80,
            parent_y_px: 144,
            parent_width_px: 96,
            parent_height_px: 48,
            frame_origin_x_px: 72,
            frame_origin_y_px: 136,
        }),
    }
}

#[test]
fn window는_좌표가_아니라_화면역할로_고른다() {
    let census = QuestFontWindowCensus {
        slots: vec![slot(0, 0), slot(0x800B_FCD8, 0)],
        active_windows: 0,
        selection_renderer_slots: 1,
        geometry_slots: 2,
    };

    let observed = select_window(&census, QuestResidentTextWindowRole::Selection).unwrap();

    assert_eq!(observed.slot_index, 2);
    assert_eq!(observed.geometry.parent_width_px, 96);
}

#[test]
fn 같은_역할의_window가_둘이면_geometry를_임의로_고르지_않는다() {
    let census = QuestFontWindowCensus {
        slots: vec![slot(1, 0), slot(2, 0)],
        active_windows: 0,
        selection_renderer_slots: 2,
        geometry_slots: 2,
    };

    assert!(select_window(&census, QuestResidentTextWindowRole::Selection).is_err());
}

#[test]
fn parent_geometry가_없는_window는_runtime_geometry로_승격하지_않는다() {
    let mut missing = slot(0, 1);
    missing.geometry = None;
    let census = QuestFontWindowCensus {
        slots: vec![missing],
        active_windows: 0,
        selection_renderer_slots: 0,
        geometry_slots: 0,
    };

    assert!(select_window(&census, QuestResidentTextWindowRole::InputIndicator).is_err());
}

fn resident_event_document(include_upload: bool) -> Vec<u8> {
    let mut events = vec![json!({
        "pc": 30,
        "registers": {
            "s0": 0x8008_0000_u64,
            "s1": 7,
            "t0": QUEST_PAGE_HOOK_STATE_RUNTIME,
            "t1": 7,
        },
    })];
    if include_upload {
        events.push(json!({
            "pc": QUEST_FONT_UPLOAD_TARGET_RUNTIME,
            "registers": {"ra": 11},
        }));
    }
    events.push(json!({"pc": 40, "registers": {}}));
    serde_json::to_vec(&json!({"dropped": 0, "events": events})).unwrap()
}

fn resident_event_points() -> HookRuntimePoints {
    HookRuntimePoints {
        upload_return: 11,
        sync_return: 21,
        primary_commit: 50,
        secondary_commit: 60,
        primary_resident: 30,
        return_to_consumer: 40,
    }
}

#[test]
fn 같은_page의_primary_resident_hit은_upload없이_return해야_한다() {
    let evidence = verify_farewell_event_path_with_points(
        resident_event_points(),
        0x8008_0000,
        7,
        &resident_event_document(false),
    )
    .unwrap();

    assert_eq!(evidence.primary_resident_events, 1);
    assert_eq!(evidence.load_image_calls, 0);
    assert_eq!(evidence.primary_commit_events, 0);
}

#[test]
fn resident_hit과_hook_upload가_함께_있으면_reuse로_승격하지_않는다() {
    assert!(verify_farewell_event_path_with_points(
        resident_event_points(),
        0x8008_0000,
        7,
        &resident_event_document(true),
    )
    .is_err());
}
