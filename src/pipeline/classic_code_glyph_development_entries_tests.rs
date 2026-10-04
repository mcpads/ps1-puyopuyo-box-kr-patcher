use super::*;

fn family(name: &'static str) -> ClassicCodeGlyphFamilyReport {
    ClassicCodeGlyphFamilyReport {
        family: name,
        translation_units: 1,
        draft_units: 1,
        approved_units: 0,
        sequence_entries: 1,
        glyph_entries: 1,
        glyph_mappings: 1,
        used_bytes: 1,
        remaining_bytes: 1,
        final_readback_exact: false,
        runtime_verified: false,
        product_eligible: false,
    }
}

fn prepared() -> PreparedClassicCodeGlyphDevelopmentEntries {
    PreparedClassicCodeGlyphDevelopmentEntries {
        entries: Vec::new(),
        verified_archives: BTreeSet::new(),
        menu06_surface: Vec::new(),
        puyo2_e6_surface: Vec::new(),
        puyo1_scene_surfaces: Vec::new(),
        report: ClassicCodeGlyphDevelopmentEntriesReport {
            menu06: family("MENU06"),
            puyo2_e6: family("PUYO2-E6"),
            puyo1_scene: family("PUYO1-SCENE"),
            physical_entries: 1,
            auxiliary_sequence_entries: 0,
            archives: ARCHIVE_PATHS.len(),
            runtime_verified: false,
            product_eligible: false,
        },
    }
}

#[test]
fn 최종readback은_family가_실제로쓰는_archive가_모두검증돼야_닫힌다() {
    let mut build = prepared();

    build
        .record_verified_archive_readback(PUYO1_SEQ_PATH)
        .unwrap();
    assert!(!build.report.puyo1_scene.final_readback_exact);
    build
        .record_verified_archive_readback(PUYO1_GRP_PATH)
        .unwrap();
    assert!(build.report.puyo1_scene.final_readback_exact);
    assert!(!build.report.menu06.final_readback_exact);

    build
        .record_verified_archive_readback(PUYO2_SEQ_PATH)
        .unwrap();
    assert!(!build.report.puyo2_e6.final_readback_exact);
    build
        .record_verified_archive_readback(PUYO2_GRP_PATH)
        .unwrap();
    assert!(build.report.menu06.final_readback_exact);
    assert!(build.report.puyo2_e6.final_readback_exact);

    assert!(build
        .record_verified_archive_readback(PUYO2_GRP_PATH)
        .unwrap_err()
        .to_string()
        .contains("중복"));
}
