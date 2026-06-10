use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use openforge::api::{build_http_client, Aggregator, Backend, ForgeCdn, SearchQuery};
use openforge::games::{Game, GameAdapter, MinecraftAdapter, Sims4Adapter, WowAdapter};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "openforge", version, about = "Open CurseForge client", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,

    #[arg(long, env = "OPENFORGE_ETERNAL_KEY", global = true)]
    eternal_key: Option<String>,
}

#[derive(Subcommand)]
enum Cmd {
    Files {
        #[arg(long)]
        mod_id: u64,
        #[arg(long, default_value_t = 10)]
        page_size: u32,
        #[arg(long, default_value_t = 0)]
        index: u32,
    },
    Info {
        #[arg(long)]
        mod_id: u64,
    },
    Search {
        #[arg(long, default_value_t = 432)]
        game_id: u64,
        #[arg(long)]
        q: String,
        #[arg(long, default_value_t = 10)]
        page_size: u32,
    },
    Download {
        #[arg(long)]
        mod_id: u64,
        #[arg(long)]
        file_id: u64,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Install {
        #[arg(long)]
        game: String,
        #[arg(long, default_value = "default")]
        instance: String,
        #[arg(long)]
        mod_id: u64,
        #[arg(long)]
        file_id: u64,
        #[arg(long)]
        wow_root: Option<PathBuf>,
        #[arg(long)]
        ark_root: Option<PathBuf>,
    },
    Url {
        #[arg(long)]
        mod_id: u64,
        #[arg(long)]
        file_id: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("openforge=info,warn")))
        .init();

    let cli = Cli::parse();
    let client = build_http_client();
    let agg = Aggregator::new(client.clone(), cli.eternal_key.clone());
    let cdn = ForgeCdn::new(client);

    match cli.command {
        Cmd::Files { mod_id, page_size, index } => {
            let files = agg.list_files(mod_id, index, page_size).await
                .context("list files")?;
            println!("{}", serde_json::to_string_pretty(&files)?);
        }
        Cmd::Info { mod_id } => {
            let info = agg.mod_info(mod_id).await.context("mod info")?;
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        Cmd::Search { game_id, q, page_size } => {
            let res = agg.search(&SearchQuery {
                game_id,
                search: Some(q),
                page_size,
                ..Default::default()
            }).await.context("search")?;
            println!("{}", serde_json::to_string_pretty(&res)?);
        }
        Cmd::Url { mod_id, file_id } => {
            let url = agg.download_url(mod_id, file_id).await.context("download url")?;
            println!("{}", url);
        }
        Cmd::Download { mod_id, file_id, out } => {
            download(&agg, &cdn, mod_id, file_id, out).await?;
        }
        Cmd::Install { game, instance, mod_id, file_id, wow_root, ark_root } => {
            let adapter: Box<dyn GameAdapter> = match Game::from_slug(&game) {
                Some(Game::Minecraft) => Box::new(MinecraftAdapter),
                Some(Game::WowRetail) => Box::new(WowAdapter { retail: true,  wow_root: wow_root.context("--wow-root required")? }),
                Some(Game::WowClassic) => Box::new(WowAdapter { retail: false, wow_root: wow_root.context("--wow-root required")? }),
                Some(Game::Sims4) => Box::new(Sims4Adapter),
                Some(Game::ArkSurvivalEvolved) => {
                    let _ = ark_root.context("--ark-root required")?;
                    bail!("ARK install routing not yet implemented (mod-id based subdir)");
                }
                _ => bail!("unsupported game slug: {}", game),
            };
            let dir = adapter.install_dir(&instance);
            tokio::fs::create_dir_all(&dir).await?;
            download(&agg, &cdn, mod_id, file_id, Some(dir)).await?;
        }
    }
    Ok(())
}

async fn download(
    agg: &Aggregator,
    cdn: &ForgeCdn,
    mod_id: u64,
    file_id: u64,
    out: Option<PathBuf>,
) -> Result<()> {
    let info = agg.file_info(mod_id, file_id).await
        .context("fetching file info")?;
    let target_dir = out.unwrap_or_else(|| std::env::current_dir().expect("cwd"));
    let target = if target_dir.is_dir() || !target_dir.exists() {
        if !target_dir.exists() { tokio::fs::create_dir_all(&target_dir).await?; }
        target_dir.join(&info.file_name)
    } else {
        target_dir
    };

    let url = agg.download_url(mod_id, file_id).await
        .context("resolving download url")?;
    println!("==> {} ({} bytes) {}", info.file_name, info.file_length, url);

    let pb = ProgressBar::new(info.file_length);
    pb.set_style(
        ProgressStyle::with_template("{bar:40.cyan/blue} {bytes}/{total_bytes} {bytes_per_sec} eta {eta}")
            .unwrap(),
    );
    let pb2 = pb.clone();
    cdn.download_to(&url, &target, Some(info.file_length), move |w, _t| {
        pb2.set_position(w);
    }).await.map_err(|e| anyhow::anyhow!("download: {e}"))?;
    pb.finish_and_clear();
    println!("OK -> {}", target.display());
    Ok(())
}
