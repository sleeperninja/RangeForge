//! The command-line options that describe the output, shared by `run` and `ui`.

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use rangeforge_export::format::{BitDepth, Codec};
use rangeforge_export::strategy::Strategy;

use rangeforge_session::controls::{ConflictKind, OutputSettings, StrategySettings, WhereKind};
use rangeforge_session::naming::{self, NameSettings};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CodecArg {
    Alac,
    Flac,
    Wav,
    Aac,
}

impl From<CodecArg> for Codec {
    fn from(arg: CodecArg) -> Self {
        match arg {
            CodecArg::Alac => Self::Alac,
            CodecArg::Flac => Self::Flac,
            CodecArg::Wav => Self::Wav,
            CodecArg::Aac => Self::Aac,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BitDepthArg {
    /// Match the source file
    Source,
    #[value(name = "16")]
    Sixteen,
    #[value(name = "24")]
    TwentyFour,
}

impl From<BitDepthArg> for BitDepth {
    fn from(arg: BitDepthArg) -> Self {
        match arg {
            BitDepthArg::Source => Self::Source,
            BitDepthArg::Sixteen => Self::Sixteen,
            BitDepthArg::TwentyFour => Self::TwentyFour,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ConflictArg {
    /// The new file gets a serial suffix such as _00; the file already there is untouched
    Number,
    /// The file already there is renamed with a suffix (default _old)
    RenameExisting,
    /// The file already there is overwritten (needs --yes)
    Overwrite,
}

/// What to write and where. These are the same choices as the Output Format and File
/// Strategy panels on screen.
#[derive(Debug, Args)]
pub struct OutputArgs {
    /// The format to write
    #[arg(long, value_enum, default_value = "alac")]
    codec: CodecArg,
    /// How precisely to store each sample
    #[arg(long, value_enum, default_value = "source")]
    bit_depth: BitDepthArg,
    /// Write all files into this one folder instead of next to the originals
    #[arg(long, conflicts_with = "subfolder")]
    out: Option<PathBuf>,
    /// Write into a folder of this name inside each original's folder
    #[arg(long)]
    subfolder: Option<String>,
    /// What to do when a file is already at the new file's path
    #[arg(long, value_enum, default_value = "number")]
    on_conflict: ConflictArg,
    /// Suffix for --on-conflict number (default _00) or rename-existing (default _old)
    #[arg(long)]
    suffix: Option<String>,
    /// Delete each original once every song from it has been written (needs --yes)
    #[arg(long)]
    delete_original: bool,
    /// File name pattern for songs cut from a CUE album, such as "{track:02} - {title}"
    #[arg(long, default_value = naming::DEFAULT_ALBUM)]
    name: String,
    /// File name pattern for single files; only {name} is available
    #[arg(long, default_value = naming::DEFAULT_SINGLE)]
    single_name: String,
}

impl OutputArgs {
    pub fn output_settings(&self) -> OutputSettings {
        OutputSettings {
            codec: self.codec.into(),
            bit_depth: self.bit_depth.into(),
        }
    }

    pub fn strategy_settings(&self) -> StrategySettings {
        let mut settings = StrategySettings::default();
        if let Some(folder) = &self.out {
            settings.where_kind = WhereKind::Folder;
            settings.folder = folder.display().to_string();
        } else if let Some(name) = &self.subfolder {
            settings.where_kind = WhereKind::Subfolder;
            settings.subfolder.clone_from(name);
        }
        match self.on_conflict {
            ConflictArg::Number => {
                settings.conflict_kind = ConflictKind::Number;
                if let Some(suffix) = &self.suffix {
                    settings.number_suffix.clone_from(suffix);
                }
            }
            ConflictArg::RenameExisting => {
                settings.conflict_kind = ConflictKind::RenameExisting;
                if let Some(suffix) = &self.suffix {
                    settings.old_suffix.clone_from(suffix);
                }
            }
            ConflictArg::Overwrite => settings.conflict_kind = ConflictKind::Overwrite,
        }
        settings.delete_original = self.delete_original;
        settings
    }

    pub fn naming_settings(&self) -> NameSettings {
        NameSettings {
            album: self.name.clone(),
            single: self.single_name.clone(),
        }
    }

    /// The strategy to run with, or a plain-words reason it cannot be used.
    pub fn strategy(&self) -> Result<Strategy, String> {
        self.strategy_settings().to_strategy()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use rangeforge_export::strategy::{Conflict, Destination};

    #[derive(Debug, Parser)]
    struct Wrapper {
        #[command(flatten)]
        output: OutputArgs,
    }

    fn parse(args: &[&str]) -> OutputArgs {
        Wrapper::try_parse_from(std::iter::once("test").chain(args.iter().copied()))
            .unwrap()
            .output
    }

    #[test]
    fn defaults_are_alac_at_source_depth_next_to_the_original() {
        let args = parse(&[]);
        assert_eq!(args.output_settings(), OutputSettings::default());
        assert_eq!(args.strategy(), Ok(Strategy::safe()));
    }

    #[test]
    fn codec_and_bit_depth_are_chosen_by_name() {
        let args = parse(&["--codec", "flac", "--bit-depth", "24"]);
        let settings = args.output_settings();
        assert_eq!(settings.codec, Codec::Flac);
        assert_eq!(settings.bit_depth, BitDepth::TwentyFour);
    }

    #[test]
    fn suffixes_apply_to_the_conflict_choice_that_uses_them() {
        let number = parse(&["--suffix", "_v01"]).strategy().unwrap();
        assert_eq!(
            number.conflict,
            Conflict::Number {
                suffix: "_v01".to_string()
            }
        );
        let rename = parse(&["--on-conflict", "rename-existing", "--suffix", "_backup"])
            .strategy()
            .unwrap();
        assert_eq!(
            rename.conflict,
            Conflict::RenameExisting {
                suffix: "_backup".to_string()
            }
        );
        let default_rename = parse(&["--on-conflict", "rename-existing"])
            .strategy()
            .unwrap();
        assert_eq!(
            default_rename.conflict,
            Conflict::RenameExisting {
                suffix: "_old".to_string()
            }
        );
    }

    #[test]
    fn overwrite_has_no_suffix() {
        let strategy = parse(&["--on-conflict", "overwrite"]).strategy().unwrap();
        assert_eq!(strategy.conflict, Conflict::Overwrite);
    }

    #[test]
    fn out_and_subfolder_choose_where_files_go() {
        let out = parse(&["--out", "C:\\music\\out"]);
        assert_eq!(out.strategy_settings().where_kind, WhereKind::Folder);
        assert_eq!(
            out.strategy().unwrap().destination,
            Destination::Folder(PathBuf::from("C:\\music\\out"))
        );

        let sub = parse(&["--subfolder", "Balanced"]).strategy().unwrap();
        assert_eq!(
            sub.destination,
            Destination::Subfolder("Balanced".to_string())
        );
    }

    #[test]
    fn out_and_subfolder_cannot_be_combined() {
        let result = Wrapper::try_parse_from(["test", "--out", "x", "--subfolder", "y"]);
        assert!(result.is_err());
    }

    #[test]
    fn deleting_originals_is_opt_in() {
        assert!(!parse(&[]).strategy().unwrap().delete_original);
        let strategy = parse(&["--delete-original"]).strategy().unwrap();
        assert!(strategy.delete_original);
        assert!(strategy.needs_confirmation());
    }

    #[test]
    fn a_serial_suffix_without_digits_is_reported() {
        let error = parse(&["--suffix", "_new"]).strategy().unwrap_err();
        assert!(error.contains("digits"), "{error}");
    }

    #[test]
    fn name_patterns_default_to_the_usual_and_can_be_changed() {
        assert_eq!(parse(&[]).naming_settings(), NameSettings::default());
        let args = parse(&[
            "--name",
            "{artist} - {title}",
            "--single-name",
            "{name} (b)",
        ]);
        let names = args.naming_settings();
        assert_eq!(names.album, "{artist} - {title}");
        assert_eq!(names.single, "{name} (b)");
        assert!(names.templates().is_ok());
        assert!(
            parse(&["--name", "{bogus}"])
                .naming_settings()
                .templates()
                .is_err()
        );
    }

    #[test]
    fn a_bad_subfolder_name_is_reported() {
        let error = parse(&["--subfolder", "a/b"]).strategy().unwrap_err();
        assert!(error.contains("folder name"), "{error}");
    }
}
