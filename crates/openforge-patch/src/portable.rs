use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::asar;
use crate::fuses;
use crate::icon;
use crate::integrity;
use crate::patch;

pub struct PortableBuild {
    pub install_root: PathBuf,
    pub out: PathBuf,
    pub force: bool,
    pub dry_run: bool,
    pub icon: Option<PathBuf>,
}

impl PortableBuild {
    pub fn run(&self) -> Result<()> {
        let quiet = std::env::var_os("OPENFORGE_INSTALLER_QUIET").is_some();
        macro_rules! qprintln {
            ($($arg:tt)*) => {
                if !quiet { println!($($arg)*); }
            };
        }
        qprintln!("=== OpenForge portable build ===");
        qprintln!("source : {}", self.install_root.display());
        qprintln!("dest   : {}", self.out.display());
        qprintln!("dry-run: {}", self.dry_run);
        qprintln!();

        if !self.install_root.exists() {
            bail!("source install not found: {}", self.install_root.display());
        }
        if self.out.exists() {
            let empty = std::fs::read_dir(&self.out)?.next().is_none();
            if !empty && !self.force {
                bail!(
                    "dest exists and is not empty: {} (pass --force to overwrite)",
                    self.out.display()
                );
            }
        } else {
            std::fs::create_dir_all(&self.out)?;
        }

        let asar_src = self.install_root.join("resources").join("app.asar");
        if !asar_src.is_file() {
            bail!("source app.asar missing: {}", asar_src.display());
        }

        qprintln!("[1/7] hashing source app.asar header (sha256) ...");
        let old_hash = asar::hash_header_sha256(&asar_src)?;
        qprintln!("      old header sha256: {}", old_hash);

        qprintln!("[2/7] cloning install tree...");
        let copy_stats = self.clone_tree()?;
        qprintln!(
            "      files: {}  bytes: {} ({:.1} MB)",
            copy_stats.0,
            copy_stats.1,
            copy_stats.1 as f64 / 1_048_576.0
        );

        let work_dir = self.out.join("_openforge_work").join("app");
        if work_dir.exists() {
            std::fs::remove_dir_all(&work_dir)?;
        }
        qprintln!("[3/7] extracting app.asar -> _openforge_work/app/ ...");
        let ext_stats = asar::extract_all(&asar_src, &work_dir)?;
        qprintln!(
            "      packed files: {}  bytes: {} ({:.1} MB)  unpacked-refs: {}",
            ext_stats.files,
            ext_stats.bytes,
            ext_stats.bytes as f64 / 1_048_576.0,
            ext_stats.unpacked
        );

        qprintln!("[4/7] keeping app.asar.unpacked/ in place (JS references it by path)...");
        let unpacked_src = self
            .install_root
            .join("resources")
            .join("app.asar.unpacked");
        if unpacked_src.is_dir() {
            qprintln!("      original unpacked dir retained at resources/app.asar.unpacked/");
        }

        qprintln!("[5/7] applying patches to working copy ...");
        let report = patch::apply_all(&work_dir, self.dry_run)?;
        if !quiet {
            for (rule, file, hits) in &report.applied {
                println!("      OK  {:32} {:42} x{}", rule, file, hits);
            }
            for line in &report.skipped {
                println!("      ..  {}", line);
            }
            for line in &report.failed {
                println!("      !!  {}", line);
            }
        }
        if !report.failed.is_empty() && !self.dry_run {
            bail!("one or more required patches failed - aborting before repack");
        }

        if let Some(icon_path) = &self.icon {
            if !self.dry_run {
                qprintln!("      icon: preparing {} ...", icon_path.display());
                let prep = icon::prepare(icon_path)?;
                let hits = icon::replace_asar_icons(&work_dir, &prep)?;
                if !quiet {
                    for h in &hits {
                        println!("      OK  asar-icon                     {}", h);
                    }
                    if hits.is_empty() {
                        println!("      ..  asar icon assets not found (skipped)");
                    }
                }
            }
        } else {
            qprintln!(
                "      ..  no icon source provided (use --icon or place OpenForge.png in cwd)"
            );
        }

        qprintln!(
            "[6/7] repacking app.asar with patches + rewriting embedded asar-integrity hash ..."
        );
        if !self.dry_run {
            let dst_asar = self.out.join("resources").join("app.asar");
            let stats = asar::pack(&work_dir, &dst_asar)?;
            qprintln!(
                "      packed files: {}  header: {} bytes  blob: {} bytes  total: {} bytes",
                stats.files,
                stats.header_bytes,
                stats.blob_bytes,
                stats.header_bytes + stats.blob_bytes
            );
            let work_root = self.out.join("_openforge_work");
            if work_root.exists() {
                std::fs::remove_dir_all(&work_root)?;
            }

            let new_hash = asar::hash_header_sha256(&dst_asar)?;
            qprintln!("      new header sha256: {}", new_hash);
            let exe = self.out.join("CurseForge.exe");
            if exe.is_file() && new_hash != old_hash {
                let hits = integrity::patch_exe_hash(&exe, &old_hash, &new_hash)?;
                qprintln!(
                    "      rewrote embedded integrity hash in CurseForge.exe ({} occurrence(s))",
                    hits
                );
            } else if new_hash == old_hash {
                qprintln!("      header bytes unchanged (no integrity patch needed)");
            }
        }

        qprintln!("[7/7] stubbing auto-updater + verifying fuses ...");
        if !self.dry_run {
            stub_update_yml(&self.out)?;
            qprintln!("      resources/app-update.yml stubbed (provider=generic, url=about:blank)");

            let exe = self.out.join("CurseForge.exe");
            if exe.is_file() {
                let changes = fuses::flip_for_portable(&exe)?;
                if changes.is_empty() {
                    qprintln!("      fuses already permissive (no change)");
                } else {
                    if !quiet {
                        for (idx, before, after) in &changes {
                            println!(
                                "      fuse[{}] 0x{:02X} -> 0x{:02X} ({})",
                                idx,
                                before,
                                after,
                                fuses::describe(*idx, *after)
                            );
                        }
                    }
                }
            }

            self.write_launcher()?;
            self.write_marker()?;

            if let Some(icon_path) = &self.icon {
                let exe = self.out.join("CurseForge.exe");
                if exe.is_file() {
                    match icon::replace_pe_icons(&exe, icon_path) {
                        Ok(n) => qprintln!(
                            "      rewrote PE icon resources in CurseForge.exe ({} frames)",
                            n
                        ),
                        Err(e) => qprintln!("      !!  PE icon update failed: {}", e),
                    }
                }
            }
        }
        qprintln!();
        qprintln!("done. portable build at: {}", self.out.display());
        qprintln!("launch with: {}\\CurseForge.exe", self.out.display());
        Ok(())
    }

