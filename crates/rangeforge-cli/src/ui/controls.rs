//! The settings panels at the bottom of the screen, apart from volume: Output Format, File
//! Strategy and Presets, plus which control has the keyboard and what mode the keys are in.
//!
//! Like the rest of the screen's state, none of this draws anything, so it is all testable.

use std::path::PathBuf;

use rangeforge_export::format::{BitDepth, Codec, OutputFormat};
use rangeforge_export::strategy::Strategy;

use super::model::Settings;
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

/// The Output Format panel. Sample rate and channels are shown as "original" for now and are
/// not settable yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSettings {
    pub codec: Codec,
    pub bit_depth: BitDepth,
}

impl Default for OutputSettings {
    /// ALAC at the source's bit depth, as in the mockup.
    fn default() -> Self {
        Self {
            codec: Codec::Alac,
            bit_depth: BitDepth::Source,
        }
    }
}

impl OutputSettings {
    pub fn format(self) -> OutputFormat {
        OutputFormat::new(self.codec, self.bit_depth)
    }

    pub fn cycle_codec(&mut self, steps: i32) {
        self.codec = cycle(&Codec::ALL, self.codec, steps);
    }

    pub fn cycle_bit_depth(&mut self, steps: i32) {
        self.bit_depth = cycle(&BitDepth::ALL, self.bit_depth, steps);
    }
}

/// The four File Strategy choices from the mockup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyKind {
    Serialize,
    RenameOld,
    Replace,
    SaveTo,
}

impl StrategyKind {
    pub const ALL: [Self; 4] = [
        Self::Serialize,
        Self::RenameOld,
        Self::Replace,
        Self::SaveTo,
    ];

    /// The wording used in the mockup.
    pub fn label(self) -> &'static str {
        match self {
            Self::Serialize => "Serialize new filename",
            Self::RenameOld => "Rename old file",
            Self::Replace => "Replace original",
            Self::SaveTo => "Save to location",
        }
    }

    /// Strategies that overwrite or move existing files need a confirmation before running.
    pub fn needs_confirmation(self) -> bool {
        matches!(self, Self::Replace)
    }
}

/// The File Strategy panel: which choice is selected, and the text next to each choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategySettings {
    pub kind: StrategyKind,
    pub serial_suffix: String,
    pub old_suffix: String,
    pub folder: String,
}

impl Default for StrategySettings {
    /// Serialize is the safe starting choice. The mockup's default, Replace, is expected to
    /// become the default once it has proven itself (decision D-14).
    fn default() -> Self {
        Self {
            kind: StrategyKind::Serialize,
            serial_suffix: Strategy::DEFAULT_SERIAL.to_string(),
            old_suffix: Strategy::DEFAULT_OLD.to_string(),
            folder: String::new(),
        }
    }
}

impl StrategySettings {
    pub fn cycle_kind(&mut self, steps: i32) {
        self.kind = cycle(&StrategyKind::ALL, self.kind, steps);
    }

    /// The text field belonging to the selected choice, if it has one.
    pub fn text(&self) -> Option<&str> {
        match self.kind {
            StrategyKind::Serialize => Some(&self.serial_suffix),
            StrategyKind::RenameOld => Some(&self.old_suffix),
            StrategyKind::SaveTo => Some(&self.folder),
            StrategyKind::Replace => None,
        }
    }

    /// Replaces the text of the selected choice's field.
    pub fn set_text(&mut self, text: String) {
        match self.kind {
            StrategyKind::Serialize => self.serial_suffix = text,
            StrategyKind::RenameOld => self.old_suffix = text,
            StrategyKind::SaveTo => self.folder = text,
            StrategyKind::Replace => {}
        }
    }

    /// The strategy to run with, or a plain-words reason it cannot be used yet.
    pub fn to_strategy(&self) -> Result<Strategy, String> {
        match self.kind {
            StrategyKind::Serialize => {
                if self
                    .serial_suffix
                    .chars()
                    .last()
                    .is_some_and(|c| c.is_ascii_digit())
                {
                    Ok(Strategy::Serialize {
                        suffix: self.serial_suffix.clone(),
                    })
                } else {
                    Err("the serial suffix must end in digits, like _00".to_string())
                }
            }
            StrategyKind::RenameOld => {
                if self.old_suffix.trim().is_empty() {
                    Err("the suffix for renamed files cannot be empty".to_string())
                } else {
                    Ok(Strategy::RenameOld {
                        suffix: self.old_suffix.clone(),
                    })
                }
            }
            StrategyKind::Replace => Ok(Strategy::Replace),
            StrategyKind::SaveTo => {
                let folder = self.folder.trim();
                if folder.is_empty() {
                    Err("choose a folder to save to".to_string())
                } else {
                    Ok(Strategy::SaveTo {
                        directory: PathBuf::from(folder),
                    })
                }
            }
        }
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
    Strategy,
    /// The suffix of Serialize or Rename old, or nothing for the other choices.
    Suffix,
    Folder,
}

impl Focus {
    const ALL: [Self; 9] = [
        Self::Target,
        Self::PeakProtection,
        Self::Ceiling,
        Self::Codec,
        Self::BitDepth,
        Self::Preset,
        Self::Strategy,
        Self::Suffix,
        Self::Folder,
    ];

