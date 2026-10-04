use super::*;
use crate::isa::{verify_typed_source, IsaProfile};

// 실제 조립·재해석한 decoder를 제한된 메모리에서 실행한다. 분기 delay와
// load delay를 적용하고, 입력/dictionary 밖 읽기와 scratch 밖 쓰기를 거부한다.
fn decode_record(record: &[u8], dictionary_rows: u16) -> (u32, Vec<u8>) {
    const BASE: u32 = 0x8010_0000;
    const INPUT: usize = 0x1001; // cell payload는 홀수 주소에서도 시작한다.
    const DICTIONARY: usize = 0x2000;
    const OUTPUT: usize = 0xE100;
    let mut kinds = Vec::new();
    append_row_dictionary_decoder(&mut kinds, dictionary_rows as i16, BASE).unwrap();
    let verified = verify_typed_source(
        IsaProfile::DECLARED,
        BASE,
        "test::glyph_row_decoder",
        &kinds,
    )
    .unwrap();
    let code = verified.instructions();
    let mut memory = vec![0xA5; OUTPUT + 100];
    memory[INPUT..INPUT + record.len()].copy_from_slice(record);
    for index in 0..usize::from(dictionary_rows) {
        memory[DICTIONARY + index * 6..DICTIONARY + index * 6 + 6]
            .copy_from_slice(&row(index as u16));
    }
    let mut registers = [0u32; 32];
    for (reg, value) in registers.iter_mut().enumerate().take(24).skip(16) {
        *value = 0xCAFE_0000 + reg as u32;
    }
    let saved = registers[16..24].to_vec();
    registers[Reg::A0.0 as usize] = INPUT as u32;
    registers[Reg::A1.0 as usize] = DICTIONARY as u32;
    registers[Reg::A2.0 as usize] = OUTPUT as u32;
    registers[Reg::A3.0 as usize] = (INPUT + record.len()) as u32;
    registers[Reg::RA.0 as usize] = BASE + code.len() as u32 * 4;
    let mut pc = 0usize;
    let mut jump = None;
    let mut delayed_load = None;
    let mut steps = 0;
    while pc < code.len() {
        steps += 1;
        assert!(steps < 2000, "decoder must terminate");
        let next = jump.take();
        let load = delayed_load.take();
        let get = |reg: Reg| registers[reg.0 as usize];
        let mut write = None;
        match code[pc].kind {
            K::Addiu { rt, rs, imm } => {
                write = Some((rt, get(rs).wrapping_add_signed(i32::from(imm))))
            }
            K::Addu { rd, rs, rt } => write = Some((rd, get(rs).wrapping_add(get(rt)))),
            K::Andi { rt, rs, imm } => write = Some((rt, get(rs) & u32::from(imm))),
            K::Or { rd, rs, rt } => write = Some((rd, get(rs) | get(rt))),
            K::Sll { rd, rt, sa } => write = Some((rd, get(rt) << sa)),
            K::Srl { rd, rt, sa } => write = Some((rd, get(rt) >> sa)),
            K::Sllv { rd, rt, rs } => write = Some((rd, get(rt) << (get(rs) & 31))),
            K::Sltiu { rt, rs, imm } => {
                write = Some((rt, u32::from(get(rs) < i32::from(imm) as u32)))
            }
            K::Sltu { rd, rs, rt } => write = Some((rd, u32::from(get(rs) < get(rt)))),
            K::Lbu { rt, base, offset } | K::Lhu { rt, base, offset } => {
                let address = get(base).wrapping_add_signed(i32::from(offset)) as usize;
                let size = if matches!(code[pc].kind, K::Lhu { .. }) {
                    2
                } else {
                    1
                };
                assert!(
                    address >= INPUT && address + size <= INPUT + record.len()
                        || address >= DICTIONARY
                            && address + size <= DICTIONARY + usize::from(dictionary_rows) * 6
                );
                if size == 2 {
                    assert_eq!(address % 2, 0);
                }
                let value = u32::from(memory[address])
                    | if size == 2 {
                        u32::from(memory[address + 1]) << 8
                    } else {
                        0
                    };
                delayed_load = Some((rt, value));
            }
            K::Sh { rt, base, offset } => {
                let address = get(base).wrapping_add_signed(i32::from(offset)) as usize;
                assert!(address >= OUTPUT && address + 2 <= OUTPUT + 96);
                assert_eq!(address % 2, 0);
                memory[address..address + 2].copy_from_slice(&(get(rt) as u16).to_le_bytes());
            }
            K::Beq { rs, rt, offset } => {
                if get(rs) == get(rt) {
                    jump = Some((pc as isize + 1 + offset as isize) as usize);
                }
            }
            K::Bne { rs, rt, offset } => {
                if get(rs) != get(rt) {
                    jump = Some((pc as isize + 1 + offset as isize) as usize);
                }
            }
            K::Bgtz { rs, offset } => {
                if get(rs) as i32 > 0 {
                    jump = Some((pc as isize + 1 + offset as isize) as usize);
                }
            }
            K::J { target } => jump = Some(((target - BASE) / 4) as usize),
            K::Jr { rs } => jump = Some(((get(rs) - BASE) / 4) as usize),
            other => panic!("unexpected {other:?}"),
        }
        if let Some((reg, value)) = load {
            registers[reg.0 as usize] = value;
        }
        if let Some((reg, value)) = write {
            registers[reg.0 as usize] = value;
        }
        registers[0] = 0;
        pc = next.unwrap_or(pc + 1);
    }
    assert_eq!(&registers[16..24], saved);
    assert_eq!(memory[OUTPUT - 1], 0xA5);
    assert_eq!(&memory[OUTPUT + 96..], [0xA5; 4]);
    (
        registers[Reg::V0.0 as usize],
        memory[OUTPUT..OUTPUT + 96].to_vec(),
    )
}

