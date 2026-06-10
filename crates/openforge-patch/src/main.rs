use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::{Read, Write};
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

mod asar;
mod fuses;
mod icon;
mod inspect;
mod integrity;
mod patch;
mod portable;

#[derive(Parser)]
#[command(
    name = "openforge-patch",
    version,
    about = "Patch the CurseForge desktop app (de-ad, de-spyware)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    Info,
    List {
        #[arg(long)]
        asar: Option<PathBuf>,
        #[arg(long, default_value_t = 50)]
        head: usize,
    },
    Extract {
        #[arg(long)]
        asar: Option<PathBuf>,
        #[arg(long, default_value = "recon/cf-extracted")]
        out: PathBuf,
    },
    Inspect {
        #[arg(long, default_value = "recon/cf-extracted")]
        root: PathBuf,
        #[arg(long, default_value_t = 5)]
        context: usize,
    },
    Patch {
        #[arg(long, default_value = "recon/cf-extracted")]
        root: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    Fuses {
        #[arg(long)]
        bin: PathBuf,
    },
    Portable {
        #[arg(long)]
        src: Option<PathBuf>,
        #[arg(long, default_value = "out/OpenForge-Portable")]
        out: PathBuf,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        icon: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("openforge_patch=info,info")),
        )
        .init();

    if std::env::args_os().len() <= 1 {
        if let Some(exe_name) = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
        {
            let stem = exe_name.to_ascii_lowercase();
            if stem.contains("installer") {
                return run_installer_mode();
            }
            if stem.contains("launcher") {
                return run_launcher_mode();
            }
        }
    }

    let cli = Cli::parse();
    match cli.cmd {
        None => {
            eprintln!("No command provided. Use --help for usage.");
            std::process::exit(2);
        }
        Some(Cmd::Info) => cmd_info(),
        Some(Cmd::List { asar, head }) => cmd_list(asar, head),
        Some(Cmd::Extract { asar, out }) => cmd_extract(asar, out),
        Some(Cmd::Inspect { root, context }) => inspect::run(&root, context),
        Some(Cmd::Fuses { bin }) => fuses::print_fuses(bin),
        Some(Cmd::Patch { root, dry_run }) => {
            let rep = patch::apply_all(&root, dry_run)?;
            for (rule, file, hits) in &rep.applied {
                println!("OK  {:32} {:42} x{}", rule, file, hits);
            }
            for l in &rep.skipped {
                println!("..  {}", l);
            }
            for l in &rep.failed {
                println!("!!  {}", l);
            }
            if !rep.failed.is_empty() {
                anyhow::bail!("{} patches failed", rep.failed.len());
            }
            Ok(())
        }
        Some(Cmd::Portable {
            src,
            out,
            force,
            dry_run,
            icon,
        }) => {
            let install_root = src
                .or_else(asar::cf_install_root)
                .context("--src not given and CurseForge install not found")?;
            let icon = icon.or_else(|| {
                let p = PathBuf::from("OpenForge.png");
                if p.is_file() {
                    Some(p)
                } else {
                    None
                }
            });
            portable::PortableBuild {
                install_root,
                out,
                force,
                dry_run,
                icon,
            }
            .run()
        }
    }
}

fn run_installer_mode() -> Result<()> {
    block_ctrl_c();
    configure_console_window();
    print_padded_line("");
    print_padded_line("");
    let mut last_pct: u64 = 0;
    let mut last_msg = String::new();
    let mut cb = |p: f32, s: &str| {
        let v = (p.clamp(0.0, 1.0) * 100.0) as u64;
        if s != last_msg || v >= last_pct + 2 || v == 100 {
            print_padded_line(&format!("{:>3}%  {}", v, s));
            last_pct = v;
            last_msg = s.to_string();
        }
    };

    let result = run_installer_mode_with_progress(&mut cb);
    match result {
        Ok(()) => {
            print_padded_line("100%  Done");
            Ok(())
        }
        Err(e) => {
            print_padded_line(&format!("FAILED  {}", e));
            Err(e)
        }
    }
}

fn print_padded_line(msg: &str) {
    const WIDTH: usize = 64;
    let mut s: String = msg.chars().take(WIDTH).collect();
    if s.len() < WIDTH {
        s.push_str(&" ".repeat(WIDTH - s.len()));
    }
    println!("  {}", s);
}

