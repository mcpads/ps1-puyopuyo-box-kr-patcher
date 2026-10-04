//! 특기 literal·문자표를 원본 검증 뒤 EXE data Expected Write에 기여한다.
//! 코드와 효과 데이터는 쓰지 않으며 주소는 각 EXE의 독립 공급 조사에 따른다.
use super::subgame_help_development::DataSpan;
use crate::coords::exe_map;
use crate::graphics::special_skill_text::{source_bytes, SkillText, TABLE_BYTES};
use sha2::{Digest, Sha256};
pub(super) struct SkillOwner {
    pub exe: &'static str,
    pub table: u32,
    pub control: u32,
    pub guards: [(u32, usize, &'static str); 4],
    pub literals: [(u32, u32); 27],
}
pub(super) fn prepare(exe: &str, text: &[u8]) -> Result<Vec<DataSpan>, String> {
    let owner = super::special_skill_sources::OWNERS
        .iter()
        .find(|owner| owner.exe == exe)
        .ok_or("특기 EXE 공급 미확인")?;
    let map = exe_map(owner.exe).ok_or("특기 EXE 사상 누락")?;
    let slice = |runtime, len| -> Result<&[u8], String> {
        let at = map.text_offset(runtime).ok_or("특기 주소 이탈")?;
        text.get(at..at + len)
            .ok_or_else(|| "특기 범위 이탈".into())
    };
    for (at, len, expected) in owner.guards {
        if format!("{:x}", Sha256::digest(slice(at, len)?)) != expected {
            return Err("특기 원본 공급/문자표 SHA-256 불일치".into());
        }
    }
    if slice(owner.control, 8)? != b"nn\0\0\x81\x40\0\0" {
        return Err("특기 개행/공백 control 불일치".into());
    }
    let skills = SkillText::load()?;
    let table = slice(owner.table, TABLE_BYTES)?;
    let mut spans = vec![DataSpan {
        runtime: owner.table,
        original: table.to_vec(),
        replacement: skills.table(table)?,
    }];
    for (message, &(name, description)) in skills.messages.iter().zip(&owner.literals) {
        for (runtime, source, ko) in [
            (name, &message.source_name, &message.name),
            (
                description,
                &message.source_description,
                &message.description,
            ),
        ] {
            let original = slice(runtime, source_bytes(source)?.len() + 1)?;
            spans.push(DataSpan {
                runtime,
                original: original.to_vec(),
                replacement: skills.literal(original, source, ko)?,
            });
        }
    }
    spans.sort_by_key(|s| s.runtime);
    if spans
        .windows(2)
        .any(|p| u64::from(p[0].runtime) + p[0].original.len() as u64 > u64::from(p[1].runtime))
    {
        return Err("특기 literal/문자표 범위 겹침".into());
    }
    Ok(spans)
}
