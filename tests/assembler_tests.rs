use lmc::assembler::{assemble_and_load, AsmResult};
use lmc::cpu::Cpu;

fn assemble(source: &str) -> (Cpu, AsmResult) {
    let mut cpu = Cpu::new();
    let mut asm = AsmResult::empty();
    let mut status = String::new();
    let mut dirty = true;
    assemble_and_load(&mut cpu, source, &mut asm, &mut status, &mut dirty);
    (cpu, asm)
}

#[test]
fn assembles_ldi_and_out() {
    let (cpu, asm) = assemble("LDI 42\nOUT 60000\n");
    assert!(asm.errors.is_empty(), "{:?}", asm.errors);
    assert_eq!(asm.code, vec![0x1003, 42, 0x1013, 60000]);
    assert_eq!(cpu.mem[0], 0x1003);
    assert_eq!(cpu.mem[1], 42);
    assert_eq!(cpu.mem[2], 0x1013);
    assert_eq!(cpu.mem[3], 60000);
}

#[test]
fn assembles_labels_and_equ() {
    let (_, asm) = assemble("start: LDI one\nOUT 60000\none: EQU 7\n");
    assert!(asm.errors.is_empty(), "{:?}", asm.errors);
    assert_eq!(asm.code, vec![0x1003, 7, 0x1013, 60000]);
}

#[test]
fn reports_unknown_mnemonic() {
    let (_, asm) = assemble("FOO\n");
    assert!(!asm.errors.is_empty());
}
