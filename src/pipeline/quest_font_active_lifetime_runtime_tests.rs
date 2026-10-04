use super::*;
use serde_json::json;

const TEST_IMAGE_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn ram_triplet() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let before = vec![0; PSX_MAIN_RAM_LEN];
    let mut after = before.clone();
    after[psx_main_ram_offset(active_runtime(1).unwrap(), 1).unwrap()] = 1;
    let settled = before.clone();
    (before, after, settled)
}

fn events() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "dropped": 0,
        "events": [
            {
                "pc": QUEST_FONT_STATE_RESET_RUNTIME,
                "registers": {
                    "v0": QUEST_FONT_STATE_BASE_RUNTIME,
                    "v1": 0,
                    "a0": 0
                }
            },
            {
                "pc": QUEST_FONT_ACTIVE_SET_RUNTIME,
                "registers": {
                    "v0": QUEST_FONT_STATE_BASE_RUNTIME + QUEST_FONT_STATE_RECORD_STRIDE as u32,
                    "v1": 1,
                    "a0": QUEST_FONT_STATE_BASE_RUNTIME
                }
            },
            {
                "pc": QUEST_FONT_ACTIVE_CLEAR_RUNTIME,
                "registers": {
                    "v0": 255,
                    "v1": 255,
                    "a0": QUEST_FONT_STATE_BASE_RUNTIME + QUEST_FONT_STATE_RECORD_STRIDE as u32
                }
            }
        ]
    }))
    .unwrap()
}

#[test]
fn active_runtime은_열_state의_정확한_byte를_가리킨다() {
    assert_eq!(active_runtime(0), Some(0x800E_F627));
    assert_eq!(active_runtime(1), Some(0x800E_F693));
    assert_eq!(active_runtime(9), Some(0x800E_F9F3));
    assert_eq!(active_runtime(10), None);
}

#[test]
fn producer_한_명령은_state_1_active만_바꾼다() {
    let (before, after, settled) = ram_triplet();
    let active_offset = psx_main_ram_offset(active_runtime(1).unwrap(), 1).unwrap();

    assert_eq!(diff_offsets(&before, &after), [active_offset]);
    assert!(active_values(&before)
        .unwrap()
        .iter()
        .all(|value| *value == 0));
    assert_eq!(
        active_values(&after).unwrap(),
        [0, 1, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert!(active_values(&settled)
        .unwrap()
        .iter()
        .all(|value| *value == 0));
}

#[test]
fn exact_producer와_clear_수명을_승인한다() {
    let (before, after, settled) = ram_triplet();

    let report = verify_bound(ActiveLifetimeEvidence {
        schema_version: QUEST_FONT_ACTIVE_LIFETIME_RUNTIME_SCHEMA_VERSION,
        image_sha256: TEST_IMAGE_SHA256,
        before_ram: &before,
        after_ram: &after,
        settled_ram: &settled,
        events_raw: &events(),
        event_sequence: ActiveEventSequence::ResetProducerClear,
        route_naturally_reached: false,
    })
    .unwrap();

    assert_eq!(report.active_state_index, 1);
    assert_eq!(report.active_runtime, 0x800E_F693);
    assert_eq!(report.producer_diff_bytes, 1);
    assert!(report.active_lifetime_verified);
    assert!(!report.route_naturally_reached);
    assert!(!report.release_eligible);
}

#[test]
fn producer와_clear_event_순서를_읽는다() {
    let (parsed, dropped) = parse_events(&events(), 3).unwrap();

    assert_eq!(dropped, 0);
    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed[0].pc, QUEST_FONT_STATE_RESET_RUNTIME);
    assert_eq!(parsed[1].pc, QUEST_FONT_ACTIVE_SET_RUNTIME);
    assert_eq!(
        parsed[1].v0,
        QUEST_FONT_STATE_BASE_RUNTIME + QUEST_FONT_STATE_RECORD_STRIDE as u32
    );
    assert_eq!(parsed[2].pc, QUEST_FONT_ACTIVE_CLEAR_RUNTIME);
    assert_eq!(
        parsed[2].a0,
        QUEST_FONT_STATE_BASE_RUNTIME + QUEST_FONT_STATE_RECORD_STRIDE as u32
    );
}

#[test]
fn event_drop과_순서_누락을_거부한다() {
    let dropped = serde_json::to_vec(&json!({"dropped": 1, "events": []})).unwrap();
    let short = serde_json::to_vec(&json!({"dropped": 0, "events": []})).unwrap();

    assert!(parse_events(&dropped, 3).is_err());
    assert!(parse_events(&short, 3).is_err());
}

#[test]
fn 잘못된_image_digest형식을_거부한다() {
    let (before, after, settled) = ram_triplet();
    let error = verify_bound(ActiveLifetimeEvidence {
        schema_version: QUEST_FONT_ACTIVE_LIFETIME_RUNTIME_SCHEMA_VERSION,
        image_sha256: "not-a-sha256",
        before_ram: &before,
        after_ram: &after,
        settled_ram: &settled,
        events_raw: &events(),
        event_sequence: ActiveEventSequence::ResetProducerClear,
        route_naturally_reached: false,
    })
    .unwrap_err();

    assert!(error.to_string().contains("image SHA-256 형식 불일치"));
}

#[test]
fn active밖의_producer_diff와_남은_active를_거부한다() {
    let (before, mut after, mut settled) = ram_triplet();
    after[0] = 1;
    let diff_error = verify_bound(ActiveLifetimeEvidence {
        schema_version: QUEST_FONT_ACTIVE_LIFETIME_RUNTIME_SCHEMA_VERSION,
        image_sha256: TEST_IMAGE_SHA256,
        before_ram: &before,
        after_ram: &after,
        settled_ram: &settled,
        events_raw: &events(),
        event_sequence: ActiveEventSequence::ResetProducerClear,
        route_naturally_reached: false,
    })
    .unwrap_err();
    assert!(diff_error.to_string().contains("diff_exact=false"));

    let (_, after, _) = ram_triplet();
    settled[psx_main_ram_offset(active_runtime(1).unwrap(), 1).unwrap()] = 1;
    let settled_error = verify_bound(ActiveLifetimeEvidence {
        schema_version: QUEST_FONT_ACTIVE_LIFETIME_RUNTIME_SCHEMA_VERSION,
        image_sha256: TEST_IMAGE_SHA256,
        before_ram: &before,
        after_ram: &after,
        settled_ram: &settled,
        events_raw: &events(),
        event_sequence: ActiveEventSequence::ResetProducerClear,
        route_naturally_reached: false,
    })
    .unwrap_err();
    assert!(settled_error.to_string().contains("settled_inactive=false"));
}

fn natural_events() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "dropped": 0,
        "events": [
            {
                "pc": QUEST_FONT_ACTIVE_SET_RUNTIME,
                "registers": {
                    "v0": QUEST_FONT_STATE_BASE_RUNTIME + QUEST_FONT_STATE_RECORD_STRIDE as u32,
                    "v1": 1,
                    "a0": QUEST_FONT_STATE_BASE_RUNTIME
                }
            },
            {
                "pc": QUEST_FONT_ACTIVE_CLEAR_RUNTIME,
                "registers": {
                    "v0": 255,
                    "v1": 255,
                    "a0": QUEST_FONT_STATE_BASE_RUNTIME + QUEST_FONT_STATE_RECORD_STRIDE as u32
                }
            }
        ]
    }))
    .unwrap()
}

