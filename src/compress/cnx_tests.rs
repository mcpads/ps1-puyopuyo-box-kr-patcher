use super::*;

fn literal_stream(data: &[u8], declared: u32) -> Vec<u8> {
    assert!(data.len() <= 255);
    let mut out = Vec::new();
    out.extend_from_slice(b"CNX\x02bin\x10");
    let body_len = u32::try_from(data.len() + 4).unwrap();
    out.extend_from_slice(&body_len.to_be_bytes());
    out.extend_from_slice(&declared.to_be_bytes());
    out.push(0x03);
    out.push(data.len() as u8);
    out.extend_from_slice(data);
    out.push(0);
    out.push(0);
    out
}

#[test]
fn parses_the_observed_header_shape() {
    let data = literal_stream(b"abc", 3);
    let header = parse_header(&data).unwrap();
    assert_eq!(header.subtype, *b"bin");
    assert_eq!(header.format_byte, 0x10);
    assert_eq!(header.decompressed_size, 3);
}

#[test]
fn decompresses_a_literal_run() {
    assert_eq!(decompress(&literal_stream(b"abc", 3)).unwrap(), b"abc");
}

#[test]
fn rejects_trailing_bytes_outside_the_declared_body() {
    let mut data = literal_stream(b"abc", 3);
    data.push(0xCD);
    assert!(matches!(
        decompress(&data),
        Err(CnxError::BodyLengthMismatch { .. })
    ));
}

#[test]
fn rejects_trailing_bytes_after_the_terminator() {
    let mut data = literal_stream(b"abc", 3);
    let old_body = u32::from_be_bytes(data[8..12].try_into().unwrap());
    data.extend_from_slice(&[0xCD, 0xCD]);
    data[8..12].copy_from_slice(&(old_body + 2).to_be_bytes());
    assert!(matches!(
        decompress(&data),
        Err(CnxError::TrailingBodyData { .. })
    ));
}

#[test]
fn rejects_a_missing_terminator() {
    let mut data = literal_stream(b"abc", 3);
    data.pop();
    let body_len = u32::from_be_bytes(data[8..12].try_into().unwrap()) - 1;
    data[8..12].copy_from_slice(&body_len.to_be_bytes());
    assert!(matches!(
        decompress(&data),
        Err(CnxError::MissingTerminator)
    ));
}

#[test]
fn rejects_output_larger_than_declared_before_returning_a_partial_result() {
    assert!(matches!(
        decompress(&literal_stream(b"abc", 2)),
        Err(CnxError::OutputTooLarge { .. })
    ));
}

#[test]
fn rejects_unsupported_header_fields() {
    let mut wrong_version = literal_stream(b"abc", 3);
    wrong_version[3] = 3;
    assert!(matches!(
        decompress(&wrong_version),
        Err(CnxError::UnsupportedVersion(3))
    ));

    let mut wrong_format = literal_stream(b"abc", 3);
    wrong_format[7] = 0;
    assert!(matches!(
        decompress(&wrong_format),
        Err(CnxError::UnsupportedFormatByte(0))
    ));
}

#[test]
fn self_roundtrip_covers_empty_short_repeated_and_page_crossing_inputs() {
    let cases = [
        Vec::new(),
        vec![0x7F],
        b"abc".to_vec(),
        b"Hello, World! Hello, World! Hello, World!".to_vec(),
        vec![0; 8_192],
        (0..12_000)
            .map(|i| ((i * 37 + i / 11 + 13) % 256) as u8)
            .collect(),
    ];
    for original in cases {
        let compressed = compress(&original, *b"tst").unwrap();
        assert!(is_cnx(&compressed));
        assert_eq!(decompress(&compressed).unwrap(), original);
    }
}

#[test]
fn encoder_output은_보수_크기_상한_안에_있다() {
    let mut state = 0xC0DE_5EEDu32;
    let pseudorandom = (0..65_536)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect::<Vec<_>>();
    for original in [vec![0; 65_536], pseudorandom] {
        let compressed = compress(&original, *b"tst").unwrap();
        assert!(compressed.len() <= cnx_output_size_upper_bound(original.len()).unwrap());
        assert_eq!(decompress(&compressed).unwrap(), original);
    }
    assert_eq!(cnx_output_size_upper_bound(0), Some(STREAM_PAGE_LEN));
    assert_eq!(cnx_output_size_upper_bound(usize::MAX), None);
}

#[test]
fn emitted_commands_do_not_cross_a_two_kibibyte_stream_page() {
    let original: Vec<u8> = (0..20_000)
        .map(|i| ((i * 73 + i / 7 + 19) % 251) as u8)
        .collect();
    let compressed = compress(&original, *b"tst").unwrap();
    let header = parse_header(&compressed).unwrap();
    let mut source = HEADER_LEN;
    let body_end = HEADER_LEN + header.compressed_size as usize;

    while source < body_end {
        let control = compressed[source];
        source += 1;
        if control == 0 {
            break;
        }
        let mut fields = control;
        for _ in 0..4 {
            let start_page = source / STREAM_PAGE_LEN;
            match fields & 3 {
                0 => {
                    let skip = compressed[source] as usize;
                    source += skip + 1;
                    break;
                }
                1 => source += 1,
                2 => source += 2,
                3 => {
                    let len = compressed[source] as usize;
                    source += len + 1;
                }
                _ => unreachable!(),
            }
            let end_page = (source - 1) / STREAM_PAGE_LEN;
            assert_eq!(start_page, end_page);
            fields >>= 2;
        }
    }
    assert_eq!(decompress(&compressed).unwrap(), original);
}
