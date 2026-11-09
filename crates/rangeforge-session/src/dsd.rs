//! What DSD files are written as.
//!
//! DSD cannot be written back out as it is, so a DSD source needs a real sample rate and bit
//! depth even when the main Output Format says "original" or "source". These are the choices
//! for that, kept in the Settings window under "DSD Format".
//!
//! The rule is simple and the same everywhere: the main Output Format wins wherever it names a
//! value. The DSD values fill in only what the main format leaves open (sample rate and bit
//! depth), or replace the codec and container if the user chose to. Channels are not touched:
//! a six-channel DSD file stays six channels.

use std::path::Path;

use rangeforge_audio::dsd::is_dsd;
use rangeforge_export::format::{BitDepth, Codec, Container, OutputFormat, SampleRate};

/// The Settings window's DSD Format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DsdFormat {
    /// Write DSD files in this codec, or `None` for whatever the main Output Format says.
    pub codec: Option<Codec>,
    /// Likewise for the container.
    pub container: Option<Container>,
    /// Used when the main sample rate is "original". Never `Original` itself.
    pub sample_rate: SampleRate,
    /// Used when the main bit depth is "source". Never `Source` itself.
    pub bit_depth: BitDepth,
}

impl Default for DsdFormat {
    /// 88.2 kHz, 24-bit: a clean whole-number step down from DSD64, with room to spare.
    fn default() -> Self {
        Self {
            codec: None,
            container: None,
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

    /// The format a DSD source is written in.
    #[must_use]
    pub fn resolve(&self, main: OutputFormat) -> OutputFormat {
        let codec = self.codec.unwrap_or(main.codec);
        let bit_depth = match main.bit_depth {
            BitDepth::Source => self.bit_depth,
            named if named.fits(codec) => named,
            _ => self.bit_depth,
        };
        let mut sample_rate = match main.sample_rate {
            SampleRate::Original => self.sample_rate,
            named => named,
        };
        // AAC stops at 96 kHz. If the codec cannot write the rate, take the nearest it can.
        if sample_rate.hz().is_some_and(|hz| !codec.supports_rate(hz)) {
            sample_rate = Self::SAMPLE_RATES
                .into_iter()
                .rev()
                .find(|rate| rate.hz().is_some_and(|hz| codec.supports_rate(hz)))
                .unwrap_or(SampleRate::Hz48000);
        }
        // The bitrate carries over when the new codec offers it, and is the codec's usual one
        // when it does not (a 320 kbps MP3 asked to be Opus is 320, an AAC 256 as MP3 is 256).
        let bitrate = if codec.bitrates().contains(&main.bitrate) || codec.is_lossless() {
            main.bitrate
        } else {
            codec.default_bitrate()
        };
        OutputFormat {
            codec,
            bit_depth,
            container: self.container.unwrap_or(main.container),
            sample_rate,
            bitrate,
            flac_level: main.flac_level,
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
    fn the_dsd_codec_and_container_replace_the_main_ones_when_chosen() {
        let dsd = DsdFormat {
            codec: Some(Codec::Flac),
            container: Some(Container::Mka),
            ..DsdFormat::default()
        };
        let resolved = dsd.resolve(main_format(Codec::Alac));
        assert_eq!(resolved.codec, Codec::Flac);
        assert_eq!(resolved.container, Container::Mka);
    }

    #[test]
    fn a_rate_the_codec_cannot_write_is_brought_down_to_one_it_can() {
        let dsd = DsdFormat {
            codec: Some(Codec::Aac),
            sample_rate: SampleRate::Hz192000,
            ..DsdFormat::default()
        };
        assert_eq!(
            dsd.resolve(main_format(Codec::Alac)).sample_rate,
            SampleRate::Hz96000
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
