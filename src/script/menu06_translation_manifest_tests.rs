use super::*;
use crate::script::{Menu06RewriteCapacity, Menu06TextSegment};

fn synthetic_text() -> Menu06TextAnalysis {
    let mut tokens = Vec::new();
    let mut offset = 0x2DC0usize;
    for _ in 0..297 {
        tokens.push(Menu06TextToken::Glyph { offset, cell: 1 });
        offset += 2;
    }
    for _ in 0..23 {
        tokens.push(Menu06TextToken::Control {
            offset,
            code: 0xFF04,
            arguments: vec![0],
        });
        offset += 4;
    }
    tokens.push(Menu06TextToken::Control {
        offset,
        code: 0xFF13,
        arguments: vec![0, 0],
    });
    offset += 6;
    tokens.push(Menu06TextToken::Control {
        offset,
        code: 0xFF17,
        arguments: vec![0],
    });
    offset += 4;
    for _ in 0..44 {
        tokens.push(Menu06TextToken::Control {
            offset,
            code: 0xFF02,
            arguments: Vec::new(),
        });
        offset += 2;
    }
    tokens.push(Menu06TextToken::Control {
        offset,
        code: 0xFF00,
        arguments: Vec::new(),
    });

    Menu06TextAnalysis {
        sources: Vec::new(),
        segments: vec![Menu06TextSegment {
            index: 0,
            start_offset: 0x2DC0,
            end_offset: 0x2DC0 + 594,
            glyph_cells: vec![1; 297],
            source: "가".repeat(297),
        }],
        tokens,
        glyph_tokens: 297,
        distinct_glyph_cells: 1,
        control_tokens: BTreeMap::new(),
        stream_sha256: String::new(),
        static_cell_prefix_records: 0,
        static_valid_cells: 0,
        static_empty_cells: 0,
        rewrite_capacity: Menu06RewriteCapacity {
            stream_words: 396,
            glyph_positions: 297,
            control_words: 70,
            control_argument_words: 26,
            terminator_padding_words: 3,
            slack_words: 0,
            repertoire_slots: 98,
            used_repertoire_slots: 1,
        },
    }
}

fn manifest(status: &str, ko: &str, padding: usize) -> String {
    format!(
        "{HEADER}\n{}\t{status}\tMENU06-TXT-0001\t0x2DC0\t0x3012\t{}\t{ko}\t{padding}\tfixture\n",
        MENU06_TRANSLATION_MANIFEST_SCHEMA_VERSION,
        "가".repeat(297),
    )
}

#[test]
fn 합성manifest는_현재입력고정값없이_보호원문과계획을_검증한다() {
    let text = synthetic_text();
    let raw = manifest("draft-needs-human-review", "한 글", 294);
    let analysis = verify_manifest(&raw, &text).unwrap();

    assert_eq!(analysis.entries.len(), 1);
    assert_eq!(analysis.protected_source_code_units, 297);
    assert_eq!(analysis.translated_code_units, 3);
    assert_eq!(analysis.padding_cells, 294);
    assert_eq!(analysis.draft_needs_human_review, 1);
    assert!(!analysis.product_eligible);
    assert_eq!(render_box_menu06_translation_manifest_tsv(&analysis), raw);
}

#[test]
fn status와_일본어_잔존을_거부한다() {
    let unsupported = manifest("reviewed", "한 글", 294);
    assert!(parse_manifest(&unsupported).is_err());
    let japanese = manifest("draft-needs-human-review", "日本語", 294);
    assert!(parse_manifest(&japanese).is_err());
}

#[test]
fn full_width_padding은_8px_space와_다른_cell을_쓴다() {
    let text = synthetic_text();
    let entries = vec![Menu06TranslationManifestEntry {
        status: Menu06TranslationStatus::DraftNeedsHumanReview,
        segment_id: "synthetic".to_string(),
        start_offset: 0,
        end_offset: 0,
        protected_source: "가".repeat(297),
        ko: " ".to_string(),
        padding_cells: 296,
        notes: "test".to_string(),
    }];
    let plan = build_plan(&entries, &text).unwrap();
    assert_eq!(plan.rewritten_stream.len(), 792);
    assert_eq!(plan.glyph_mappings[0].glyph, Some(' '));
    assert_eq!(plan.glyph_mappings[0].cell, 0);
    assert_eq!(plan.padding_cell, 1);
    assert_eq!(plan.glyph_mappings[1].glyph, None);
    assert_eq!(plan.remaining_slots, 96);
}
