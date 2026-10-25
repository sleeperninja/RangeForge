//! The settings panels at the bottom of the screen, apart from volume: Output Format, File
//! Strategy and Presets, plus which control has the keyboard and what mode the keys are in.
//!
//! Like the rest of the screen's state, none of this draws anything, so it is all testable.

use std::path::PathBuf;

use rangeforge_export::format::{BitDepth, Codec, Container, OutputFormat, SampleRate};
use rangeforge_export::strategy::{Conflict, Destination, Strategy};

use crate::model::Settings;
use crate::presets::Preset;

/// Steps to the next or previous item in a list, wrapping around at either end.
fn cycle<T: Copy + PartialEq>(items: &[T], current: T, steps: i32) -> T {
    let length = i32::try_from(items.len()).unwrap_or(1).max(1);
    let position = items
        .iter()
        .position(|item| *item == current)
        .map_or(0, |index| i32::try_from(index).unwrap_or(0));
    let next = (position + steps).rem_euclid(length);
    items
        .get(usize::try_from(next).unwrap_or(0))
        .copied()
        .unwrap_or(current)
}

/// The Output Format panel. Channels are not settable: they are always the source's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSettings {
    pub codec: Codec,
    pub bit_depth: BitDepth,
    pub container: Container,
    pub sample_rate: SampleRate,
}

impl Default for OutputSettings {
    /// ALAC in its usual file, at the source's bit depth and sample rate, as in the mockup.
    fn default() -> Self {
        Self {
            codec: Codec::Alac,
            bit_depth: BitDepth::Source,
            container: Container::Native,
            sample_rate: SampleRate::Original,
        }
    }
}

/// The sample rates offered for `codec`: the source's own, and every rate it can be written at.
pub fn sample_rates_for(codec: Codec) -> Vec<SampleRate> {
    SampleRate::ALL
        .into_iter()
        .filter(|rate| rate.hz().is_none_or(|hz| codec.supports_rate(hz)))
        .collect()
}

impl OutputSettings {
    pub fn format(self) -> OutputFormat {
        OutputFormat::new(self.codec, self.bit_depth)
            .with_container(self.container)
            .with_sample_rate(self.sample_rate)
    }

    /// Puts back to "original" any setting the codec cannot do, such as 192 kHz AAC. Returns
    /// true if something had to change.
    pub fn fit_to_codec(&mut self) -> bool {
        let fits = self
            .sample_rate
            .hz()
            .is_none_or(|hz| self.codec.supports_rate(hz));
        if !fits {
            self.sample_rate = SampleRate::Original;
        }
        !fits
    }

    pub fn cycle_codec(&mut self, steps: i32) {
        self.codec = cycle(&Codec::ALL, self.codec, steps);
        self.fit_to_codec();
    }

    pub fn cycle_bit_depth(&mut self, steps: i32) {
        self.bit_depth = cycle(&BitDepth::ALL, self.bit_depth, steps);
    }

    pub fn cycle_container(&mut self, steps: i32) {
        self.container = cycle(&Container::ALL, self.container, steps);
    }

    pub fn cycle_sample_rate(&mut self, steps: i32) {
        self.sample_rate = cycle(&sample_rates_for(self.codec), self.sample_rate, steps);
    }
}

/// Where the finished files go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhereKind {
    NextToOriginal,
    Subfolder,
    Folder,
}

impl WhereKind {
    pub const ALL: [Self; 3] = [Self::NextToOriginal, Self::Subfolder, Self::Folder];

    pub fn label(self) -> &'static str {
        match self {
            Self::NextToOriginal => "Next to the original",
            Self::Subfolder => "In a subfolder of the original's folder",
            Self::Folder => "All in one folder",
        }
    }
}

/// What to do when a file is already at the new file's path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    Number,
    RenameExisting,
    Overwrite,
}

impl ConflictKind {
    pub const ALL: [Self; 3] = [Self::Number, Self::RenameExisting, Self::Overwrite];

    pub fn label(self) -> &'static str {
        match self {
            Self::Number => "Add a number to the new file",
            Self::RenameExisting => "Rename the existing file",
            Self::Overwrite => "Overwrite the existing file",
        }
    }
}

/// The File Strategy panel: where files go, what to do about a name that is taken, and
/// whether to delete the original afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategySettings {
    pub where_kind: WhereKind,
    pub subfolder: String,
    pub folder: String,
    pub conflict_kind: ConflictKind,
    pub number_suffix: String,
    pub old_suffix: String,
    pub delete_original: bool,
}

