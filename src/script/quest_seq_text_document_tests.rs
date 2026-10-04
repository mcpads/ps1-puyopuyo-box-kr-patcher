use super::*;
use crate::font::{QuestCodeTable, QUEST_CODE_TABLE_COUNT};
use crate::script::audit_quest_seq_text_extraction;

fn code_table_with(codes: &[[u8; 2]]) -> QuestCodeTable {
    let mut table = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| {
            let value = u16::try_from(index).expect("test index");
            value.to_le_bytes()
        })
        .collect::<Vec<_>>();
    for (index, code) in codes.iter().copied().enumerate() {
        table[index] = code;
    }
    let raw = table.into_iter().flatten().collect::<Vec<_>>();
    QuestCodeTable::parse(&raw).expect("test code table")
}

#[test]
fn 전_파일을_opaque와_text_stream으로_빈틈없이_왕복한다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let text = [
        0xAA, 0xBB, 0xCC, 0xDD, 0x21, 0, 0, 0, 0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0, 0xEE, 0xFF,
    ];
    let opaque = [1, 2, 3, 4, 5, 6];
    let extraction = audit_quest_seq_text_extraction(
        [
            ("TEXT.SEQ", text.as_slice()),
            ("OPAQUE.SEQ", opaque.as_slice()),
        ],
        &code_table,
    )
    .unwrap();
    let analysis = audit_quest_seq_text_documents(
        [
            ("OPAQUE.SEQ", opaque.as_slice()),
            ("TEXT.SEQ", text.as_slice()),
        ],
        &extraction,
    )
    .unwrap();

    assert_eq!(analysis.documents.len(), 2);
    assert_eq!(analysis.text_stream_chunks, 1);
    assert_eq!(analysis.text_stream_bytes, 12);
    let text_document = analysis
        .documents
        .iter()
        .find(|document| document.sequence == "TEXT.SEQ")
        .unwrap();
    assert_eq!(text_document.chunks.len(), 3);
    assert_eq!(
        serialize_quest_seq_text_document(text_document),
        Ok(text.to_vec())
    );
    let opaque_document = analysis
        .documents
        .iter()
        .find(|document| document.sequence == "OPAQUE.SEQ")
        .unwrap();
    assert_eq!(opaque_document.chunks.len(), 1);
    assert_eq!(
        serialize_quest_seq_text_document(opaque_document),
        Ok(opaque.to_vec())
    );
}

#[test]
fn 겹침_빈틈_원본_drift는_부분_출력하지_않는다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let raw = [0x21, 0, 0, 0, 0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0];
    let extraction =
        audit_quest_seq_text_extraction([("TEST.SEQ", raw.as_slice())], &code_table).unwrap();
    let analysis =
        audit_quest_seq_text_documents([("TEST.SEQ", raw.as_slice())], &extraction).unwrap();

    let mut gap = analysis.documents[0].clone();
    gap.chunks[0] = QuestSeqTextDocumentChunk::Opaque {
        offset: 1,
        raw: raw.to_vec(),
    };
    assert!(serialize_quest_seq_text_document(&gap).is_err());

    let mut drift = analysis.documents[0].clone();
    let QuestSeqTextDocumentChunk::TextStream(stream) = &mut drift.chunks[0] else {
        panic!("첫 chunk는 text stream");
    };
    stream.tokens[0].raw[0] ^= 1;
    assert!(serialize_quest_seq_text_document(&drift).is_err());

    let mut wrong_owner = analysis.documents[0].clone();
    let QuestSeqTextDocumentChunk::TextStream(stream) = &mut wrong_owner.chunks[0] else {
        panic!("첫 chunk는 text stream");
    };
    stream.sequence = "OTHER.SEQ".to_string();
    assert!(serialize_quest_seq_text_document(&wrong_owner).is_err());
}

#[test]
fn 입력_순서와_무관하게_문서_요약이_결정적이다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let a = [0x21, 0, 0, 0, 0x00, 0xFF, 0, 0, 0, 0];
    let b = [1, 2, 3, 4];
    let extraction = audit_quest_seq_text_extraction(
        [("B.SEQ", b.as_slice()), ("A.SEQ", a.as_slice())],
        &code_table,
    )
    .unwrap();
    let left = audit_quest_seq_text_documents(
        [("B.SEQ", b.as_slice()), ("A.SEQ", a.as_slice())],
        &extraction,
    )
    .unwrap();
    let right = audit_quest_seq_text_documents(
        [("A.SEQ", a.as_slice()), ("B.SEQ", b.as_slice())],
        &extraction,
    )
    .unwrap();
    assert_eq!(
        render_quest_seq_text_document_summary_tsv(&left),
        render_quest_seq_text_document_summary_tsv(&right)
    );
}

#[test]
fn 중복_입력과_추출_분모_drift를_거부한다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let raw = [0u8; 4];
    let extraction =
        audit_quest_seq_text_extraction([("TEST.SEQ", raw.as_slice())], &code_table).unwrap();
    assert!(audit_quest_seq_text_documents(
        [("TEST.SEQ", raw.as_slice()), ("TEST.SEQ", raw.as_slice())],
        &extraction,
    )
    .unwrap_err()
    .to_string()
    .contains("입력 중복"));

    let mut drift = extraction;
    drift.source_bytes += 1;
    assert!(
        audit_quest_seq_text_documents([("TEST.SEQ", raw.as_slice())], &drift)
            .unwrap_err()
            .to_string()
            .contains("분모 불일치")
    );
}
