use super::*;

fn write_bank(gpu: &mut [u8], x_words: usize, word: u16) {
    for row in 0..BANK_HEIGHT_ROWS {
        let start = (QUEST_GLYPH_CACHE_VRAM_Y + row) * VRAM_ROW_BYTES + x_words * BYTES_PER_WORD;
        for offset in (start..start + BANK_WIDTH_WORDS * BYTES_PER_WORD).step_by(2) {
            gpu[offset..offset + 2].copy_from_slice(&word.to_le_bytes());
        }
    }
}

fn write_cell(gpu: &mut [u8], x_words: usize, y: usize, cell: &[u8]) {
    let row_bytes = QUEST_GLYPH_CACHE_CELL_BYTES / 16;
    for (row, source) in cell.chunks_exact(row_bytes).enumerate() {
        let start = (y + row) * VRAM_ROW_BYTES + x_words * BYTES_PER_WORD;
        gpu[start..start + row_bytes].copy_from_slice(source);
    }
}

#[test]
fn bank_점유는_vram_row_stride를_보존해_분류한다() {
    let mut gpu = vec![0_u8; VRAM_BYTE_LEN];
    write_bank(&mut gpu, QUEST_GLYPH_CACHE_SECONDARY_VRAM_X, 0x7FFF);

    let primary = summarize_bank(&gpu, QUEST_GLYPH_CACHE_VRAM_X).expect("primary 분류");
    let secondary =
        summarize_bank(&gpu, QUEST_GLYPH_CACHE_SECONDARY_VRAM_X).expect("secondary 분류");

    assert_eq!(primary.content, QuestVramBankContent::Zero);
    assert_eq!(primary.nonzero_words, 0);
    assert_eq!(secondary.content, QuestVramBankContent::UniformWord(0x7FFF));
    assert_eq!(secondary.nonzero_words, BANK_WIDTH_WORDS * BANK_HEIGHT_ROWS);
    assert_eq!(secondary.unique_words, 1);
}

#[test]
fn bank_밖의_pixel은_점유_digest와_분류를_바꾸지_않는다() {
    let mut gpu = vec![0_u8; VRAM_BYTE_LEN];
    let before = summarize_bank(&gpu, QUEST_GLYPH_CACHE_VRAM_X).expect("변경 전");
    let outside =
        QUEST_GLYPH_CACHE_VRAM_Y * VRAM_ROW_BYTES + (QUEST_GLYPH_CACHE_VRAM_X - 1) * BYTES_PER_WORD;
    gpu[outside..outside + 2].copy_from_slice(&0x1234_u16.to_le_bytes());
    let after = summarize_bank(&gpu, QUEST_GLYPH_CACHE_VRAM_X).expect("변경 후");

    assert_eq!(before, after);
}

#[test]
fn package_identity_불일치는_exact_guard에서_거부한다() {
    let error = require_exact(&[1, 2, 3], &[1, 9, 3], "package").expect_err("불일치");

    assert!(error.to_string().contains("offset=0x1"));
}

#[test]
fn 잘린_gpu는_bank_점유로_승격하지_않는다() {
    let gpu = vec![0_u8; VRAM_BYTE_LEN - 1];

    let error = summarize_bank(&gpu, QUEST_GLYPH_CACHE_SECONDARY_VRAM_X).expect_err("잘린 GPU");

    assert!(error.to_string().contains("VRAM row 범위 이탈"));
}

#[test]
fn resident_cell은선택된_bank의slot_payload를_exact대조한다() {
    let mut gpu = vec![0_u8; VRAM_BYTE_LEN];
    let cell = (0..QUEST_GLYPH_CACHE_CELL_BYTES)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let slot_x = QUEST_GLYPH_CACHE_VRAM_X + 3;
    let slot_y = QUEST_GLYPH_CACHE_VRAM_Y + 16;
    let assignments = vec![(slot_x, slot_y, cell.clone())];
    let secondary_x = slot_x + QUEST_GLYPH_CACHE_SECONDARY_VRAM_X - QUEST_GLYPH_CACHE_VRAM_X;
    write_cell(&mut gpu, secondary_x, slot_y, &cell);

    let exact =
        summarize_assigned_cells(&gpu, 112, QUEST_GLYPH_CACHE_SECONDARY_VRAM_X, &assignments)
            .expect("secondary resident cell 대조");
    assert!(exact.exact);
    assert!(exact.mismatched_slots.is_empty());
    assert_eq!(exact.expected_sha256, exact.actual_sha256);

    let corrupt = (slot_y + 5) * VRAM_ROW_BYTES + secondary_x * BYTES_PER_WORD + 2;
    gpu[corrupt] ^= 0x80;
    let mismatch =
        summarize_assigned_cells(&gpu, 112, QUEST_GLYPH_CACHE_SECONDARY_VRAM_X, &assignments)
            .expect("secondary resident cell 불일치 대조");
    assert!(!mismatch.exact);
    assert_eq!(mismatch.mismatched_slots, [0]);
    assert_ne!(mismatch.expected_sha256, mismatch.actual_sha256);
}

#[test]
fn 빈resident_assignment는exact로승격하지않는다() {
    let gpu = vec![0_u8; VRAM_BYTE_LEN];

    let error = summarize_assigned_cells(&gpu, 112, QUEST_GLYPH_CACHE_VRAM_X, &[])
        .expect_err("빈 assignment");

    assert!(error.to_string().contains("assignment가 비어 있음"));
}
