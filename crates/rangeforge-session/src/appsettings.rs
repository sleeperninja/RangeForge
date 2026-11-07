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

use rangeforge_export::format::{BitDepth, Codec, Container, SampleRate};

use crate::controls::StrategySettings;
use crate::dsd::DsdFormat;
use crate::naming::NameSettings;
use crate::snapshot::{conflict_from_id, conflict_id, place_from_id, place_id};

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedSettings {
    pub naming: NameSettings,
    pub metadata: MetadataSettings,
    /// The File Strategy panel: where files go, what to do about a clash, deleting originals.
    pub strategy: StrategySettings,
    /// Ask before a run that overwrites files or deletes originals. On until the user ticks
    /// "don't ask again" in that question, and it can be turned back on in Settings.
    pub confirm_destructive: bool,
    /// What DSD files are written as.
    pub dsd: DsdFormat,
}

impl Default for SavedSettings {
    fn default() -> Self {
        Self {
            naming: NameSettings::default(),
            metadata: MetadataSettings::default(),
            strategy: StrategySettings::default(),
            confirm_destructive: true,
            dsd: DsdFormat::default(),
        }
    }
}

/// The File Strategy as written in the file.
#[derive(Debug, Serialize, Deserialize)]
struct StoredStrategy {
    place: String,
    subfolder: String,
    folder: String,
    conflict: String,
    number_suffix: String,
    old_suffix: String,
    delete_original: bool,
}

impl StoredStrategy {
    fn from(strategy: &StrategySettings) -> Self {
        Self {
            place: place_id(strategy.where_kind).to_string(),
            subfolder: strategy.subfolder.clone(),
            folder: strategy.folder.clone(),
            conflict: conflict_id(strategy.conflict_kind).to_string(),
            number_suffix: strategy.number_suffix.clone(),
            old_suffix: strategy.old_suffix.clone(),
            delete_original: strategy.delete_original,
        }
    }

    /// The strategy, with a plain-words note for anything that was not understood. A value
    /// that is not understood falls back to the safe default for that one setting.
    fn into_strategy(self, notes: &mut Vec<String>) -> StrategySettings {
        let defaults = StrategySettings::default();
        let where_kind = place_from_id(&self.place).unwrap_or_else(|| {
            notes.push(format!(
                "Unknown saved place {:?}; using the default",
                self.place
            ));
            defaults.where_kind
        });
        let conflict_kind = conflict_from_id(&self.conflict).unwrap_or_else(|| {
            notes.push(format!(
                "Unknown saved way of handling a taken name {:?}; using the default",
                self.conflict
            ));
            defaults.conflict_kind
        });
        StrategySettings {
            where_kind,
            subfolder: self.subfolder,
            folder: self.folder,
            conflict_kind,
            number_suffix: self.number_suffix,
            old_suffix: self.old_suffix,
            delete_original: self.delete_original,
        }
    }
}

/// The DSD Format as written in the file. `same` means "whatever the main format says".
#[derive(Debug, Serialize, Deserialize)]
struct StoredDsd {
    codec: String,
    container: String,
    sample_rate: String,
    bit_depth: String,
}

impl StoredDsd {
    fn from(dsd: DsdFormat) -> Self {
        Self {
            codec: dsd.codec.map_or("same", Codec::name).to_string(),
            container: dsd.container.map_or("same", Container::name).to_string(),
            sample_rate: dsd.sample_rate.name().to_string(),
            bit_depth: dsd.bit_depth.name().to_string(),
        }
    }

    fn into_dsd(self, notes: &mut Vec<String>) -> DsdFormat {
        let defaults = DsdFormat::default();
        let mut note = |what: &str, text: &str| {
            notes.push(format!(
                "Unknown saved DSD {what} {text:?}; using the default"
            ));
        };
        let codec = if self.codec == "same" {
            None
        } else {
            Codec::from_name(&self.codec).or_else(|| {
                note("codec", &self.codec);
                None
            })
        };
        let container = if self.container == "same" {
            None
        } else {
            Container::from_name(&self.container).or_else(|| {
                note("container", &self.container);
                None
            })
        };
        let sample_rate = SampleRate::from_name(&self.sample_rate)
            .filter(|rate| *rate != SampleRate::Original)
            .unwrap_or_else(|| {
                note("sample rate", &self.sample_rate);
                defaults.sample_rate
            });
        let bit_depth = BitDepth::from_name(&self.bit_depth)
            .filter(|depth| *depth != BitDepth::Source)
            .unwrap_or_else(|| {
                note("bit depth", &self.bit_depth);
                defaults.bit_depth
            });
        DsdFormat {
            codec,
            container,
            sample_rate,
            bit_depth,
        }
    }
}

