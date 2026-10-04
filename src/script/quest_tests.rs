use super::*;

#[test]
fn 확정한_handler와_layout만_노출한다() {
    let expected = [
        (0x00, 0x8003_DDF0, QuestControlLayout::FourReserved),
        (0x01, 0x8003_DE24, QuestControlLayout::OpcodeOnly),
        (0x02, 0x8003_DE40, QuestControlLayout::U16),
        (0x03, 0x8003_DEE8, QuestControlLayout::FourReservedAlign4),
        (0x04, 0x8003_BA14, QuestControlLayout::OpcodeOnly),
        (0x05, 0x8003_DF04, QuestControlLayout::OpcodeOnly),
        (0x06, 0x8003_DF40, QuestControlLayout::U8Reserved),
        (0x07, 0x8003_BB30, QuestControlLayout::TwoWords),
        (0x08, 0x8003_DF64, QuestControlLayout::TwoWords),
        (0x09, 0x8003_DFF0, QuestControlLayout::TwoWords),
        (0x0A, 0x8003_E06C, QuestControlLayout::TwoU8),
        (0x0B, 0x8003_E0E8, QuestControlLayout::U8Reserved),
        (0x0C, 0x8003_E134, QuestControlLayout::U8Reserved),
        (0x0D, 0x8003_E180, QuestControlLayout::U8Reserved),
        (0x0E, 0x8003_E1CC, QuestControlLayout::U8Reserved),
        (0x0F, 0x8003_E218, QuestControlLayout::U8Reserved),
        (0x10, 0x8003_BC18, QuestControlLayout::TwoU8),
        (0x11, 0x8003_E264, QuestControlLayout::TwoU8),
        (0x12, 0x8003_E308, QuestControlLayout::U8Reserved),
        (0x13, 0x8003_E36C, QuestControlLayout::TwoU8Word),
        (0x14, 0x8003_E450, QuestControlLayout::TwoWords),
        (0x15, 0x8003_E4CC, QuestControlLayout::OpcodeOnly),
        (0x16, 0x8003_BCFC, QuestControlLayout::U8Reserved),
        (0x17, 0x8003_BEE0, QuestControlLayout::U8Reserved),
        (0x18, 0x8003_E580, QuestControlLayout::U8Reserved),
        (0x19, 0x8003_E5CC, QuestControlLayout::U8Reserved),
        (0x1A, 0x8003_E5F4, QuestControlLayout::FourReserved),
        (0x1B, 0x8003_C23C, QuestControlLayout::TwoU8TwoWords),
        (0x1C, 0x8003_E640, QuestControlLayout::U8Reserved),
        (0x1D, 0x8003_C490, QuestControlLayout::TwoU8),
        (0x1E, 0x8003_C670, QuestControlLayout::U8Reserved),
        (0x1F, 0x8003_DDDC, QuestControlLayout::OpcodeOnly),
    ];
    assert_eq!(expected.len(), QUEST_CONTROL_COUNT);
    for (code, handler, layout) in expected {
        assert_eq!(
            quest_control_spec(code),
            Some(QuestControlSpec {
                code,
                handler,
                layout,
            })
        );
    }
}

#[test]
fn 의미는_소비코드로_확정한_32종을_전부_노출한다() {
    assert_eq!(QUEST_CONFIRMED_CONTROL_MEANINGS.len(), 32);
    for (code, meaning) in QUEST_CONFIRMED_CONTROL_MEANINGS {
        assert_eq!(quest_control_meaning(code), Some(meaning));
        assert!(quest_control_spec(code).is_some());
    }
    for code in 0..QUEST_CONTROL_COUNT as u8 {
        let expected = QUEST_CONFIRMED_CONTROL_MEANINGS
            .iter()
            .find_map(|&(confirmed, meaning)| (confirmed == code).then_some(meaning));
        assert_eq!(quest_control_meaning(code), expected);
    }
}

#[test]
fn ff03은_현재_주소에_따라_6바이트나_8바이트를_소비한다() {
    let layout = quest_control_spec(0x03).expect("FF03 spec").layout;
    assert_eq!(layout.encoded_len(0x8019_9A70), Some(8));
    assert_eq!(layout.encoded_len(0x8019_9A72), Some(6));
    assert_eq!(layout.argument_len(), 0);
    assert_eq!(layout.reserved_len(), 4);
}

#[test]
fn u8_reserved는_읽는_인자와_보존할_byte를_구분한다() {
    let layout = quest_control_spec(0x0F).expect("FF0F spec").layout;
    assert_eq!(layout.encoded_len(0x8019_BCFC), Some(4));
    assert_eq!(layout.argument_len(), 1);
    assert_eq!(layout.reserved_len(), 1);
}

