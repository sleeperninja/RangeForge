//! Command-line options shared by the commands.

use std::path::PathBuf;

use clap::Args;
use rangeforge_audio::cache::Cache;
use rangeforge_session::paths;

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
            .or_else(paths::default_cache_dir)
            .map(Cache::new)
    }
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
        paths::presets_path(self.config_dir.clone())
    }
}
