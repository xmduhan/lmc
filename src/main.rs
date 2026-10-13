use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::{Duration, Instant};

use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::mouse::MouseButton;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::ttf::Font;
use sdl2::video::{Window, WindowContext};

const MEM_SIZE: usize = 65536;
const PORT_BASE: u16 = 60000;
const PORT_END: u16 = 60015;

#[derive(Clone)]
struct AsmResult {
    code: Vec<u16>,
    line_map: HashMap<u16, usize>,
    errors: Vec<String>,
}

impl AsmResult {
    fn empty() -> Self {
        AsmResult {
            code: Vec::new(),
            line_map: HashMap::new(),
            errors: Vec::new(),
        }
    }
}

struct Cpu {
    mem: Vec<u16>,
    pc: u16,
    acc: u16,
    z: bool,
    n: bool,
    halted: bool,
    cycles: u64,
    io_log: Vec<String>,
    input_queue: Vec<i32>,
}

impl Cpu {
    fn new() -> Self {
        let mut mem = vec![0u16; MEM_SIZE];
        mem[0] = 4999;
        mem[1] = 4999;
        Cpu {
            mem,
            pc: 0,
            acc: 0,
            z: false,
            n: false,
            halted: false,
            cycles: 0,
            io_log: Vec::new(),
            input_queue: Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.mem = vec![0u16; MEM_SIZE];
        self.mem[0] = 4999;
        self.mem[1] = 4999;
        self.pc = 0;
        self.acc = 0;
        self.z = false;
        self.n = false;
        self.halted = false;
        self.cycles = 0;
        self.io_log.clear();
        self.input_queue.clear();
    }

    fn sp(&self) -> u16 { self.mem[0] }
    fn set_sp(&mut self, v: u16) { self.mem[0] = v; }
    fn fp(&self) -> u16 { self.mem[1] }

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

    fn step(&mut self) {
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
                let sp = self.sp() as usize;
                if sp < MEM_SIZE { self.mem[sp] = self.acc; }
                self.set_sp(self.sp().wrapping_sub(1));
            }
            (0, 0x006) => {
                self.set_sp(self.sp().wrapping_add(1));
                let sp = self.sp() as usize;
                if sp < MEM_SIZE { self.acc = self.mem[sp]; }
                self.set_flags_from_acc();
            }
            (0, 0x007) => {
                self.set_sp(self.sp().wrapping_add(1));
                let sp = self.sp() as usize;
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
            (1, 0x009) => self.pc = operands[0],
            (1, 0x00A) => { if self.z { self.pc = operands[0]; } }
            (1, 0x00B) => { if self.n { self.pc = operands[0]; } }
            (1, 0x011) => {
                let sp = self.sp() as usize;
                if sp < MEM_SIZE { self.mem[sp] = self.pc; }
                self.set_sp(self.sp().wrapping_sub(1));
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
            _ => {
                self.halted = true;
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputMode {
    None,
    Open,
    Save,
    Export,
}

fn find_font() -> Option<String> {
    let paths = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
        "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
        "C:\\Windows\\Fonts\\consola.ttf",
        "C:\\Windows\\Fonts\\lucon.ttf",
    ];
    paths.iter().find(|p| std::path::Path::new(p).exists()).map(|s| s.to_string())
}

fn draw_text(
    canvas: &mut Canvas<Window>,
    tc: &TextureCreator<WindowContext>,
    font: &Font,
    text: &str,
    x: i32,
    y: i32,
    color: Color,
) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }
    let surface = font.render(text).blended(color).map_err(|e| e.to_string())?;
    let texture = tc.create_texture_from_surface(&surface).map_err(|e| e.to_string())?;
    let query = texture.query();
    canvas.copy(&texture, None, Some(Rect::new(x, y, query.width, query.height)))?;
    Ok(())
}

fn cursor_line_col(text: &str, cursor: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    for (i, ch) in text.char_indices() {
        if i >= cursor { break; }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn line_start_byte(text: &str, line: usize) -> usize {
    if line == 0 { return 0; }
    let mut count = 0;
    for (i, ch) in text.char_indices() {
        if ch == '\n' {
            count += 1;
            if count == line {
                return i + 1;
            }
        }
    }
    text.len()
}

fn line_end_byte(text: &str, line: usize) -> usize {
    let start = line_start_byte(text, line);
    let rest = &text[start..];
    if let Some(pos) = rest.find('\n') {
        start + pos
    } else {
        text.len()
    }
}

fn set_cursor_from_line_col(text: &str, cursor: &mut usize, line: usize, col: usize) {
    let start = line_start_byte(text, line);
    let end = line_end_byte(text, line);
    let mut byte = start;
    let mut c = 0;
    for (i, ch) in text[start..end].char_indices() {
        if c >= col { break; }
        byte = start + i + ch.len_utf8();
        c += 1;
    }
    *cursor = byte.min(text.len());
}

fn instr_info(mnemonic: &str) -> Option<(usize, u16)> {
    match mnemonic {
        "HLT" => Some((0, 0x001)),
        "NOP" => Some((0, 0x002)),
        "PUSH" => Some((0, 0x005)),
        "POP" => Some((0, 0x006)),
        "RET" => Some((0, 0x007)),
        "LDA" => Some((1, 0x001)),
        "STA" => Some((1, 0x002)),
        "LDI" => Some((1, 0x003)),
        "ADD" => Some((1, 0x004)),
        "SUB" => Some((1, 0x005)),
        "AND" => Some((1, 0x006)),
        "OR" => Some((1, 0x007)),
        "XOR" => Some((1, 0x008)),
        "BRA" => Some((1, 0x009)),
        "BRZ" => Some((1, 0x00A)),
        "BLT" => Some((1, 0x00B)),
        "CALL" => Some((1, 0x011)),
        "IN" => Some((1, 0x012)),
        "OUT" => Some((1, 0x013)),
        "CMP" => Some((2, 0x004)),
        "OUTI" => Some((2, 0x005)),
        _ => None,
    }
}

fn parse_operand(
    s: &str,
    labels: &HashMap<String, u16>,
    consts: &HashMap<String, u16>,
) -> Result<u16, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("缺少操作数".to_string());
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).map_err(|e| e.to_string())
    } else if let Ok(v) = s.parse::<u16>() {
        Ok(v)
    } else if let Some(v) = consts.get(s).or_else(|| labels.get(s)) {
        Ok(*v)
    } else {
        Err(format!("未知符号 {}", s))
    }
}

fn assemble(source: &str) -> AsmResult {
    let mut labels = HashMap::new();
    let mut consts = HashMap::new();
    let mut errors = Vec::new();
    let lines: Vec<&str> = source.split('\n').collect();
    let mut addr: u16 = 0;

    for (line_idx, raw) in lines.iter().enumerate() {
        let mut line = raw.split(';').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        let mut label_opt: Option<String> = None;
        if let Some(colon) = line.find(':') {
            let label = line[..colon].trim();
            if !label.is_empty() {
                label_opt = Some(label.to_string());
            }
            line = line[colon + 1..].trim();
            if line.is_empty() {
                if let Some(lbl) = label_opt {
                    if labels.contains_key(&lbl) || consts.contains_key(&lbl) {
                        errors.push(format!("行 {}: 重复符号 {}", line_idx + 1, lbl));
                    } else {
                        labels.insert(lbl, addr);
                    }
                }
                continue;
            }
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            if let Some(lbl) = label_opt {
                if labels.contains_key(&lbl) || consts.contains_key(&lbl) {
                    errors.push(format!("行 {}: 重复符号 {}", line_idx + 1, lbl));
                } else {
                    labels.insert(lbl, addr);
                }
            }
            continue;
        }
        if tokens[0].to_uppercase() == "EQU" {
            let name = label_opt.clone().unwrap_or_default();
            if name.is_empty() {
                errors.push(format!("行 {}: EQU 缺少名称", line_idx + 1));
                continue;
            }
            let value_tok = tokens.get(1).copied().unwrap_or("");
            match parse_operand(value_tok, &labels, &consts) {
                Ok(v) => { consts.insert(name, v); }
                Err(e) => errors.push(format!("行 {}: EQU 值错误 {}", line_idx + 1, e)),
            }
            continue;
        }
        if tokens.len() >= 2 && tokens[1].to_uppercase() == "EQU" {
            let name = tokens[0].to_string();
            let value_tok = tokens.get(2).copied().unwrap_or("");
            match parse_operand(value_tok, &labels, &consts) {
                Ok(v) => { consts.insert(name, v); }
                Err(e) => errors.push(format!("行 {}: EQU 值错误 {}", line_idx + 1, e)),
            }
            continue;
        }
        if let Some(lbl) = label_opt {
            if labels.contains_key(&lbl) || consts.contains_key(&lbl) {
                errors.push(format!("行 {}: 重复符号 {}", line_idx + 1, lbl));
            } else {
                labels.insert(lbl, addr);
            }
        }
        let mnemonic = tokens[0].to_uppercase();
        if let Some((n, _)) = instr_info(&mnemonic) {
            addr = addr.wrapping_add(1 + n as u16);
        } else {
            errors.push(format!("行 {}: 未知指令 {}", line_idx + 1, mnemonic));
        }
    }

    let mut code = Vec::new();
    let mut line_map = HashMap::new();
    let mut addr: u16 = 0;
    for (line_idx, raw) in lines.iter().enumerate() {
        let mut line = raw.split(';').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        if let Some(colon) = line.find(':') {
            line = line[colon + 1..].trim();
            if line.is_empty() { continue; }
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() { continue; }
        if tokens[0].to_uppercase() == "EQU" { continue; }
        if tokens.len() >= 2 && tokens[1].to_uppercase() == "EQU" { continue; }
        let mnemonic = tokens[0].to_uppercase();
        if let Some((n, sub)) = instr_info(&mnemonic) {
            let machine = ((n as u16) << 12) | sub;
            code.push(machine);
            line_map.insert(addr, line_idx);
            addr = addr.wrapping_add(1);
            for i in 0..n {
                let tok = tokens.get(1 + i).copied().unwrap_or("");
                let val = match parse_operand(tok, &labels, &consts) {
                    Ok(v) => v,
                    Err(e) => {
                        errors.push(format!("行 {}: 操作数错误 {} ({})", line_idx + 1, tok, e));
                        0
                    }
                };
                code.push(val);
                line_map.insert(addr, line_idx);
                addr = addr.wrapping_add(1);
            }
            if tokens.len() > 1 + n {
                errors.push(format!("行 {}: 指令 {} 操作数过多", line_idx + 1, mnemonic));
            }
        }
    }

    AsmResult { code, line_map, errors }
}

fn assemble_and_load(
    cpu: &mut Cpu,
    source: &str,
    asm: &mut AsmResult,
    status: &mut String,
    dirty: &mut bool,
) {
    let result = assemble(source);
    *asm = result;
    if asm.errors.is_empty() {
        cpu.reset();
        for (i, word) in asm.code.iter().enumerate() {
            if i < MEM_SIZE {
                cpu.mem[i] = *word;
            }
        }
        cpu.pc = 0;
        cpu.halted = false;
        *dirty = false;
        *status = format!("汇编成功: {} 字", asm.code.len());
    } else {
        *status = format!("汇编错误: {}", asm.errors[0]);
    }
}

fn main() -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let video = sdl_context.video()?;
    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string())?;
    let font_path = find_font().ok_or_else(|| "找不到等宽字体".to_string())?;
    let mut font = ttf_context.load_font(font_path, 18)?;
    font.set_style(sdl2::ttf::FontStyle::NORMAL);

