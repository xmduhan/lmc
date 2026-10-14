pub const MEM_SIZE: usize = 65536;
const PORT_BASE: u16 = 60000;
const PORT_END: u16 = 60015;

pub struct Cpu {
    pub mem: Vec<u16>,
    pub pc: u16,
    pub acc: u16,
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub sp: u16,
    pub fp: u16,
    pub z: bool,
    pub n: bool,
    pub halted: bool,
    pub cycles: u64,
    pub io_log: Vec<String>,
    pub input_queue: Vec<i32>,
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            mem: vec![0u16; MEM_SIZE],
            pc: 0,
            acc: 0,
            ax: 0,
            bx: 0,
            cx: 0,
            dx: 0,
            sp: 4999,
            fp: 4999,
            z: false,
            n: false,
            halted: false,
            cycles: 0,
            io_log: Vec::new(),
            input_queue: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.mem = vec![0u16; MEM_SIZE];
        self.pc = 0;
        self.acc = 0;
        self.ax = 0;
        self.bx = 0;
        self.cx = 0;
        self.dx = 0;
        self.sp = 4999;
        self.fp = 4999;
        self.z = false;
        self.n = false;
        self.halted = false;
        self.cycles = 0;
        self.io_log.clear();
        self.input_queue.clear();
    }

    fn set_flags_from_acc(&mut self) {
        self.z = self.acc == 0;
        self.n = (self.acc as i16) < 0;
    }

    fn read_port(&mut self, addr: u16) -> u16 {
        if addr < PORT_BASE || addr > PORT_END {
            self.io_log.push(format!("非法端口读 {}", addr));
            return 0;
        }
        let port = addr - PORT_BASE;
        match port {
            0 | 1 => {
                if let Some(v) = self.input_queue.pop() {
                    v as u16
                } else {
                    0
                }
            }
            2 => 0,
            3 => self.cycles as u16,
            4 => 0,
            _ => 0,
        }
    }

    fn write_port(&mut self, addr: u16, value: u16) {
        if addr < PORT_BASE || addr > PORT_END {
            self.io_log.push(format!("非法端口写 {} <- {}", addr, value));
            return;
        }
        let port = addr - PORT_BASE;
        match port {
            0 => self.io_log.push(format!("OUT 60000 <- {}", value)),
            1 => self.io_log.push(format!(
                "OUT 60001 <- {} ('{}')",
                value,
                char::from_u32((value & 0xFF) as u32).unwrap_or('?')
            )),
            4 => self.io_log.push(format!("OUT 60004 <- {}", value)),
            5 => self.io_log.push(format!("[DEBUG] {}", value)),
            _ => self.io_log.push(format!("OUT {} <- {}", addr, value)),
        }
        if self.io_log.len() > 1000 {
            self.io_log.remove(0);
        }
    }

    pub fn step(&mut self) {
        if self.halted { return; }
        let pc = self.pc as usize;
        let op = self.mem[pc];
        let n = ((op >> 12) & 0xF) as usize;
        let sub = op & 0x0FFF;
        let mut operands = [0u16; 2];
        for i in 0..n {
            let idx = pc + 1 + i;
            if idx < MEM_SIZE {
                operands[i] = self.mem[idx];
            }
        }
        self.pc = self.pc.wrapping_add(1 + n as u16);
        self.cycles += 1;

        match (n, sub) {
            (0, 0x001) => self.halted = true,
            (0, 0x002) => {}
            (0, 0x005) => {
                let sp = self.sp as usize;
                if sp < MEM_SIZE { self.mem[sp] = self.acc; }
                self.sp = self.sp.wrapping_sub(1);
            }
            (0, 0x006) => {
                self.sp = self.sp.wrapping_add(1);
                let sp = self.sp as usize;
                if sp < MEM_SIZE { self.acc = self.mem[sp]; }
                self.set_flags_from_acc();
            }
            (0, 0x007) => {
                self.sp = self.sp.wrapping_add(1);
                let sp = self.sp as usize;
                if sp < MEM_SIZE { self.pc = self.mem[sp]; }
            }
            (1, 0x001) => {
                let a = operands[0] as usize;
                if a < MEM_SIZE { self.acc = self.mem[a]; }
                self.set_flags_from_acc();
            }
            (1, 0x002) => {
                let a = operands[0] as usize;
                if a < MEM_SIZE { self.mem[a] = self.acc; }
            }
            (1, 0x003) => {
                self.acc = operands[0];
                self.set_flags_from_acc();
            }
            (1, 0x004) => {
                let a = operands[0] as usize;
                let v = if a < MEM_SIZE { self.mem[a] } else { 0 };
                self.acc = self.acc.wrapping_add(v);
                self.set_flags_from_acc();
            }
            (1, 0x005) => {
                let a = operands[0] as usize;
                let v = if a < MEM_SIZE { self.mem[a] } else { 0 };
                self.acc = self.acc.wrapping_sub(v);
                self.set_flags_from_acc();
            }
            (1, 0x006) => {
                let a = operands[0] as usize;
                let v = if a < MEM_SIZE { self.mem[a] } else { 0 };
                self.acc &= v;
                self.set_flags_from_acc();
            }
            (1, 0x007) => {
                let a = operands[0] as usize;
                let v = if a < MEM_SIZE { self.mem[a] } else { 0 };
                self.acc |= v;
                self.set_flags_from_acc();
            }
            (1, 0x008) => {
                let a = operands[0] as usize;
                let v = if a < MEM_SIZE { self.mem[a] } else { 0 };
                self.acc ^= v;
                self.set_flags_from_acc();
            }
            (1, 0x021) => {
                let v = operands[0];
                self.acc = self.acc.wrapping_add(v);
                self.set_flags_from_acc();
            }
            (1, 0x022) => {
                let v = operands[0];
                self.acc = self.acc.wrapping_sub(v);
                self.set_flags_from_acc();
            }
            (1, 0x023) => {
                let v = operands[0];
                self.acc &= v;
                self.set_flags_from_acc();
            }
            (1, 0x024) => {
                let v = operands[0];
                self.acc |= v;
                self.set_flags_from_acc();
            }
            (1, 0x025) => {
                let v = operands[0];
                self.acc ^= v;
                self.set_flags_from_acc();
            }
            (1, 0x009) => self.pc = operands[0],
            (1, 0x00A) => { if self.z { self.pc = operands[0]; } }
            (1, 0x00B) => { if self.n { self.pc = operands[0]; } }
            (1, 0x011) => {
                let sp = self.sp as usize;
                if sp < MEM_SIZE { self.mem[sp] = self.pc; }
                self.sp = self.sp.wrapping_sub(1);
                self.pc = operands[0];
            }
            (1, 0x012) => {
                self.acc = self.read_port(operands[0]);
                self.set_flags_from_acc();
            }
            (1, 0x013) => {
                self.write_port(operands[0], self.acc);
            }
            (2, 0x004) => {
                let a = operands[0] as usize;
                let b = operands[1] as usize;
                let va = if a < MEM_SIZE { self.mem[a] } else { 0 };
                let vb = if b < MEM_SIZE { self.mem[b] } else { 0 };
                self.z = va == vb;
                self.n = (va as i16) < (vb as i16);
            }
            (2, 0x005) => {
                self.write_port(operands[0], operands[1]);
            }
            (2, 0x006) => {
                let a = operands[0] as usize;
                let va = if a < MEM_SIZE { self.mem[a] } else { 0 };
                let vb = operands[1];
                self.z = va == vb;
                self.n = (va as i16) < (vb as i16);
            }
            _ => {
                self.halted = true;
            }
        }
    }
}
