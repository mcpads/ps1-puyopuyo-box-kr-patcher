use super::*;

fn selector(runtime: u32, literal: DescriptorRootLiteral) -> Vec<Instruction> {
    [
        K::Lui {
            rt: Reg::T0,
            imm: literal.upper,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: literal.lower,
        },
        K::Sll {
            rd: Reg::T1,
            rt: Reg::A0,
            sa: 2,
        },
        K::Addu {
            rd: Reg::T1,
            rs: Reg::T1,
            rt: Reg::T0,
        },
        K::Lw {
            rt: Reg::T2,
            base: Reg::T1,
            offset: 0,
        },
        K::Sll {
            rd: Reg::T3,
            rt: Reg::A1,
            sa: 3,
        },
        K::Addu {
            rd: Reg::T3,
            rs: Reg::T3,
            rt: Reg::A1,
        },
        K::Sll {
            rd: Reg::A2,
            rt: Reg::T3,
            sa: 2,
        },
        K::Addu {
            rd: Reg::S0,
            rs: Reg::T2,
            rt: Reg::A2,
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(index, kind)| Instruction::new(runtime + index as u32 * 4, 0, kind))
    .collect()
}

#[test]
fn literal과_resource_x36_chain을_찾는다() {
    let runtime = 0x8001_0000;
    let literal = DescriptorRootLiteral::new(0x800B, 0x6710);
    let instructions = selector(runtime, literal);

    assert_eq!(
        descriptor_root_literal_combines(&instructions, literal, 8),
        vec![runtime + 4]
    );
    assert_eq!(
        descriptor_resource_x36_select(&instructions, runtime + 4, literal, 25),
        Some(runtime + 0x20)
    );
}

#[test]
fn root가_descriptor전에_덮이면_selector가_아니다() {
    let runtime = 0x8001_0000;
    let literal = DescriptorRootLiteral::new(0x800B, 0x4330);
    let mut instructions = selector(runtime, literal);
    instructions.insert(
        2,
        Instruction::new(
            runtime + 8,
            0,
            K::Addiu {
                rt: Reg::T0,
                rs: Reg::T0,
                imm: literal.lower,
            },
        ),
    );
    for (index, instruction) in instructions.iter_mut().enumerate().skip(3) {
        instruction.addr = runtime + index as u32 * 4;
    }

    assert_eq!(
        descriptor_root_literal_combines(&instructions, literal, 8),
        vec![runtime + 4, runtime + 8]
    );
    assert_eq!(
        descriptor_resource_x36_select(&instructions, runtime + 4, literal, 25),
        None
    );
    assert_eq!(
        descriptor_resource_x36_select(&instructions, runtime + 8, literal, 25),
        Some(runtime + 0x24)
    );
}

#[test]
fn 다른_upper나_잘못된_stride는_거부한다() {
    let runtime = 0x8001_0000;
    let literal = DescriptorRootLiteral::new(0x800B, 0x6710);
    let mut instructions = selector(runtime, literal);
    instructions[0].kind = K::Lui {
        rt: Reg::T0,
        imm: 0x800C,
    };
    assert!(descriptor_root_literal_combines(&instructions, literal, 8).is_empty());

    instructions[0].kind = K::Lui {
        rt: Reg::T0,
        imm: literal.upper,
    };
    instructions[7].kind = K::Sll {
        rd: Reg::A2,
        rt: Reg::T3,
        sa: 1,
    };
    assert_eq!(
        descriptor_resource_x36_select(&instructions, runtime + 4, literal, 25),
        None
    );
}
