//! `dartscore` CLI entry point.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dartscore::{score_detection, Detection};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "dartscore",
    about = "Automatic steel-tip dart scoring",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Score a detection frame supplied as JSON (4 calibration points + tips).
    Score {
        /// Path to a JSON file matching the `Detection` schema.
        input: PathBuf,
    },
    /// Run YOLOv8 inference on an image (Phase 1; needs `--features detection`).
    Detect { image: PathBuf },
    /// Capture from the camera and score live (Phase 3).
    Capture,
    /// Serve the web UI + WebSocket game server (Phase 4).
    Serve,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Score { input } => cmd_score(&input),
        Command::Detect { image } => cmd_detect(&image),
        Command::Capture => {
            anyhow::bail!("`capture` is not implemented yet (Phase 3: camera)")
        }
        Command::Serve => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(dartscore::web::run())
        }
    }
}

#[cfg(feature = "detection")]
fn cmd_detect(image: &std::path::Path) -> Result<()> {
    dartscore::detect::run(image)
}

#[cfg(not(feature = "detection"))]
fn cmd_detect(_image: &std::path::Path) -> Result<()> {
    anyhow::bail!("detection is disabled; rebuild with `--features detection`")
}

fn cmd_score(input: &PathBuf) -> Result<()> {
    let raw = std::fs::read_to_string(input)
        .with_context(|| format!("reading detection file {}", input.display()))?;
    let det: Detection = serde_json::from_str(&raw).context("parsing detection JSON")?;
    let scores = score_detection(&det)?;

    let total: u32 = scores.iter().map(|s| s.value).sum();
    for (i, s) in scores.iter().enumerate() {
        println!("dart {}: {:?} {} -> {}", i + 1, s.ring, s.segment, s.value);
    }
    println!("total: {total}");
    Ok(())
}