impl Default for StrategySettings {
    /// Next to the original, numbering a clash, deleting nothing: the safe starting choice.
    /// Overwriting and deleting are opt-in until they have proven themselves (decision D-14).
    fn default() -> Self {
        Self {
            where_kind: WhereKind::NextToOriginal,
            subfolder: Strategy::DEFAULT_SUBFOLDER.to_string(),
            folder: String::new(),
            conflict_kind: ConflictKind::Number,
            number_suffix: Strategy::DEFAULT_NUMBER.to_string(),
            old_suffix: Strategy::DEFAULT_OLD.to_string(),
            delete_original: false,
        }
    }
}

impl StrategySettings {
    pub fn cycle_where(&mut self, steps: i32) {
        self.where_kind = cycle(&WhereKind::ALL, self.where_kind, steps);
    }

    pub fn cycle_conflict(&mut self, steps: i32) {
        self.conflict_kind = cycle(&ConflictKind::ALL, self.conflict_kind, steps);
    }

    /// The text belonging to the selected place, if it has one.
    pub fn where_text(&self) -> Option<&str> {
        match self.where_kind {
            WhereKind::NextToOriginal => None,
            WhereKind::Subfolder => Some(&self.subfolder),
            WhereKind::Folder => Some(&self.folder),
        }
    }

    pub fn set_where_text(&mut self, text: String) {
        match self.where_kind {
            WhereKind::NextToOriginal => {}
            WhereKind::Subfolder => self.subfolder = text,
            WhereKind::Folder => self.folder = text,
        }
    }

    /// The suffix belonging to the selected way of handling a clash, if it has one.
    pub fn conflict_text(&self) -> Option<&str> {
        match self.conflict_kind {
            ConflictKind::Number => Some(&self.number_suffix),
            ConflictKind::RenameExisting => Some(&self.old_suffix),
            ConflictKind::Overwrite => None,
        }
    }

    pub fn set_conflict_text(&mut self, text: String) {
        match self.conflict_kind {
            ConflictKind::Number => self.number_suffix = text,
            ConflictKind::RenameExisting => self.old_suffix = text,
            ConflictKind::Overwrite => {}
        }
    }

    /// Overwriting files or deleting originals needs a confirmation before running.
    pub fn needs_confirmation(&self) -> bool {
        self.delete_original || self.conflict_kind == ConflictKind::Overwrite
    }

    /// The strategy to run with, or a plain-words reason it cannot be used yet.
    pub fn to_strategy(&self) -> Result<Strategy, String> {
        let destination = match self.where_kind {
            WhereKind::NextToOriginal => Destination::NextToOriginal,
            WhereKind::Subfolder => {
                if self.subfolder.trim().is_empty() {
                    return Err("name the subfolder to save into".to_string());
                }
                Destination::Subfolder(self.subfolder.trim().to_string())
            }
            WhereKind::Folder => {
                let folder = self.folder.trim();
                if folder.is_empty() {
                    return Err("choose a folder to save to".to_string());
                }
                Destination::Folder(PathBuf::from(folder))
            }
        };
        let conflict = match self.conflict_kind {
            ConflictKind::Number => {
                if !self
                    .number_suffix
                    .chars()
                    .last()
                    .is_some_and(|c| c.is_ascii_digit())
                {
                    return Err("the serial suffix must end in digits, like _00".to_string());
                }
                Conflict::Number {
                    suffix: self.number_suffix.clone(),
                }
            }
            ConflictKind::RenameExisting => {
                if self.old_suffix.trim().is_empty() {
                    return Err("the suffix for renamed files cannot be empty".to_string());
                }
                Conflict::RenameExisting {
                    suffix: self.old_suffix.clone(),
                }
            }
            ConflictKind::Overwrite => Conflict::Overwrite,
        };
        let strategy = Strategy {
            destination,
            conflict,
            delete_original: self.delete_original,
        };
        strategy.validate().map_err(|error| error.to_string())?;
        Ok(strategy)
    }
}

/// The controls the keyboard can be on, in the order Tab visits them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Target,
    PeakProtection,
    Ceiling,
    Codec,
    BitDepth,
    Preset,
    Where,
    /// The subfolder name or folder, when the place has one.
    WhereText,
    Conflict,
    /// The suffix, when the way of handling a clash has one.
    ConflictText,
    DeleteOriginal,
}

