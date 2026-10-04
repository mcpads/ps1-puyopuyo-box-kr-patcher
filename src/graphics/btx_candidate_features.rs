//! 미분류 BTX 후보를 비교하기 위한 픽셀·이름 특징 계산.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use super::{
    error, BtxCandidateConfidence, BtxFamilyCandidate, BtxFamilyCandidateError, BtxOccupiedBox,
    BtxReviewLane, PixelFormat,
};

pub(super) fn decode_pixels(raw: &[u8], format: PixelFormat) -> Vec<u8> {
    match format {
        PixelFormat::Indexed4 => raw
            .iter()
            .flat_map(|byte| [byte & 0x0F, byte >> 4])
            .collect(),
        PixelFormat::Indexed8 => raw.to_vec(),
    }
}

pub(super) fn review_layout(
    pixel_count: usize,
) -> Result<(usize, usize, usize), BtxFamilyCandidateError> {
    let maximum_width = 1024.min(pixel_count / 8);
    let mut candidates = (8..=maximum_width)
        .step_by(8)
        .filter_map(|width| {
            if !pixel_count.is_multiple_of(width) {
                return None;
            }
            let height = pixel_count / width;
            (8..=1024).contains(&height).then_some((width, height))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(error(format!(
            "8px 정렬 review layout 후보가 없음: pixels={pixel_count}"
        )));
    }
    candidates.sort_by_key(|(width, _)| (width.abs_diff(256), *width));
    let (width, height) = candidates[0];
    Ok((width, height, candidates.len()))
}

pub(super) fn palette_histogram(pixels: &[u8], format: PixelFormat) -> Vec<usize> {
    let mut histogram = vec![0usize; 1usize << format.bits_per_pixel()];
    for &pixel in pixels {
        histogram[usize::from(pixel)] += 1;
    }
    histogram
}

pub(super) fn occupied_features(
    pixels: &[u8],
    width: usize,
    height: usize,
    background: u8,
) -> (Option<BtxOccupiedBox>, usize, usize) {
    let mut columns = vec![false; width];
    let mut rows = vec![false; height];
    let mut minimum_x = width;
    let mut minimum_y = height;
    let mut maximum_x = 0usize;
    let mut maximum_y = 0usize;
    let mut found = false;
    for (offset, &pixel) in pixels.iter().enumerate() {
        if pixel == background {
            continue;
        }
        let x = offset % width;
        let y = offset / width;
        columns[x] = true;
        rows[y] = true;
        minimum_x = minimum_x.min(x);
        minimum_y = minimum_y.min(y);
        maximum_x = maximum_x.max(x);
        maximum_y = maximum_y.max(y);
        found = true;
    }
    let bands = |occupied: &[bool]| {
        occupied
            .iter()
            .enumerate()
            .filter(|(index, value)| **value && (*index == 0 || !occupied[*index - 1]))
            .count()
    };
    let bbox = found.then_some(BtxOccupiedBox {
        x: minimum_x,
        y: minimum_y,
        width: maximum_x - minimum_x + 1,
        height: maximum_y - minimum_y + 1,
    });
    (bbox, bands(&rows), bands(&columns))
}

pub(super) fn cell_features(
    pixels: &[u8],
    width: usize,
    height: usize,
    background: u8,
) -> (usize, usize, usize, usize, bool) {
    let columns = width / 16;
    let rows = height / 16;
    let mut cell_digests = BTreeSet::new();
    let mut background_cells = 0usize;
    for cell_y in 0..rows {
        for cell_x in 0..columns {
            let mut digest = Sha256::new();
            let mut is_background = true;
            for y in 0..16 {
                let start = (cell_y * 16 + y) * width + cell_x * 16;
                let row = &pixels[start..start + 16];
                is_background &= row.iter().all(|pixel| *pixel == background);
                digest.update(row);
            }
            background_cells += usize::from(is_background);
            cell_digests.insert(<[u8; 32]>::from(digest.finalize()));
        }
    }
    let total = rows * columns;
    let unique = cell_digests.len();
    (
        total,
        unique,
        total.saturating_sub(unique),
        background_cells,
        !width.is_multiple_of(16) || !height.is_multiple_of(16),
    )
}

pub(super) fn owner_scope(archive: &str) -> &'static str {
    if archive.starts_with("/FIL_") {
        "shell"
    } else if archive.starts_with("/PUYO1/") || archive.starts_with("/PUYO2/") {
        "classic-puyo"
    } else if archive.starts_with("/QUEST/") {
        "quest"
    } else if archive.starts_with("/PCOMI/") {
        "pcomi"
    } else {
        "subgame"
    }
}

