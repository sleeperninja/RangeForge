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
    /// Lossy MP3 (LAME), in an `.mp3` file. Holds two channels at most.
    Mp3,
    /// Lossy Opus, in an `.opus` file. Always 48 kHz inside.
    Opus,
}

/// How much a lossy encoder is allowed to spend: a steady bitrate, or LAME's variable-rate
/// scale (V0 is the best, V4 the smallest offered).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bitrate {
    /// A steady rate in kilobits per second.
    Kbps(u16),
    /// LAME's variable-rate quality, 0 (best) to 4.
    Vbr(u8),
}

impl Bitrate {
    /// The name used in saved settings and on the command line: `320`, or `v0`.
    pub fn name(self) -> String {
        match self {
            Self::Kbps(rate) => rate.to_string(),
            Self::Vbr(level) => format!("v{level}"),
        }
    }

    /// What the picker says: `320 kbps`, or `V0 (about 245 kbps)`.
    pub fn label(self) -> String {
        match self {
            Self::Kbps(rate) => format!("{rate} kbps"),
            Self::Vbr(level) => {
                let average = match level {
                    0 => 245,
                    1 => 225,
                    2 => 190,
                    3 => 175,
                    _ => 165,
                };
                format!("V{level} (~{average} kbps)")
            }
        }
    }

    /// The choice with this saved name, ignoring case.
    pub fn from_name(text: &str) -> Option<Self> {
        let text = text.trim().to_ascii_lowercase();
        match text.strip_prefix('v') {
            Some(level) => level.parse().ok().map(Self::Vbr),
            None => text.parse().ok().map(Self::Kbps),
        }
    }
}

/// The bitrates offered for MP3, from smallest to best. The variable-rate steps sit where their
/// average rate falls among the steady ones; V0 closes the list as the top quality choice.
const MP3_BITRATES: [Bitrate; 10] = [
    Bitrate::Kbps(128),
    Bitrate::Kbps(160),
    Bitrate::Vbr(4),
    Bitrate::Vbr(3),
    Bitrate::Vbr(2),
    Bitrate::Kbps(192),
    Bitrate::Vbr(1),
    Bitrate::Kbps(256),
    Bitrate::Kbps(320),
    Bitrate::Vbr(0),
];

/// Opus is variable-rate already: these are the rates it aims for.
const OPUS_BITRATES: [Bitrate; 6] = [
    Bitrate::Kbps(96),
    Bitrate::Kbps(128),
    Bitrate::Kbps(160),
    Bitrate::Kbps(192),
    Bitrate::Kbps(256),
    Bitrate::Kbps(320),
];

const AAC_BITRATES: [Bitrate; 5] = [
    Bitrate::Kbps(128),
    Bitrate::Kbps(160),
    Bitrate::Kbps(192),
    Bitrate::Kbps(256),
    Bitrate::Kbps(320),
];

/// The FLAC compression levels: 0 is the largest file and 8 the smallest. The sound is the same
/// at every level; only the size and the time it takes change.
pub const FLAC_LEVELS: [u8; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];
/// FLAC's own default.
pub const DEFAULT_FLAC_LEVEL: u8 = 5;

impl Codec {
    /// Every codec, in the order a picker shows them.
    pub const ALL: [Self; 6] = [
        Self::Alac,
        Self::Flac,
        Self::Wav,
        Self::Aac,
        Self::Mp3,
        Self::Opus,
    ];

