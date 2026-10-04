//! 압축 경계.
//!
//! 조사 스크립트의 해제 결과를 제품 구현의 근거로 직접 사용하지 않는다.
//! 제품 경로는 이 모듈에서 입력 경계와 출력 크기를 다시 검증한다.

pub mod cnx;
pub mod inventory;

pub use cnx::{
    cnx_output_size_upper_bound, compress, decompress, is_cnx, parse_header, CnxError, CnxHeader,
};
pub use inventory::{
    audit_archives as audit_cnx_archives, verify_box_inventory as verify_box_cnx_inventory,
    ArchiveCnxInventory, CnxInventoryError, CnxInventorySummary, BOX_CNX_ARCHIVES,
    BOX_CNX_INVENTORY,
};
