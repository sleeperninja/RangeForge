// Helpers outside `#[test]` functions may unwrap: a failure here is a failed test. The small
// casts are on generated test data.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]
//! Mixing channels while decoding and measuring, on small generated files (multichannel WAV).
//!
//! The real-music checks, on 5.1 FLAC and DSD files, are in the export crate's tests.

use std::fs;
use std::path::Path;

use rangeforge_audio::AudioError;
use rangeforge_audio::cache::{Cache, Source};
use rangeforge_audio::decode::{StreamInfo, for_each_block, for_each_block_mixed};
use rangeforge_audio::dsd;
use rangeforge_audio::measure::measure_whole_file;
use rangeforge_core::mix::{Channels, Downmix, MINUS_3_DB, MixPlan};
use tempfile::tempdir;

const RATE: u32 = 48_000;

fn plan(channels: Channels) -> MixPlan {
    MixPlan {
        channels,
        style: Downmix::Standard,
    }
}

/// Writes a 16-bit WAVE file with an explicit channel mask (needed for more than two channels).
/// `sample(channel, frame)` gives each sample, from -1.0 to 1.0.
fn write_wav(
    path: &Path,
    channels: u16,
    mask: u32,
    frames: usize,
    sample: impl Fn(usize, usize) -> f32,
) {
    let mut data = Vec::with_capacity(frames * usize::from(channels) * 2);
    for frame in 0..frames {
        for channel in 0..usize::from(channels) {
            #[allow(clippy::cast_possible_truncation)]
            let value = (sample(channel, frame).clamp(-1.0, 1.0) * 32767.0).round() as i16;
            data.extend_from_slice(&value.to_le_bytes());
        }
    }
    let block_align = channels * 2;
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&0xFFFE_u16.to_le_bytes()); // WAVE_FORMAT_EXTENSIBLE
    fmt.extend_from_slice(&channels.to_le_bytes());
    fmt.extend_from_slice(&RATE.to_le_bytes());
    fmt.extend_from_slice(&(RATE * u32::from(block_align)).to_le_bytes());
    fmt.extend_from_slice(&block_align.to_le_bytes());
    fmt.extend_from_slice(&16_u16.to_le_bytes());
    fmt.extend_from_slice(&22_u16.to_le_bytes()); // size of the extension
    fmt.extend_from_slice(&16_u16.to_le_bytes()); // valid bits
    fmt.extend_from_slice(&mask.to_le_bytes());
    // The PCM sub-format.
    fmt.extend_from_slice(&[
        1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71,
    ]);
    let mut file = Vec::new();
    file.extend_from_slice(b"RIFF");
    file.extend_from_slice(&(4 + 8 + fmt.len() as u32 + 8 + data.len() as u32).to_le_bytes());
    file.extend_from_slice(b"WAVEfmt ");
    file.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    file.extend_from_slice(&fmt);
    file.extend_from_slice(b"data");
    file.extend_from_slice(&(data.len() as u32).to_le_bytes());
    file.extend_from_slice(&data);
    fs::write(path, file).unwrap();
}

const MASK_51: u32 = 0x3F; // front left and right, centre, LFE, back left and right
const MASK_QUAD: u32 = 0x33; // front left and right, back left and right

fn tone(frame: usize, hertz: f32, amplitude: f32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let t = frame as f32 / RATE as f32;
    amplitude * (std::f32::consts::TAU * hertz * t).sin()
}

