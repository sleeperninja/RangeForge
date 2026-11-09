//! What DSD files are written as.
//!
//! DSD cannot be written back out as it is, so a DSD source needs a real sample rate and bit
//! depth even when the main Output Format says "source". Those two are the only things the
//! Settings window keeps for DSD ("DSD Format"). Everything else (codec, container, bitrate,
//! compression, channels and the downmix) is the main Output Format's own, as it is for any
//! other file.
//!
//! The rule is the same everywhere and applies to each of the two separately: where the main
//! Output Format names a sample rate (or a bit depth), that is used; where it says "source",
//! the DSD value fills in. One being named never forces the other. A six-channel DSD file stays
//! six channels.

use std::path::Path;

use rangeforge_audio::dsd::is_dsd;
use rangeforge_export::format::{BitDepth, OutputFormat, SampleRate};

/// The Settings window's DSD Format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DsdFormat {
    /// Used when the main sample rate is "original". Never `Original` itself.
    pub sample_rate: SampleRate,
    /// Used when the main bit depth is "source". Never `Source` itself.
    pub bit_depth: BitDepth,
}

impl Default for DsdFormat {
    /// 88.2 kHz, 24-bit: a clean whole-number step down from DSD64, with room to spare.
    fn default() -> Self {
        Self {
            sample_rate: SampleRate::Hz88200,
            bit_depth: BitDepth::TwentyFour,
        }
    }
}

impl DsdFormat {
    /// The sample rates that make sense here: every one except "original".
    pub const SAMPLE_RATES: [SampleRate; 6] = [
        SampleRate::Hz44100,
        SampleRate::Hz48000,
        SampleRate::Hz88200,
        SampleRate::Hz96000,
        SampleRate::Hz176400,
        SampleRate::Hz192000,
    ];

    /// The bit depths that make sense here: every one except "source".
    pub const BIT_DEPTHS: [BitDepth; 2] = [BitDepth::Sixteen, BitDepth::TwentyFour];

    /// The format to write `audio` in, given the main Output Format. Anything that is not a
    /// DSD file gets `main` back untouched.
    #[must_use]
    pub fn format_for(&self, main: OutputFormat, audio: &Path) -> OutputFormat {
        if is_dsd(audio) {
            self.resolve(main)
        } else {
            main
        }
    }

    /// The format a DSD source is written in: the main format, with the DSD sample rate and bit
    /// depth filling in whichever of the two the main format leaves at "source".
    #[must_use]
    pub fn resolve(&self, main: OutputFormat) -> OutputFormat {
        let codec = main.codec;
        let bit_depth = match main.bit_depth {
            BitDepth::Source => self.bit_depth,
            named => named,
        };
        let mut sample_rate = match main.sample_rate {
            SampleRate::Original => self.sample_rate,
            named => named,
        };
        // A codec that cannot write the rate (AAC stops at 96 kHz, MP3 at 48, Opus is 48 only)
        // takes the nearest it can.
        if sample_rate.hz().is_some_and(|hz| !codec.supports_rate(hz)) {
            sample_rate = Self::SAMPLE_RATES
                .into_iter()
                .rev()
                .find(|rate| rate.hz().is_some_and(|hz| codec.supports_rate(hz)))
                .unwrap_or(SampleRate::Hz48000);
        }
        OutputFormat {
            bit_depth,
            sample_rate,
            ..main
        }
    }

    /// Chooses the sample rate. "Original" is refused, because DSD has no such rate.
    pub fn set_sample_rate(&mut self, rate: SampleRate) -> Result<(), String> {
        if rate == SampleRate::Original {
            return Err("DSD has no original rate to keep: choose a real one".to_string());
        }
        self.sample_rate = rate;
        Ok(())
    }