    let window = video
        .window("LMC 模拟器 - Turbo C 风格", 1920, 1080)
        .fullscreen_desktop()
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;
    let mut canvas = window
        .into_canvas()
        .accelerated()
        .present_vsync()
        .build()
        .map_err(|e| e.to_string())?;
    let texture_creator = canvas.texture_creator();
    let mut event_pump = sdl_context.event_pump()?;
    let mut _text_input = video.text_input();
    _text_input.start();

    let mut cpu = Cpu::new();
    let default_source = r#"LDI 0
STA 100
LDI 1
STA 101
loop:
LDA 100
ADD 101
STA 100
LDA 101
ADD one
STA 101
LDA 101
SUB ten
BLT loop
LDA 100
OUT 60000
HLT
one: EQU 1
ten: EQU 10
"#;
    let mut editor_text = default_source.to_string();
    let mut editor_cursor = editor_text.len();
    let mut editor_scroll = 0usize;
    let mut mem_scroll: u16 = 0;
    let mut breakpoints: HashSet<usize> = HashSet::new();
    let mut asm = AsmResult::empty();
    let mut dirty = true;
    let mut running = false;
    let mut speed: u32 = 5;
    let mut status = String::from("就绪");
    let mut input_mode = InputMode::None;
    let mut input_buffer = String::new();
    let mut mouse_x = 0;
    let mut mouse_y = 0;
    let mut last_tick = Instant::now();

