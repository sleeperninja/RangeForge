//! Mixing channels down: 5.1 to quad, stereo or mono, quad to stereo or mono, stereo to mono.
//!
//! Mixing channels is like turning a colour photo into black and white: there is more than one
//! honest way, and they do not sound the same. So the choice has two parts, what to end up with
//! ([`Channels`]) and how the channels that go away are treated ([`Downmix`]). The rules are in
//! decision D-35: a mix only ever reduces the channel count, nothing is upmixed, files with more
//! than six channels are left as they are, and there is no centre boost.
//!
//! Channel order is the usual one for decoded audio (what WAVE, FLAC and FFmpeg use):
//! 5.1 is front left, front right, centre, LFE, back left, back right; quad is front left,
//! front right, back left, back right.

/// Minus 3 dB as a multiplier: how loud the centre and the surrounds go into left and right.
pub const MINUS_3_DB: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// How many channels to end up with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Channels {
    /// Leave the channels as they are.
    #[default]
    Original,
    /// Four: front left, front right, back left, back right.
    Quad,
    Stereo,
    Mono,
}

impl Channels {
    /// Every choice, in the order a picker shows them.
    pub const ALL: [Self; 4] = [Self::Original, Self::Quad, Self::Stereo, Self::Mono];

    /// The short name used on screen and in saved settings, such as `stereo`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Quad => "quad",
            Self::Stereo => "stereo",
            Self::Mono => "mono",
        }
    }

    /// The choice with this short name, ignoring case.
    #[must_use]
    pub fn from_name(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|channels| channels.name().eq_ignore_ascii_case(text.trim()))
    }

    /// What the picker says.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Original => "source",
            Self::Quad => "quad (4.0)",
            Self::Stereo => "stereo (2.0)",
            Self::Mono => "mono (1.0)",
        }
    }
}

/// How the channels that go away are treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Downmix {
    /// The centre and the surrounds go into left and right at -3 dB; the LFE is dropped.
    #[default]
    Standard,
    /// The same, and the LFE is added at -3 dB to the front channels.
    WithBass,
    /// Left and right as they were mixed. The centre, the surrounds and the LFE are not used.
    FrontOnly,
}

impl Downmix {
    /// Every style, in the order a picker shows them.
    pub const ALL: [Self; 3] = [Self::Standard, Self::WithBass, Self::FrontOnly];

    /// The short name used in saved settings, such as `bass`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::WithBass => "bass",
            Self::FrontOnly => "front",
        }
    }

    /// The style with this short name, ignoring case.
    #[must_use]
    pub fn from_name(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|style| style.name().eq_ignore_ascii_case(text.trim()))
    }

    /// What the picker says.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::WithBass => "Standard + bass",
            Self::FrontOnly => "Front pair only",
        }
    }
}

/// What to do about the channels: the two choices together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MixPlan {
    pub channels: Channels,
    pub style: Downmix,
}

impl MixPlan {
    /// A short text that names this plan, for saved measurements to be keyed by. It is empty for
    /// a plan that leaves the audio alone, so what was measured before channels could be mixed
    /// still counts.
    #[must_use]
    pub fn key(self) -> String {
        match self.channels {
            Channels::Original => String::new(),
            channels => format!("{}-{}", channels.name(), self.style.name()),
        }
    }
}

/// A set of mixing coefficients: each output channel is a sum of the input channels, each
/// scaled. Stored row by row, one row per output channel.
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    inputs: usize,
    outputs: usize,
    rows: Vec<f32>,
}

impl Matrix {
    fn new(inputs: usize, outputs: usize, weights: &[&[(usize, f32)]]) -> Self {
        let mut rows = vec![0.0; inputs * outputs];
        for (out, taps) in weights.iter().enumerate() {
            for &(input, weight) in *taps {
                rows[out * inputs + input] = weight;
            }
        }
        Self {
            inputs,
            outputs,
            rows,
        }
    }

    /// How many channels come in.
    #[must_use]
    pub fn inputs(&self) -> usize {
        self.inputs
    }

    /// How many channels come out.
    #[must_use]
    pub fn outputs(&self) -> usize {
        self.outputs
    }

    /// The weight of input channel `input` in output channel `output`.
    #[must_use]
    pub fn weight(&self, output: usize, input: usize) -> f32 {
        self.rows[output * self.inputs + input]
    }