fn yes() -> bool {
    true
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
    #[serde(default)]
    file_strategy: Option<StoredStrategy>,
    #[serde(default = "yes")]
    confirm_destructive: bool,
    #[serde(default)]
    dsd_format: Option<StoredDsd>,
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
            let mut notes = Vec::new();
            let settings = SavedSettings {
                naming: NameSettings {
                    album: stored.album_name,
                    single: stored.single_name,
                },
                metadata: MetadataSettings {
                    enabled: stored.metadata_enabled,
                    lastfm_enabled: stored.lastfm_enabled,
                },
                strategy: stored
                    .file_strategy
                    .map_or_else(StrategySettings::default, |s| s.into_strategy(&mut notes)),
                confirm_destructive: stored.confirm_destructive,
                dsd: stored
                    .dsd_format
                    .map_or_else(DsdFormat::default, |d| d.into_dsd(&mut notes)),
            };
            // Patterns that no longer make sense fall back to the defaults, with a note.
            match settings.naming.templates() {
                Ok(_) => Loaded { settings, notes },
                Err(problem) => {
                    notes.insert(
                        0,
                        format!(
                            "The saved file name patterns were not usable ({problem}), so the defaults are in use"
                        ),
                    );
                    Loaded {
                        settings: SavedSettings {
                            naming: NameSettings::default(),
                            ..settings
                        },
                        notes,
                    }
                }
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
        file_strategy: Some(StoredStrategy::from(&settings.strategy)),
        confirm_destructive: settings.confirm_destructive,
        dsd_format: Some(StoredDsd::from(settings.dsd)),
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
    use crate::controls::{ConflictKind, WhereKind};
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
            strategy: StrategySettings {
                where_kind: WhereKind::Subfolder,
                subfolder: "Balanced".to_string(),
                conflict_kind: ConflictKind::RenameExisting,
                old_suffix: "_before".to_string(),
                delete_original: true,
                ..StrategySettings::default()
            },
            confirm_destructive: false,
            dsd: DsdFormat {
                codec: Some(Codec::Flac),
                container: Some(Container::Mka),
                sample_rate: SampleRate::Hz176400,
                bit_depth: BitDepth::Sixteen,
            },
        }
    }

    #[test]
    fn the_file_strategy_the_question_and_the_dsd_format_are_kept() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        save(&path, &custom()).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.settings.strategy, custom().strategy);
        assert!(!loaded.settings.confirm_destructive);
        assert_eq!(loaded.settings.dsd, custom().dsd);
    }

    #[test]
    fn an_old_file_gets_the_safe_defaults_for_the_new_settings() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            br#"{"version": 1, "album_name": "{title}", "single_name": "{name}"}"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.settings.strategy, StrategySettings::default());
        assert!(loaded.settings.confirm_destructive, "asking is the default");
        assert_eq!(loaded.settings.dsd, DsdFormat::default());
    }

    #[test]
    fn values_that_are_not_understood_fall_back_one_by_one_with_notes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            br#"{"version": 1, "album_name": "{title}", "single_name": "{name}",
                "file_strategy": {"place": "moon", "subfolder": "X", "folder": "", "conflict": "number",
                    "number_suffix": "_00", "old_suffix": "_old", "delete_original": true},
                "dsd_format": {"codec": "same", "container": "same", "sample_rate": "original", "bit_depth": "24"}}"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert_eq!(
            loaded.settings.strategy.where_kind,
            WhereKind::NextToOriginal
        );
        assert!(loaded.settings.strategy.delete_original, "the rest is kept");
        assert_eq!(loaded.settings.dsd.sample_rate, SampleRate::Hz88200);
        assert_eq!(loaded.notes.len(), 2, "{:?}", loaded.notes);
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
