//! The command-line options that describe the output, shared by `run` and `ui`.

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use rangeforge_core::mix::{Channels, Downmix, MixPlan};
use rangeforge_export::format::{BitDepth, Codec, Container, SampleRate};
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
enum ContainerArg {
    /// The codec's usual file: .m4a, .flac or .wav
    Native,
    /// Matroska audio, .mka
    Mka,
}

impl From<ContainerArg> for Container {
    fn from(arg: ContainerArg) -> Self {
        match arg {
            ContainerArg::Native => Self::Native,
            ContainerArg::Mka => Self::Mka,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SampleRateArg {
    /// Keep each source's own rate
    Original,
    #[value(name = "44100")]
    Hz44100,
    #[value(name = "48000")]
    Hz48000,
    #[value(name = "88200")]
    Hz88200,
    #[value(name = "96000")]
    Hz96000,
    #[value(name = "176400")]
    Hz176400,
    #[value(name = "192000")]
    Hz192000,
}

impl From<SampleRateArg> for SampleRate {
    fn from(arg: SampleRateArg) -> Self {
        match arg {
            SampleRateArg::Original => Self::Original,
            SampleRateArg::Hz44100 => Self::Hz44100,
            SampleRateArg::Hz48000 => Self::Hz48000,
            SampleRateArg::Hz88200 => Self::Hz88200,
            SampleRateArg::Hz96000 => Self::Hz96000,
            SampleRateArg::Hz176400 => Self::Hz176400,
            SampleRateArg::Hz192000 => Self::Hz192000,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ChannelsArg {
    /// Keep each source's own channels
    Original,
    /// Four channels: front left and right, back left and right (from a 5.1 source)
    Quad,
    /// Two channels (from a 5.1, quad or other source with more)
    Stereo,
    /// One channel
    Mono,
}

impl From<ChannelsArg> for Channels {
    fn from(arg: ChannelsArg) -> Self {
        match arg {
            ChannelsArg::Original => Self::Original,
            ChannelsArg::Quad => Self::Quad,
            ChannelsArg::Stereo => Self::Stereo,
            ChannelsArg::Mono => Self::Mono,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DownmixArg {
    /// Centre and surrounds go into left and right at -3 dB; the LFE is dropped
    Standard,
    /// The same, with the LFE added at -3 dB to the front channels
    Bass,
    /// Left and right as mixed; the centre, surrounds and LFE are not used
    Front,
}

impl From<DownmixArg> for Downmix {
    fn from(arg: DownmixArg) -> Self {
        match arg {
            DownmixArg::Standard => Self::Standard,
            DownmixArg::Bass => Self::WithBass,
            DownmixArg::Front => Self::FrontOnly,
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
    /// The kind of file to wrap the audio in
    #[arg(long, value_enum, default_value = "native")]
    container: ContainerArg,
    /// The sample rate to write, in hertz, or original
    #[arg(long, value_enum, default_value = "original")]
    sample_rate: SampleRateArg,
    /// How many channels to write. A mix only ever reduces them, and files with more than
    /// six channels are always written as they are
    #[arg(long, value_enum, default_value = "original")]
    channels: ChannelsArg,
    /// How the channels that go away are treated, when --channels is not original
    #[arg(long, value_enum, default_value = "standard")]
    downmix: DownmixArg,
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
    /// File name pattern for single files, such as "{name}"
    #[arg(long, default_value = naming::DEFAULT_SINGLE)]
    single_name: String,
}

impl OutputArgs {
    pub fn output_settings(&self) -> OutputSettings {
        OutputSettings {
            codec: self.codec.into(),
            bit_depth: self.bit_depth.into(),
            container: self.container.into(),
            sample_rate: self.sample_rate.into(),
            mix: MixPlan {
                channels: self.channels.into(),
                style: self.downmix.into(),
            },
        }
    }

    /// A plain-words reason the chosen codec cannot write the chosen sample rate or channels,
    /// if so.
    pub fn format_problem(&self) -> Option<String> {
        let settings = self.output_settings();
        if !settings.can_write_mix() {
            return Some(format!(
                "{} cannot hold quad (four-channel) audio: use flac, wav or aac, or another --channels",
                settings.codec.name()
            ));
        }
        let hz = settings.sample_rate.hz()?;
        (!settings.codec.supports_rate(hz)).then(|| {
            format!(
                "{} cannot be written at {}",
                settings.codec.name(),
                settings.sample_rate.label()
            )
        })
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
    fn channels_and_downmix_are_chosen_by_name_and_quad_is_refused_for_alac() {
        let args = parse(&["--channels", "stereo", "--downmix", "bass"]);
        assert_eq!(
            args.output_settings().mix,
            MixPlan {
                channels: Channels::Stereo,
                style: Downmix::WithBass
            }
        );
        assert_eq!(args.format_problem(), None);
        assert_eq!(parse(&[]).output_settings().mix, MixPlan::default());
        // ALAC cannot hold quad: said up front in plain words, and fine for the others.
        let problem = parse(&["--channels", "quad"]).format_problem().unwrap();
        assert!(
            problem.contains("alac") && problem.contains("quad"),
            "{problem}"
        );
        assert_eq!(
            parse(&["--codec", "flac", "--channels", "quad"]).format_problem(),
            None
        );
        assert!(Wrapper::try_parse_from(["test", "--channels", "surround"]).is_err());
        assert!(Wrapper::try_parse_from(["test", "--downmix", "boost"]).is_err());
    }

    #[test]
    fn container_and_sample_rate_are_chosen_by_name() {
        let args = parse(&["--container", "mka", "--sample-rate", "96000"]);
        let settings = args.output_settings();
        assert_eq!(settings.container, Container::Mka);
        assert_eq!(settings.sample_rate, SampleRate::Hz96000);
        assert_eq!(args.format_problem(), None);
        assert!(Wrapper::try_parse_from(["test", "--sample-rate", "22050"]).is_err());
        assert!(Wrapper::try_parse_from(["test", "--container", "ogg"]).is_err());
    }

    #[test]
    fn a_rate_the_codec_cannot_write_is_reported() {
        let args = parse(&["--codec", "aac", "--sample-rate", "192000"]);
        assert_eq!(
            args.format_problem().as_deref(),
            Some("aac cannot be written at 192 kHz")
        );
        assert_eq!(
            parse(&["--codec", "aac", "--sample-rate", "96000"]).format_problem(),
            None
        );
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
