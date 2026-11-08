//! Where the app keeps its files on this computer.

use std::path::PathBuf;

use directories::ProjectDirs;

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", "RangeForge")
}

/// The per-user cache folder for measurements, such as `%LOCALAPPDATA%\RangeForge\cache\analysis`
/// on Windows.
pub fn default_cache_dir() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.cache_dir().join("analysis"))
}

/// The per-user folder for saved settings, such as `%APPDATA%\RangeForge\config` on Windows.
pub fn default_config_dir() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.config_dir().to_path_buf())
}

/// Where presets are kept: in `config_dir` if given, otherwise in the standard settings folder.
pub fn presets_path(config_dir: Option<PathBuf>) -> Option<PathBuf> {
    config_dir
        .or_else(default_config_dir)
        .map(|dir| dir.join("presets.json"))
}

/// Where the small local copy of the Billboard Hot 100 history is kept, next to the
/// measurements cache.
pub fn hot100_path() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.cache_dir().join("hot100.json"))
}

/// Where what Last.fm has said about albums and songs is remembered, next to the Hot 100 copy.
pub fn lastfm_cache_path() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.cache_dir().join("lastfm.json"))
}

/// Where song tags pasted from Last.fm's tags page are kept. That is the user's own work, so it
/// lives with the settings and not in the cache, which may be cleared at any time.
pub fn song_tags_path() -> Option<PathBuf> {
    default_config_dir().map(|dir| dir.join("song-tags.json"))
}

/// Where the other settings (such as the file name patterns) are kept.
pub fn settings_path(config_dir: Option<PathBuf>) -> Option<PathBuf> {
    config_dir
        .or_else(default_config_dir)
        .map(|dir| dir.join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_folder_wins_for_presets() {
        let path = presets_path(Some(PathBuf::from("somewhere"))).unwrap();
        assert_eq!(path, PathBuf::from("somewhere").join("presets.json"));
    }

    #[test]
    fn settings_sit_beside_the_presets() {
        let path = settings_path(Some(PathBuf::from("somewhere"))).unwrap();
        assert_eq!(path, PathBuf::from("somewhere").join("settings.json"));
    }
}
