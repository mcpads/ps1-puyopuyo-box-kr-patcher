use super::*;

#[test]
fn poc은_exe가_아닌_puyo2_vdd두개만_대상으로_선언한다() {
    assert_eq!(
        BTreeSet::from([PUYO2_SEQ_PATH, PUYO2_GRP_PATH]),
        BTreeSet::from(["/PUYO2/PP2_SEQ.VDD;1", "/PUYO2/PP2_GRP.VDD;1"])
    );
    assert!(PUYO1_EXE_PATH.ends_with("PUYO1.EXE;1"));
    assert!(PUYO2_EXE_PATH.ends_with("PUYO2.EXE;1"));
    assert_eq!(DEMO_FONT_NAME, "DEMOFONT.BTX");
    let synthetic = Puyo2E6BuildInput::SyntheticSingleGlyph.write_identity();
    let full_population = Puyo2E6BuildInput::SyntheticFullPopulation.write_identity();
    let approved = Puyo2E6BuildInput::ApprovedManifest.write_identity();
    assert_ne!(synthetic.sequence, approved.sequence);
    assert_ne!(synthetic.actor, approved.actor);
    assert_ne!(synthetic.sequence, full_population.sequence);
    assert_ne!(synthetic.actor, full_population.actor);
    assert_ne!(full_population.sequence, approved.sequence);
}

#[test]
fn 제품빌드는_e6_font_profile_외형승인전까지_거부한다() {
    let error = require_puyo2_e6_demo_font_profile_product_approval().unwrap_err();
    assert!(error.to_string().contains("runtime/HITL 외형 승인"));
}