fn run_installer_mode_with_progress<F>(progress: &mut F) -> Result<()>
where
    F: FnMut(f32, &str),
{
    progress(0.02, "Preparing install paths");

    let exe = std::env::current_exe().context("failed to get current exe path")?;
    let base = exe
        .parent()
        .context("failed to get installer directory")?
        .to_path_buf();

    let local = std::env::var_os("LOCALAPPDATA")
        .context("LOCALAPPDATA not set")?;
    let install_root = PathBuf::from(local)
        .join("Programs")
        .join("CurseForge Windows");
    let install_parent = install_root
        .parent()
        .context("failed to resolve install parent")?
        .to_path_buf();
    fs::create_dir_all(&install_parent)?;
    fs::create_dir_all(&install_root)?;

    let work_root = base.join("_installer_work");
    let setup_exe = work_root.join("CurseForgeSetup.exe");
    let latest_yml = work_root.join("latest.yml");
    let patched_out = work_root.join("PatchedCurseForge");
    let setup_cache_dir = std::env::temp_dir().join("OpenForgeInstaller");
    fs::create_dir_all(&setup_cache_dir)?;
    let setup_cache = setup_cache_dir.join("CurseForgeSetup.exe");
    let legacy_setup_cache = base.join("CurseForgeSetup.exe");
    let embedded_icon_path = work_root.join("OpenForge.png");

    if legacy_setup_cache.is_file() {
        if !setup_cache.is_file() {
            let _ = fs::rename(&legacy_setup_cache, &setup_cache);
        }
        let _ = fs::remove_file(&legacy_setup_cache);
    }

    if work_root.exists() {
        fs::remove_dir_all(&work_root)
            .with_context(|| format!("remove {}", work_root.display()))?;
    }
    fs::create_dir_all(&work_root)?;
    write_embedded_icon(&embedded_icon_path)?;
    progress(0.10, "Paths ready");

    progress(0.15, "Downloading installer metadata");
    if setup_cache.is_file() {
        fs::copy(&setup_cache, &setup_exe)
            .with_context(|| format!("copy setup cache {}", setup_cache.display()))?;
        progress(0.45, "Using cached installer");
    } else {
        let feed_url = "https://electron-updates.overwolf.com/electron-updates/electron/cfiahnpaolfnlgaihhmobmnjdafknjnjdpdabpcm/latest.yml";
        download_with_curl(feed_url, &latest_yml, 0.15, 0.20, progress)?;
        let setup_url = parse_setup_url_from_latest_yml(&latest_yml)?;
        download_with_curl(&setup_url, &setup_exe, 0.20, 0.45, progress)?;
        let _ = fs::copy(&setup_exe, &setup_cache);
    }

    progress(0.50, "Installing CurseForge");
    stop_running_clients();
    run_setup_for_target(&setup_exe, &install_root)?;
    progress(0.65, "CurseForge installed");

    let icon = {
        let p = base.join("OpenForge.png");
        if p.is_file() { Some(p) } else { Some(embedded_icon_path.clone()) }
    };

    progress(0.70, "Applying OpenForge patch set");
    std::env::set_var("OPENFORGE_INSTALLER_QUIET", "1");
    portable::PortableBuild {
        install_root: install_root.clone(),
        out: patched_out.clone(),
        force: true,
        dry_run: false,
        icon,
    }
    .run()?;
    std::env::remove_var("OPENFORGE_INSTALLER_QUIET");
    progress(0.90, "Patch complete");

    progress(0.95, "Finalizing and creating shortcuts");
    stop_running_clients();
    replace_install_with_backup(&install_root, &patched_out)?;

    let exe_path = install_root.join("CurseForge.exe");
    let launcher_path = install_root.join("OpenForgeLauncher.exe");
    if let Ok(self_exe) = std::env::current_exe() {
        let _ = fs::copy(&self_exe, &launcher_path);
    }
    if exe_path.is_file() {
        let shortcut_target = if launcher_path.is_file() { &launcher_path } else { &exe_path };
        let _ = create_shortcuts(shortcut_target, &exe_path);
        let _ = launch_detached(&exe_path);
    }

    if work_root.exists() {
        let _ = fs::remove_dir_all(&work_root);
    }
    let _ = fs::remove_file(&setup_cache);
    progress(1.0, "Install complete");
    Ok(())
}

#[cfg(windows)]
fn launch_detached(exe_path: &std::path::Path) -> Result<()> {
    Command::new("cmd")
        .args([
            "/C",
            "start",
            "",
            &exe_path.to_string_lossy(),
        ])
        .status()
        .with_context(|| format!("launch {}", exe_path.display()))?;
    Ok(())
}

#[cfg(not(windows))]
fn launch_detached(exe_path: &std::path::Path) -> Result<()> {
    let _ = Command::new(exe_path).spawn()
        .with_context(|| format!("launch {}", exe_path.display()))?;
    Ok(())
}

