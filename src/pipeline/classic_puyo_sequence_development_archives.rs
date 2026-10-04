//! classic PUYO의 현재 SEQ 엔트리를 VDD별 한 replacement로 합성한다.

use std::error::Error;
use std::fmt;

use super::classic_code_glyph_development_entries::PreparedClassicCodeGlyphDevelopmentEntries;
use super::cnx_development_archive_replacement::{
    prepare_cnx_development_archive_replacement,
    verify_cnx_development_archive_replacement_readback, CnxDevelopmentDiscContribution,
    PreparedCnxDevelopmentArchiveReplacement,
};

const ACTOR: &str = "pipeline::classic_puyo_sequence_development_archives";

#[derive(Debug, Clone, Copy)]
struct ArchiveSpec {
    iso_path: &'static str,
    patch_id: &'static str,
}

const ARCHIVES: [ArchiveSpec; 2] = [
    ArchiveSpec {
        iso_path: "/PUYO1/PP1_SEQ.VDD;1",
        patch_id: "classic-puyo-development-puyo1-seq",
    },
    ArchiveSpec {
        iso_path: "/PUYO2/PP2_SEQ.VDD;1",
        patch_id: "classic-puyo-development-puyo2-seq",
    },
];

/// 두 classic SEQ VDD의 배타적 disc 기여.
pub(super) struct ClassicPuyoSequenceDevelopmentArchivesBuild {
    archives: Vec<PreparedCnxDevelopmentArchiveReplacement>,
}

impl ClassicPuyoSequenceDevelopmentArchivesBuild {
    pub fn disc_contributions(&self) -> Vec<CnxDevelopmentDiscContribution<'_>> {
        self.archives
            .iter()
            .map(PreparedCnxDevelopmentArchiveReplacement::disc_contribution)
            .collect()
    }
}

/// SEQ 엔트리 합성·저장·최종 readback 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ClassicPuyoSequenceDevelopmentArchivesError(String);

impl fmt::Display for ClassicPuyoSequenceDevelopmentArchivesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ClassicPuyoSequenceDevelopmentArchivesError {}

fn error(message: impl Into<String>) -> ClassicPuyoSequenceDevelopmentArchivesError {
    ClassicPuyoSequenceDevelopmentArchivesError(message.into())
}

/// code glyph SEQ를 각 불변 원본 VDD에서 한 번씩 합성한다.
pub(super) fn prepare_classic_puyo_sequence_development_archives(
    code_glyph: &PreparedClassicCodeGlyphDevelopmentEntries,
) -> Result<ClassicPuyoSequenceDevelopmentArchivesBuild, ClassicPuyoSequenceDevelopmentArchivesError>
{
    let mut archives = Vec::new();
    for archive in ARCHIVES {
        let entries = code_glyph
            .entry_refs(archive.iso_path)
            .map_err(|source| error(source.to_string()))?;
        archives.push(
            prepare_cnx_development_archive_replacement(
                archive.iso_path,
                archive.patch_id,
                ACTOR,
                &entries,
            )
            .map_err(|source| error(source.to_string()))?,
        );
    }
    Ok(ClassicPuyoSequenceDevelopmentArchivesBuild { archives })
}

/// 누적 disc에서 두 SEQ VDD 전체를 다시 읽고 code glyph 소비를 결속한다.
pub(super) fn verify_classic_puyo_sequence_development_archives_readback(
    image: &[u8],
    build: &ClassicPuyoSequenceDevelopmentArchivesBuild,
    code_glyph: &mut PreparedClassicCodeGlyphDevelopmentEntries,
) -> Result<(), ClassicPuyoSequenceDevelopmentArchivesError> {
    if build.archives.len() != ARCHIVES.len() {
        return Err(error("classic PUYO SEQ archive 분모 drift"));
    }
    for (archive, prepared) in ARCHIVES.iter().zip(&build.archives) {
        verify_cnx_development_archive_replacement_readback(image, prepared)
            .map_err(|source| error(source.to_string()))?;
        code_glyph
            .record_verified_archive_readback(archive.iso_path)
            .map_err(|source| error(source.to_string()))?;
    }
    Ok(())
}
