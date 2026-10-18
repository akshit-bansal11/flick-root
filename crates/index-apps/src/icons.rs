//! Icon extraction to PNG and an on-disk icon cache. Background-only: slow, needs COM, never call on the keystroke path.

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
    GetObjectW, HBITMAP, HGDIOBJ, ReleaseDC,
};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
};
use windows::core::HSTRING;

use crate::{AppEntry, AppsError};

fn icon_err(what: &str, e: impl std::fmt::Display) -> AppsError {
    AppsError::Icon(format!("{what}: {e}"))
}

/// Render the app icon at `size` x `size` and return PNG bytes. Needs COM on the calling thread.
pub fn extract_icon(entry: &AppEntry, size: u32) -> Result<Vec<u8>, AppsError> {
    let path = HSTRING::from(format!("shell:AppsFolder\\{}", entry.parsing_name));
    // SAFETY: `path` is a valid NUL-terminated HSTRING for the call; COM is initialised by the caller.
    let factory: IShellItemImageFactory = unsafe { SHCreateItemFromParsingName(&path, None) }
        .map_err(|e| icon_err("create item", e))?;
    let s = i32::try_from(size).map_err(|e| icon_err("size", e))?;
    // SAFETY: `factory` is a live interface; the returned HBITMAP is owned by us and freed below.
    let hbm =
        unsafe { factory.GetImage(SIZE { cx: s, cy: s }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK) }
            .map_err(|e| icon_err("get image", e))?;
    let res = bitmap_to_png(hbm);
    // SAFETY: `hbm` is a valid GDI bitmap we own and no longer use.
    let _ = unsafe { DeleteObject(HGDIOBJ(hbm.0)) };
    res
}

fn bitmap_to_png(hbm: HBITMAP) -> Result<Vec<u8>, AppsError> {
    let mut bm = BITMAP::default();
    // SAFETY: `bm` is a BITMAP and the size passed matches it.
    let got = unsafe {
        GetObjectW(
            HGDIOBJ(hbm.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&mut bm as *mut BITMAP).cast()),
        )
    };
    if got == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 {
        return Err(AppsError::Icon("bad bitmap".into()));
    }
    let (w, h) = (bm.bmWidth as u32, bm.bmHeight as u32);
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32), // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut px = vec![0u8; (w * h * 4) as usize];
    // SAFETY: a screen DC is requested and released below; `px` holds exactly w*h*4 bytes
    // matching the 32bpp header, and `bmi` is initialised.
    let lines = unsafe {
        let dc = GetDC(None);
        let n = GetDIBits(
            dc,
            hbm,
            0,
            h,
            Some(px.as_mut_ptr().cast()),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, dc);
        n
    };
    if lines == 0 {
        return Err(AppsError::Icon("GetDIBits failed".into()));
    }
    let opaque = px.as_chunks::<4>().0.iter().all(|p| p[3] == 0);
    for p in px.as_chunks_mut::<4>().0.iter_mut() {
        p.swap(0, 2); // BGRA -> RGBA
        if opaque {
            p[3] = 255;
        }
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| icon_err("png header", e))?;
    wr.write_image_data(&px)
        .map_err(|e| icon_err("png data", e))?;
    wr.finish().map_err(|e| icon_err("png finish", e))?;
    Ok(out)
}

/// Sizes written by [`IconCache::ensure`].
pub const ICON_SIZES: [u32; 2] = [32, 64];

/// PNG files at `<cache>/icons/<id>-<size>.png`.
pub struct IconCache {
    dir: PathBuf,
}

impl IconCache {
    /// Cache rooted at `<cache_dir>/icons`.
    pub fn new(cache_dir: &Path) -> Self {
        Self {
            dir: cache_dir.join("icons"),
        }
    }

    /// Path of an existing cached icon.
    pub fn icon_path(&self, entry: &AppEntry, size: u32) -> Option<PathBuf> {
        let p = self.dir.join(format!("{}-{size}.png", entry.id));
        p.is_file().then_some(p)
    }

    /// Extract and write the 32 and 64 px icons, skipping files that exist. Needs COM on the thread.
    pub fn ensure(&self, entry: &AppEntry) -> Result<(), AppsError> {
        std::fs::create_dir_all(&self.dir).map_err(AppsError::Cache)?;
        for size in ICON_SIZES {
            if self.icon_path(entry, size).is_some() {
                continue;
            }
            let png = extract_icon(entry, size)?;
            let tmp = self.dir.join(format!("{}-{size}.png.tmp", entry.id));
            std::fs::write(&tmp, png).map_err(AppsError::Cache)?;
            std::fs::rename(&tmp, self.dir.join(format!("{}-{size}.png", entry.id)))
                .map_err(AppsError::Cache)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppIndex;

    #[test]
    #[ignore = "uses the shell"]
    fn live_calculator_icons() {
        let _com = crate::com::ComGuard::new().unwrap();
        let idx = AppIndex::new();
        idx.refresh_blocking().unwrap();
        let e = idx
            .entries()
            .iter()
            .find(|a| a.name == "Calculator")
            .cloned()
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let c = IconCache::new(dir.path());
        c.ensure(&e).unwrap();
        for s in ICON_SIZES {
            let p = c.icon_path(&e, s).unwrap();
            let b = std::fs::read(p).unwrap();
            println!("icon {s}px: {} bytes", b.len());
            assert_eq!(&b[..8], b"\x89PNG\r\n\x1a\n");
        }
    }
}
