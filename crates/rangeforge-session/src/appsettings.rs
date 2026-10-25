//! Settings that are not presets, kept in a small JSON file (`settings.json`): for now, the
//! file name patterns.
//!
//! Like the presets file, loading never fails. A missing file means the defaults, and a file
//! that cannot be read is set aside as `settings.json.bad` with a note, never overwritten.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::naming::NameSettings;

/// Bump this if the file's layout changes in a way older versions cannot read.
const VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    version: u32,
    album_name: String,
    single_name: String,
}

/// What loading found.
#[derive(Debug)]
pub struct Loaded {
    pub naming: NameSettings,
    /// Things worth telling the user, such as a damaged file that was set aside.
    pub notes: Vec<String>,
}

/// Reads the settings at `path`. See the module notes for what happens when it cannot.
pub fn load(path: &Path) -> Loaded {
    let defaults = |notes: Vec<String>| Loaded {
        naming: NameSettings::default(),
        notes,
    };
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return defaults(Vec::new()),
        Err(error) => {
            return defaults(vec![format!(
                "Could not read settings at {}: {error}. Using the defaults",
                path.display()
            )]);
        }
    };
    match serde_json::from_slice::<Stored>(&bytes) {
        Ok(stored) if stored.version == VERSION => {
            let naming = NameSettings {
                album: stored.album_name,
                single: stored.single_name,
            };
            // Patterns that no longer make sense fall back to the defaults, with a note.
            match naming.templates() {
                Ok(_) => Loaded {
                    naming,
                    notes: Vec::new(),
                },
                Err(problem) => defaults(vec![format!(
                    "The saved file name patterns were not usable ({problem}), so the defaults are in use"
                )]),
            }
        }
        _ => {
            let backup = path.with_extension("json.bad");
            let kept = fs::rename(path, &backup).is_ok();
            defaults(vec![if kept {
                format!(
                    "The settings file could not be read. It was kept as {} and the defaults are in use",
                    backup.display()
                )
            } else {
                format!(
                    "The settings file {} could not be read, so the defaults are in use",
                    path.display()
                )
            }])
        }
    }
}

/// Writes the settings to `path`, through a temporary file so a crash cannot leave a
/// half-written file behind.
pub fn save(path: &Path, naming: &NameSettings) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let stored = Stored {
        version: VERSION,
        album_name: naming.album.clone(),
        single_name: naming.single.clone(),
    };
    let json = serde_json::to_vec_pretty(&stored).context("writing settings")?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, json).with_context(|| format!("writing {}", temporary.display()))?;
    fs::rename(&temporary, path).with_context(|| format!("saving {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn custom() -> NameSettings {
        NameSettings {
            album: "{artist} - {title}".to_string(),
            single: "{name} (balanced)".to_string(),
        }
    }

    #[test]
    fn a_missing_file_means_the_defaults() {
        let dir = tempdir().unwrap();
        let loaded = load(&dir.path().join("settings.json"));
        assert_eq!(loaded.naming, NameSettings::default());
        assert_eq!(loaded.notes, Vec::<String>::new());
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("deeper").join("settings.json");
        save(&path, &custom()).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.naming, custom());
        assert_eq!(loaded.notes, Vec::<String>::new());
    }

    #[test]
    fn a_damaged_file_is_set_aside_not_overwritten() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"{ not json").unwrap();

        let loaded = load(&path);

        assert_eq!(loaded.naming, NameSettings::default());
        assert!(loaded.notes[0].contains("settings.json.bad"));
        assert_eq!(
            fs::read(dir.path().join("settings.json.bad")).unwrap(),
            b"{ not json"
        );
        assert!(!path.exists());
    }

    #[test]
    fn patterns_that_no_longer_work_fall_back_with_a_note() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            br#"{"version": 1, "album_name": "{bogus}", "single_name": "{name}"}"#,
        )
        .unwrap();

        let loaded = load(&path);

        assert_eq!(loaded.naming, NameSettings::default());
        assert!(loaded.notes[0].contains("not usable"), "{:?}", loaded.notes);
        assert!(path.exists(), "a readable file is left where it is");
    }

    #[test]
    fn saving_leaves_no_temporary_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        save(&path, &custom()).unwrap();
        let names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["settings.json"]);
    }
}
