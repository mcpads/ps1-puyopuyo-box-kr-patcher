use super::*;

fn unique_table() -> Vec<u8> {
    (0..QUEST_CODE_TABLE_COUNT)
        .flat_map(|index| (index as u16).to_le_bytes())
        .collect()
}

fn poc_plan_fixture() -> (QuestCodeTable, BTreeSet<[u8; 2]>, BTreeSet<char>) {
    let mut raw = unique_table();
    for mapping in QUEST_POC_GLYPH_MAPPINGS {
        raw[mapping.index * 2..mapping.index * 2 + 2].copy_from_slice(&mapping.code);
    }
    let table = QuestCodeTable::parse(&raw).unwrap();
    let mut candidates = BTreeSet::from([0usize, 1usize]);
    candidates.extend(30..43);
    candidates.extend(QUEST_POC_GLYPH_MAPPINGS.iter().map(|mapping| mapping.index));
    assert_eq!(candidates.len(), QUEST_POC_UNOBSERVED_CODE_COUNT);
    let observed = table
        .codes()
        .iter()
        .enumerate()
        .filter(|(index, _)| !candidates.contains(index))
        .map(|(_, code)| *code)
        .collect();
    let required = QUEST_POC_GLYPH_MAPPINGS
        .iter()
        .map(|mapping| mapping.glyph)
        .collect();
    (table, observed, required)
}

#[test]
fn runtime_표본과_끝_경계를_만족한다() {
    let cases = [
        (0, (0, 0, 6, 16)),
        (18, (108, 0, 6, 16)),
        (19, (114, 0, 12, 16)),
        (29, (234, 0, 12, 16)),
        (30, (0, 16, 12, 16)),
        (109, (192, 64, 12, 16)),
        (344, (240, 240, 12, 16)),
    ];
    for (index, expected) in cases {
        let slot = QuestGlyphSlot::for_index(index).unwrap();
        assert_eq!(
            (slot.x, slot.y, slot.width, slot.height),
            expected,
            "index {index}"
        );
        assert!(slot.x + slot.width <= QUEST_FONT_LAYOUT.width);
        assert!(slot.y + slot.height <= QUEST_FONT_LAYOUT.height);
    }
}

#[test]
fn 모든_slot은_겹치지_않고_아틀라스_안에_있다() {
    let mut occupied = vec![false; QUEST_FONT_LAYOUT.width * QUEST_FONT_LAYOUT.height];
    for index in 0..QUEST_CODE_TABLE_COUNT {
        let slot = QuestGlyphSlot::for_index(index).unwrap();
        for y in slot.y..slot.y + slot.height {
            for x in slot.x..slot.x + slot.width {
                let pixel = y * QUEST_FONT_LAYOUT.width + x;
                assert!(!occupied[pixel], "index {index}가 ({x},{y})에서 겹침");
                occupied[pixel] = true;
            }
        }
    }
}

#[test]
fn 코드표_길이와_중복을_실패시킨다() {
    assert!(matches!(
        QuestCodeTable::parse(&unique_table()[..QUEST_CODE_TABLE_BYTES - 2]),
        Err(QuestFontError::CodeTableLength { .. })
    ));

    let mut duplicate = unique_table();
    let first = [duplicate[0], duplicate[1]];
    duplicate[20..22].copy_from_slice(&first);
    assert_eq!(
        QuestCodeTable::parse(&duplicate),
        Err(QuestFontError::DuplicateCode {
            code: [0, 0],
            first: 0,
            second: 10,
        })
    );
}

#[test]
fn 코드와_slot을_같은_index로_연결한다() {
    let table = QuestCodeTable::parse(&unique_table()).unwrap();
    let code = 109u16.to_le_bytes();
    assert_eq!(table.index_of(code), Some(109));
    assert_eq!(table.code_at(109), Some(code));
    assert_eq!(table.slot_for_code(code).unwrap().x, 192);
}

#[test]
fn 전용_두_문자_모양을_한_cell_의미_문자로_해독한다() {
    assert_eq!(
        decode_quest_source_glyph(QUEST_DOUBLE_EXCLAMATION_CODE),
        Some(QUEST_DOUBLE_EXCLAMATION_REVIEW_GLYPH)
    );
    assert_eq!(
        decode_quest_source_glyph(QUEST_DOUBLE_QUESTION_CODE),
        Some(QUEST_DOUBLE_QUESTION_REVIEW_GLYPH)
    );
    assert_eq!(decode_quest_source_glyph([0x82, 0xA0]), Some('あ'));
    assert_eq!(decode_quest_source_glyph([0x41, 0x42]), None);
}