/// Every block of the file, mixed as asked, joined into one list with the info it came with.
fn read(path: &Path, mix: MixPlan) -> (StreamInfo, Vec<f32>) {
    let mut all = Vec::new();
    let mut seen = None;
    let info =
        for_each_block_mixed::<AudioError, _>(path, dsd::ANALYSIS_RATE, mix, |info, samples| {
            seen = Some(*info);
            all.extend_from_slice(samples);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        seen,
        Some(info),
        "the blocks and the result agree on the channels"
    );
    (info, all)
}

#[test]
fn a_5_1_file_comes_out_as_stereo_mixed_by_the_standard_rules() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("surround.wav");
    // One sound per channel, so each channel's part in the mix can be checked.
    let hertz = [300.0, 400.0, 500.0, 60.0, 700.0, 800.0];
    write_wav(&path, 6, MASK_51, 4800, |channel, frame| {
        tone(frame, hertz[channel], 0.4)
    });

    let (source_info, source) = read(&path, MixPlan::default());
    assert_eq!(source_info.channels, 6);
    let (info, mixed) = read(&path, plan(Channels::Stereo));
    assert_eq!((info.channels, info.sample_rate), (2, RATE));
    assert_eq!(mixed.len(), 4800 * 2);

    // Left is front left + centre and back left at -3 dB; right the same on its side; the LFE
    // (channel 3) is not in either.
    for frame in 0..4800 {
        let s = |channel: usize| source[frame * 6 + channel];
        let left = s(0) + MINUS_3_DB * s(2) + MINUS_3_DB * s(4);
        let right = s(1) + MINUS_3_DB * s(2) + MINUS_3_DB * s(5);
        assert!(
            (mixed[frame * 2] - left).abs() < 1e-6,
            "left, frame {frame}"
        );
        assert!(
            (mixed[frame * 2 + 1] - right).abs() < 1e-6,
            "right, frame {frame}"
        );
    }
}

#[test]
fn audio_the_plan_leaves_alone_is_handed_over_unchanged() {
    let dir = tempdir().unwrap();
    let stereo = dir.path().join("stereo.wav");
    write_wav(&stereo, 2, 0x3, 2400, |channel, frame| {
        tone(frame, 300.0 + 100.0 * channel as f32, 0.3)
    });
    let surround = dir.path().join("surround.wav");
    write_wav(&surround, 6, MASK_51, 2400, |channel, frame| {
        tone(frame, 200.0 + 50.0 * channel as f32, 0.2)
    });

    let mut plain = Vec::new();
    let info = for_each_block::<AudioError, _>(&stereo, |_, samples| {
        plain.extend_from_slice(samples);
        Ok(())
    })
    .unwrap();
    // Asking for stereo from stereo, quad from stereo, or nothing at all: untouched.
    for channels in [Channels::Original, Channels::Stereo, Channels::Quad] {
        let (mixed_info, mixed) = read(&stereo, plan(channels));
        assert_eq!(mixed_info, info);
        assert_eq!(mixed, plain, "{channels:?}");
    }
    // Asking a 5.1 file for its own six channels, as the original: untouched too.
    let (info, same) = read(&surround, plan(Channels::Original));
    assert_eq!(info.channels, 6);
    assert_eq!(same.len(), 2400 * 6);
}

#[test]
fn quad_comes_out_with_the_centre_folded_into_the_fronts() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("surround.wav");
    // Only the centre plays.
    write_wav(&path, 6, MASK_51, 2400, |channel, frame| {
        if channel == 2 {
            tone(frame, 440.0, 0.5)
        } else {
            0.0
        }
    });
    let (info, quad) = read(&path, plan(Channels::Quad));
    assert_eq!(info.channels, 4);
    for frame in 0..2400 {
        let centre = tone(frame, 440.0, 0.5);
        let at = |channel: usize| quad[frame * 4 + channel];
        assert!((at(0) - MINUS_3_DB * centre).abs() < 1e-3, "front left");
        assert!((at(1) - MINUS_3_DB * centre).abs() < 1e-3, "front right");
        assert!(
            at(2).abs() < 1e-6 && at(3).abs() < 1e-6,
            "back channels stay silent"
        );
    }

    // A quad file goes to stereo with the backs folded in, and not to quad again.
    let quad_file = dir.path().join("quad.wav");
    write_wav(&quad_file, 4, MASK_QUAD, 2400, |channel, frame| {
        if channel == 2 {
            tone(frame, 440.0, 0.5)
        } else {
            0.0
        }
    });
    let (info, stereo) = read(&quad_file, plan(Channels::Stereo));
    assert_eq!(info.channels, 2);
    assert!((stereo[200 * 2] - MINUS_3_DB * tone(200, 440.0, 0.5)).abs() < 1e-3);
    assert_eq!(read(&quad_file, plan(Channels::Quad)).0.channels, 4);
}

