use super::super::quest_glyph_cache_plan::{
    QuestPackBitsCatalogSummary, QuestRowDictionaryCatalogSummary,
    QUEST_GLYPH_CACHE_PLAN_SCHEMA_VERSION, QUEST_GLYPH_CACHE_ROW_INDEX_BITS,
};
use super::*;

fn synthetic_plan() -> QuestGlyphCachePlan {
    QuestGlyphCachePlan {
        schema_version: QUEST_GLYPH_CACHE_PLAN_SCHEMA_VERSION,
        glyphs: 2,
        pages: 2,
        max_page_glyphs: 2,
        page_assignments: 3,
        aligned_wide_slots: 2,
        original_atlas_source_slots: 2,
        catalog: QuestRowDictionaryCatalogSummary {
            cells: 2,
            plain_bytes: 192,
            dictionary_rows: 2,
            dictionary_bytes: 12,
            row_index_bits: QUEST_GLYPH_CACHE_ROW_INDEX_BITS,
            cell_index_bytes: 52,
            stored_bytes: 64,
        },
        original_atlas_source: QuestPackBitsCatalogSummary {
            cells: 2,
            plain_bytes: 192,
            index_bytes: 6,
            payload_bytes: 5,
            stored_bytes: 11,
        },
        page_directory_bytes: 6,
        page_assignment_plain_bytes: 6,
        page_assignment_bytes: 3,
        slot_rect_bytes: 8,
        korean_descriptor_bytes: QUEST_GLYPH_CACHE_KOREAN_DESCRIPTOR_BYTES,
        alignment_padding_bytes: 3,
        scratch_bytes: 96,
        package_bytes: 216,
        carrier_bytes: 300,
        carrier_remaining_bytes: 84,
        package_fits_carrier: true,
        glyph_roundtrip_exact: true,
        original_atlas_source_roundtrip_exact: true,
        page_assignment_roundtrip_exact: true,
        halfword_aligned_slots: true,
        runtime_verified: false,
        build_eligible: false,
    }
}

#[test]
fn package_layout은_original_atlas_source를_직렬화하지_않는다() {
    let plan = synthetic_plan();
    let layout = QuestGlyphCachePackageLayout::from_plan(&plan).unwrap();

    assert_eq!(layout.slot_rects_offset, 0);
    assert_eq!(layout.korean_descriptor_offset, layout.slot_rects_bytes);
    assert_eq!(layout.korean_descriptor_bytes, 36);
    assert_eq!(layout.glyph_row_dictionary_offset, 44);
    assert_eq!(layout.glyph_cell_indices_offset, 56);
    assert_eq!(layout.page_directory_offset, 108);
    assert_eq!(
        layout.page_directory_offset,
        layout.glyph_cell_indices_offset + layout.glyph_cell_indices_bytes
    );
    assert!(plan.original_atlas_source.stored_bytes > 0);
    assert_eq!(layout.page_assignments_offset, 114);
    assert_eq!(layout.alignment_padding_offset, 117);
    assert_eq!(layout.scratch_offset, 120);
    assert_eq!(layout.package_bytes, plan.package_bytes);
    assert!(layout.scratch_offset.is_multiple_of(4));
}

#[test]
fn package_writer는_겹침_범위초과_미등록바이트를_거부한다() {
    let mut overlap = PackageWriter::new(4);
    overlap.write_section("first", 0, 2, &[1, 2]).unwrap();
    assert!(overlap.write_section("overlap", 1, 1, &[3]).is_err());

    let mut out_of_range = PackageWriter::new(4);
    assert!(out_of_range
        .write_section("out of range", 4, 1, &[0])
        .is_err());

    assert!(PackageWriter::new(4).finish().is_err());
}

#[test]
fn u16_table은_little_endian으로_왕복한다() {
    let values = [0, 1, 0x1234, u16::MAX];
    let raw = encode_u16_table(&values);
    assert_eq!(raw, [0, 0, 1, 0, 0x34, 0x12, 0xFF, 0xFF]);
    assert_eq!(decode_u16_table(&raw).unwrap(), values);
    assert!(decode_u16_table(&[0]).is_err());
}

