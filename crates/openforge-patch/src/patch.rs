use anyhow::{Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct PatchRule {
    pub name: &'static str,
    pub find: &'static str,
    pub replace: &'static str,
    pub is_regex: bool,
    pub min_hits: usize,
    pub max_hits: usize,
    pub target_globs: &'static [&'static str],
}

pub fn rules() -> Vec<PatchRule> {
    vec![
        PatchRule {
            name: "kill_owadview_double",
            find: r#"document.createElement("owadview")"#,
            replace: r#"document.createElement("div")"#,
            is_regex: false,
            min_hits: 1, max_hits: 16,
            target_globs: &["dist/desktop/desktop.js", "dist/game-dashboard/game-dashboard.js"],
        },
        PatchRule {
            name: "kill_owadview_single",
            find: r#"document.createElement('owadview')"#,
            replace: r#"document.createElement('div')"#,
            is_regex: false,
            min_hits: 0, max_hits: 16,
            target_globs: &["dist/desktop/desktop.js", "dist/game-dashboard/game-dashboard.js"],
        },
        PatchRule {
            name: "kill_owadview_tagname",
            find: r#""owadview""#,
            replace: r#""ow-noop""#,
            is_regex: false,
            min_hits: 0, max_hits: 32,
            target_globs: &["dist/desktop/desktop.js", "dist/game-dashboard/game-dashboard.js", "dist/preload/preload.js"],
        },
        PatchRule {
            name: "neuter_client_overrides",
            find: r#""https://curseforge.overwolf.com/downloads/client-overrides-v2.json""#,
            replace: r#""about:blank""#,
            is_regex: false,
            min_hits: 1, max_hits: 4,
            target_globs: &["dist/background/background.js"],
        },
        PatchRule {
            name: "neuter_updater_endpoint_electron",
            find: r#""https://electron-updates.overwolf.com/electron-updates/electron/"#,
            replace: r#""about:blank?disabled=#"#,
            is_regex: false,
            min_hits: 1, max_hits: 4,
            target_globs: &["dist/background/background.js"],
        },
        PatchRule {
            name: "neuter_updater_endpoint_mac",
            find: r#""https://curseforge.overwolf.com/electron/mac""#,
            replace: r#""about:blank""#,
            is_regex: false,
            min_hits: 0, max_hits: 2,
            target_globs: &["dist/background/background.js"],
        },
        PatchRule {
            name: "kill_sentry_init_bundle",
            find: r#"Sentry\.init\("#,
            replace: r#"(function(){})("#,
            is_regex: true,
            min_hits: 0, max_hits: 8,
            target_globs: &[
                "dist/background/background.js",
                "dist/desktop/desktop.js",
                "dist/game-dashboard/game-dashboard.js",
                "dist/preload/preload.js",
            ],
        },
        PatchRule {
            name: "kill_sentry_dsn",
            find: r#"dsn:\s*"https://[^"]+sentry\.io/\d+""#,
            replace: r#"dsn:"""#,
            is_regex: true,
            min_hits: 0, max_hits: 8,
            target_globs: &[
                "dist/background/background.js",
                "dist/desktop/desktop.js",
                "dist/game-dashboard/game-dashboard.js",
                "dist/preload/preload.js",
            ],
        },
        PatchRule {
            name: "blank_sentry_ingest_url",
            find: r#"https://o\d+\.ingest\.sentry\.io"#,
            replace: r#"https://localhost.invalid"#,
            is_regex: true,
            min_hits: 0, max_hits: 8,
            target_globs: &[
                "dist/background/background.js",
                "dist/desktop/desktop.js",
                "dist/game-dashboard/game-dashboard.js",
                "dist/preload/preload.js",
            ],
        },
        PatchRule {
            name: "neuter_overwolf_analytics_host",
            find: r#"https://analyticsnew\.overwolf\.com"#,
            replace: r#"https://localhost.invalid"#,
            is_regex: true,
            min_hits: 0, max_hits: 16,
            target_globs: &[
                "dist/background/background.js",
                "dist/desktop/desktop.js",
                "dist/game-dashboard/game-dashboard.js",
                "dist/preload/preload.js",
            ],
        },
        PatchRule {
            name: "neuter_overwolf_analytics_alt",
            find: r#"https://(?:analytics|events|telemetry|ow-events)\.overwolf\.com"#,
            replace: r#"https://localhost.invalid"#,
            is_regex: true,
            min_hits: 0, max_hits: 16,
            target_globs: &[
                "dist/background/background.js",
                "dist/desktop/desktop.js",
                "dist/game-dashboard/game-dashboard.js",
                "dist/preload/preload.js",
            ],
        },
        PatchRule {
            name: "kill_analytics_send",
            find: r#"AnalyticsService\] Executing"#,
            replace: r#"AnalyticsService] Skipping"#,
            is_regex: true,
            min_hits: 0, max_hits: 4,
            target_globs: &[
                "dist/background/background.js",
            ],
        },
        PatchRule {
            name: "fake_premium_stub",
            find: "getActiveSubscriptionTypes(){return Promise.resolve([])}",
            replace: "getActiveSubscriptionTypes(){return Promise.resolve([1])}",
            is_regex: false,
            min_hits: 1, max_hits: 16,
            target_globs: &[
                "dist/background/background.js",
                "dist/desktop/desktop.js",
                "dist/game-dashboard/game-dashboard.js",
                "dist/preload/preload.js",
            ],
        },
        PatchRule {
            name: "fake_premium_facade",
            find: "getActiveSubscriptionTypes(){return this.subscriptionService.getActiveSubscriptionTypes()}",
            replace: "getActiveSubscriptionTypes(){return Promise.resolve([1])}",
            is_regex: false,
            min_hits: 0, max_hits: 8,
            target_globs: &[
                "dist/background/background.js",
                "dist/desktop/desktop.js",
                "dist/game-dashboard/game-dashboard.js",
                "dist/preload/preload.js",
            ],
        },
        PatchRule {
            name: "fake_premium_tebex_chain",
            find: "async getActiveSubscriptionTypes(){let e=await this.legacySubscriptionService.getActiveSubscriptionTypes();return e?.length||(e=await this.tebexApiSubscriptionServiceImpl.getActiveSubscriptionTypes()),e}",
            replace: "async getActiveSubscriptionTypes(){return [1]}",
            is_regex: false,
            min_hits: 0, max_hits: 4,
            target_globs: &[
                "dist/background/background.js",
            ],
        },
        PatchRule {
            name: "unlock_premium_theme_cards",
            find: "isPremium:!0",
            replace: "isPremium:!0,forceUnlock:!0",
            is_regex: false,
            min_hits: 8, max_hits: 24,
            target_globs: &["dist/desktop/desktop.js", "dist/game-dashboard/game-dashboard.js"],
        },
        PatchRule {
            name: "rename_desktop_window_title",
            find: "<title>CurseForge</title>",
            replace: "<title>OpenForge</title>",
            is_regex: false,
            min_hits: 1, max_hits: 1,
            target_globs: &["dist/desktop/desktop.html"],
        },
        PatchRule {
            name: "rename_background_window_title",
            find: "<title>CurseForge - Background</title>",
            replace: "<title>OpenForge - Background</title>",
            is_regex: false,
            min_hits: 1, max_hits: 1,
            target_globs: &["dist/background/background.html"],
        },
        PatchRule {
            name: "rename_startup_error_title",
            find: r#"showErrorBox("CurseForge App""#,
            replace: r#"showErrorBox("OpenForge App""#,
            is_regex: false,
            min_hits: 1, max_hits: 2,
            target_globs: &["dist/background/background.js"],
        },
        PatchRule {
            name: "rename_tray_open_label",
            find: r#"label:"Open CurseForge""#,
            replace: r#"label:"Open OpenForge""#,
            is_regex: false,
            min_hits: 1, max_hits: 2,
            target_globs: &["dist/background/background.js"],
        },
        PatchRule {
            name: "rename_desktop_crash_text",
            find: "Seems like CurseForge has crashed unexpectedly",
            replace: "Seems like OpenForge has crashed unexpectedly",
            is_regex: false,
            min_hits: 0, max_hits: 2,
            target_globs: &["dist/desktop/desktop.js"],
        },
        PatchRule {
            name: "rename_desktop_website_label",
            find: r#""CurseForge Website""#,
            replace: r#""OpenForge Website""#,
            is_regex: false,
            min_hits: 1, max_hits: 4,
            target_globs: &["dist/desktop/desktop.js"],
        },
        PatchRule {
            name: "rename_desktop_gallery_label",
            find: r#"text:"CurseForge Gallery""#,
            replace: r#"text:"OpenForge Gallery""#,
            is_regex: false,
            min_hits: 1, max_hits: 4,
            target_globs: &["dist/desktop/desktop.js"],
        },
        PatchRule {
            name: "rename_desktop_titlebar_logo",
            find: r#"(0,a.jsxs)("div",{className:"curseforge-logo",children:[(0,a.jsx)(d.GO,{iconName:"logo-type"}),C?.title&&(0,a.jsx)("span",{className:"alpha-tag",children:C?.title})]})"#,
            replace: r#"(0,a.jsxs)("div",{className:"curseforge-logo",children:[(0,a.jsx)("span",{className:"window-title",children:"OpenForge"}),C?.title&&(0,a.jsx)("span",{className:"alpha-tag",children:C?.title})]})"#,
            is_regex: false,
            min_hits: 1, max_hits: 1,
            target_globs: &["dist/desktop/desktop.js"],
        },
        PatchRule {
            name: "rename_dashboard_titlebar_logo",
            find: r#"(0,i.jsxs)("div",{className:"curseforge-logo",children:[(0,i.jsx)(u.GO,{iconName:"logo-type"}),E?.title&&(0,i.jsx)("span",{className:"alpha-tag",children:E?.title})]})"#,
            replace: r#"(0,i.jsxs)("div",{className:"curseforge-logo",children:[(0,i.jsx)("span",{className:"window-title",children:"OpenForge"}),E?.title&&(0,i.jsx)("span",{className:"alpha-tag",children:E?.title})]})"#,
            is_regex: false,
            min_hits: 0, max_hits: 1,
            target_globs: &["dist/game-dashboard/game-dashboard.js"],
        },
        PatchRule {
            name: "rename_discord_presence_text",
            find: r#"defaultCurseForgeImageText="CurseForge""#,
            replace: r#"defaultCurseForgeImageText="OpenForge""#,
            is_regex: false,
            min_hits: 0, max_hits: 2,
            target_globs: &["dist/background/background.js"],
        },
        PatchRule {
            name: "replace_intro_splash_video",
            find: r#"src:`\$\{[^}]+\}video/intro\.webm`,autoPlay:!0,muted:!0,onEnded:([A-Za-z_$][A-Za-z0-9_$]*)"#,
            replace: r#"src:"https://filedrop.malice.nz/p/BetterIntro.mp4",autoPlay:!0,muted:!1,onEnded:$1"#,
            is_regex: true,
            min_hits: 1, max_hits: 2,
            target_globs: &["dist/desktop/desktop.js"],
        },
        PatchRule {
            name: "rename_app_metadata",
            find: "\"name\": \"CurseForge\"",
            replace: "\"name\": \"OpenForge\"",
            is_regex: false,
            min_hits: 1, max_hits: 2,
            target_globs: &["package.json"],
        },
        PatchRule {
            name: "rename_app_description",
            find: "\"description\": \"The CurseForge Electron App\"",
            replace: "\"description\": \"The OpenForge Electron App\"",
            is_regex: false,
            min_hits: 1, max_hits: 2,
            target_globs: &["package.json"],
        },
        PatchRule {
            name: "rename_app_homepage",
            find: "\"homepage\": \"https://curseforge.overwolf.com\"",
            replace: "\"homepage\": \"https://openforge.local\"",
            is_regex: false,
            min_hits: 1, max_hits: 2,
            target_globs: &["package.json"],
        },
        PatchRule {
            name: "rename_app_repository",
            find: "\"repository\": \"https://github.com/overwolf/curseforge-app.git\"",
            replace: "\"repository\": \"https://github.com/malice-nz/OpenForge.git\"",
            is_regex: false,
            min_hits: 1, max_hits: 2,
            target_globs: &["package.json"],
        },
    ]
}