impl Focus {
    const ALL: [Self; 11] = [
        Self::Target,
        Self::PeakProtection,
        Self::Ceiling,
        Self::Codec,
        Self::BitDepth,
        Self::Preset,
        Self::Where,
        Self::WhereText,
        Self::Conflict,
        Self::ConflictText,
        Self::DeleteOriginal,
    ];

    /// A text control is only visited when it is showing.
    fn is_active(self, strategy: &StrategySettings) -> bool {
        match self {
            Self::WhereText => strategy.where_text().is_some(),
            Self::ConflictText => strategy.conflict_text().is_some(),
            _ => true,
        }
    }

    /// The next (or, with a negative step, previous) active control, wrapping around.
    #[must_use]
    pub fn step(self, steps: i32, strategy: &StrategySettings) -> Self {
        let active: Vec<Self> = Self::ALL
            .into_iter()
            .filter(|focus| focus.is_active(strategy))
            .collect();
        cycle(&active, self, steps)
    }

    /// True for controls whose value is typed text.
    pub fn is_text(self) -> bool {
        matches!(self, Self::WhereText | Self::ConflictText)
    }
}

/// What the keys are doing right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    /// Typing into a text control.
    Editing(String),
    /// Typing a name for a preset about to be saved.
    NamingPreset(String),
    /// Waiting for a yes before running a strategy that overwrites files.
    ConfirmRun,
}

/// The saved presets and which one is selected.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetList {
    pub items: Vec<Preset>,
    pub selected: Option<usize>,
}

/// What saving a preset did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saved {
    Added,
    Updated,
}

impl PresetList {
    pub fn new(items: Vec<Preset>) -> Self {
        Self {
            items,
            selected: None,
        }
    }

    pub fn selected_preset(&self) -> Option<&Preset> {
        self.selected.and_then(|index| self.items.get(index))
    }

    /// Picks the preset that matches the current settings exactly, if any.
    pub fn select_matching(&mut self, volume: &Settings, output: OutputSettings) {
        self.selected = self
            .items
            .iter()
            .position(|preset| matches(preset, volume, output));
    }

    /// True if a preset is selected and the settings have been changed from it.
    pub fn is_modified(&self, volume: &Settings, output: OutputSettings) -> bool {
        self.selected_preset()
            .is_some_and(|preset| !matches(preset, volume, output))
    }

    /// Moves the selection and returns the preset now selected. From "no preset", forward
    /// goes to the first and backward to the last.
    pub fn cycle(&mut self, steps: i32) -> Option<Preset> {
        let length = i32::try_from(self.items.len()).ok().filter(|n| *n > 0)?;
        let next = match self.selected {
            Some(index) => (i32::try_from(index).unwrap_or(0) + steps).rem_euclid(length),
            None if steps >= 0 => 0,
            None => length - 1,
        };
        let index = usize::try_from(next).ok()?;
        self.selected = Some(index);
        self.items.get(index).cloned()
    }

    /// Saves the current settings under `name`, replacing a preset of the same name.
    pub fn save_as(&mut self, name: &str, volume: &Settings, output: OutputSettings) -> Saved {
        let preset = Preset {
            name: name.trim().to_string(),
            codec: output.codec,
            bit_depth: output.bit_depth,
            container: output.container,
            sample_rate: output.sample_rate,
            target_lufs: volume.target_lufs,
            peak_protection: volume.peak_protection,
            peak_ceiling_dbtp: volume.peak_ceiling_dbtp,
        };
        if let Some(index) = self.items.iter().position(|p| p.name == preset.name) {
            self.items[index] = preset;
            self.selected = Some(index);
            Saved::Updated
        } else {
            self.items.push(preset);
            self.selected = Some(self.items.len() - 1);
            Saved::Added
        }
    }

    /// Removes the selected preset and returns its name.
    pub fn delete_selected(&mut self) -> Option<String> {
        let index = self.selected.filter(|index| *index < self.items.len())?;
        let removed = self.items.remove(index);
        self.selected = None;
        Some(removed.name)
    }
}

/// Copies a preset's values into the live settings.
pub fn apply_preset(preset: &Preset, volume: &mut Settings, output: &mut OutputSettings) {
    *volume = Settings::new(
        preset.target_lufs,
        preset.peak_protection,
        preset.peak_ceiling_dbtp,
    );
    output.codec = preset.codec;
    output.bit_depth = preset.bit_depth;
    output.container = preset.container;
    output.sample_rate = preset.sample_rate;
}

