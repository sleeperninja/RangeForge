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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_folder_wins_for_presets() {
        let path = presets_path(Some(PathBuf::from("somewhere"))).unwrap();
        assert_eq!(path, PathBuf::from("somewhere").join("presets.json"));
    }
}
