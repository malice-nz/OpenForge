use anyhow::{bail, Context, Result};
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, RgbaImage};
use std::io::Cursor;
use std::path::Path;

pub const ICO_SIZES: &[u32] = &[16, 20, 24, 32, 40, 48, 64, 96, 128, 256];
pub const PNG_SQUARE_SIZES: &[(u32, &str)] = &[
    (256, "assets/images/icon.png"),
    (256, "assets/images/icon_gray.png"),
    (256, "assets/images/taskbar_icon.png"),
];

pub struct PreparedIcon {
    pub ico_bytes: Vec<u8>,
    pub png_256: Vec<u8>,
}

pub fn prepare(src_png: &Path) -> Result<PreparedIcon> {
    let bytes = std::fs::read(src_png)
        .with_context(|| format!("read icon source {}", src_png.display()))?;
    let img = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .context("decode source PNG")?;
    let rgba = img.to_rgba8();

    let mut ico_dir = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in ICO_SIZES {
        let resized = resize_square(&rgba, size);
        let mut png = Vec::new();
        DynamicImage::ImageRgba8(resized)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .context("encode icon PNG frame")?;
        let entry =
            ico::IconImage::read_png(Cursor::new(&png)).context("re-read PNG for ico crate")?;
        ico_dir.add_entry(ico::IconDirEntry::encode(&entry).context("encode ico entry")?);
    }
    let mut ico_buf = Vec::new();
    ico_dir.write(&mut ico_buf).context("write ICO container")?;

    let png_256_img = resize_square(&rgba, 256);
    let mut png_256 = Vec::new();
    DynamicImage::ImageRgba8(png_256_img)
        .write_to(&mut Cursor::new(&mut png_256), ImageFormat::Png)
        .context("encode 256 PNG")?;

    Ok(PreparedIcon {
        ico_bytes: ico_buf,
        png_256,
    })
}

fn resize_square(src: &RgbaImage, size: u32) -> RgbaImage {
    image::imageops::resize(src, size, size, FilterType::Lanczos3)
}

pub fn replace_asar_icons(app_root: &Path, prep: &PreparedIcon) -> Result<Vec<String>> {
    let mut hits = Vec::new();
    let ico_path = app_root.join("assets/images/desktop_icon.ico");
    if ico_path.is_file() {
        std::fs::write(&ico_path, &prep.ico_bytes)
            .with_context(|| format!("write {}", ico_path.display()))?;
        hits.push("assets/images/desktop_icon.ico".into());
    }
    for &(_size, rel) in PNG_SQUARE_SIZES {
        let p = app_root.join(rel);
        if p.is_file() {
            std::fs::write(&p, &prep.png_256).with_context(|| format!("write {}", p.display()))?;
            hits.push(rel.into());
        }
    }
    let icns = app_root.join("assets/images/icon.icns");
    if icns.is_file() {
        let _ = std::fs::write(&icns, &prep.png_256);
    }
    Ok(hits)
}

#[cfg(windows)]
pub fn replace_pe_icons(exe_path: &Path, src_png: &Path) -> Result<usize> {
    use std::io::{Cursor as IoCursor, Read, Seek, SeekFrom};

    let prep = prepare(src_png)?;
    let mut cur = IoCursor::new(&prep.ico_bytes);
    let mut hdr = [0u8; 6];
    cur.read_exact(&mut hdr)?;
    if hdr[0] != 0 || hdr[1] != 0 || hdr[2] != 1 || hdr[3] != 0 {
        bail!("generated ICO has bad header");
    }
    let count = u16::from_le_bytes([hdr[4], hdr[5]]);
    if count == 0 {
        bail!("generated ICO has 0 frames");
    }

    let mut entries: Vec<IconEntry> = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut e = [0u8; 16];
        cur.read_exact(&mut e)?;
        let width = e[0];
        let height = e[1];
        let colors = e[2];
        let reserved = e[3];
        let planes = u16::from_le_bytes([e[4], e[5]]);
        let bitcount = u16::from_le_bytes([e[6], e[7]]);
        let size = u32::from_le_bytes([e[8], e[9], e[10], e[11]]);
        let offset = u32::from_le_bytes([e[12], e[13], e[14], e[15]]);
        entries.push(IconEntry {
            width,
            height,
            colors,
            reserved,
            planes,
            bitcount,
            size,
            offset,
            data: Vec::new(),
        });
    }
    for ent in entries.iter_mut() {
        cur.seek(SeekFrom::Start(ent.offset as u64))?;
        let mut buf = vec![0u8; ent.size as usize];
        cur.read_exact(&mut buf)?;
        ent.data = buf;
    }

    let n = pe_write_icons(exe_path, &entries)?;
    Ok(n)
}

#[cfg(not(windows))]
pub fn replace_pe_icons(_exe_path: &Path, _src_png: &Path) -> Result<usize> {
    Ok(0)
}

struct IconEntry {
    width: u8,
    height: u8,
    colors: u8,
    reserved: u8,
    planes: u16,
    bitcount: u16,
    size: u32,
    offset: u32,
    data: Vec<u8>,
}

