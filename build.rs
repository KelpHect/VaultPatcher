//! Embeds the Windows resources: version info and the app icon, which is
//! rendered from `assets/brand/*.svg` at build time (no binary icon in git).

fn main() {
    println!("cargo:rerun-if-changed=assets/brand/logo.svg");
    println!("cargo:rerun-if-changed=assets/brand/logo-small.svg");
    #[cfg(windows)]
    {
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
        let icon = out.join("vault-patcher.ico");
        std::fs::write(&icon, build_ico()).expect("write icon");
        let mut res = winresource::WindowsResource::new();
        res.set("ProductName", "Vault Patcher");
        res.set("FileDescription", "Vault Patcher - Borderlands patcher and mod installer");
        res.set("LegalCopyright", "GPL-3.0-or-later");
        res.set_icon(icon.to_str().expect("utf-8 path"));
        res.compile().expect("compile Windows resources");
    }
}

/// A multi-size .ico with PNG entries: the simplified mark for small sizes,
/// the detailed one from 48px up.
#[cfg(windows)]
fn build_ico() -> Vec<u8> {
    let full = std::fs::read("assets/brand/logo.svg").expect("logo.svg");
    let small = std::fs::read("assets/brand/logo-small.svg").expect("logo-small.svg");
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

#[cfg(windows)]
fn render(svg: &[u8], size: u32) -> Vec<u8> {
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default()).expect("valid svg");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("pixmap");
    let scale = size as f32 / tree.size().width();
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap.encode_png().expect("png")
}
