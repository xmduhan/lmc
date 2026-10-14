mod assembler;
mod cpu;
mod editor;
mod ui;

use std::collections::HashSet;
use std::fs;
use std::time::{Duration, Instant};

use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::mouse::MouseButton;
use sdl2::pixels::Color;
use sdl2::rect::Rect;

use assembler::{assemble_table, load_into_cpu, AsmResult, instr_info};
use cpu::Cpu;
use ui::{draw_text, find_font};

const FONT_SIZE: u16 = 18;
const MEM_RIGHT_MARGIN: i32 = 8;
const MEM_GAP_TRIM: i32 = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputMode {
    None,
    Open,
    Save,
    Export,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VimMode {
    Normal,
    Insert,
    Command,
}

impl VimMode {
    fn name(self) -> &'static str {
        match self {
            VimMode::Normal => "普通",
            VimMode::Insert => "编辑",
            VimMode::Command => "命令",
        }
    }
}

fn main() -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let video = sdl_context.video()?;
    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string())?;
    let font_path = find_font().ok_or_else(|| "找不到等宽字体".to_string())?;

    let mut font = ttf_context.load_font(&font_path, FONT_SIZE)?;
    font.set_style(sdl2::ttf::FontStyle::NORMAL);
    let char_w = font.size_of("M").map(|(w, _)| w).unwrap_or(8) as i32;

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
    let mut table: Vec<[String; 4]> = vec![
        ["".to_string(), "LDI".to_string(), "0x0000".to_string(), "".to_string()],
        ["".to_string(), "STA".to_string(), "[0x0064]".to_string(), "".to_string()],
        ["".to_string(), "LDI".to_string(), "0x0001".to_string(), "".to_string()],
        ["".to_string(), "STA".to_string(), "[0x0065]".to_string(), "".to_string()],
        ["loop".to_string(), "LDA".to_string(), "[0x0065]".to_string(), "".to_string()],
        ["".to_string(), "ADD".to_string(), "[0x0064]".to_string(), "".to_string()],
        ["".to_string(), "STA".to_string(), "[0x0064]".to_string(), "".to_string()],
        ["".to_string(), "LDA".to_string(), "[0x0065]".to_string(), "".to_string()],
        ["".to_string(), "ADD".to_string(), "[one]".to_string(), "".to_string()],
        ["".to_string(), "STA".to_string(), "[0x0065]".to_string(), "".to_string()],
        ["".to_string(), "LDA".to_string(), "[0x0065]".to_string(), "".to_string()],
        ["".to_string(), "SUB".to_string(), "[ten]".to_string(), "".to_string()],
        ["".to_string(), "BLT".to_string(), "loop".to_string(), "".to_string()],
        ["".to_string(), "LDA".to_string(), "[0x0064]".to_string(), "".to_string()],
        ["".to_string(), "OUT".to_string(), "0xEA60".to_string(), "".to_string()],
        ["".to_string(), "HLT".to_string(), "".to_string(), "".to_string()],
        ["one".to_string(), "0x0001".to_string(), "".to_string(), "".to_string()],
        ["ten".to_string(), "0x000A".to_string(), "".to_string(), "".to_string()],
    ];
    let mut cursor_row = 0;
    let mut cursor_col = 0;
    let mut cell_cursor = 0;
    let mut editor_scroll = 0usize;
    let mut mem_scroll: u16 = 0;
    let mut breakpoints: HashSet<usize> = HashSet::new();
    let mut asm = AsmResult::empty();
    let mut dirty = true;
    let mut running = false;
    let mut speed: u32 = 5;
    let mut status = String::from("普通模式: hjkl 移动, i 编辑, :q 退出");
    let mut input_mode = InputMode::None;
    let mut input_buffer = String::new();
    let mut vim_mode = VimMode::Normal;
    let mut command_buffer = String::new();
    let mut current_file: Option<String> = None;
    let mut suppress_text_input = false;
    let mut mouse_x = 0;
    let mut mouse_y = 0;
    let mut last_tick = Instant::now();

    asm = assemble_table(&table);
    load_into_cpu(&mut cpu, &asm);

    'outer: loop {
        let (win_w, win_h) = canvas.output_size()?;
        let line_h = font.height() + 4;

        let left_w = win_w / 2;
        let title_h: u32 = line_h as u32;
        let header_h: u32 = line_h as u32;
        let status_h: u32 = line_h as u32;
        let editor_y = title_h + header_h;
        let editor_h = win_h.saturating_sub(title_h + header_h + status_h);
        let editor_rect = Rect::new(0, editor_y as i32, left_w, editor_h);
        let status_rect = Rect::new(0, (win_h - status_h) as i32, left_w, status_h);

        let right_x = left_w as i32;
        let right_w = win_w - left_w;
        let reg_h: u32 = 140;
        let reg_rect = Rect::new(right_x, 0, right_w, reg_h);
        let mem_rect = Rect::new(
            right_x,
            reg_h as i32 + line_h as i32,
            right_w,
            win_h.saturating_sub(reg_h + line_h as u32),
        );

        let col_widths = [
            left_w * 12 / 100,
            left_w * 15 / 100,
            left_w * 15 / 100,
            left_w * 15 / 100,
            left_w * 10 / 100,
            left_w * 11 / 100,
            left_w * 11 / 100,
            left_w * 11 / 100,
        ];
        let mut col_x = [0i32; 9];
        let mut x_acc = 0i32;
        for i in 0..8 {
            col_x[i] = x_acc;
            x_acc += col_widths[i] as i32;
        }
        col_x[8] = left_w as i32;

        let now = Instant::now();
        let dt = now.duration_since(last_tick).as_secs_f64();
        last_tick = now;

        if running && !cpu.halted && asm.errors.is_empty() {
            let target_ips = 80.0 * (speed as f64).powi(2);
            let mut steps = (target_ips * dt).round() as u32;
            if steps == 0 && dt > 0.0 {
                steps = 1;
            }
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
                Event::TextInput { text, .. } => {
                    if suppress_text_input {
                        suppress_text_input = false;
                    } else if input_mode != InputMode::None {
                        input_buffer.push_str(&text);
                    } else if vim_mode == VimMode::Command {
                        for ch in text.chars() {
                            if ch == ':' && command_buffer.is_empty() {
                                continue;
                            }
                            command_buffer.push(ch);
                        }
                    } else if vim_mode == VimMode::Insert {
                        let cell = &mut table[cursor_row][cursor_col];
                        cell.insert_str(cell_cursor, &text);
                        cell_cursor += text.len();
                        dirty = true;
                    }
                }
                Event::KeyDown { keycode: Some(key), keymod, .. } => {
                    if input_mode != InputMode::None {
                        match key {
                            Keycode::Escape => {
                                input_mode = InputMode::None;
                                input_buffer.clear();
                                status = "已取消".to_string();
                            }
                            Keycode::Return => {
                                let mode = input_mode;
                                input_mode = InputMode::None;
                                match mode {
                                    InputMode::Open => match load_table(&input_buffer) {
                                        Ok(t) => {
                                            table = t;
                                            cursor_row = 0;
                                            cursor_col = 0;
                                            cell_cursor = 0;
                                            dirty = true;
                                            current_file = Some(input_buffer.clone());
                                            status = format!("已打开 {}", input_buffer);
                                        }
                                        Err(e) => status = format!("打开失败: {}", e),
                                    },
                                    InputMode::Save => match save_table(&input_buffer, &table) {
                                        Ok(_) => {
                                            current_file = Some(input_buffer.clone());
                                            dirty = false;
                                            status = format!("已保存 {}", input_buffer);
                                        }
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
                                    asm = assemble_table(&table);
                                    dirty = false;
                                }
                                if !asm.errors.is_empty() {
                                    status = format!("汇编错误: {}", asm.errors[0]);
                                    running = false;
                                } else {
                                    if cpu.halted {
                                        load_into_cpu(&mut cpu, &asm);
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
                                    asm = assemble_table(&table);
                                    dirty = false;
                                }
                                running = false;
                                if asm.errors.is_empty() && !cpu.halted {
                                    cpu.step();
                                    status = format!("单步 PC=0x{:04X}", cpu.pc);
                                } else if cpu.halted {
                                    status = "已停机".to_string();
                                } else {
                                    status = format!(
                                        "汇编错误: {}",
                                        asm.errors.first().cloned().unwrap_or_default()
                                    );
                                }
                            }
                            Keycode::F7 => {
                                asm = assemble_table(&table);
                                load_into_cpu(&mut cpu, &asm);
                                running = false;
                                status = "复位".to_string();
                            }
                            Keycode::F8 => {
                                running = false;
                                input_mode = InputMode::Export;
                                input_buffer = "program.hex".to_string();
                            }
                            Keycode::F9 => {
                                if breakpoints.contains(&cursor_row) {
                                    breakpoints.remove(&cursor_row);
                                } else {
                                    breakpoints.insert(cursor_row);
                                }
                                status = format!("断点行 {}", cursor_row + 1);
                            }
                            Keycode::O if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => {
                                running = false;
                                input_mode = InputMode::Open;
                                input_buffer = current_file
                                    .clone()
                                    .unwrap_or_else(|| "program.tab".to_string());
                            }
                            Keycode::S if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => {
                                running = false;
                                input_mode = InputMode::Save;
                                input_buffer = current_file
                                    .clone()
                                    .unwrap_or_else(|| "program.tab".to_string());
                            }
                            _ => match vim_mode {
                                VimMode::Command => match key {
                                    Keycode::Escape => {
                                        command_buffer.clear();
                                        vim_mode = VimMode::Normal;
                                        status = "普通模式".to_string();
                                    }
                                    Keycode::Return => {
                                        let cmd = command_buffer
                                            .trim()
                                            .trim_start_matches(':')
                                            .to_string();
                                        command_buffer.clear();
                                        vim_mode = VimMode::Normal;
                                        if cmd == "q" || cmd == "q!" || cmd == "quit" {
                                            break 'outer;
                                        } else if cmd == "w" {
                                            if let Some(path) = &current_file {
                                                match save_table(path, &table) {
                                                    Ok(_) => {
                                                        dirty = false;
                                                        status = format!("已保存 {}", path);
                                                    }
                                                    Err(e) => status = format!("保存失败: {}", e),
                                                }
                                            } else {
                                                status = "没有文件名，使用 :w <file>".to_string();
                                            }
                                        } else if let Some(path) = cmd.strip_prefix("w ") {
                                            let path = path.trim();
                                            if path.is_empty() {
                                                status = "用法: :w <file>".to_string();
                                            } else {
                                                match save_table(path, &table) {
                                                    Ok(_) => {
                                                        current_file = Some(path.to_string());
                                                        dirty = false;
                                                        status = format!("已保存 {}", path);
                                                    }
                                                    Err(e) => status = format!("保存失败: {}", e),
                                                }
                                            }
                                        } else if cmd == "wq" || cmd == "x" {
                                            if let Some(path) = &current_file {
                                                match save_table(path, &table) {
                                                    Ok(_) => break 'outer,
                                                    Err(e) => status = format!("保存失败: {}", e),
                                                }
                                            } else {
                                                status = "没有文件名，使用 :w <file>".to_string();
                                            }
                                        } else if cmd == "e" {
                                            status = "用法: :e <file>".to_string();
                                        } else if let Some(path) = cmd.strip_prefix("e ") {
                                            let path = path.trim();
                                            if path.is_empty() {
                                                status = "用法: :e <file>".to_string();
                                            } else {
                                                match load_table(path) {
                                                    Ok(t) => {
                                                        table = t;
                                                        cursor_row = 0;
                                                        cursor_col = 0;
                                                        cell_cursor = 0;
                                                        dirty = true;
                                                        current_file = Some(path.to_string());
                                                        status = format!("已打开 {}", path);
                                                    }
                                                    Err(e) => status = format!("打开失败: {}", e),
                                                }
                                            }
                                        } else {
                                            status = format!("未知命令: {}", cmd);
                                        }
                                    }
                                    Keycode::Backspace => {
                                        command_buffer.pop();
                                    }
                                    _ => {}
                                },
                                VimMode::Insert => match key {
                                    Keycode::Escape => {
                                        normalize_cell(
                                            cursor_col,
                                            &mut table[cursor_row][cursor_col],
                                        );
                                        vim_mode = VimMode::Normal;
                                        status = "普通模式".to_string();
                                    }
                                    Keycode::Backspace => {
                                        if cell_cursor > 0 {
                                            let prev = table[cursor_row][cursor_col][..cell_cursor]
                                                .char_indices()
                                                .last()
                                                .map(|(i, _)| i)
                                                .unwrap_or(0);
                                            table[cursor_row][cursor_col].remove(prev);
                                            cell_cursor = prev;
                                            dirty = true;
                                        }
                                    }
                                    Keycode::Delete => {
                                        let cell = &mut table[cursor_row][cursor_col];
                                        if cell_cursor < cell.len() {
                                            cell.remove(cell_cursor);
                                            dirty = true;
                                        }
                                    }
                                    Keycode::Left => {
                                        if cell_cursor > 0 {
                                            cell_cursor = table[cursor_row][cursor_col]
                                                [..cell_cursor]
                                                .char_indices()
                                                .last()
                                                .map(|(i, _)| i)
                                                .unwrap_or(0);
                                        }
                                    }
                                    Keycode::Right => {
                                        let cell = &table[cursor_row][cursor_col];
                                        if cell_cursor < cell.len() {
                                            if let Some(ch) = cell[cell_cursor..].chars().next() {
                                                cell_cursor += ch.len_utf8();
                                            }
                                        }
                                    }
                                    Keycode::Up => {
                                        if cursor_row > 0 {
                                            normalize_cell(
                                                cursor_col,
                                                &mut table[cursor_row][cursor_col],
                                            );
                                            cursor_row -= 1;
                                            cell_cursor = table[cursor_row][cursor_col].len();
                                        }
                                    }
                                    Keycode::Down => {
                                        if cursor_row + 1 < table.len() {
                                            normalize_cell(
                                                cursor_col,
                                                &mut table[cursor_row][cursor_col],
                                            );
                                            cursor_row += 1;
                                            cell_cursor = table[cursor_row][cursor_col].len();
                                        }
                                    }
                                    Keycode::Home => {
                                        cell_cursor = 0;
                                    }
                                    Keycode::End => {
                                        cell_cursor = table[cursor_row][cursor_col].len();
                                    }
                                    Keycode::Return => {
                                        normalize_cell(
                                            cursor_col,
                                            &mut table[cursor_row][cursor_col],
                                        );
                                        if cursor_row + 1 < table.len() {
                                            cursor_row += 1;
                                        } else {
                                            table.push([
                                                "".to_string(),
                                                "".to_string(),
                                                "".to_string(),
                                                "".to_string(),
                                            ]);
                                            cursor_row = table.len() - 1;
                                        }
                                        cell_cursor = table[cursor_row][cursor_col].len();
                                        dirty = true;
                                    }
                                    Keycode::Tab => {
                                        normalize_cell(
                                            cursor_col,
                                            &mut table[cursor_row][cursor_col],
                                        );
                                        if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                                            if cursor_col > 0 {
                                                cursor_col -= 1;
                                            } else if cursor_row > 0 {
                                                cursor_row -= 1;
                                                cursor_col = 3;
                                            }
                                        } else {
                                            if cursor_col < 3 {
                                                cursor_col += 1;
                                            } else if cursor_row + 1 < table.len() {
                                                cursor_row += 1;
                                                cursor_col = 0;
                                            } else {
                                                table.push([
                                                    "".to_string(),
                                                    "".to_string(),
                                                    "".to_string(),
                                                    "".to_string(),
                                                ]);
                                                cursor_row = table.len() - 1;
                                                cursor_col = 0;
                                            }
                                        }
                                        cell_cursor = table[cursor_row][cursor_col].len();
                                        dirty = true;
                                    }
                                    _ => {}
                                },
                                VimMode::Normal => match key {
                                    Keycode::Escape => {
                                        status = "普通模式".to_string();
                                    }
                                    Keycode::H => {
                                        if cursor_col > 0 {
                                            cursor_col -= 1;
                                            cell_cursor = table[cursor_row][cursor_col].len();
                                        }
                                    }
                                    Keycode::L => {
                                        if cursor_col < 3 {
                                            cursor_col += 1;
                                            cell_cursor = table[cursor_row][cursor_col].len();
                                        }
                                    }
                                    Keycode::J => {
                                        if cursor_row + 1 < table.len() {
                                            cursor_row += 1;
                                            cell_cursor = table[cursor_row][cursor_col].len();
                                        }
                                    }
                                    Keycode::K => {
                                        if cursor_row > 0 {
                                            cursor_row -= 1;
                                            cell_cursor = table[cursor_row][cursor_col].len();
                                        }
                                    }
                                    Keycode::I => {
                                        vim_mode = VimMode::Insert;
                                        cell_cursor = 0;
                                        suppress_text_input = true;
                                        status = "编辑模式".to_string();
                                    }
                                    Keycode::A => {
                                        vim_mode = VimMode::Insert;
                                        cell_cursor = table[cursor_row][cursor_col].len();
                                        suppress_text_input = true;
                                        status = "编辑模式".to_string();
                                    }
                                    Keycode::O => {
                                        if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                                            table.insert(
                                                cursor_row,
                                                [
                                                    "".to_string(),
                                                    "".to_string(),
                                                    "".to_string(),
                                                    "".to_string(),
                                                ],
                                            );
                                            cell_cursor = 0;
                                        } else {
                                            if cursor_row + 1 < table.len() {
                                                table.insert(
                                                    cursor_row + 1,
                                                    [
                                                        "".to_string(),
                                                        "".to_string(),
                                                        "".to_string(),
                                                        "".to_string(),
                                                    ],
                                                );
                                                cursor_row += 1;
                                            } else {
                                                table.push([
                                                    "".to_string(),
                                                    "".to_string(),
                                                    "".to_string(),
                                                    "".to_string(),
                                                ]);
                                                cursor_row = table.len() - 1;
                                            }
                                            cell_cursor = 0;
                                        }
                                        cursor_col = 0;
                                        dirty = true;
                                        vim_mode = VimMode::Insert;
                                        suppress_text_input = true;
                                        status = "编辑模式".to_string();
                                    }
                                    Keycode::X => {
                                        table[cursor_row][cursor_col].clear();
                                        dirty = true;
                                    }
                                    Keycode::Semicolon | Keycode::Colon => {
                                        vim_mode = VimMode::Command;
                                        command_buffer.clear();
                                        status = "命令模式".to_string();
                                    }
                                    Keycode::Plus | Keycode::Equals => {
                                        speed = (speed + 1).min(10);
                                        status = format!("速度 {}", speed);
                                    }
                                    Keycode::Minus => {
                                        speed = speed.saturating_sub(1).max(1);
                                        status = format!("速度 {}", speed);
                                    }
                                    Keycode::Return => {
                                        if cursor_row + 1 < table.len() {
                                            cursor_row += 1;
                                            cell_cursor = table[cursor_row][cursor_col].len();
                                        }
                                    }
                                    Keycode::Tab => {
                                        if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                                            if cursor_col > 0 {
                                                cursor_col -= 1;
                                            } else if cursor_row > 0 {
                                                cursor_row -= 1;
                                                cursor_col = 3;
                                            }
                                        } else {
                                            if cursor_col < 3 {
                                                cursor_col += 1;
                                            } else if cursor_row + 1 < table.len() {
                                                cursor_row += 1;
                                                cursor_col = 0;
                                            }
                                        }
                                        cell_cursor = table[cursor_row][cursor_col].len();
                                    }
                                    _ => {}
                                },
                            },
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
                    } else if mouse_x >= right_x
                        && mouse_y >= mem_rect.y()
                        && mouse_y < mem_rect.y() + mem_rect.height() as i32
                    {
                        if y > 0 {
                            mem_scroll = mem_scroll.wrapping_sub(16);
                        } else if y < 0 {
                            mem_scroll = mem_scroll.wrapping_add(16);
                        }
                    }
                }
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    ..
                } => {
                    if x < left_w as i32
                        && y >= editor_rect.y()
                        && y < editor_rect.y() + editor_rect.height() as i32
                    {
                        let rel_y = y - editor_rect.y() - header_h as i32;
                        if rel_y >= 0 {
                            let row = editor_scroll + (rel_y / line_h) as usize;
                            if row < table.len() {
                                let rel_x = x;
                                let mut col = 0;
                                for i in 0..8 {
                                    if rel_x >= col_x[i] && rel_x < col_x[i + 1] {
                                        col = i;
                                        break;
                                    }
                                }
                                if col < 4 {
                                    cursor_row = row;
                                    cursor_col = col;
                                    cell_cursor = table[cursor_row][cursor_col].len();
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if dirty {
            asm = assemble_table(&table);
        }

        canvas.set_draw_color(Color::RGB(0, 0, 170));
        canvas.clear();

        canvas.set_draw_color(Color::RGB(0, 0, 110));
        canvas.fill_rect(Rect::new(0, 0, left_w, title_h))?;
        draw_text(
            &mut canvas,
            &texture_creator,
            &font,
            &format!("LMC(扩展版) 模拟器 - {}模式", vim_mode.name()),
            10,
            6,
            Color::RGB(255, 255, 0),
        )?;

        canvas.set_draw_color(Color::RGB(0, 0, 140));
        canvas.fill_rect(Rect::new(0, title_h as i32, left_w, header_h))?;
        let headers = [
            "符号",
            "指令/数据",
            "操作数0",
            "操作数1",
            "地址",
            "机器码",
            "操作数0码",
            "操作数1码",
        ];
        for i in 0..8 {
            draw_text(
                &mut canvas,
                &texture_creator,
                &font,
                headers[i],
                col_x[i] + 4,
                title_h as i32 + 4,
                Color::RGB(0, 255, 255),
            )?;
        }

        let visible_rows = (editor_h as i32 / line_h).max(1) as usize;
        let start_row = editor_scroll;
        let end_row = (start_row + visible_rows).min(table.len());
        let text_y_start = editor_rect.y() + 4;
        for row_idx in start_row..end_row {
            let y = text_y_start + ((row_idx - start_row) as i32) * line_h;
            if row_idx == cursor_row && vim_mode != VimMode::Insert {
                canvas.set_draw_color(Color::RGB(0, 0, 80));
                canvas.fill_rect(Rect::new(0, y, left_w, line_h as u32))?;
            }
            if breakpoints.contains(&row_idx) {
                canvas.set_draw_color(Color::RGB(255, 0, 0));
                canvas.fill_rect(Rect::new(5, y + line_h / 2 - 4, 8, 8))?;
            }
            for col in 0..8 {
                let x = col_x[col] + 4;
                let cell_rect = Rect::new(
                    col_x[col],
                    y - 2,
                    (col_x[col + 1] - col_x[col]) as u32,
                    line_h as u32,
                );
                if col >= 4 {
                    let err_idx = col - 4;
                    if asm.rows.get(row_idx).map_or(false, |r| r.error[err_idx]) {
                        canvas.set_draw_color(Color::RGB(255, 0, 0));
                        canvas.fill_rect(cell_rect)?;
                    }
                }
                if row_idx == cursor_row && col == cursor_col && vim_mode == VimMode::Insert {
                    canvas.set_draw_color(Color::RGB(0, 0, 120));
                    canvas.fill_rect(cell_rect)?;
                }
                let display = if col < 4 {
                    table[row_idx][col].clone()
                } else {
                    if let Some(info) = asm.rows.get(row_idx) {
                        let err_idx = col - 4;
                        if info.error[err_idx] {
                            "?".to_string()
                        } else {
                            match col {
                                4 => info
                                    .address
                                    .map(|a| format!("0x{:04X}", a))
                                    .unwrap_or_default(),
                                5 => info.words[0]
                                    .map(|w| format!("0x{:04X}", w))
                                    .unwrap_or_default(),
                                6 => info.words[1]
                                    .map(|w| format!("0x{:04X}", w))
                                    .unwrap_or_default(),
                                7 => info.words[2]
                                    .map(|w| format!("0x{:04X}", w))
                                    .unwrap_or_default(),
                                _ => String::new(),
                            }
                        }
                    } else {
                        String::new()
                    }
                };
                let text_color = if col >= 4 {
                    if asm
                        .rows
                        .get(row_idx)
                        .map_or(false, |r| r.error[col - 4])
                    {
                        Color::RGB(255, 255, 255)
                    } else {
                        Color::RGB(255, 255, 0)
                    }
                } else {
                    Color::RGB(255, 255, 0)
                };
                draw_text(
                    &mut canvas,
                    &texture_creator,
                    &font,
                    &display,
                    x,
                    y,
                    text_color,
                )?;
            }
            if row_idx == cursor_row && vim_mode == VimMode::Insert {
                let cell = &table[cursor_row][cursor_col];
                let prefix = &cell[..cell_cursor];
                let prefix_w = font.size_of(prefix).map(|(w, _)| w).unwrap_or(0) as i32;
                let cursor_x = col_x[cursor_col] + 4 + prefix_w;
                canvas.set_draw_color(Color::RGB(255, 255, 255));
                canvas.fill_rect(Rect::new(cursor_x, y, 2, font.height() as u32))?;
            }
        }

        let title_y = reg_rect.y() + 6;
        draw_text(
            &mut canvas,
            &texture_creator,
            &font,
            "寄存器",
            reg_rect.x() + 8,
            title_y,
            Color::RGB(0, 255, 255),
        )?;
        let data_y = title_y + line_h;
        let col_w = reg_rect.width() as i32 / 2;
        let left_regs = [
            format!("PC    = 0x{:04X} ({})", cpu.pc, cpu.pc),
            format!("ACC   = 0x{:04X} ({})", cpu.acc, cpu.acc as i16),
            format!("Z     = {}", cpu.z),
            format!("N     = {}", cpu.n),
        ];
        let right_regs = [
            format!("HALT  = {}", cpu.halted),
            format!("CYCLE = {}", cpu.cycles),
            format!("SP    = 0x{:04X} ({})", cpu.sp(), cpu.sp()),
            format!("FP    = 0x{:04X} ({})", cpu.fp(), cpu.fp()),
        ];
        for i in 0..4 {
            let y = data_y + (i as i32) * line_h;
            draw_text(
                &mut canvas,
                &texture_creator,
                &font,
                &left_regs[i],
                reg_rect.x() + 8,
                y,
                Color::RGB(255, 255, 0),
            )?;
            draw_text(
                &mut canvas,
                &texture_creator,
                &font,
                &right_regs[i],
                reg_rect.x() + col_w,
                y,
                Color::RGB(255, 255, 0),
            )?;
        }

        draw_text(
            &mut canvas,
            &texture_creator,
            &font,
            "内存 (十六进制)",
            mem_rect.x() + 8,
            mem_rect.y() + 4,
            Color::RGB(0, 255, 255),
        )?;
        let mem_visible = ((mem_rect.height() as i32 - 30) / line_h).max(1) as usize;
        let start = mem_scroll & !0xF;
        let mem_left = mem_rect.x() + 8;
        let mem_right = mem_rect.x() + mem_rect.width() as i32 - MEM_RIGHT_MARGIN;
        let addr_text_w = char_w * 5;
        let data_w = (mem_right - (mem_left + addr_text_w)).max(0);
        let digit_w = font
            .size_of("0000")
            .map(|(w, _)| w as i32)
            .unwrap_or(char_w * 4);
        let gap = if data_w > 16 * digit_w {
            ((data_w - 16 * digit_w) / 15 - MEM_GAP_TRIM).max(0)
        } else {
            0
        };
        let digit_step = digit_w + gap;
        let mut my = mem_rect.y() + 30;
        for row in 0..mem_visible {
            let addr = start.wrapping_add((row as u16) * 16);
            draw_text(
                &mut canvas,
                &texture_creator,
                &font,
                &format!("{:04X}:", addr),
                mem_left,
                my,
                Color::RGB(255, 255, 0),
            )?;
            for col in 0..16 {
                let a = addr.wrapping_add(col);
                let x = mem_left + addr_text_w + (col as i32) * digit_step;
                draw_text(
                    &mut canvas,
                    &texture_creator,
                    &font,
                    &format!("{:04X}", cpu.mem[a as usize]),
                    x,
                    my,
                    Color::RGB(255, 255, 0),
                )?;
            }
            my += line_h;
        }

        canvas.set_draw_color(Color::RGB(0, 0, 110));
        canvas.fill_rect(status_rect)?;
        let io = cpu.io_log.last().map(|s| s.as_str()).unwrap_or("");
        if input_mode != InputMode::None {
            let prompt = match input_mode {
                InputMode::Open => "打开文件路径: ",
                InputMode::Save => "保存文件路径: ",
                InputMode::Export => "导出机器码路径: ",
                InputMode::None => "",
            };
            draw_text(
                &mut canvas,
                &texture_creator,
                &font,
                &format!("{}{}_", prompt, input_buffer),
                8,
                status_rect.y() + 4,
                Color::RGB(255, 255, 255),
            )?;
        } else if vim_mode == VimMode::Command {
            draw_text(
                &mut canvas,
                &texture_creator,
                &font,
                &format!(":{}_", command_buffer),
                8,
                status_rect.y() + 4,
                Color::RGB(255, 255, 255),
            )?;
        } else {
            let run_state = if running {
                "运行"
            } else if cpu.halted {
                "停机"
            } else {
                "暂停"
            };
            let hint = if io.is_empty() {
                status.clone()
            } else {
                format!("{} | {}", status, io)
            };
            let merged = format!(
                "{}模式 | {} | 速度 {} | {}",
                vim_mode.name(),
                run_state,
                speed,
                hint
            );
            draw_text(
                &mut canvas,
                &texture_creator,
                &font,
                &merged,
                8,
                status_rect.y() + 4,
                Color::RGB(255, 255, 0),
            )?;
        }

        canvas.present();
        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}

fn normalize_cell(col: usize, cell: &mut String) {
    let s = cell.trim().to_string();
    if s.is_empty() {
        *cell = String::new();
        return;
    }
    let mut normalized = match col {
        0 => s.to_lowercase(),
        1 => {
            let upper = s.to_uppercase();
            if instr_info(&upper).is_some() {
                upper
            } else if s.contains("0x") || s.contains("0X") {
                s.to_uppercase()
            } else {
                s.to_lowercase()
            }
        }
        2 | 3 => {
            if s.contains("0x") || s.contains("0X") {
                s.to_uppercase()
            } else {
                s.to_lowercase()
            }
        }
        _ => s,
    };
    normalized = pad_hex_in_string(&normalized);
    *cell = normalized;
}

fn pad_hex_in_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '0' && chars.peek() == Some(&'x') {
            result.push('0');
            result.push('x');
            chars.next();
            let mut hex = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_hexdigit() {
                    hex.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            if hex.len() < 4 {
                result.push_str(&format!("{:0>4}", hex));
            } else {
                result.push_str(&hex);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn save_table(path: &str, table: &[[String; 4]]) -> Result<(), String> {
    let mut out = String::new();
    for row in table {
        out.push_str(&row.join("\t"));
        out.push('\n');
    }
    fs::write(path, out).map_err(|e| e.to_string())
}

fn load_table(path: &str) -> Result<Vec<[String; 4]>, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut table = Vec::new();
    for line in content.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        let mut row = [
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
        ];
        for i in 0..4 {
            if i < parts.len() {
                row[i] = parts[i].to_string();
            }
        }
        table.push(row);
    }
    Ok(table)
}
