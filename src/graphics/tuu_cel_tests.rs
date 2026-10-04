use super::*;

fn fixture(records: &[[u16; RECORD_FIELDS]]) -> Vec<u8> {
    records
        .iter()
        .flat_map(|record| record.iter().flat_map(|value| value.to_le_bytes()))
        .collect()
}

#[test]
fn u16_le_10개_레코드를_바이트_동일하게_왕복한다() {
    let raw = fixture(&[
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        [0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29],
    ]);
    let table = TuuCelTable::parse(&raw).unwrap();

    assert_eq!(table.records().len(), 2);
    assert_eq!(table.records()[0].values(), &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(table.serialize(), raw);
}

#[test]
fn 빈_입력과_불완전한_레코드를_거부한다() {
    assert_eq!(TuuCelTable::parse(&[]), Err(TuuCelError::Empty));
    assert_eq!(
        TuuCelTable::parse(&[0; RECORD_LEN - 1]),
        Err(TuuCelError::LengthNotMultiple {
            actual: RECORD_LEN - 1,
            record_len: RECORD_LEN,
        })
    );
    assert_eq!(
        TuuCelTable::parse(&[0; RECORD_LEN + 1]),
        Err(TuuCelError::LengthNotMultiple {
            actual: RECORD_LEN + 1,
            record_len: RECORD_LEN,
        })
    );
}