fn row(index: u16) -> [u8; 6] {
    if index == 0 {
        [0; 6]
    } else {
        let [low, high] = index.to_le_bytes();
        [
            low,
            high,
            low ^ 0x55,
            high ^ 0xAA,
            low.rotate_left(1),
            high.rotate_right(1),
        ]
    }
}

fn record(indices: &[u16; 16]) -> Vec<u8> {
    let mask = indices.iter().enumerate().fold(0u16, |mask, (i, index)| {
        mask | (u16::from(*index != 0) << i)
    });
    let mut bytes = mask.to_le_bytes().to_vec();
    let mut bits = 0u64;
    let mut count = 0;
    for &index in indices.iter().filter(|index| **index != 0) {
        bits |= u64::from(index) << count;
        count += 13;
        while count >= 8 {
            bytes.push(bits as u8);
            bits >>= 8;
            count -= 8;
        }
    }
    if count != 0 {
        bytes.push(bits as u8);
    }
    bytes
}

#[test]
fn bitmap_decoder는_빈행과_13bit_경계를_96바이트로_복원한다() {
    let masks = (0..16)
        .map(|bit| 1u16 << bit)
        .chain([0, u16::MAX, 0x5555, 0xAAAA, 0x8001]);
    for mask in masks {
        let indices = std::array::from_fn(|i| {
            if mask & (1 << i) == 0 {
                0
            } else if i % 2 == 0 {
                1
            } else {
                8191
            }
        });
        let (result, output) = decode_record(&record(&indices), 8192);
        assert_eq!(result, 1);
        assert_eq!(
            output,
            indices.into_iter().flat_map(row).collect::<Vec<_>>()
        );
    }
}

#[test]
fn bitmap_decoder는_잘림_범위이탈_여분입력과_padding을_거부한다() {
    let mut indices = [0; 16];
    indices[7] = 8191;
    let valid = record(&indices);
    for end in 0..valid.len() {
        assert_eq!(decode_record(&valid[..end], 8192).0, 0);
    }
    assert_eq!(decode_record(&valid, 8191).0, 0);
    let mut extra = valid.clone();
    extra.push(0);
    assert_eq!(decode_record(&extra, 8192).0, 0);
    let mut padding = valid;
    *padding.last_mut().unwrap() |= 0x80;
    assert_eq!(decode_record(&padding, 8192).0, 0);
    assert_eq!(decode_record(&[1, 0, 0, 0], 8192).0, 0);
}