    /// The short name used on screen and in saved settings, such as `alac`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Alac => "alac",
            Self::Flac => "flac",
            Self::Wav => "wav",
            Self::Aac => "aac",
            Self::Mp3 => "mp3",
            Self::Opus => "opus",
        }
    }

    /// Whether this codec can hold audio in the quad layout (front left and right, back left and
    /// right). ALAC cannot: FFmpeg turns it into five channels with a silent centre, and MP3
    /// holds two channels at most, so the program refuses them and the other codecs are offered
    /// instead (decision D-35).
    pub fn can_write_quad(self) -> bool {
        !matches!(self, Self::Alac | Self::Mp3)
    }

    /// The most channels a file of this codec can hold (of the layouts offered here).
    pub fn max_channels(self) -> usize {
        if self == Self::Mp3 { 2 } else { 8 }
    }

    /// The bitrates offered for this codec, smallest first. Empty for the lossless ones.
    pub fn bitrates(self) -> &'static [Bitrate] {
        match self {
            Self::Aac => &AAC_BITRATES,
            Self::Mp3 => &MP3_BITRATES,
            Self::Opus => &OPUS_BITRATES,
            Self::Alac | Self::Flac | Self::Wav => &[],
        }
    }

    /// The bitrate chosen until the user picks another: 256 for AAC, 320 for MP3, 192 for Opus.
    pub fn default_bitrate(self) -> Bitrate {
        match self {
            Self::Mp3 => Bitrate::Kbps(320),
            Self::Opus => Bitrate::Kbps(192),
            _ => Bitrate::Kbps(256),
        }
    }

    /// The only sample rate this codec can be written at, if there is just one. Opus always
    /// runs at 48 kHz.
    pub fn fixed_rate(self) -> Option<u32> {
        (self == Self::Opus).then_some(48_000)
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
            Self::Mp3 => "mp3",
            Self::Opus => "opus",
        }
    }

    /// The FFmpeg container name. `ipod` is what makes `.m4a` files that iTunes accepts.
    pub(crate) fn muxer(self) -> &'static str {
        match self {
            Self::Alac | Self::Aac => "ipod",
            Self::Flac => "flac",
            Self::Wav => "wav",
            Self::Mp3 => "mp3",
            Self::Opus => "opus",
        }
    }

    pub fn is_lossless(self) -> bool {
        matches!(self, Self::Alac | Self::Flac | Self::Wav)
    }

    /// Whether this codec can be written at `hz`. FLAC, ALAC and PCM take every rate offered
    /// here; FFmpeg's AAC encoder stops at 96 kHz, LAME at 48 kHz, and Opus is 48 kHz only.
    pub fn supports_rate(self, hz: u32) -> bool {
        match self {
            Self::Aac => hz <= 96_000,
            Self::Mp3 => hz <= 48_000,
            Self::Opus => hz == 48_000,
            Self::Alac | Self::Flac | Self::Wav => true,
        }
    }

    /// The rate to write when a source has `source` Hz and the user asked for `chosen`
    /// (`None` is "original"). `None` back means "keep the source's own". A source rate the
    /// codec cannot write is replaced by the nearest one below in the same family (96 for 192,
    /// 88.2 for 176.4), and Opus is always 48 kHz.
    pub fn target_rate(self, chosen: Option<u32>, source: u32) -> Option<u32> {
        if let Some(fixed) = self.fixed_rate() {
            return Some(fixed);
        }
        if let Some(hz) = chosen {
            return Some(hz);
        }
        if self.supports_rate(source) {
            return None;
        }
        let top = if self == Self::Mp3 { 48_000 } else { 96_000 };
        Some(if source.is_multiple_of(48_000) {
            top
        } else {
            top * 147 / 160
        })
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
            Self::Original => "source",
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
    /// 32-bit floating point, WAV only. Whole-number samples stop at full scale; floating
    /// point does not, so peaks above 0 dB are kept (decision D-41).
    Float32,
}

impl BitDepth {
    /// Every choice, in the order a picker shows them.
    pub const ALL: [Self; 4] = [Self::Source, Self::Sixteen, Self::TwentyFour, Self::Float32];

    /// Whether `codec` can be written at this depth. Only WAV can be written as 32-bit float.
    pub fn fits(self, codec: Codec) -> bool {
        self != Self::Float32 || codec == Codec::Wav
    }

    /// The name used in saved settings and on the command line: `source`, `16` or `24`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Sixteen => "16",
            Self::TwentyFour => "24",
            Self::Float32 => "32f",
        }
    }

    /// The name shown on screen.
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Sixteen => "16-bit",
            Self::TwentyFour => "24-bit",
            Self::Float32 => "32-bit float",
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
    /// What a lossy codec aims for; ignored by the lossless ones.
    pub bitrate: Bitrate,
    /// FLAC's compression level, 0 to 8; ignored by every other codec.
    pub flac_level: u8,
}

impl OutputFormat {
    /// A format in the codec's usual file, at the source's own sample rate.
    pub fn new(codec: Codec, bit_depth: BitDepth) -> Self {
        Self {
            codec,
            bit_depth,
            container: Container::Native,
            sample_rate: SampleRate::Original,
            bitrate: codec.default_bitrate(),
            flac_level: DEFAULT_FLAC_LEVEL,
        }
    }

    #[must_use]
    pub fn with_bitrate(self, bitrate: Bitrate) -> Self {
        Self { bitrate, ..self }
    }

    #[must_use]
    pub fn with_flac_level(self, flac_level: u8) -> Self {
        Self { flac_level, ..self }
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
        matches!(self.codec, Codec::Flac | Codec::Opus) || self.container == Container::Mka
    }

    /// The `aresample` filter options that change the sample rate to `target`, or `None` when
    /// the source already has it (or no rate was asked for). The resampler is set to a long
    /// filter, since quality matters more than speed here.
    pub(crate) fn resample_options(self, source_rate: u32) -> Option<String> {
        let target = self
            .codec
            .target_rate(self.sample_rate.hz(), source_rate)
            .filter(|hz| *hz != source_rate)?;
        Some(format!(
            "osr={target}:filter_size=64:phase_shift=14:cutoff=0.97"
        ))
    }

