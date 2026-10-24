//! Helpers shared by the commands.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use directories::ProjectDirs;
use rangeforge_audio::cache::{Cache, Source};
use rangeforge_audio::measure::{self, Measured};
use rangeforge_core::cue::{self, CueSheet, Segment};

/// Options that decide where measurements are remembered.
#[derive(Debug, Args)]
pub struct CacheOptions {
    /// Always decode the files; neither read nor write the cache
    #[arg(long)]
    pub no_cache: bool,
    /// Keep the cache here instead of in the standard cache folder
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,
}

impl CacheOptions {
    /// The cache to use, or `None` if caching is switched off or no folder can be found.
    pub fn open(&self) -> Option<Cache> {
        if self.no_cache {
            return None;
        }
        self.cache_dir
            .clone()
            .or_else(default_cache_dir)
            .map(Cache::new)
    }
}

/// The per-user cache folder for this app, such as `%LOCALAPPDATA%\RangeForge\cache\analysis`
/// on Windows.
fn default_cache_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "RangeForge").map(|dirs| dirs.cache_dir().join("analysis"))
}

/// Options that decide where saved settings, such as presets, are kept.
#[derive(Debug, Args)]
pub struct ConfigOptions {
    /// Keep saved settings here instead of in the standard config folder
    #[arg(long)]
    pub config_dir: Option<PathBuf>,
}

impl ConfigOptions {
    /// Where the presets file lives, or `None` if no folder can be found.
    pub fn presets_path(&self) -> Option<PathBuf> {
        self.config_dir
            .clone()
            .or_else(default_config_dir)
            .map(|dir| dir.join("presets.json"))
    }
}

/// The per-user config folder for this app, such as `%APPDATA%\RangeForge\config` on Windows.
fn default_config_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "RangeForge").map(|dirs| dirs.config_dir().to_path_buf())
}

/// Reads and parses a CUE sheet.
pub fn read_sheet(path: &Path) -> Result<CueSheet> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    // Many CUE sheets are not UTF-8. Lossy decoding keeps them readable; proper
    // encoding detection comes later.
    let text = String::from_utf8_lossy(&bytes);
    cue::parse(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Measures a whole file as one track.
pub fn measure_whole(audio: &Path, cache: Option<&Cache>) -> Result<(Measured, Source)> {
    Ok(match cache {
        Some(cache) => cache.measure_whole_file(audio)?,
        None => (
            measure::measure_whole_file(audio)?,
            Source::Decoded { save_error: None },
        ),
    })
}

/// Measures each CUE segment of a file.
pub fn measure_split(
    audio: &Path,
    segments: &[Segment<'_>],
    cache: Option<&Cache>,
) -> Result<(Measured, Source)> {
    Ok(match cache {
        Some(cache) => cache.measure_segments(audio, segments)?,
        None => (
            measure::measure_segments(audio, segments)?,
            Source::Decoded { save_error: None },
        ),
    })
}