pub(super) fn normalized_name_stem(name: &str) -> String {
    let mut stem = name
        .strip_suffix(".BTX")
        .unwrap_or(name)
        .to_ascii_uppercase();
    let bytes = stem.as_bytes();
    if bytes.len() >= 2
        && matches!(bytes[bytes.len() - 1], b'A' | b'B')
        && bytes[bytes.len() - 2].is_ascii_digit()
    {
        stem.pop();
    }
    while stem.ends_with(|character: char| character.is_ascii_digit()) {
        stem.pop();
    }
    while stem.ends_with('_') {
        stem.pop();
    }
    if stem.is_empty() {
        name.to_ascii_uppercase()
    } else {
        stem
    }
}

pub(super) fn candidate_lane(
    stem: &str,
    format: PixelFormat,
    bytes: usize,
    review_width: usize,
    review_height: usize,
) -> (BtxReviewLane, BtxCandidateConfidence, &'static str) {
    const UI_TOKENS: [&str; 15] = [
        "FONT", "FNT", "MESS", "MES", "MENU", "INFO", "RULE", "OPT", "RANK", "TITLE", "TEXT",
        "NAME", "OVER", "KANBAN", "SYS",
    ];
    const ART_TOKENS: [&str; 9] = [
        "MAP", "FLAME", "FACE", "BACK", "EFECT", "EFFECT", "PUYO", "OBJ", "CEL",
    ];
    if UI_TOKENS.iter().any(|token| stem.contains(token)) || stem.starts_with("ED_") {
        return (
            BtxReviewLane::UiFontTextCandidate,
            BtxCandidateConfidence::High,
            "ui-font-text name token",
        );
    }
    if ART_TOKENS.iter().any(|token| stem.contains(token)) || stem == "GT" {
        return (
            BtxReviewLane::CharacterBackgroundEffectCandidate,
            BtxCandidateConfidence::High,
            "character-background-effect name token",
        );
    }
    if format == PixelFormat::Indexed4 && bytes == 16_384 && review_width == 256 {
        return (
            BtxReviewLane::CharacterBackgroundEffectCandidate,
            BtxCandidateConfidence::Medium,
            "common 256x128 4bpp sprite-atlas review geometry",
        );
    }
    if review_height <= 32 && review_width >= 96 {
        return (
            BtxReviewLane::UiFontTextCandidate,
            BtxCandidateConfidence::Low,
            "wide shallow review geometry",
        );
    }
    (
        BtxReviewLane::Ambiguous,
        BtxCandidateConfidence::Low,
        "no strong source-derived lane feature",
    )
}

pub(super) fn family_lane(
    members: &[&BtxFamilyCandidate],
) -> (BtxReviewLane, BtxCandidateConfidence) {
    let lanes = members
        .iter()
        .map(|member| member.review_lane)
        .collect::<BTreeSet<_>>();
    if lanes.len() != 1 {
        return (BtxReviewLane::Ambiguous, BtxCandidateConfidence::Low);
    }
    let lane = *lanes.iter().next().expect("한 개 이상의 family member");
    let confidence = members
        .iter()
        .map(|member| member.confidence)
        .min()
        .expect("한 개 이상의 family member");
    (lane, confidence)
}
