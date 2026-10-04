//! 18개 SEQ·GRP VDD의 모든 CNX를 다시 만드는 control 빌드.
//!
//! 해제 내용은 바꾸지 않고 CNX encoder만 통과시킨다. 따라서 이 산출물은
//! 네 subtype 전체의 정적 호환성·제자리 용량·디스크 쓰기 경계를 검증한다.
//! 실제 게임에서 어느 loader가 어느 자산을 소비했는지는 별도 런타임
//! 관측으로 판정해야 한다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::{
    compress, decompress, is_cnx, parse_header, verify_box_cnx_inventory, CnxInventorySummary,
    BOX_CNX_ARCHIVES,
};
use crate::disc::{apply_form1_write_plan, DiscImage, Iso9660, VerifiedSourceImage};
use crate::patch::WritePlan;

use super::vdd_plan::{add_vdd_repack, logical_file};

#[derive(Debug)]
struct AssetRecord {
    name: String,
    subtype: Option<[u8; 3]>,
    plain_or_raw: Vec<u8>,
}

#[derive(Debug)]
struct ArchiveRecord {
    path: &'static str,
    original: Vec<u8>,
    replacement: Vec<u8>,
    assets: Vec<AssetRecord>,
}

/// 전체 CNX control 빌드의 정적 검증 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllCnxControlReport {
    pub inventory: CnxInventorySummary,
    pub original_cnx_bytes: u64,
    pub recompressed_cnx_bytes: u64,
    pub byte_identical_cnx: usize,
    pub data_writes: usize,
    pub final_expected_writes: usize,
    pub touched_sectors: usize,
    pub image_sha256: String,
}

/// 완전히 검증된 전체 CNX control raw BIN.
#[derive(Debug)]
pub struct AllCnxControlBuild {
    pub image: Vec<u8>,
    pub report: AllCnxControlReport,
}

/// 전체 CNX control 파이프라인 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllCnxControlError(String);

impl fmt::Display for AllCnxControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for AllCnxControlError {}