    assemble_and_load(&mut cpu, &editor_text, &mut asm, &mut status, &mut dirty);

    'outer: loop {
        let (win_w, win_h) = canvas.output_size()?;
        let line_h = font.height() + 4;
        let char_w = font.size_of("M").map(|(w, _)| w).unwrap_or(8) as i32;

        let top_h: u32 = 34;
        let bottom_h: u32 = 30;
        let main_h = win_h.saturating_sub(top_h).saturating_sub(bottom_h);
        let left_w = win_w * 3 / 5;
        let editor_rect = Rect::new(0, top_h as i32, left_w, main_h);
        let right_x = left_w as i32;
        let right_w = win_w - left_w;
        let reg_rect = Rect::new(right_x, top_h as i32, right_w, 230);
        let mem_rect = Rect::new(right_x, top_h as i32 + 230, right_w, main_h.saturating_sub(230));
        let status_rect = Rect::new(0, (win_h - bottom_h) as i32, win_w, bottom_h);

        let now = Instant::now();
        let dt = now.duration_since(last_tick).as_secs_f64();
        last_tick = now;

        if running && !cpu.halted && asm.errors.is_empty() {
            let target_ips = 80.0 * (speed as f64).powi(2);
            let mut steps = (target_ips * dt).round() as u32;
            if steps == 0 && dt > 0.0 { steps = 1; }
            for _ in 0..steps.min(200_000) {
                if let Some(&line) = asm.line_map.get(&cpu.pc) {
                    if breakpoints.contains(&line) {
                        running = false;
                        status = format!("断点: 行 {}", line + 1);
                        break;
                    }
                }
                cpu.step();
                if cpu.halted {
                    running = false;
                    status = "停机".to_string();
                    break;
                }
            }
        }

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'outer,
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    if input_mode != InputMode::None {
                        input_mode = InputMode::None;
                        input_buffer.clear();
                    } else {
                        break 'outer;
                    }
                }
                Event::TextInput { text, .. } => {
                    if input_mode != InputMode::None {
                        input_buffer.push_str(&text);
                    } else {
                        editor_text.insert_str(editor_cursor, &text);
                        editor_cursor += text.len();
                        dirty = true;
                    }
                }
                Event::KeyDown { keycode: Some(key), keymod, .. } => {
                    if input_mode != InputMode::None {
                        match key {
                            Keycode::Return => {
                                let mode = input_mode;
                                input_mode = InputMode::None;
                                match mode {
                                    InputMode::Open => match fs::read_to_string(&input_buffer) {
                                        Ok(s) => {
                                            editor_text = s;
                                            editor_cursor = editor_text.len();
                                            dirty = true;
                                            status = format!("已打开 {}", input_buffer);
                                        }
                                        Err(e) => status = format!("打开失败: {}", e),
                                    },
                                    InputMode::Save => match fs::write(&input_buffer, &editor_text) {
                                        Ok(_) => status = format!("已保存 {}", input_buffer),
                                        Err(e) => status = format!("保存失败: {}", e),
                                    },
                                    InputMode::Export => {
                                        let mut out = String::new();
                                        for word in &asm.code {
                                            out.push_str(&format!("{:04X}\n", word));
                                        }
                                        match fs::write(&input_buffer, out) {
                                            Ok(_) => status = format!("已导出 {}", input_buffer),
                                            Err(e) => status = format!("导出失败: {}", e),
                                        }
                                    }
                                    InputMode::None => {}
                                }
                                input_buffer.clear();
                            }
                            Keycode::Backspace => {
                                input_buffer.pop();
                            }
                            _ => {}
                        }
                    } else {
                        match key {
                            Keycode::F5 => {
                                if dirty {
                                    assemble_and_load(&mut cpu, &editor_text, &mut asm, &mut status, &mut dirty);
                                }
                                if !asm.errors.is_empty() {
                                    status = format!("汇编错误: {}", asm.errors[0]);
                                    running = false;
                                } else {
                                    if cpu.halted {
                                        cpu.reset();
                                        assemble_and_load(&mut cpu, &editor_text, &mut asm, &mut status, &mut dirty);
                                    }
                                    running = !running;
                                    if running {
                                        last_tick = Instant::now();
                                        status = "运行".to_string();
                                    } else {
                                        status = "暂停".to_string();
                                    }
                                }
                            }
                            Keycode::F6 => {
                                if dirty {
                                    assemble_and_load(&mut cpu, &editor_text, &mut asm, &mut status, &mut dirty);
                                }
                                running = false;
                                if asm.errors.is_empty() && !cpu.halted {
                                    cpu.step();
                                    status = format!("单步 PC=0x{:04X}", cpu.pc);
                                } else if cpu.halted {
                                    status = "已停机".to_string();
                                } else {
                                    status = format!("汇编错误: {}", asm.errors.first().cloned().unwrap_or_default());
                                }
                            }
                            Keycode::F7 => {
                                cpu.reset();
                                assemble_and_load(&mut cpu, &editor_text, &mut asm, &mut status, &mut dirty);
                                running = false;
                                status = "复位".to_string();
                            }
                            Keycode::F8 => {
                                running = false;
                                input_mode = InputMode::Export;
                                input_buffer = "program.hex".to_string();
                            }
                            Keycode::F9 => {
                                let (line, _) = cursor_line_col(&editor_text, editor_cursor);
                                if breakpoints.contains(&line) {
                                    breakpoints.remove(&line);
                                } else {
                                    breakpoints.insert(line);
                                }
                                status = format!("断点行 {}", line + 1);
                            }
                            Keycode::O if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => {
                                running = false;
                                input_mode = InputMode::Open;
                                input_buffer = "program.asm".to_string();
                            }
                            Keycode::S if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => {
                                running = false;
                                input_mode = InputMode::Save;
                                input_buffer = "program.asm".to_string();
                            }
                            Keycode::Backspace => {
                                if editor_cursor > 0 {
                                    let prev = editor_text[..editor_cursor]
                                        .char_indices()
                                        .last()
                                        .map(|(i, _)| i)
                                        .unwrap_or(0);
                                    editor_text.remove(prev);
                                    editor_cursor = prev;
                                    dirty = true;
                                }
                            }
                            Keycode::Delete => {
                                if editor_cursor < editor_text.len() {
                                    editor_text.remove(editor_cursor);
                                    dirty = true;
                                }
                            }
                            Keycode::Left => {
                                if editor_cursor > 0 {
                                    editor_cursor = editor_text[..editor_cursor]
                                        .char_indices()
                                        .last()
                                        .map(|(i, _)| i)
                                        .unwrap_or(0);
                                }
                            }
                            Keycode::Right => {
                                if editor_cursor < editor_text.len() {
                                    if let Some(ch) = editor_text[editor_cursor..].chars().next() {
                                        editor_cursor += ch.len_utf8();
                                    }
                                }
                            }
                            Keycode::Up => {
                                let (line, col) = cursor_line_col(&editor_text, editor_cursor);
                                if line > 0 {
                                    set_cursor_from_line_col(&editor_text, &mut editor_cursor, line - 1, col);
                                }
                            }
                            Keycode::Down => {
                                let (line, col) = cursor_line_col(&editor_text, editor_cursor);
                                set_cursor_from_line_col(&editor_text, &mut editor_cursor, line + 1, col);
                            }
                            Keycode::Home => {
                                let (line, _) = cursor_line_col(&editor_text, editor_cursor);
                                set_cursor_from_line_col(&editor_text, &mut editor_cursor, line, 0);
                            }
                            Keycode::End => {
                                let (line, _) = cursor_line_col(&editor_text, editor_cursor);
                                let end = line_end_byte(&editor_text, line);
                                editor_cursor = end;
                            }
                            Keycode::Return => {
                                editor_text.insert(editor_cursor, '\n');
                                editor_cursor += 1;
                                dirty = true;
                            }
                            Keycode::Tab => {
                                editor_text.insert_str(editor_cursor, "    ");
                                editor_cursor += 4;
                                dirty = true;
                            }
                            Keycode::Plus | Keycode::Equals => {
                                speed = (speed + 1).min(10);
                                status = format!("速度 {}", speed);
                            }
                            Keycode::Minus => {
                                speed = speed.saturating_sub(1).max(1);
                                status = format!("速度 {}", speed);
                            }
                            _ => {}
                        }
                    }
                }
                Event::MouseMotion { x, y, .. } => {
                    mouse_x = x;
                    mouse_y = y;
                }
                Event::MouseWheel { y, .. } => {
                    if mouse_x < left_w as i32
                        && mouse_y >= editor_rect.y()
                        && mouse_y < editor_rect.y() + editor_rect.height() as i32
                    {
                        if y > 0 {
                            editor_scroll = editor_scroll.saturating_sub(1);
                        } else if y < 0 {
                            editor_scroll = editor_scroll.saturating_add(1);
                        }
                    } else if mouse_y >= mem_rect.y()
                        && mouse_y < mem_rect.y() + mem_rect.height() as i32
                    {
                        if y > 0 {
                            mem_scroll = mem_scroll.wrapping_sub(16);
                        } else if y < 0 {
                            mem_scroll = mem_scroll.wrapping_add(16);
                        }
                    }
                }
                Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. } => {
                    if x < left_w as i32
                        && y >= editor_rect.y()
                        && y < editor_rect.y() + editor_rect.height() as i32
                    {
                        if x < editor_rect.x() + 60 {
                            let line = editor_scroll + ((y - editor_rect.y()) / line_h) as usize;
                            if breakpoints.contains(&line) {
                                breakpoints.remove(&line);
                            } else {
                                breakpoints.insert(line);
                            }
                        } else {
                            let line = editor_scroll + ((y - editor_rect.y()) / line_h) as usize;
                            let col = ((x - (editor_rect.x() + 64)) / char_w).max(0) as usize;
                            set_cursor_from_line_col(&editor_text, &mut editor_cursor, line, col);
                        }
                    }
                }
                _ => {}
            }
        }

        canvas.set_draw_color(Color::RGB(0, 0, 170));
        canvas.clear();

        canvas.set_draw_color(Color::RGB(0, 0, 110));
        canvas.fill_rect(Rect::new(0, 0, win_w, top_h))?;
        draw_text(&mut canvas, &texture_creator, &font, "LMC 模拟器 - SDL2 自绘 UI - Turbo C 风格", 10, 6, Color::RGB(255, 255, 0))?;

        canvas.set_draw_color(Color::RGB(0, 0, 140));
        canvas.fill_rect(editor_rect)?;
        canvas.set_draw_color(Color::RGB(0, 0, 110));
        canvas.fill_rect(Rect::new(editor_rect.x(), editor_rect.y(), 60, editor_rect.height()))?;

        canvas.set_draw_color(Color::RGB(0, 0, 150));
        canvas.fill_rect(reg_rect)?;
        canvas.fill_rect(mem_rect)?;

        let text_x = editor_rect.x() + 64;
        let text_y = editor_rect.y() + 4;
        let visible_lines = (editor_rect.height() as i32 / line_h).max(1) as usize;
        let total_lines = editor_text.split('\n').count();
        if editor_scroll + visible_lines > total_lines {
            editor_scroll = total_lines.saturating_sub(visible_lines);
        }
        let lines: Vec<&str> = editor_text.split('\n').collect();
        let exec_line = asm.line_map.get(&cpu.pc).copied();
        for i in 0..visible_lines {
            let line_idx = editor_scroll + i;
            if line_idx >= lines.len() { break; }
            let y = text_y + (i as i32) * line_h;
            if Some(line_idx) == exec_line && !cpu.halted {
                canvas.set_draw_color(Color::RGB(0, 0, 80));
                canvas.fill_rect(Rect::new(editor_rect.x(), y, editor_rect.width(), line_h as u32))?;
            }
            if breakpoints.contains(&line_idx) {
                canvas.set_draw_color(Color::RGB(255, 0, 0));
                canvas.fill_rect(Rect::new(editor_rect.x() + 5, y + line_h / 2 - 4, 8, 8))?;
            }
            draw_text(&mut canvas, &texture_creator, &font, &format!("{:4}", line_idx + 1), editor_rect.x() + 5, y, Color::RGB(180, 180, 255))?;
            draw_text(&mut canvas, &texture_creator, &font, lines[line_idx], text_x, y, Color::RGB(255, 255, 0))?;
        }
        if input_mode == InputMode::None {
            let cursor_line = editor_text[..editor_cursor].chars().filter(|&c| c == '\n').count();
            if cursor_line >= editor_scroll && cursor_line < editor_scroll + visible_lines {
                let line_start = editor_text[..editor_cursor].rfind('\n').map(|i| i + 1).unwrap_or(0);
                let col = editor_text[line_start..editor_cursor].chars().count();
                let y = text_y + ((cursor_line - editor_scroll) as i32) * line_h;
                let x = text_x + (col as i32) * char_w;
                canvas.set_draw_color(Color::RGB(255, 255, 255));
                canvas.fill_rect(Rect::new(x, y, 2, font.height() as u32))?;
            }
        }

        let mut ry = reg_rect.y() + 8;
        draw_text(&mut canvas, &texture_creator, &font, "寄存器", reg_rect.x() + 8, ry, Color::RGB(0, 255, 255))?;
        ry += line_h;
        let regs = [
            format!("PC    = 0x{:04X} ({})", cpu.pc, cpu.pc),
            format!("ACC   = 0x{:04X} ({})", cpu.acc, cpu.acc as i16),
            format!("Z     = {}", cpu.z),
            format!("N     = {}", cpu.n),
            format!("HALT  = {}", cpu.halted),
            format!("CYCLE = {}", cpu.cycles),
            format!("SP    = 0x{:04X} ({})", cpu.sp(), cpu.sp()),
            format!("FP    = 0x{:04X} ({})", cpu.fp(), cpu.fp()),
        ];
        for reg in &regs {
            draw_text(&mut canvas, &texture_creator, &font, reg, reg_rect.x() + 8, ry, Color::RGB(255, 255, 0))?;
            ry += line_h;
        }

        draw_text(&mut canvas, &texture_creator, &font, "内存 (十六进制)", mem_rect.x() + 8, mem_rect.y() + 4, Color::RGB(0, 255, 255))?;
        let mem_visible = ((mem_rect.height() as i32 - 30) / line_h).max(1) as usize;
        let start = mem_scroll & !0xF;
        let mut my = mem_rect.y() + 30;
        for row in 0..mem_visible {
            let addr = start.wrapping_add((row as u16) * 16);
            let mut s = format!("{:04X}:", addr);
            for col in 0..16 {
                let a = addr.wrapping_add(col);
                s.push_str(&format!(" {:04X}", cpu.mem[a as usize]));
            }
            draw_text(&mut canvas, &texture_creator, &font, &s, mem_rect.x() + 8, my, Color::RGB(255, 255, 0))?;
            my += line_h;
        }

        canvas.set_draw_color(Color::RGB(0, 0, 110));
        canvas.fill_rect(status_rect)?;
        let run_state = if running { "运行" } else if cpu.halted { "停机" } else { "暂停" };
        let io = cpu.io_log.last().map(|s| s.as_str()).unwrap_or("");
        let status_text = format!(
            "{} | 速度 {} (+/-) | {} | F5运行/暂停 F6单步 F7复位 F8导出 F9断点 Ctrl+O打开 Ctrl+S保存 | {}",
            run_state, speed, status, io
        );
        draw_text(&mut canvas, &texture_creator, &font, &status_text, 8, status_rect.y() + 4, Color::RGB(255, 255, 0))?;
        if input_mode != InputMode::None {
            let prompt = match input_mode {
                InputMode::Open => "打开文件路径: ",
                InputMode::Save => "保存文件路径: ",
                InputMode::Export => "导出机器码路径: ",
                InputMode::None => "",
            };
            draw_text(&mut canvas, &texture_creator, &font, &format!("{}{}_", prompt, input_buffer), 8, status_rect.y() + 4, Color::RGB(255, 255, 255))?;
        }

        canvas.present();
        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}
