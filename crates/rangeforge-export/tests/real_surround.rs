// Helpers outside `#[test]` functions may unwrap: a failure here is a failed test.
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Checks the channel mix (D-35) on real 5.1 music kept in `testdata/` (not in git): Pink
//! Floyd's *Dark Side of the Moon* as 24-bit/96 kHz FLAC, and Alan Parsons Project's *I Robot*
//! as six-channel DSD64.
//!
//! These tests read real audio, so they are skipped by default and on CI. Run them with:
//!
//! ```text
//! cargo test -p rangeforge-export --release --test real_surround -- --ignored
//! ```
//!
//! What they prove, on real files and not on generated tones:
//! - the channel order the program assumes (front left, front right, centre, LFE, back left,
//!   back right) is the one in the files, because the mixed samples equal FFmpeg's own mix;
//! - the program's measurement of the mixed sound agrees with FFmpeg's `ebur128` filter;
//! - a leveled, mixed export lands on the loudness it was planned to, under the peak ceiling.
//!
//! One thing about the real music: the standard stereo mix of *Speak To Me* goes over full scale
//! (about +0.8 dBTP) where loud channels add up. FFmpeg's `pan` clips such a mix at 1.0, so it
//! is compared here with the input at half level, where nothing can clip. The program keeps the
//! values above 1.0 until the gain comes down, which is the point of measuring the mix.

use std::path::{Path, PathBuf};

use rangeforge_audio::AudioError;
use rangeforge_audio::decode::for_each_block_mixed;
use rangeforge_audio::dsd::ANALYSIS_RATE;
use rangeforge_audio::measure::measure_whole_file;
use rangeforge_core::mix::{Channels, Downmix, MixPlan};
use rangeforge_core::planner::{Target, plan_gain};
use rangeforge_export::ffmpeg::Ffmpeg;
use rangeforge_export::format::{BitDepth, Codec, OutputFormat};
use rangeforge_export::pipeline::{Options, TrackPlan, export_file};
use rangeforge_export::strategy::Strategy;
use rangeforge_export::tags::Tags;
use tempfile::tempdir;

/// -3 dB, written the way FFmpeg's `pan` filter is given it, and half of it.
const K: &str = "0.7071067811865476";
const HALF_K: &str = "0.3535533905932738";

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// The shortest of the Floyd tracks: 70 s of 24-bit/96 kHz in six channels.
fn floyd() -> PathBuf {
    let path = testdata()
        .join("Pink Floyd")
        .join("Dark Side Of The Moon")
        .join("01 - Speak To Me.flac");
    assert!(path.is_file(), "missing {}", path.display());
    path
}

fn i_robot() -> PathBuf {
    let path = testdata()
        .join("Alan Parsons Project - I Robot")
        .join("1977 - I Robot")
        .join("6ch")
        .join("01 - I Robot.dsf");
    assert!(path.is_file(), "missing {}", path.display());
    path
}

fn ffmpeg() -> Ffmpeg {
    let ffmpeg = Ffmpeg::locate().expect("FFmpeg is needed for these tests");
    ffmpeg.offer_to_audio();
    ffmpeg
}

fn plan(channels: Channels, style: Downmix) -> MixPlan {
    MixPlan { channels, style }
}

/// Every sample of `path` as the program decodes it with `mix` applied, and how many channels.
fn ours(path: &Path, mix: MixPlan) -> (usize, Vec<f32>) {
    let mut samples = Vec::new();
    let info = for_each_block_mixed::<AudioError, _>(path, ANALYSIS_RATE, mix, |_, block| {
        samples.extend_from_slice(block);
        Ok(())
    })
    .unwrap();
    (info.channels, samples)
}