    fn clone_tree(&self) -> Result<(u64, u64)> {
        let total_bytes: u64 = WalkDir::new(&self.install_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
            .sum();
        let bar = ProgressBar::new(total_bytes);
        bar.set_style(
            ProgressStyle::with_template(
                "      [{bar:30}] {bytes}/{total_bytes} {bytes_per_sec} ETA {eta}",
            )
            .unwrap()
            .progress_chars("##-"),
        );

        let mut n_files = 0u64;
        let mut n_bytes = 0u64;
        for ent in WalkDir::new(&self.install_root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let src = ent.path();
            let rel = src.strip_prefix(&self.install_root).unwrap();
            let dst = self.out.join(rel);
            if ent.file_type().is_dir() {
                if !self.dry_run {
                    std::fs::create_dir_all(&dst)?;
                }
                continue;
            }
            if ent.file_type().is_file() {
                if !self.dry_run {
                    if let Some(parent) = dst.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let n = std::fs::copy(src, &dst)
                        .with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;
                    n_files += 1;
                    n_bytes += n;
                    bar.inc(n);
                } else {
                    n_files += 1;
                    let n = ent.metadata().map(|m| m.len()).unwrap_or(0);
                    n_bytes += n;
                    bar.inc(n);
                }
            }
        }
        bar.finish_and_clear();
        Ok((n_files, n_bytes))
    }

    fn merge_dir(&self, src_dir: &Path, dst_dir: &Path) -> Result<(u64, u64)> {
        let mut n = 0u64;
        let mut b = 0u64;
        for ent in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
            let src = ent.path();
            let rel = src.strip_prefix(src_dir).unwrap();
            let dst = dst_dir.join(rel);
            if ent.file_type().is_dir() {
                if !self.dry_run {
                    std::fs::create_dir_all(&dst)?;
                }
                continue;
            }
            if ent.file_type().is_file() {
                if !self.dry_run {
                    if let Some(parent) = dst.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let bytes = std::fs::copy(src, &dst)?;
                    n += 1;
                    b += bytes;
                }
            }
        }
        Ok((n, b))
    }

    fn write_launcher(&self) -> Result<()> {
        let cmd = self.out.join("OpenForge.cmd");
        let body = "@echo off\r\nstart \"\" \"%~dp0CurseForge.exe\" %*\r\n";
        std::fs::write(&cmd, body)?;
        Ok(())
    }

    fn write_marker(&self) -> Result<()> {
        let marker = self.out.join("OpenForge-Portable.txt");
        let body = format!(
            "OpenForge portable build\r\nbuilt: {}\r\nsource: {}\r\npatches: kill_owadview, neuter_client_overrides, neuter_updater_endpoint, openforge_branding, freezer_intro_splash, unlock_premium_themes, ads_support_israel_text\r\nresources/app.asar.original is the pristine asar (rename back to disable patches)\r\n",
            chrono_now(),
            self.install_root.display()
        );
        std::fs::write(&marker, body)?;
        Ok(())
    }
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{}", secs)
}

fn stub_update_yml(root: &Path) -> Result<()> {
    let yml = root.join("resources").join("app-update.yml");
    let body = "provider: generic\r\nurl: about:blank\r\nupdaterCacheDirName: openforge-updater\r\n";
    std::fs::write(&yml, body).with_context(|| format!("write {}", yml.display()))?;
    Ok(())
}
