//! Output formats and the FFmpeg settings that produce them.

/// The audio codec of the files written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    /// Apple Lossless, in an `.m4a` file. The main output.
    Alac,
    Flac,
    /// Uncompressed PCM, in a `.wav` file.
    Wav,
    /// Lossy AAC, in an `.m4a` file.
    Aac,
}

impl Codec {
    /// Every codec, in the order a picker shows them.
    pub const ALL: [Self; 4] = [Self::Alac, Self::Flac, Self::Wav, Self::Aac];

    /// The short name used on screen and in saved settings, such as `alac`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Alac => "alac",
            Self::Flac => "flac",
            Self::Wav => "wav",
            Self::Aac => "aac",
        }
    }

    /// The codec with this short name, ignoring case.
    pub fn from_name(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|codec| codec.name().eq_ignore_ascii_case(text.trim()))
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Alac | Self::Aac => "m4a",
            Self::Flac => "flac",
            Self::Wav => "wav",
        }
    }

    /// The FFmpeg container name. `ipod` is what makes `.m4a` files that iTunes accepts.
    pub(crate) fn muxer(self) -> &'static str {
        match self {
            Self::Alac | Self::Aac => "ipod",
            Self::Flac => "flac",
            Self::Wav => "wav",
        }
    }

    pub fn is_lossless(self) -> bool {
        !matches!(self, Self::Aac)
    }
}

/// How precisely each sample of the output is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BitDepth {
    /// Match the source: 16 stays 16, 24 stays 24.
    #[default]
    Source,
    Sixteen,
    TwentyFour,
}

impl BitDepth {
    /// Every choice, in the order a picker shows them.
    pub const ALL: [Self; 3] = [Self::Source, Self::Sixteen, Self::TwentyFour];

