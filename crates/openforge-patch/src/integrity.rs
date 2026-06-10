use anyhow::{bail, Context, Result};
use std::path::Path;

pub fn patch_exe_hash(exe_path: &Path, old_hex: &str, new_hex: &str) -> Result<usize> {
    if old_hex.len() != 64 || new_hex.len() != 64 {
        bail!(
            "expected 64-char sha256 hex; got old={} new={}",
            old_hex.len(),
            new_hex.len()
        );
    }
    if !old_hex.chars().all(|c| c.is_ascii_hexdigit())
        || !new_hex.chars().all(|c| c.is_ascii_hexdigit())
    {
        bail!("hash strings must be ascii hex");
    }
    if old_hex.eq_ignore_ascii_case(new_hex) {
        return Ok(0);
    }

    let mut data =
        std::fs::read(exe_path).with_context(|| format!("read {}", exe_path.display()))?;
    let old_l = old_hex.to_ascii_lowercase();
    let old_u = old_hex.to_ascii_uppercase();
    let new_bytes_l = new_hex.to_ascii_lowercase();
    let new_bytes_u = new_hex.to_ascii_uppercase();

    let mut total = 0usize;
    for (needle, replacement) in [
        (old_l.as_bytes(), new_bytes_l.as_bytes()),
        (old_u.as_bytes(), new_bytes_u.as_bytes()),
    ] {
        total += in_place_replace(&mut data, needle, replacement);
    }

    if total == 0 {
        bail!("hash '{}' not found in {}", old_hex, exe_path.display());
    }

    std::fs::write(exe_path, &data)
        .with_context(|| format!("write back {}", exe_path.display()))?;
    Ok(total)
}

fn in_place_replace(buf: &mut [u8], needle: &[u8], replacement: &[u8]) -> usize {
    if needle.len() != replacement.len() {
        return 0;
    }
    if needle.is_empty() || buf.len() < needle.len() {
        return 0;
    }
    let mut count = 0usize;
    let mut i = 0usize;
    let last = buf.len() - needle.len();
    while i <= last {
        if buf[i] == needle[0] && &buf[i..i + needle.len()] == needle {
            buf[i..i + needle.len()].copy_from_slice(replacement);
            count += 1;
            i += needle.len();
        } else {
            i += 1;
        }
    }
    count
}
