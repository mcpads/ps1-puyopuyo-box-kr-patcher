use crate::coords::LogicalFile;
use crate::isa::encode_all;

use super::*;

fn analysis() -> PcomiGlyphCodeAnalysis {
    PcomiGlyphCodeAnalysis {
        consumers: Vec::new(),
        tables: vec![
            PcomiGlyphSuffixTable {
                suffix: PcomiGlyphSuffix::Underscore,
                runtime: PCOMI_GLYPH_UNDERSCORE_TABLE_RUNTIME,
                entries: (0..PCOMI_GLYPH_SUFFIX_TABLE_ENTRIES)
                    .map(|index| index as u16)
                    .collect(),
                nonzero_entries: 95,
                unique_entries: 96,
                unique_nonzero_entries: 95,
                maximum_entry: 95,
                sha256: [0; 32],
            },
            PcomiGlyphSuffixTable {
                suffix: PcomiGlyphSuffix::Star,
                runtime: PCOMI_GLYPH_STAR_TABLE_RUNTIME,
                entries: vec![7; PCOMI_GLYPH_SUFFIX_TABLE_ENTRIES],
                nonzero_entries: 96,
                unique_entries: 1,
                unique_nonzero_entries: 1,
                maximum_entry: 7,
                sha256: [0; 32],
            },
        ],
    }
}

#[test]
fn direct_code는_cell_base를_더한다() {
    assert_eq!(
        classify_pcomi_glyph_code(&analysis(), 0x00A0).unwrap(),
        PcomiGlyphCodeRoute::DirectCell { cell: 0x0136 }
    );
}

#[test]
fn suffix_code는_high_byte로_table을_고른다() {
    assert_eq!(
        classify_pcomi_glyph_code(&analysis(), 0x415F).unwrap(),
        PcomiGlyphCodeRoute::SuffixTable {
            suffix: PcomiGlyphSuffix::Underscore,
            high: 0x41,
            cell: 0x00B7,
        }
    );
    assert_eq!(
        classify_pcomi_glyph_code(&analysis(), 0x412A).unwrap(),
        PcomiGlyphCodeRoute::SuffixTable {
            suffix: PcomiGlyphSuffix::Star,
            high: 0x41,
            cell: 0x009D,
        }
    );
}

#[test]
fn table_밖_suffix_code는_bios_fallback으로_바꾸지_않는다() {
    let failure = classify_pcomi_glyph_code(&analysis(), 0x805F).unwrap_err();
    assert!(failure.to_string().contains("0x20..0x7F"));
}

#[test]
fn suffix가_아닌_확장_code는_bios_cache로_간다() {
    assert_eq!(
        classify_pcomi_glyph_code(&analysis(), 0x82A0).unwrap(),
        PcomiGlyphCodeRoute::BiosGlyphCache
    );
}

#[test]
fn typed_instruction_불일치를_거부한다() {
    let map = ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_BE4C,
        text_len: 4,
        entry_pc: 0x8001_BE4C,
        file: LogicalFile::new(0, 0x804),
    };
    let bytes = encode_all(&[K::Sltiu {
        rt: Reg::V0,
        rs: Reg::S1,
        imm: 0x0100,
    }]);
    let instructions = linear::sweep(&map, &bytes).unwrap();
    let spec = ConsumerSpec {
        role: "test",
        code_register: Reg::S1,
        threshold_runtime: map.load_addr,
        direct_store_runtime: 0,
        underscore_branch_runtime: 0,
        star_branch_runtime: 0,
        fallback_call_runtime: 0,
        fallback_store_runtime: 0,
        instructions: &[(
            0x8001_BE4C,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::S1,
                imm: 0x1000,
            },
        )],
    };
    assert!(verify_instructions(&instructions, spec)
        .unwrap_err()
        .to_string()
        .contains("명령 불일치"));
}

#[test]
fn tsv는_consumer와_table을_분리한다() {
    let mut analysis = analysis();
    analysis.consumers.push(PcomiGlyphCodeConsumer {
        role: "object-loop",
        code_register: Reg::S1,
        threshold_runtime: 1,
        direct_store_runtime: 2,
        underscore_branch_runtime: 3,
        star_branch_runtime: 4,
        fallback_call_runtime: 5,
        fallback_store_runtime: 6,
        consumer_entry_reached: false,
    });
    let report = render_pcomi_glyph_codes_tsv(&analysis);
    assert!(report.starts_with("kind\trole\tcode_register"));
    assert!(report.contains("consumer\tobject-loop\ts1"));
    assert!(report.contains("table\t\t\t\t\t\t\t\t\t\tunderscore"));
}
