//! Settings that are not presets, kept in a small JSON file (`settings.json`): the file name
//! patterns and which outside lookups are switched on.
//!
//! API keys are not kept here. They live in the operating system's credential store (see
//! [`crate::secrets`]), so this file can be copied or shared without leaking one.
//!
//! Like the presets file, loading never fails. A missing file means the defaults, and a file
//! that cannot be read is set aside as `settings.json.bad` with a note, never overwritten.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::naming::NameSettings;

/// Bump this if the file's layout changes in a way older versions cannot read. Adding a field
/// with a default does not need it.
const VERSION: u32 = 1;

/// Which outside lookups are switched on. Everything starts off: nothing touches the network
/// until the user says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MetadataSettings {
    /// The "Metadata API" switch on the main screen. Without it, no lookups happen at all.
    pub enabled: bool,
    /// Use Last.fm for genres. Needs a saved key as well.
    pub lastfm_enabled: bool,
}

/// Everything this file keeps.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SavedSettings {
    pub naming: NameSettings,
    pub metadata: MetadataSettings,
}

#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    version: u32,
    album_name: String,
    single_name: String,
    #[serde(default)]
    metadata_enabled: bool,
    #[serde(default)]
    lastfm_enabled: bool,
}

/// What loading found.
#[derive(Debug)]
pub struct Loaded {
    pub settings: SavedSettings,
    /// Things worth telling the user, such as a damaged file that was set aside.
    pub notes: Vec<String>,
}

/// Reads the settings at `path`. See the module notes for what happens when it cannot.
pub fn load(path: &Path) -> Loaded {
    let defaults = |notes: Vec<String>| Loaded {
        settings: SavedSettings::default(),
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
            let settings = SavedSettings {
                naming: NameSettings {
                    album: stored.album_name,
                    single: stored.single_name,
                },
                metadata: MetadataSettings {
                    enabled: stored.metadata_enabled,
                    lastfm_enabled: stored.lastfm_enabled,
                },
            };
            // Patterns that no longer make sense fall back to the defaults, with a note.
            match settings.naming.templates() {
                Ok(_) => Loaded {
                    settings,
                    notes: Vec::new(),
                },
                Err(problem) => Loaded {
                    settings: SavedSettings {
                        naming: NameSettings::default(),
                        ..settings
                    },
                    notes: vec![format!(
                        "The saved file name patterns were not usable ({problem}), so the defaults are in use"
                    )],
                },
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
pub fn save(path: &Path, settings: &SavedSettings) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let stored = Stored {
        version: VERSION,
        album_name: settings.naming.album.clone(),
        single_name: settings.naming.single.clone(),
        metadata_enabled: settings.metadata.enabled,
        lastfm_enabled: settings.metadata.lastfm_enabled,
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

    fn custom() -> SavedSettings {
        SavedSettings {
            naming: NameSettings {
                album: "{artist} - {title}".to_string(),
                single: "{name} (balanced)".to_string(),
            },
            metadata: MetadataSettings {
                enabled: true,
                lastfm_enabled: false,
            },
        }
    }

    #[test]
    fn a_missing_file_means_the_defaults_with_lookups_off() {
        let dir = tempdir().unwrap();
        let loaded = load(&dir.path().join("settings.json"));
        assert_eq!(loaded.settings, SavedSettings::default());
        assert!(!loaded.settings.metadata.enabled);
        assert!(!loaded.settings.metadata.lastfm_enabled);
        assert_eq!(loaded.notes, Vec::<String>::new());
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("deeper").join("settings.json");
        save(&path, &custom()).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.settings, custom());
        assert_eq!(loaded.notes, Vec::<String>::new());
    }

    #[test]
    fn a_file_from_before_the_lookup_switches_still_loads_with_them_off() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            br#"{"version": 1, "album_name": "{title}", "single_name": "{name}"}"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.settings.naming.album, "{title}");
        assert_eq!(loaded.settings.metadata, MetadataSettings::default());
        assert_eq!(loaded.notes, Vec::<String>::new());
    }

    #[test]
    fn no_secret_is_ever_written_to_the_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        save(&path, &custom()).unwrap();
        let text = fs::read_to_string(&path).unwrap().to_lowercase();
        assert!(!text.contains("key"), "{text}");
        assert!(!text.contains("secret"), "{text}");
    }

    #[test]
    fn a_damaged_file_is_set_aside_not_overwritten() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"{ not json").unwrap();

        let loaded = load(&path);

        assert_eq!(loaded.settings, SavedSettings::default());
        assert!(loaded.notes[0].contains("settings.json.bad"));
        assert_eq!(
            fs::read(dir.path().join("settings.json.bad")).unwrap(),
            b"{ not json"
        );
        assert!(!path.exists());
    }

    #[test]
    fn patterns_that_no_longer_work_fall_back_but_the_switches_are_kept() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            br#"{"version": 1, "album_name": "{bogus}", "single_name": "{name}", "metadata_enabled": true}"#,
        )
        .unwrap();

        let loaded = load(&path);

        assert_eq!(loaded.settings.naming, NameSettings::default());
        assert!(loaded.settings.metadata.enabled);
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
