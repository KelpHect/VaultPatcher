//! Grabs a frame from another process's window with `PrintWindow`
//! (`PW_RENDERFULLCONTENT`), which also captures DirectX/Vulkan content of a
//! windowed or borderless game. Used by the comparison capture so shots never
//! go through (and clutter) Steam's screenshot library.

use anyhow::{Result, bail};

#[cfg(windows)]
pub fn capture_process_window(pid: u32) -> Result<image::RgbImage> {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
    };
    use windows_sys::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetClientRect, GetWindowThreadProcessId, IsWindowVisible};

    struct Search {
        pid: u32,
        best: HWND,
        area: i64,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam is the &mut Search passed to EnumWindows below.
        let search = unsafe { &mut *(lparam as *mut Search) };
        let mut owner = 0u32;
        // SAFETY: plain Win32 queries on a window handle EnumWindows gave us.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut owner);
            if owner == search.pid && IsWindowVisible(hwnd) != 0 {
                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);
                let area = i64::from(rect.right - rect.left) * i64::from(rect.bottom - rect.top);
                if area > search.area {
                    search.area = area;
                    search.best = hwnd;
                }
            }
        }
        1
    }

    let mut search = Search { pid, best: std::ptr::null_mut(), area: 0 };
    // SAFETY: the callback only touches `search`, which outlives the call.
    unsafe { EnumWindows(Some(visit), &mut search as *mut Search as LPARAM) };
    if search.best.is_null() {
        bail!("game window not found");
    }

    // SAFETY: standard GDI capture sequence; every handle is released below.
    unsafe {
        let hwnd = search.best;
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rect);
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
        if w <= 0 || h <= 0 {
            bail!("game window has no size");
        }
        let screen = GetDC(std::ptr::null_mut());
        let mem = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bitmap);
        // PW_CLIENTONLY (1) | PW_RENDERFULLCONTENT (2)
        let ok = PrintWindow(hwnd, mem, 3 as PRINT_WINDOW_FLAGS);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        let lines = GetDIBits(mem, bitmap, 0, h as u32, bgra.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);
        SelectObject(mem, old);
        DeleteObject(bitmap);
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);
        if ok == 0 || lines == 0 {
            bail!("couldn't read the game window");
        }
        let rgb: Vec<u8> = bgra.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0]]).collect();
        let img = image::RgbImage::from_raw(w as u32, h as u32, rgb).expect("buffer matches size");
        // An all-black frame means the window couldn't be read (e.g. exclusive fullscreen).
        if img.pixels().all(|p| p.0 == [0, 0, 0]) {
            bail!("captured an empty frame");
        }
        Ok(img)
    }
}

#[cfg(not(windows))]
pub fn capture_process_window(_pid: u32) -> Result<image::RgbImage> {
    bail!("window capture is only supported on Windows")
}
