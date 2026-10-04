use super::*;
use crate::btx::{Layout, PixelFormat};
use crate::graphics::text_catalog::{PaletteIndexSpec, Rect, TextBlockSpec, TextStorageSpec};

const ARCHIVES: [&str; 1] = ["/FIXTURE.VDD;1"];
const PALETTE: [u8; 2] = [0, 1];
const BLOCKS: [TextBlockSpec<'static>; 3] = [
    TextBlockSpec {
        id: "BLOCK-0001",
        source: "日本語",
        bbox: Rect {
            x: 0,
            y: 0,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&PALETTE),
        expected_pixels_sha256: "block-one",
    },
    TextBlockSpec {
        id: "BLOCK-0002",
        source: "かな\n漢字",
        bbox: Rect {
            x: 32,
            y: 0,
            width: 32,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&PALETTE),
        expected_pixels_sha256: "block-two",
    },
    TextBlockSpec {
        id: "BLOCK-0003",
        source: "REFERENCE",
        bbox: Rect {
            x: 64,
            y: 0,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&PALETTE),
        expected_pixels_sha256: "block-three",
    },
];
const SPEC: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &ARCHIVES,
    name: "FIXTURE.BTX",
    layout: Layout {
        width: 96,
        height: 32,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256: "asset",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &BLOCKS,
};

fn entry(
    block: TextBlockSpec<'static>,
    category: MmSprTextCategory,
    status: MmSprTranslationStatus,
    ko: &str,
) -> MmSprTranslationEntry {
    MmSprTranslationEntry {
        archive_paths: ARCHIVES.join("\n"),
        asset_name: SPEC.name.to_string(),
        asset_sha256: SPEC.expected_decompressed_sha256.to_string(),
        block_id: block.id.to_string(),
        block_pixels_sha256: block.expected_pixels_sha256.to_string(),
        bbox_x: block.bbox.x,
        bbox_y: block.bbox.y,
        bbox_width: block.bbox.width,
        bbox_height: block.bbox.height,
        protected_source: block.source.to_string(),
        category,
        status,
        ko: ko.to_string(),
    }
}

fn entries() -> Vec<MmSprTranslationEntry> {
    vec![
        entry(
            BLOCKS[0],
            MmSprTextCategory::ModeDescriptionOneLine,
            MmSprTranslationStatus::PocApproved,
            "한글",
        ),
        entry(
            BLOCKS[1],
            MmSprTextCategory::ModeDescriptionTwoLines,
            MmSprTranslationStatus::DraftHumanReviewRequired,
            "첫줄\n둘째줄",
        ),
    ]
}

#[test]
fn 합성_manifest의_상태와_행_category를_집계한다() {
    assert_eq!(
        verify_manifest(SPEC, &entries()),
        Ok(MmSprTranslationReport {
            japanese_targets: 2,
            poc_approved: 1,
            approved: 0,
            draft_human_review_required: 1,
            pending_human_translation: 0,
            one_line: 1,
            two_lines: 1,
            three_lines: 0,
        })
    );
}

#[test]
fn 행_category마다_독립_profile을_선택한다() {
    let one = MmSprTextCategory::ModeDescriptionOneLine.profile();
    let two = MmSprTextCategory::ModeDescriptionTwoLines.profile();
    let three = MmSprTextCategory::ModeDescriptionThreeLines.profile();

    assert_eq!((one.line_count, one.line_height), (1, 16));
    assert_eq!((two.line_count, two.line_height), (2, 16));
    assert_eq!((three.line_count, three.line_height), (3, 16));
    assert_ne!(one.id, two.id);
    assert_ne!(two.id, three.id);
    assert_eq!(one.font_id, MAPLESTORY_LIGHT_FONT_ID);
    assert_eq!(two.font_id, MAPLESTORY_LIGHT_FONT_ID);
    assert_eq!(three.font_id, MAPLESTORY_LIGHT_FONT_ID);
}