/// Every sample of `path` after FFmpeg has run `filter` on it, as 32-bit floats, with the input
/// at half level so that a mix that adds up over full scale is not clipped.
fn ffmpegs_halved(ffmpeg: &Ffmpeg, path: &Path, filter: &str) -> Vec<f32> {
    let out = ffmpeg
        .command()
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(path)
        .args([
            "-vn",
            "-af",
            &format!("volume=0.5,{filter},aformat=sample_fmts=flt"),
        ])
        .args(["-f", "f32le", "-"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let (samples, _) = out.stdout.as_chunks::<4>();
    samples.iter().map(|b| f32::from_le_bytes(*b)).collect()
}

/// FFmpeg's `ebur128` reading of `path` after `filter`: integrated loudness and true peak.
fn ffmpegs_loudness(ffmpeg: &Ffmpeg, path: &Path, filter: &str) -> (f64, f64) {
    let out = ffmpeg
        .command()
        .args(["-hide_banner", "-nostats", "-nostdin", "-i"])
        .arg(path)
        .args([
            "-vn",
            "-af",
            &format!("{filter},ebur128=peak=true"),
            "-f",
            "null",
            "-",
        ])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stderr).replace('\n', " ");
    let number_after = |label: &str, key: &str| -> f64 {
        let from = text
            .rfind(label)
            .unwrap_or_else(|| panic!("no {label} in {text}"));
        let rest = &text[from + label.len()..];
        let at = rest.find(key).unwrap() + key.len();
        rest[at..]
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    (
        number_after("Integrated loudness:", "I:"),
        number_after("True peak:", "Peak:"),
    )
}

/// The same samples at half level (exact: a power of two).
fn halved(samples: &[f32]) -> Vec<f32> {
    samples.iter().map(|s| s * 0.5).collect()
}

fn biggest_difference(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(
        a.len(),
        b.len(),
        "the two have a different number of samples"
    );
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

fn pan_stereo(extra_left: &str, extra_right: &str) -> String {
    format!("pan=stereo|FL=FL+{K}*FC+{K}*BL{extra_left}|FR=FR+{K}*FC+{K}*BR{extra_right}")
}

#[test]
#[ignore = "reads real audio from testdata/, which is not in git"]
fn the_stereo_mix_of_a_real_5_1_flac_is_what_ffmpeg_makes_of_it() {
    let ffmpeg = ffmpeg();
    let path = floyd();

    // Standard: the channel order in the real file is the one the mix assumes.
    let (channels, mixed) = ours(&path, plan(Channels::Stereo, Downmix::Standard));
    assert_eq!(channels, 2);
    let reference = ffmpegs_halved(&ffmpeg, &path, &pan_stereo("", ""));
    let difference = biggest_difference(&halved(&mixed), &reference);
    assert!(
        difference < 1e-5,
        "differs from FFmpeg's mix by {difference}"
    );

    // Standard + bass: the LFE at -3 dB on both sides.
    let (_, with_bass) = ours(&path, plan(Channels::Stereo, Downmix::WithBass));
    let reference = ffmpegs_halved(
        &ffmpeg,
        &path,
        &pan_stereo(&format!("+{K}*LFE"), &format!("+{K}*LFE")),
    );
    assert!(
        biggest_difference(&halved(&with_bass), &reference) < 1e-5,
        "with bass"
    );
    assert!(
        biggest_difference(&with_bass, &mixed) > 1e-3,
        "the bass is really in it: the LFE track of this recording is not silent"
    );

    // Front pair only.
    let (_, front) = ours(&path, plan(Channels::Stereo, Downmix::FrontOnly));
    let reference = ffmpegs_halved(&ffmpeg, &path, "pan=stereo|FL=FL|FR=FR");
    assert!(
        biggest_difference(&halved(&front), &reference) < 1e-5,
        "front pair only"
    );

    // Quad: the centre into the fronts, the backs as they are.
    let (channels, quad) = ours(&path, plan(Channels::Quad, Downmix::Standard));
    assert_eq!(channels, 4);
    let reference = ffmpegs_halved(
        &ffmpeg,
        &path,
        &format!("pan=quad|FL=FL+{K}*FC|FR=FR+{K}*FC|BL=BL|BR=BR"),
    );
    assert!(
        biggest_difference(&halved(&quad), &reference) < 1e-5,
        "quad"
    );

    // Mono: the average of the stereo mix.
    let (channels, mono) = ours(&path, plan(Channels::Mono, Downmix::Standard));
    assert_eq!(channels, 1);
    let reference = ffmpegs_halved(
        &ffmpeg,
        &path,
        &format!("pan=mono|c0=0.5*FL+0.5*FR+{K}*FC+{HALF_K}*BL+{HALF_K}*BR"),
    );
    // (Half of left plus half of right: the centre counts once in each, so 2 * 0.5 * K, and each
    // back channel only in its own side, so 0.5 * K.)
    assert!(
        biggest_difference(&halved(&mono), &reference) < 1e-5,
        "mono"
    );

    // And the mix is not clipped on the way: this recording's standard stereo mix really does go
    // over full scale, and the program keeps those values so the gain can bring them down.
    let loudest = mixed.iter().fold(0.0_f32, |peak, s| peak.max(s.abs()));
    assert!(
        loudest > 1.0,
        "the mix peaks at {loudest}, it was expected to pass full scale"
    );
}

#[test]
#[ignore = "reads real audio from testdata/, which is not in git"]
fn the_measurement_of_the_mixed_sound_agrees_with_ffmpegs_ebur128() {
    let ffmpeg = ffmpeg();
    let path = floyd();
    let measured = measure_whole_file(&path, plan(Channels::Stereo, Downmix::Standard)).unwrap();
    assert_eq!(measured.channels, 2);
    let ours = measured.tracks[0];
    // FFmpeg measures the half-level mix (so nothing is clipped), which is 6.0206 dB down in
    // both loudness and peak.
    let filter = format!("volume=0.5,{}", pan_stereo("", ""));
    let (lufs, peak) = ffmpegs_loudness(&ffmpeg, &path, &filter);
    let half = 20.0 * 2.0_f64.log10();
    assert!(
        (ours.integrated_lufs - half - lufs).abs() < 0.1,
        "loudness: ours {} (less {half}) against FFmpeg's {lufs}",
        ours.integrated_lufs
    );
    // The two meters oversample for the true peak a little differently.
    assert!(
        (ours.true_peak_dbtp - half - peak).abs() < 0.2,
        "true peak: ours {} (less {half}) against FFmpeg's {peak}",
        ours.true_peak_dbtp
    );
    // This mix goes over full scale: the peak is above 0 dBTP, and the gain will bring it down.
    assert!(ours.true_peak_dbtp > 0.0, "{ours:?}");

    // And the 6-channel measurement is another number: the mix really has to be measured.
    let six = measure_whole_file(&path, MixPlan::default())
        .unwrap()
        .tracks[0];
    assert!(
        (six.integrated_lufs - ours.integrated_lufs).abs() > 0.2
            || (six.true_peak_dbtp - ours.true_peak_dbtp).abs() > 0.5,
        "six channels {six:?}, stereo mix {ours:?}"
    );
}

/// Levels `input` as a stereo mix to -22 LUFS under a -0.6 dBTP ceiling, writes it as 24-bit
/// FLAC, and checks the written file lands where it was planned to.
fn leveled_and_written_as_stereo(input: &Path) {
    let ffmpeg = ffmpeg();
    let mix = plan(Channels::Stereo, Downmix::Standard);
    let target = Target {
        loudness_lufs: -22.0,
        peak_ceiling_dbtp: Some(-0.6),
    };
    let measured = measure_whole_file(input, mix).unwrap().tracks[0];
    let planned = plan_gain(measured, target).unwrap();

    let out = tempdir().unwrap();
    let options = Options {
        format: OutputFormat::new(Codec::Flac, BitDepth::TwentyFour),
        strategy: Strategy::safe(),
        directory: out.path().to_path_buf(),
        copy_source_tags: false,
        mix,
    };
    let track = TrackPlan {
        gain_db: planned.gain_db,
        file_name: "stereo.flac".to_string(),
        tags: Tags::default(),
        skip: false,
    };
    let written = export_file(&ffmpeg, input, None, &[track], &options, &mut |_| {}).unwrap();

    let after = measure_whole_file(&written[0].path, MixPlan::default()).unwrap();
    assert_eq!(after.channels, 2, "written as stereo");
    let after = after.tracks[0];
    assert!(
        (after.integrated_lufs - planned.resulting_lufs).abs() < 0.05,
        "{} LUFS written, {} planned (from {measured:?}, gain {})",
        after.integrated_lufs,
        planned.resulting_lufs,
        planned.gain_db
    );
    assert!(
        after.true_peak_dbtp <= -0.6 + 0.1,
        "the peak is over the ceiling: {} dBTP",
        after.true_peak_dbtp
    );
    // The target is reached unless the ceiling held the song back.
    if !planned.limited_by_peak {
        assert!((after.integrated_lufs - -22.0).abs() < 0.05, "{after:?}");
    }
}

#[test]
#[ignore = "reads real audio from testdata/, which is not in git"]
fn a_real_5_1_flac_is_leveled_after_the_mix_and_lands_where_planned() {
    leveled_and_written_as_stereo(&floyd());
}

#[test]
#[ignore = "reads real audio from testdata/, which is not in git"]
fn a_real_six_channel_dsd_rip_is_leveled_after_the_mix_and_lands_where_planned() {
    leveled_and_written_as_stereo(&i_robot());
}