#[test]
fn korean_descriptor는_대체_atlas좌표와_cell_table을_직렬화한다() {
    let raw = encode_korean_descriptor();

    assert_eq!(raw.len(), QUEST_GLYPH_CACHE_KOREAN_DESCRIPTOR_BYTES);
    assert!(raw[..0x18].iter().all(|byte| *byte == 0));
    assert_eq!(u16::from_le_bytes([raw[0x18], raw[0x19]]), 896);
    assert_eq!(u16::from_le_bytes([raw[0x1A], raw[0x1B]]), 256);
    assert_eq!(
        u32::from_le_bytes([raw[0x1C], raw[0x1D], raw[0x1E], raw[0x1F]]),
        QUEST_GLYPH_CACHE_KOREAN_CELL_TABLE_RUNTIME
    );
    assert_eq!(
        u32::from_le_bytes([raw[0x20], raw[0x21], raw[0x22], raw[0x23]]),
        QUEST_GLYPH_CACHE_KOREAN_PIXEL_FORMAT
    );
}

#[test]
fn package_page_reader는_일기반_id와_delta_payload를_검증한다() {
    let mut plan = synthetic_plan();
    plan.package_bytes = 16;
    let layout = QuestGlyphCachePackageLayout {
        slot_rects_offset: 0,
        slot_rects_bytes: 0,
        korean_descriptor_offset: 0,
        korean_descriptor_bytes: 0,
        glyph_row_dictionary_offset: 0,
        glyph_row_dictionary_bytes: 0,
        glyph_cell_indices_offset: 0,
        glyph_cell_indices_bytes: 0,
        page_directory_offset: 0,
        page_directory_bytes: 6,
        page_assignments_offset: 6,
        page_assignments_bytes: 3,
        alignment_padding_offset: 9,
        alignment_padding_bytes: 3,
        scratch_offset: 12,
        scratch_bytes: 4,
        package_bytes: 16,
    };
    let mut bytes = vec![0; layout.package_bytes];
    bytes[..6].copy_from_slice(&encode_u16_table(&[0, 2, 3]));
    bytes[6..9].copy_from_slice(&[3, 4, 9]);
    let package = QuestGlyphCachePackage {
        catalog_glyphs: String::new(),
        menu_font_page: None,
        space_slots: vec![None; 2],
        object_particle_codes: vec![vec![]; 2],
        latin_advance_slots: vec![vec![]; 2],
        schema_version: QUEST_GLYPH_CACHE_PACKAGE_SCHEMA_VERSION,
        layout,
        plan,
        bytes,
        sha256: String::new(),
        sections_exact: false,
        catalog_readback_exact: false,
        original_atlas_source_roundtrip_exact: false,
        page_assignment_readback_exact: false,
        slot_rect_readback_exact: false,
        korean_descriptor_readback_exact: false,
        scratch_zero: false,
        runtime_verified: false,
        build_eligible: false,
        original_atlas_source_catalog: QuestEncodedPackBitsCatalog {
            offsets: vec![0, 0, 0],
            payload: Vec::new(),
            cells: vec![vec![0; QUEST_GLYPH_CACHE_CELL_BYTES]; 2],
            summary: QuestPackBitsCatalogSummary {
                cells: 2,
                plain_bytes: 2 * QUEST_GLYPH_CACHE_CELL_BYTES,
                index_bytes: 6,
                payload_bytes: 0,
                stored_bytes: 6,
            },
        },
    };

    assert_eq!(package.page_catalog_indices(1).unwrap(), [3, 7]);
    assert_eq!(package.page_catalog_indices(2).unwrap(), [9]);
    assert!(package.page_catalog_indices(0).is_err());
    assert!(package.page_catalog_indices(3).is_err());
}

#[test]
fn package_digest형식은_소문자_sha256만_허용한다() {
    assert!(is_sha256_hex(&"a".repeat(64)));
    assert!(!is_sha256_hex(&"A".repeat(64)));
    assert!(!is_sha256_hex(&"a".repeat(63)));
}