    /// The name used in saved settings and on the command line: `source`, `16` or `24`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Sixteen => "16",
            Self::TwentyFour => "24",
        }
    }

    /// The name shown on screen.
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Sixteen => "16-bit",
            Self::TwentyFour => "24-bit",
        }
    }

    /// The choice with this saved name, ignoring case.
    pub fn from_name(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|depth| depth.name().eq_ignore_ascii_case(text.trim()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputFormat {
    pub codec: Codec,
    pub bit_depth: BitDepth,
}

impl OutputFormat {
    pub fn new(codec: Codec, bit_depth: BitDepth) -> Self {
        Self { codec, bit_depth }
    }

    /// The bit depth actually written (16 or 24), given what the source reports. A source with
    /// no fixed precision, such as MP3, becomes 16.
    pub fn resolved_bits(self, source_bits: Option<u32>) -> u32 {
        match self.bit_depth {
            BitDepth::Sixteen => 16,
            BitDepth::TwentyFour => 24,
            BitDepth::Source => match source_bits {
                Some(bits) if bits > 16 => 24,
                _ => 16,
            },
        }
    }

    /// True when the samples should be dithered on their way down to 16 bits.
    ///
    /// Applying a gain to 16-bit audio, or reducing 24-bit audio to 16, produces values that
    /// fall between 16-bit steps. Rounding them plainly adds a faint harmonic distortion;
    /// triangular dither trades that for a steady noise floor about 96 dB down. An untouched
    /// 16-bit sample at 0 dB is already exact, and is left alone so it stays bit for bit
    /// identical.
    pub fn needs_dither(self, source_bits: Option<u32>, gain_db: f64) -> bool {
        let bits = self.resolved_bits(source_bits);
        let reduced = source_bits.is_none_or(|source| source > 16);
        self.codec.is_lossless() && bits == 16 && (gain_db.abs() > 1e-6 || reduced)
    }

    /// The FFmpeg arguments that choose the encoder and sample format.
    pub(crate) fn encoder_args(self, bits: u32) -> Vec<String> {
        let wide = bits > 16;
        let strings = |items: &[&str]| items.iter().map(|s| (*s).to_string()).collect();
        match self.codec {
            Codec::Alac => strings(&[
                "-c:a",
                "alac",
                "-sample_fmt",
                if wide { "s32p" } else { "s16p" },
            ]),
            Codec::Flac if wide => strings(&[
                "-c:a",
                "flac",
                "-sample_fmt",
                "s32",
                "-bits_per_raw_sample",
                "24",
            ]),
            Codec::Flac => strings(&["-c:a", "flac", "-sample_fmt", "s16"]),
            Codec::Wav => strings(&["-c:a", if wide { "pcm_s24le" } else { "pcm_s16le" }]),
            Codec::Aac => strings(&["-c:a", "aac", "-b:a", "256k"]),
        }
    }

    /// The sample format the encoder wants, used to make the dither filter convert to it.
    pub(crate) fn sample_format(self, bits: u32) -> &'static str {
        match (self.codec, bits > 16) {
            (Codec::Alac, false) => "s16p",
            (Codec::Alac, true) => "s32p",
            (Codec::Flac | Codec::Wav, false) => "s16",
            (Codec::Flac | Codec::Wav, true) => "s32",
            (Codec::Aac, _) => "fltp",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alac() -> OutputFormat {
        OutputFormat::new(Codec::Alac, BitDepth::Source)
    }

    #[test]
    fn extensions_and_containers() {
        assert_eq!(Codec::Alac.extension(), "m4a");
        assert_eq!(Codec::Aac.extension(), "m4a");
        assert_eq!(Codec::Flac.extension(), "flac");
        assert_eq!(Codec::Wav.extension(), "wav");
        assert_eq!(Codec::Alac.muxer(), "ipod");
    }

    #[test]
    fn names_round_trip() {
        for codec in Codec::ALL {
            assert_eq!(Codec::from_name(codec.name()), Some(codec));
        }
        assert_eq!(Codec::from_name("  ALAC "), Some(Codec::Alac));
        assert_eq!(Codec::from_name("mp3"), None);
        for depth in BitDepth::ALL {
            assert_eq!(BitDepth::from_name(depth.name()), Some(depth));
        }
        assert_eq!(BitDepth::from_name("32"), None);
        assert_eq!(BitDepth::TwentyFour.label(), "24-bit");
    }

    #[test]
    fn source_bit_depth_is_kept_when_asked() {
        assert_eq!(alac().resolved_bits(Some(16)), 16);
        assert_eq!(alac().resolved_bits(Some(24)), 24);
        // 32-bit sources are written as 24, the widest the lossless codecs here store.
        assert_eq!(alac().resolved_bits(Some(32)), 24);
        // A lossy source has no fixed precision.
        assert_eq!(alac().resolved_bits(None), 16);
    }

    #[test]
    fn an_explicit_bit_depth_wins() {
        let sixteen = OutputFormat::new(Codec::Flac, BitDepth::Sixteen);
        let twenty_four = OutputFormat::new(Codec::Flac, BitDepth::TwentyFour);
        assert_eq!(sixteen.resolved_bits(Some(24)), 16);
        assert_eq!(twenty_four.resolved_bits(Some(16)), 24);
    }

    #[test]
    fn dither_only_where_it_helps() {
        // 16 to 16 at 0 dB is exact: no dither, so the result is bit for bit identical.
        assert!(!alac().needs_dither(Some(16), 0.0));
        // Any gain on 16-bit audio lands between steps.
        assert!(alac().needs_dither(Some(16), -3.2));
        // Asking for 16 bits from a 24-bit source reduces precision, even at 0 dB.
        let down_to_16 = OutputFormat::new(Codec::Alac, BitDepth::Sixteen);
        assert!(down_to_16.needs_dither(Some(24), 0.0));
        // Keeping the source's 24 bits needs no dither: the noise floor is far below
        // anything audible, with or without a gain.
        assert!(!alac().needs_dither(Some(24), 0.0));
        assert!(!alac().needs_dither(Some(24), -3.2));
        // Lossy output does its own thing.
        assert!(!OutputFormat::new(Codec::Aac, BitDepth::Source).needs_dither(Some(16), -3.0));
        // A lossy source decoded to 16 bits is a reduction.
        assert!(alac().needs_dither(None, 0.0));
    }

    #[test]
    fn encoder_arguments_per_codec() {
        assert_eq!(
            alac().encoder_args(16),
            ["-c:a", "alac", "-sample_fmt", "s16p"]
        );
        assert_eq!(
            alac().encoder_args(24),
            ["-c:a", "alac", "-sample_fmt", "s32p"]
        );
        let flac = OutputFormat::new(Codec::Flac, BitDepth::Source);
        assert_eq!(
            flac.encoder_args(16),
            ["-c:a", "flac", "-sample_fmt", "s16"]
        );
        assert_eq!(
            flac.encoder_args(24),
            [
                "-c:a",
                "flac",
                "-sample_fmt",
                "s32",
                "-bits_per_raw_sample",
                "24"
            ]
        );
        let wav = OutputFormat::new(Codec::Wav, BitDepth::Source);
        assert_eq!(wav.encoder_args(24), ["-c:a", "pcm_s24le"]);
        let aac = OutputFormat::new(Codec::Aac, BitDepth::Source);
        assert_eq!(aac.encoder_args(16), ["-c:a", "aac", "-b:a", "256k"]);
    }

    #[test]
    fn sample_formats_match_the_encoder_arguments() {
        assert_eq!(alac().sample_format(16), "s16p");
        assert_eq!(alac().sample_format(24), "s32p");
        assert_eq!(
            OutputFormat::new(Codec::Flac, BitDepth::Source).sample_format(24),
            "s32"
        );
        assert_eq!(
            OutputFormat::new(Codec::Aac, BitDepth::Source).sample_format(16),
            "fltp"
        );
    }
}