fn run_launcher_mode() -> Result<()> {
    let self_exe = std::env::current_exe().context("get launcher exe path")?;
    let install_root = self_exe
        .parent()
        .context("resolve install dir from launcher")?
        .to_path_buf();
    let asar_path = install_root.join("resources").join("app.asar");
    let cf_exe = install_root.join("CurseForge.exe");

    if asar_path.is_file() && !asar_is_patched(&asar_path).unwrap_or(true) {
        if let Err(e) = repatch_in_place(&install_root) {
            eprintln!("OpenForge: re-patch skipped ({})", e);
        }
    }

    if !cf_exe.is_file() {
        anyhow::bail!(
            "CurseForge.exe not found next to launcher at {}",
            install_root.display()
        );
    }
    launch_detached(&cf_exe)
}

fn asar_is_patched(asar_path: &std::path::Path) -> Result<bool> {
    let data = fs::read(asar_path).with_context(|| format!("read {}", asar_path.display()))?;
    Ok(find_subslice(&data, b"OpenForge").is_some())
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn repatch_in_place(install_root: &std::path::Path) -> Result<()> {
    let asar_path = install_root.join("resources").join("app.asar");
    let cf_exe = install_root.join("CurseForge.exe");
    if !asar_path.is_file() {
        anyhow::bail!("app.asar missing at {}", asar_path.display());
    }

    let old_hash = asar::hash_header_sha256(&asar_path)?;

    let work_root = install_root.join("_openforge_relaunch");
    let work = work_root.join("app");
    if work_root.exists() {
        fs::remove_dir_all(&work_root)?;
    }
    asar::extract_all(&asar_path, &work)?;

    let report = patch::apply_all(&work, false)?;
    if !report.failed.is_empty() {
        let _ = fs::remove_dir_all(&work_root);
        anyhow::bail!("{} patch rule(s) failed", report.failed.len());
    }

    asar::pack(&work, &asar_path)?;
    let new_hash = asar::hash_header_sha256(&asar_path)?;
    if cf_exe.is_file() {
        if new_hash != old_hash {
            let _ = integrity::patch_exe_hash(&cf_exe, &old_hash, &new_hash);
        }
        let _ = fuses::flip_for_portable(&cf_exe);
    }
    let _ = fs::remove_dir_all(&work_root);
    Ok(())
}

fn write_embedded_icon(path: &std::path::Path) -> Result<()> {
    static OPENFORGE_PNG: &[u8] = include_bytes!("../../../OpenForge.png");
    fs::write(path, OPENFORGE_PNG)
        .with_context(|| format!("write embedded icon {}", path.display()))?;
    Ok(())
}

fn replace_install_with_backup(install_root: &std::path::Path, patched_out: &std::path::Path) -> Result<()> {
    let backup = install_root.with_extension("backup_openforge");
    if backup.exists() {
        fs::remove_dir_all(&backup)
            .with_context(|| format!("remove old backup {}", backup.display()))?;
    }

    if install_root.exists() {
        fs::rename(install_root, &backup)
            .with_context(|| format!("backup existing install {}", install_root.display()))?;
    }

    match fs::rename(patched_out, install_root) {
        Ok(_) => {
            if backup.exists() {
                let _ = fs::remove_dir_all(&backup);
            }
            Ok(())
        }
        Err(e) => {
            if backup.exists() && !install_root.exists() {
                let _ = fs::rename(&backup, install_root);
            }
            Err(e).with_context(|| {
                format!(
                    "replace install with {}",
                    patched_out.display()
                )
            })
        }
    }
}

fn download_with_curl<F>(
    url: &str,
    out: &std::path::Path,
    start: f32,
    end: f32,
    progress: &mut F,
) -> Result<()>
where
    F: FnMut(f32, &str),
{
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .context("build http client")?;

    let mut resp = client
        .get(url)
        .send()
        .with_context(|| format!("http get {}", url))?;
    if !resp.status().is_success() {
        anyhow::bail!("download failed: {} (status {})", url, resp.status());
    }

    let total = resp.content_length().unwrap_or(0);
    let label = format!("Downloading {}", url);
    progress(start, &label);

    let mut file = File::create(out)
        .with_context(|| format!("create {}", out.display()))?;
    let mut got: u64 = 0;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = resp
            .read(&mut buf)
            .with_context(|| format!("read from {}", url))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .with_context(|| format!("write {}", out.display()))?;
        got += n as u64;
        if total > 0 {
            let t = start + (end - start) * (got as f32 / total as f32);
            progress(t, &label);
        }
    }
    progress(end, &label);
    Ok(())
}

