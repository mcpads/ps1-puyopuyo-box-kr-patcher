use super::*;

#[test]
fn box_원본_일본어_표본을_바이트_동일하게_왕복한다() {
    let raw = [
        0x83, 0x41, 0x83, 0x8B, 0x83, 0x8B, 0x81, 0x75, // アルル「
        0x82, 0xC8, 0x82, 0xF1, 0x82, 0xBE, 0x82, 0xEB, // なんだろ
        0x81, 0x41, // 、
        0x82, 0xB1, 0x82, 0xCC, // この
        0x83, 0x6A, 0x83, 0x49, 0x83, 0x43, // ニオイ
        0x81, 0x45, 0x81, 0x45, 0x81, 0x45, // ・・・
    ];
    let decoded = decode_sjis(&raw).unwrap();

    assert_eq!(decoded, "アルル「なんだろ、このニオイ・・・");
    assert_eq!(encode_sjis(&decoded).unwrap(), raw);
}

#[test]
fn ascii와_반각_가나도_지원한다() {
    let text = "Puyo 123 ｱﾙﾙ";
    let encoded = encode_sjis(text).unwrap();

    assert_eq!(decode_sjis(&encoded).unwrap(), text);
}

#[test]
fn 잘린_lead와_잘못된_trail을_부분_decode하지_않는다() {
    assert_eq!(decode_sjis(&[0x82]), Err(SjisError::InvalidBytes));
    assert_eq!(decode_sjis(&[0x82, 0x20]), Err(SjisError::InvalidBytes));
}

#[test]
fn 한글과_emoji_mapping_누락을_입력_순서로_한번씩_보고한다() {
    let error = encode_sjis("한글한🙂글").unwrap_err();

    assert_eq!(
        error,
        SjisError::UnmappableCharacters(vec!['한', '글', '🙂'])
    );
    assert!(!error.to_string().contains("&#"));
}
