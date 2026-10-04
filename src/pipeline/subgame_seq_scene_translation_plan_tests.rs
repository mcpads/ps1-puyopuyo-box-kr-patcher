use super::*;

fn manifest_entry(
    logical_id: &str,
    sequence: &str,
    status: SubgameSeqSceneTranslationStatus,
    ko: &str,
) -> super::super::SubgameSeqSceneTranslationManifestEntry {
    super::super::SubgameSeqSceneTranslationManifestEntry {
        status,
        logical_id: logical_id.to_string(),
        sequence: sequence.to_string(),
        raw_sha256: "0".repeat(64),
        copy_count: 2,
        puyo1_message_id: Some(format!("PUYO1:{logical_id}")),
        puyo2_message_id: Some(format!("PUYO2:{logical_id}")),
        puyo1_record_offset: Some(0),
        puyo2_record_offset: Some(0),
        protected_source: "あ<6565>".to_string(),
        ko: ko.to_string(),
        reference: "fixture".to_string(),
        notes: "fixture".to_string(),
    }
}

fn translation_manifest() -> SubgameSeqSceneTranslationManifestAnalysis {
    SubgameSeqSceneTranslationManifestAnalysis {
        entries: vec![
            manifest_entry(
                "A:L01",
                "A.SEQ",
                SubgameSeqSceneTranslationStatus::DraftNeedsHumanReview,
                "가<6565>",
            ),
            manifest_entry(
                "A:L02",
                "A.SEQ",
                SubgameSeqSceneTranslationStatus::Approved,
                "나<6565>",
            ),
            manifest_entry(
                "B:L01",
                "B.SEQ",
                SubgameSeqSceneTranslationStatus::PendingHumanTranslation,
                "",
            ),
            manifest_entry(
                "B:L02",
                "B.SEQ",
                SubgameSeqSceneTranslationStatus::PendingHumanTranslation,
                "",
            ),
        ],
        sequences: 2,
        logical_payloads: 4,
        physical_messages: 8,
        paired_payloads: 4,
        singleton_payloads: 0,
        protected_source_code_units: 0,
        translated_code_units: 0,
        pending_human_translation: 1,
        draft_needs_human_review: 1,
        approved: 1,
        product_eligible: false,
        manifest_sha256: String::new(),
    }
}

#[test]
fn annotated_parser는_text와_control과_종결을_분리한다() {
    let parts = parse_annotated("가<71:20>나<72><6565>").unwrap();
    assert_eq!(
        parts,
        vec![
            AnnotatedPart::Text("가".to_string()),
            AnnotatedPart::Control(vec![0x71, 0x20]),
            AnnotatedPart::Text("나".to_string()),
            AnnotatedPart::Control(vec![0x72]),
            AnnotatedPart::Control(vec![0x65, 0x65]),
        ]
    );
}

#[test]
fn annotated_parser는_잘못된_placeholder와_중간종결을_거부한다() {
    assert!(parse_annotated("가<7G><6565>").is_err());
    assert!(parse_annotated("가<71:2><6565>").is_err());
    assert!(parse_annotated("가<6565>나<72>").is_err());
    assert!(parse_annotated("가<72>").is_err());
}

#[test]
fn record_compile은_header와_control을_보존하고_glyph만_pair로_바꾼다() {
    let source = [1, 2, 3, 4, 0x82, 0xA0, 0x72, 0x65, 0x65];
    let translated = parse_annotated("가<72><6565>").unwrap();
    let codes = [('가', [0x82, 0xA0])].into_iter().collect();
    assert_eq!(
        compile_record(&source, "あ<72><6565>", &translated, &codes).unwrap(),
        source
    );
    assert!(compile_record(
        &source,
        "あ<72><6565>",
        &parse_annotated("가<73><6565>").unwrap(),
        &codes,
    )
    .is_err());
}

#[test]
fn 사람이_쓴_ascii공백은_배정된_빈glyph_pair로_컴파일한다() {
    let source = [1, 2, 3, 4, 0x82, 0xA0, 0x82, 0xA2, 0x72, 0x65, 0x65];
    let translated = parse_annotated("가 나<72><6565>").unwrap();
    let codes = [
        ('가', [0x82, 0xA0]),
        (' ', [0x82, 0xA2]),
        ('나', [0x82, 0xA4]),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        compile_record(&source, "あい<72><6565>", &translated, &codes).unwrap(),
        [1, 2, 3, 4, 0x82, 0xA0, 0x82, 0xA2, 0x82, 0xA4, 0x72, 0x65, 0x65]
    );
}

#[test]
fn synthetic_text는_각_glyph_run을_한글한자로_축약한다() {
    assert_eq!(
        synthetic_annotated("あ<72><72><77:10>いう<6565>").unwrap(),
        "가<72><72><77:10>가<6565>"
    );
}

#[test]
fn family_번역은_전체행이_작성된_경우에만_모은다() {
    let manifest = translation_manifest();
    let translations = collect_subgame_seq_scene_family_translations(&manifest, "A.SEQ").unwrap();
    assert_eq!(
        translations
            .iter()
            .map(|translation| translation.logical_id.as_str())
            .collect::<Vec<_>>(),
        ["A:L01", "A:L02"]
    );
    assert!(collect_subgame_seq_scene_family_translations(&manifest, "B.SEQ").is_err());
    assert!(collect_subgame_seq_scene_family_translations(&manifest, "C.SEQ").is_err());
}

#[test]
fn 작성된_scene_family는_전부_모으고_전부pending인_family는_보존한다() {
    let manifest = translation_manifest();
    let translations = collect_authored_subgame_seq_scene_translations(&manifest).unwrap();
    assert_eq!(
        translations
            .iter()
            .map(|translation| translation.logical_id.as_str())
            .collect::<Vec<_>>(),
        ["A:L01", "A:L02"]
    );

    let mut partial = manifest;
    partial.entries[3].status = SubgameSeqSceneTranslationStatus::DraftNeedsHumanReview;
    partial.entries[3].ko = "다<6565>".to_string();
    assert!(collect_authored_subgame_seq_scene_translations(&partial).is_err());
}

#[test]
fn scene_compiler_선택은_family의_일부행만_받지_않는다() {
    let manifest = translation_manifest();
    assert!(select_complete_translation_families(&BTreeSet::from(["A:L01"]), &manifest).is_err());

    let selected =
        select_complete_translation_families(&BTreeSet::from(["A:L01", "A:L02"]), &manifest)
            .unwrap();
    assert_eq!(selected.len(), 2);
    assert!(select_complete_translation_families(&BTreeSet::from(["UNKNOWN"]), &manifest).is_err());
}