#[test]
fn source_fullwidth_tilde는_review_wave_dash와_같은glyph로_정규화한다() {
    assert_eq!(canonicalize_quest_source_glyph('～'), '〜');
    assert_eq!(canonicalize_quest_source_glyph('「'), '「');
}

#[test]
fn 검토표기를_quest_표시glyph로_정규화한다() {
    assert_eq!(
        normalize_quest_display_text(" (1/6)!?.,:~～·…「」 ").unwrap(),
        "　（1／6）！？。、：〜〜・…「」　"
    );
    assert!(normalize_quest_display_text("첫줄\n둘째줄").is_err());
}

#[test]
fn 번역문의_숫자와_단위는_반각을_유지하고_명시한_전각과_구별한다() {
    assert_eq!(
        normalize_quest_display_text("20cm ２０cm").unwrap(),
        "20cm　２０cm"
    );
}

#[test]
fn poc_한글_11자는_잘림_없이_12x16으로_생성된다() {
    assert_eq!(QUEST_POC_RASTER_PROFILE.id, "quest-dialogue-12x16");
    assert_eq!(QUEST_POC_RASTER_PROFILE.font_id, MAPLESTORY_LIGHT_FONT_ID);
    assert_eq!(QUEST_POC_RASTER_PROFILE.effect, QuestRasterEffect::None);
    let font = crate::font::source::bundled_maplestory_light().unwrap();
    for glyph in "드라코해냈다내가이겼어".chars() {
        let pixels = rasterize_wide_glyph(&font, glyph, QUEST_POC_RASTER_PROFILE).unwrap();
        assert_eq!(pixels.len(), 12 * 16);
        assert!(pixels.contains(&15), "{glyph}");
        assert!(pixels.iter().all(|value| *value <= 15), "{glyph}");
    }
}

#[test]
fn poc_font_plan은_승인_11자와_임시_wide_후보_24개를_구분한다() {
    let (table, observed, required) = poc_plan_fixture();
    assert_eq!(
        verify_quest_poc_font_plan(&table, &observed, &required),
        Ok(QuestPocFontPlanReport {
            source_unobserved_codes: 26,
            provisional_wide_slots: 24,
            assigned_glyphs: 11,
            provisional_remaining: 13,
        })
    );
    for mapping in QUEST_POC_GLYPH_MAPPINGS {
        assert_eq!(quest_poc_code(mapping.glyph), Some(mapping.code));
    }
    assert_eq!(quest_poc_code('한'), None);
}

#[test]
fn poc_font_plan은_승인_glyph_drift와_후보_수_drift를_거부한다() {
    let (table, mut observed, mut required) = poc_plan_fixture();
    required.remove(&'어');
    assert!(verify_quest_poc_font_plan(&table, &observed, &required)
        .unwrap_err()
        .to_string()
        .contains("승인 번역 glyph와 mapping 불일치"));

    let required = QUEST_POC_GLYPH_MAPPINGS
        .iter()
        .map(|mapping| mapping.glyph)
        .collect();
    observed.insert(QUEST_POC_GLYPH_MAPPINGS[0].code);
    assert!(matches!(
        verify_quest_poc_font_plan(&table, &observed, &required),
        Err(QuestFontError::PocCapacityDrift {
            field: "source-unobserved-codes",
            expected: 26,
            actual: 25,
        })
    ));
}

#[test]
fn poc_font_plan_tsv는_할당_11개와_잠정_13개를_분리한다() {
    let (table, observed, required) = poc_plan_fixture();
    let candidates = quest_poc_font_candidates(&table, &observed, &required).unwrap();
    assert_eq!(candidates.len(), 24);
    assert_eq!(
        candidates
            .iter()
            .filter(|candidate| candidate.status == QuestPocFontCandidateStatus::PocAssigned)
            .count(),
        11
    );
    assert_eq!(
        candidates
            .iter()
            .filter(|candidate| {
                candidate.status == QuestPocFontCandidateStatus::ProvisionalUnobserved
            })
            .count(),
        13
    );

    let report = render_quest_poc_font_plan_tsv(&table, &observed, &required).unwrap();
    assert_eq!(report.lines().count(), 25);
    assert_eq!(report.matches("\tpoc-assigned\t").count(), 11);
    assert_eq!(report.matches("\tprovisional-unobserved\t").count(), 13);
    assert!(report.contains(
        "quest-poc-font-plan-v1\tRPG_FNT1.BTX\t98151af59e76b8be63bb99d4a7c0c3ec96615ffdf1090ab511dc1dabf8ae0a4c"
    ));
    assert!(!report.as_bytes().contains(&0));
}
