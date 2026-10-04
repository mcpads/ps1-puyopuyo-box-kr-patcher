use super::*;

#[test]
fn translated_streams_roundtrip_and_keep_control_and_bounds() {
    let text = HelpText::load().unwrap();
    let mut source = vec![0; MESSAGE_COUNT * MESSAGE_BYTES];
    for (i, m) in text.messages.iter().enumerate() {
        let (raw, _, failed) = encoding_rs::SHIFT_JIS.encode(&m.source);
        assert!(!failed);
        source[i * MESSAGE_BYTES..i * MESSAGE_BYTES + raw.len()].copy_from_slice(&raw);
        source[i * MESSAGE_BYTES + raw.len()..i * MESSAGE_BYTES + raw.len() + 2]
            .copy_from_slice(b"ee");
    }
    let output = text.strings(&source).unwrap();
    let reverse: BTreeMap<_, _> = text
        .glyphs
        .iter()
        .map(|(c, s)| (HelpText::code(*s), *c))
        .collect();
    for (raw, m) in output.chunks_exact(MESSAGE_BYTES).zip(&text.messages) {
        let mut decoded = String::new();
        for pair in raw.chunks_exact(2) {
            if pair == b"ee" {
                break;
            }
            if pair == b"rr" {
                decoded.push('\n');
            } else {
                decoded.push(reverse[&[pair[0], pair[1]]]);
            }
        }
        assert_eq!(decoded, m.ko);
    }
    source[0] ^= 1;
    assert!(text.strings(&source).is_err());
}

#[test]
fn overlapping_label_slots_and_control_codes_are_never_assigned() {
    let text = HelpText::load().unwrap();
    for slot in text.glyphs.values() {
        assert!((5..176).contains(slot));
        assert!(!b"er".contains(&HelpText::code(*slot)[0]));
    }
    let source = vec![0; 352];
    let table = text.table(&source).unwrap();
    assert_eq!(&source[..10], &table[..10]);
    let mut collision = source;
    collision[..2].copy_from_slice(&HelpText::code(5));
    assert!(text.table(&collision).is_err());
}

#[test]
fn unsupported_glyph_or_overlong_line_is_rejected() {
    let raw = include_str!("../../assets/translations/subgame-help/messages.tsv");
    assert!(HelpText::parse(&raw.replace("게임을 시작합니다.", "게임🙂")).is_err());
    assert!(HelpText::parse(&raw.replace("게임을 시작합니다.", &"가".repeat(17))).is_err());
    assert!(HelpText::parse(&raw.replace("게임을 시작합니다.", "첫줄\\n둘째\\n셋째")).is_err());
}