    /// Mixes interleaved frames: `source` holds whole frames of `inputs` channels, and the same
    /// frames, mixed, are put in `mixed` (which is emptied first) as frames of `outputs` channels.
    pub fn apply(&self, source: &[f32], mixed: &mut Vec<f32>) {
        mixed.clear();
        mixed.reserve(source.len() / self.inputs * self.outputs);
        for frame in source.chunks_exact(self.inputs) {
            for row in self.rows.chunks_exact(self.inputs) {
                mixed.push(
                    row.iter()
                        .zip(frame)
                        .map(|(weight, sample)| weight * sample)
                        .sum(),
                );
            }
        }
    }
}

/// The input channels that go into one output channel, each with its weight.
type Taps<'a> = &'a [(usize, f32)];

// Positions in a 5.1 frame and in a quad frame.
const FL: usize = 0;
const FR: usize = 1;
const FC: usize = 2;
const LFE: usize = 3;
const BL: usize = 4;
const BR: usize = 5;

/// How to mix a source with `source_channels` channels, or `None` when the audio should be left
/// as it is: the plan asks for nothing, or for as many channels as the source has or more
/// (nothing is upmixed), or the source is not a layout that is mixed (files with more than six
/// channels, and layouts such as three or five channels).
#[must_use]
pub fn matrix_for(source_channels: usize, plan: MixPlan) -> Option<Matrix> {
    let k = MINUS_3_DB;
    let style = plan.style;
    match (source_channels, plan.channels) {
        // 5.1: front left, front right, centre, LFE, back left, back right.
        (6, Channels::Quad) => {
            let (fl, fr): (Taps<'_>, Taps<'_>) = match style {
                Downmix::Standard => (&[(FL, 1.0), (FC, k)], &[(FR, 1.0), (FC, k)]),
                Downmix::WithBass => (
                    &[(FL, 1.0), (FC, k), (LFE, k)],
                    &[(FR, 1.0), (FC, k), (LFE, k)],
                ),
                Downmix::FrontOnly => (&[(FL, 1.0)], &[(FR, 1.0)]),
            };
            Some(Matrix::new(6, 4, &[fl, fr, &[(BL, 1.0)], &[(BR, 1.0)]]))
        }
        (6, Channels::Stereo) => Some(match style {
            Downmix::Standard => Matrix::new(
                6,
                2,
                &[
                    &[(FL, 1.0), (FC, k), (BL, k)],
                    &[(FR, 1.0), (FC, k), (BR, k)],
                ],
            ),
            Downmix::WithBass => Matrix::new(
                6,
                2,
                &[
                    &[(FL, 1.0), (FC, k), (BL, k), (LFE, k)],
                    &[(FR, 1.0), (FC, k), (BR, k), (LFE, k)],
                ],
            ),
            Downmix::FrontOnly => Matrix::new(6, 2, &[&[(FL, 1.0)], &[(FR, 1.0)]]),
        }),
        // Mono is the average of the stereo mix of the same style.
        (6, Channels::Mono) => {
            let stereo = matrix_for(
                6,
                MixPlan {
                    channels: Channels::Stereo,
                    style,
                },
            )?;
            Some(average_of_two(&stereo))
        }
        // Quad: front left, front right, back left, back right. There is no centre or LFE.
        (4, Channels::Stereo) => Some(match style {
            Downmix::FrontOnly => Matrix::new(4, 2, &[&[(0, 1.0)], &[(1, 1.0)]]),
            Downmix::Standard | Downmix::WithBass => {
                Matrix::new(4, 2, &[&[(0, 1.0), (2, k)], &[(1, 1.0), (3, k)]])
            }
        }),
        (4, Channels::Mono) => {
            let stereo = matrix_for(
                4,
                MixPlan {
                    channels: Channels::Stereo,
                    style,
                },
            )?;
            Some(average_of_two(&stereo))
        }
        (2, Channels::Mono) => Some(Matrix::new(2, 1, &[&[(0, 0.5), (1, 0.5)]])),
        _ => None,
    }
}

/// A mono matrix that is the average of the two rows of a stereo one.
fn average_of_two(stereo: &Matrix) -> Matrix {
    let rows = (0..stereo.inputs)
        .map(|input| f32::midpoint(stereo.weight(0, input), stereo.weight(1, input)))
        .collect();
    Matrix {
        inputs: stereo.inputs,
        outputs: 1,
        rows,
    }
}

