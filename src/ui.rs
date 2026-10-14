use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::ttf::Font;
use sdl2::video::{Window, WindowContext};

/// 按优先级寻找可用字体。
///
/// UI 文案是中文，如果选用不含中文字形的西文等宽字体（DejaVuSansMono、
/// LiberationMono、Consola、Lucida Console……），SDL2_ttf 只能画出“豆腐块”，
/// 表现为乱码。因此这里先尝试各种系统中文字体，都没有时才回退到西文等宽字体。
/// 列表顺序：等宽且含中文 -> 含中文但可能非等宽 -> 只有西文。
pub fn find_font() -> Option<String> {
    const CANDIDATES: &[&str] = &[
        // ==== Linux：等宽且含中文（最优先） ====
        "/usr/share/fonts/opentype/noto/NotoSansMonoCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansMonoCJKsc-Regular.otf",
        "/usr/share/fonts/truetype/noto/NotoSansMonoCJK-Regular.ttc",
        // ==== Linux：含中文，但可能不是等宽 ====
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc",
        // 文泉驿
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        // Droid / Arphic
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        "/usr/share/fonts/truetype/arphic/uming.ttc",
        "/usr/share/fonts/truetype/arphic/ukai.ttc",
        // ==== macOS ====
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        // ==== Windows ====
        "C:\\Windows\\Fonts\\msyh.ttc",   // 微软雅黑
        "C:\\Windows\\Fonts\\msyh.ttf",
        "C:\\Windows\\Fonts\\msyhbd.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",  // 黑体
        "C:\\Windows\\Fonts\\simsun.ttc",  // 宋体
        "C:\\Windows\\Fonts\\Deng.ttf",    // 等线
        // ==== 最后的回退：西文等宽（不含中文，仅保证界面能跑） ====
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
        "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
        "C:\\Windows\\Fonts\\consola.ttf",
        "C:\\Windows\\Fonts\\lucon.ttf",
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
    let surface = font.render(text).blended(color).map_err(|e| e.to_string())?;
    let texture = tc.create_texture_from_surface(&surface).map_err(|e| e.to_string())?;
    let query = texture.query();
    canvas.copy(&texture, None, Some(Rect::new(x, y, query.width, query.height)))?;
    Ok(())
}
