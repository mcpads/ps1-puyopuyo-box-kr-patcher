use super::*;

fn draw_events(rows: &[(u32, u32)], dropped: u64) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "dropped": dropped,
        "events": rows
            .iter()
            .map(|(object, texture_page)| serde_json::json!({
                "pc": 0x8002_B8C4u32,
                "registers": {
                    "s1": object,
                    "v0": texture_page,
                }
            }))
            .collect::<Vec<_>>(),
    }))
    .expect("event JSON")
}

#[test]
fn 두_bank_draw는_object_marker마다_서로_다른_texture_page를_요구한다() {
    let objects = [
        LiveObject {
            runtime: 0x800B_E998,
            marker: 52,
        },
        LiveObject {
            runtime: 0x800B_EA78,
            marker: 0x9E,
        },
    ];
    let raw = draw_events(
        &[
            (
                objects[0].runtime,
                u32::from(QUEST_GLYPH_CACHE_KOREAN_TEXTURE_PAGE),
            ),
            (
                objects[1].runtime,
                u32::from(QUEST_GLYPH_CACHE_SECONDARY_TEXTURE_PAGE),
            ),
        ],
        0,
    );

    let evidence =
        verify_draw_events_at(&raw, 0x8002_B8C4, &objects, 52, 0x9E).expect("두 bank 소비");

    assert_eq!(evidence.primary, 1);
    assert_eq!(evidence.secondary, 1);
    assert_eq!(evidence.dropped, 0);
}

#[test]
fn secondary_marker를_primary_texture_page로_그리면_거부한다() {
    let objects = [
        LiveObject {
            runtime: 0x800B_E998,
            marker: 52,
        },
        LiveObject {
            runtime: 0x800B_EA78,
            marker: 0x9E,
        },
    ];
    let raw = draw_events(
        &[
            (
                objects[0].runtime,
                u32::from(QUEST_GLYPH_CACHE_KOREAN_TEXTURE_PAGE),
            ),
            (
                objects[1].runtime,
                u32::from(QUEST_GLYPH_CACHE_KOREAN_TEXTURE_PAGE),
            ),
        ],
        0,
    );

    let error =
        verify_draw_events_at(&raw, 0x8002_B8C4, &objects, 52, 0x9E).expect_err("오류 tpage");

    assert!(error.to_string().contains("expected_tpage=0x003F"));
}

#[test]
fn event_drop은_완전한_draw_증거가_아니다() {
    let objects = [
        LiveObject {
            runtime: 0x800B_E998,
            marker: 52,
        },
        LiveObject {
            runtime: 0x800B_EA78,
            marker: 0x9E,
        },
    ];
    let raw = draw_events(
        &[
            (
                objects[0].runtime,
                u32::from(QUEST_GLYPH_CACHE_KOREAN_TEXTURE_PAGE),
            ),
            (
                objects[1].runtime,
                u32::from(QUEST_GLYPH_CACHE_SECONDARY_TEXTURE_PAGE),
            ),
        ],
        1,
    );

    let error =
        verify_draw_events_at(&raw, 0x8002_B8C4, &objects, 52, 0x9E).expect_err("dropped event");

    assert!(error.to_string().contains("dropped=1"));
}

#[test]
fn cold_route와_state_load는_서로_배타적인_provenance로_받는다() {
    let cold = serde_json::json!({
        "state_load_intervention": false,
        "cold_route_verified": true,
    });
    let loaded = serde_json::json!({
        "state_load_intervention": true,
        "cold_route_verified": false,
    });

    assert!(verify_route_provenance(&cold).expect("cold route"));
    assert!(!verify_route_provenance(&loaded).expect("state-load route"));
}

#[test]
fn route_provenance가_둘_다_같으면_거부한다() {
    for value in [false, true] {
        let ambiguous = serde_json::json!({
            "state_load_intervention": value,
            "cold_route_verified": value,
        });

        let error = verify_route_provenance(&ambiguous).expect_err("모순 provenance");
        assert!(error.to_string().contains("배타적이지 않음"));
    }
}
