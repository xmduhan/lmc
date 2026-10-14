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

use assembler::{assemble_and_load, AsmResult};
use cpu::Cpu;
use editor::{cursor_line_col, line_end_byte, line_start_byte, set_cursor_from_line_col};
use ui::{draw_text, find_font, SPACE_WIDTH};

/// 界面渲染字号。
const FONT_SIZE: u16 = 18;

/// 内存观察器最右一列到内存区右边界的留白（像素）。
const MEM_RIGHT_MARGIN: i32 = 8;

/// 内存观察器列间距的额外收窄量（像素）。
///
/// 0 表示最后一列右边缘正好对齐到内存区右边界（不越界）。
/// 想让它再往左收，就把它调大；每加 1，最后一列左移 15 像素。
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

    assemble_and_load(&mut cpu, &editor_text, &mut asm, &mut status, &mut dirty);

    'outer: loop {
        let (win_w, win_h) = canvas.output_size()?;
        let line_h = font.height() + 4;

        // 左侧面板布局：标题行、快捷键行、代码编辑区、底部状态/输入行
        let left_w = win_w / 2;
        let title_h: u32 = line_h as u32;
        let shortcut_h: u32 = line_h as u32;
        let status_h: u32 = line_h as u32;
        let editor_y = title_h + shortcut_h;
        let editor_h = win_h.saturating_sub(title_h + shortcut_h + status_h);
        let editor_rect = Rect::new(0, editor_y as i32, left_w, editor_h);
        let status_rect = Rect::new(0, (win_h - status_h) as i32, left_w, status_h);

        // 右侧面板布局：寄存器区、内存区
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
                        editor_text.insert_str(editor_cursor, &text);
                        editor_cursor += text.len();
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
                                    InputMode::Open => match fs::read_to_string(&input_buffer) {
                                        Ok(s) => {
                                            editor_text = s;
                                            editor_cursor = editor_text.len();
                                            dirty = true;
                                            current_file = Some(input_buffer.clone());
                                            status = format!("已打开 {}", input_buffer);
                                        }
                                        Err(e) => status = format!("打开失败: {}", e),
                                    },
                                    InputMode::Save => match fs::write(&input_buffer, &editor_text) {
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
                                input_buffer = current_file.clone().unwrap_or_else(|| "program.asm".to_string());
                            }
                            Keycode::S if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => {
                                running = false;
                                input_mode = InputMode::Save;
                                input_buffer = current_file.clone().unwrap_or_else(|| "program.asm".to_string());
                            }
                            _ => match vim_mode {
                                VimMode::Command => {
                                    match key {
                                        Keycode::Escape => {
                                            command_buffer.clear();
                                            vim_mode = VimMode::Normal;
                                            status = "普通模式".to_string();
                                        }
                                        Keycode::Return => {
                                            let cmd = command_buffer.trim().trim_start_matches(':').to_string();
                                            command_buffer.clear();
                                            vim_mode = VimMode::Normal;
                                            if cmd == "q" || cmd == "q!" || cmd == "quit" {
                                                break 'outer;
                                            } else if cmd == "w" {
                                                if let Some(path) = &current_file {
                                                    match fs::write(path, &editor_text) {
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
                                                    match fs::write(path, &editor_text) {
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
                                                    match fs::write(path, &editor_text) {
                                                        Ok(_) => {
                                                            break 'outer;
                                                        }
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
                                                    match fs::read_to_string(path) {
                                                        Ok(s) => {
                                                            editor_text = s;
                                                            editor_cursor = editor_text.len();
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
                                    }
                                }
                                VimMode::Insert => {
                                    match key {
                                        Keycode::Escape => {
                                            vim_mode = VimMode::Normal;
                                            status = "普通模式".to_string();
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
                                        _ => {}
                                    }
                                }
                                VimMode::Normal => {
                                    match key {
                                        Keycode::Escape => {
                                            status = "普通模式".to_string();
                                        }
                                        Keycode::H => {
                                            if editor_cursor > 0 {
                                                editor_cursor = editor_text[..editor_cursor]
                                                    .char_indices()
                                                    .last()
                                                    .map(|(i, _)| i)
                                                    .unwrap_or(0);
                                            }
                                        }
                                        Keycode::L => {
                                            if editor_cursor < editor_text.len() {
                                                if let Some(ch) = editor_text[editor_cursor..].chars().next() {
                                                    editor_cursor += ch.len_utf8();
                                                }
                                            }
                                        }
                                        Keycode::J => {
                                            let (line, col) = cursor_line_col(&editor_text, editor_cursor);
                                            let total_lines = editor_text.split('\n').count();
                                            if line + 1 < total_lines {
                                                set_cursor_from_line_col(&editor_text, &mut editor_cursor, line + 1, col);
                                            }
                                        }
                                        Keycode::K => {
                                            let (line, col) = cursor_line_col(&editor_text, editor_cursor);
                                            if line > 0 {
                                                set_cursor_from_line_col(&editor_text, &mut editor_cursor, line - 1, col);
                                            }
                                        }
                                        Keycode::I => {
                                            if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                                                let (line, _) = cursor_line_col(&editor_text, editor_cursor);
                                                set_cursor_from_line_col(&editor_text, &mut editor_cursor, line, 0);
                                            }
                                            vim_mode = VimMode::Insert;
                                            suppress_text_input = true;
                                            status = "编辑模式".to_string();
                                        }
                                        Keycode::A => {
                                            if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                                                let (line, _) = cursor_line_col(&editor_text, editor_cursor);
                                                let end = line_end_byte(&editor_text, line);
                                                editor_cursor = end;
                                            } else if editor_cursor < editor_text.len() {
                                                if let Some(ch) = editor_text[editor_cursor..].chars().next() {
                                                    editor_cursor += ch.len_utf8();
                                                }
                                            }
                                            vim_mode = VimMode::Insert;
                                            suppress_text_input = true;
                                            status = "编辑模式".to_string();
                                        }
                                        Keycode::O => {
                                            if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                                                let (line, _) = cursor_line_col(&editor_text, editor_cursor);
                                                let start = line_start_byte(&editor_text, line);
                                                editor_text.insert(start, '\n');
                                                editor_cursor = start;
                                            } else {
                                                let (line, _) = cursor_line_col(&editor_text, editor_cursor);
                                                let end = line_end_byte(&editor_text, line);
                                                editor_text.insert(end, '\n');
                                                editor_cursor = end + 1;
                                            }
                                            dirty = true;
                                            vim_mode = VimMode::Insert;
                                            suppress_text_input = true;
                                            status = "编辑模式".to_string();
                                        }
                                        Keycode::X => {
                                            if editor_cursor < editor_text.len() {
                                                editor_text.remove(editor_cursor);
                                                dirty = true;
                                            }
                                        }
                                        Keycode::Num0 => {
                                            let (line, _) = cursor_line_col(&editor_text, editor_cursor);
                                            set_cursor_from_line_col(&editor_text, &mut editor_cursor, line, 0);
                                        }
                                        Keycode::Dollar => {
                                            let (line, _) = cursor_line_col(&editor_text, editor_cursor);
                                            let end = line_end_byte(&editor_text, line);
                                            editor_cursor = end;
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
                                            let (line, col) = cursor_line_col(&editor_text, editor_cursor);
                                            set_cursor_from_line_col(&editor_text, &mut editor_cursor, line + 1, col);
                                        }
                                        _ => {}
                                    }
                                }
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

        // 左侧标题行
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

        // 左侧快捷键行
        canvas.fill_rect(Rect::new(0, title_h as i32, left_w, shortcut_h))?;
        let shortcut_text = "Vim: hjkl移动 i插入 a追加 o/O新行 x删除 :命令 | :q退出 | F5运行 F6单步 F7复位 F8导出 F9断点";
        draw_text(&mut canvas, &texture_creator, &font, shortcut_text, 10, title_h as i32 + 6, Color::RGB(255, 255, 0))?;

        // 编辑器背景
        canvas.set_draw_color(Color::RGB(0, 0, 140));
        canvas.fill_rect(editor_rect)?;
        canvas.set_draw_color(Color::RGB(0, 0, 110));
        canvas.fill_rect(Rect::new(editor_rect.x(), editor_rect.y(), 60, editor_rect.height()))?;

        // 右侧面板
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
        let cursor_line = editor_text[..editor_cursor].chars().filter(|&c| c == '\n').count();
        if cursor_line < editor_scroll {
            editor_scroll = cursor_line;
        } else if cursor_line >= editor_scroll + visible_lines {
            editor_scroll = cursor_line.saturating_sub(visible_lines - 1);
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

        let title_y = reg_rect.y() + 6;
        draw_text(&mut canvas, &texture_creator, &font, "寄存器", reg_rect.x() + 8, title_y, Color::RGB(0, 255, 255))?;
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
            draw_text(&mut canvas, &texture_creator, &font, &left_regs[i], reg_rect.x() + 8, y, Color::RGB(255, 255, 0))?;
            draw_text(&mut canvas, &texture_creator, &font, &right_regs[i], reg_rect.x() + col_w, y, Color::RGB(255, 255, 0))?;
        }

        draw_text(&mut canvas, &texture_creator, &font, "内存 (十六进制)", mem_rect.x() + 8, mem_rect.y() + 4, Color::RGB(0, 255, 255))?;
        let mem_visible = ((mem_rect.height() as i32 - 30) / line_h).max(1) as usize;
        let start = mem_scroll & !0xF;
        let mem_left = mem_rect.x() + 8;
        let mem_right = mem_rect.x() + mem_rect.width() as i32 - MEM_RIGHT_MARGIN;
        let addr_text_w = char_w * 5;
        let data_w = (mem_right - (mem_left + addr_text_w)).max(0);
        // 用字体实际渲染宽度来算数字宽度，避免用 char_w * 4 估算导致最右一列越界。
        let digit_w = font
            .size_of("0000")
            .map(|(w, _)| w as i32)
            .unwrap_or(char_w * 4);
        // 一行 16 个数字，之间有 15 个间隙。
        // 这样最后一列的右边缘恰好落在 mem_right 上，无需再手动减 N 像素。
        let gap = if data_w > 16 * digit_w {
            ((data_w - 16 * digit_w) / 15 - MEM_GAP_TRIM).max(0)
        } else {
            0
        };
        let digit_step = digit_w + gap;
        let mut my = mem_rect.y() + 30;
        for row in 0..mem_visible {
            let addr = start.wrapping_add((row as u16) * 16);
            draw_text(&mut canvas, &texture_creator, &font, &format!("{:04X}:", addr), mem_left, my, Color::RGB(255, 255, 0))?;
            for col in 0..16 {
                let a = addr.wrapping_add(col);
                let x = mem_left + addr_text_w + (col as i32) * digit_step;
                draw_text(&mut canvas, &texture_creator, &font, &format!("{:04X}", cpu.mem[a as usize]), x, my, Color::RGB(255, 255, 0))?;
            }
            my += line_h;
        }

        // 底部状态行：只负责提示 / 输入，运行状态栏已移到编辑器左下角
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
            let hint = if io.is_empty() {
                status.clone()
            } else {
                format!("{} | {}", status, io)
            };
            draw_text(
                &mut canvas,
                &texture_creator,
                &font,
                &hint,
                8,
                status_rect.y() + 4,
                Color::RGB(255, 255, 0),
            )?;
        }

        // 运行状态栏：绘制在编辑器区域的左下角，和右侧内存观察器保持距离，
        // 避免像之前那样贴到左右面板分界线（甚至向右溢出）时压到内存观察器。
        let run_state = if running {
            "运行"
        } else if cpu.halted {
            "停机"
        } else {
            "暂停"
        };
        let run_text = format!("{}模式 | {} | 速度 {}", vim_mode.name(), run_state, speed);
        let (rtw, rth) = font.size_of(&run_text).map_err(|e| e.to_string())?;
        // draw_text 对含 " | " 的文本会按固定空格宽度渲染，这里按同一规则估算真实绘制宽度，
        // 否则背景框会比文字窄，或者文字会溢出到右侧。
        let native_space_w = font.size_of(" ").map(|(w, _)| w as i32).unwrap_or(0);
        let space_count = run_text.chars().filter(|&ch| ch == ' ').count() as i32;
        let run_w = rtw as i32 + space_count * (SPACE_WIDTH - native_space_w);
        let margin = 6;
        let rx = editor_rect.x() + margin;
        let ry = (editor_rect.y() + editor_rect.height() as i32 - rth as i32 - margin + 2)
            .max(editor_rect.y());
        canvas.set_draw_color(Color::RGB(0, 0, 80));
        canvas.fill_rect(Rect::new(rx - 4, ry - 2, run_w.max(0) as u32 + 8, rth + 4))?;
        draw_text(
            &mut canvas,
            &texture_creator,
            &font,
            &run_text,
            rx,
            ry,
            Color::RGB(255, 255, 0),
        )?;

        canvas.present();
        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}
