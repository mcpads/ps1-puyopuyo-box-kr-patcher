use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    for evidence in EVIDENCE {
        let offset = map
            .text_offset(evidence.address)
            .expect("fixture 명령 범위");
        text[offset..offset + 4].copy_from_slice(&encode(evidence.kind).to_le_bytes());
    }
    (map, text)
}

#[test]
fn 공용_base의_branch_return_전이_8종을_고정한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_seq_base_transitions(map, &text),
        Ok(QuestSeqBaseTransitionSummary {
            meanings: 8,
            lifecycle_meanings: 1,
            stack_branch_meanings: 4,
            return_meanings: 1,
            branch_table_meanings: 5,
            branch_state_base_runtime: 0x800D_B6C0,
            catalog_group_table_runtime: 0x800D_B328,
            release_interpreter_runtime: 0x8001_4CD0,
            dispatch_yield_gp_offset: 0x518,
        })
    );
}

#[test]
fn 전이_spec은_base_dispatch_handler와_일치한다() {
    for spec in QUEST_SEQ_BASE_TRANSITION_SPECS {
        assert_eq!(
            quest_seq_opcode_handler(spec.opcode),
            Some(spec.handler_runtime),
            "OP{:02X}",
            spec.opcode
        );
    }
    for spec in QUEST_SEQ_BASE_LIFECYCLE_SPECS {
        assert_eq!(
            quest_seq_opcode_handler(spec.opcode),
            Some(spec.handler_runtime),
            "OP{:02X}",
            spec.opcode
        );
        assert!(!QUEST_SEQ_BASE_TRANSITION_SPECS
            .iter()
            .any(|transition| transition.opcode == spec.opcode));
    }
}

#[test]
fn op00은_interpreter를_해제하고_stream을_옮긴_뒤_dispatcher를_yield한다() {
    let (map, text) = fixture();
    let summary = verify_box_quest_seq_base_transitions(map, &text).expect("OP00 수명주기 의미");
    assert_eq!(summary.lifecycle_meanings, 1);
    assert_eq!(
        summary.release_interpreter_runtime,
        QUEST_RELEASE_OBJECT_RUNTIME
    );
    assert_eq!(summary.dispatch_yield_gp_offset, 0x518);
    assert_eq!(QUEST_SEQ_BASE_LIFECYCLE_SPECS[0].opcode, 0x00);
    assert_eq!(
        QUEST_SEQ_BASE_LIFECYCLE_SPECS[0].meaning,
        QuestSeqBaseLifecycleMeaning::ReleaseInterpreterAndYieldDispatcher
    );
}

#[test]
fn op00의_interpreter_release_target_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8002_265C).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Jal {
            target: 0x8001_4B14,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_base_transitions(map, &text),
        Err(QuestSeqBaseTransitionError::InstructionMismatch {
            opcode: 0x00,
            address: 0x8002_265C,
            ..
        })
    ));
}

#[test]
fn op00의_dispatcher_yield_store_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8002_2680).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Sw {
            rt: Reg::V1,
            base: Reg::GP,
            offset: 0x51C,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_base_transitions(map, &text),
        Err(QuestSeqBaseTransitionError::InstructionMismatch {
            opcode: 0x00,
            address: 0x8002_2680,
            ..
        })
    ));
}

#[test]
fn op03의_pointer_load_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8002_2708).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 8,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_base_transitions(map, &text),
        Err(QuestSeqBaseTransitionError::InstructionMismatch {
            opcode: 0x03,
            address: 0x8002_2708,
            ..
        })
    ));
}

#[test]
fn op39의_catalog_target_store_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8002_3348).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x20,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_base_transitions(map, &text),
        Err(QuestSeqBaseTransitionError::InstructionMismatch {
            opcode: 0x39,
            address: 0x8002_3348,
            ..
        })
    ));
}