#[cfg(windows)]
fn configure_console_window() {
    unsafe {
        use windows_sys::Win32::System::Console::*;
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let hwnd = windows_sys::Win32::System::Console::GetConsoleWindow();
        if !hwnd.is_null() {
            let mut style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
            style &= !(WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX);
            SetWindowLongW(hwnd, GWL_STYLE, style as i32);

            let w = 560;
            let h = 360;
            let sw = GetSystemMetrics(SM_CXSCREEN);
            let sh = GetSystemMetrics(SM_CYSCREEN);
            let x = (sw - w) / 2;
            let y = (sh - h) / 2;

            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                x,
                y,
                w,
                h,
                SWP_FRAMECHANGED,
            );

            let out = GetStdHandle(STD_OUTPUT_HANDLE);
            if !out.is_null() && (out as isize) != -1 {
                let mut info = std::mem::zeroed::<CONSOLE_SCREEN_BUFFER_INFO>();
                if GetConsoleScreenBufferInfo(out, &mut info) != 0 {
                    let mut sz = info.dwSize;
                    let window_h = info.srWindow.Bottom - info.srWindow.Top + 1;
                    if sz.Y != window_h {
                        sz.Y = window_h;
                        let _ = SetConsoleScreenBufferSize(out, sz);
                    }
                }
            }

            let input = GetStdHandle(STD_INPUT_HANDLE);
            if !input.is_null() && (input as isize) != -1 {
                let mut mode: u32 = 0;
                if GetConsoleMode(input, &mut mode) != 0 {
                    mode &= !ENABLE_MOUSE_INPUT;
                    let _ = SetConsoleMode(input, mode);
                }
            }

            {
                const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
                const DWMWCP_ROUND: u32 = 2;
                let pref: u32 = DWMWCP_ROUND;
                let _ = windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &pref as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );
            }

            windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                hwnd,
                windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW,
            );
        }
    }
}

#[cfg(not(windows))]
fn configure_console_window() {}

#[cfg(windows)]
fn block_ctrl_c() {
    unsafe {
        use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
        let _ = SetConsoleCtrlHandler(Some(installer_ctrl_handler), 1);
    }
}

#[cfg(not(windows))]
fn block_ctrl_c() {}

#[cfg(windows)]
unsafe extern "system" fn installer_ctrl_handler(_ctrl_type: u32) -> i32 {
    1
}

fn parse_setup_url_from_latest_yml(path: &std::path::Path) -> Result<String> {
    let yml = fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    for line in yml.lines() {
        let s = line.trim();
        if let Some(v) = s.strip_prefix("- url:") {
            let u = v.trim();
            if u.starts_with("http://") || u.starts_with("https://") {
                return Ok(u.to_string());
            }
        }
        if let Some(v) = s.strip_prefix("url:") {
            let u = v.trim();
            if u.starts_with("http://") || u.starts_with("https://") {
                return Ok(u.to_string());
            }
        }
    }
    anyhow::bail!("setup url not found in {}", path.display())
}

fn run_setup_for_target(setup_exe: &std::path::Path, target: &std::path::Path) -> Result<()> {
    let target_arg = format!("/D={}", target.display());
    let tries: &[&[&str]] = &[
        &["/S", "--silent"],
        &["/S", "--silent", "--no-launch"],
        &["/S"],
    ];

    for flags in tries {
        let mut args: Vec<OsString> = flags.iter().map(|s| OsString::from(*s)).collect();
        args.push(OsString::from(&target_arg));
        let status = Command::new(setup_exe)
            .args(&args)
            .status();
        if let Ok(st) = status {
            if st.success() && target.join("resources").join("app.asar").is_file() {
                return Ok(());
            }
        }
    }

    let status = Command::new(setup_exe)
        .status()
        .context("launch setup interactively")?;
    if !status.success() {
        anyhow::bail!("setup failed with status {}", status);
    }
    if !target.join("resources").join("app.asar").is_file() {
        anyhow::bail!("setup finished but app.asar not found at {}", target.display());
    }
    Ok(())
}

fn stop_running_clients() {
    let _ = Command::new("cmd")
        .args([
            "/C",
            "taskkill /F /IM CurseForge.exe >NUL 2>&1",
        ])
        .status();
    let _ = Command::new("cmd")
        .args([
            "/C",
            "taskkill /F /IM Curse.Agent.Host.exe >NUL 2>&1",
        ])
        .status();
}

