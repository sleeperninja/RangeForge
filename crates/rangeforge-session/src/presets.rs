//! Presets: saved sets of output and volume settings, kept in a small JSON file.
//!
//! A preset stores the codec, bit depth and volume settings. It does not store the File
//! Strategy, which is about what is on disk right now and not about the sound.
//!
//! Loading never fails. A missing file means the built-in presets, and a file that cannot be
//! read is set aside as `presets.json.bad` and replaced by the built-ins, with a note saying
//! so, so a damaged file can never stop the app or be quietly overwritten.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rangeforge_export::format::{BitDepth, Codec, Container, SampleRate};
use serde::{Deserialize, Serialize};

/// Bump this if the file's layout changes in a way older versions cannot read. (A new field
/// with a default does not need it.)
const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub name: String,
    pub codec: Codec,
    pub bit_depth: BitDepth,
    pub container: Container,
    pub sample_rate: SampleRate,
    pub target_lufs: f64,
    pub peak_protection: bool,
    pub peak_ceiling_dbtp: f64,
}

impl Preset {
    fn built_in(name: &str, codec: Codec, bit_depth: BitDepth) -> Self {
        Self {
            name: name.to_string(),
            codec,
            bit_depth,
            container: Container::Native,
            sample_rate: SampleRate::Original,
            target_lufs: -22.0,
            peak_protection: true,
            peak_ceiling_dbtp: -0.6,
        }
    }
}

/// The presets listed in the mockup. They keep the source's sample rate, so "CD Quality" means
/// 16-bit FLAC; a preset saved after changing the sample rate remembers it.
pub fn built_in() -> Vec<Preset> {
    vec![
        Preset::built_in("iTunes Lossy", Codec::Aac, BitDepth::Source),
        Preset::built_in("iTunes Lossless", Codec::Alac, BitDepth::Source),
        Preset::built_in("CD Quality", Codec::Flac, BitDepth::Sixteen),
        Preset::built_in("Flac HD Multi", Codec::Flac, BitDepth::TwentyFour),
        Preset::built_in("ALAC Multi", Codec::Alac, BitDepth::Source),
    ]
}

/// What loading found.
#[derive(Debug)]
pub struct Loaded {
    pub presets: Vec<Preset>,
    /// Things worth telling the user, such as a damaged file that was set aside.
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    version: u32,
    presets: Vec<StoredPreset>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredPreset {
    name: String,
    codec: String,
    bit_depth: String,
    /// Added later: a preset saved before then is in the codec's usual file.
    #[serde(default = "default_container")]
    container: String,
    /// Added later: a preset saved before then keeps the source's sample rate.
    #[serde(default = "default_sample_rate")]
    sample_rate: String,
    target_lufs: f64,
    peak_protection: bool,
    peak_ceiling_dbtp: f64,
}

fn default_container() -> String {
    Container::Native.name().to_string()
}

fn default_sample_rate() -> String {
    SampleRate::Original.name().to_string()
}

impl From<&Preset> for StoredPreset {
    fn from(preset: &Preset) -> Self {
        Self {
            name: preset.name.clone(),
            codec: preset.codec.name().to_string(),
            bit_depth: preset.bit_depth.name().to_string(),
            container: preset.container.name().to_string(),
            sample_rate: preset.sample_rate.name().to_string(),
            target_lufs: preset.target_lufs,
            peak_protection: preset.peak_protection,
            peak_ceiling_dbtp: preset.peak_ceiling_dbtp,
        }
    }
}

impl StoredPreset {
    fn into_preset(self) -> Option<Preset> {
        let valid = !self.name.trim().is_empty()
            && self.target_lufs.is_finite()
            && self.peak_ceiling_dbtp.is_finite();
        valid.then_some(())?;
        Some(Preset {
            name: self.name,
            codec: Codec::from_name(&self.codec)?,
            bit_depth: BitDepth::from_name(&self.bit_depth)?,
            container: Container::from_name(&self.container)?,
            sample_rate: SampleRate::from_name(&self.sample_rate)?,
            target_lufs: self.target_lufs,
            peak_protection: self.peak_protection,
            peak_ceiling_dbtp: self.peak_ceiling_dbtp,
        })
    }
}

/// Reads the presets at `path`. See the module notes for what happens when it cannot.
pub fn load(path: &Path) -> Loaded {
    let defaults = |notes: Vec<String>| Loaded {
        presets: built_in(),
        notes,
    };
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return defaults(Vec::new()),
        Err(error) => {
            return defaults(vec![format!(
                "Could not read presets at {}: {error}. Using the built-in presets",
                path.display()
            )]);
        }
    };

    match serde_json::from_slice::<Stored>(&bytes) {
        Ok(stored) if stored.version == VERSION => {
            let total = stored.presets.len();
            let presets: Vec<Preset> = stored
                .presets
                .into_iter()
                .filter_map(StoredPreset::into_preset)
                .collect();
            let mut notes = Vec::new();
            if presets.len() < total {
                notes.push(format!(
                    "Skipped {} preset(s) in {} that could not be understood",
                    total - presets.len(),
                    path.display()
                ));
            }
            Loaded { presets, notes }
        }
        _ => {
            let backup = path.with_extension("json.bad");
            let kept = fs::rename(path, &backup).is_ok();
            let note = if kept {
                format!(
                    "The presets file could not be read. It was kept as {} and the built-in presets are in use",
                    backup.display()
                )
            } else {
                format!(
                    "The presets file {} could not be read, so the built-in presets are in use",
                    path.display()
                )
            };
            defaults(vec![note])
        }
    }
}

