use super::*;

fn base_used_pages() -> BTreeMap<u8, BTreeSet<char>> {
    [(1, ['가'].into_iter().collect()), (2, BTreeSet::new())]
        .into_iter()
        .collect()
}

fn supplemental_page(page_id: u8, used_glyphs: &str) -> QuestTranslationEncodingSupplementalPage {
    QuestTranslationEncodingSupplementalPage {
        page_id,
        used_glyphs: used_glyphs.to_owned(),
    }
}

fn fixture_slot_count() -> usize {
    QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX + 16
}

fn source_codes(space_index: usize) -> BTreeMap<char, QuestTranslationSourceGlyphCode> {
    BTreeMap::from([(
        '\u{3000}',
        QuestTranslationSourceGlyphCode {
            index: space_index,
            code: [0x81, 0x40],
        },
    )])
}

#[test]
fn 검토표기_문장부호를_정규화하고_반각숫자는_유지한다() {
    assert_eq!(
        normalize_quest_translation_text(" (1/6)!?.,:~～·…「」 ").unwrap(),
        "　（1／6）！？。、：〜〜・…「」　"
    );
}

#[test]
fn stream밖_control문자는_거부한다() {
    assert!(normalize_quest_translation_text("첫줄\n둘째줄").is_err());
}

#[test]
fn 독립bank는_전각공백을뺈_사용glyph전체를_page에_공급한다() {
    let used = ['　', '가', '！', '？']
        .into_iter()
        .collect::<BTreeSet<_>>();
    let dynamic = page_dynamic_glyphs(&used);
    assert_eq!(dynamic, ['가', '！', '？'].into_iter().collect());
}

#[test]
fn 전각공백_source_slot이_upload와겹칠때만_page에_공급한다() {
    let mut used = (0..263)
        .map(|index| char::from_u32(0xAC00 + index).unwrap())
        .collect::<BTreeSet<_>>();
    used.insert('\u{3000}');
    let collision = page_storage_glyphs(&used, &source_codes(292)).unwrap();
    assert!(collision.contains(&'\u{3000}'));

    let safe = page_storage_glyphs(&used, &source_codes(293)).unwrap();
    assert!(!safe.contains(&'\u{3000}'));
}

