use super::*;

#[test]
fn constructor_call_분모는_선택callback_세곳과_직접caller_두곳을_포함한다() {
    let instructions = QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_CALL_RUNTIMES
        .iter()
        .map(|runtime| Instruction {
            addr: *runtime,
            word: 0,
            kind: K::Jal {
                target: QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME,
            },
        })
        .collect::<Vec<_>>();

    assert_eq!(
        constructor_calls(&instructions),
        QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_CALL_RUNTIMES
    );
}

#[test]
fn 다른_target은_description_constructor_caller로_세지_않는다() {
    let instructions = [Instruction {
        addr: 0x8003_8BF8,
        word: 0,
        kind: K::Jal {
            target: QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME + 4,
        },
    }];

    assert!(constructor_calls(&instructions).is_empty());
}
