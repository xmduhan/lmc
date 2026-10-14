use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::ttf::Font;
use sdl2::video::{Window, WindowContext};

pub const SPACE_WIDTH: i32 = 10;

pub fn find_font() -> Option<String> {
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/opentype/noto/NotoSansMonoCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/simhei.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    ];
    CANDIDATES
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(|s| s.to_string())
}

pub fn draw_text(
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
    if text.contains(" | ") && text.len() < 200 {
        return draw_text_with_fixed_spaces(canvas, tc, font, text, x, y, color, SPACE_WIDTH);
    }
    let surface = font.render(text).blended(color).map_err(|e| e.to_string())?;
    let texture = tc.create_texture_from_surface(&surface).map_err(|e| e.to_string())?;
    let query = texture.query();
    canvas.copy(&texture, None, Some(Rect::new(x, y, query.width, query.height)))?;
    Ok(())
}

fn draw_text_with_fixed_spaces(
    canvas: &mut Canvas<Window>,
    tc: &TextureCreator<WindowContext>,
    font: &Font,
    text: &str,
    x: i32,
    y: i32,
    color: Color,
    space_width: i32,
) -> Result<(), String> {
    let mut draw_x = x;
    let mut segment = String::new();
    for ch in text.chars() {
        if ch == ' ' {
            if !segment.is_empty() {
                let surface = font.render(&segment).blended(color).map_err(|e| e.to_string())?;
                let texture = tc.create_texture_from_surface(&surface).map_err(|e| e.to_string())?;
                let query = texture.query();
                canvas.copy(&texture, None, Some(Rect::new(draw_x, y, query.width, query.height)))?;
                draw_x += query.width as i32;
                segment.clear();
            }
            draw_x += space_width;
        } else {
            segment.push(ch);
        }
    }
    if !segment.is_empty() {
        let surface = font.render(&segment).blended(color).map_err(|e| e.to_string())?;
        let texture = tc.create_texture_from_surface(&surface).map_err(|e| e.to_string())?;
        let query = texture.query();
        canvas.copy(&texture, None, Some(Rect::new(draw_x, y, query.width, query.height)))?;
    }
    Ok(())
}
