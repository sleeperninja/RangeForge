//! `rangeforge run`: measure, plan each track's gain, and write the finished files.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use rangeforge_core::planner::{GainPlan, Measurement, PlanError, plan_gain};
use rangeforge_export::ffmpeg::Ffmpeg;
use rangeforge_export::format::{BitDepth, Codec, OutputFormat};
use rangeforge_export::naming::{converted_file_name, track_file_name};
use rangeforge_export::pipeline::{Options, Progress, TrackPlan, export_file, preview};
use rangeforge_export::strategy::{Resolved, Strategy};
use rangeforge_export::tags::Tags;

use crate::support::{CacheOptions, measure_split, measure_whole};
use crate::ui::load::{Job, load};
use crate::ui::model::{Group, Settings, Status};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CodecArg {
    Alac,
    Flac,
    Wav,
    Aac,
}

impl From<CodecArg> for Codec {
    fn from(arg: CodecArg) -> Self {
        match arg {
            CodecArg::Alac => Self::Alac,
            CodecArg::Flac => Self::Flac,
            CodecArg::Wav => Self::Wav,
            CodecArg::Aac => Self::Aac,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BitDepthArg {
    /// Match the source file
    Source,
    #[value(name = "16")]
    Sixteen,
    #[value(name = "24")]
    TwentyFour,
}

impl From<BitDepthArg> for BitDepth {
    fn from(arg: BitDepthArg) -> Self {
        match arg {
            BitDepthArg::Source => Self::Source,
            BitDepthArg::Sixteen => Self::Sixteen,
            BitDepthArg::TwentyFour => Self::TwentyFour,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum StrategyArg {
    /// New files get a serial suffix such as _00; nothing existing is touched
    Serialize,
    /// An existing file at the same name is renamed with the suffix _old
    RenameOld,
    /// A file at exactly the same path is overwritten (needs --yes)
    Replace,
}

#[derive(Debug, Args)]
pub struct RunOptions {
    /// Audio files, CUE sheets, or folders containing them
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// Target loudness in LUFS
    #[arg(long, default_value_t = -22.0, allow_negative_numbers = true)]
    target: f64,
    /// Peak ceiling in dBTP
    #[arg(long, default_value_t = -0.6, allow_negative_numbers = true)]
    ceiling: f64,
    /// Do not cap the gain to keep the true peak under the ceiling
    #[arg(long)]
    no_peak_protection: bool,
    /// The format to write
    #[arg(long, value_enum, default_value = "alac")]
    codec: CodecArg,
    /// How precisely to store each sample
    #[arg(long, value_enum, default_value = "source")]
    bit_depth: BitDepthArg,
    /// What to do about files that already exist
    #[arg(long, value_enum, default_value = "serialize", conflicts_with = "out")]
    strategy: StrategyArg,
    /// Suffix for --strategy serialize (default _00) or rename-old (default _old)
    #[arg(long, conflicts_with = "out")]
    suffix: Option<String>,
    /// Write into this folder instead of next to the sources
    #[arg(long)]
    out: Option<PathBuf>,
    /// Show what would be written, with every gain and file name, and write nothing
    #[arg(long)]
    dry_run: bool,
    /// Confirm a strategy that overwrites files
    #[arg(long)]
    yes: bool,
    #[command(flatten)]
    cache: CacheOptions,
}

impl RunOptions {
    fn strategy(&self) -> Strategy {
        if let Some(directory) = &self.out {
            return Strategy::SaveTo {
                directory: directory.clone(),
            };
        }
        match self.strategy {
            StrategyArg::Serialize => Strategy::Serialize {
                suffix: self
                    .suffix
                    .clone()
                    .unwrap_or_else(|| Strategy::DEFAULT_SERIAL.to_string()),
            },
            StrategyArg::RenameOld => Strategy::RenameOld {
                suffix: self
                    .suffix
                    .clone()
                    .unwrap_or_else(|| Strategy::DEFAULT_OLD.to_string()),
            },
            StrategyArg::Replace => Strategy::Replace,
        }
    }
}

/// One track with its measurement and the gain chosen for it.
struct Planned {
    label: String,
    measurement: Measurement,
    gain: Result<GainPlan, PlanError>,
}

pub fn run(options: &RunOptions) -> Result<()> {
    let loaded = load(&options.paths);
    for line in &loaded.log {
        if ["Skipped", "Could not", "Failed"]
            .iter()
            .any(|p| line.starts_with(p))
        {
            eprintln!("warning: {line}");
        }
    }
    if loaded.jobs.is_empty() {
        bail!("nothing to do: no audio files or CUE sheets were found");
    }

    let strategy = options.strategy();
    if matches!(strategy, Strategy::Replace) && !options.dry_run && !options.yes {
        bail!("--strategy replace overwrites files. Add --yes to confirm, or --dry-run to preview");
    }
    let ffmpeg = if options.dry_run {
        None
    } else {
        Some(Ffmpeg::locate()?)
    };

    let settings = Settings::new(options.target, !options.no_peak_protection, options.ceiling);
    let format = OutputFormat::new(options.codec.into(), options.bit_depth.into());
    let cache = options.cache.open();

    let started = Instant::now();
    let mut failures = 0_usize;
    let mut written_files = 0_usize;
    for job in &loaded.jobs {
        let group = &loaded.groups[job.group];
        match run_job(
            job,
            group,
            settings,
            format,
            &strategy,
            cache.as_ref(),
            ffmpeg.as_ref(),
        ) {
            Ok(count) => written_files += count,
            Err(error) => {
                eprintln!("error: {}: {error:#}", group.name);
                failures += 1;
            }
        }
    }

    if options.dry_run {
        println!("\nDry run: nothing was written.");
    } else {
        println!(
            "\nWrote {written_files} file(s) in {:.1} s.",
            started.elapsed().as_secs_f64()
        );
    }
    if failures > 0 {
        bail!("{failures} file(s) failed");
    }
    Ok(())
}

fn run_job(
    job: &Job,
    group: &Group,
    settings: Settings,
    format: OutputFormat,
    strategy: &Strategy,
    cache: Option<&rangeforge_audio::cache::Cache>,
    ffmpeg: Option<&Ffmpeg>,
) -> Result<usize> {
    if let Status::Failed(message) = &group.status {
        bail!("{message}");
    }
    let planned = measure_and_plan(job, settings, cache)?;
    let (plans, source_dir) = track_plans(job, &planned, format);
    let write_options = Options {
        format,
        strategy: strategy.clone(),
        directory: source_dir.clone(),
        copy_source_tags: job.cue_file.is_none(),
    };
    let destinations = preview(&write_options, &plans)?;
    let folder = strategy.directory(&source_dir);

    println!("\n{}", group.name);
    println!("  into {}", folder.display());
    for (planned, destination) in planned.iter().zip(&destinations) {
        println!("  {}", describe_track(planned, destination, &folder));
    }

    let Some(ffmpeg) = ffmpeg else {
        return Ok(0);
    };
    let segments = job
        .cue_file
        .as_ref()
        .map(rangeforge_core::cue::CueFile::segments);
    let written = export_file(
        ffmpeg,
        &job.audio,
        segments.as_deref(),
        &plans,
        &write_options,
        &mut |progress| {
            if let Progress::Saved { index, path } = progress {
                println!("  saved {}/{}: {}", index + 1, plans.len(), path.display());
            }
        },
    )
    .with_context(|| format!("writing {}", job.audio.display()))?;
    Ok(written.len())
}

/// Measures the file (using the cache) and works out each track's gain.
fn measure_and_plan(
    job: &Job,
    settings: Settings,
    cache: Option<&rangeforge_audio::cache::Cache>,
) -> Result<Vec<Planned>> {
    let (measured, _) = match &job.cue_file {
        Some(file) => measure_split(&job.audio, &file.segments(), cache)?,
        None => measure_whole(&job.audio, cache)?,
    };
    let labels: Vec<String> = match &job.cue_file {
        Some(file) => file
            .tracks
            .iter()
            .map(|t| {
                format!(
                    "{} - {}",
                    t.number,
                    t.title.as_deref().unwrap_or("(untitled)")
                )
            })
            .collect(),
        None => vec![file_stem(&job.audio)],
    };
    Ok(labels
        .into_iter()
        .zip(measured.tracks)
        .map(|(label, measurement)| Planned {
            label,
            measurement,
            gain: plan_gain(measurement, settings.target()),
        })
        .collect())
}

/// What to write for each track, and the folder the sources live in.
fn track_plans(job: &Job, planned: &[Planned], format: OutputFormat) -> (Vec<TrackPlan>, PathBuf) {
    let extension = format.codec.extension();
    let source_dir = job
        .audio
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    // A track that cannot be measured (silence) is passed through untouched.
    let gain_of = |planned: &Planned| planned.gain.as_ref().map_or(0.0, |plan| plan.gain_db);

    let plans = match (&job.cue_file, &job.sheet) {
        (Some(file), Some(sheet)) => file
            .tracks
            .iter()
            .zip(planned)
            .map(|(track, planned)| TrackPlan {
                gain_db: gain_of(planned),
                file_name: track_file_name(track.number, track.title.as_deref(), extension),
                tags: Tags::for_cue_track(sheet, track, file.tracks.len()),
            })
            .collect(),
        _ => planned
            .iter()
            .map(|planned| TrackPlan {
                gain_db: gain_of(planned),
                file_name: converted_file_name(&file_stem(&job.audio), extension),
                tags: Tags::default(),
            })
            .collect(),
    };
    (plans, source_dir)
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
}

/// One line of the plan: the song, its loudness, its gain, and where it goes.
fn describe_track(planned: &Planned, destination: &Resolved, folder: &Path) -> String {
    let gain = match &planned.gain {
        Ok(plan) => {
            let star = if plan.limited_by_peak { "*" } else { "" };
            format!("{:+.2} dB{star}", plan.gain_db)
        }
        Err(_) => "no change (cannot measure)".to_string(),
    };
    let name = |path: &Path| {
        path.strip_prefix(folder)
            .unwrap_or(path)
            .display()
            .to_string()
    };
    let mut line = format!(
        "{:<34} {:>8.2} LUFS  {:>16}  -> {}",
        planned.label,
        planned.measurement.integrated_lufs,
        gain,
        name(&destination.final_path)
    );
    if destination.overwrites {
        line.push_str("  (overwrites the existing file)");
    }
    if let Some(old) = &destination.move_existing_to {
        line.push_str("  (existing file becomes ");
        line.push_str(&name(old));
        line.push(')');
    }
    line
}
