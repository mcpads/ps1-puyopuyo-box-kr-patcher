use super::*;

const ARCHIVES_A: [&str; 1] = ["/A.VDD;1"];
const ARCHIVES_B: [&str; 1] = ["/B.VDD;1"];
const MEMBERS: [BtxRasterProfileCandidateMember; 2] = [
    BtxRasterProfileCandidateMember {
        candidate_index: 1,
        semantic_family_proposal: "family-a",
        semantic_family_purpose: "fixture",
        archive_paths: &ARCHIVES_A,
        asset_name: "NORMAL.BTX",
        asset_sha256: "asset-a",
        block_id: "BLOCK-0001",
        block_pixels_sha256: "block-a",
        bbox_x: 0,
        bbox_y: 0,
        bbox_width: 64,
        bbox_height: 16,
        protected_source: "日本語",
    },
    BtxRasterProfileCandidateMember {
        candidate_index: 2,
        semantic_family_proposal: "family-b",
        semantic_family_purpose: "fixture",
        archive_paths: &ARCHIVES_B,
        asset_name: "EXCLUDED.BTX",
        asset_sha256: "asset-b",
        block_id: "BLOCK-0002",
        block_pixels_sha256: "block-b",
        bbox_x: 8,
        bbox_y: 0,
        bbox_width: 32,
        bbox_height: 16,
        protected_source: "かな",
    },
];

fn groups() -> Vec<BtxRasterProfileCandidateGroup> {
    vec![
        BtxRasterProfileCandidateGroup {
            pixel_format: "indexed4",
            bbox_height: 16,
            source_lines: 1,
            palette_indices: vec![0, 1],
            blocks: 1,
            assets: 1,
            minimum_width: 64,
            maximum_width: 64,
            block_pixels: 1_024,
        },
        BtxRasterProfileCandidateGroup {
            pixel_format: "indexed4",
            bbox_height: 16,
            source_lines: 1,
            palette_indices: vec![0, 15],
            blocks: 1,
            assets: 1,
            minimum_width: 32,
            maximum_width: 32,
            block_pixels: 512,
        },
    ]
}

fn delegated(product_eligible: bool) -> BtxRasterTranslationReport {
    BtxRasterTranslationReport {
        entries: 3,
        drafts: usize::from(!product_eligible) * 3,
        approved: usize::from(product_eligible) * 3,
        product_eligible,
    }
}

fn translation_row(
    member: &BtxRasterProfileCandidateMember,
    source: &str,
    ko: &str,
    status: &str,
) -> String {
    [
        BTX_RASTER_GENERAL_TRANSLATION_SCHEMA_VERSION.to_string(),
        member.semantic_family_proposal.to_string(),
        member.candidate_index.to_string(),
        member.archive_paths.join("\\n"),
        member.asset_name.to_string(),
        member.asset_sha256.to_string(),
        member.block_id.to_string(),
        member.block_pixels_sha256.to_string(),
        member.bbox_x.to_string(),
        member.bbox_y.to_string(),
        member.bbox_width.to_string(),
        member.bbox_height.to_string(),
        source.to_string(),
        ko.to_string(),
        status.to_string(),
    ]
    .join("\t")
}

fn translation_manifest(first_status: &str, first_ko: &str) -> String {
    format!(
        "{TRANSLATION_HEADER}\n{}\n{}\n",
        translation_row(
            &MEMBERS[0],
            MEMBERS[0].protected_source,
            first_ko,
            first_status
        ),
        translation_row(
            &MEMBERS[1],
            MEMBERS[1].protected_source,
            "범위 제외",
            "scope-excluded-protected-no-write",
        )
    )
}

fn profile_row(
    member: &BtxRasterProfileCandidateMember,
    palette: &str,
    status: &str,
    approved: bool,
    effect: &str,
) -> String {
    let configuration = if approved {
        [
            "fixture-category",
            "fixture-profile",
            "maplestory-light",
            "12",
            "13",
            "1",
            "16",
            "0",
            "center",
            "0",
            "1",
            effect,
        ]
    } else {
        ["", "", "", "", "", "", "", "", "", "", "", ""]
    };
    let mut fields = vec![
        BTX_RASTER_CATEGORY_PROFILE_SCHEMA_VERSION.to_string(),
        member.semantic_family_proposal.to_string(),
        member.candidate_index.to_string(),
        "indexed4".to_string(),
        "16".to_string(),
        "1".to_string(),
        palette.to_string(),
    ];
    fields.extend(configuration.into_iter().map(str::to_string));
    fields.push(status.to_string());
    fields.join("\t")
}