#[test]
fn supplemental이_비면_기존page의_동적배정을_그대로_유지한다() {
    let used_by_page = base_used_pages();
    let expected = used_by_page
        .iter()
        .map(|(&page_id, used)| QuestTranslationEncodingPage {
            page_id,
            glyphs: page_dynamic_glyphs(used).into_iter().collect(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        page_encodings_with_supplemental_pages(
            used_by_page,
            &[],
            &source_codes(292),
            fixture_slot_count(),
        )
        .unwrap(),
        expected
    );
}

#[test]
fn supplemental_used_glyph전체를_독립bank에_공급한다() {
    let used_by_page = base_used_pages();
    let next_page_id = used_by_page
        .keys()
        .next_back()
        .copied()
        .unwrap()
        .checked_add(1)
        .unwrap();
    let pages = page_encodings_with_supplemental_pages(
        used_by_page,
        &[supplemental_page(next_page_id, "한。")],
        &source_codes(292),
        fixture_slot_count(),
    )
    .unwrap();
    assert_eq!(
        pages
            .iter()
            .find(|page| page.page_id == next_page_id)
            .unwrap()
            .glyphs,
        "。한"
    );
}

#[test]
fn supplemental_page_id_0과중복과vdd충돌을_거부한다() {
    let zero = page_encodings_with_supplemental_pages(
        base_used_pages(),
        &[supplemental_page(0, "한")],
        &source_codes(292),
        fixture_slot_count(),
    )
    .unwrap_err();
    assert!(zero.to_string().contains("ID 0"));

    let next_page_id = base_used_pages()
        .keys()
        .next_back()
        .copied()
        .unwrap()
        .checked_add(1)
        .unwrap();
    let duplicate = page_encodings_with_supplemental_pages(
        base_used_pages(),
        &[
            supplemental_page(next_page_id, "한"),
            supplemental_page(next_page_id, "글"),
        ],
        &source_codes(292),
        fixture_slot_count(),
    )
    .unwrap_err();
    assert!(duplicate.to_string().contains("ID가 중복"));

    let collision = page_encodings_with_supplemental_pages(
        base_used_pages(),
        &[supplemental_page(2, "한")],
        &source_codes(292),
        fixture_slot_count(),
    )
    .unwrap_err();
    assert!(collision.to_string().contains("기존 VDD page와 충돌"));
}

#[test]
fn supplemental의_빈집합_중복_제어문자를_거부한다() {
    let empty = page_encodings_with_supplemental_pages(
        base_used_pages(),
        &[supplemental_page(3, "")],
        &source_codes(292),
        fixture_slot_count(),
    )
    .unwrap_err();
    assert!(empty.to_string().contains("비었거나 공백뿐임"));

    let whitespace = page_encodings_with_supplemental_pages(
        base_used_pages(),
        &[supplemental_page(3, " ")],
        &source_codes(292),
        fixture_slot_count(),
    )
    .unwrap_err();
    assert!(whitespace.to_string().contains("비었거나 공백뿐임"));

    let duplicate = page_encodings_with_supplemental_pages(
        base_used_pages(),
        &[supplemental_page(3, "한글한")],
        &source_codes(292),
        fixture_slot_count(),
    )
    .unwrap_err();
    assert!(duplicate.to_string().contains("사용 glyph가 중복"));

    let normalized_duplicate = page_encodings_with_supplemental_pages(
        base_used_pages(),
        &[supplemental_page(3, "!！")],
        &source_codes(292),
        fixture_slot_count(),
    )
    .unwrap_err();
    assert!(normalized_duplicate
        .to_string()
        .contains("사용 glyph가 중복"));

    let control = page_encodings_with_supplemental_pages(
        base_used_pages(),
        &[supplemental_page(3, "한\n글")],
        &source_codes(292),
        fixture_slot_count(),
    )
    .unwrap_err();
    assert!(control.to_string().contains("제어 문자"));
}

#[test]
fn supplemental의_중간page누락과_동적slot초과를_거부한다() {
    let gap = page_encodings_with_supplemental_pages(
        base_used_pages(),
        &[supplemental_page(4, "한")],
        &source_codes(292),
        fixture_slot_count(),
    )
    .unwrap_err();
    assert!(gap.to_string().contains("기존 VDD 뒤에서 연속"));

    let dynamic_capacity = fixture_slot_count() - QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX;
    let used_glyphs = (0..=dynamic_capacity)
        .map(|index| char::from_u32(0xAC00 + u32::try_from(index).unwrap()).unwrap())
        .collect::<String>();
    let overflow = page_encodings_with_supplemental_pages(
        base_used_pages(),
        &[QuestTranslationEncodingSupplementalPage {
            page_id: 3,
            used_glyphs,
        }],
        &source_codes(292),
        fixture_slot_count(),
    )
    .unwrap_err();
    assert!(overflow.to_string().contains("독립 bank slot 용량을 초과"));
}

#[test]
fn 공통glyph는_대상page의_같은code_prefix로_정렬한다() {
    let mut plan = QuestTranslationEncodingPlan {
        pages: vec![
            QuestTranslationEncodingPage {
                page_id: 1,
                glyphs: "가나다".to_owned(),
            },
            QuestTranslationEncodingPage {
                page_id: 2,
                glyphs: "나라마".to_owned(),
            },
        ],
        repertoire: "가나다라마".to_owned(),
        selected_routes: 0,
        retained_source_routes: 0,
        source_codes: BTreeMap::new(),
    };

    let prefix = align_quest_page_code_prefix(&mut plan, &[1, 2], "나").unwrap();
    assert_eq!(prefix, "나");
    assert_eq!(plan.page_glyphs(1), Some("나가다"));
    assert_eq!(plan.page_glyphs(2), Some("나라마"));
}

#[test]
fn 공통glyph의_dynamic공급이_page마다다르면_거부한다() {
    let mut plan = QuestTranslationEncodingPlan {
        pages: vec![
            QuestTranslationEncodingPage {
                page_id: 1,
                glyphs: "가나".to_owned(),
            },
            QuestTranslationEncodingPage {
                page_id: 2,
                glyphs: "나다".to_owned(),
            },
        ],
        repertoire: "가나다".to_owned(),
        selected_routes: 0,
        retained_source_routes: 0,
        source_codes: BTreeMap::new(),
    };

    assert!(align_quest_page_code_prefix(&mut plan, &[1, 2], "가").is_err());
}
