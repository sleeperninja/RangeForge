//! Settings that are not presets, kept in a small JSON file (`settings.json`): the file name
//! patterns, which outside lookups are switched on, the File Strategy, and what the main
//! screen was last set to (Output Format and Volume), so the program opens as it was left.
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

use rangeforge_export::format::{
    BitDepth, Bitrate, Codec, Container, DEFAULT_FLAC_LEVEL, SampleRate,
};

use rangeforge_core::mix::{Channels, Downmix, MixPlan};

use crate::controls::{OutputSettings, StrategySettings};
use crate::dsd::DsdFormat;
use crate::model::Settings;
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

/// The tag for songs that charted on the Billboard Hot 100.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioSettings {
    /// Whether charted songs get the tag at all.
    pub enabled: bool,
    /// The tag's text. `Radio` unless the user chose another.
    pub tag: String,
    /// A song gets the tag when its best chart position is this or better (1 to 100).
    pub threshold: u16,
}

impl RadioSettings {
    pub const DEFAULT_TAG: &'static str = "Radio";
    pub const MAX_TAG_LEN: usize = 30;

    /// Tidies a typed tag: commas and semicolons would split it in the genre tag, so they
    /// become spaces; blanks collapse; an empty result is the default tag.
    #[must_use]
    pub fn clean_tag(text: &str) -> String {
        let spaced = text.replace([',', ';'], " ");
        let tidy = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
        let tidy: String = tidy.chars().take(Self::MAX_TAG_LEN).collect();
        if tidy.is_empty() {
            Self::DEFAULT_TAG.to_string()
        } else {
            tidy
        }
    }

    /// Keeps a threshold inside 1 to 100.
    #[must_use]
    pub fn clamp_threshold(value: i64) -> u16 {
        u16::try_from(value.clamp(1, 100)).unwrap_or(100)
    }
}

impl Default for RadioSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            tag: Self::DEFAULT_TAG.to_string(),
            threshold: 100,
        }
    }
}

/// Everything this file keeps.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedSettings {
    pub naming: NameSettings,
    pub metadata: MetadataSettings,
    /// The Hot 100 tag: on or off, its text, and how high a song must have charted.
    pub radio: RadioSettings,
    /// The File Strategy panel: where files go, what to do about a clash, deleting originals.
    pub strategy: StrategySettings,
    /// Ask before a run that overwrites files or deletes originals. On until the user ticks
    /// "don't ask again" in that question, and it can be turned back on in Settings.
    pub confirm_destructive: bool,
    /// What DSD files are written as.
    pub dsd: DsdFormat,
    /// The Output Format panel as it was last left: codec, container, sample rate, bit depth.
    pub output: OutputSettings,
    /// The Volume Adjustment panel as it was last left: target, peak protection, ceiling.
    pub volume: Settings,
}

