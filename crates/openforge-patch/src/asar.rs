use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AsarEntry {
    Directory {
        files: BTreeMap<String, AsarEntry>,
    },
    File {
        size: u64,
        #[serde(default)]
        offset: Option<String>,
        #[serde(default)]
        executable: Option<bool>,
        #[serde(default)]
        unpacked: Option<bool>,
        #[serde(default)]
        integrity: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsarHeader {
    pub files: BTreeMap<String, AsarEntry>,
}

pub struct AsarReader {
    pub header: AsarHeader,
    pub data_offset: u64,
    pub file: File,
}

impl AsarReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut f = File::open(path.as_ref())
            .with_context(|| format!("open {}", path.as_ref().display()))?;
        let mut hdr = [0u8; 16];
        f.read_exact(&mut hdr).context("read 16-byte asar prefix")?;
        let size_of_size = u32::from_le_bytes(hdr[0..4].try_into()?);
        if size_of_size != 4 {
            bail!("unexpected asar size_of_size: {}", size_of_size);
        }
        let _pickle_payload = u32::from_le_bytes(hdr[4..8].try_into()?);
        let _aligned = u32::from_le_bytes(hdr[8..12].try_into()?);
        let json_size = u32::from_le_bytes(hdr[12..16].try_into()?) as u64;

        let mut json_buf = vec![0u8; json_size as usize];
        f.read_exact(&mut json_buf).context("read asar json")?;

        let json_padded = (json_size + 3) & !3u64;
        let data_offset = 16 + json_padded;

        let header: AsarHeader =
            serde_json::from_slice(&json_buf).context("parse asar header json")?;
        Ok(Self {
            header,
            data_offset,
            file: f,
        })
    }

    pub fn read_file(&mut self, offset_str: &str, size: u64) -> Result<Vec<u8>> {
        let off: u64 = offset_str
            .parse()
            .with_context(|| format!("parse offset '{}'", offset_str))?;
        let abs = self.data_offset + off;
        self.file.seek(SeekFrom::Start(abs))?;
        let mut buf = vec![0u8; size as usize];
        self.file.read_exact(&mut buf)?;
        Ok(buf)
    }
}

pub fn read_header_bytes<P: AsRef<Path>>(asar_path: P) -> Result<Vec<u8>> {
    let mut f = File::open(asar_path.as_ref())
        .with_context(|| format!("open {}", asar_path.as_ref().display()))?;
    let mut hdr = [0u8; 16];
    f.read_exact(&mut hdr)?;
    let size_of_size = u32::from_le_bytes(hdr[0..4].try_into()?);
    if size_of_size != 4 {
        bail!("unexpected asar size_of_size: {}", size_of_size);
    }
    let json_size = u32::from_le_bytes(hdr[12..16].try_into()?) as usize;
    let mut json_buf = vec![0u8; json_size];
    f.read_exact(&mut json_buf)?;
    Ok(json_buf)
}

