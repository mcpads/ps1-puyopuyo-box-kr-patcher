use super::*;

#[test]
fn population_plan은_17개_atlas와_33개_후보_연결을_한번에_합성한다() {
    let (targets, links, incompatible) = population_targets().unwrap();
    assert_eq!(targets.len(), 17);
    assert_eq!(links, 33);
    assert_eq!(incompatible, 1);

    let skel = targets
        .iter()
        .find(|target| target.asset == "MZ_SKEL.BTX")
        .unwrap();
    assert!(skel
        .consumers
        .contains(&"same-stem:SKEL_SEQ.SEQ".to_string()));
    assert!(skel.consumers.contains(&"selected:NAS_SEQ.SEQ".to_string()));
    assert_eq!(skel.repertoire_indices, (0..42).collect::<Vec<_>>());

    let sat1 = targets
        .iter()
        .find(|target| target.asset == "MZ_SAT1.BTX")
        .unwrap();
    assert!(sat1
        .consumers
        .contains(&"same-stem:SAT1_SEQ.SEQ".to_string()));
    assert!(sat1
        .consumers
        .contains(&"selected:SAT2_SEQ.SEQ".to_string()));
    assert_eq!(sat1.repertoire_indices, (0..64).collect::<Vec<_>>());

    let sat2 = targets
        .iter()
        .find(|target| target.asset == "MZ_SAT2.BTX")
        .unwrap();
    assert_eq!(sat2.consumers, ["same-stem:SAT2_SEQ.SEQ"]);
    assert_eq!(sat2.repertoire_indices, (0..64).collect::<Vec<_>>());
}