    /// The bit depth actually written (16 or 24), given what the source reports. A source with
    /// no fixed precision, such as MP3, becomes 16.
    pub fn resolved_bits(self, source_bits: Option<u32>) -> u32 {
        match self.bit_depth {
            BitDepth::Sixteen => 16,
            BitDepth::Float32 if self.codec == Codec::Wav => 32,
            BitDepth::TwentyFour | BitDepth::Float32 => 24,
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
            Codec::Flac => {
                let mut args = if wide {
                    strings(&[
                        "-c:a",
                        "flac",
                        "-sample_fmt",
                        "s32",
                        "-bits_per_raw_sample",
                        "24",
                    ])
                } else {
                    strings(&["-c:a", "flac", "-sample_fmt", "s16"])
                };
                args.push("-compression_level".to_string());
                args.push(self.flac_level.min(8).to_string());
                args
            }
            Codec::Wav if bits > 24 => strings(&["-c:a", "pcm_f32le"]),
            Codec::Wav => strings(&["-c:a", if wide { "pcm_s24le" } else { "pcm_s16le" }]),
            Codec::Aac => self.lossy_args("aac"),
            Codec::Mp3 => self.lossy_args("libmp3lame"),
            Codec::Opus => {
                let mut args = self.lossy_args("libopus");
                args.push("-vbr".to_string());
                args.push("on".to_string());
                args
            }
        }
    }

    /// The encoder and its bitrate (`-b:a 320k`) or, for LAME's variable rate, its quality.
    fn lossy_args(self, encoder: &str) -> Vec<String> {
        let mut args = vec!["-c:a".to_string(), encoder.to_string()];
        match self.bitrate {
            Bitrate::Kbps(rate) => {
                args.push("-b:a".to_string());
                args.push(format!("{rate}k"));
            }
            Bitrate::Vbr(level) => {
                args.push("-q:a".to_string());
                args.push(level.min(9).to_string());
            }
        }
        args
    }

    /// The sample format the encoder wants, used to make the dither filter convert to it.
    pub(crate) fn sample_format(self, bits: u32) -> &'static str {
        match (self.codec, bits > 16) {
            (Codec::Alac, false) => "s16p",
            (Codec::Alac, true) => "s32p",
            (Codec::Flac | Codec::Wav, false) => "s16",
            (Codec::Wav, true) if bits > 24 => "flt",
            (Codec::Flac | Codec::Wav, true) => "s32",
            (Codec::Aac | Codec::Mp3, _) => "fltp",
            (Codec::Opus, _) => "flt",
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
    fn wav_can_be_32_bit_float_and_nothing_else_can() {
        let wav = OutputFormat::new(Codec::Wav, BitDepth::Float32);
        assert_eq!(wav.resolved_bits(Some(16)), 32);
        assert_eq!(wav.encoder_args(32), ["-c:a", "pcm_f32le"]);
        assert_eq!(wav.sample_format(32), "flt");
        assert!(
            !wav.needs_dither(Some(16), -3.0, true),
            "float is never dithered"
        );
        for codec in [
            Codec::Alac,
            Codec::Flac,
            Codec::Aac,
            Codec::Mp3,
            Codec::Opus,
        ] {
            assert!(!BitDepth::Float32.fits(codec), "{codec:?}");
            assert_eq!(
                OutputFormat::new(codec, BitDepth::Float32).resolved_bits(None),
                24
            );
        }
        assert!(BitDepth::Float32.fits(Codec::Wav) && BitDepth::Sixteen.fits(Codec::Flac));
        assert_eq!(BitDepth::from_name("32f"), Some(BitDepth::Float32));
        assert_eq!(BitDepth::Float32.label(), "32-bit float");
    }

    #[test]
    fn names_round_trip() {
        assert_eq!(Codec::from_name("mp3"), Some(Codec::Mp3));
        assert_eq!(Codec::from_name("OPUS"), Some(Codec::Opus));
        assert_eq!(Codec::from_name("ogg"), None);
        for codec in Codec::ALL {
            assert_eq!(Codec::from_name(codec.name()), Some(codec));
        }
        assert_eq!(Codec::from_name("  ALAC "), Some(Codec::Alac));
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
            [
                "-c:a",
                "flac",
                "-sample_fmt",
                "s16",
                "-compression_level",
                "5"
            ]
        );
        assert_eq!(
            flac.with_flac_level(8).encoder_args(16)[5],
            "8",
            "the level is passed on"
        );
        assert_eq!(
            flac.encoder_args(24),
            [
                "-c:a",
                "flac",
                "-sample_fmt",
                "s32",
                "-bits_per_raw_sample",
                "24",
                "-compression_level",
                "5"
            ]
        );
        let wav = OutputFormat::new(Codec::Wav, BitDepth::Source);
        assert_eq!(wav.encoder_args(24), ["-c:a", "pcm_s24le"]);
        let aac = OutputFormat::new(Codec::Aac, BitDepth::Source);
        assert_eq!(aac.encoder_args(16), ["-c:a", "aac", "-b:a", "256k"]);
    }

