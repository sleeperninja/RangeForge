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

    /// Whether this codec can hold audio in the quad layout (front left and right, back left and
    /// right). ALAC cannot: FFmpeg turns it into five channels with a silent centre, so the
    /// program refuses it and the other codecs are offered instead (decision D-35).
    pub fn can_write_quad(self) -> bool {
        self != Self::Alac
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

    /// Whether this codec can be written at `hz`. FLAC, ALAC and PCM take every rate offered
    /// here; FFmpeg's AAC encoder stops at 96 kHz.
    pub fn supports_rate(self, hz: u32) -> bool {
        !matches!(self, Self::Aac) || hz <= 96_000
    }
}

/// The kind of file the audio is wrapped in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Container {
    /// The codec's usual file: `.m4a` for ALAC and AAC, `.flac`, `.wav`.
    #[default]
    Native,
    /// Matroska audio, `.mka`. Every codec offered here can be stored in it.
    Mka,
}

impl Container {
    /// Every choice, in the order a picker shows them.
    pub const ALL: [Self; 2] = [Self::Native, Self::Mka];

    /// The name used in saved settings and on the command line: `native` or `mka`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Mka => "mka",
        }
    }

    /// What to show for this choice when the codec is `codec`: the usual file's ending
    /// (`m4a`, `flac`, `wav`), or `mka`.
    pub fn label(self, codec: Codec) -> &'static str {
        match self {
            Self::Native => codec.extension(),
            Self::Mka => "mka",
        }
    }

    /// The choice with this saved name, ignoring case.
    pub fn from_name(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|container| container.name().eq_ignore_ascii_case(text.trim()))
    }
}

/// The sample rate of the files written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SampleRate {
    /// Whatever each source has.
    #[default]
    Original,
    Hz44100,
    Hz48000,
    Hz88200,
    Hz96000,
    Hz176400,
    Hz192000,
}

impl SampleRate {
    /// Every choice, in the order a picker shows them.
    pub const ALL: [Self; 7] = [
        Self::Original,
        Self::Hz44100,
        Self::Hz48000,
        Self::Hz88200,
        Self::Hz96000,
        Self::Hz176400,
        Self::Hz192000,
    ];

    /// The rate in hertz, or `None` for the source's own.
    pub fn hz(self) -> Option<u32> {
        match self {
            Self::Original => None,
            Self::Hz44100 => Some(44_100),
            Self::Hz48000 => Some(48_000),
            Self::Hz88200 => Some(88_200),
            Self::Hz96000 => Some(96_000),
            Self::Hz176400 => Some(176_400),
            Self::Hz192000 => Some(192_000),
        }
    }

    /// The name used in saved settings and on the command line: `original`, or the rate in
    /// hertz such as `96000`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Hz44100 => "44100",
            Self::Hz48000 => "48000",
            Self::Hz88200 => "88200",
            Self::Hz96000 => "96000",
            Self::Hz176400 => "176400",
            Self::Hz192000 => "192000",
        }
    }

    /// The name shown on screen.
    pub fn label(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Hz44100 => "44.1 kHz",
            Self::Hz48000 => "48 kHz",
            Self::Hz88200 => "88.2 kHz",
            Self::Hz96000 => "96 kHz",
            Self::Hz176400 => "176.4 kHz",
            Self::Hz192000 => "192 kHz",
        }
    }

    /// The choice with this saved name, ignoring case.
    pub fn from_name(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|rate| rate.name().eq_ignore_ascii_case(text.trim()))
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
    pub container: Container,
    pub sample_rate: SampleRate,
}

impl OutputFormat {
    /// A format in the codec's usual file, at the source's own sample rate.
    pub fn new(codec: Codec, bit_depth: BitDepth) -> Self {
        Self {
            codec,
            bit_depth,
            container: Container::Native,
            sample_rate: SampleRate::Original,
        }
    }

    #[must_use]
    pub fn with_container(self, container: Container) -> Self {
        Self { container, ..self }
    }

    #[must_use]
    pub fn with_sample_rate(self, sample_rate: SampleRate) -> Self {
        Self {
            sample_rate,
            ..self
        }
    }

