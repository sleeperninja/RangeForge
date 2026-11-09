//! `rangeforge`: command line front end.
//!
//! This is a thin layer. Parsing, planning and measuring all live in the library crates;
//! this file only reads input and prints results.

mod output;
mod run;
mod support;
mod ui;

use rangeforge_core::mix::MixPlan;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use rangeforge_audio::cache::{Cache, Source};
use rangeforge_audio::measure::Measured;

use rangeforge_session::measuring::{measure_split, measure_whole, read_sheet};
use rangeforge_session::model::Settings;
use support::CacheOptions;

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
        #[command(flatten)]
        cache: CacheOptions,
    },
    /// Open the interactive view: a table of measurements with live volume targets
    Ui {
        /// Audio files, CUE sheets, or folders containing them
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Starting target loudness in LUFS
        #[arg(long, default_value_t = -22.0, allow_negative_numbers = true)]
        target: f64,
        /// Starting peak ceiling in dBTP
        #[arg(long, default_value_t = -0.6, allow_negative_numbers = true)]
        ceiling: f64,
        /// Start with peak protection off
        #[arg(long)]
        no_peak_protection: bool,
        /// Start with the log panel open
        #[arg(long)]
        show_logs: bool,
        /// Print one frame as text and exit, instead of opening the interactive view
        #[arg(long)]
        snapshot: bool,
        /// Width of the snapshot in characters
        #[arg(long, default_value_t = 130)]
        width: u16,
        /// Height of the snapshot in lines
        #[arg(long, default_value_t = 30)]
        height: u16,
        /// With --snapshot: press Run once measuring is done, wait for the files to be
        /// written, and show the result
        #[arg(long, requires = "snapshot")]
        run: bool,
        /// With --run: confirm a File Strategy that overwrites files
        #[arg(long, requires = "run")]
        yes: bool,
        #[command(flatten)]
        output: output::OutputArgs,
        #[command(flatten)]
        config: support::ConfigOptions,
        #[command(flatten)]
        cache: CacheOptions,
    },
    /// Balance volume, split CUE albums, tag, and write the finished files
    Run(run::RunOptions),
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Run(options) => run::run(&options),
        Command::Cue { path } => show_cue(&path),
        Command::Measure { audio, cue, cache } => {
            show_measurements(&audio, cue.as_deref(), cache.open().as_ref())
        }
        Command::Ui {
            paths,
            target,
            ceiling,
            no_peak_protection,
            show_logs,
            snapshot,
            width,
            height,
            run,
            yes,
            output,
            config,
            cache,
        } => {
            let options = ui::Options {
                paths,
                settings: Settings::new(target, !no_peak_protection, ceiling),
                output: output.output_settings(),
                strategy: output.strategy_settings(),
                naming: output.naming_settings(),
                cache: cache.open(),
                show_log: show_logs,
                presets_path: config.presets_path(),
                run,
                yes,
            };
            if snapshot {
                println!("{}", ui::snapshot(&options, width, height)?);
                Ok(())
            } else {
                ui::run(&options)
            }
        }
    }
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

fn show_measurements(audio: &Path, cue_path: Option<&Path>, cache: Option<&Cache>) -> Result<()> {
    let started = Instant::now();
    let (labels, (measured, source)): (Vec<String>, (Measured, Source)) = if let Some(cue_path) =
        cue_path
    {
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
        (
            labels,
            measure_split(audio, &segments, MixPlan::default(), cache)?,
        )
    } else {
        let name = audio
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        (vec![name], measure_whole(audio, MixPlan::default(), cache)?)
    };
    let elapsed = started.elapsed().as_secs_f64();

    println!("{:>9}  {:>10}  {:>11}", "LUFS", "TruePeak", "Range (LRA)");
    for (label, track) in labels.iter().zip(&measured.tracks) {
        println!(
            "{:>9.2}  {:>6.2} dBTP  {:>8.2} LU  {label}",
            track.integrated_lufs, track.true_peak_dbtp, track.loudness_range_lu
        );
    }

    let audio_seconds = measured.duration_seconds();
    let details = format!("{} Hz, {} ch", measured.sample_rate, measured.channels);
    match source {
        Source::Cache => println!(
            "\n{audio_seconds:.1} s of audio ({details}): results came from the cache in {elapsed:.2} s"
        ),
        Source::Decoded { save_error } => {
            println!(
                "\n{audio_seconds:.1} s of audio ({details}) decoded and measured in {elapsed:.2} s, {:.0}x real time",
                audio_seconds / elapsed
            );
            if let Some(error) = save_error {
                eprintln!("warning: the result could not be saved to the cache: {error}");
            }
        }
    }
    Ok(())
}