#[test]
fn review_tsv는_합성원문과_번역을_같은행에_내보낸다() {
    let report = render_mm_spr_review_tsv_from(SPEC, &entries()).unwrap();
    let rows = report.lines().skip(1).collect::<Vec<_>>();

    assert_eq!(rows.len(), 2);
    assert!(rows[0].contains("\tBLOCK-0001\t"));
    assert!(rows[0].ends_with("日本語\t한글"));
    assert!(rows[1].contains("かな\\n漢字"));
    assert!(rows[1].ends_with("첫줄\\n둘째줄"));
}

#[test]
fn 누락_중복_원문_drift와_reference_only를_거부한다() {
    assert!(matches!(
        verify_manifest(SPEC, &[]),
        Err(MmSprTranslationError::EmptyManifest)
    ));

    let mut missing = entries();
    missing.pop();
    assert!(matches!(
        verify_manifest(SPEC, &missing),
        Err(MmSprTranslationError::MissingJapaneseTarget(id)) if id == "BLOCK-0002"
    ));

    let mut duplicate = entries();
    duplicate[1] = duplicate[0].clone();
    assert!(matches!(
        verify_manifest(SPEC, &duplicate),
        Err(MmSprTranslationError::DuplicateBlockId(_))
    ));

    let mut drift = entries();
    drift[0].protected_source = "다른 원문".to_string();
    assert!(matches!(
        verify_manifest(SPEC, &drift),
        Err(MmSprTranslationError::ProtectedSourceMismatch { .. })
    ));

    let mut reference_only = entries();
    reference_only[0] = entry(
        BLOCKS[2],
        MmSprTextCategory::ModeDescriptionOneLine,
        MmSprTranslationStatus::PendingHumanTranslation,
        "",
    );
    assert!(matches!(
        verify_manifest(SPEC, &reference_only),
        Err(MmSprTranslationError::ReferenceOnlyBlock(_))
    ));
}

#[test]
fn 보호_자산field와_canonical순서_drift를_거부한다() {
    let mut asset_drift = entries();
    asset_drift[0].bbox_width += 1;
    assert!(matches!(
        verify_manifest(SPEC, &asset_drift),
        Err(MmSprTranslationError::ProtectedAssetMismatch(_))
    ));

    let mut reordered = entries();
    reordered.swap(0, 1);
    assert!(matches!(
        verify_manifest(SPEC, &reordered),
        Err(MmSprTranslationError::NonCanonicalOrder(_))
    ));
}

#[test]
fn 번역상태와_한국어_유무가_어긋나면_거부한다() {
    let mut pending_with_text = entries();
    pending_with_text[0].status = MmSprTranslationStatus::PendingHumanTranslation;
    assert!(matches!(
        verify_manifest(SPEC, &pending_with_text),
        Err(MmSprTranslationError::StatusTranslationMismatch(id)) if id == "BLOCK-0001"
    ));

    let raw = format!(
        "{TRANSLATION_HEADER}\n{}\t{}\t{}\t{}\t{}\t{}\t0\t0\t32\t16\t{}\t{}\t{}\tunknown-review-state\n",
        MM_SPR_TRANSLATION_SCHEMA_VERSION,
        ARCHIVES[0],
        SPEC.name,
        SPEC.expected_decompressed_sha256,
        BLOCKS[0].id,
        BLOCKS[0].expected_pixels_sha256,
        BLOCKS[0].source,
        MmSprTextCategory::ModeDescriptionOneLine.label(),
        "한글",
    );
    assert!(matches!(
        parse_translation_manifest(&raw),
        Err(MmSprTranslationError::InvalidManifest(_))
    ));
}

#[test]
fn 승인_항목이_없으면_poc_build_입력을_거부한다() {
    let mut drafts = entries();
    for entry in &mut drafts {
        entry.status = MmSprTranslationStatus::DraftHumanReviewRequired;
    }
    assert_eq!(
        approved_poc_translations_from(SPEC, &drafts),
        Err(MmSprTranslationError::NoApprovedPocTranslations)
    );
}
