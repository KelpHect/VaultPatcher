//! Embeds the Windows resources: version info and the app icon, a multi-size
//! .ico scaled at build time from the logo renders in `assets/brand/`.

fn main() {
    println!("cargo:rerun-if-changed=assets/brand/logo.png");
    println!("cargo:rerun-if-changed=assets/brand/logo-small.png");
    // gpui-component's tree-sitter JSON grammar marks its entry point
    // dllexport, so MSVC would write an unused import library for the exe
    // (and report it on every build).
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/NOIMPLIB");
        println!("cargo:rustc-link-arg-bins=/NOEXP");
    }
    #[cfg(windows)]
    {
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
        let icon = out.join("vaulter.ico");
        std::fs::write(&icon, build_ico()).expect("write icon");
        let mut res = winresource::WindowsResource::new();
        res.set("ProductName", "Vaulter");
        res.set("FileDescription", "Vaulter - Borderlands patcher and mod installer");
        res.set("LegalCopyright", "GPL-3.0-or-later");
        res.set_icon(icon.to_str().expect("utf-8 path"));
        res.compile().expect("compile Windows resources");
    }
}

/// A multi-size .ico with PNG entries: the small-size render (heavier ink,
/// no hatching) up to 40px, the full one from 48px up.
#[cfg(windows)]
fn build_ico() -> Vec<u8> {
    let full = resvg::tiny_skia::Pixmap::decode_png(&std::fs::read("assets/brand/logo.png").expect("logo.png")).expect("logo.png is a PNG");
    let small = resvg::tiny_skia::Pixmap::decode_png(&std::fs::read("assets/brand/logo-small.png").expect("logo-small.png")).expect("logo-small.png is a PNG");
    let sizes = [16u32, 20, 24, 32, 40, 48, 64, 128, 256];
    let pngs: Vec<(u32, Vec<u8>)> = sizes
        .iter()
        .map(|&s| (s, render(if s < 48 { &small } else { &full }, s)))
        .collect();
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&(pngs.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * pngs.len() as u32;
    for (size, png) in &pngs {
        let dim = if *size >= 256 { 0 } else { *size as u8 };
        ico.extend_from_slice(&[dim, dim, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes()); // planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in &pngs {
        ico.extend_from_slice(png);
    }
    ico
}

/// `src` scaled to `size` square with bicubic filtering, as PNG bytes.
#[cfg(windows)]
fn render(src: &resvg::tiny_skia::Pixmap, size: u32) -> Vec<u8> {
    use resvg::tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};
    let mut out = Pixmap::new(size, size).expect("pixmap");
    let scale = size as f32 / src.width() as f32;
    let paint = PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() };
    out.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::from_scale(scale, scale), None);
    out.encode_png().expect("png")
}
