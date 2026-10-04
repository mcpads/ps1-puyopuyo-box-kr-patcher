use super::*;

#[test]
fn register_names_cover_all_32() {
    assert_eq!(Reg(0).name(), "zero");
    assert_eq!(Reg(31).name(), "ra");
    assert_eq!(Reg::SP.name(), "sp");
    for i in 0..32u8 {
        assert!(!Reg(i).name().is_empty());
    }
}

#[test]
fn branch_target_is_pc_plus_4_plus_offset_times_4() {
    let ins = Instruction::new(
        0x8001_0000,
        0,
        InstructionKind::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 3,
        },
    );
    match ins.flow_kind() {
        FlowKind::Branch { taken } => assert_eq!(taken, 0x8001_0004 + 12),
        other => panic!("분기가 아님: {other:?}"),
    }
}

#[test]
fn negative_branch_offset_goes_backward() {
    let ins = Instruction::new(
        0x8001_0100,
        0,
        InstructionKind::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: -4,
        },
    );
    match ins.flow_kind() {
        FlowKind::Branch { taken } => assert_eq!(taken, 0x8001_00F4),
        other => panic!("분기가 아님: {other:?}"),
    }
}

#[test]
fn jr_ra_is_return_but_other_registers_are_not() {
    let ret = Instruction::new(0, 0, InstructionKind::Jr { rs: Reg::RA });
    assert_eq!(ret.flow_kind(), FlowKind::Return);

    let indirect = Instruction::new(0, 0, InstructionKind::Jr { rs: Reg::T2 });
    assert_eq!(indirect.flow_kind(), FlowKind::JumpRegister { rs: Reg::T2 });
}

#[test]
fn delay_slot_requirement_matches_flow_kind() {
    assert!(FlowKind::Jump { target: 0 }.has_delay_slot());
    assert!(FlowKind::Call { target: 0 }.has_delay_slot());
    assert!(FlowKind::Branch { taken: 0 }.has_delay_slot());
    assert!(FlowKind::Return.has_delay_slot());
    assert!(!FlowKind::Sequential.has_delay_slot());
    assert!(!FlowKind::Trap.has_delay_slot());
}

#[test]
fn writes_to_zero_are_not_reported_as_destinations() {
    let ins = Instruction::new(
        0,
        0,
        InstructionKind::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    );
    assert_eq!(ins.dest_reg(), None);
}

#[test]
fn load_destination_is_reported_for_hazard_checks() {
    let ins = Instruction::new(
        0,
        0,
        InstructionKind::Lw {
            rt: Reg::V0,
            base: Reg::SP,
            offset: 0,
        },
    );
    assert!(ins.is_load());
    assert_eq!(ins.dest_reg(), Some(Reg::V0));
}

#[test]
fn reads_reg는_source와_lwl_merge를_구분한다() {
    let arithmetic = Instruction::new(
        0x8001_0000,
        0,
        InstructionKind::Addu {
            rd: Reg::V0,
            rs: Reg::A0,
            rt: Reg::A1,
        },
    );
    assert!(arithmetic.reads_reg(Reg::A0));
    assert!(arithmetic.reads_reg(Reg::A1));
    assert!(!arithmetic.reads_reg(Reg::V0));

    let load = Instruction::new(
        0x8001_0004,
        0,
        InstructionKind::Lw {
            rt: Reg::A0,
            base: Reg::S0,
            offset: 4,
        },
    );
    assert!(!load.reads_reg(Reg::A0));
    assert!(load.reads_reg(Reg::S0));

    let merge_load = Instruction::new(
        0x8001_0008,
        0,
        InstructionKind::Lwl {
            rt: Reg::A0,
            base: Reg::S0,
            offset: 1,
        },
    );
    assert!(merge_load.reads_reg(Reg::A0));
    assert!(merge_load.reads_reg(Reg::S0));
}

#[test]
fn reads_reg는_unknown을_보수적으로_거부한다() {
    let unknown = Instruction::new(
        0x8001_0000,
        0xFFFF_FFFF,
        InstructionKind::Unknown { word: 0xFFFF_FFFF },
    );
    assert!((0..32).all(|register| unknown.reads_reg(Reg(register))));
}

#[test]
fn unknown_and_unknown_gte_are_outside_the_profile() {
    assert!(!InstructionKind::Unknown { word: 0 }.is_decoded());
    assert!(!InstructionKind::Gte {
        command: GteCommand::Unknown(0x02),
        raw: 0
    }
    .is_decoded());
    assert!(InstructionKind::Gte {
        command: GteCommand::Rtps,
        raw: 0
    }
    .is_decoded());
}
