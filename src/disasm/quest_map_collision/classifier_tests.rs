use super::*;

fn context() -> QuestCollisionClassifierContext {
    QuestCollisionClassifierContext {
        actor_is_primary: true,
        actor_layer: 0,
        actor_direction: 2,
        actor_state: 0,
        input_state: 0,
        op92_conditions: [0, 0],
    }
}

#[test]
fn 관측한_suke00_player문맥에서_tile0은_collision을_건너뛴다() {
    assert_eq!(
        classify_quest_collision_tile(0, context()),
        QuestCollisionClass::NoCollision
    );
    assert_eq!(QuestCollisionClass::NoCollision.code(), 0);
    assert_eq!(QuestCollisionClass::SpecialContact.code(), 1);
    assert_eq!(QuestCollisionClass::Blocking.code(), 2);
}

#[test]
fn 일반tile은_actor_layer와_4000_flag가_맞지_않으면_blocking이다() {
    let lower = context();
    let upper = QuestCollisionClassifierContext {
        actor_layer: 1,
        ..lower
    };

    assert_eq!(
        classify_quest_collision_tile(7, lower),
        QuestCollisionClass::NoCollision
    );
    assert_eq!(
        classify_quest_collision_tile(0x4007, lower),
        QuestCollisionClass::Blocking
    );
    assert_eq!(
        classify_quest_collision_tile(7, upper),
        QuestCollisionClass::Blocking
    );
    assert_eq!(
        classify_quest_collision_tile(0x4007, upper),
        QuestCollisionClass::NoCollision
    );
    assert_eq!(
        classify_quest_collision_tile(0x8007, upper),
        QuestCollisionClass::NoCollision
    );
}

#[test]
fn id2와_3부터5는_일반_plane선택보다_먼저_갈라진다() {
    let lower = context();
    let upper = QuestCollisionClassifierContext {
        actor_layer: 1,
        ..lower
    };

    assert_eq!(
        classify_quest_collision_tile(2, lower),
        QuestCollisionClass::Blocking
    );
    assert_eq!(
        classify_quest_collision_tile(0x4002, upper),
        QuestCollisionClass::NoCollision
    );
    for low in 3..=5 {
        assert_eq!(
            classify_quest_collision_tile(0x4000 | low, lower),
            QuestCollisionClass::NoCollision
        );
    }
}

#[test]
fn op92조건은_97과109_99와110을_서로_전환한다() {
    let clear = context();
    let set = QuestCollisionClassifierContext {
        op92_conditions: [1, 1],
        ..clear
    };

    for low in [97, 99] {
        assert_eq!(
            classify_quest_collision_tile(low, clear),
            QuestCollisionClass::Blocking
        );
        assert_eq!(
            classify_quest_collision_tile(low, set),
            QuestCollisionClass::NoCollision
        );
    }
    for low in [109, 110] {
        assert_eq!(
            classify_quest_collision_tile(low, clear),
            QuestCollisionClass::NoCollision
        );
        assert_eq!(
            classify_quest_collision_tile(low, set),
            QuestCollisionClass::Blocking
        );
    }
}

#[test]
fn id6은_plane과_방향을_통과한_input에서만_special_contact다() {
    let mut value = context();
    assert_eq!(
        classify_quest_collision_tile(6, value),
        QuestCollisionClass::Blocking
    );

    value.actor_direction = 0;
    value.input_state = QUEST_COLLISION_INPUT_STATE_MASK;
    assert_eq!(
        classify_quest_collision_tile(6, value),
        QuestCollisionClass::SpecialContact
    );

    assert_eq!(
        classify_quest_collision_tile(0xC006, value),
        QuestCollisionClass::NoCollision
    );
}

#[test]
fn primary전용_id는_비primary를막고_primary_state5를_통과시킨다() {
    for low in [102, 112, 127] {
        assert_eq!(
            classify_quest_collision_tile(low, context()),
            QuestCollisionClass::SpecialContact
        );
        assert_eq!(
            classify_quest_collision_tile(
                low,
                QuestCollisionClassifierContext {
                    actor_is_primary: false,
                    ..context()
                }
            ),
            QuestCollisionClass::Blocking
        );
        assert_eq!(
            classify_quest_collision_tile(
                low,
                QuestCollisionClassifierContext {
                    actor_state: 5,
                    ..context()
                }
            ),
            QuestCollisionClass::NoCollision
        );
    }
}
