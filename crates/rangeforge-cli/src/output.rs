//! The command-line options that describe the output, shared by `run` and `ui`.

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use rangeforge_export::format::{BitDepth, Codec};
use rangeforge_export::strategy::Strategy;

use crate::ui::controls::{OutputSettings, StrategyKind, StrategySettings};

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
enum StrategyArg {
    /// New files get a serial suffix such as _00; nothing existing is touched
    Serialize,
    /// An existing file at the same name is renamed with the suffix _old
    RenameOld,
    /// A file at exactly the same path is overwritten (needs --yes)
    Replace,
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
    /// What to do about files that already exist
    #[arg(long, value_enum, default_value = "serialize", conflicts_with = "out")]
    strategy: StrategyArg,
    /// Suffix for --strategy serialize (default _00) or rename-old (default _old)
    #[arg(long, conflicts_with = "out")]
    suffix: Option<String>,
    /// Write into this folder instead of next to the sources
    #[arg(long)]
    out: Option<PathBuf>,
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
            settings.kind = StrategyKind::SaveTo;
            settings.folder = folder.display().to_string();
            return settings;
        }
        match self.strategy {
            StrategyArg::Serialize => {
                settings.kind = StrategyKind::Serialize;
                if let Some(suffix) = &self.suffix {
                    settings.serial_suffix.clone_from(suffix);
                }
            }
            StrategyArg::RenameOld => {
                settings.kind = StrategyKind::RenameOld;
                if let Some(suffix) = &self.suffix {
                    settings.old_suffix.clone_from(suffix);
                }
            }
            StrategyArg::Replace => settings.kind = StrategyKind::Replace,
        }
        settings
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
    fn defaults_are_alac_at_source_depth_with_a_serial_suffix() {
        let args = parse(&[]);
        assert_eq!(args.output_settings(), OutputSettings::default());
        assert_eq!(
            args.strategy(),
            Ok(Strategy::Serialize {
                suffix: "_00".to_string()
            })
        );
    }

    #[test]
    fn codec_and_bit_depth_are_chosen_by_name() {
        let args = parse(&["--codec", "flac", "--bit-depth", "24"]);
        let settings = args.output_settings();
        assert_eq!(settings.codec, Codec::Flac);
        assert_eq!(settings.bit_depth, BitDepth::TwentyFour);
    }

    #[test]
    fn suffixes_apply_to_the_strategy_that_uses_them() {
        let serialize = parse(&["--suffix", "_v01"]);
        assert_eq!(
            serialize.strategy(),
            Ok(Strategy::Serialize {
                suffix: "_v01".to_string()
            })
        );
        let rename = parse(&["--strategy", "rename-old", "--suffix", "_backup"]);
        assert_eq!(
            rename.strategy(),
            Ok(Strategy::RenameOld {
                suffix: "_backup".to_string()
            })
        );
        assert_eq!(
            parse(&["--strategy", "rename-old"]).strategy(),
            Ok(Strategy::RenameOld {
                suffix: "_old".to_string()
            })
        );
    }

    #[test]
    fn replace_has_no_suffix() {
        assert_eq!(
            parse(&["--strategy", "replace"]).strategy(),
            Ok(Strategy::Replace)
        );
    }

    #[test]
    fn out_means_save_to_that_folder() {
        let args = parse(&["--out", "C:\\music\\out"]);
        assert_eq!(args.strategy_settings().kind, StrategyKind::SaveTo);
        assert_eq!(
            args.strategy(),
            Ok(Strategy::SaveTo {
                directory: PathBuf::from("C:\\music\\out")
            })
        );
    }

    #[test]
    fn out_cannot_be_combined_with_a_strategy() {
        let result = Wrapper::try_parse_from(["test", "--out", "x", "--strategy", "replace"]);
        assert!(result.is_err());
    }

    #[test]
    fn a_serial_suffix_without_digits_is_reported() {
        let error = parse(&["--suffix", "_new"]).strategy().unwrap_err();
        assert!(error.contains("digits"), "{error}");
    }
}
