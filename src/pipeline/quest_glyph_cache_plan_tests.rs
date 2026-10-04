use super::*;

#[test]
fn packbits는_literal_repeat와_128byte경계를_왕복한다() {
    let mut raw = (0u8..=127).collect::<Vec<_>>();
    raw.extend([7; 128]);
    raw.extend([1, 1, 2, 2, 3, 3]);
    let encoded = encode_packbits(&raw);
    assert_eq!(decode_packbits(&encoded, raw.len()), Ok(raw));
}

#[test]
fn packbits는_잘림_예약control과_길이초과를_거부한다() {
    assert!(decode_packbits(&[0], 1).is_err());
    assert!(decode_packbits(&[128], 0).is_err());
    assert!(decode_packbits(&[255, 1], 1).is_err());
}

#[test]
fn page_index는_delta_uleb128로_경계값을_왕복한다() {
    let indices = [0, 1, 127, 128, 255, 256, 511, 636];
    let encoded = encode_page_indices(&indices).unwrap();
    assert_eq!(decode_page_indices(&encoded), Ok(indices.to_vec()));
    assert!(encoded.len() < indices.len() * QUEST_GLYPH_CACHE_PLAIN_ASSIGNMENT_BYTES);
}

#[test]
fn page_index는_역전과_재시작을_압축하며_기존absolute도_읽는다() {
    let indices = [12, 3, 511, 7];
    let encoded = encode_page_indices(&indices).unwrap();
    assert!(encoded.len() < 2 + indices.len() * 2);
    assert_eq!(
        decode_page_indices(&encode_absolute_page_indices(&indices)),
        Ok(indices.to_vec())
    );
    assert_eq!(decode_page_indices(&encoded), Ok(indices.to_vec()));
}

#[test]
fn page_index는_잘림_중복_delta와_u16초과를_거부한다() {
    assert!(decode_page_indices(&[0x80]).is_err());
    assert!(decode_page_indices(&[1, 0]).is_err());
    assert!(decode_page_indices(&[0xFF, 0xFF, 0x04]).is_err());
    assert!(decode_page_indices(&[0x80, 0x00, 1]).is_err());
}

#[test]
fn 열세bit_row_index는_한_cell의_열여섯행을_정확히_왕복한다() {
    let indices = (0..QUEST_GLYPH_CACHE_ROWS_PER_CELL)
        .map(|index| u16::try_from(1 + index * 509).unwrap())
        .collect::<Vec<_>>();
    let packed = pack_row_indices(&indices).unwrap();
    assert_eq!(packed.len(), QUEST_GLYPH_CACHE_MAX_CELL_ROW_INDEX_BYTES);
    assert_eq!(unpack_row_indices(&packed), Ok(indices));
}

#[test]
fn row_index는_행수와_열세bit범위를_벗어나면_거부한다() {
    assert!(pack_row_indices(&[0; QUEST_GLYPH_CACHE_ROWS_PER_CELL - 1]).is_err());
    let mut indices = [0; QUEST_GLYPH_CACHE_ROWS_PER_CELL];
    indices[3] = QUEST_GLYPH_CACHE_ROW_INDEX_LIMIT as u16;
    assert!(pack_row_indices(&indices).is_err());
    assert!(unpack_row_indices(&[0; QUEST_GLYPH_CACHE_MAX_CELL_ROW_INDEX_BYTES - 1]).is_err());
}

#[test]
fn 빈_행의_모든_bitmap과_index_경계를_정확히_왕복한다() {
    for mask in 0..=u16::MAX {
        let indices = (0..16)
            .map(|row| {
                if mask & (1 << row) == 0 {
                    0
                } else if row % 2 == 0 {
                    1
                } else {
                    8191
                }
            })
            .collect::<Vec<_>>();
        let packed = pack_row_indices(&indices).unwrap();
        assert_eq!(unpack_row_indices(&packed).unwrap(), indices);
        assert_eq!(
            packed.len(),
            2 + (mask.count_ones() as usize * 13).div_ceil(8)
        );
    }
    assert_eq!(pack_row_indices(&[0; 16]).unwrap(), [0, 0]);
    assert!(unpack_row_indices(&[0]).is_err());
    assert!(unpack_row_indices(&[1, 0, 0, 0]).is_err());
    assert!(unpack_row_indices(&[1, 0, 1, 0xE0]).is_err());
    assert!(unpack_row_indices(&[1, 0, 1]).is_err());
}

