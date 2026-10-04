//! 다섯 EXE의 BOXOPT 설명과 특기 data span을 최종 Expected Write에 기여한다.
//! 문자열과 lookup table 밖의 EXE bytes는 최종 readback에서 모두 보존한다.
use crate::coords::exe_map;
use crate::disc::{DiscImage, Iso9660, PsxExe, VerifiedSourceImage};
use crate::graphics::subgame_help::{HelpText, MESSAGE_BYTES, MESSAGE_COUNT, TABLE_BYTES};
use crate::patch::{WriteIntent, WritePlan};
use sha2::{Digest, Sha256};

struct Owner {
    exe: &'static str,
    path: &'static str,
    strings: u32,
    table: u32,
    consumer: u32,
    strings_sha: &'static str,
    table_sha: &'static str,
    consumer_sha: &'static str,
}
const OWNERS: [Owner; 5] = [
    Owner {
        exe: "GOCHA1.EXE",
        path: "/GOCHA1/GOCHA1.EXE;1",
        strings: 0x800936a0,
        table: 0x80010700,
        consumer: 0x80022918,
        strings_sha: "04993efe8fbbe4b28a508cd3614076fd252d40ee3fdd3d9ab84ad024c3c8d333",
        table_sha: "0eac4346b674a6977e9f1cd1b7cabfcc9cef0f7d78ba2d44e16126cbb6eb91e6",
        consumer_sha: "c4e8a93f038879d0006e6bfbe45e99dab1f47975362bda3ad6a5b994b502d519",
    },
    Owner {
        exe: "GOCHA2.EXE",
        path: "/GOCHA2/GOCHA2.EXE;1",
        strings: 0x80078cec,
        table: 0x80010000,
        consumer: 0x80014edc,
        strings_sha: "21c2fa5bf9997facabb6466f28f29a64d82a2d60332ecc7e6a796a42a0bfaea7",
        table_sha: "75c79a0989dd4db78dc2724f8a92266d6c8381af6ec00060842174bdfaaf0043",
        consumer_sha: "71a44f1ba902f37a2f10b98c7a7c71d14b7a854a873481d67a8c3d77a749b5b4",
    },
    Owner {
        exe: "HORI2.EXE",
        path: "/HORI2/HORI2.EXE;1",
        strings: 0x80078b34,
        table: 0x80010000,
        consumer: 0x80014ef8,
        strings_sha: "04993efe8fbbe4b28a508cd3614076fd252d40ee3fdd3d9ab84ad024c3c8d333",
        table_sha: "75c79a0989dd4db78dc2724f8a92266d6c8381af6ec00060842174bdfaaf0043",
        consumer_sha: "ca19013c24e326ffd18799c04e290847a96a4493bf45b3cb6a3abd77710d2297",
    },
    Owner {
        exe: "HORI4.EXE",
        path: "/HORI4/HORI4.EXE;1",
        strings: 0x80078de8,
        table: 0x80010000,
        consumer: 0x80014ee0,
        strings_sha: "04993efe8fbbe4b28a508cd3614076fd252d40ee3fdd3d9ab84ad024c3c8d333",
        table_sha: "75c79a0989dd4db78dc2724f8a92266d6c8381af6ec00060842174bdfaaf0043",
        consumer_sha: "14cea2f56bd29ba3a8b8f6ee8bc7bfd7ed31f0c4b2b6ca69667b0af481ae2225",
    },
    Owner {
        exe: "MINPUYO.EXE",
        path: "/MINPUYO/MINPUYO.EXE;1",
        strings: 0x8007931c,
        table: 0x80010000,
        consumer: 0x80014ec4,
        strings_sha: "21c2fa5bf9997facabb6466f28f29a64d82a2d60332ecc7e6a796a42a0bfaea7",
        table_sha: "75c79a0989dd4db78dc2724f8a92266d6c8381af6ec00060842174bdfaaf0043",
        consumer_sha: "e7ebcbbbd18beed446432ba6e5286cf04bab9c86352ea0daba4dcf4ea61c2f8b",
    },
];
pub(super) struct DataSpan {
    pub(super) runtime: u32,
    pub(super) original: Vec<u8>,
    pub(super) replacement: Vec<u8>,
}
struct Executable {
    owner: &'static Owner,
    original: Vec<u8>,
    spans: Vec<DataSpan>,
}
pub(super) struct PreparedSubgameHelp {
    executables: Vec<Executable>,
}

