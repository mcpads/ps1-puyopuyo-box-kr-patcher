//! PUYO1 번역 scene의 열 간격만 16px에서 선택한 Galmuri의 12px로 바꾼다.
//! 같은 길이의 basic block 안에서 상수 복사를 줄여 추가 연산을 배치한다.
use crate::coords::exe_map;
use crate::disc::{DiscImage, Iso9660, VerifiedSourceImage};
use crate::isa::{verify_typed_source, InstructionKind as K, IsaProfile, Reg, VerifiedMachineCode};
use crate::patch::{plan_logical_machine_code, ImageRegion, RegionKind, WritePlan};
const PATH: &str = "/PUYO1/PUYO1.EXE;1";
const START: u32 = 0x80017800;
const ACTOR: &str = "pipeline::classic_scene_spacing";

fn original_code() -> Result<VerifiedMachineCode, String> {
    verify_typed_source(
        IsaProfile::DECLARED,
        START,
        "puyo1-scene-source",
        &[
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::ZERO,
                imm: 1,
            },
            K::Addu {
                rd: Reg::V1,
                rs: Reg::A0,
                rt: Reg::ZERO,
            },
            K::Addiu {
                rt: Reg::A3,
                rs: Reg::ZERO,
                imm: 4,
            },
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x70,
            },
            K::Lw {
                rt: Reg::V1,
                base: Reg::GP,
                offset: 0x410,
            },
            K::Lw {
                rt: Reg::A1,
                base: Reg::S0,
                offset: 0x50,
            },
            K::Lw {
                rt: Reg::A2,
                base: Reg::GP,
                offset: 0x414,
            },
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 3,
            },
            K::Sll {
                rd: Reg::A1,
                rt: Reg::A1,
                sa: 4,
            },
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::A1,
            },
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 12,
            },
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x50,
            },
        ],
    )
    .map_err(|e| e.to_string())
}
fn replacement_code() -> Result<VerifiedMachineCode, String> {
    verify_typed_source(
        IsaProfile::DECLARED,
        START,
        "puyo1-scene-12px-columns",
        &[
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::ZERO,
                imm: 1,
            },
            K::Addiu {
                rt: Reg::A3,
                rs: Reg::ZERO,
                imm: 4,
            },
            K::Sh {
                rt: Reg::A0,
                base: Reg::V0,
                offset: 0x70,
            },
            K::Lw {
                rt: Reg::V1,
                base: Reg::GP,
                offset: 0x410,
            },
            K::Lw {
                rt: Reg::A1,
                base: Reg::S0,
                offset: 0x50,
            },
            K::Lw {
                rt: Reg::A2,
                base: Reg::GP,
                offset: 0x414,
            },
            // ((header_x * 2) + column + column * 2) << 14 = (header_x * 8 + column * 12) << 12.
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 1,
            },
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::A1,
            },
            K::Sll {
                rd: Reg::A1,
                rt: Reg::A1,
                sa: 1,
            },
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::A1,
            },
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 14,
            },
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x50,
            },
        ],
    )
    .map_err(|e| e.to_string())
}
pub(super) struct PreparedClassicSceneSpacing {
    original: Vec<u8>,
    code: VerifiedMachineCode,
}
pub(super) fn prepare(source: &VerifiedSourceImage) -> Result<PreparedClassicSceneSpacing, String> {
    let iso = Iso9660::parse(source.image()).map_err(|e| e.to_string())?;
    let original = iso.read_file(PATH).map_err(|e| e.to_string())?;
    let map = exe_map("PUYO1.EXE").ok_or("PUYO1 사상 누락")?;
    let at = map.file_offset(START).ok_or("PUYO1 간격 주소 범위 이탈")? as usize;
    let expected = original_code()?;
    let code = replacement_code()?;
    if original.get(at..at + expected.bytes().len()) != Some(expected.bytes())
        || expected.bytes().len() != code.bytes().len()
    {
        return Err("PUYO1 간격 원본 명령 불일치".into());
    }
    // 열 register는 직후 row load로 덮어쓰므로 변경한 중간값은 외부로 나가지 않는다.
    let tail = verify_typed_source(
        IsaProfile::DECLARED,
        START + 48,
        "puyo1-scene-row-load",
        &[K::Lw {
            rt: Reg::A1,
            base: Reg::S0,
            offset: 0x54,
        }],
    )
    .map_err(|e| e.to_string())?;
    if original.get(at + 48..at + 52) != Some(tail.bytes()) {
        return Err("PUYO1 열 register 수명 변경".into());
    }
    Ok(PreparedClassicSceneSpacing { original, code })
}
impl PreparedClassicSceneSpacing {
    pub(super) fn add_writes(
        &self,
        iso: &Iso9660<'_>,
        plan: &mut WritePlan,
    ) -> Result<usize, String> {
        let map = exe_map("PUYO1.EXE").ok_or("PUYO1 사상 누락")?;
        let file = super::vdd_plan::logical_file(iso.find(PATH).map_err(|e| e.to_string())?)?;
        if file != map.file || iso.read_file(PATH).map_err(|e| e.to_string())? != self.original {
            return Err("PUYO1 간격 EXE·ISO 원본 불일치".into());
        }
        let at = map.file_offset(START).ok_or("PUYO1 file 주소 누락")?;
        let writes = plan_logical_machine_code(
            file,
            at,
            original_code()?.bytes(),
            &self.code,
            "puyo1-scene-spacing",
            ACTOR,
            "PUYO1 번역 scene의 열 간격 12px",
        )
        .ok_or("PUYO1 typed 쓰기 실패")?;
        let count = writes.len();
        for w in writes {
            plan.regions.push(ImageRegion::new(
                format!("region:{}", w.id),
                w.range(),
                RegionKind::MachineCode,
                "PUYO1 scene 열 좌표 basic block",
            ));
            plan.writes.push(w);
        }
        Ok(count)
    }
    pub(super) fn verify_readback(&self, image: &[u8]) -> Result<(), String> {
        let disc = DiscImage::from_bytes(image.to_vec()).map_err(|e| e.to_string())?;
        let iso = Iso9660::parse(&disc).map_err(|e| e.to_string())?;
        let at = exe_map("PUYO1.EXE")
            .and_then(|m| m.file_offset(START))
            .ok_or("PUYO1 좌표 누락")? as usize;
        let mut expected = self.original.clone();
        expected[at..at + self.code.bytes().len()].copy_from_slice(self.code.bytes());
        if iso.read_file(PATH).map_err(|e| e.to_string())? != expected {
            return Err("PUYO1 간격 이외 EXE 변경 또는 readback 불일치".into());
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "classic_scene_spacing_tests.rs"]
mod tests;
