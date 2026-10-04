//! 원본 컨테이너와 BTX runtime 검증 명령.

use std::fs;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::btx::PixelFormat;
use ps1_puyopuyo_box::compress::{compress, decompress, is_cnx, parse_header};
use ps1_puyopuyo_box::coords::exe_map;
use ps1_puyopuyo_box::disc::{Iso9660, PsxExe, VerifiedSourceImage};
use ps1_puyopuyo_box::graphics::inventory::{audit_archives, verify_box_inventory};
use ps1_puyopuyo_box::graphics::runtime_layout::infer_unique_vram_layout;
use ps1_puyopuyo_box::source::BOX_EXE_ISO_PATHS;

use super::usage::usage;
use super::{AuditTotals, BoxExecutable};

pub(super) fn read_box_executables(iso: &Iso9660<'_>) -> Result<Vec<BoxExecutable>, String> {
    let mut executables = Vec::with_capacity(BOX_EXE_ISO_PATHS.len());
    for (iso_path, exe_name) in BOX_EXE_ISO_PATHS {
        let entry = iso.find(iso_path).map_err(|error| error.to_string())?;
        let map = exe_map(exe_name).ok_or_else(|| format!("{exe_name}: EXE 사상이 없다"))?;
        let [extent] = entry.extents() else {
            return Err(format!(
                "{iso_path}: EXE는 단일 extent여야 함: actual={}",
                entry.extents().len()
            ));
        };
        if extent.lba != map.file.lba {
            return Err(format!(
                "{iso_path}: EXE LBA 불일치: expected={}, actual={}",
                map.file.lba, extent.lba
            ));
        }
        let raw = iso.read_file(iso_path).map_err(|error| error.to_string())?;
        let exe = PsxExe::parse(&raw).map_err(|error| format!("{iso_path}: {error}"))?;
        exe.verify_map(map)
            .map_err(|error| format!("{iso_path}: {error}"))?;
        if exe.initial_sp().map_err(|error| error.to_string())? != 0x801f_fff0 {
            return Err(format!("{iso_path}: 초기 SP가 0x801FFFF0이 아님"));
        }
        executables.push(BoxExecutable {
            map,
            text: exe.text().to_vec(),
        });
    }
    Ok(executables)
}

pub(super) fn audit_vdd_data<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    require_original_bytes: bool,
) -> Result<(), String> {
    let mut totals = AuditTotals::default();
    for (source_id, raw) in sources {
        let archive = VddArchive::parse(raw)
            .map_err(|error| format!("{source_id} VDD 파싱 실패: {error}"))?;
        let serialized = archive.serialize_lossless();
        if serialized != raw {
            return Err(format!("{source_id} VDD lossless 직렬화가 원본과 다름"));
        }

        let mut archive_cnx = 0usize;
        let mut archive_raw = 0usize;
        let mut archive_byte_identical = 0usize;
        let mut archive_not_larger = 0usize;
        for entry in archive.entries() {
            let entry_raw = archive.read_file(entry);
            if !is_cnx(entry_raw) {
                archive_raw += 1;
                continue;
            }

            let header = parse_header(entry_raw)
                .map_err(|error| format!("{source_id}:{} CNX 헤더 실패: {error}", entry.name()))?;
            let plain = decompress(entry_raw)
                .map_err(|error| format!("{source_id}:{} CNX 해제 실패: {error}", entry.name()))?;
            let recompressed = compress(&plain, header.subtype)
                .map_err(|error| format!("{source_id}:{} CNX 압축 실패: {error}", entry.name()))?;
            let restored = decompress(&recompressed).map_err(|error| {
                format!("{source_id}:{} 재압축 CNX 해제 실패: {error}", entry.name(),)
            })?;
            if restored != plain {
                return Err(format!(
                    "{source_id}:{} CNX 자기 라운드트립 불일치",
                    entry.name(),
                ));
            }

            archive_cnx += 1;
            if recompressed == entry_raw {
                archive_byte_identical += 1;
            }
            if recompressed.len() <= entry_raw.len() {
                archive_not_larger += 1;
            }
            totals.original_cnx_bytes += entry_raw.len() as u64;
            totals.recompressed_cnx_bytes += recompressed.len() as u64;
            totals.largest_growth = totals
                .largest_growth
                .max(recompressed.len() as i64 - entry_raw.len() as i64);
        }

        println!(
            "PASS {}: VDD {} entries, CNX A {}/{}, CNX B {}/{}, 원본 이하 {}/{}, raw {}",
            source_id,
            archive.entry_count(),
            archive_cnx,
            archive_cnx,
            archive_byte_identical,
            archive_cnx,
            archive_not_larger,
            archive_cnx,
            archive_raw
        );
        totals.archives += 1;
        totals.entries += archive.entry_count();
        totals.cnx += archive_cnx;
        totals.raw += archive_raw;
        totals.cnx_self_roundtrip += archive_cnx;
        totals.cnx_original_bytes += archive_byte_identical;
        totals.cnx_not_larger += archive_not_larger;
    }

    if totals.archives == 0 {
        return Err("검증할 VDD 경로가 없다".to_string());
    }
    println!(
        "합계: VDD {}, entries {}, CNX A {}/{}, CNX B {}/{}, 원본 이하 {}/{}, raw {}, \
         CNX 크기 {}B -> {}B, 최대 증가 {}B",
        totals.archives,
        totals.entries,
        totals.cnx_self_roundtrip,
        totals.cnx,
        totals.cnx_original_bytes,
        totals.cnx,
        totals.cnx_not_larger,
        totals.cnx,
        totals.raw,
        totals.original_cnx_bytes,
        totals.recompressed_cnx_bytes,
        totals.largest_growth
    );
    if require_original_bytes && totals.cnx_original_bytes != totals.cnx {
        return Err(format!(
            "CNX 원본 바이트 재현 실패: {}/{}",
            totals.cnx_original_bytes, totals.cnx
        ));
    }
    Ok(())
}

