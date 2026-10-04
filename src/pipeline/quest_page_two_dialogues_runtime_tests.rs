use super::*;
use serde_json::json;

fn events(pcs: &[u32], dropped: u64) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "dropped": dropped,
        "events": pcs
            .iter()
            .map(|pc| json!({
                "pc": pc,
                "registers": {
                    "a0": 1,
                    "a1": 2,
                    "gp": 3,
                    "ra": 4,
                    "sp": 5,
                }
            }))
            .collect::<Vec<_>>()
    }))
    .unwrap()
}

#[test]
fn event_parser는_순서와_register를_보존한다() {
    let parsed = parse_events(&events(&[10, 20], 0), "test").unwrap();

    assert_eq!(
        parsed,
        [
            RuntimeEvent {
                pc: 10,
                a0: 1,
                a1: 2,
                gp: 3,
                ra: 4,
                sp: 5,
            },
            RuntimeEvent {
                pc: 20,
                a0: 1,
                a1: 2,
                gp: 3,
                ra: 4,
                sp: 5,
            },
        ]
    );
}

#[test]
fn event_drop은_실패한다() {
    assert!(parse_events(&events(&[10], 1), "test").is_err());
}

#[test]
fn png_shape는_280x240_ihdr만_받는다() {
    let mut png = vec![0; 24];
    png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    png[12..16].copy_from_slice(b"IHDR");
    png[16..20].copy_from_slice(&280u32.to_be_bytes());
    png[20..24].copy_from_slice(&240u32.to_be_bytes());

    assert!(png_shape_exact(&png));
    png[20..24].copy_from_slice(&224u32.to_be_bytes());
    assert!(!png_shape_exact(&png));
}

#[test]
fn 복귀_record는_town00_runtime_base와_결속된다() {
    assert_eq!(QUEST_PAGE_TWO_DIALOGUES_RESTORE_RECORD_OFFSET, 0x362C);
    assert_eq!(QUEST_PAGE_TWO_DIALOGUES_RESTORE_RECORD_RUNTIME, 0x8019_D09C);
}
