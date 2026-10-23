//! `rangeforge`: command line front end.
//!
//! This is a thin layer. Parsing, planning and measuring all live in the library crates;
//! this file only reads input and prints results.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use rangeforge_audio::measure::{self, Measured};
use rangeforge_core::cue::{self, CueSheet};

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
    /// Measure loudness, true peak and loudness range
    Measure {
        /// The audio file to measure
        audio: PathBuf,
        /// A CUE sheet describing the tracks inside the audio file
        #[arg(long)]
        cue: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Cue { path } => show_cue(&path),
        Command::Measure { audio, cue } => show_measurements(&audio, cue.as_deref()),
    }
}

fn read_sheet(path: &Path) -> Result<CueSheet> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    // Many CUE sheets are not UTF-8. Lossy decoding keeps them readable; proper
    // encoding detection comes later.
    let text = String::from_utf8_lossy(&bytes);
    cue::parse(&text).with_context(|| format!("parsing {}", path.display()))
}

fn show_cue(path: &Path) -> Result<()> {
    let sheet = read_sheet(path)?;

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

fn show_measurements(audio: &Path, cue_path: Option<&Path>) -> Result<()> {
    let started = Instant::now();
    let (labels, measured): (Vec<String>, Measured) = if let Some(cue_path) = cue_path {
        let sheet = read_sheet(cue_path)?;
        let [file] = sheet.files.as_slice() else {
            bail!(
                "this CUE sheet lists {} audio files; only single-file sheets are supported so far",
                sheet.files.len()
            );
        };
        let segments = file.segments();
        let labels = segments
            .iter()
            .map(|segment| {
                format!(
                    "{:>2}  {}",
                    segment.track.number,
                    segment.track.title.as_deref().unwrap_or("(untitled)")
                )
            })
            .collect();
        (labels, measure::measure_segments(audio, &segments)?)
    } else {
        let name = audio
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        (vec![name], measure::measure_whole_file(audio)?)
    };
    let elapsed = started.elapsed().as_secs_f64();

    println!("{:>9}  {:>10}  {:>8}", "LUFS", "TruePeak", "Range");
    for (label, track) in labels.iter().zip(&measured.tracks) {
        println!(
            "{:>9.2}  {:>6.2} dBTP  {:>5.2} LU  {label}",
            track.integrated_lufs, track.true_peak_dbtp, track.loudness_range_lu
        );
    }

    let audio_seconds = measured.duration_seconds();
    println!(
        "\n{audio_seconds:.1} s of audio ({} Hz, {} ch) decoded and measured in {elapsed:.2} s, {:.0}x real time",
        measured.sample_rate,
        measured.channels,
        audio_seconds / elapsed
    );
    Ok(())
}
