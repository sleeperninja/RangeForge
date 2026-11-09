//! `rangeforge run`: measure, plan each track's gain, and write the finished files.

use rangeforge_core::mix::MixPlan;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::Args;
use rangeforge_audio::cache::Cache;
use rangeforge_export::ffmpeg::Ffmpeg;
use rangeforge_export::format::OutputFormat;
use rangeforge_export::pipeline::{Options, Progress, export_file, preview};
use rangeforge_export::strategy::{Resolved, Strategy};
use rangeforge_session::dsd::DsdFormat;
use rangeforge_session::identity::identify_file;
use rangeforge_session::load::{Job, load};
use rangeforge_session::measuring::{measure_split, measure_whole};
use rangeforge_session::model::{Group, Settings, Status};
use rangeforge_session::naming::NameTemplates;
use rangeforge_session::plan::{Planned, plan_tracks, track_plans};

use crate::output::OutputArgs;
use crate::support::CacheOptions;

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
    #[command(flatten)]
    output: OutputArgs,
    /// Show what would be written, with every gain and file name, and write nothing
    #[arg(long)]
    dry_run: bool,
    /// Confirm a strategy that overwrites files
    #[arg(long)]
    yes: bool,
    #[command(flatten)]
    cache: CacheOptions,
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

    if let Some(problem) = options.output.format_problem() {
        bail!("{problem}");
    }
    let strategy = options.output.strategy().map_err(anyhow::Error::msg)?;
    if strategy.needs_confirmation() && !options.dry_run && !options.yes {
        bail!(
            "this overwrites files or deletes originals. Add --yes to confirm, or --dry-run to preview"
        );
    }
    let ffmpeg = if options.dry_run {
        None
    } else {
        Some(Ffmpeg::locate()?)
    };

    let names = options
        .output
        .naming_settings()
        .templates()
        .map_err(anyhow::Error::msg)?;
    let plan = RunPlan {
        settings: Settings::new(options.target, !options.no_peak_protection, options.ceiling),
        format: options.output.output_settings().format(),
        strategy: &strategy,
        names: &names,
    };
    let cache = options.cache.open();

    let started = Instant::now();
    let mut failures = 0_usize;
    let mut written_files = 0_usize;
    for job in &loaded.jobs {
        let group = &loaded.groups[job.group];
        match run_job(job, group, &plan, cache.as_ref(), ffmpeg.as_ref()) {
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

/// Everything that decides what is written, the same for every file in the run.
struct RunPlan<'a> {
    settings: Settings,
    format: OutputFormat,
    strategy: &'a Strategy,
    names: &'a NameTemplates,
}

fn run_job(
    job: &Job,
    group: &Group,
    plan: &RunPlan<'_>,
    cache: Option<&Cache>,
    ffmpeg: Option<&Ffmpeg>,
) -> Result<usize> {
    let settings = plan.settings;
    // A DSD source cannot be written as it is, so it gets the usual DSD Format (88.2 kHz, 24-bit)
    // wherever the output says original or source.
    let format = DsdFormat::default().format_for(plan.format, &job.audio);
    let (strategy, names) = (plan.strategy, plan.names);
    if let Status::Failed(message) = &group.status {
        bail!("{message}");
    }
    let (measured, _) = match &job.cue_file {
        Some(file) => measure_split(&job.audio, &file.segments(), MixPlan::default(), cache)?,
        None => measure_whole(&job.audio, MixPlan::default(), cache)?,
    };
    let planned = plan_tracks(job, &measured.tracks, settings.target());
    // A single file is named from its own tags, or failing that its file name.
    let identities = if job.cue_file.is_none() {
        vec![Some(identify_file(&job.audio))]
    } else {
        Vec::new()
    };
    let (plans, source_dir) = track_plans(job, &planned, format, names, &identities);
    let write_options = Options {
        format,
        strategy: strategy.clone(),
        directory: source_dir.clone(),
        copy_source_tags: job.cue_file.is_none(),
        mix: rangeforge_core::mix::MixPlan::default(),
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
        &mut |progress| match progress {
            Progress::Saved { index, path } => {
                println!("  saved {}/{}: {}", index + 1, plans.len(), path.display());
            }
            Progress::RemovedOriginal { path } => {
                println!("  deleted original: {}", path.display());
            }
            Progress::Encoding { .. } => {}
        },
    )
    .with_context(|| format!("writing {}", job.audio.display()))?;
    Ok(written.len())
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
