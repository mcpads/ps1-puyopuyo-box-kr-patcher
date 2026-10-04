use crate::script::audit_subgame_seq_text_candidates;
use crate::text::encode_sjis;

use super::*;

const ARCHIVE: &str = "/PUYO1/PP1_SEQ.VDD;1";

fn overlay(payload: &[u8], base: u32) -> Vec<u8> {
    assert!(payload.len().is_multiple_of(4));
    let initialized_len = payload.len() as u32;
    let bss_addr = base + initialized_len;
    let words = [
        0,
        0,
        0,
        base,
        initialized_len,
        bss_addr,
        0x2C,
        bss_addr + 0x30,
        0,
        0x8000,
        0x0080_0000,
    ];
    let mut raw = payload.to_vec();
    for word in words {
        raw.extend_from_slice(&word.to_le_bytes());
    }
    raw
}

#[test]
fn 정확한_footer만_overlay로_분류한다() {
    let raw = overlay(&[1, 2, 3, 4, 5, 6, 7, 8], 0x801B_5270);
    let footer = parse_subgame_seq_overlay_footer(&raw).unwrap().unwrap();

    assert_eq!(footer.footer_offset, 8);
    assert_eq!(footer.initialized_len, 8);
    assert_eq!(footer.bss_addr, 0x801B_5278);
    assert_eq!(footer.next_addr, 0x801B_52A8);
    assert_eq!(
        footer.runtime_map().unwrap().file_offset(0x801B_5274),
        Some(4)
    );
}

#[test]
fn signature가_없는_짧은_파일은_raw로_남긴다() {
    assert_eq!(parse_subgame_seq_overlay_footer(&[0; 12]).unwrap(), None);
    assert_eq!(parse_subgame_seq_overlay_footer(&[0; 48]).unwrap(), None);
}

#[test]
fn signature만_있고_계약이_깨진_footer는_거부한다() {
    let mut raw = overlay(&[0; 8], 0x801B_5270);
    let bss_len_offset = raw.len() - SUBGAME_SEQ_OVERLAY_FOOTER_LEN + 6 * 4;
    raw[bss_len_offset..bss_len_offset + 4].copy_from_slice(&0x30u32.to_le_bytes());

    assert!(parse_subgame_seq_overlay_footer(&raw).is_err());
}

#[test]
fn text_summary와_layout을_원본_digest로_교차한다() {
    let overlay = overlay(&[0; 8], 0x801B_5270);
    let raw = [1, 2, 3, 4];
    let text = audit_subgame_seq_text_candidates([
        (ARCHIVE, "OVERLAY.SEQ", overlay.as_slice()),
        (ARCHIVE, "RAW.SEQ", raw.as_slice()),
    ])
    .unwrap();
    let analysis = audit_subgame_seq_layouts(
        [
            (ARCHIVE, "RAW.SEQ", raw.as_slice()),
            (ARCHIVE, "OVERLAY.SEQ", overlay.as_slice()),
        ],
        &text,
    )
    .unwrap();

    assert_eq!(analysis.overlays, 1);
    assert_eq!(analysis.raw_opaque, 1);
    assert_eq!(analysis.load_base_counts.get(&0x801B_5270), Some(&1));
}

#[test]
fn raw의_가나_run을_조용히_no_text로_축약하지_않는다() {
    let raw = encode_sjis("アルル").unwrap();
    let text = audit_subgame_seq_text_candidates([(ARCHIVE, "RAW.SEQ", raw.as_slice())]).unwrap();

    assert!(audit_subgame_seq_layouts([(ARCHIVE, "RAW.SEQ", raw.as_slice())], &text).is_err());
}

#[test]
fn 내부_pointer는_field와_target을_모두_initialized범위로_검증한다() {
    let base = 0x801B_5270u32;
    let mut raw = vec![0; 16];
    raw[4..8].copy_from_slice(&(base + 12).to_le_bytes());
    let map = RuntimeAssetMap::new(base, raw.len() as u32).unwrap();

    assert_eq!(
        resolve_internal_pointer(&raw, map, 4).unwrap(),
        (12, base + 12)
    );
    raw[4..8].copy_from_slice(&(base + 16).to_le_bytes());
    assert!(resolve_internal_pointer(&raw, map, 4).is_err());
    assert!(resolve_internal_pointer(&raw, map, 16).is_err());
}