#[derive(Debug, Default)]
pub struct PatchReport {
    pub applied: Vec<(String, String, usize)>,
    pub skipped: Vec<String>,
    pub failed: Vec<String>,
}

pub fn apply_all(app_root: &Path, dry_run: bool) -> Result<PatchReport> {
    let rules = rules();
    let mut report = PatchReport::default();
    let mut file_cache: std::collections::HashMap<PathBuf, String> = Default::default();

    for rule in &rules {
        let mut total_hits = 0usize;
        let mut hits_per_file: Vec<(PathBuf, usize)> = Vec::new();

        for glob in rule.target_globs {
            let target = app_root.join(glob);
            if !target.is_file() {
                continue;
            }
            let content = match file_cache.get(&target) {
                Some(s) => s.clone(),
                None => {
                    let s = std::fs::read_to_string(&target)
                        .with_context(|| format!("read {}", target.display()))?;
                    file_cache.insert(target.clone(), s.clone());
                    s
                }
            };
            let (n, new_text) = if rule.is_regex {
                let re = Regex::new(rule.find)
                    .with_context(|| format!("compile regex for {}", rule.name))?;
                let n = re.find_iter(&content).count();
                if n == 0 {
                    continue;
                }
                let new_text = re.replace_all(&content, rule.replace).into_owned();
                (n, new_text)
            } else {
                let n = content.matches(rule.find).count();
                if n == 0 {
                    continue;
                }
                let new_text = content.replace(rule.find, rule.replace);
                (n, new_text)
            };
            file_cache.insert(target.clone(), new_text);
            total_hits += n;
            hits_per_file.push((target, n));
        }

        if total_hits < rule.min_hits {
            report.failed.push(format!(
                "{}: required >= {} hits, found {}",
                rule.name, rule.min_hits, total_hits
            ));
            continue;
        }
        if total_hits > rule.max_hits {
            report.failed.push(format!(
                "{}: too many hits ({}) - aborting to avoid over-patching",
                rule.name, total_hits
            ));
            continue;
        }
        if total_hits == 0 {
            report
                .skipped
                .push(format!("{}: 0 hits (optional)", rule.name));
            continue;
        }
        for (p, n) in &hits_per_file {
            report.applied.push((
                rule.name.to_string(),
                p.strip_prefix(app_root)
                    .unwrap_or(p)
                    .display()
                    .to_string()
                    .replace('\\', "/"),
                *n,
            ));
        }
    }

    apply_locale_branding(app_root, dry_run, &mut report)?;

    if !dry_run {
        for (path, new_content) in file_cache {
            std::fs::write(&path, new_content)
                .with_context(|| format!("write {}", path.display()))?;
        }
    }
    Ok(report)
}