    /// A control is only visited when it is showing: the suffix for the two choices that
    /// have one, and the folder for Save to location.
    fn is_active(self, strategy: &StrategySettings) -> bool {
        match self {
            Self::Suffix => matches!(
                strategy.kind,
                StrategyKind::Serialize | StrategyKind::RenameOld
            ),
            Self::Folder => strategy.kind == StrategyKind::SaveTo,
            _ => true,
        }
    }

    /// The next (or, with a negative step, previous) active control, wrapping around.
    pub fn step(self, steps: i32, strategy: &StrategySettings) -> Self {
        let active: Vec<Self> = Self::ALL
            .into_iter()
            .filter(|focus| focus.is_active(strategy))
            .collect();
        cycle(&active, self, steps)
    }

    /// True for controls whose value is typed text.
    pub fn is_text(self) -> bool {
        matches!(self, Self::Suffix | Self::Folder)
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
}

fn matches(preset: &Preset, volume: &Settings, output: OutputSettings) -> bool {
    preset.codec == output.codec
        && preset.bit_depth == output.bit_depth
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
    fn the_container_follows_the_codec() {
        let mut output = OutputSettings::default();
        assert_eq!(output.codec.extension(), "m4a");
        output.codec = Codec::Flac;
        assert_eq!(output.codec.extension(), "flac");
    }

    #[test]
    fn strategy_defaults_are_the_safe_choice_with_the_mockups_suffixes() {
        let strategy = StrategySettings::default();
        assert_eq!(strategy.kind, StrategyKind::Serialize);
        assert_eq!(strategy.serial_suffix, "_00");
        assert_eq!(strategy.old_suffix, "_old");
        assert_eq!(
            strategy.to_strategy(),
            Ok(Strategy::Serialize {
                suffix: "_00".to_string()
            })
        );
    }

    #[test]
    fn each_choice_converts_or_explains_what_is_missing() {
        let mut strategy = StrategySettings {
            serial_suffix: "_new".to_string(),
            ..StrategySettings::default()
        };
        assert!(strategy.to_strategy().unwrap_err().contains("digits"));

        strategy.kind = StrategyKind::RenameOld;
        assert_eq!(
            strategy.to_strategy(),
            Ok(Strategy::RenameOld {
                suffix: "_old".to_string()
            })
        );
        strategy.old_suffix = "  ".to_string();
        assert!(strategy.to_strategy().is_err());

        strategy.kind = StrategyKind::Replace;
        assert_eq!(strategy.to_strategy(), Ok(Strategy::Replace));

        strategy.kind = StrategyKind::SaveTo;
        assert!(strategy.to_strategy().unwrap_err().contains("folder"));
        strategy.folder = " C:\\music\\out ".to_string();
        assert_eq!(
            strategy.to_strategy(),
            Ok(Strategy::SaveTo {
                directory: PathBuf::from("C:\\music\\out")
            })
        );
    }

    #[test]
    fn text_belongs_to_the_selected_choice() {
        let mut strategy = StrategySettings::default();
        assert_eq!(strategy.text(), Some("_00"));
        strategy.set_text("_07".to_string());
        assert_eq!(strategy.serial_suffix, "_07");

        strategy.kind = StrategyKind::RenameOld;
        assert_eq!(strategy.text(), Some("_old"));
        strategy.kind = StrategyKind::Replace;
        assert_eq!(strategy.text(), None);
        strategy.set_text("ignored".to_string());
        assert_eq!(strategy.serial_suffix, "_07");
    }

    #[test]
    fn only_replace_asks_for_confirmation() {
        for kind in StrategyKind::ALL {
            assert_eq!(
                kind.needs_confirmation(),
                kind == StrategyKind::Replace,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn focus_skips_text_fields_that_are_not_showing() {
        let mut strategy = StrategySettings::default();
        // Serialize shows its suffix but not the folder.
        let order: Vec<Focus> = (0..9)
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
                Focus::Strategy,
                Focus::Suffix,
                Focus::Target
            ]
        );

        strategy.kind = StrategyKind::Replace;
        assert_eq!(
            Focus::Strategy.step(1, &strategy),
            Focus::Target,
            "Replace has no field"
        );
        strategy.kind = StrategyKind::SaveTo;
        assert_eq!(Focus::Strategy.step(1, &strategy), Focus::Folder);
        assert_eq!(Focus::Folder.step(1, &strategy), Focus::Target);
        assert_eq!(
            Focus::Target.step(-1, &strategy),
            Focus::Folder,
            "backward wraps"
        );
    }

    #[test]
    fn only_the_two_text_controls_are_text() {
        assert!(Focus::Suffix.is_text());
        assert!(Focus::Folder.is_text());
        assert!(!Focus::Codec.is_text());
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
