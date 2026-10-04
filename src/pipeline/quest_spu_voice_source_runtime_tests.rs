use super::*;

#[test]
fn exact_offset은_겹치는_occurrence도_보존한다() {
    assert_eq!(exact_offsets(b"AAAA", b"AA"), vec![0, 1, 2]);
    assert!(exact_offsets(b"AA", b"").is_empty());
    assert!(exact_offsets(b"A", b"AA").is_empty());
}

#[test]
fn 상주_vab_분모는_누락과_추가를_거부한다() {
    let rows = EXPECTED_LATER_RESIDENT_VAB_BODIES
        .iter()
        .map(|(name, spu_offset, body_bytes)| QuestResidentVabBody {
            checkpoint: "second",
            name: (*name).to_string(),
            spu_offset: *spu_offset,
            body_bytes: *body_bytes,
            body_sha256: String::new(),
        })
        .collect::<Vec<_>>();
    verify_expected_resident_bodies(&rows, &EXPECTED_LATER_RESIDENT_VAB_BODIES).unwrap();

    assert!(
        verify_expected_resident_bodies(&rows[..3], &EXPECTED_LATER_RESIDENT_VAB_BODIES).is_err()
    );
    let mut extra = rows;
    extra.push(QuestResidentVabBody {
        checkpoint: "second",
        name: "EXTRA.VB".to_string(),
        spu_offset: 0x70000,
        body_bytes: 16,
        body_sha256: String::new(),
    });
    assert!(verify_expected_resident_bodies(&extra, &EXPECTED_LATER_RESIDENT_VAB_BODIES).is_err());
}

#[test]
fn report는_source와_sequence_consumer_경계를_분리한다() {
    let report = QuestSpuVoiceSourceRuntimeReport {
        schema_version: QUEST_SPU_VOICE_SOURCE_RUNTIME_SCHEMA_VERSION,
        image_sha256: "image".to_string(),
        state_sha256: Vec::new(),
        ram_sha256: Vec::new(),
        spu_sha256: Vec::new(),
        resident_vab_bodies: Vec::new(),
        resident_sequences: Vec::new(),
        voice_sources: Vec::new(),
        long_lived_voice_source_exact: true,
        wakeup_vab_resident: true,
        wakeup_sequence_resident: true,
        sequence_consumer_verified: false,
        bgm_track_identity_verified: false,
        runtime_verified: true,
        release_eligible: false,
    };
    let rendered = render_quest_spu_voice_source_runtime_tsv(&report);
    assert!(rendered.contains("long_lived_source_exact=true"));
    assert!(rendered.contains("sequence_consumer_verified=false"));
    assert!(rendered.contains("bgm_track_identity_verified=false"));
    assert!(rendered.ends_with("\timage\n"));
}