fn natural_route(
    image_sha256: &str,
    checkpoint: &[u8],
    message_png: &[u8],
    choice_png: &[u8],
) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": QUEST_FONT_ACTIVE_NATURAL_RUNTIME_SCHEMA_VERSION,
        "image_sha256": image_sha256,
        "checkpoint_sha256": digest(checkpoint),
        "message_png_sha256": digest(message_png),
        "choice_png_sha256": digest(choice_png),
        "tracking_run_id": "01KZWXZRGEZ1J66EYYFXG27PDR",
        "producer_frame": 23422,
        "producer_samples": 2,
        "origin_load_state": true,
        "gamepad_input_only_after_checkpoint": true,
        "memory_write_count_after_checkpoint": 0,
        "natural_event": "random_encounter_choice"
    }))
    .unwrap()
}

#[test]
fn input_only_자연인카운터를_별도승인한다() {
    let checkpoint = b"checkpoint";
    let message_png = b"message";
    let choice_png = b"choice";
    let route = natural_route(TEST_IMAGE_SHA256, checkpoint, message_png, choice_png);
    let (before, after, settled) = ram_triplet();

    let report = verify_bound(ActiveLifetimeEvidence {
        schema_version: QUEST_FONT_ACTIVE_NATURAL_RUNTIME_SCHEMA_VERSION,
        image_sha256: TEST_IMAGE_SHA256,
        before_ram: &before,
        after_ram: &after,
        settled_ram: &settled,
        events_raw: &natural_events(),
        event_sequence: ActiveEventSequence::ProducerClear,
        route_naturally_reached: true,
    })
    .unwrap();
    assert!(report.route_naturally_reached);
    assert_eq!(report.event_count, 2);

    let route = verify_natural_route_evidence(
        TEST_IMAGE_SHA256,
        checkpoint,
        &route,
        message_png,
        choice_png,
    )
    .unwrap();
    assert_eq!(route.producer_samples, 2);
    assert_eq!(route.memory_write_count_after_checkpoint, 0);
}

#[test]
fn 자연경로의_memory_write선언을_거부한다() {
    let mut route: Value = serde_json::from_slice(&natural_route(
        TEST_IMAGE_SHA256,
        b"checkpoint",
        b"message",
        b"choice",
    ))
    .unwrap();
    route["memory_write_count_after_checkpoint"] = json!(1);

    let raw = serde_json::to_vec(&route).unwrap();
    let error = verify_natural_route_evidence(
        TEST_IMAGE_SHA256,
        b"checkpoint",
        &raw,
        b"message",
        b"choice",
    )
    .unwrap_err();
    assert!(error.to_string().contains("경로 선언"));
}