fn profile_manifest(first_approved: bool, effect: &str) -> String {
    let first_status = if first_approved {
        "approved"
    } else {
        "profile-candidate-human-approval-required"
    };
    format!(
        "{PROFILE_HEADER}\n{}\n{}\n",
        profile_row(&MEMBERS[0], "0,1", first_status, first_approved, effect),
        profile_row(
            &MEMBERS[1],
            "0,15",
            "scope-excluded-protected-no-write",
            false,
            "",
        )
    )
}

fn verify_fixture(
    translations: &str,
    profiles: &str,
) -> Result<
    (
        Vec<GeneralRasterTranslationEntry>,
        GeneralRasterManifestReport,
    ),
    GeneralRasterManifestError,
> {
    verify_manifests_against(
        translations,
        profiles,
        &groups(),
        &MEMBERS,
        delegated(false),
        &["EXCLUDED.BTX"],
    )
}

#[test]
fn 합성일반raster를_번역_profile_위임범위에_연결한다() {
    let (_, report) = verify_fixture(
        &translation_manifest("draft-human-review-required", "한글 초안"),
        &profile_manifest(false, ""),
    )
    .unwrap();
    assert_eq!(
        report,
        GeneralRasterManifestReport {
            translation_blocks: 2,
            translation_assets: 2,
            profile_assignments: 2,
            semantic_families: 2,
            pending_translations: 0,
            draft_translations: 1,
            approved_translations: 0,
            scope_excluded_translations: 1,
            pending_profiles: 1,
            development_profiles: 0,
            approved_profiles: 0,
            scope_excluded_profiles: 1,
            delegated_g_mess_blocks: 3,
            covered_raster_blocks: 5,
            all_in_scope_human_approved: false,
        }
    );
}

#[test]
fn 보호원문_bbox_profile제약_drift를_거부한다() {
    let source_drift = format!(
        "{TRANSLATION_HEADER}\n{}\n{}\n",
        translation_row(
            &MEMBERS[0],
            "다른 원문",
            "한글 초안",
            "draft-human-review-required",
        ),
        translation_row(
            &MEMBERS[1],
            MEMBERS[1].protected_source,
            "범위 제외",
            "scope-excluded-protected-no-write",
        )
    );
    assert!(verify_fixture(&source_drift, &profile_manifest(false, ""))
        .unwrap_err()
        .to_string()
        .contains("보호 원본 필드 drift"));

    let profile_drift =
        profile_manifest(false, "").replacen("indexed4\t16\t1\t0,1", "indexed4\t8\t1\t0,1", 1);
    assert!(verify_fixture(
        &translation_manifest("draft-human-review-required", "한글 초안"),
        &profile_drift,
    )
    .unwrap_err()
    .to_string()
    .contains("profile 보호 제약 drift"));
}

#[test]
fn 미승인_번역과_profile에_편집값이_들어가면_거부한다() {
    let translations = translation_manifest("pending-human-translation", "한글 초안");
    assert!(verify_fixture(&translations, &profile_manifest(false, ""))
        .unwrap_err()
        .to_string()
        .contains("대기 번역에 한국어가 있음"));

    let configured_pending = profile_row(
        &MEMBERS[0],
        "0,1",
        "profile-candidate-human-approval-required",
        true,
        "none",
    );
    let profiles = format!(
        "{PROFILE_HEADER}\n{configured_pending}\n{}\n",
        profile_row(
            &MEMBERS[1],
            "0,15",
            "scope-excluded-protected-no-write",
            false,
            "",
        )
    );
    assert!(verify_fixture(
        &translation_manifest("draft-human-review-required", "한글 초안"),
        &profiles,
    )
    .unwrap_err()
    .to_string()
    .contains("설정 없는 profile에 설정값이 있음"));
}

#[test]
fn manifest는_block과_family_profile의_canonical순서를_강제한다() {
    let mut lines = translation_manifest("draft-human-review-required", "한글 초안")
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    lines.swap(1, 2);
    let reordered = format!("{}\n", lines.join("\n"));
    assert!(verify_fixture(&reordered, &profile_manifest(false, ""))
        .unwrap_err()
        .to_string()
        .contains("canonical 순서"));

    let mut lines = profile_manifest(false, "")
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    lines.swap(1, 2);
    let reordered = format!("{}\n", lines.join("\n"));
    assert!(verify_fixture(
        &translation_manifest("draft-human-review-required", "한글 초안"),
        &reordered,
    )
    .unwrap_err()
    .to_string()
    .contains("canonical 순서"));
}

