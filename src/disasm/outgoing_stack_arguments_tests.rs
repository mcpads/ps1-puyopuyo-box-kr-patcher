use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{find_outgoing_stack_argument_flows, OutgoingStackArgumentOrigin};

fn instruction(offset: u32, kind: K) -> Instruction {
    Instruction::new(0x801B_0000 + offset, 0, kind)
}

fn prologue() -> Vec<Instruction> {
    vec![
        instruction(
            0,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -32,
            },
        ),
        instruction(
            4,
            K::Sw {
                rt: Reg::RA,
                base: Reg::SP,
                offset: 28,
            },
        ),
    ]
}

#[test]
fn 현재_call_setup과_delay_slot을_앞선_call이_소비한값과_구분한다() {
    let mut setup = prologue();
    setup.extend([
        instruction(
            8,
            K::Sw {
                rt: Reg::A0,
                base: Reg::SP,
                offset: 16,
            },
        ),
        instruction(
            12,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        instruction(
            16,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
    ]);
    let rows = find_outgoing_stack_argument_flows(&setup, 16);
    assert_eq!(
        rows[0].origin,
        OutgoingStackArgumentOrigin::CurrentCallSetup
    );
    assert!(rows[0].prior_consuming_calls.is_empty());

    let mut delay = prologue();
    delay.extend([
        instruction(
            8,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        instruction(
            12,
            K::Sw {
                rt: Reg::A1,
                base: Reg::SP,
                offset: 16,
            },
        ),
    ]);
    let rows = find_outgoing_stack_argument_flows(&delay, 16);
    assert_eq!(
        rows[0].origin,
        OutgoingStackArgumentOrigin::CurrentCallDelaySlot
    );

    let mut reused = prologue();
    reused.extend([
        instruction(
            8,
            K::Sw {
                rt: Reg::A2,
                base: Reg::SP,
                offset: 16,
            },
        ),
        instruction(
            12,
            K::Jal {
                target: 0x8001_1000,
            },
        ),
        instruction(
            16,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
        instruction(
            20,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V1,
            },
        ),
        instruction(
            24,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
    ]);
    let rows = find_outgoing_stack_argument_flows(&reused, 16);
    assert_eq!(
        rows[0].origin,
        OutgoingStackArgumentOrigin::PreviouslyConsumed
    );
    assert_eq!(rows[0].prior_consuming_calls, vec![0x801B_000C]);
}

#[test]
fn 앞선_call의_delay_store도_이미_소비된값으로_분류한다() {
    let mut instructions = prologue();
    instructions.extend([
        instruction(
            8,
            K::Jal {
                target: 0x8001_1000,
            },
        ),
        instruction(
            12,
            K::Sw {
                rt: Reg::A0,
                base: Reg::SP,
                offset: 16,
            },
        ),
        instruction(
            16,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        instruction(
            20,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
    ]);
    let rows = find_outgoing_stack_argument_flows(&instructions, 16);
    assert_eq!(
        rows[0].origin,
        OutgoingStackArgumentOrigin::PreviouslyConsumed
    );
    assert_eq!(rows[0].prior_consuming_calls, vec![0x801B_0008]);
}

#[test]
fn callee_save_slot과_이미_닫힌_frame을_호출인자로_세지않는다() {
    let mut callee_save = prologue();
    callee_save.extend([
        instruction(
            8,
            K::Sw {
                rt: Reg::S0,
                base: Reg::SP,
                offset: 16,
            },
        ),
        instruction(
            12,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        instruction(
            16,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
        instruction(
            20,
            K::Lw {
                rt: Reg::S0,
                base: Reg::SP,
                offset: 16,
            },
        ),
    ]);
    assert!(find_outgoing_stack_argument_flows(&callee_save, 16).is_empty());

    let mut closed = prologue();
    closed.extend([
        instruction(
            8,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: 32,
            },
        ),
        instruction(
            12,
            K::Sw {
                rt: Reg::A0,
                base: Reg::SP,
                offset: 16,
            },
        ),
        instruction(
            16,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
    ]);
    assert!(find_outgoing_stack_argument_flows(&closed, 16).is_empty());
}
