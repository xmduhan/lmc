use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::ttf::Font;
use sdl2::video::{Window, WindowContext};

pub fn find_font() -> Option<String> {
    let paths = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
        "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
        "C:\\Windows\\Fonts\\consola.ttf",
        "C:\\Windows\\Fonts\\lucon.ttf",
    ];
    paths.iter().find(|p| std::path::Path::new(p).exists()).map(|s| s.to_string())
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
    let surface = font.render(text).blended(color).map_err(|e| e.to_string())?;
    let texture = tc.create_texture_from_surface(&surface).map_err(|e| e.to_string())?;
    let query = texture.query();
    canvas.copy(&texture, None, Some(Rect::new(x, y, query.width, query.height)))?;
    Ok(())
}