#[test]
fn 홀수_주소와_미분류_control은_승인하지_않는다() {
    assert_eq!(
        QuestControlLayout::OpcodeOnly.encoded_len(0x8019_9A71),
        None
    );
    assert_eq!(quest_control_spec(0x20), None);
    assert_eq!(quest_control_spec(0xFF), None);
}

#[test]
fn 추가_layout의_길이와_argument_reserved를_구분한다() {
    let cases = [
        (QuestControlLayout::TwoWords, 6, 4, 0),
        (QuestControlLayout::TwoU8Word, 6, 4, 0),
        (QuestControlLayout::TwoU8TwoWords, 8, 6, 0),
        (QuestControlLayout::FourReserved, 6, 0, 4),
    ];
    for (layout, encoded, arguments, reserved) in cases {
        assert_eq!(layout.encoded_len(0x8019_9A70), Some(encoded));
        assert_eq!(layout.argument_len(), arguments);
        assert_eq!(layout.reserved_len(), reserved);
    }
}

#[test]
fn 첫_대사_형태의_표본을_빈틈없이_왕복한다() {
    let raw = [
        0x83, 0x68, 0x83, 0x89, 0x0B, 0xFF, 0x04, 0x00, 0x18, 0xFF, 0x01, 0x00, 0x82, 0xE2, 0x04,
        0xFF,
    ];
    let tokens = parse_quest_tokens(&raw, 0x8019_BC54).expect("QUEST token 표본");
    assert_eq!(tokens.len(), 6);
    let serialized: Vec<_> = tokens
        .iter()
        .flat_map(|token| token.raw().iter().copied())
        .collect();
    assert_eq!(serialized, raw);
}

#[test]
fn ff03의_reserved와_alignment_padding을_원형_보존한다() {
    let aligned = [0x03, 0xFF, 1, 2, 3, 4, 5, 6];
    let token = parse_quest_token(&aligned, 0x8019_9A70).expect("8 B FF03");
    assert_eq!(token.raw(), aligned);

    let half_aligned = [0x03, 0xFF, 1, 2, 3, 4];
    let token = parse_quest_token(&half_aligned, 0x8019_9A72).expect("6 B FF03");
    assert_eq!(token.raw(), half_aligned);
}

#[test]
fn 미분류_control과_잘린_control은_부분_파싱하지_않는다() {
    assert_eq!(
        parse_quest_tokens(&[0x20, 0xFF, 0, 1], 0x8019_9A70),
        Err(QuestTokenError::UnknownControl {
            address: 0x8019_9A70,
            code: 0x20,
        })
    );
    assert_eq!(
        parse_quest_tokens(&[0x0F, 0xFF, 1], 0x8019_9A70),
        Err(QuestTokenError::TruncatedControl {
            address: 0x8019_9A70,
            code: 0x0F,
            expected: 4,
            available: 3,
        })
    );
}

#[test]
fn ff1b의_뒤쪽_ff바이트도_인자이며_다음_control로_오인하지_않는다() {
    let raw = [
        0x1B, 0xFF, 0x00, 0x01, 0x28, 0x00, 0xD8, 0xFF, 0x02, 0xFF, 0x1E, 0x00,
    ];
    let tokens = parse_quest_tokens(&raw, 0x8019_BC40).expect("FF1B + FF02");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].raw(), &raw[..8]);
    assert_eq!(tokens[1].raw(), &raw[8..]);
}

#[test]
fn dispatch_32개를_전수_대조하고_drift를_거부한다() {
    let mut raw = Vec::with_capacity(QUEST_CONTROL_DISPATCH_BYTES);
    for code in 0..QUEST_CONTROL_COUNT {
        let spec = quest_control_spec(code as u8).expect("전수 spec");
        raw.extend_from_slice(&spec.handler.to_le_bytes());
    }
    assert_eq!(
        verify_quest_control_dispatch(&raw),
        Ok(QuestControlDispatchReport {
            controls: 32,
            unique_handlers: 32,
        })
    );

    raw[0] ^= 1;
    assert!(matches!(
        verify_quest_control_dispatch(&raw),
        Err(QuestControlDispatchError::HandlerMismatch { code: 0, .. })
    ));
    assert_eq!(
        verify_quest_control_dispatch(&raw[..4]),
        Err(QuestControlDispatchError::WrongLength {
            expected: QUEST_CONTROL_DISPATCH_BYTES,
            actual: 4,
        })
    );
}