fn matches(preset: &Preset, volume: &Settings, output: OutputSettings) -> bool {
    preset.codec == output.codec
        && preset.bit_depth == output.bit_depth
        && preset.container == output.container
        && preset.sample_rate == output.sample_rate
        && preset.peak_protection == volume.peak_protection
        && (preset.target_lufs - volume.target_lufs).abs() < 1e-9
        && (preset.peak_ceiling_dbtp - volume.peak_ceiling_dbtp).abs() < 1e-9
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::built_in;

    #[test]
    fn cycling_wraps_in_both_directions() {
        let mut output = OutputSettings::default();
        assert_eq!(output.codec, Codec::Alac);
        output.cycle_codec(1);
        assert_eq!(output.codec, Codec::Flac);
        output.cycle_codec(-2);
        assert_eq!(
            output.codec,
            Codec::Aac,
            "backward from the first wraps to the last"
        );
        output.cycle_codec(1);
        assert_eq!(output.codec, Codec::Alac);

        output.cycle_bit_depth(-1);
        assert_eq!(output.bit_depth, BitDepth::TwentyFour);
        output.cycle_bit_depth(1);
        assert_eq!(output.bit_depth, BitDepth::Source);
    }

    #[test]
    fn the_native_container_follows_the_codec_and_mka_is_always_available() {
        let mut output = OutputSettings::default();
        assert_eq!(output.format().extension(), "m4a");
        output.codec = Codec::Flac;
        assert_eq!(output.format().extension(), "flac");
        output.cycle_container(1);
        assert_eq!(output.container, Container::Mka);
        assert_eq!(output.format().extension(), "mka");
        output.cycle_container(1);
        assert_eq!(output.container, Container::Native, "it wraps");
    }

    #[test]
    fn sample_rates_cycle_through_what_the_codec_can_write() {
        let mut output = OutputSettings::default();
        assert_eq!(output.sample_rate, SampleRate::Original);
        output.cycle_sample_rate(1);
        assert_eq!(output.sample_rate, SampleRate::Hz44100);
        output.cycle_sample_rate(-2);
        assert_eq!(
            output.sample_rate,
            SampleRate::Hz192000,
            "backward from the first wraps to the last"
        );

        // AAC stops at 96 kHz, so the list is shorter and 192 kHz cannot stay chosen.
        assert_eq!(sample_rates_for(Codec::Alac).len(), 7);
        assert_eq!(
            sample_rates_for(Codec::Aac),
            [
                SampleRate::Original,
                SampleRate::Hz44100,
                SampleRate::Hz48000,
                SampleRate::Hz88200,
                SampleRate::Hz96000
            ]
        );
        output.codec = Codec::Aac;
        assert!(output.fit_to_codec());
        assert_eq!(output.sample_rate, SampleRate::Original);
        assert!(!output.fit_to_codec(), "nothing more to fix");

        // Cycling the codec fixes it as it goes.
        output.sample_rate = SampleRate::Hz176400;
        output.codec = Codec::Wav;
        output.cycle_codec(1);
        assert_eq!(output.codec, Codec::Aac);
        assert_eq!(output.sample_rate, SampleRate::Original);
    }

    #[test]
    fn the_container_and_rate_travel_with_a_preset() {
        let mut presets = list();
        let volume = Settings::default();
        let mut output = OutputSettings {
            container: Container::Mka,
            sample_rate: SampleRate::Hz88200,
            ..OutputSettings::default()
        };
        presets.save_as("Archive", &volume, output);
        let saved = presets.selected_preset().unwrap().clone();
        assert_eq!(
            (saved.container, saved.sample_rate),
            (Container::Mka, SampleRate::Hz88200)
        );

        // Applying it brings both back, and changing either marks the preset as modified.
        let mut live_volume = Settings::default();
        let mut live = OutputSettings::default();
        apply_preset(&saved, &mut live_volume, &mut live);
        assert_eq!(live, output);
        output.sample_rate = SampleRate::Hz96000;
        assert!(presets.is_modified(&volume, output));
        output.sample_rate = SampleRate::Hz88200;
        output.container = Container::Native;
        assert!(presets.is_modified(&volume, output));
    }

    #[test]
    fn strategy_defaults_are_the_safe_choice_with_the_usual_suffixes() {
        let strategy = StrategySettings::default();
        assert_eq!(strategy.where_kind, WhereKind::NextToOriginal);
        assert_eq!(strategy.conflict_kind, ConflictKind::Number);
        assert_eq!(strategy.number_suffix, "_00");
        assert_eq!(strategy.old_suffix, "_old");
        assert!(!strategy.delete_original);
        assert_eq!(strategy.to_strategy(), Ok(Strategy::safe()));
        assert!(!strategy.needs_confirmation());
    }

    #[test]
    fn each_choice_converts_or_explains_what_is_missing() {
        let mut strategy = StrategySettings {
            number_suffix: "_new".to_string(),
            ..StrategySettings::default()
        };
        assert!(strategy.to_strategy().unwrap_err().contains("digits"));

        strategy.conflict_kind = ConflictKind::RenameExisting;
        assert_eq!(
            strategy.to_strategy().unwrap().conflict,
            Conflict::RenameExisting {
                suffix: "_old".to_string()
            }
        );
        strategy.old_suffix = "  ".to_string();
        assert!(strategy.to_strategy().is_err());

        strategy.conflict_kind = ConflictKind::Overwrite;
        assert_eq!(
            strategy.to_strategy().unwrap().conflict,
            Conflict::Overwrite
        );

        strategy.where_kind = WhereKind::Folder;
        assert!(strategy.to_strategy().unwrap_err().contains("folder"));
        strategy.folder = " C:\\music\\out ".to_string();
        assert_eq!(
            strategy.to_strategy().unwrap().destination,
            Destination::Folder(PathBuf::from("C:\\music\\out"))
        );

        strategy.where_kind = WhereKind::Subfolder;
        strategy.subfolder = "  ".to_string();
        assert!(strategy.to_strategy().unwrap_err().contains("subfolder"));
        strategy.subfolder = "a/b".to_string();
        assert!(strategy.to_strategy().unwrap_err().contains("folder name"));
        strategy.subfolder = " Balanced ".to_string();
        assert_eq!(
            strategy.to_strategy().unwrap().destination,
            Destination::Subfolder("Balanced".to_string())
        );
    }

    #[test]
    fn text_belongs_to_the_selected_choice() {
        let mut strategy = StrategySettings::default();
        assert_eq!(strategy.where_text(), None);
        strategy.set_where_text("ignored".to_string());
        assert_eq!(strategy.subfolder, "RangeForge");

        strategy.where_kind = WhereKind::Subfolder;
        assert_eq!(strategy.where_text(), Some("RangeForge"));
        strategy.set_where_text("Balanced".to_string());
        assert_eq!(strategy.subfolder, "Balanced");

        assert_eq!(strategy.conflict_text(), Some("_00"));
        strategy.set_conflict_text("_07".to_string());
        assert_eq!(strategy.number_suffix, "_07");

        strategy.conflict_kind = ConflictKind::RenameExisting;
        assert_eq!(strategy.conflict_text(), Some("_old"));
        strategy.conflict_kind = ConflictKind::Overwrite;
        assert_eq!(strategy.conflict_text(), None);
        strategy.set_conflict_text("ignored".to_string());
        assert_eq!(strategy.number_suffix, "_07");
    }

    #[test]
    fn only_overwriting_or_deleting_asks_for_confirmation() {
        let mut strategy = StrategySettings::default();
        for kind in ConflictKind::ALL {
            strategy.conflict_kind = kind;
            assert_eq!(
                strategy.needs_confirmation(),
                kind == ConflictKind::Overwrite,
                "{kind:?}"
            );
        }
        strategy.conflict_kind = ConflictKind::Number;
        strategy.delete_original = true;
        assert!(strategy.needs_confirmation());
        assert!(strategy.to_strategy().unwrap().delete_original);
    }

    #[test]
    fn focus_skips_text_fields_that_are_not_showing() {
        let mut strategy = StrategySettings::default();
        // Next to the original has no text, numbering has a suffix.
        let order: Vec<Focus> = (0..10)
            .scan(Focus::Target, |at, _| {
                let current = *at;
                *at = at.step(1, &strategy);
                Some(current)
            })
            .collect();
        assert_eq!(
            order,
            [
                Focus::Target,
                Focus::PeakProtection,
                Focus::Ceiling,
                Focus::Codec,
                Focus::BitDepth,
                Focus::Preset,
                Focus::Where,
                Focus::Conflict,
                Focus::ConflictText,
                Focus::DeleteOriginal
            ]
        );

        strategy.where_kind = WhereKind::Folder;
        assert_eq!(Focus::Where.step(1, &strategy), Focus::WhereText);
        strategy.conflict_kind = ConflictKind::Overwrite;
        assert_eq!(
            Focus::Conflict.step(1, &strategy),
            Focus::DeleteOriginal,
            "overwrite has no suffix"
        );
        assert_eq!(
            Focus::Target.step(-1, &strategy),
            Focus::DeleteOriginal,
            "backward wraps"
        );
    }

    #[test]
    fn only_the_two_text_controls_are_text() {
        assert!(Focus::WhereText.is_text());
        assert!(Focus::ConflictText.is_text());
        assert!(!Focus::Codec.is_text());
        assert!(!Focus::DeleteOriginal.is_text());
    }

    fn list() -> PresetList {
        PresetList::new(built_in())
    }

    #[test]
    fn a_matching_preset_is_found_and_changes_are_noticed() {
        let mut presets = list();
        let mut volume = Settings::default();
        let mut output = OutputSettings::default();

        presets.select_matching(&volume, output);
        assert_eq!(presets.selected_preset().unwrap().name, "iTunes Lossless");
        assert!(!presets.is_modified(&volume, output));

        volume.nudge_target(1);
        assert!(presets.is_modified(&volume, output));
        volume.nudge_target(-1);
        assert!(
            !presets.is_modified(&volume, output),
            "changing it back clears the flag"
        );

        output.cycle_codec(1);
        assert!(presets.is_modified(&volume, output));
    }

    #[test]
    fn custom_settings_match_no_preset_and_are_not_flagged() {
        let mut presets = list();
        let volume = Settings::new(-14.0, true, -0.6);
        let output = OutputSettings::default();
        presets.select_matching(&volume, output);
        assert_eq!(presets.selected, None);
        assert!(!presets.is_modified(&volume, output));
    }

    #[test]
    fn cycling_presets_applies_their_values() {
        let mut presets = list();
        let mut volume = Settings::default();
        let mut output = OutputSettings::default();

        let preset = presets.cycle(1).unwrap();
        assert_eq!(
            preset.name, "iTunes Lossy",
            "forward from none is the first"
        );
        apply_preset(&preset, &mut volume, &mut output);
        assert_eq!(output.codec, Codec::Aac);

        let preset = presets.cycle(2).unwrap();
        assert_eq!(preset.name, "CD Quality");
        apply_preset(&preset, &mut volume, &mut output);
        assert_eq!(
            (output.codec, output.bit_depth),
            (Codec::Flac, BitDepth::Sixteen)
        );

        presets.selected = None;
        assert_eq!(
            presets.cycle(-1).unwrap().name,
            "ALAC Multi",
            "backward from none is the last"
        );
        presets.cycle(1);
        assert_eq!(presets.selected, Some(0), "wraps");
    }

    #[test]
    fn an_empty_list_cycles_to_nothing() {
        let mut presets = PresetList::new(Vec::new());
        assert_eq!(presets.cycle(1), None);
        assert_eq!(presets.selected, None);
    }

    #[test]
    fn saving_adds_then_updates_by_name() {
        let mut presets = list();
        let volume = Settings::new(-14.0, false, -1.0);
        let output = OutputSettings {
            codec: Codec::Wav,
            bit_depth: BitDepth::TwentyFour,
            container: Container::Mka,
            sample_rate: SampleRate::Hz96000,
        };

        assert_eq!(
            presets.save_as("  Car stereo ", &volume, output),
            Saved::Added
        );
        assert_eq!(presets.items.len(), 6);
        assert_eq!(presets.selected_preset().unwrap().name, "Car stereo");
        assert!(!presets.is_modified(&volume, output));

        let louder = Settings::new(-12.0, false, -1.0);
        assert_eq!(
            presets.save_as("Car stereo", &louder, output),
            Saved::Updated
        );
        assert_eq!(presets.items.len(), 6, "same name replaces");
        assert!((presets.selected_preset().unwrap().target_lufs - -12.0).abs() < 1e-9);
    }

    #[test]
    fn deleting_removes_the_selected_preset_only() {
        let mut presets = list();
        presets.selected = Some(2);
        assert_eq!(presets.delete_selected().as_deref(), Some("CD Quality"));
        assert_eq!(presets.items.len(), 4);
        assert_eq!(presets.selected, None);
        assert_eq!(
            presets.delete_selected(),
            None,
            "nothing selected, nothing deleted"
        );
        assert_eq!(presets.items.len(), 4);
    }
}
