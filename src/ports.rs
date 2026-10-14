pub const PORT_BASE: u16 = 60000;
pub const PORT_END: u16 = 60015;

pub fn read_port(addr: u16, cycles: u64, input_queue: &mut Vec<i32>, io_log: &mut Vec<String>) -> u16 {
    if addr < PORT_BASE || addr > PORT_END {
        io_log.push(format!("非法端口读 {}", addr));
        return 0;
    }
    let port = addr - PORT_BASE;
    match port {
        0 | 1 => input_queue.pop().map(|v| v as u16).unwrap_or(0),
        2 => 0,
        3 => cycles as u16,
        4 => 0,
        _ => 0,
    }
}

pub fn write_port(addr: u16, value: u16, io_log: &mut Vec<String>) {
    if addr < PORT_BASE || addr > PORT_END {
        io_log.push(format!("非法端口写 {} <- {}", addr, value));
        return;
    }
    let port = addr - PORT_BASE;
    match port {
        0 => io_log.push(format!("OUT 60000 <- {}", value)),
        1 => io_log.push(format!(
            "OUT 60001 <- {} ('{}')",
            value,
            char::from_u32((value & 0xFF) as u32).unwrap_or('?')
        )),
        4 => io_log.push(format!("OUT 60004 <- {}", value)),
        5 => io_log.push(format!("[DEBUG] {}", value)),
        _ => io_log.push(format!("OUT {} <- {}", addr, value)),
    }
    if io_log.len() > 1000 {
        io_log.remove(0);
    }
}