fn error(message: impl Into<String>) -> AllCnxControlError {
    AllCnxControlError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn verify_final_archives(
    iso: &Iso9660<'_>,
    expected_inventory: CnxInventorySummary,
    records: &[ArchiveRecord],
) -> Result<(), AllCnxControlError> {
    let mut final_archives = Vec::with_capacity(records.len());
    for record in records {
        let final_vdd = iso
            .read_file(record.path)
            .map_err(|source| error(source.to_string()))?;
        if final_vdd != record.replacement {
            return Err(error(format!("{}: 최종 VDD readback 불일치", record.path)));
        }
        let archive = VddArchive::parse(&final_vdd).map_err(|source| error(source.to_string()))?;
        for asset in &record.assets {
            let entry = archive.find(&asset.name).ok_or_else(|| {
                error(format!("{}:{} 최종 readback 없음", record.path, asset.name))
            })?;
            let final_raw = archive.read_file(entry);
            match asset.subtype {
                Some(subtype) => {
                    let header =
                        parse_header(final_raw).map_err(|source| error(source.to_string()))?;
                    if header.subtype != subtype {
                        return Err(error(format!(
                            "{}:{} 최종 subtype 불일치",
                            record.path, asset.name
                        )));
                    }
                    let final_plain =
                        decompress(final_raw).map_err(|source| error(source.to_string()))?;
                    if final_plain != asset.plain_or_raw {
                        return Err(error(format!(
                            "{}:{} 최종 CNX 해제 readback 불일치",
                            record.path, asset.name
                        )));
                    }
                }
                None if final_raw != asset.plain_or_raw => {
                    return Err(error(format!(
                        "{}:{} raw 엔트리가 바뀜",
                        record.path, asset.name
                    )));
                }
                None => {}
            }
        }
        final_archives.push((record.path, archive));
    }

    let actual = verify_box_cnx_inventory(
        final_archives
            .iter()
            .map(|(path, archive)| (*path, archive)),
    )
    .map_err(|source| error(source.to_string()))?;
    if actual != expected_inventory {
        return Err(error("최종 CNX 인벤토리가 원본 계약과 다름"));
    }
    Ok(())
}

/// 지원 원본의 18개 VDD에서 1,289개 CNX를 모두 재압축한 control BIN을 만든다.
pub fn build_all_cnx_control(
    source: &VerifiedSourceImage,
) -> Result<AllCnxControlBuild, AllCnxControlError> {
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let mut originals = Vec::with_capacity(BOX_CNX_ARCHIVES.len());
    for spec in BOX_CNX_ARCHIVES {
        let iso_entry = iso
            .find(spec.path)
            .map_err(|source| error(source.to_string()))?
            .clone();
        let file = logical_file(&iso_entry).map_err(error)?;
        let raw = iso
            .read_file(spec.path)
            .map_err(|source| error(source.to_string()))?;
        let archive = VddArchive::parse(&raw).map_err(|source| error(source.to_string()))?;
        originals.push((spec.path, file, raw, archive));
    }

    let inventory = verify_box_cnx_inventory(
        originals
            .iter()
            .map(|(path, _, _, archive)| (*path, archive)),
    )
    .map_err(|source| error(source.to_string()))?;

    let mut data_plan = WritePlan::new();
    let mut records = Vec::with_capacity(originals.len());
    let mut original_cnx_bytes = 0u64;
    let mut recompressed_cnx_bytes = 0u64;
    let mut byte_identical_cnx = 0usize;

    for (archive_index, (path, file, original, archive)) in originals.into_iter().enumerate() {
        let mut replacements = BTreeMap::new();
        let mut assets = Vec::with_capacity(archive.entry_count());
        for entry in archive.entries() {
            let entry_raw = archive.read_file(entry);
            if !is_cnx(entry_raw) {
                assets.push(AssetRecord {
                    name: entry.name().to_string(),
                    subtype: None,
                    plain_or_raw: entry_raw.to_vec(),
                });
                continue;
            }

            let header = parse_header(entry_raw).map_err(|source| {
                error(format!("{path}:{} CNX 헤더 실패: {source}", entry.name()))
            })?;
            let plain = decompress(entry_raw).map_err(|source| {
                error(format!("{path}:{} CNX 해제 실패: {source}", entry.name()))
            })?;
            let replacement = compress(&plain, header.subtype).map_err(|source| {
                error(format!("{path}:{} CNX 압축 실패: {source}", entry.name()))
            })?;
            let restored = decompress(&replacement).map_err(|source| {
                error(format!(
                    "{path}:{} 재압축 CNX 해제 실패: {source}",
                    entry.name()
                ))
            })?;
            if restored != plain {
                return Err(error(format!(
                    "{path}:{} CNX 자기 readback 불일치",
                    entry.name()
                )));
            }

            original_cnx_bytes += entry_raw.len() as u64;
            recompressed_cnx_bytes += replacement.len() as u64;
            if replacement == entry_raw {
                byte_identical_cnx += 1;
            }
            assets.push(AssetRecord {
                name: entry.name().to_string(),
                subtype: Some(header.subtype),
                plain_or_raw: plain,
            });
            replacements.insert(entry.name().to_string(), replacement);
        }

        let replacement = archive
            .repack_in_container(&replacements)
            .map_err(|source| error(format!("{path}: {source}")))?;
        add_vdd_repack(
            &mut data_plan,
            file,
            &original,
            &replacement,
            &format!("cnx-all-{archive_index:02}"),
            "pipeline::cnx_control",
        )
        .map_err(error)?;
        records.push(ArchiveRecord {
            path,
            original,
            replacement,
            assets,
        });
    }

    if records
        .iter()
        .any(|record| record.original.len() != record.replacement.len())
    {
        return Err(error("전체 CNX control에서 VDD 크기가 바뀜"));
    }
    let data_writes = data_plan.writes.len();
    let touched_sectors = data_plan.touched_sectors().len();
    let final_expected_writes = data_writes + touched_sectors;
    let output = apply_form1_write_plan(source.image(), &data_plan)
        .map_err(|source| error(source.to_string()))?;

    let final_image = DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
    verify_final_archives(&final_iso, inventory, &records)?;
    let output = final_image.into_bytes();
    let image_sha256 = digest(&output);

    Ok(AllCnxControlBuild {
        image: output,
        report: AllCnxControlReport {
            inventory,
            original_cnx_bytes,
            recompressed_cnx_bytes,
            byte_identical_cnx,
            data_writes,
            final_expected_writes,
            touched_sectors,
            image_sha256,
        },
    })
}