#[cfg(windows)]
fn pe_write_icons(exe_path: &Path, entries: &[IconEntry]) -> Result<usize> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{FreeLibrary, GetLastError, BOOL, FALSE, TRUE};
    use windows_sys::Win32::System::LibraryLoader::{
        BeginUpdateResourceW, EndUpdateResourceW, EnumResourceNamesW, FindResourceW,
        LoadLibraryExW, LoadResource, LockResource, SizeofResource, UpdateResourceW,
        LOAD_LIBRARY_AS_DATAFILE,
    };

    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;

    let wide: Vec<u16> = OsStr::new(exe_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut existing_group_ids: Vec<u32> = Vec::new();
    let mut existing_icon_ids: Vec<u32> = Vec::new();

    unsafe {
        let h = LoadLibraryExW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_AS_DATAFILE,
        );
        if h.is_null() {
            bail!("LoadLibraryExW failed (code {})", GetLastError());
        }

        extern "system" fn collect_cb(
            _h: *mut core::ffi::c_void,
            _ty: *const u16,
            name: *const u16,
            param: isize,
        ) -> BOOL {
            let v = unsafe { &mut *(param as *mut Vec<u32>) };
            let n = name as usize;
            if n & 0xFFFF0000 == 0 {
                v.push(n as u32);
            }
            TRUE
        }

        let ok = EnumResourceNamesW(
            h,
            RT_GROUP_ICON as usize as *const u16,
            Some(collect_cb),
            &mut existing_group_ids as *mut _ as isize,
        );
        if ok == 0 {
            FreeLibrary(h);
            bail!("EnumResourceNamesW RT_GROUP_ICON failed");
        }

        for &gid in &existing_group_ids {
            let hres = FindResourceW(
                h,
                gid as usize as *const u16,
                RT_GROUP_ICON as usize as *const u16,
            );
            if hres.is_null() {
                continue;
            }
            let sz = SizeofResource(h, hres);
            let hg = LoadResource(h, hres);
            if hg.is_null() || sz < 6 {
                continue;
            }
            let p = LockResource(hg) as *const u8;
            if p.is_null() {
                continue;
            }
            let slice = std::slice::from_raw_parts(p, sz as usize);
            let n = u16::from_le_bytes([slice[4], slice[5]]) as usize;
            for i in 0..n {
                let off = 6 + i * 14;
                if off + 14 > slice.len() {
                    break;
                }
                let id = u16::from_le_bytes([slice[off + 12], slice[off + 13]]) as u32;
                if !existing_icon_ids.contains(&id) {
                    existing_icon_ids.push(id);
                }
            }
        }
        FreeLibrary(h);
    }

    let mut new_icon_ids: Vec<u32> = Vec::with_capacity(entries.len());
    let mut next_id: u32 = 1;
    for i in 0..entries.len() {
        if let Some(&id) = existing_icon_ids.get(i) {
            new_icon_ids.push(id);
        } else {
            while existing_icon_ids.contains(&next_id) || new_icon_ids.contains(&next_id) {
                next_id += 1;
            }
            new_icon_ids.push(next_id);
            next_id += 1;
        }
    }

    let mut grp = Vec::with_capacity(6 + entries.len() * 14);
    grp.extend_from_slice(&0u16.to_le_bytes());
    grp.extend_from_slice(&1u16.to_le_bytes());
    grp.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for (ent, &id) in entries.iter().zip(new_icon_ids.iter()) {
        grp.push(ent.width);
        grp.push(ent.height);
        grp.push(ent.colors);
        grp.push(ent.reserved);
        grp.extend_from_slice(&ent.planes.to_le_bytes());
        grp.extend_from_slice(&ent.bitcount.to_le_bytes());
        grp.extend_from_slice(&ent.size.to_le_bytes());
        grp.extend_from_slice(&(id as u16).to_le_bytes());
    }

    unsafe {
        let upd = BeginUpdateResourceW(wide.as_ptr(), FALSE);
        if upd.is_null() {
            bail!("BeginUpdateResourceW failed (code {})", GetLastError());
        }

        for (ent, &id) in entries.iter().zip(new_icon_ids.iter()) {
            let r = UpdateResourceW(
                upd,
                RT_ICON as usize as *const u16,
                id as usize as *const u16,
                0,
                ent.data.as_ptr() as *const _,
                ent.data.len() as u32,
            );
            if r == 0 {
                let err = GetLastError();
                let _ = EndUpdateResourceW(upd, TRUE);
                bail!("UpdateResourceW (write icon {}) failed (err {})", id, err);
            }
        }

        let group_id: u32 = if existing_group_ids.is_empty() {
            1
        } else {
            *existing_group_ids.iter().min().unwrap()
        };
        let r = UpdateResourceW(
            upd,
            RT_GROUP_ICON as usize as *const u16,
            group_id as usize as *const u16,
            0,
            grp.as_ptr() as *const _,
            grp.len() as u32,
        );
        if r == 0 {
            let err = GetLastError();
            let _ = EndUpdateResourceW(upd, TRUE);
            bail!(
                "UpdateResourceW (write group {}) failed (err {})",
                group_id,
                err
            );
        }

        let r = EndUpdateResourceW(upd, FALSE);
        if r == 0 {
            bail!("EndUpdateResourceW failed (code {})", GetLastError());
        }
    }

    Ok(entries.len())
}
