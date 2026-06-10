use anyhow::Result;
use regex::Regex;
use std::collections::BTreeMap;
use std::path::Path;
use walkdir::WalkDir;

pub fn run(root: &Path, context: usize) -> Result<()> {
    if !root.exists() {
        anyhow::bail!(
            "not extracted yet: {} (run `openforge-patch extract` first)",
            root.display()
        );
    }
    let patterns: Vec<(&str, Regex)> = vec![
        (
            "AD_HOST_overwolf",
            Regex::new(r"(?i)content[-.]ads?\.overwolf\.com")?,
        ),
        ("AD_HOST_doubleclick", Regex::new(r"(?i)doubleclick\.net")?),
        (
            "AD_HOST_googlesyndication",
            Regex::new(r"(?i)googlesyndication\.com")?,
        ),
        (
            "AD_HOST_googleadservices",
            Regex::new(r"(?i)googleadservices\.com")?,
        ),
        (
            "AD_HOST_amazon_adsystem",
            Regex::new(r"(?i)amazon-adsystem\.com")?,
        ),
        ("AD_HOST_adsbygoogle", Regex::new(r"(?i)adsbygoogle")?),
        (
            "AD_HOST_generic",
            Regex::new(r"(?i)\bads?[-_.]?(server|service|sdk|provider|manager|module)\b")?,
        ),
        ("TELEMETRY_mixpanel", Regex::new(r"(?i)mixpanel")?),
        ("TELEMETRY_segment", Regex::new(r"(?i)segment\.(io|com)")?),
        ("TELEMETRY_amplitude", Regex::new(r"(?i)amplitude")?),
        ("TELEMETRY_datadog", Regex::new(r"(?i)datadoghq")?),
        ("TELEMETRY_sentry", Regex::new(r"(?i)sentry\.io")?),
        (
            "TELEMETRY_ga",
            Regex::new(r"(?i)google[-_.]?analytics|gtag|gtm\.js")?,
        ),
        (
            "TELEMETRY_overwolf",
            Regex::new(r"(?i)analytics\.overwolf\.com|telemetry\.overwolf\.com")?,
        ),
        (
            "OW_SDK_ads",
            Regex::new(r"overwolf\.windows\.openUrl|owadview|OwAdView|owAdSdk|adManager")?,
        ),
        (
            "OW_SDK_track",
            Regex::new(r"overwolf\.utils\.openUrlInDefaultBrowser|overwolfPlugin|owLogger")?,
        ),
        (
            "ENDPOINT_curseforge_api",
            Regex::new(r"https?://[A-Za-z0-9.\-]*curseforge\.com")?,
        ),
        (
            "ENDPOINT_overwolf_api",
            Regex::new(r"https?://[A-Za-z0-9.\-]*overwolf\.com")?,
        ),
        (
            "FEATURE_ad_iframe",
            Regex::new(r#"(?i)<iframe[^>]*\bads?\b"#)?,
        ),
        (
            "FEATURE_consent",
            Regex::new(r"(?i)consent|gdpr|cookieconsent")?,
        ),
    ];

    let mut hits: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut scanned = 0u64;
    let mut bytes = 0u64;

    for ent in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !ent.file_type().is_file() {
            continue;
        }
        let p = ent.path();
        let ext_ok = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| {
                matches!(
                    e,
                    "js" | "mjs" | "cjs" | "json" | "html" | "htm" | "css" | "txt" | "yml" | "yaml"
                )
            })
            .unwrap_or(false);
        if !ext_ok {
            continue;
        }
        let Ok(bytes_v) = std::fs::read(p) else {
            continue;
        };
        bytes += bytes_v.len() as u64;
        scanned += 1;
        let Ok(text) = std::str::from_utf8(&bytes_v) else {
            continue;
        };
        let rel = p
            .strip_prefix(root)
            .unwrap_or(p)
            .display()
            .to_string()
            .replace('\\', "/");

        for (name, re) in &patterns {
            for m in re.find_iter(text).take(3) {
                let mut s = m.start().saturating_sub(context * 20);
                let mut e = (m.end() + context * 20).min(text.len());
                while s < text.len() && !text.is_char_boundary(s) {
                    s += 1;
                }
                while e < text.len() && !text.is_char_boundary(e) {
                    e += 1;
                }
                if e > text.len() {
                    e = text.len();
                }
                let snippet: String = text[s..e]
                    .chars()
                    .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
                    .take(180)
                    .collect();
                hits.entry((*name).to_string())
                    .or_default()
                    .push((rel.clone(), snippet));
            }
        }
    }

    println!(
        "scanned {} text files ({:.2} MB)",
        scanned,
        bytes as f64 / 1_048_576.0
    );
    println!("");
    for (name, list) in &hits {
        println!(
            "##### {}  ({} hit{})",
            name,
            list.len(),
            if list.len() == 1 { "" } else { "s" }
        );
        let mut seen_files = std::collections::BTreeSet::new();
        for (f, snippet) in list {
            let cap = snippet.len().min(40);
            let mut cap_b = cap;
            while cap_b > 0 && !snippet.is_char_boundary(cap_b) {
                cap_b -= 1;
            }
            let key = format!("{}::{}", f, &snippet[..cap_b]);
            if !seen_files.insert(key) {
                continue;
            }
            println!("  {}", f);
            println!("    ... {} ...", snippet.trim());
        }
        println!("");
    }
    if hits.is_empty() {
        println!("(no matches across any pattern - that's surprising; CF may compress strings)");
    }
    Ok(())
}