fn verify_hash(bytes: &[u8], expected: &str) -> Result<(), String> {
    if format!("{:x}", Sha256::digest(bytes)) != expected {
        return Err("서브게임 설명 원본 SHA-256 불일치".into());
    }
    Ok(())
}
pub(super) fn prepare_subgame_help(
    source: &VerifiedSourceImage,
) -> Result<PreparedSubgameHelp, String> {
    let iso = Iso9660::parse(source.image()).map_err(|e| e.to_string())?;
    let text = HelpText::load()?;
    let mut executables = Vec::new();
    for owner in &OWNERS {
        let map = exe_map(owner.exe).ok_or("서브게임 EXE 사상 누락")?;
        let raw = iso.read_file(owner.path).map_err(|e| e.to_string())?;
        let exe = PsxExe::parse(&raw).map_err(|e| e.to_string())?;
        exe.verify_map(map).map_err(|e| e.to_string())?;
        let slice = |runtime, len| -> Result<&[u8], String> {
            let at = map.text_offset(runtime).ok_or("설명 주소 범위 이탈")?;
            exe.text()
                .get(at..at + len)
                .ok_or_else(|| "설명 데이터 범위 이탈".into())
        };
        // lookup 전체와 e/r 처리·MoveImage callback을 EXE별 원본에 결속한다.
        verify_hash(slice(owner.consumer, 0x29c)?, owner.consumer_sha)?;
        let original_table = slice(owner.table, TABLE_BYTES)?;
        let original_strings = slice(owner.strings, MESSAGE_COUNT * MESSAGE_BYTES)?;
        verify_hash(original_table, owner.table_sha)?;
        verify_hash(original_strings, owner.strings_sha)?;
        let mut spans = vec![
            DataSpan {
                runtime: owner.table,
                original: original_table.to_vec(),
                replacement: text.table(original_table)?,
            },
            DataSpan {
                runtime: owner.strings,
                original: original_strings.to_vec(),
                replacement: text.strings(original_strings)?,
            },
        ];
        spans.extend(super::special_skills_development::prepare(
            owner.exe,
            exe.text(),
        )?);
        executables.push(Executable {
            owner,
            original: raw,
            spans,
        });
    }
    Ok(PreparedSubgameHelp { executables })
}
impl PreparedSubgameHelp {
    pub(super) fn add_writes(&self, iso: &Iso9660<'_>, plan: &mut WritePlan) -> Result<(), String> {
        for exe in &self.executables {
            let map = exe_map(exe.owner.exe).ok_or("서브게임 EXE 사상 누락")?;
            if iso.read_file(exe.owner.path).map_err(|e| e.to_string())? != exe.original {
                return Err("서브게임 설명 원본 EXE 불일치".into());
            }
            let file = super::vdd_plan::logical_file(
                iso.find(exe.owner.path).map_err(|e| e.to_string())?,
            )?;
            if file != map.file {
                return Err("서브게임 설명 ISO 사상 불일치".into());
            }
            for (i, span) in exe.spans.iter().enumerate() {
                super::vdd_plan::add_logical_span(
                    plan,
                    file,
                    map.file_offset(span.runtime).ok_or("설명 file 좌표 없음")? as usize,
                    &span.original,
                    &span.replacement,
                    &format!("subgame-help-{}-{i}", exe.owner.exe),
                    "pipeline::subgame_help_development",
                    "검증된 설명 문자열과 전용 lookup table data 교체",
                    WriteIntent::Data,
                )?;
            }
        }
        Ok(())
    }
    pub(super) fn verify_readback(&self, image: &[u8]) -> Result<(), String> {
        let disc = DiscImage::from_bytes(image.to_vec()).map_err(|e| e.to_string())?;
        let iso = Iso9660::parse(&disc).map_err(|e| e.to_string())?;
        for exe in &self.executables {
            let map = exe_map(exe.owner.exe).ok_or("서브게임 EXE 사상 누락")?;
            let mut expected = exe.original.clone();
            for span in &exe.spans {
                let at = map.file_offset(span.runtime).ok_or("설명 file 좌표 없음")? as usize;
                expected[at..at + span.replacement.len()].copy_from_slice(&span.replacement);
            }
            if iso.read_file(exe.owner.path).map_err(|e| e.to_string())? != expected {
                return Err(format!(
                    "{} 설명 삽입 또는 비대상 EXE 보존 실패",
                    exe.owner.exe
                ));
            }
        }
        Ok(())
    }
}
