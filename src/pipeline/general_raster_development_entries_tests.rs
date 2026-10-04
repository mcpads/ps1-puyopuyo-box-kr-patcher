use super::*;

#[test]
fn archive_registry는_경로대소문자를무시하고_알수없는경로를거부한다() {
    let puyo1 = archive_spec_for_path("/puyo1/pp1_grp.vdd;1").unwrap();
    assert_eq!(puyo1.scope, "PUYO1");
    assert_eq!(puyo1.iso_path, "/PUYO1/PP1_GRP.VDD;1");

    let error = archive_spec_for_path("/UNKNOWN/GRP.VDD;1").unwrap_err();
    assert!(error.to_string().contains("registry 결속 수 불일치"));
}

#[test]
fn 활성_flat_background자산은_catalog의모든물리경로를사용한다() {
    for development_asset in FLAT_BACKGROUND_DEVELOPMENT_ASSETS {
        let archives = archive_specs_for_asset(development_asset.asset).unwrap();
        let resolved_paths = archives
            .iter()
            .map(|archive| archive.iso_path)
            .collect::<Vec<_>>();
        assert_eq!(resolved_paths, development_asset.asset.archive_paths);
    }
}
