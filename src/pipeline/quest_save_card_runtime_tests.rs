use super::*;

const TEST_IMAGE_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn finish_frame_checksum(frame: &mut [u8]) {
    frame[127] = frame[..127]
        .iter()
        .fold(0_u8, |checksum, byte| checksum ^ byte);
}

fn blank_card() -> Vec<u8> {
    let mut card = vec![0; PS1_MEMORY_CARD_BYTES];
    card[..2].copy_from_slice(b"MC");
    finish_frame_checksum(&mut card[..PS1_MEMORY_CARD_FRAME_BYTES]);
    for index in PS1_MEMORY_CARD_DIRECTORY_FRAMES {
        let start = index * PS1_MEMORY_CARD_FRAME_BYTES;
        let entry = &mut card[start..start + PS1_MEMORY_CARD_FRAME_BYTES];
        entry[0] = PS1_MEMORY_CARD_FREE_STATUS;
        entry[8..10].copy_from_slice(&u16::MAX.to_le_bytes());
        finish_frame_checksum(entry);
    }
    card
}

fn saved_card() -> Vec<u8> {
    let mut card = blank_card();
    let first = &mut card[PS1_MEMORY_CARD_FRAME_BYTES..PS1_MEMORY_CARD_FRAME_BYTES * 2];
    first.fill(0);
    first[0] = PS1_MEMORY_CARD_FIRST_BLOCK_STATUS;
    first[4..8].copy_from_slice(&PUYO_BOX_SAVE_BYTES.to_le_bytes());
    first[8..10].copy_from_slice(&u16::MAX.to_le_bytes());
    first[10..10 + PUYO_BOX_SAVE_FILENAME.len()].copy_from_slice(PUYO_BOX_SAVE_FILENAME.as_bytes());
    finish_frame_checksum(first);
    card[PS1_MEMORY_CARD_FIRST_DATA_BLOCK_OFFSET..PS1_MEMORY_CARD_FIRST_DATA_BLOCK_OFFSET + 2]
        .copy_from_slice(b"SC");
    card
}

#[test]
fn exact_puyo_box_save_entry를_승인한다() {
    let blank = blank_card();
    let saved = saved_card();

    let report = verify_card_contract(TEST_IMAGE_SHA256, &blank, &saved).unwrap();

    assert_eq!(report.directory_status, 0x51);
    assert_eq!(report.declared_save_bytes, 0x2000);
    assert_eq!(report.filename, PUYO_BOX_SAVE_FILENAME);
    assert!(report.persistent_card_verified);
    assert!(!report.cold_load_verified);
    assert!(!report.release_eligible);
}

#[test]
fn 저장_완료_문구만_거친_system_area_diff를_거부한다() {
    let blank = blank_card();
    let mut incomplete = blank.clone();
    incomplete[0x1F80] = 3;

    let error = verify_card_contract(TEST_IMAGE_SHA256, &blank, &incomplete).unwrap_err();

    assert!(error.to_string().contains("save directory 불일치"));
}

#[test]
fn directory_checksum_손상을_거부한다() {
    let blank = blank_card();
    let mut saved = saved_card();
    saved[0x90] ^= 1;

    let error = verify_card_contract(TEST_IMAGE_SHA256, &blank, &saved).unwrap_err();

    assert!(error.to_string().contains("checksum"));
}

#[test]
fn 잘못된_image_digest형식을_거부한다() {
    let error = verify_card_contract("not-a-sha256", &blank_card(), &saved_card()).unwrap_err();

    assert!(error.to_string().contains("image SHA-256 형식 불일치"));
}