fn create_shortcuts(target_exe: &std::path::Path, icon_source: &std::path::Path) -> Result<()> {
    let user_profile = std::env::var_os("USERPROFILE").context("USERPROFILE not set")?;
    let appdata = std::env::var_os("APPDATA").context("APPDATA not set")?;
    let public = std::env::var_os("PUBLIC");

    let desktop_dir = PathBuf::from(&user_profile).join("Desktop");
    let desktop = desktop_dir.join("OpenForge.lnk");
    let start_menu_dir = PathBuf::from(&appdata).join("Microsoft").join("Windows").join("Start Menu").join("Programs");
    fs::create_dir_all(&start_menu_dir)?;
    let start_menu = start_menu_dir.join("OpenForge.lnk");

    cleanup_existing_shortcuts(&desktop_dir)?;
    cleanup_existing_shortcuts(&start_menu_dir)?;
    if let Some(public_root) = public {
        let public_desktop = PathBuf::from(public_root).join("Desktop");
        let _ = cleanup_existing_shortcuts(&public_desktop);
    }

    write_shortcut_via_powershell(&desktop, target_exe, icon_source)?;
    write_shortcut_via_powershell(&start_menu, target_exe, icon_source)?;
    Ok(())
}

fn cleanup_existing_shortcuts(dir: &std::path::Path) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let names = [
        "OpenForge.lnk",
        "CurseForge.lnk",
        "OpenForge.lnk",
        "OpenForge Installer.lnk",
    ];
    for name in names {
        let p = dir.join(name);
        if p.is_file() {
            let _ = fs::remove_file(&p);
        }
    }
    Ok(())
}

fn write_shortcut_via_powershell(link_path: &std::path::Path, target_exe: &std::path::Path, icon_source: &std::path::Path) -> Result<()> {
    let link = link_path.to_string_lossy().replace('"', "`\"");
    let target = target_exe.to_string_lossy().replace('"', "`\"");
    let icon = icon_source.to_string_lossy().replace('"', "`\"");
    let workdir = target_exe
        .parent()
        .map(|p| p.to_string_lossy().replace('"', "`\""))
        .unwrap_or_default();

    let ps = format!(
        "$W=New-Object -ComObject WScript.Shell; $S=$W.CreateShortcut(\"{}\"); $S.TargetPath=\"{}\"; $S.WorkingDirectory=\"{}\"; $S.IconLocation=\"{},0\"; $S.Save()",
        link,
        target,
        workdir,
        icon
    );

    let status = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(ps)
        .status()
        .context("create shortcut via powershell")?;
    if !status.success() {
        anyhow::bail!("failed to create shortcut at {}", link_path.display());
    }
    Ok(())
}

fn cmd_info() -> Result<()> {
    let root = asar::cf_install_root()
        .context("CurseForge install not found in %LOCALAPPDATA%\\Programs\\CurseForge Windows")?;
    println!("install root: {}", root.display());
    let asar_path = root.join("resources").join("app.asar");
    println!("app.asar    : {}", asar_path.display());
    let meta = std::fs::metadata(&asar_path)?;
    println!("size        : {} bytes", meta.len());

    let reader = asar::AsarReader::open(&asar_path)?;
    println!("data offset : {}", reader.data_offset);
    let mut top = Vec::new();
    for (k, _) in reader.header.files.iter().take(20) {
        top.push(k.clone());
    }
    println!("top entries : {}", top.join(", "));
    Ok(())
}

fn cmd_list(asar_path: Option<PathBuf>, head: usize) -> Result<()> {
    let p = asar_path.map(Ok).unwrap_or_else(asar::cf_asar_path)?;
    let entries = asar::list_all(&p)?;
    println!("total entries: {}", entries.len());
    let mut sorted = entries.clone();
    sorted.sort_by_key(|(_, s, _)| std::cmp::Reverse(*s));
    println!("--- {} largest ---", head);
    for (path, size, unpacked) in sorted.iter().take(head) {
        println!(
            "{:>12}  {}{}",
            size,
            path.display(),
            if *unpacked { " [unpacked]" } else { "" }
        );
    }
    Ok(())
}

fn cmd_extract(asar_path: Option<PathBuf>, out: PathBuf) -> Result<()> {
    let p = asar_path.map(Ok).unwrap_or_else(asar::cf_asar_path)?;
    println!("extracting {} -> {}", p.display(), out.display());
    let stats = asar::extract_all(p, out.clone())?;
    println!(
        "files: {}  dirs: {}  bytes: {}  unpacked-skipped: {}",
        stats.files, stats.dirs, stats.bytes, stats.unpacked
    );
    println!("wrote {}/__asar_header__.json", out.display());
    Ok(())
}