#[test]
fn 가변_cell_directory는_혼합_길이와_투명_원형을_보존하고_손상을_거부한다() {
    let mut partial = vec![0; QUEST_GLYPH_CACHE_CELL_BYTES];
    partial[12..18].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
    let cells = vec![vec![0; 96], partial, vec![255; 96]];
    let catalog = encode_row_dictionary_catalog(&cells).unwrap();
    assert_eq!(catalog.rows[0], [0; 6]);
    for (index, expected) in cells.iter().enumerate() {
        let record =
            cell_row_index_record(&catalog.packed_cell_indices, cells.len(), index).unwrap();
        let actual = unpack_row_indices(record)
            .unwrap()
            .into_iter()
            .flat_map(|row| catalog.rows[usize::from(row)])
            .collect::<Vec<_>>();
        assert_eq!(&actual, expected);
    }
    let mut bad = catalog.packed_cell_indices.clone();
    bad[2..4].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(cell_row_index_record(&bad, cells.len(), 0).is_err());
    assert!(cell_row_index_record(&bad[..4], cells.len(), 0).is_err());
    assert!(cell_row_index_record(&catalog.packed_cell_indices, cells.len(), cells.len()).is_err());
    // 투명 행이 원래 없는 글자에도 row 0은 명시적으로 들어간다.
    let opaque = encode_row_dictionary_catalog(&[vec![255; 96]]).unwrap();
    assert_eq!(opaque.rows[0], [0; 6]);
}

#[test]
fn sparse_upload_slot은_첫wide행을_피하고_halfword에_정렬된다() {
    let slots = aligned_wide_slots().unwrap();
    assert_eq!(
        slots.len(),
        QUEST_FONT_WIDE_SLOTS - QUEST_FONT_FIRST_ROW_WIDE_SLOTS
    );
    assert_eq!(
        slots[0].index,
        QUEST_FONT_NARROW_SLOTS + QUEST_FONT_FIRST_ROW_WIDE_SLOTS
    );
    assert!(slots.iter().all(|slot| slot.x % 4 == 0));
}

#[test]
fn scratch_padding은_앞section크기에서_파생된다() {
    assert_eq!(scratch_alignment_padding(0), 0);
    assert_eq!(scratch_alignment_padding(1), 3);
    assert_eq!(scratch_alignment_padding(2), 2);
    assert_eq!(scratch_alignment_padding(3), 1);
    assert_eq!(scratch_alignment_padding(4), 0);
}

#[test]
fn 원본_기호도_denki로_다시_그린다() {
    let slot = QuestGlyphSlot::for_index(QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX).unwrap();
    let mut atlas = vec![0; QUEST_FONT_LAYOUT.width * QUEST_FONT_LAYOUT.height];
    for row in 0..slot.height {
        for column in 0..slot.width {
            atlas[(slot.y + row) * QUEST_FONT_LAYOUT.width + slot.x + column] =
                u8::try_from(1 + (row + column) % 15).unwrap();
        }
    }
    let repertoire = BTreeSet::from(['「']);
    let source_slots = BTreeMap::from([('「', slot)]);

    let cells = render_quest_glyph_cache_repertoire_cells_with_source_atlas(
        &repertoire,
        &atlas,
        &source_slots,
    )
    .unwrap();

    assert_ne!(cells, vec![extract_cell(&atlas, slot).unwrap()]);
    assert_eq!(
        cells,
        render_quest_glyph_cache_repertoire_cells(&repertoire).unwrap()
    );
}

#[test]
fn 그림_기호의_동적_cell은_원본_픽셀을_유지한다() {
    let slot = QuestGlyphSlot::for_index(185).unwrap();
    let mut atlas = vec![0; QUEST_FONT_LAYOUT.width * QUEST_FONT_LAYOUT.height];
    for row in 0..slot.height {
        for col in 0..slot.width {
            atlas[(slot.y + row) * QUEST_FONT_LAYOUT.width + slot.x + col] =
                ((row + col) % 16) as u8;
        }
    }
    for glyph in ['●', '□', '♪', '＊'] {
        let cells = render_quest_glyph_cache_repertoire_cells_with_source_atlas(
            &BTreeSet::from([glyph]),
            &atlas,
            &BTreeMap::from([(glyph, slot)]),
        )
        .unwrap();
        assert_eq!(cells, vec![extract_cell(&atlas, slot).unwrap()]);
    }
}

#[test]
fn 공백_slot은_page에서_승격한_위치를_따른다() {
    let first = QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX;
    assert_eq!(
        space_slot_for_page(&['가', '　', '나'], 292),
        Some((first + 1) as u16)
    );
}

#[test]
fn 공백을_승격하지_않은_page는_보존된_원본_slot을_쓴다() {
    assert_eq!(space_slot_for_page(&['가', '나'], 292), Some(292));
}

#[test]
fn 원본_공백_slot을_다른_글자가_차지하면_그_글자를_좁히지_않는다() {
    let first = QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX;
    assert_eq!(space_slot_for_page(&['가', '나'], first + 1), None);
}
#[test]
fn 메뉴와_설명의_동일_문자는_같은_denki_cell로_공급한다() {
    let menu = crate::font::selected::menu_catalog_text("장")
        .chars()
        .next()
        .unwrap();
    let repertoire = std::collections::BTreeSet::from(['장', menu]);
    let cells = super::render_quest_glyph_cache_repertoire_cells(&repertoire).unwrap();
    assert_eq!(cells.len(), 2);
    assert_eq!(cells[0], cells[1]);
    let plain =
        super::render_quest_glyph_cache_repertoire_cells(&std::collections::BTreeSet::from(['장']))
            .unwrap();
    assert_eq!(cells[0], plain[0]);
}
