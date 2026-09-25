//! Game artwork, read from the user's own machine: Steam's library cache
//! (logo, hero banner, cover) and the game executable's icon. Nothing is
//! bundled; games without art fall back to text tiles.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct Art {
    /// Square icon, extracted from the exe (cached as PNG) or Steam's 32px icon.
    pub icon: Option<PathBuf>,
    /// Transparent title logo.
    pub logo: Option<PathBuf>,
    /// Wide 1920×620 banner.
    pub hero: Option<PathBuf>,
    /// Pre-blurred banner, for an ambient backdrop.
    pub hero_blur: Option<PathBuf>,
    /// 600×900 box art.
    pub cover: Option<PathBuf>,
}

pub fn find(game_id: &str, steam_app_ids: &[u32], exe: Option<&Path>) -> Art {
    let mut art = Art::default();
    if let Some(root) = super::detect::steam_root() {
        for id in steam_app_ids {
            let dir = root.join("appcache").join("librarycache").join(id.to_string());
            if !dir.is_dir() {
                continue;
            }
            art.logo = art.logo.or_else(|| find_named(&dir, "logo.png"));
            art.hero = art.hero.or_else(|| find_named(&dir, "library_hero.jpg"));
            art.hero_blur = art.hero_blur.or_else(|| find_named(&dir, "library_hero_blur.jpg"));
            art.cover = art
                .cover
                .or_else(|| find_named(&dir, "library_600x900.jpg"))
                .or_else(|| find_named(&dir, "library_capsule.jpg"));
            art.icon = art.icon.or_else(|| steam_icon(&dir));
        }
    }
    if let Some(exe) = exe
        && let Some(icon) = exe_icon(game_id, exe)
    {
        art.icon = Some(icon);
    }
    art
}

/// Steam stores art either flat (`<appid>/logo.png`) or, in newer clients, in
/// hashed subfolders (`<appid>/<hash>/logo.png`); the newest copy wins.
fn find_named(dir: &Path, name: &str) -> Option<PathBuf> {
    let nested = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path().join(name));
    std::iter::once(dir.join(name))
        .chain(nested)
        .filter_map(|p| Some((std::fs::metadata(&p).ok().filter(|m| m.is_file())?.modified().ok()?, p)))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// The client icon is the one `<sha1>.jpg` next to the named files.
fn steam_icon(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jpg"))
            && p.file_stem().is_some_and(|s| s.len() == 40 && s.to_string_lossy().chars().all(|c| c.is_ascii_hexdigit()))
    })
}

/// Extracts the exe's largest icon once and caches it as a PNG.
fn exe_icon(game_id: &str, exe: &Path) -> Option<PathBuf> {
    let out = super::backup::data_dir().join("art").join(format!("{game_id}-icon.png"));
    let fresh = |p: &Path| -> Option<bool> {
        Some(std::fs::metadata(p).ok()?.modified().ok()? >= std::fs::metadata(exe).ok()?.modified().ok()?)
    };
    if fresh(&out) == Some(true) {
        return Some(out);
    }
    let image = extract_icon(exe, 256)?;
    std::fs::create_dir_all(out.parent()?).ok()?;
    image.save(&out).ok()?;
    Some(out)
}

#[cfg(windows)]
fn extract_icon(exe: &Path, size: i32) -> Option<image::RgbaImage> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDIBits,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO, PrivateExtractIconsW};

    let mut wide: Vec<u16> = exe.as_os_str().encode_wide().collect();
    wide.resize(260, 0);
    let mut icon: HICON = std::ptr::null_mut();
    // SAFETY: `wide` is a MAX_PATH, NUL-terminated buffer; one icon handle is
    // written to `icon` and destroyed below.
    unsafe {
        let got = PrivateExtractIconsW(wide.as_ptr(), 0, size, size, &mut icon, std::ptr::null_mut(), 1, 0);
        if got == 0 || got == u32::MAX || icon.is_null() {
            return None;
        }
        let mut info: ICONINFO = std::mem::zeroed();
        let ok = GetIconInfo(icon, &mut info);
        let result = (|| {
            if ok == 0 || info.hbmColor.is_null() {
                return None;
            }
            let read = |bitmap, w: i32, h: i32| -> Option<Vec<u8>> {
                let dc = CreateCompatibleDC(std::ptr::null_mut());
                let mut bmi: BITMAPINFO = std::mem::zeroed();
                bmi.bmiHeader = BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..std::mem::zeroed()
                };
                let mut buf = vec![0u8; (w * h * 4) as usize];
                let lines = GetDIBits(dc, bitmap, 0, h as u32, buf.as_mut_ptr().cast(), &mut bmi, DIB_RGB_COLORS);
                DeleteDC(dc);
                (lines > 0).then_some(buf)
            };
            let color = read(info.hbmColor, size, size)?;
            let has_alpha = color.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
            // Old-style icons carry transparency in the AND mask instead.
            let mask = if has_alpha || info.hbmMask.is_null() { None } else { read(info.hbmMask, size, size) };
            let rgba: Vec<u8> = color
                .as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .flat_map(|(i, p)| {
                    let alpha = match &mask {
                        Some(m) => if m[i * 4] == 0 { 255 } else { 0 },
                        None => p[3],
                    };
                    [p[2], p[1], p[0], alpha]
                })
                .collect();
            image::RgbaImage::from_raw(size as u32, size as u32, rgba)
        })();
        if !info.hbmColor.is_null() {
            DeleteObject(info.hbmColor);
        }
        if !info.hbmMask.is_null() {
            DeleteObject(info.hbmMask);
        }
        DestroyIcon(icon);
        result
    }
}

#[cfg(not(windows))]
fn extract_icon(_exe: &Path, _size: i32) -> Option<image::RgbaImage> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_flat_and_hashed_steam_art() {
        let dir = std::env::temp_dir().join(format!("vp-art-{}", std::process::id()));
        let hashed = dir.join("0123abcd");
        std::fs::create_dir_all(&hashed).unwrap();
        std::fs::write(dir.join("library_hero.jpg"), b"x").unwrap();
        std::fs::write(hashed.join("logo.png"), b"x").unwrap();
        std::fs::write(dir.join(format!("{}.jpg", "a".repeat(40))), b"x").unwrap();
        assert_eq!(find_named(&dir, "library_hero.jpg"), Some(dir.join("library_hero.jpg")));
        assert_eq!(find_named(&dir, "logo.png"), Some(hashed.join("logo.png")));
        assert_eq!(find_named(&dir, "header.jpg"), None);
        assert!(steam_icon(&dir).is_some());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    #[ignore = "reads a real BL2 install"]
    fn live_extracts_exe_icon() {
        let exe = Path::new(r"D:\SteamLibrary\steamapps\common\Borderlands 2\Binaries\Win32\Borderlands2.exe");
        let img = extract_icon(exe, 256).expect("icon");
        assert!(img.pixels().any(|p| p.0[3] > 0));
        img.save(std::env::temp_dir().join("vp-bl2-icon.png")).unwrap();
    }
}
