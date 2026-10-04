use super::*;

fn bank() -> (Vec<u8>, Vec<u8>) {
    let programs = 2usize;
    let header_bytes = HEADER_BYTES
        + PROGRAM_COUNT * PROGRAM_BYTES
        + programs * TONES_PER_PROGRAM * TONE_BYTES
        + WAVE_SIZE_COUNT * WAVE_SIZE_BYTES;
    let body = vec![0x5a; 40];
    let mut header = vec![0; header_bytes];
    header[..4].copy_from_slice(VAB_MAGIC);
    header[4..8].copy_from_slice(&VAB_VERSION.to_le_bytes());
    header[8..12].copy_from_slice(&9u32.to_le_bytes());
    header[0x0c..0x10].copy_from_slice(&((header_bytes + body.len()) as u32).to_le_bytes());
    header[0x12..0x14].copy_from_slice(&(programs as u16).to_le_bytes());
    header[0x14..0x16].copy_from_slice(&2u16.to_le_bytes());
    header[0x16..0x18].copy_from_slice(&2u16.to_le_bytes());
    header[0x18] = 127;
    header[0x19] = 64;

    let first_program = program_offset(0);
    header[first_program..first_program + 5].copy_from_slice(&[1, 110, 7, 0, 32]);
    let second_program = program_offset(2);
    header[second_program..second_program + 5].copy_from_slice(&[1, 120, 6, 1, 64]);

    let tone_table = HEADER_BYTES + PROGRAM_COUNT * PROGRAM_BYTES;
    header[tone_table + 2..tone_table + 6].copy_from_slice(&[100, 20, 60, 1]);
    header[tone_table + 0x14] = 0;
    header[tone_table + 0x16..tone_table + 0x18].copy_from_slice(&1u16.to_le_bytes());
    let second_tone = tone_table + TONES_PER_PROGRAM * TONE_BYTES;
    header[second_tone + 2..second_tone + 6].copy_from_slice(&[127, 64, 83, 59]);
    header[second_tone + 0x14] = 2;
    header[second_tone + 0x16..second_tone + 0x18].copy_from_slice(&2u16.to_le_bytes());

    let wave_table = tone_table + programs * TONES_PER_PROGRAM * TONE_BYTES;
    header[wave_table + 2..wave_table + 4].copy_from_slice(&2u16.to_le_bytes());
    header[wave_table + 4..wave_table + 6].copy_from_slice(&3u16.to_le_bytes());
    (header, body)
}

#[test]
fn 비연속_program의_compact_tone_block과_sample_범위를_읽는다() {
    let (header, body) = bank();
    let parsed = VabBank::parse(&header, &body).unwrap();

    assert_eq!(parsed.version, 7);
    assert_eq!(parsed.bank_id, 9);
    assert_eq!(parsed.programs.len(), 2);
    assert_eq!(parsed.programs[0].number, 0);
    assert_eq!(parsed.programs[1].number, 2);
    assert_eq!(parsed.programs[1].tones[0].sample_number, 2);
    assert_eq!(parsed.samples[0].body_range, 0..16);
    assert_eq!(parsed.samples[1].body_range, 16..40);
    assert_eq!(parsed.sample_containing(39).unwrap().number, 2);
    assert!(parsed.sample_containing(40).is_none());
    assert_eq!(parsed.tones_for_sample(2)[0].program_number, 2);
}

#[test]
fn header와_body_길이_drift를_거부한다() {
    let (mut header, body) = bank();
    header.pop();
    assert!(matches!(
        VabBank::parse(&header, &body),
        Err(VabError::HeaderLengthMismatch { .. })
    ));

    let (header, mut body) = bank();
    body.pop();
    assert!(matches!(
        VabBank::parse(&header, &body),
        Err(VabError::BodyLengthMismatch { .. })
    ));
}

#[test]
fn program_tone_sample_교차참조_drift를_거부한다() {
    let (mut header, body) = bank();
    let tone_table = HEADER_BYTES + PROGRAM_COUNT * PROGRAM_BYTES;
    let second_tone = tone_table + TONES_PER_PROGRAM * TONE_BYTES;
    header[second_tone + 0x14] = 1;
    assert!(matches!(
        VabBank::parse(&header, &body),
        Err(VabError::ToneProgramMismatch { .. })
    ));

    let (mut header, body) = bank();
    header[second_tone + 0x16..second_tone + 0x18].copy_from_slice(&3u16.to_le_bytes());
    assert!(matches!(
        VabBank::parse(&header, &body),
        Err(VabError::ToneSampleOutOfRange { .. })
    ));
}

#[test]
fn version과선언길이_drift를_거부한다() {
    let (mut header, body) = bank();
    header[4..8].copy_from_slice(&6u32.to_le_bytes());
    assert_eq!(
        VabBank::parse(&header, &body),
        Err(VabError::UnsupportedVersion(6))
    );

    let (mut header, body) = bank();
    header[0x0c..0x10].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        VabBank::parse(&header, &body),
        Err(VabError::DeclaredLengthMismatch { .. })
    ));
}
