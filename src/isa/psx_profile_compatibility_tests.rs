use psx_r3000a::{
    verify_placed_program, Assembler, AssemblerError, Instruction, InstructionForm, PsxR3000A,
    Register, PROFILE_ID,
};
use typed_isa_core::{ControlAction, ControlTarget, DelaySlotExecution, StaticSemantics};

#[test]
fn 공통_psx_profile은_배치된_분기와_delay_slot을_연속_왕복한다() {
    assert_eq!(PROFILE_ID, "psx-r3000a");

    let origin = 0x8001_0000;
    let mut assembler = Assembler::new();
    assembler
        .label("loop")
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 1,
        })
        .bne(Register::T0, Register::T1, "loop")
        .emit(Instruction::nop());

    let program = assembler.assemble(origin).unwrap();
    let decoded = verify_placed_program(program.bytes(), origin).unwrap();
    assert_eq!(decoded, program.instructions());
    assert_eq!(program.instruction_spans()[1].offset, 4);
    assert_eq!(program.instruction_spans()[1].location, origin + 4);
    assert_eq!(
        program.instruction_spans()[1].bytes.as_slice(),
        &program.bytes()[4..8]
    );
    assert_eq!(decoded[1].form(), InstructionForm::Bne);
    let semantics = PsxR3000A::semantics(&decoded[1], &(origin + 4)).unwrap();
    assert_eq!(
        semantics.control_flow.action,
        ControlAction::Transfer {
            target: ControlTarget::Direct(origin)
        }
    );
    assert_eq!(semantics.control_flow.fallthrough, Some(origin + 12));
    let delay_slot = semantics.control_flow.delay_slot.unwrap();
    assert_eq!(delay_slot.point, origin + 8);
    assert_eq!(delay_slot.execution, DelaySlotExecution::Always);
}

#[test]
fn 공통_psx_assembler는_delay_slot_없는_제어흐름을_거부한다() {
    let origin = 0x8001_0000;
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Jr { rs: Register::RA });

    assert_eq!(
        assembler.assemble(origin),
        Err(AssemblerError::MissingDelaySlot { address: origin })
    );
}
