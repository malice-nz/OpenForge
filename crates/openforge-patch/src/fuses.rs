use anyhow::{anyhow, bail, Context, Result};
use std::path::Path;

pub const SENTINEL: &[u8] = b"dL7pKGdnNz796PbbjQWNKmHXBZaB9tsX";

pub const FUSE_REMOVED: u8 = 0x30;
pub const FUSE_DISABLED: u8 = 0x31;
pub const FUSE_ENABLED: u8 = 0x32;

#[derive(Debug, Clone, Copy)]
pub enum Fuse {
    RunAsNode = 0,
    EnableCookieEncryption = 1,
    EnableNodeOptionsEnvVar = 2,
    EnableNodeCliInspectArguments = 3,
    EnableEmbeddedAsarIntegrityValidation = 4,
    OnlyLoadAppFromAsar = 5,
    LoadBrowserProcessSpecificV8Snapshot = 6,
    GrantFileProtocolExtraPrivileges = 7,
}

#[derive(Debug)]
pub struct FuseHeader {
    pub sentinel_offset: usize,
    pub version: u8,
    pub fuse_count: u8,
    pub fuses_offset: usize,
}

pub fn find_fuses(bin: &[u8]) -> Result<FuseHeader> {
    let sentinel_offset = memmem(bin, SENTINEL).ok_or_else(|| {
        anyhow!("electron fuse sentinel not found - not an Electron binary or unsupported version")
    })?;
    let after = sentinel_offset + SENTINEL.len();
    if after + 2 > bin.len() {
        bail!("binary truncated near fuse sentinel");
    }
    let version = bin[after];
    let fuse_count = bin[after + 1];
    let fuses_offset = after + 2;
    if fuses_offset + fuse_count as usize > bin.len() {
        bail!("binary truncated inside fuse wire");
    }
    Ok(FuseHeader {
        sentinel_offset,
        version,
        fuse_count,
        fuses_offset,
    })
}

pub fn read_fuses(bin: &[u8]) -> Result<(FuseHeader, Vec<u8>)> {
    let h = find_fuses(bin)?;
    let v = bin[h.fuses_offset..h.fuses_offset + h.fuse_count as usize].to_vec();
    Ok((h, v))
}

pub fn describe(fuse_idx: usize, value: u8) -> String {
    let name = match fuse_idx {
        0 => "RunAsNode",
        1 => "EnableCookieEncryption",
        2 => "EnableNodeOptionsEnvVar",
        3 => "EnableNodeCliInspectArguments",
        4 => "EnableEmbeddedAsarIntegrityValidation",
        5 => "OnlyLoadAppFromAsar",
        6 => "LoadBrowserProcessSpecificV8Snapshot",
        7 => "GrantFileProtocolExtraPrivileges",
        _ => "UNKNOWN",
    };
    let state = match value {
        FUSE_REMOVED => "removed",
        FUSE_DISABLED => "disabled",
        FUSE_ENABLED => "enabled",
        _ => "INVALID",
    };
    format!("[{}] {:38} = {} (0x{:02X})", fuse_idx, name, state, value)
}

pub fn print_fuses<P: AsRef<Path>>(bin_path: P) -> Result<()> {
    let bytes = std::fs::read(bin_path.as_ref())
        .with_context(|| format!("read {}", bin_path.as_ref().display()))?;
    let (h, fuses) = read_fuses(&bytes)?;
    println!("binary: {}", bin_path.as_ref().display());
    println!("sentinel offset : 0x{:08X}", h.sentinel_offset);
    println!("fuse version    : 0x{:02X}", h.version);
    println!("fuse count      : {}", h.fuse_count);
    println!("fuse offset     : 0x{:08X}", h.fuses_offset);
    for (i, b) in fuses.iter().enumerate() {
        println!("  {}", describe(i, *b));
    }
    Ok(())
}

pub fn flip_for_portable<P: AsRef<Path>>(bin_path: P) -> Result<Vec<(usize, u8, u8)>> {
    let path = bin_path.as_ref();
    let mut bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let h = find_fuses(&bytes)?;
    let mut changes = Vec::new();

    let targets: &[usize] = &[
        Fuse::EnableEmbeddedAsarIntegrityValidation as usize,
        Fuse::OnlyLoadAppFromAsar as usize,
    ];
    for &idx in targets {
        if idx >= h.fuse_count as usize {
            continue;
        }
        let off = h.fuses_offset + idx;
        let cur = bytes[off];
        if cur == FUSE_ENABLED {
            bytes[off] = FUSE_DISABLED;
            changes.push((idx, cur, FUSE_DISABLED));
        }
    }

    if !changes.is_empty() {
        std::fs::write(path, &bytes).with_context(|| format!("write back {}", path.display()))?;
    }
    Ok(changes)
}

fn memmem(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    let first = needle[0];
    let last_start = hay.len() - needle.len();
    for i in 0..=last_start {
        if hay[i] == first && &hay[i..i + needle.len()] == needle {
            return Some(i);
        }
    }
    None
}