/// How many channels come out of mixing a source with `source_channels`, which is the source's
/// own count when the audio is left as it is.
#[must_use]
pub fn channels_after(source_channels: usize, plan: MixPlan) -> usize {
    matrix_for(source_channels, plan).map_or(source_channels, |matrix| matrix.outputs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(channels: Channels, style: Downmix) -> MixPlan {
        MixPlan { channels, style }
    }

    /// One frame of 5.1 with a single channel playing at full scale.
    fn only(channel: usize) -> Vec<f32> {
        let mut frame = vec![0.0; 6];
        frame[channel] = 1.0;
        frame
    }

    fn mixed(matrix: &Matrix, frame: &[f32]) -> Vec<f32> {
        let mut out = Vec::new();
        matrix.apply(frame, &mut out);
        out
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn the_standard_stereo_mix_folds_centre_and_surrounds_in_at_minus_3_db_and_drops_the_lfe() {
        let m = matrix_for(6, plan(Channels::Stereo, Downmix::Standard)).unwrap();
        assert_eq!((m.inputs(), m.outputs()), (6, 2));
        let (left, right) = (0, 1);
        assert_eq!(mixed(&m, &only(FL)), [1.0, 0.0]);
        assert_eq!(mixed(&m, &only(FR)), [0.0, 1.0]);
        // The centre goes to both sides, equally, at -3 dB.
        let c = mixed(&m, &only(FC));
        assert!(close(c[left], MINUS_3_DB) && close(c[right], MINUS_3_DB));
        // A back channel goes to its own side only.
        let bl = mixed(&m, &only(BL));
        assert!(close(bl[left], MINUS_3_DB) && bl[right] == 0.0);
        let br = mixed(&m, &only(BR));
        assert!(br[left] == 0.0 && close(br[right], MINUS_3_DB));
        // The LFE is not heard at all.
        assert_eq!(mixed(&m, &only(LFE)), [0.0, 0.0]);
        // -3 dB really is 0.7071 (a power of one half).
        assert!(close(MINUS_3_DB * MINUS_3_DB, 0.5));
    }

    #[test]
    fn standard_plus_bass_adds_the_lfe_at_minus_3_db_to_both_sides_and_changes_nothing_else() {
        let standard = matrix_for(6, plan(Channels::Stereo, Downmix::Standard)).unwrap();
        let bass = matrix_for(6, plan(Channels::Stereo, Downmix::WithBass)).unwrap();
        let lfe = mixed(&bass, &only(LFE));
        assert!(close(lfe[0], MINUS_3_DB) && close(lfe[1], MINUS_3_DB));
        for channel in [FL, FR, FC, BL, BR] {
            assert_eq!(
                mixed(&standard, &only(channel)),
                mixed(&bass, &only(channel))
            );
        }
    }

    #[test]
    fn the_front_pair_style_uses_only_left_and_right_and_never_a_centre_boost() {
        let m = matrix_for(6, plan(Channels::Stereo, Downmix::FrontOnly)).unwrap();
        assert_eq!(mixed(&m, &only(FL)), [1.0, 0.0]);
        assert_eq!(mixed(&m, &only(FR)), [0.0, 1.0]);
        for channel in [FC, LFE, BL, BR] {
            assert_eq!(
                mixed(&m, &only(channel)),
                [0.0, 0.0],
                "channel {channel} is not used"
            );
        }
    }

    #[test]
    fn quad_keeps_the_four_corners_and_folds_the_centre_into_the_fronts() {
        let m = matrix_for(6, plan(Channels::Quad, Downmix::Standard)).unwrap();
        assert_eq!((m.inputs(), m.outputs()), (6, 4));
        assert_eq!(mixed(&m, &only(FL)), [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(mixed(&m, &only(BR)), [0.0, 0.0, 0.0, 1.0]);
        let c = mixed(&m, &only(FC));
        assert!(close(c[0], MINUS_3_DB) && close(c[1], MINUS_3_DB) && c[2] == 0.0 && c[3] == 0.0);
        assert_eq!(mixed(&m, &only(LFE)), [0.0; 4]);

        let bass = matrix_for(6, plan(Channels::Quad, Downmix::WithBass)).unwrap();
        let lfe = mixed(&bass, &only(LFE));
        assert!(close(lfe[0], MINUS_3_DB) && close(lfe[1], MINUS_3_DB) && lfe[2] == 0.0);

        let corners = matrix_for(6, plan(Channels::Quad, Downmix::FrontOnly)).unwrap();
        assert_eq!(mixed(&corners, &only(FC)), [0.0; 4]);
        assert_eq!(mixed(&corners, &only(BL)), [0.0, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn mono_is_the_average_of_the_stereo_mix_of_the_same_style() {
        for style in Downmix::ALL {
            let stereo = matrix_for(6, plan(Channels::Stereo, style)).unwrap();
            let mono = matrix_for(6, plan(Channels::Mono, style)).unwrap();
            assert_eq!((mono.inputs(), mono.outputs()), (6, 1));
            for channel in 0..6 {
                let s = mixed(&stereo, &only(channel));
                assert!(
                    close(mixed(&mono, &only(channel))[0], f32::midpoint(s[0], s[1])),
                    "{style:?} {channel}"
                );
            }
        }
        // Stereo to mono is the plain average.
        let m = matrix_for(2, plan(Channels::Mono, Downmix::Standard)).unwrap();
        assert_eq!(mixed(&m, &[1.0, 0.5]), [0.75]);
        assert_eq!(mixed(&m, &[0.5, 0.5]), [0.5]);
    }

    #[test]
    fn quad_sources_fold_the_backs_into_the_fronts_at_minus_3_db() {
        let m = matrix_for(4, plan(Channels::Stereo, Downmix::Standard)).unwrap();
        let back_left = mixed(&m, &[0.0, 0.0, 1.0, 0.0]);
        assert!(close(back_left[0], MINUS_3_DB) && back_left[1] == 0.0);
        let front_only = matrix_for(4, plan(Channels::Stereo, Downmix::FrontOnly)).unwrap();
        assert_eq!(mixed(&front_only, &[0.3, 0.4, 0.9, 0.9]), [0.3, 0.4]);
        assert!(matrix_for(4, plan(Channels::Mono, Downmix::Standard)).is_some());
        assert!(matrix_for(4, plan(Channels::Quad, Downmix::Standard)).is_none());
    }

    #[test]
    fn nothing_is_upmixed_and_layouts_that_are_not_mixed_are_left_alone() {
        // Asking for as many channels as there are, or more, leaves the audio as it is.
        for source in [1, 2, 4, 6] {
            assert!(matrix_for(source, plan(Channels::Original, Downmix::Standard)).is_none());
        }
        assert!(matrix_for(2, plan(Channels::Stereo, Downmix::Standard)).is_none());
        assert!(matrix_for(2, plan(Channels::Quad, Downmix::Standard)).is_none());
        assert!(matrix_for(1, plan(Channels::Stereo, Downmix::Standard)).is_none());
        assert!(matrix_for(1, plan(Channels::Mono, Downmix::Standard)).is_none());
        assert!(matrix_for(6, plan(Channels::Original, Downmix::WithBass)).is_none());
        // 7.1 and bigger, and layouts that are not 1, 2, 4 or 6 channels, are never mixed.
        for source in [3, 5, 7, 8, 10, 12] {
            for channels in [Channels::Quad, Channels::Stereo, Channels::Mono] {
                assert!(
                    matrix_for(source, plan(channels, Downmix::Standard)).is_none(),
                    "{source}"
                );
            }
        }
    }

    #[test]
    fn the_channel_count_after_mixing_and_the_key_for_saved_measurements() {
        assert_eq!(
            channels_after(6, plan(Channels::Stereo, Downmix::Standard)),
            2
        );
        assert_eq!(
            channels_after(6, plan(Channels::Quad, Downmix::Standard)),
            4
        );
        assert_eq!(
            channels_after(6, plan(Channels::Mono, Downmix::FrontOnly)),
            1
        );
        assert_eq!(
            channels_after(2, plan(Channels::Quad, Downmix::Standard)),
            2
        );
        assert_eq!(
            channels_after(8, plan(Channels::Stereo, Downmix::Standard)),
            8
        );
        // Leaving the audio alone has an empty key, so older saved measurements still count.
        assert_eq!(plan(Channels::Original, Downmix::WithBass).key(), "");
        assert_eq!(
            plan(Channels::Stereo, Downmix::Standard).key(),
            "stereo-standard"
        );
        assert_eq!(plan(Channels::Mono, Downmix::WithBass).key(), "mono-bass");
    }

    #[test]
    fn mixing_a_block_of_frames_and_the_names_of_the_choices() {
        let m = matrix_for(6, plan(Channels::Stereo, Downmix::Standard)).unwrap();
        let two_frames = [only(FL), only(FC)].concat();
        let mut out = vec![9.0; 3];
        m.apply(&two_frames, &mut out);
        assert_eq!(out.len(), 4, "two frames of two channels");
        assert_eq!(&out[..2], &[1.0, 0.0]);
        assert!(close(out[2], MINUS_3_DB) && close(out[3], MINUS_3_DB));

        for channels in Channels::ALL {
            assert_eq!(Channels::from_name(channels.name()), Some(channels));
        }
        for style in Downmix::ALL {
            assert_eq!(Downmix::from_name(style.name()), Some(style));
        }
        assert_eq!(Channels::from_name(" STEREO "), Some(Channels::Stereo));
        assert_eq!(Channels::from_name("surround"), None);
        assert_eq!(
            Downmix::from_name("boost"),
            None,
            "there is no centre boost"
        );
    }
}
