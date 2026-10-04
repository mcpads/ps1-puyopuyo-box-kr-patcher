use super::*;

#[test]
fn control_인자와_glyph를_분리하고_ff02_구간을_보존한다() {
    let mut values: Vec<u16> = vec![0xFF17, 1, 83, 84, 0xFF02, 0xFF04, 120, 85, 86, 87];
    values.resize(STREAM_BYTES / 2 - 4, 1);
    values.extend([0xFF00, 0, 0, 0]);
    let raw = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let analysis = parse_stream(&raw).unwrap();

    assert_eq!(analysis.segments.len(), 2);
    assert_eq!(analysis.segments[0].source, "連鎖");
    assert!(analysis.segments[1].source.starts_with("高得点"));
    assert_eq!(analysis.control_tokens[&0xFF17], 1);
    assert_eq!(analysis.control_tokens[&0xFF04], 1);
    assert_eq!(analysis.control_tokens[&0xFF02], 1);
    assert_eq!(analysis.control_tokens[&0xFF00], 1);
    assert_eq!(serialize_menu06_text_tokens(&analysis.tokens).unwrap(), raw);
    assert_eq!(analysis.rewrite_capacity.stream_words, 396);
    assert_eq!(
        analysis.rewrite_capacity.glyph_positions,
        analysis.glyph_tokens
    );
    assert_eq!(analysis.rewrite_capacity.slack_words, 0);
}

#[test]
fn glyph_보호원문은_실제_연속_cell을_복원한다() {
    assert_eq!(MENU06_GLYPH_SOURCES[83], "連");
    assert_eq!(MENU06_GLYPH_SOURCES[84], "鎖");
    assert_eq!(MENU06_GLYPH_SOURCES[85..=87].concat(), "高得点");
    assert_eq!(MENU06_GLYPH_SOURCES[90..=92].concat(), "相手側");
    assert_eq!(MENU06_GLYPH_SOURCES[96..=97].concat(), "応用");
}

#[test]
fn 미등록_control과_cell을_거부한다() {
    let mut raw = vec![0u8; STREAM_BYTES];
    raw[..10].copy_from_slice(&[0x18, 0xFF, 0x62, 0x00, 0x00, 0xFF, 0, 0, 0, 0]);
    assert!(parse_stream(&raw).is_err());
}

#[test]
fn fixed_slot_rewrite는_glyph만_바꾸고_control과_위치를_보호한다() {
    let mut values = vec![1u16; STREAM_BYTES / 2 - 4];
    values.extend([0xFF00, 0, 0, 0]);
    let raw = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let analysis = parse_stream(&raw).unwrap();

    let mut replacement = analysis.tokens.clone();
    let Menu06TextToken::Glyph { cell, .. } = &mut replacement[0] else {
        panic!("첫 token은 glyph여야 함");
    };
    *cell = 97;
    let rewritten =
        serialize_menu06_control_preserving_rewrite(&analysis.tokens, &replacement).unwrap();
    assert_eq!(&rewritten[..2], &97u16.to_le_bytes());
    assert_eq!(&rewritten[2..], &raw[2..]);

    let last = replacement.len() - 1;
    let Menu06TextToken::Control { code, .. } = &mut replacement[last] else {
        panic!("마지막 token은 control이어야 함");
    };
    *code = 0xFF02;
    assert!(serialize_menu06_control_preserving_rewrite(&analysis.tokens, &replacement).is_err());
}
