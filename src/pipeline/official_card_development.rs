//! 공식전 카드 SEQ와 글꼴을 같은 원본에서 함께 준비한다.
use super::cnx_development_archive_replacement::{
    prepare_cnx_development_archive_replacement, PreparedCnxDevelopmentArchiveReplacement,
};
use super::cnx_development_entry::{
    prepare_cnx_development_entry, CnxDevelopmentEntrySpec, PreparedCnxDevelopmentEntry,
};
use crate::archive::VddArchive;
use crate::compress::decompress;
use crate::disc::{Iso9660, VerifiedSourceImage};
use crate::graphics::official_card_glyphs::render_official_card_glyphs;
use crate::script::official_card_text::{
    compile_official_card_streams, official_card_glyph_mapping,
};

pub(super) struct OfficialCardDevelopment {
    pub atlas: PreparedCnxDevelopmentEntry,
    pub sequence: PreparedCnxDevelopmentArchiveReplacement,
}
fn read(source: &VerifiedSourceImage, path: &str, name: &str) -> Result<Vec<u8>, String> {
    let iso = Iso9660::parse(source.image()).map_err(|e| e.to_string())?;
    let bytes = iso.read_file(path).map_err(|e| e.to_string())?;
    let archive = VddArchive::parse(&bytes).map_err(|e| e.to_string())?;
    let entry = archive.find(name).ok_or("카드 참조 엔트리 누락")?;
    decompress(archive.read_file(entry)).map_err(|e| e.to_string())
}
pub(super) fn prepare(source: &VerifiedSourceImage) -> Result<OfficialCardDevelopment, String> {
    let reference_sequence = read(source, "/PCOMI/PC1_SEQ.VDD;1", "MCRD1.SEQ")?;
    let reference_atlas = read(source, "/PCOMI/PC1_GRP.VDD;1", "MCRD01.BTX")?;
    let mapping = official_card_glyph_mapping()?;
    let spec = CnxDevelopmentEntrySpec {
        iso_path: "/KOUSIKI/KOU_SEQ.VDD;1",
        entry_name: "MCRD1OF.SEQ",
        expected_cnx_subtype: *b"bin",
        patch_id: "official-card-sequence",
        actor: "pipeline::official_card_development",
    };
    let (sequence, ()) = prepare_cnx_development_entry(source, spec, |original| {
        let streams = compile_official_card_streams(
            original,
            &mapping,
            &reference_sequence,
            &reference_atlas,
        )?;
        // 알려진 직접 주소가 조판으로 이동하는 본문 중간을 가리키면 실패한다.
        // 계산된 진입 전체의 증명은 아니며 개발 후보 실행에서 별도로 확인한다.
        for offset in (0..original.len() - 3).step_by(2) {
            let value =
                u32::from_le_bytes(original[offset..offset + 4].try_into().expect("4바이트"));
            if let Some(relative) = value.checked_sub(0x801b0270) {
                if streams.iter().any(|s| {
                    relative as usize > s.offset && (relative as usize) < s.offset + s.bytes.len()
                }) {
                    return Err("카드 본문 내부 직접 주소 후보 발견".into());
                }
            }
        }
        let mut result = original.to_vec();
        for stream in streams {
            result[stream.offset..stream.offset + stream.bytes.len()]
                .copy_from_slice(&stream.bytes);
        }
        Ok((result, ()))
    })
    .map_err(|e| e.to_string())?;
    let (atlas, ()) = prepare_cnx_development_entry(
        source,
        CnxDevelopmentEntrySpec {
            iso_path: "/KOUSIKI/KOU_GRP.VDD;1",
            entry_name: "MCRD01.BTX",
            expected_cnx_subtype: *b"bi_",
            patch_id: "official-card-glyphs",
            ..spec
        },
        |original| {
            let mut glyphs = render_official_card_glyphs(original, &mapping)?;
            let (badges, _) =
                crate::graphics::mcrd01_development_surface::patch_mcrd01_development_surface(
                    original,
                )
                .map_err(|e| e.to_string())?;
            // 같은 불변 원본에서 별개 영역을 만든다. 적용 순서로 충돌을 숨기지 않는다.
            for ((target, &badge), &before) in glyphs.iter_mut().zip(&badges).zip(original) {
                if badge != before {
                    if *target != before {
                        return Err("카드 문자·배지 쓰기 겹침".into());
                    }
                    *target = badge;
                }
            }
            Ok((glyphs, ()))
        },
    )
    .map_err(|e| e.to_string())?;
    let sequence = prepare_cnx_development_archive_replacement(
        spec.iso_path,
        spec.patch_id,
        spec.actor,
        &[&sequence],
    )
    .map_err(|e| e.to_string())?;
    Ok(OfficialCardDevelopment { atlas, sequence })
}