pub fn hash_header_sha256<P: AsRef<Path>>(asar_path: P) -> Result<String> {
    use sha2::{Digest, Sha256};
    let bytes = read_header_bytes(asar_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

pub fn extract_all<P: AsRef<Path>>(asar_path: P, dest: P) -> Result<ExtractStats> {
    let mut reader = AsarReader::open(asar_path.as_ref())?;
    let dest = dest.as_ref().to_path_buf();
    std::fs::create_dir_all(&dest)?;
    let mut stats = ExtractStats::default();
    let header = reader.header.clone();
    walk_and_extract(&mut reader, &header.files, &dest, &mut stats)?;

    let header_json = serde_json::to_string_pretty(&header)?;
    std::fs::write(dest.join("__asar_header__.json"), header_json)?;
    Ok(stats)
}

#[derive(Debug, Default)]
pub struct ExtractStats {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub unpacked: u64,
}

fn walk_and_extract(
    reader: &mut AsarReader,
    entries: &BTreeMap<String, AsarEntry>,
    out_dir: &Path,
    stats: &mut ExtractStats,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    for (name, entry) in entries {
        let child = out_dir.join(name);
        match entry {
            AsarEntry::Directory { files } => {
                stats.dirs += 1;
                walk_and_extract(reader, files, &child, stats)?;
            }
            AsarEntry::File {
                size,
                offset,
                unpacked,
                ..
            } => {
                if unpacked.unwrap_or(false) || offset.is_none() {
                    stats.unpacked += 1;
                    continue;
                }
                let data = reader.read_file(offset.as_ref().unwrap(), *size)?;
                if let Some(parent) = child.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut f =
                    File::create(&child).with_context(|| format!("create {}", child.display()))?;
                f.write_all(&data)?;
                stats.files += 1;
                stats.bytes += *size;
            }
        }
    }
    Ok(())
}

pub fn list_all<P: AsRef<Path>>(asar_path: P) -> Result<Vec<(PathBuf, u64, bool)>> {
    let reader = AsarReader::open(asar_path.as_ref())?;
    let mut out = Vec::new();
    walk_list(&reader.header.files, PathBuf::new(), &mut out);
    Ok(out)
}

fn walk_list(
    entries: &BTreeMap<String, AsarEntry>,
    prefix: PathBuf,
    out: &mut Vec<(PathBuf, u64, bool)>,
) {
    for (name, entry) in entries {
        let p = prefix.join(name);
        match entry {
            AsarEntry::Directory { files } => walk_list(files, p, out),
            AsarEntry::File { size, unpacked, .. } => {
                out.push((p, *size, unpacked.unwrap_or(false)));
            }
        }
    }
}

#[allow(dead_code)]
pub fn ensure_exists<P: AsRef<Path>>(p: P) -> Result<PathBuf> {
    let p = p.as_ref();
    if !p.exists() {
        bail!("not found: {}", p.display());
    }
    Ok(p.to_path_buf())
}

#[allow(dead_code)]
pub fn cf_install_root() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    let p = PathBuf::from(local)
        .join("Programs")
        .join("CurseForge Windows");
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

#[allow(dead_code)]
pub fn cf_asar_path() -> Result<PathBuf> {
    let root = cf_install_root().ok_or_else(|| anyhow!("CurseForge install not found"))?;
    let p = root.join("resources").join("app.asar");
    if !p.exists() {
        bail!("app.asar not at {}", p.display());
    }
    Ok(p)
}

#[derive(Debug, Default)]
pub struct PackStats {
    pub files: u64,
    pub header_bytes: u64,
    pub blob_bytes: u64,
}

pub fn pack<P: AsRef<Path>>(extracted_dir: P, dst_asar: P) -> Result<PackStats> {
    let extracted = extracted_dir.as_ref();
    let dst = dst_asar.as_ref();

    let hdr_path = extracted.join("__asar_header__.json");
    let hdr_json = std::fs::read_to_string(&hdr_path)
        .with_context(|| format!("read header {}", hdr_path.display()))?;
    let mut header: AsarHeader =
        serde_json::from_str(&hdr_json).context("parse saved asar header")?;

    let mut next_offset: u64 = 0;
    let mut blobs: Vec<PathBuf> = Vec::new();
    assign_offsets(
        &mut header.files,
        extracted,
        &PathBuf::new(),
        &mut next_offset,
        &mut blobs,
    )?;

    let json = serde_json::to_string(&header).context("serialize new header")?;
    let json_bytes = json.as_bytes();
    let l = json_bytes.len() as u64;
    let pad = ((4 - (l % 4)) % 4) as u64;
    let header_payload_size: u32 = (4 + l + pad) as u32;
    let size_value: u32 = 4 + header_payload_size;

    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = File::create(dst).with_context(|| format!("create {}", dst.display()))?;
    out.write_all(&4u32.to_le_bytes())?;
    out.write_all(&size_value.to_le_bytes())?;
    out.write_all(&header_payload_size.to_le_bytes())?;
    out.write_all(&(l as u32).to_le_bytes())?;
    out.write_all(json_bytes)?;
    if pad > 0 {
        out.write_all(&vec![0u8; pad as usize])?;
    }

    let mut total_blob = 0u64;
    let mut buf = vec![0u8; 1024 * 1024];
    for src in &blobs {
        let mut f = File::open(src).with_context(|| format!("open blob {}", src.display()))?;
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            total_blob += n as u64;
        }
    }

    Ok(PackStats {
        files: blobs.len() as u64,
        header_bytes: 16 + l + pad,
        blob_bytes: total_blob,
    })
}

fn assign_offsets(
    entries: &mut BTreeMap<String, AsarEntry>,
    ext_root: &Path,
    prefix: &Path,
    next_offset: &mut u64,
    blobs: &mut Vec<PathBuf>,
) -> Result<()> {
    for (name, entry) in entries.iter_mut() {
        let rel = prefix.join(name);
        match entry {
            AsarEntry::Directory { files } => {
                assign_offsets(files, ext_root, &rel, next_offset, blobs)?;
            }
            AsarEntry::File {
                size,
                offset,
                unpacked,
                integrity,
                ..
            } => {
                if unpacked.unwrap_or(false) {
                    *offset = None;
                } else {
                    let disk = ext_root.join(&rel);
                    let (file_size, file_integrity) = stream_hash_file(&disk)
                        .with_context(|| format!("hash {}", disk.display()))?;
                    *size = file_size;
                    *offset = Some(next_offset.to_string());
                    *next_offset += file_size;
                    *integrity = Some(file_integrity);
                    blobs.push(disk);
                }
            }
        }
    }
    Ok(())
}

const INTEGRITY_BLOCK_SIZE: usize = 4 * 1024 * 1024;

fn stream_hash_file(path: &Path) -> Result<(u64, serde_json::Value)> {
    use sha2::{Digest, Sha256};
    let mut f = File::open(path)?;
    let mut whole = Sha256::new();
    let mut blocks: Vec<String> = Vec::new();
    let mut buf = vec![0u8; INTEGRITY_BLOCK_SIZE];
    let mut total: u64 = 0;
    loop {
        let mut filled = 0usize;
        while filled < buf.len() {
            let n = f.read(&mut buf[filled..])?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        if filled == 0 {
            break;
        }
        whole.update(&buf[..filled]);
        let mut blk = Sha256::new();
        blk.update(&buf[..filled]);
        blocks.push(hex::encode(blk.finalize()));
        total += filled as u64;
        if filled < buf.len() {
            break;
        }
    }
    if blocks.is_empty() {
        let mut blk = Sha256::new();
        blk.update(&[]);
        blocks.push(hex::encode(blk.finalize()));
    }
    let hash = hex::encode(whole.finalize());
    Ok((
        total,
        serde_json::json!({
            "algorithm": "SHA256",
            "hash": hash,
            "blockSize": INTEGRITY_BLOCK_SIZE,
            "blocks": blocks,
        }),
    ))
}