    /// The ending of the files written: `m4a`, `flac`, `wav`, or `mka`.
    pub fn extension(self) -> &'static str {
        self.container.label(self.codec)
    }

    /// The FFmpeg container name.
    pub(crate) fn muxer(self) -> &'static str {
        match self.container {
            Container::Native => self.codec.muxer(),
            Container::Mka => "matroska",
        }
    }

    /// True if tags should be named the way FLAC and Matroska name them (`ALBUMARTIST`,
    /// `TRACKNUMBER`) and not the MP4 way (`album_artist`, `track` as `n/total`).
    pub(crate) fn uses_vorbis_style_tags(self) -> bool {
        self.codec == Codec::Flac || self.container == Container::Mka
    }

    /// The `aresample` filter options that change the sample rate to `target`, or `None` when
    /// the source already has it (or no rate was asked for). The resampler is set to a long
    /// filter, since quality matters more than speed here.
    pub(crate) fn resample_options(self, source_rate: u32) -> Option<String> {
        let target = self.sample_rate.hz().filter(|hz| *hz != source_rate)?;
        Some(format!(
            "osr={target}:filter_size=64:phase_shift=14:cutoff=0.97"
        ))
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
    /// Applying a gain to 16-bit audio, reducing 24-bit audio to 16, or converting the signal (a new
    /// sample rate, or a mix of the channels) produces values that fall between 16-bit steps. Rounding them plainly adds a faint harmonic distortion;
    /// triangular dither trades that for a steady noise floor about 96 dB down. An untouched
    /// 16-bit sample at 0 dB is already exact, and is left alone so it stays bit for bit
    /// identical.
    pub fn needs_dither(self, source_bits: Option<u32>, gain_db: f64, converted: bool) -> bool {
        let bits = self.resolved_bits(source_bits);
        let reduced = source_bits.is_none_or(|source| source > 16);
        self.codec.is_lossless() && bits == 16 && (gain_db.abs() > 1e-6 || reduced || converted)
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
    fn mka_is_an_option_for_every_codec_and_names_the_file_accordingly() {
        for codec in Codec::ALL {
            let native = OutputFormat::new(codec, BitDepth::Source);
            assert_eq!(native.extension(), codec.extension());
            let mka = native.with_container(Container::Mka);
            assert_eq!(mka.extension(), "mka");
            assert_eq!(mka.muxer(), "matroska");
            assert!(mka.uses_vorbis_style_tags());
        }
        assert_eq!(alac().muxer(), "ipod");
        assert!(!alac().uses_vorbis_style_tags());
        assert!(
            OutputFormat::new(Codec::Flac, BitDepth::Source).uses_vorbis_style_tags(),
            "FLAC names its tags that way natively"
        );
    }

    #[test]
    fn container_labels_follow_the_codec() {
        assert_eq!(Container::Native.label(Codec::Alac), "m4a");
        assert_eq!(Container::Native.label(Codec::Flac), "flac");
        assert_eq!(Container::Mka.label(Codec::Wav), "mka");
        for container in Container::ALL {
            assert_eq!(Container::from_name(container.name()), Some(container));
        }
        assert_eq!(Container::from_name(" MKA "), Some(Container::Mka));
        assert_eq!(Container::from_name("ogg"), None);
    }

    #[test]
    fn sample_rates_have_names_labels_and_hertz() {
        assert_eq!(SampleRate::default(), SampleRate::Original);
        assert_eq!(SampleRate::Original.hz(), None);
        assert_eq!(SampleRate::Hz88200.hz(), Some(88_200));
        assert_eq!(SampleRate::Hz176400.label(), "176.4 kHz");
        for rate in SampleRate::ALL {
            assert_eq!(SampleRate::from_name(rate.name()), Some(rate));
            if let Some(hz) = rate.hz() {
                assert_eq!(rate.name(), hz.to_string());
            }
        }
        assert_eq!(SampleRate::from_name("22050"), None);
    }

    #[test]
    fn aac_stops_at_96_khz() {
        for rate in SampleRate::ALL.into_iter().filter_map(SampleRate::hz) {
            assert!(Codec::Flac.supports_rate(rate), "{rate}");
            assert!(Codec::Alac.supports_rate(rate), "{rate}");
            assert_eq!(Codec::Aac.supports_rate(rate), rate <= 96_000, "{rate}");
        }
    }

    #[test]
    fn resampling_is_only_asked_for_when_the_rate_changes() {
        let at_96 = alac().with_sample_rate(SampleRate::Hz96000);
        assert!(
            at_96
                .resample_options(44_100)
                .unwrap()
                .starts_with("osr=96000:")
        );
        assert_eq!(at_96.resample_options(96_000), None, "already there");
        assert_eq!(
            alac().resample_options(44_100),
            None,
            "original means leave it"
        );
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
    fn dither_also_follows_a_converted_signal_and_alac_cannot_hold_quad() {
        // 16 bits at 0 dB is left alone, until the signal is converted (a new rate or a mix).
        assert!(!alac().needs_dither(Some(16), 0.0, false));
        assert!(alac().needs_dither(Some(16), 0.0, true));
        // 24-bit output and lossy codecs are never dithered, converted or not.
        assert!(!alac().needs_dither(Some(24), 0.0, true));
        assert!(!OutputFormat::new(Codec::Aac, BitDepth::Source).needs_dither(Some(16), 0.0, true));
        // Quad: every codec but ALAC (FFmpeg makes it five channels with a silent centre).
        assert!(!Codec::Alac.can_write_quad());
        for codec in [Codec::Flac, Codec::Wav, Codec::Aac] {
            assert!(codec.can_write_quad(), "{codec:?}");
        }
    }

    #[test]
    fn dither_only_where_it_helps() {
        // 16 to 16 at 0 dB is exact: no dither, so the result is bit for bit identical.
        assert!(!alac().needs_dither(Some(16), 0.0, false));
        // Any gain on 16-bit audio lands between steps.
        assert!(alac().needs_dither(Some(16), -3.2, false));
        // Asking for 16 bits from a 24-bit source reduces precision, even at 0 dB.
        let down_to_16 = OutputFormat::new(Codec::Alac, BitDepth::Sixteen);
        assert!(down_to_16.needs_dither(Some(24), 0.0, false));
        // Keeping the source's 24 bits needs no dither: the noise floor is far below
        // anything audible, with or without a gain.
        assert!(!alac().needs_dither(Some(24), 0.0, false));
        assert!(!alac().needs_dither(Some(24), -3.2, false));
        // Lossy output does its own thing.
        assert!(
            !OutputFormat::new(Codec::Aac, BitDepth::Source).needs_dither(Some(16), -3.0, false)
        );
        // A lossy source decoded to 16 bits is a reduction.
        assert!(alac().needs_dither(None, 0.0, false));
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