pub(super) fn audit_vdds(paths: &[PathBuf], require_original_bytes: bool) -> Result<(), String> {
    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let raw =
            fs::read(path).map_err(|error| format!("{} 읽기 실패: {error}", path.display()))?;
        sources.push((path.display().to_string(), raw));
    }
    audit_vdd_data(
        sources
            .iter()
            .map(|(source_id, raw)| (source_id.as_str(), raw.as_slice())),
        require_original_bytes,
    )
}

pub(super) fn audit_btx_data<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Result<(), String> {
    let mut archives = Vec::new();
    for (source_id, raw) in sources {
        let archive = VddArchive::parse(raw)
            .map_err(|error| format!("{source_id} VDD 파싱 실패: {error}"))?;
        archives.push((source_id.to_string(), archive));
    }
    let summary = audit_archives(
        archives
            .iter()
            .map(|(archive_id, archive)| (archive_id.as_str(), archive)),
    )
    .map_err(|error| error.to_string())?;
    verify_box_inventory(summary).map_err(|error| error.to_string())?;

    println!(
        "PASS BTX inventory: GRP {}, entries {}, BTX {} \
         (4bpp {}, 8bpp {}), unique {}, other {}",
        summary.archives,
        summary.entries,
        summary.physical_btx,
        summary.indexed4,
        summary.indexed8,
        summary.unique_content,
        summary.other_entries
    );
    Ok(())
}

pub(super) fn audit_btx_inventory(paths: &[PathBuf]) -> Result<(), String> {
    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let raw =
            fs::read(path).map_err(|error| format!("{} 읽기 실패: {error}", path.display()))?;
        sources.push((path.display().to_string(), raw));
    }
    if sources.is_empty() {
        return Err("검증할 GRP VDD 경로가 없다".to_string());
    }
    audit_btx_data(
        sources
            .iter()
            .map(|(source_id, raw)| (source_id.as_str(), raw.as_slice())),
    )
}

pub(super) fn verify_runtime_btx_layout(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let values = args.collect::<Vec<_>>();
    let [source_path, archive_path, asset_name, gpu_path] = values.as_slice() else {
        return Err(usage().to_string());
    };
    let source_path = PathBuf::from(source_path);
    let archive_path = archive_path
        .to_str()
        .ok_or_else(|| "GRP ISO 경로가 UTF-8이 아님".to_string())?;
    let asset_name = asset_name
        .to_str()
        .ok_or_else(|| "BTX 이름이 UTF-8이 아님".to_string())?;
    let gpu_path = PathBuf::from(gpu_path);
    if !asset_name.to_ascii_uppercase().ends_with(".BTX") {
        return Err(format!("BTX 이름이 .BTX로 끝나지 않음: {asset_name}"));
    }

    let source = VerifiedSourceImage::open(&source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let archive_raw = iso
        .read_file(archive_path)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&archive_raw)
        .map_err(|error| format!("{archive_path} VDD 파싱 실패: {error}"))?;
    let entry = archive
        .find(asset_name)
        .ok_or_else(|| format!("{archive_path}에 {asset_name}이 없음"))?;
    let compressed = archive.read_file(entry);
    let header = parse_header(compressed)
        .map_err(|error| format!("{archive_path}:{asset_name} CNX 헤더 실패: {error}"))?;
    let format = PixelFormat::from_cnx_subtype(header.subtype)
        .map_err(|error| format!("{archive_path}:{asset_name}: {error}"))?;
    let plain = decompress(compressed)
        .map_err(|error| format!("{archive_path}:{asset_name} CNX 해제 실패: {error}"))?;
    let gpu = fs::read(&gpu_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", gpu_path.display()))?;
    let (layout, placements) = infer_unique_vram_layout(&plain, format, &gpu)
        .map_err(|error| format!("{archive_path}:{asset_name}: {error}"))?;

    let format_name = match format {
        PixelFormat::Indexed4 => "indexed4",
        PixelFormat::Indexed8 => "indexed8",
    };
    let coordinates = placements
        .iter()
        .map(|placement| format!("({}, {})", placement.x_words, placement.y))
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "PASS BTX runtime layout: {archive_path}:{asset_name} {format_name} {}x{}, \
         exact placements {} [{}], asset SHA-256 {:x}, GPU SHA-256 {:x}",
        layout.width,
        layout.height,
        placements.len(),
        coordinates,
        Sha256::digest(&plain),
        Sha256::digest(&gpu),
    );
    Ok(())
}
