use crate::text::encode_sjis;

use super::*;

#[test]
fn e6_stream은_glyph_span과_세_control과_종결을_분리한다() {
    let first = encode_sjis("アイ").unwrap();
    let second = encode_sjis("ウ").unwrap();
    let table = first
        .chunks_exact(2)
        .chain(second.chunks_exact(2))
        .map(|pair| pair.try_into().unwrap())
        .collect::<BTreeSet<[u8; 2]>>();
    let mut raw = vec![0; 4];
    raw.extend_from_slice(&first);
    raw.extend_from_slice(&CONTROL_72);
    raw.extend_from_slice(&second);
    raw.extend_from_slice(&CONTROL_70);
    raw.extend_from_slice(&CONTROL_77);
    raw.extend_from_slice(&TERMINATOR);

    let parsed = parse_stream(&raw, 4, raw.len(), &table).unwrap();
    assert_eq!(parsed.spans.len(), 2);
    assert_eq!(parsed.spans[0].2, "アイ");
    assert_eq!(parsed.spans[1].2, "ウ");
    assert_eq!(parsed.control_70, 1);
    assert_eq!(parsed.control_72, 1);
    assert_eq!(parsed.control_77, 1);
    assert_eq!(parsed.raw, raw[4..]);
}

#[test]
fn e6_stream은_미등록_pair와_종결누락을_실패폐쇄한다() {
    let glyph = encode_sjis("ア").unwrap();
    let empty = BTreeSet::new();
    let mut raw = glyph.clone();
    raw.extend_from_slice(&TERMINATOR);
    assert!(parse_stream(&raw, 0, raw.len(), &empty).is_err());

    let table = [glyph.as_slice().try_into().unwrap()]
        .into_iter()
        .collect::<BTreeSet<[u8; 2]>>();
    assert!(parse_stream(&glyph, 0, glyph.len(), &table).is_err());
}

#[test]
fn e6_stream은_initialized_경계_밖의_종결자를_읽지_않는다() {
    let glyph = encode_sjis("ア").unwrap();
    let table = [glyph.as_slice().try_into().unwrap()]
        .into_iter()
        .collect::<BTreeSet<[u8; 2]>>();
    let mut raw = glyph.clone();
    raw.extend_from_slice(&TERMINATOR);

    assert!(parse_stream(&raw, 0, glyph.len(), &table).is_err());
    assert!(parse_stream(&raw, 0, raw.len() + 2, &table).is_err());
}
