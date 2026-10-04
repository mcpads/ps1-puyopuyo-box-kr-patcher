use super::*;
#[test]
fn 같은창의_모든_정렬복귀주소를_손실없이_표현한다() {
    for offset in (4..QUEST_SEQ_ACTIVE_WINDOW_BYTES as u32).step_by(4) {
        let target = QUEST_SEQ_ACTIVE_RUNTIME_BASE + offset;
        let record = encode_return(target).unwrap();
        assert_eq!(&record[..2], &[0x21, 0xFF]);
        assert_eq!(
            QUEST_SEQ_ACTIVE_RUNTIME_BASE
                + u32::from(u16::from_le_bytes([record[2], record[3]])) * 4,
            target
        );
    }
    for target in [
        0,
        QUEST_SEQ_ACTIVE_RUNTIME_BASE,
        QUEST_SEQ_ACTIVE_RUNTIME_BASE + 1,
        QUEST_SEQ_ACTIVE_RUNTIME_BASE + 32768,
        u32::MAX,
    ] {
        assert!(encode_return(target).is_err());
    }
}