#[test]
fn 승인profile은_번역항목에_category와profile을_결속한다() {
    let (entries, report) = verify_fixture(
        &translation_manifest("approved", "한글 확정"),
        &profile_manifest(true, "outline:1"),
    )
    .unwrap();
    assert_eq!(report.approved_profiles, 1);
    assert_eq!(entries[0].category_id.as_deref(), Some("fixture-category"));
    assert_eq!(entries[0].profile_id.as_deref(), Some("fixture-profile"));

    let palette_violation = profile_manifest(true, "outline:2");
    assert!(verify_fixture(
        &translation_manifest("approved", "한글 확정"),
        &palette_violation,
    )
    .unwrap_err()
    .to_string()
    .contains("effect가 원본 palette 밖임"));
}

#[test]
fn 개발profile은_사람승인과_분리해_설정만노출한다() {
    let configured = profile_row(
        &MEMBERS[0],
        "0,1",
        "development-draft-human-review-required",
        true,
        "none",
    );
    let profiles = format!(
        "{PROFILE_HEADER}\n{configured}\n{}\n",
        profile_row(
            &MEMBERS[1],
            "0,15",
            "scope-excluded-protected-no-write",
            false,
            "",
        )
    );
    let (entries, report) = verify_fixture(
        &translation_manifest("draft-human-review-required", "한글 초안"),
        &profiles,
    )
    .unwrap();
    assert_eq!(report.pending_profiles, 1);
    assert_eq!(report.development_profiles, 1);
    assert_eq!(report.approved_profiles, 0);
    assert_eq!(entries[0].profile_id, None);

    let parsed = parse_profile_manifest(&profiles).unwrap();
    let assignment = public_profile_assignment(parsed[0].clone(), 2).unwrap();
    assert_eq!(
        assignment.status,
        GeneralRasterProfileStatus::DevelopmentDraftHumanReviewRequired
    );
    assert!(assignment.configuration.is_some());
}

#[test]
fn 번역의_수동행수는_보호원문의_행수와_독립적이다() {
    let configured = profile_row(
        &MEMBERS[0],
        "0,1",
        "development-draft-human-review-required",
        true,
        "none",
    )
    .replacen("\t1\t16\t0\tcenter\t", "\t2\t8\t0\tcenter\t", 1);
    let profiles = format!(
        "{PROFILE_HEADER}\n{configured}\n{}\n",
        profile_row(
            &MEMBERS[1],
            "0,15",
            "scope-excluded-protected-no-write",
            false,
            "",
        )
    );
    let (entries, report) = verify_fixture(
        &translation_manifest("draft-human-review-required", "첫째\\n둘째"),
        &profiles,
    )
    .unwrap();

    assert_eq!(report.development_profiles, 1);
    assert_eq!(entries[0].ko, "첫째\n둘째");
    let parsed = parse_profile_manifest(&profiles).unwrap();
    let assignment = public_profile_assignment(parsed[0].clone(), 2).unwrap();
    assert_eq!(assignment.source_lines, 1);
    assert_eq!(assignment.configuration.unwrap().line_count, 2);

    assert!(verify_fixture(
        &translation_manifest("draft-human-review-required", "한 줄"),
        &profiles,
    )
    .unwrap_err()
    .to_string()
    .contains("번역문과 profile 행 수 불일치"));
}

#[test]
fn 범위제외자산과_번역_profile상태를_일치시킨다() {
    let included = translation_manifest("draft-human-review-required", "한글 초안").replacen(
        "scope-excluded-protected-no-write",
        "draft-human-review-required",
        1,
    );
    assert!(verify_fixture(&included, &profile_manifest(false, ""))
        .unwrap_err()
        .to_string()
        .contains("제품 범위 상태 불일치"));

    let included_profile = profile_manifest(false, "").replacen(
        "scope-excluded-protected-no-write",
        "profile-candidate-human-approval-required",
        1,
    );
    assert!(verify_fixture(
        &translation_manifest("draft-human-review-required", "한글 초안"),
        &included_profile,
    )
    .unwrap_err()
    .to_string()
    .contains("profile 제품 범위 상태 불일치"));
}

#[test]
fn 조밀한_palette는_제외목록으로도_같은index집합을_표현한다() {
    let parsed = parse_palette_indices("all-except:152,157,250,251,252,253,254,255", 2)
        .expect("조밀한 palette 제외 목록");
    assert_eq!(parsed.len(), 248);
    assert!(parsed.contains(&0));
    assert!(parsed.contains(&249));
    assert!(!parsed.contains(&152));
    assert!(!parsed.contains(&255));

    let error = parse_palette_indices("all-except:157,152", 2).unwrap_err();
    assert!(error.to_string().contains("canonical 오름차순"));
}