fn apply_locale_branding(app_root: &Path, dry_run: bool, report: &mut PatchReport) -> Result<()> {
    let locales_dir = app_root.join("_locales");
    if !locales_dir.is_dir() {
        report
            .skipped
            .push("rename_locale_values: _locales not found (optional)".to_string());
        return Ok(());
    }

    let mut changed_files = 0usize;
    let mut changed_values = 0usize;
    for ent in WalkDir::new(&locales_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !ent.file_type().is_file() {
            continue;
        }
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let mut json: serde_json::Value =
            serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        let hits = rewrite_brand_values(&mut json);
        if hits == 0 {
            continue;
        }

        changed_files += 1;
        changed_values += hits;
        if !dry_run {
            let mut next = serde_json::to_string_pretty(&json)
                .with_context(|| format!("serialize {}", path.display()))?;
            next.push('\n');
            std::fs::write(path, next).with_context(|| format!("write {}", path.display()))?;
        }
    }

    if changed_files == 0 {
        report
            .skipped
            .push("rename_locale_values: 0 hits (optional)".to_string());
    } else {
        report.applied.push((
            "rename_locale_values".to_string(),
            "_locales/*.json".to_string(),
            changed_values,
        ));
    }

    Ok(())
}

fn rewrite_brand_values(value: &mut serde_json::Value) -> usize {
    match value {
        serde_json::Value::String(s) => {
            let next = if s == "Ads support authors" {
                "Ads support Israel".to_string()
            } else if s == "Show Intro" {
                "Watch the best video ever".to_string()
            } else {
                s.clone()
            };

            let next = next
                .replace("CurseForge", "OpenForge")
                .replace("curseforge", "openforge")
                .replace("CURSEFORGE", "OPENFORGE");
            if next == *s {
                0
            } else {
                *s = next;
                1
            }
        }
        serde_json::Value::Array(values) => values.iter_mut().map(rewrite_brand_values).sum(),
        serde_json::Value::Object(map) => map.values_mut().map(rewrite_brand_values).sum(),
        _ => 0,
    }
}

pub fn remove_update_yml(install_root: &Path) -> Result<bool> {
    let yml = install_root.join("resources").join("app-update.yml");
    if yml.exists() {
        std::fs::remove_file(&yml).with_context(|| format!("remove {}", yml.display()))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn list_text_targets(app_root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for ent in WalkDir::new(app_root).into_iter().filter_map(|e| e.ok()) {
        if !ent.file_type().is_file() {
            continue;
        }
        let p = ent.path();
        if p.extension().and_then(|e| e.to_str()) == Some("js") {
            out.push(p.to_path_buf());
        }
    }
    out
}
