use lmc::cpu::Cpu;

#[test]
fn add_immediate_and_memory() {
    let mut cpu = Cpu::new();
    cpu.pc = 100;
    cpu.mem[100] = 0x1003;
    cpu.mem[101] = 5;
    cpu.mem[102] = 0x1004;
    cpu.mem[103] = 200;
    cpu.mem[200] = 7;
    cpu.step();
    assert_eq!(cpu.acc, 5);
    cpu.step();
    assert_eq!(cpu.acc, 12);
    assert!(!cpu.z);
    assert!(!cpu.n);
}

#[test]
fn out_port_logs_value() {
    let mut cpu = Cpu::new();
    cpu.pc = 100;
    cpu.mem[100] = 0x1003;
    cpu.mem[101] = 42;
    cpu.mem[102] = 0x1013;
    cpu.mem[103] = 60000;
    cpu.step();
    cpu.step();
    assert_eq!(cpu.io_log.last().unwrap(), "OUT 60000 <- 42");
}

#[test]
fn call_ret_uses_stack() {
    let mut cpu = Cpu::new();
    cpu.pc = 100;
    cpu.mem[100] = 0x1011;
    cpu.mem[101] = 104;
    cpu.mem[102] = 0x0001;
    cpu.mem[104] = 0x1003;
    cpu.mem[105] = 9;
    cpu.mem[106] = 0x0007;
    cpu.step();
    assert_eq!(cpu.pc, 104);
    cpu.step();
    assert_eq!(cpu.acc, 9);
    cpu.step();
    assert_eq!(cpu.pc, 102);
}

#[test]
fn add_immediate_instruction() {
    let mut cpu = Cpu::new();
    cpu.pc = 100;
    cpu.mem[100] = 0x1021; // ADD imm
    cpu.mem[101] = 5;
    cpu.acc = 10;
    cpu.step();
    assert_eq!(cpu.acc, 15);
}

#[test]
fn cmp_immediate_instruction() {
    let mut cpu = Cpu::new();
    cpu.pc = 100;
    cpu.mem[100] = 0x2006; // CMP [a] imm
    cpu.mem[101] = 200; // addr a
    cpu.mem[102] = 42; // imm
    cpu.mem[200] = 42;
    cpu.step();
    assert!(cpu.z);
    assert!(!cpu.n);

    cpu.mem[200] = 41;
    cpu.pc = 100;
    cpu.step();
    assert!(!cpu.z);
    assert!(cpu.n);
}
