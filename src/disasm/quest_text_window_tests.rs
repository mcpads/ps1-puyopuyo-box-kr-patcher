use super::*;

fn evidence_fixture() -> Vec<Instruction> {
    DRAW_EVIDENCE
        .iter()
        .map(|&(address, kind)| Instruction::new(address, 0, kind))
        .collect()
}

#[test]
fn state_parent_geometry를_읽는_명령_shape를_받는다() {
    verify_draw_instructions(&evidence_fixture()).unwrap();
}

#[test]
fn extent_소비_명령이_바뀌면_거부한다() {
    let mut instructions = evidence_fixture();
    let extent = instructions
        .iter_mut()
        .find(|instruction| instruction.addr == 0x8002_A544)
        .expect("extent evidence");
    extent.kind = K::Lw {
        rt: Reg::T3,
        base: Reg::A1,
        offset: 0x54,
    };

    let error = verify_draw_instructions(&instructions).unwrap_err();
    assert!(error.to_string().contains("0x8002A544"));
}
