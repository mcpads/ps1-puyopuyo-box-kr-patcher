use super::*;
use serde_json::json;

fn event(pc: u32, a0: u32, gp: u32, ra: u32, sp: u32, v0: u32) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "dropped": 0,
        "events": [{
            "pc": pc,
            "registers": {
                "a0": a0,
                "gp": gp,
                "ra": ra,
                "sp": sp,
                "v0": v0,
            }
        }]
    }))
    .unwrap()
}

#[test]
fn collision_event는_단일_hit의_필수_register를_읽는다() {
    let parsed = parse_single_event(&event(1, 2, 3, 4, 5, 6), "test").unwrap();

    assert_eq!(
        parsed,
        CollisionEvent {
            pc: 1,
            a0: 2,
            gp: 3,
            ra: 4,
            sp: 5,
            v0: 6,
        }
    );
}

#[test]
fn collision_event_drop과_복수_hit는_실패한다() {
    let dropped = serde_json::to_vec(&json!({
        "dropped": 1,
        "events": [{"pc": 1, "registers": {}}]
    }))
    .unwrap();
    let multiple = serde_json::to_vec(&json!({
        "dropped": 0,
        "events": [
            {"pc": 1, "registers": {}},
            {"pc": 2, "registers": {}}
        ]
    }))
    .unwrap();

    assert!(parse_single_event(&dropped, "test").is_err());
    assert!(parse_single_event(&multiple, "test").is_err());
}

#[test]
fn diff_offset은_바뀐_byte만_오름차순으로_보고한다() {
    assert_eq!(diff_offsets(&[0, 1, 2, 3], &[0, 9, 2, 8]), [1, 3]);
}

#[test]
fn collision_runtime_상수는_source_verified_abi와_같다() {
    assert_eq!(QUEST_FONT_STATE_FIRST_ACTIVE_RUNTIME, 0x800E_F627);
    assert_eq!(QUEST_GLYPH_CACHE_COLLISION_YIELD_RUNTIME, 0x800B_E4A0);
    assert_eq!(QUEST_GLYPH_CACHE_COLLISION_STREAM_RUNTIME, 0x8019_BC3C);
    assert_eq!(QUEST_GLYPH_CACHE_BODY_RETURN_RUNTIME, 0x8002_25E0);
    assert_eq!(
        TOWN00_SEQ_RUNTIME_BASE + QUEST_GLYPH_CACHE_TRANSITION_THIRD_RECORD_OFFSET as u32,
        0x8019_D09C
    );
    assert_eq!(QUEST_GLYPH_CACHE_TRANSITION_THIRD_PAGE_ID, 50);
}

#[test]
fn active_개입은_여러_font_state를_정확히_식별한다() {
    let pristine = vec![0; PSX_MAIN_RAM_LEN];
    let mut forced = pristine.clone();
    for index in [0, 7] {
        let runtime = font_active_runtime(index).unwrap();
        let offset = psx_main_ram_offset(runtime, 1).unwrap();
        forced[offset] = 1;
    }

    assert_eq!(
        active_intervention_indices(&pristine, &forced).unwrap(),
        [0, 7]
    );
}

#[test]
fn active_개입은_font_state_밖의_diff를_거부한다() {
    let pristine = vec![0; PSX_MAIN_RAM_LEN];
    let mut forced = pristine.clone();
    let active_offset = psx_main_ram_offset(font_active_runtime(0).unwrap(), 1).unwrap();
    forced[active_offset] = 1;
    forced[active_offset + 1] = 1;

    assert!(active_intervention_indices(&pristine, &forced).is_err());
}

#[test]
fn active_개입은_빈_집합을_거부한다() {
    let pristine = vec![0; PSX_MAIN_RAM_LEN];

    assert!(active_intervention_indices(&pristine, &pristine).is_err());
}