    #[test]
    fn mp3_and_opus_arguments_follow_the_chosen_bitrate() {
        let mp3 = OutputFormat::new(Codec::Mp3, BitDepth::Source);
        assert_eq!(mp3.encoder_args(16), ["-c:a", "libmp3lame", "-b:a", "320k"]);
        assert_eq!(
            mp3.with_bitrate(Bitrate::Vbr(0)).encoder_args(16),
            ["-c:a", "libmp3lame", "-q:a", "0"]
        );
        let opus = OutputFormat::new(Codec::Opus, BitDepth::Source);
        assert_eq!(
            opus.encoder_args(16),
            ["-c:a", "libopus", "-b:a", "192k", "-vbr", "on"]
        );
        let aac = OutputFormat::new(Codec::Aac, BitDepth::Source).with_bitrate(Bitrate::Kbps(128));
        assert_eq!(aac.encoder_args(16), ["-c:a", "aac", "-b:a", "128k"]);
        assert_eq!(mp3.extension(), "mp3");
        assert_eq!(opus.extension(), "opus");
        assert!(opus.uses_vorbis_style_tags());
        assert!(!mp3.uses_vorbis_style_tags());
    }

    #[test]
    fn bitrates_have_names_labels_and_the_lists_the_codecs_offer() {
        for codec in Codec::ALL {
            for bitrate in codec.bitrates() {
                assert_eq!(Bitrate::from_name(&bitrate.name()), Some(*bitrate));
            }
            assert_eq!(codec.bitrates().is_empty(), codec.is_lossless());
            if !codec.is_lossless() {
                assert!(codec.bitrates().contains(&codec.default_bitrate()));
            }
        }
        assert_eq!(Bitrate::from_name(" V2 "), Some(Bitrate::Vbr(2)));
        assert_eq!(Bitrate::from_name("fast"), None);
        assert_eq!(Bitrate::Kbps(320).label(), "320 kbps");
        assert_eq!(Bitrate::Vbr(0).label(), "V0 (~245 kbps)");
        // Nothing below 128 for MP3, and the list ends with 320 and then V0.
        let mp3 = Codec::Mp3.bitrates();
        assert_eq!(mp3.first(), Some(&Bitrate::Kbps(128)));
        assert_eq!(&mp3[mp3.len() - 2..], [Bitrate::Kbps(320), Bitrate::Vbr(0)]);
        assert!(!mp3.contains(&Bitrate::Vbr(5)));
    }

    #[test]
    fn rates_each_codec_can_write_and_the_ones_it_falls_back_to() {
        // Opus is 48 kHz whatever is asked for or found.
        for source in [44_100, 48_000, 96_000, 192_000] {
            assert_eq!(Codec::Opus.target_rate(None, source), Some(48_000));
            assert_eq!(Codec::Opus.target_rate(Some(96_000), source), Some(48_000));
        }
        assert!(Codec::Opus.supports_rate(48_000) && !Codec::Opus.supports_rate(44_100));
        // MP3 stops at 48 kHz; a higher source goes to the nearest rate in its family.
        assert_eq!(Codec::Mp3.target_rate(None, 44_100), None);
        assert_eq!(Codec::Mp3.target_rate(None, 96_000), Some(48_000));
        assert_eq!(Codec::Mp3.target_rate(None, 88_200), Some(44_100));
        assert_eq!(Codec::Mp3.target_rate(None, 176_400), Some(44_100));
        // AAC stops at 96 kHz.
        assert_eq!(Codec::Aac.target_rate(None, 96_000), None);
        assert_eq!(Codec::Aac.target_rate(None, 192_000), Some(96_000));
        assert_eq!(Codec::Aac.target_rate(None, 176_400), Some(88_200));
        // The lossless ones keep what they are given.
        assert_eq!(Codec::Flac.target_rate(None, 192_000), None);
        assert_eq!(Codec::Flac.target_rate(Some(44_100), 96_000), Some(44_100));
    }

    #[test]
    fn mp3_holds_two_channels_and_not_quad() {
        assert_eq!(Codec::Mp3.max_channels(), 2);
        assert_eq!(Codec::Opus.max_channels(), 8);
        assert!(!Codec::Mp3.can_write_quad() && Codec::Opus.can_write_quad());
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
