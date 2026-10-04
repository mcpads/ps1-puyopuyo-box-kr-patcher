use super::*;

#[test]
fn every_literal_fits_its_original_copy_and_roundtrips_through_lookup() {
    let text = SkillText::load().unwrap();
    let table = text.table(&vec![0; TABLE_BYTES]).unwrap();
    for m in &text.messages {
        for (source, ko) in [
            (&m.source_name, &m.name),
            (&m.source_description, &m.description),
        ] {
            let mut original = source_bytes(source).unwrap();
            original.push(0);
            let output = text.literal(&original, source, ko).unwrap();
            let end = ko.chars().count() * 2;
            assert_eq!(output[end], 0);
            assert_eq!(&output[end + 1..], &original[end + 1..]);
            let decoded: String = output[..end]
                .chunks_exact(2)
                .map(|p| {
                    if p == SPACE {
                        ' '
                    } else if p == NEWLINE {
                        '\n'
                    } else {
                        let slot = table.chunks_exact(2).position(|code| code == p).unwrap();
                        *text
                            .glyphs
                            .iter()
                            .find(|(_, value)| **value == slot)
                            .unwrap()
                            .0
                    }
                })
                .collect();
            assert_eq!(decoded, *ko);
        }
    }
}

#[test]
fn source_change_copy_overflow_and_missing_glyph_fail() {
    let text = SkillText::load().unwrap();
    let mut original = source_bytes("カット").unwrap();
    original.push(0);
    assert!(text.literal(&original, "バリア", "컷").is_err());
    assert!(text.literal(&original, "カット", "컷컷컷컷").is_err());
    assert!(text.literal(&original, "カット", "漢").is_err());
}

#[test]
fn lookup_preserves_unallocated_slots_and_rejects_duplicate_codes() {
    let text = SkillText::load().unwrap();
    let mut original = vec![0; TABLE_BYTES];
    original[500..].copy_from_slice(&[0x81, 0x48]);
    let result = text.table(&original).unwrap();
    assert_eq!(
        &result[text.glyphs.len() * 2..],
        &original[text.glyphs.len() * 2..]
    );
    original[500..].copy_from_slice(&SkillText::code(0));
    assert!(text.table(&original).is_err());
}

#[test]
fn thirteen_columns_fail_even_when_original_copy_has_room() {
    let raw = include_str!("../../assets/translations/special-skills/messages.tsv");
    let rejected = raw.replacen("다른 플레이어의 임의의", "다른 플레이어의 임의의가", 1);
    assert!(SkillText::parse(&rejected)
        .err()
        .unwrap()
        .contains("창 용량 초과"));
}