/// Writes `presets` to `path`, through a temporary file so a crash cannot leave a half-written
/// file behind.
pub fn save(path: &Path, presets: &[Preset]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let stored = Stored {
        version: VERSION,
        presets: presets.iter().map(StoredPreset::from).collect(),
    };
    let json = serde_json::to_vec_pretty(&stored).context("writing presets")?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, json).with_context(|| format!("writing {}", temporary.display()))?;
    fs::rename(&temporary, path).with_context(|| format!("saving {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn custom() -> Preset {
        Preset {
            name: "Car stereo".to_string(),
            codec: Codec::Aac,
            bit_depth: BitDepth::Sixteen,
            container: Container::Mka,
            sample_rate: SampleRate::Hz48000,
            target_lufs: -14.5,
            peak_protection: false,
            peak_ceiling_dbtp: -1.0,
        }
    }

    #[test]
    fn a_preset_saved_before_these_options_existed_still_loads_with_the_old_meaning() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("presets.json");
        fs::write(
            &path,
            br#"{"version": 1, "presets": [
                {"name": "Old", "codec": "flac", "bit_depth": "24", "target_lufs": -20.0, "peak_protection": true, "peak_ceiling_dbtp": -0.6}
            ]}"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.presets.len(), 1);
        assert_eq!(loaded.presets[0].container, Container::Native);
        assert_eq!(loaded.presets[0].sample_rate, SampleRate::Original);
        assert_eq!(loaded.notes, Vec::<String>::new());
    }

    #[test]
    fn a_preset_with_an_unknown_container_or_rate_is_skipped_with_a_note() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("presets.json");
        fs::write(
            &path,
            br#"{"version": 1, "presets": [
                {"name": "A", "codec": "flac", "bit_depth": "24", "container": "ogg", "target_lufs": -20.0, "peak_protection": true, "peak_ceiling_dbtp": -0.6},
                {"name": "B", "codec": "flac", "bit_depth": "24", "sample_rate": "22050", "target_lufs": -20.0, "peak_protection": true, "peak_ceiling_dbtp": -0.6}
            ]}"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.presets.len(), 0);
        assert!(loaded.notes[0].contains("Skipped 2"), "{:?}", loaded.notes);
    }

    #[test]
    fn the_built_in_presets_match_the_mockup() {
        let names: Vec<String> = built_in().into_iter().map(|p| p.name).collect();
        assert_eq!(
            names,
            [
                "iTunes Lossy",
                "iTunes Lossless",
                "CD Quality",
                "Flac HD Multi",
                "ALAC Multi"
            ]
        );
    }

    #[test]
    fn a_missing_file_means_the_built_ins_and_no_fuss() {
        let dir = tempdir().unwrap();
        let loaded = load(&dir.path().join("presets.json"));
        assert_eq!(loaded.presets, built_in());
        assert_eq!(loaded.notes, Vec::<String>::new());
    }

    #[test]
    fn presets_survive_a_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("deeper").join("presets.json");
        let mut presets = built_in();
        presets.push(custom());

        save(&path, &presets).unwrap();
        let loaded = load(&path);

        assert_eq!(loaded.presets, presets);
        assert_eq!(loaded.notes, Vec::<String>::new());
    }

    #[test]
    fn deleting_every_preset_is_remembered() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("presets.json");
        save(&path, &[]).unwrap();
        assert_eq!(load(&path).presets, Vec::<Preset>::new());
    }

    #[test]
    fn a_damaged_file_is_set_aside_not_overwritten() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("presets.json");
        fs::write(&path, b"{ this is not json").unwrap();

        let loaded = load(&path);

        assert_eq!(loaded.presets, built_in());
        assert_eq!(loaded.notes.len(), 1);
        assert!(
            loaded.notes[0].contains("presets.json.bad"),
            "{:?}",
            loaded.notes
        );
        assert_eq!(
            fs::read(dir.path().join("presets.json.bad")).unwrap(),
            b"{ this is not json"
        );
        assert!(!path.exists());
    }

    #[test]
    fn a_file_from_a_newer_version_is_set_aside_too() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("presets.json");
        fs::write(&path, br#"{"version": 99, "presets": []}"#).unwrap();

        let loaded = load(&path);
        assert_eq!(loaded.presets, built_in());
        assert!(dir.path().join("presets.json.bad").exists());
    }

    #[test]
    fn entries_that_cannot_be_understood_are_skipped_with_a_note() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("presets.json");
        fs::write(
            &path,
            br#"{"version": 1, "presets": [
                {"name": "Good", "codec": "flac", "bit_depth": "24", "target_lufs": -20.0, "peak_protection": true, "peak_ceiling_dbtp": -0.6},
                {"name": "Bad codec", "codec": "mp3", "bit_depth": "16", "target_lufs": -20.0, "peak_protection": true, "peak_ceiling_dbtp": -0.6},
                {"name": "  ", "codec": "alac", "bit_depth": "16", "target_lufs": -20.0, "peak_protection": true, "peak_ceiling_dbtp": -0.6}
            ]}"#,
        )
        .unwrap();

        let loaded = load(&path);

        assert_eq!(loaded.presets.len(), 1);
        assert_eq!(loaded.presets[0].name, "Good");
        assert_eq!(loaded.presets[0].bit_depth, BitDepth::TwentyFour);
        assert!(loaded.notes[0].contains("Skipped 2"), "{:?}", loaded.notes);
        assert!(path.exists(), "a partly readable file is kept in place");
    }

    #[test]
    fn saving_leaves_no_temporary_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("presets.json");
        save(&path, &built_in()).unwrap();
        let names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["presets.json"]);
    }
}