    /// Chooses the bit depth. "Source" is refused, because DSD has one bit.
    pub fn set_bit_depth(&mut self, depth: BitDepth) -> Result<(), String> {
        if depth == BitDepth::Source {
            return Err("DSD has no bit depth to keep: choose 16 or 24".to_string());
        }
        self.bit_depth = depth;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rangeforge_export::format::{Codec, Container};

    fn main_format(codec: Codec) -> OutputFormat {
        OutputFormat::new(codec, BitDepth::Source)
    }

    #[test]
    fn open_values_are_filled_in_from_the_dsd_choices() {
        let resolved = DsdFormat::default().resolve(main_format(Codec::Alac));
        assert_eq!(resolved.sample_rate, SampleRate::Hz88200);
        assert_eq!(resolved.bit_depth, BitDepth::TwentyFour);
        assert_eq!(resolved.codec, Codec::Alac);
        assert_eq!(resolved.container, Container::Native);
    }

    #[test]
    fn what_the_main_format_names_always_wins() {
        let main = main_format(Codec::Flac)
            .with_sample_rate(SampleRate::Hz48000)
            .with_container(Container::Mka);
        let main = OutputFormat {
            bit_depth: BitDepth::Sixteen,
            ..main
        };
        let resolved = DsdFormat::default().resolve(main);
        assert_eq!(resolved, main);
    }

    #[test]
    fn the_codec_container_bitrate_and_level_are_the_main_formats_own() {
        let main = main_format(Codec::Flac)
            .with_container(Container::Mka)
            .with_flac_level(8);
        let resolved = DsdFormat::default().resolve(main);
        assert_eq!(resolved.codec, Codec::Flac);
        assert_eq!(resolved.container, Container::Mka);
        assert_eq!(resolved.flac_level, 8);
        let lossy =
            main_format(Codec::Mp3).with_bitrate(rangeforge_export::format::Bitrate::Vbr(0));
        assert_eq!(
            DsdFormat::default().resolve(lossy).bitrate,
            rangeforge_export::format::Bitrate::Vbr(0)
        );
    }

    #[test]
    fn the_sample_rate_and_the_bit_depth_fall_back_on_their_own() {
        let dsd = DsdFormat::default();
        // 32-bit float WAV with the rate left at source: the float stays, the rate is DSD's.
        let float = OutputFormat::new(Codec::Wav, BitDepth::Float32);
        let resolved = dsd.resolve(float);
        assert_eq!(resolved.bit_depth, BitDepth::Float32);
        assert_eq!(resolved.sample_rate, SampleRate::Hz88200);
        assert!(resolved.keeps_overs());
        // A named rate with the depth left at source: the rate stays, the depth is DSD's.
        let rate = main_format(Codec::Alac).with_sample_rate(SampleRate::Hz48000);
        let resolved = dsd.resolve(rate);
        assert_eq!(resolved.sample_rate, SampleRate::Hz48000);
        assert_eq!(resolved.bit_depth, BitDepth::TwentyFour);
    }

    #[test]
    fn a_rate_the_codec_cannot_write_is_brought_down_to_one_it_can() {
        let dsd = DsdFormat {
            sample_rate: SampleRate::Hz192000,
            ..DsdFormat::default()
        };
        assert_eq!(
            dsd.resolve(main_format(Codec::Aac)).sample_rate,
            SampleRate::Hz96000
        );
        assert_eq!(
            dsd.resolve(main_format(Codec::Mp3)).sample_rate,
            SampleRate::Hz48000
        );
        assert_eq!(
            dsd.resolve(main_format(Codec::Opus)).sample_rate,
            SampleRate::Hz48000
        );
        let named = main_format(Codec::Aac).with_sample_rate(SampleRate::Hz176400);
        assert_eq!(
            DsdFormat::default().resolve(named).sample_rate,
            SampleRate::Hz96000
        );
    }

    #[test]
    fn only_dsd_files_are_changed() {
        let dsd = DsdFormat::default();
        let main = main_format(Codec::Alac);
        assert_eq!(dsd.format_for(main, Path::new("a/song.flac")), main);
        assert_ne!(dsd.format_for(main, Path::new("a/song.dsf")), main);
        assert_ne!(dsd.format_for(main, Path::new("a/SONG.DFF")), main);
    }

    #[test]
    fn original_and_source_are_refused_for_dsd() {
        let mut dsd = DsdFormat::default();
        assert!(dsd.set_sample_rate(SampleRate::Original).is_err());
        assert!(dsd.set_bit_depth(BitDepth::Source).is_err());
        assert_eq!(dsd, DsdFormat::default());
        dsd.set_sample_rate(SampleRate::Hz176400).unwrap();
        dsd.set_bit_depth(BitDepth::Sixteen).unwrap();
        assert_eq!(dsd.sample_rate, SampleRate::Hz176400);
        assert_eq!(dsd.bit_depth, BitDepth::Sixteen);
    }
}