#[test]
fn the_measurement_is_of_the_mixed_sound() {
    let dir = tempdir().unwrap();

    // Front channels only: the stereo mix is exactly that stereo sound, so the numbers match a
    // genuine stereo file of the same sound.
    let front = dir.path().join("front.wav");
    write_wav(
        &front,
        6,
        MASK_51,
        48_000 * 4,
        |channel, frame| match channel {
            0 => tone(frame, 330.0, 0.3),
            1 => tone(frame, 440.0, 0.3),
            _ => 0.0,
        },
    );
    let reference = dir.path().join("reference.wav");
    write_wav(&reference, 2, 0x3, 48_000 * 4, |channel, frame| {
        tone(frame, if channel == 0 { 330.0 } else { 440.0 }, 0.3)
    });
    let mixed = measure_whole_file(&front, plan(Channels::Stereo)).unwrap();
    let genuine = measure_whole_file(&reference, MixPlan::default()).unwrap();
    assert_eq!(
        mixed.channels, 2,
        "the measured channels are the mixed ones"
    );
    let (m, g) = (mixed.tracks[0], genuine.tracks[0]);
    assert!(
        (m.integrated_lufs - g.integrated_lufs).abs() < 1e-6,
        "{m:?} {g:?}"
    );
    assert!((m.true_peak_dbtp - g.true_peak_dbtp).abs() < 1e-6);

    // Centre only: the 6-channel meter counts the centre at full weight, and the stereo mix
    // puts it in both sides at -3 dB, which is the same total power, so the loudness is the same.
    let centre = dir.path().join("centre.wav");
    write_wav(&centre, 6, MASK_51, 48_000 * 4, |channel, frame| {
        if channel == 2 {
            tone(frame, 500.0, 0.4)
        } else {
            0.0
        }
    });
    let six = measure_whole_file(&centre, MixPlan::default()).unwrap();
    let two = measure_whole_file(&centre, plan(Channels::Stereo)).unwrap();
    assert_eq!(six.channels, 6);
    assert!(
        (six.tracks[0].integrated_lufs - two.tracks[0].integrated_lufs).abs() < 0.05,
        "{:?} {:?}",
        six.tracks[0],
        two.tracks[0]
    );
    // But the peak of the one centre channel is the sample peak of that channel: in the mix it is
    // 3 dB lower on each side.
    assert!((six.tracks[0].true_peak_dbtp - two.tracks[0].true_peak_dbtp - 3.0).abs() < 0.2);
}

#[test]
fn the_cache_keeps_each_way_of_mixing_apart() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("surround.wav");
    write_wav(&path, 6, MASK_51, 48_000 * 2, |channel, frame| {
        tone(frame, 200.0 + 100.0 * channel as f32, 0.2)
    });
    let cache = Cache::new(dir.path().join("cache"));
    let measure = |mix: MixPlan| cache.measure_whole_file(&path, mix).unwrap();
    let decoded = |source: &Source| matches!(source, Source::Decoded { .. });

    let (six, source) = measure(MixPlan::default());
    assert!(decoded(&source));
    let (stereo, source) = measure(plan(Channels::Stereo));
    assert!(
        decoded(&source),
        "a different mix is a different measurement"
    );
    let (mono, source) = measure(plan(Channels::Mono));
    assert!(decoded(&source));
    let (bass, source) = measure(MixPlan {
        channels: Channels::Stereo,
        style: Downmix::WithBass,
    });
    assert!(decoded(&source), "and so is a different style");

    // Each is now remembered, separately, and gives back what it gave.
    for (mix, before) in [
        (MixPlan::default(), &six),
        (plan(Channels::Stereo), &stereo),
        (plan(Channels::Mono), &mono),
        (
            MixPlan {
                channels: Channels::Stereo,
                style: Downmix::WithBass,
            },
            &bass,
        ),
    ] {
        let (again, source) = measure(mix);
        assert_eq!(source, Source::Cache, "{mix:?}");
        // Saved numbers come back through text, which can differ in the last digit.
        assert_eq!(
            (again.channels, again.sample_rate, again.total_sample_frames),
            (
                before.channels,
                before.sample_rate,
                before.total_sample_frames
            ),
            "{mix:?}"
        );
        let (a, b) = (again.tracks[0], before.tracks[0]);
        assert!(
            (a.integrated_lufs - b.integrated_lufs).abs() < 1e-9,
            "{mix:?}"
        );
        assert!(
            (a.true_peak_dbtp - b.true_peak_dbtp).abs() < 1e-9,
            "{mix:?}"
        );
    }
    assert_eq!((six.channels, stereo.channels, mono.channels), (6, 2, 1));
}
