//! `rangeforge`: command line front end.
//!
//! This is a thin layer. Parsing, planning and (later) measuring all live in
//! `rangeforge-core`; this file only reads input and prints results.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rangeforge_core::cue;

#[derive(Debug, Parser)]
#[command(
    name = "rangeforge",
    version,
    about = "Volume balancing, CUE splitting and tagging for audio libraries"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Parse a CUE sheet and list the tracks it describes
    Cue {
        /// Path to the .cue file
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Cue { path } => show_cue(&path),
    }
}

fn show_cue(path: &Path) -> Result<()> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    // Many CUE sheets are not UTF-8. Lossy decoding keeps them readable; proper
    // encoding detection comes later.
    let text = String::from_utf8_lossy(&bytes);
    let sheet = cue::parse(&text).with_context(|| format!("parsing {}", path.display()))?;

    let performer = sheet.performer.as_deref().unwrap_or("(unknown artist)");
    let title = sheet.title.as_deref().unwrap_or("(untitled)");
    println!("{performer} - {title}");

    for file in &sheet.files {
        println!("\n{}", file.name);
        for segment in file.segments() {
            let end = segment
                .end
                .map_or_else(|| "end of file".to_string(), |end| end.to_string());
            println!(
                "  {:>2}  {}  ->  {}  {}",
                segment.track.number,
                segment.start,
                end,
                segment.track.title.as_deref().unwrap_or("(untitled)")
            );
        }
    }
    Ok(())
}