impl Default for SavedSettings {
    fn default() -> Self {
        Self {
            naming: NameSettings::default(),
            metadata: MetadataSettings::default(),
            radio: RadioSettings::default(),
            strategy: StrategySettings::default(),
            confirm_destructive: true,
            dsd: DsdFormat::default(),
            output: OutputSettings::default(),
            volume: Settings::default(),
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

/// The DSD Format as written in the file: the sample rate and bit depth to use where the main
/// Output Format says source. (An older file may also hold a codec and a container; they are
/// ignored, since DSD follows the main Output Format for those.)
#[derive(Debug, Serialize, Deserialize)]
struct StoredDsd {
    sample_rate: String,
    bit_depth: String,
}

impl StoredDsd {
    fn from(dsd: DsdFormat) -> Self {
        Self {
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
            sample_rate,
            bit_depth,
        }
    }
}

/// The Output Format panel as written in the file.
#[derive(Debug, Serialize, Deserialize)]
struct StoredOutput {
    codec: String,
    container: String,
    sample_rate: String,
    bit_depth: String,
    /// Added later: a file saved before then leaves the channels as they are. Each has its own
    /// default, so an older file still loads.
    #[serde(default = "original_channels")]
    channels: String,
    #[serde(default = "standard_downmix")]
    downmix: String,
    /// Added later: the lossy bitrate (`default` is the codec's usual one) and FLAC's level.
    #[serde(default = "usual_bitrate")]
    bitrate: String,
    #[serde(default = "usual_flac_level")]
    flac_level: u8,
}

fn usual_bitrate() -> String {
    "default".to_string()
}

fn usual_flac_level() -> u8 {
    DEFAULT_FLAC_LEVEL
}

fn original_channels() -> String {
    Channels::Original.name().to_string()
}

fn standard_downmix() -> String {
    Downmix::Standard.name().to_string()
}

impl StoredOutput {
    fn from(output: OutputSettings) -> Self {
        Self {
            codec: output.codec.name().to_string(),
            container: output.container.name().to_string(),
            sample_rate: output.sample_rate.name().to_string(),
            bit_depth: output.bit_depth.name().to_string(),
            channels: output.mix.channels.name().to_string(),
            downmix: output.mix.style.name().to_string(),
            bitrate: output.bitrate.name(),
            flac_level: output.flac_level,
        }
    }

    /// The panel, with a plain-words note for anything not understood. Each value that is not
    /// understood falls back to the default for that one setting, and a sample rate the codec
    /// cannot write goes back to the original's.
    fn into_output(self, notes: &mut Vec<String>) -> OutputSettings {
        let defaults = OutputSettings::default();
        let mut note = |what: &str, text: &str| {
            notes.push(format!(
                "Unknown saved output {what} {text:?}; using the default"
            ));
        };
        let codec = Codec::from_name(&self.codec).unwrap_or_else(|| {
            note("codec", &self.codec);
            defaults.codec
        });
        let container = Container::from_name(&self.container).unwrap_or_else(|| {
            note("container", &self.container);
            defaults.container
        });
        let sample_rate = SampleRate::from_name(&self.sample_rate).unwrap_or_else(|| {
            note("sample rate", &self.sample_rate);
            defaults.sample_rate
        });
        let bit_depth = BitDepth::from_name(&self.bit_depth).unwrap_or_else(|| {
            note("bit depth", &self.bit_depth);
            defaults.bit_depth
        });
        let channels = Channels::from_name(&self.channels).unwrap_or_else(|| {
            note("channels", &self.channels);
            Channels::Original
        });
        let style = Downmix::from_name(&self.downmix).unwrap_or_else(|| {
            note("downmix", &self.downmix);
            Downmix::Standard
        });
        let bitrate = if self.bitrate == "default" {
            codec.default_bitrate()
        } else {
            Bitrate::from_name(&self.bitrate)
                .filter(|rate| codec.is_lossless() || codec.bitrates().contains(rate))
                .unwrap_or_else(|| {
                    note("bitrate", &self.bitrate);
                    codec.default_bitrate()
                })
        };
        let mut output = OutputSettings {
            codec,
            bit_depth,
            container,
            sample_rate,
            mix: MixPlan { channels, style },
            bitrate,
            flac_level: self.flac_level.min(8),
        };
        if !output.can_write_mix() {
            output.mix.channels = Channels::Original;
            notes.push(format!(
                "The saved channels (quad) cannot be written as {}, so they are back to original",
                codec.name()
            ));
        }
        if output.fit_to_codec() {
            notes.push(format!(
                "The saved sample rate cannot be written as {}, so it is back to original",
                codec.name()
            ));
        }
        output
    }
}

/// The Volume Adjustment panel as written in the file.
#[derive(Debug, Serialize, Deserialize)]
struct StoredVolume {
    target_lufs: f64,
    peak_protection: bool,
    peak_ceiling_dbtp: f64,
}

impl StoredVolume {
    fn from(volume: Settings) -> Self {
        Self {
            target_lufs: volume.target_lufs,
            peak_protection: volume.peak_protection,
            peak_ceiling_dbtp: volume.peak_ceiling_dbtp,
        }
    }

    /// The panel, with numbers kept inside their allowed ranges.
    fn into_volume(self) -> Settings {
        Settings::new(
            self.target_lufs,
            self.peak_protection,
            self.peak_ceiling_dbtp,
        )
    }
}

fn yes() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
struct Stored {
    version: u32,
    album_name: String,
    single_name: String,
    #[serde(default)]
    metadata_enabled: bool,
    #[serde(default)]
    lastfm_enabled: bool,
    #[serde(default = "yes")]
    radio_enabled: bool,
    #[serde(default)]
    radio_tag: Option<String>,
    #[serde(default)]
    radio_threshold: Option<i64>,
    #[serde(default)]
    file_strategy: Option<StoredStrategy>,
    #[serde(default = "yes")]
    confirm_destructive: bool,
    #[serde(default)]
    dsd_format: Option<StoredDsd>,
    #[serde(default)]
    output_format: Option<StoredOutput>,
    #[serde(default)]
    volume: Option<StoredVolume>,
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
                radio: RadioSettings {
                    enabled: stored.radio_enabled,
                    tag: stored.radio_tag.map_or_else(
                        || RadioSettings::DEFAULT_TAG.to_string(),
                        |t| RadioSettings::clean_tag(&t),
                    ),
                    threshold: stored
                        .radio_threshold
                        .map_or(100, RadioSettings::clamp_threshold),
                },
                strategy: stored
                    .file_strategy
                    .map_or_else(StrategySettings::default, |s| s.into_strategy(&mut notes)),
                confirm_destructive: stored.confirm_destructive,
                dsd: stored
                    .dsd_format
                    .map_or_else(DsdFormat::default, |d| d.into_dsd(&mut notes)),
                output: stored
                    .output_format
                    .map_or_else(OutputSettings::default, |o| o.into_output(&mut notes)),
                volume: stored
                    .volume
                    .map_or_else(Settings::default, StoredVolume::into_volume),
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
        radio_enabled: settings.radio.enabled,
        radio_tag: Some(settings.radio.tag.clone()),
        radio_threshold: Some(i64::from(settings.radio.threshold)),
        file_strategy: Some(StoredStrategy::from(&settings.strategy)),
        confirm_destructive: settings.confirm_destructive,
        dsd_format: Some(StoredDsd::from(settings.dsd)),
        output_format: Some(StoredOutput::from(settings.output)),
        volume: Some(StoredVolume::from(settings.volume)),
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
            radio: RadioSettings {
                enabled: false,
                tag: "Hit".to_string(),
                threshold: 40,
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
                sample_rate: SampleRate::Hz176400,
                bit_depth: BitDepth::Sixteen,
            },
            output: OutputSettings {
                codec: Codec::Flac,
                bit_depth: BitDepth::TwentyFour,
                container: Container::Mka,
                sample_rate: SampleRate::Hz96000,
                mix: MixPlan {
                    channels: Channels::Stereo,
                    style: Downmix::WithBass,
                },
                bitrate: Bitrate::Kbps(256),
                flac_level: 8,
            },
            volume: Settings::new(-18.5, false, -1.2),
        }
    }

    #[test]
    fn the_radio_tag_text_and_threshold_are_tidied_and_kept_in_range() {
        assert_eq!(RadioSettings::clean_tag("  Top,  40;hit "), "Top 40 hit");
        assert_eq!(RadioSettings::clean_tag(" , ; "), "Radio");
        assert_eq!(RadioSettings::clean_tag(&"x".repeat(80)).len(), 30);
        assert_eq!(RadioSettings::clamp_threshold(0), 1);
        assert_eq!(RadioSettings::clamp_threshold(-5), 1);
        assert_eq!(RadioSettings::clamp_threshold(101), 100);
        assert_eq!(RadioSettings::clamp_threshold(40), 40);
        let defaults = RadioSettings::default();
        assert!(defaults.enabled);
        assert_eq!((defaults.tag.as_str(), defaults.threshold), ("Radio", 100));
    }

    #[test]
    fn an_older_file_with_a_dsd_codec_and_container_still_loads_and_they_are_ignored() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"version": 1, "album_name": "{title}", "single_name": "{name}",
                "dsd_format": {"codec": "flac", "container": "mka", "sample_rate": "176400", "bit_depth": "16"}}"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.settings.dsd.sample_rate, SampleRate::Hz176400);
        assert_eq!(loaded.settings.dsd.bit_depth, BitDepth::Sixteen);
        assert!(loaded.notes.is_empty(), "{:?}", loaded.notes);
    }

    #[test]
    fn the_bitrate_and_the_flac_level_are_kept_and_old_files_get_the_usual_ones() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut saved = SavedSettings::default();
        saved.output.codec = Codec::Mp3;
        saved.output.bitrate = Bitrate::Vbr(2);
        saved.output.flac_level = 8;
        save(&path, &saved).unwrap();
        let loaded = load(&path).settings;
        assert_eq!(loaded.output.bitrate, Bitrate::Vbr(2));
        assert_eq!(loaded.output.flac_level, 8);

        let file = |output: &str| {
            format!(
                r#"{{"version": 1, "album_name": "{{title}}", "single_name": "{{name}}", "output_format": {output}}}"#
            )
        };
        // A file without them, and a bitrate the codec does not offer.
        fs::write(&path, file(r#"{"codec": "opus", "container": "native", "sample_rate": "original", "bit_depth": "source"}"#)).unwrap();
        let old = load(&path);
        assert_eq!(old.settings.output.bitrate, Bitrate::Kbps(192));
        assert_eq!(old.settings.output.flac_level, DEFAULT_FLAC_LEVEL);
        fs::write(&path, file(r#"{"codec": "opus", "container": "native", "sample_rate": "original", "bit_depth": "source", "bitrate": "v0", "flac_level": 99}"#)).unwrap();
        let odd = load(&path);
        assert_eq!(odd.settings.output.bitrate, Bitrate::Kbps(192));
        assert_eq!(odd.settings.output.flac_level, 8, "kept inside 0 to 8");
        assert!(
            odd.notes.iter().any(|n| n.contains("bitrate")),
            "{:?}",
            odd.notes
        );
    }

    #[test]
    fn the_output_format_and_the_volume_are_kept() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        save(&path, &custom()).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.settings.output, custom().output);
        assert_eq!(loaded.settings.volume, custom().volume);
        assert_eq!(loaded.notes, Vec::<String>::new());
    }

    #[test]
    fn a_file_without_them_opens_on_the_defaults_for_both() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            br#"{"version": 1, "album_name": "{title}", "single_name": "{name}"}"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.settings.output, OutputSettings::default());
        assert_eq!(loaded.settings.volume, Settings::default());
        assert_eq!(loaded.notes, Vec::<String>::new());
    }

    #[test]
    fn output_values_not_understood_or_not_possible_fall_back_one_by_one() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // An unknown codec; a container and bit depth that are fine; AAC cannot write 192 kHz.
        fs::write(
            &path,
            br#"{"version": 1, "album_name": "{title}", "single_name": "{name}",
                "output_format": {"codec": "banana", "container": "mka", "sample_rate": "192000", "bit_depth": "24"},
                "volume": {"target_lufs": -300.0, "peak_protection": true, "peak_ceiling_dbtp": 50.0}}"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert_eq!(
            loaded.settings.output.codec,
            OutputSettings::default().codec
        );
        assert_eq!(loaded.settings.output.container, Container::Mka);
        assert_eq!(loaded.settings.output.bit_depth, BitDepth::TwentyFour);
        assert_eq!(loaded.settings.output.sample_rate, SampleRate::Hz192000);
        assert_eq!(loaded.notes.len(), 1, "{:?}", loaded.notes);
        // Numbers out of range are brought inside it, not refused.
        assert_eq!(loaded.settings.volume, Settings::new(-300.0, true, 50.0));
        assert!(loaded.settings.volume.target_lufs > -300.0);
        assert!(loaded.settings.volume.peak_ceiling_dbtp < 50.0);

        fs::write(
            &path,
            br#"{"version": 1, "album_name": "{title}", "single_name": "{name}",
                "output_format": {"codec": "aac", "container": "native", "sample_rate": "192000", "bit_depth": "16"}}"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.settings.output.codec, Codec::Aac);
        assert_eq!(loaded.settings.output.sample_rate, SampleRate::Original);
        assert!(
            loaded.notes[0].contains("back to original"),
            "{:?}",
            loaded.notes
        );
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
    fn an_older_output_block_has_original_channels_and_a_bad_mix_falls_back_with_a_note() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let file = |output: &str| {
            format!(
                r#"{{"version": 1, "album_name": "{{title}}", "single_name": "{{name}}", "output_format": {output}}}"#
            )
        };
        // Saved before channels could be chosen: the whole file still loads.
        fs::write(&path, file(r#"{"codec": "flac", "container": "native", "sample_rate": "original", "bit_depth": "16"}"#)).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.settings.output.mix, MixPlan::default());
        assert_eq!(loaded.notes, Vec::<String>::new());

        // Quad cannot be written as ALAC: it goes back to original, with a note, and the style stays.
        fs::write(&path, file(r#"{"codec": "alac", "container": "native", "sample_rate": "original", "bit_depth": "source", "channels": "quad", "downmix": "front"}"#)).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.settings.output.mix.channels, Channels::Original);
        assert_eq!(loaded.settings.output.mix.style, Downmix::FrontOnly);
        assert!(
            loaded.notes.iter().any(|n| n.contains("quad")),
            "{:?}",
            loaded.notes
        );

        // Names that are not understood fall back one by one.
        fs::write(&path, file(r#"{"codec": "flac", "container": "native", "sample_rate": "original", "bit_depth": "16", "channels": "surround", "downmix": "boost"}"#)).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.settings.output.mix, MixPlan::default());
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
