// Helpers outside `#[test]` functions may unwrap: a failure here is a failed test.
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! End-to-end checks of the export pipeline, using real FFmpeg on generated audio.
//!
//! These tests skip themselves, with a note on standard error, when FFmpeg is not installed.
//! Point `RANGEFORGE_FFMPEG` at it if it is not on the path.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rangeforge_audio::AudioError;
use rangeforge_audio::decode::{StreamInfo, for_each_block};
use rangeforge_audio::measure::measure_whole_file;
use rangeforge_core::cue;
use rangeforge_core::mix::{Channels, Downmix, MINUS_3_DB, MixPlan};
use rangeforge_export::ExportError;
use rangeforge_export::ffmpeg::Ffmpeg;
use rangeforge_export::format::{BitDepth, Bitrate, Codec, Container, OutputFormat, SampleRate};
use rangeforge_export::naming::track_file_name;
use rangeforge_export::pipeline::{Options, Progress, TrackPlan, Written, export_file, preview};
use rangeforge_export::strategy::{Conflict, Destination, Strategy};
use rangeforge_export::tags::Tags;
use tempfile::{TempDir, tempdir};

fn ffmpeg() -> Option<Ffmpeg> {
    match Ffmpeg::locate() {
        Ok(ffmpeg) => Some(ffmpeg),
        Err(error) => {
            // On a build server FFmpeg is installed on purpose, so a missing one is a real
            // failure there and not a reason to quietly skip these tests.
            assert!(
                std::env::var_os("CI").is_none(),
                "FFmpeg is required in CI: {error}"
            );
            eprintln!("skipping: {error}");
            None
        }
    }
}

/// Writes a stereo 16-bit 44.1 kHz WAV of a repeating ramp. `amplitude` scales the ramp.
fn write_wav(path: &Path, seconds: u32, amplitude: i32) {
    let rate: u32 = 44_100;
    let frames = rate * seconds;
    let data_len = frames * 4;
    let mut bytes = Vec::with_capacity(44 + usize::try_from(data_len).unwrap());
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 4).to_le_bytes());
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for frame in 0..frames {
        let ramp = i32::try_from(frame % 200).unwrap() - 100;
        let left = i16::try_from(ramp * amplitude).unwrap();
        let right = i16::try_from((100 - i32::try_from(frame % 200).unwrap()) * amplitude).unwrap();
        bytes.extend_from_slice(&left.to_le_bytes());
        bytes.extend_from_slice(&right.to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}

fn decode_all(path: &Path) -> (StreamInfo, Vec<f32>) {
    let mut samples = Vec::new();
    let info = for_each_block::<AudioError, _>(path, |_, block| {
        samples.extend_from_slice(block);
        Ok(())
    })
    .unwrap();
    (info, samples)
}

fn loudness(path: &Path) -> f64 {
    measure_whole_file(path, MixPlan::default()).unwrap().tracks[0].integrated_lufs
}

fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn plan(gain_db: f64, file_name: &str) -> TrackPlan {
    TrackPlan {
        gain_db,
        file_name: file_name.to_string(),
        tags: Tags::default(),
        skip: false,
    }
}

fn options(codec: Codec, strategy: Strategy, directory: &Path) -> Options {
    Options {
        format: OutputFormat::new(codec, BitDepth::Source),
        strategy,
        directory: directory.to_path_buf(),
        copy_source_tags: false,
        mix: rangeforge_core::mix::MixPlan::default(),
    }
}

/// Next to the original, numbering a new file whose name is taken.
fn serialize() -> Strategy {
    Strategy::safe()
}

fn with_conflict(conflict: Conflict) -> Strategy {
    Strategy {
        conflict,
        ..Strategy::safe()
    }
}

fn overwrite() -> Strategy {
    with_conflict(Conflict::Overwrite)
}

fn rename_existing() -> Strategy {
    with_conflict(Conflict::RenameExisting {
        suffix: "_old".to_string(),
    })
}

/// Exports a whole file as one track, ignoring progress.
fn export_whole(
    ffmpeg: &Ffmpeg,
    input: &Path,
    plan: TrackPlan,
    options: &Options,
) -> Result<Vec<Written>, ExportError> {
    export_file(ffmpeg, input, None, &[plan], options, &mut |_| {})
}

/// A folder holding one generated source file.
fn with_source(name: &str, seconds: u32, amplitude: i32) -> (TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let path = dir.path().join(name);
    write_wav(&path, seconds, amplitude);
    (dir, path)
}

#[test]
fn every_lossless_codec_is_bit_exact_at_zero_gain() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("in.wav", 3, 200);
    let (_, original) = decode_all(&input);

    for codec in [Codec::Alac, Codec::Flac, Codec::Wav] {
        let name = format!("out.{}", codec.extension());
        let opts = options(codec, serialize(), dir.path());
        let written = export_whole(&ffmpeg, &input, plan(0.0, &name), &opts).unwrap();

        let (info, decoded) = decode_all(&written[0].path);
        assert_eq!(info.sample_rate, 44_100, "{codec:?}");
        assert_eq!(info.channels, 2, "{codec:?}");
        assert_eq!(decoded, original, "{codec:?} changed the audio");
    }
}

#[test]
fn a_gain_moves_loudness_and_peak_by_exactly_that_much() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("in.wav", 4, 200);
    let before = measure_whole_file(&input, MixPlan::default())
        .unwrap()
        .tracks[0];

    for gain in [-6.0, -2.5, 3.0] {
        let opts = options(Codec::Alac, serialize(), dir.path());
        let written = export_whole(&ffmpeg, &input, plan(gain, "out.m4a"), &opts).unwrap();
        let after = measure_whole_file(&written[0].path, MixPlan::default())
            .unwrap()
            .tracks[0];

        let loudness_change = after.integrated_lufs - before.integrated_lufs;
        let peak_change = after.true_peak_dbtp - before.true_peak_dbtp;
        assert!(
            (loudness_change - gain).abs() < 0.05,
            "gain {gain}: loudness moved {loudness_change}"
        );
        assert!(
            (peak_change - gain).abs() < 0.1,
            "gain {gain}: peak moved {peak_change}"
        );
    }
}

#[test]
fn a_cue_album_is_cut_exactly_and_tagged() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("in.wav", 3, 200);
    let (_, original) = decode_all(&input);

    let sheet = cue::parse(
        "PERFORMER \"The Band\"\nTITLE \"Test Album\"\nREM GENRE Rock\nREM DATE 2024\nFILE \"in.wav\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"First\"\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    TITLE \"Second/Part\"\n    INDEX 01 00:01:00\n",
    )
    .unwrap();
    let file = &sheet.files[0];
    let segments = file.segments();
    let plans: Vec<TrackPlan> = file
        .tracks
        .iter()
        .map(|track| TrackPlan {
            gain_db: 0.0,
            file_name: track_file_name(track.number, track.title.as_deref(), "m4a"),
            tags: Tags::for_cue_track(&sheet, track, file.tracks.len()),
            skip: false,
        })
        .collect();
    let out_dir = dir.path().join("album");
    let opts = options(Codec::Alac, overwrite(), &out_dir);

    let mut events = Vec::new();
    let written = export_file(&ffmpeg, &input, Some(&segments), &plans, &opts, &mut |p| {
        events.push(p.clone());
    })
    .unwrap();

    // Names are numbered, titled and safe; the folder was created.
    assert_eq!(
        file_names(&out_dir),
        ["01 - First.m4a", "02 - Second_Part.m4a"]
    );
    assert_eq!(written.len(), 2);

    // One second is exactly 75 CUE frames, so the split lands on sample 44,100.
    let split = 44_100 * 2;
    assert_eq!(decode_all(&written[0].path).1, original[..split]);
    assert_eq!(decode_all(&written[1].path).1, original[split..]);

    // Progress was reported for each track, in order.
    assert_eq!(
        events.first(),
        Some(&Progress::Encoding { index: 0, total: 2 })
    );
    assert!(matches!(
        events.last(),
        Some(Progress::Saved { index: 1, .. })
    ));

    // The tags made it into the file.
    let metadata = Command::new(ffmpeg.path())
        .args(["-v", "error", "-i"])
        .arg(&written[1].path)
        .args(["-f", "ffmetadata", "-"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&metadata.stdout);
    for expected in [
        "title=Second/Part",
        "artist=The Band",
        "album=Test Album",
        "track=2/2",
        "genre=Rock",
        "date=2024",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }
}

#[test]
fn a_free_name_is_written_as_it_is() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("song.wav", 2, 200);
    let original_bytes = fs::read(&input).unwrap();

    let opts = options(Codec::Alac, serialize(), dir.path());
    let written = export_whole(&ffmpeg, &input, plan(-3.0, "song.m4a"), &opts).unwrap();

    assert_eq!(written[0].path, dir.path().join("song.m4a"));
    assert_eq!(fs::read(&input).unwrap(), original_bytes);
    assert_eq!(file_names(dir.path()), ["song.m4a", "song.wav"]);
}

#[test]
fn numbering_a_taken_name_leaves_what_is_there_alone() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("song.wav", 2, 200);
    fs::write(dir.path().join("song.m4a"), b"already here").unwrap();

    let opts = options(Codec::Alac, serialize(), dir.path());
    let written = export_whole(&ffmpeg, &input, plan(-3.0, "song.m4a"), &opts).unwrap();

    assert_eq!(written[0].path, dir.path().join("song_00.m4a"));
    assert_eq!(
        fs::read(dir.path().join("song.m4a")).unwrap(),
        b"already here"
    );
    assert_eq!(
        file_names(dir.path()),
        ["song.m4a", "song.wav", "song_00.m4a"]
    );
}

#[test]
fn a_subfolder_of_the_original_is_created_and_used() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("song.wav", 2, 200);

    let strategy = Strategy {
        destination: Destination::Subfolder("Balanced".to_string()),
        ..Strategy::safe()
    };
    let opts = options(Codec::Alac, strategy, dir.path());
    let written = export_whole(&ffmpeg, &input, plan(-3.0, "song.m4a"), &opts).unwrap();

    assert_eq!(
        written[0].path,
        dir.path().join("Balanced").join("song.m4a")
    );
    assert_eq!(file_names(dir.path()), ["Balanced", "song.wav"]);
    assert_eq!(file_names(&dir.path().join("Balanced")), ["song.m4a"]);
}

#[test]
fn a_song_left_out_is_not_written_and_does_not_disturb_the_rest() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("in.wav", 3, 200);
    let sheet = cue::parse(
        "PERFORMER \"The Band\"\nTITLE \"Album\"\nFILE \"in.wav\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"First\"\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    TITLE \"Second\"\n    INDEX 01 00:01:00\n  TRACK 03 AUDIO\n    TITLE \"Third\"\n    INDEX 01 00:02:00\n",
    )
    .unwrap();
    let file = &sheet.files[0];
    let segments = file.segments();
    let plans: Vec<TrackPlan> = ["01 - First.m4a", "02 - Second.m4a", "03 - Third.m4a"]
        .iter()
        .enumerate()
        .map(|(index, name)| TrackPlan {
            skip: index == 1,
            ..plan(0.0, name)
        })
        .collect();
    let opts = options(Codec::Alac, serialize(), dir.path());

    let written =
        export_file(&ffmpeg, &input, Some(&segments), &plans, &opts, &mut |_| {}).unwrap();

    let indexes: Vec<usize> = written.iter().map(|w| w.index).collect();
    assert_eq!(indexes, [0, 2]);
    assert_eq!(
        file_names(dir.path()),
        ["01 - First.m4a", "03 - Third.m4a", "in.wav"]
    );
}

#[test]
fn the_original_is_deleted_only_when_asked_and_everything_was_written() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let strategy = Strategy {
        delete_original: true,
        ..Strategy::safe()
    };

    // Everything written: the original goes.
    let (dir, input) = with_source("song.wav", 1, 200);
    let opts = options(Codec::Alac, strategy.clone(), dir.path());
    let mut events = Vec::new();
    export_file(
        &ffmpeg,
        &input,
        None,
        &[plan(0.0, "song.m4a")],
        &opts,
        &mut |p| events.push(p.clone()),
    )
    .unwrap();
    assert_eq!(file_names(dir.path()), ["song.m4a"]);
    assert!(events.contains(&Progress::RemovedOriginal { path: input }));

    // A song left out: the original stays, because deleting it would lose that song.
    let (dir, input) = with_source("song.wav", 1, 200);
    let opts = options(Codec::Alac, strategy.clone(), dir.path());
    let left_out = TrackPlan {
        skip: true,
        ..plan(0.0, "song.m4a")
    };
    export_whole(&ffmpeg, &input, left_out, &opts).unwrap();
    assert_eq!(file_names(dir.path()), ["song.wav"]);

    // The original is itself the file written (overwrite): it must not be deleted.
    let (dir, input) = with_source("song.wav", 1, 200);
    let both = Strategy {
        conflict: Conflict::Overwrite,
        ..strategy
    };
    let opts = options(Codec::Wav, both, dir.path());
    export_whole(&ffmpeg, &input, plan(-3.0, "song.wav"), &opts).unwrap();
    assert_eq!(file_names(dir.path()), ["song.wav"]);
}

#[test]
fn nothing_is_deleted_when_the_run_fails() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let dir = tempdir().unwrap();
    let input = dir.path().join("bad.wav");
    fs::write(&input, b"this is not audio at all").unwrap();
    let strategy = Strategy {
        delete_original: true,
        ..Strategy::safe()
    };
    let opts = options(Codec::Alac, strategy, dir.path());

    export_whole(&ffmpeg, &input, plan(0.0, "out.m4a"), &opts).unwrap_err();

    assert_eq!(file_names(dir.path()), ["bad.wav"]);
}

#[test]
fn replace_overwrites_a_source_with_the_same_name() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("song.wav", 2, 200);
    let before = loudness(&input);

    let opts = options(Codec::Wav, overwrite(), dir.path());
    let written = export_whole(&ffmpeg, &input, plan(-6.0, "song.wav"), &opts).unwrap();

    assert_eq!(written[0].path, input);
    assert!(written[0].overwrote);
    assert!((loudness(&input) - (before - 6.0)).abs() < 0.05);
    assert_eq!(file_names(dir.path()), ["song.wav"], "no leftovers");
}

#[test]
fn replace_never_removes_a_source_with_a_different_name() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("song.wav", 2, 200);
    let original_bytes = fs::read(&input).unwrap();

    let opts = options(Codec::Alac, overwrite(), dir.path());
    let written = export_whole(&ffmpeg, &input, plan(0.0, "song.m4a"), &opts).unwrap();

    assert!(!written[0].overwrote);
    assert_eq!(fs::read(&input).unwrap(), original_bytes);
    assert_eq!(file_names(dir.path()), ["song.m4a", "song.wav"]);
}

#[test]
fn rename_old_keeps_the_original_under_the_old_name() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("song.wav", 2, 200);
    let original_bytes = fs::read(&input).unwrap();
    let before = loudness(&input);

    let opts = options(Codec::Wav, rename_existing(), dir.path());
    let written = export_whole(&ffmpeg, &input, plan(-6.0, "song.wav"), &opts).unwrap();

    let old = dir.path().join("song_old.wav");
    assert_eq!(
        written[0].renamed_existing_to.as_deref(),
        Some(old.as_path())
    );
    assert_eq!(
        fs::read(&old).unwrap(),
        original_bytes,
        "the original is intact"
    );
    assert!(
        (loudness(&input) - (before - 6.0)).abs() < 0.05,
        "the new file took the name"
    );
    assert_eq!(file_names(dir.path()), ["song.wav", "song_old.wav"]);
}

#[test]
fn save_to_writes_elsewhere_and_leaves_the_source() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("song.wav", 2, 200);
    let target = tempdir().unwrap();

    let strategy = Strategy {
        destination: Destination::Folder(target.path().to_path_buf()),
        ..Strategy::safe()
    };
    let opts = options(Codec::Flac, strategy, dir.path());
    let written = export_whole(&ffmpeg, &input, plan(0.0, "song.flac"), &opts).unwrap();

    assert_eq!(written[0].path, target.path().join("song.flac"));
    assert_eq!(file_names(dir.path()), ["song.wav"]);
    assert_eq!(file_names(target.path()), ["song.flac"]);
}

#[test]
fn a_failed_placement_changes_nothing_and_leaves_no_temporary_files() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("song.wav", 2, 200);
    // A song and its backup both exist, so renaming must refuse rather than overwrite.
    fs::write(dir.path().join("out.m4a"), b"current").unwrap();
    fs::write(dir.path().join("out_old.m4a"), b"older backup").unwrap();

    let opts = options(Codec::Alac, rename_existing(), dir.path());
    let error = export_whole(&ffmpeg, &input, plan(0.0, "out.m4a"), &opts).unwrap_err();

    assert!(
        matches!(error, ExportError::DestinationExists(_)),
        "{error}"
    );
    assert_eq!(fs::read(dir.path().join("out.m4a")).unwrap(), b"current");
    assert_eq!(
        fs::read(dir.path().join("out_old.m4a")).unwrap(),
        b"older backup"
    );
    assert_eq!(
        file_names(dir.path()),
        ["out.m4a", "out_old.m4a", "song.wav"]
    );
}

#[test]
fn an_unreadable_source_fails_cleanly() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let dir = tempdir().unwrap();
    let input = dir.path().join("bad.wav");
    fs::write(&input, b"this is not audio at all").unwrap();

    let opts = options(Codec::Alac, serialize(), dir.path());
    let error = export_whole(&ffmpeg, &input, plan(0.0, "out.m4a"), &opts).unwrap_err();

    assert!(matches!(error, ExportError::Audio(_)), "{error}");
    assert_eq!(file_names(dir.path()), ["bad.wav"]);
}

#[test]
fn a_plan_for_the_wrong_number_of_tracks_is_refused() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("song.wav", 1, 200);
    let opts = options(Codec::Alac, serialize(), dir.path());

    let two = [plan(0.0, "a.m4a"), plan(0.0, "b.m4a")];
    let error = export_file(&ffmpeg, &input, None, &two, &opts, &mut |_| {}).unwrap_err();
    assert!(
        matches!(error, ExportError::TrackCountMismatch { .. }),
        "{error}"
    );
    assert_eq!(file_names(dir.path()), ["song.wav"]);
}

#[test]
fn preview_matches_what_a_run_does_and_writes_nothing() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("song.wav", 1, 200);
    fs::write(dir.path().join("song.m4a"), b"taken").unwrap();
    fs::write(dir.path().join("song_00.m4a"), b"taken").unwrap();
    let opts = options(Codec::Alac, serialize(), dir.path());
    let tracks = [plan(0.0, "song.m4a")];

    let planned = preview(&opts, &tracks).unwrap();
    assert_eq!(
        file_names(dir.path()),
        ["song.m4a", "song.wav", "song_00.m4a"],
        "preview wrote nothing"
    );

    let written = export_file(&ffmpeg, &input, None, &tracks, &opts, &mut |_| {}).unwrap();
    assert_eq!(planned[0].final_path, written[0].path);
    assert_eq!(written[0].path, dir.path().join("song_01.m4a"));
}

/// The sample rate FFmpeg says a file has, by reading its description of the audio stream.
fn probed_rate(ffmpeg: &Ffmpeg, path: &Path) -> u32 {
    let output = Command::new(ffmpeg.path())
        .args(["-hide_banner", "-i"])
        .arg(path)
        .output()
        .unwrap();
    // FFmpeg prints a description such as "Audio: flac, 96000 Hz, stereo, s32" on standard
    // error, and complains about having no output file, which is expected here.
    let text = String::from_utf8_lossy(&output.stderr).into_owned();
    let line = text
        .lines()
        .find(|line| line.contains("Audio:"))
        .unwrap_or_else(|| panic!("no audio stream described in:\n{text}"));
    line.split(',')
        .find_map(|part| part.trim().strip_suffix(" Hz")?.parse().ok())
        .unwrap_or_else(|| panic!("no rate in: {line}"))
}

#[test]
fn every_codec_can_be_written_at_a_new_rate_in_either_container() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("in.wav", 2, 200);
    let source_loudness = loudness(&input);

    // (MP3 stops at 48 kHz and Opus is 48 kHz only: they have their own test below.)
    for codec in Codec::ALL.into_iter().filter(|c| c.supports_rate(96_000)) {
        for container in Container::ALL {
            let format = OutputFormat::new(codec, BitDepth::TwentyFour)
                .with_container(container)
                .with_sample_rate(SampleRate::Hz96000);
            let name = format!(
                "{}-{}.{}",
                codec.name(),
                container.name(),
                format.extension()
            );
            let opts = Options {
                format,
                strategy: serialize(),
                directory: dir.path().to_path_buf(),
                copy_source_tags: false,
                mix: rangeforge_core::mix::MixPlan::default(),
            };
            let written = export_whole(&ffmpeg, &input, plan(0.0, &name), &opts).unwrap();

            let path = &written[0].path;
            assert_eq!(probed_rate(&ffmpeg, path), 96_000, "{name}");
            let extension = path.extension().unwrap().to_string_lossy().into_owned();
            assert_eq!(extension, format.extension(), "{name}");
            // Resampling must not change how loud it is.
            if container == Container::Native && codec != Codec::Aac {
                let after = loudness(path);
                assert!(
                    (after - source_loudness).abs() < 0.1,
                    "{name}: {source_loudness} -> {after}"
                );
            }
        }
    }
}

#[test]
fn mp3_and_opus_are_written_at_the_rates_they_can_hold_in_either_container() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("in.wav", 2, 200);
    // (The test source is 44.1 kHz.)
    for (codec, expected) in [(Codec::Mp3, 44_100), (Codec::Opus, 48_000)] {
        for container in Container::ALL {
            let format = OutputFormat::new(codec, BitDepth::Source).with_container(container);
            let name = format!(
                "{}-{}.{}",
                codec.name(),
                container.name(),
                format.extension()
            );
            let opts = Options {
                format,
                strategy: serialize(),
                directory: dir.path().to_path_buf(),
                copy_source_tags: false,
                mix: rangeforge_core::mix::MixPlan::default(),
            };
            let written = export_whole(&ffmpeg, &input, plan(-3.0, &name), &opts).unwrap();
            assert_eq!(probed_rate(&ffmpeg, &written[0].path), expected, "{name}");
            assert_eq!(
                written[0].path.extension().unwrap().to_string_lossy(),
                format.extension()
            );
        }
    }
    // A variable-rate MP3 and FLAC at its smallest setting are written too.
    for format in [
        OutputFormat::new(Codec::Mp3, BitDepth::Source).with_bitrate(Bitrate::Vbr(2)),
        OutputFormat::new(Codec::Flac, BitDepth::Source).with_flac_level(8),
    ] {
        let opts = Options {
            format,
            strategy: serialize(),
            directory: dir.path().to_path_buf(),
            copy_source_tags: false,
            mix: rangeforge_core::mix::MixPlan::default(),
        };
        let written = export_whole(&ffmpeg, &input, plan(0.0, "other"), &opts).unwrap();
        assert!(written[0].path.metadata().unwrap().len() > 0);
    }
}

#[test]
fn mp3_refuses_a_file_with_more_than_two_channels_and_says_what_to_do() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let dir = tempdir().unwrap();
    let input = surround_source(dir.path());
    let opts = Options {
        format: OutputFormat::new(Codec::Mp3, BitDepth::Source),
        strategy: serialize(),
        directory: dir.path().to_path_buf(),
        copy_source_tags: false,
        mix: rangeforge_core::mix::MixPlan::default(),
    };
    let error = export_whole(&ffmpeg, &input, plan(0.0, "six"), &opts).unwrap_err();
    assert!(error.to_string().contains("Stereo"), "{error}");
    // Mixed to stereo first, it is written.
    let mixed = Options {
        mix: rangeforge_core::mix::MixPlan {
            channels: rangeforge_core::mix::Channels::Stereo,
            style: rangeforge_core::mix::Downmix::Standard,
        },
        ..opts
    };
    assert!(export_whole(&ffmpeg, &input, plan(0.0, "six"), &mixed).is_ok());
}

#[test]
fn opus_and_aac_carry_five_point_one_and_quad() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let dir = tempdir().unwrap();
    let input = surround_source(dir.path());
    for codec in [Codec::Opus, Codec::Aac] {
        for channels in [
            rangeforge_core::mix::Channels::Original,
            rangeforge_core::mix::Channels::Quad,
        ] {
            let opts = Options {
                format: OutputFormat::new(codec, BitDepth::Source),
                strategy: serialize(),
                directory: dir.path().to_path_buf(),
                copy_source_tags: false,
                mix: rangeforge_core::mix::MixPlan {
                    channels,
                    style: rangeforge_core::mix::Downmix::Standard,
                },
            };
            let name = format!("{}-{}", codec.name(), channels.name());
            let written = export_whole(&ffmpeg, &input, plan(0.0, &name), &opts);
            assert!(written.is_ok(), "{name}: {:?}", written.err());
        }
    }
}

#[test]
fn the_original_rate_is_left_alone_and_asking_for_the_same_rate_changes_nothing() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("in.wav", 1, 200);
    for rate in [SampleRate::Original, SampleRate::Hz44100] {
        let format = OutputFormat::new(Codec::Flac, BitDepth::Source).with_sample_rate(rate);
        let opts = Options {
            format,
            strategy: serialize(),
            directory: dir.path().to_path_buf(),
            copy_source_tags: false,
            mix: rangeforge_core::mix::MixPlan::default(),
        };
        let name = format!("kept-{}.flac", rate.name());
        let written = export_whole(&ffmpeg, &input, plan(0.0, &name), &opts).unwrap();
        assert_eq!(probed_rate(&ffmpeg, &written[0].path), 44_100);
        // At 0 dB and the same rate, the samples are still bit for bit the source's.
        let (_, ours) = decode_all(&written[0].path);
        let (_, theirs) = decode_all(&input);
        assert!(ours == theirs, "{name}: samples differ");
    }
}

#[test]
fn matroska_files_carry_their_tags() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("in.wav", 1, 200);
    let format = OutputFormat::new(Codec::Flac, BitDepth::Source).with_container(Container::Mka);
    let opts = Options {
        format,
        strategy: serialize(),
        directory: dir.path().to_path_buf(),
        copy_source_tags: false,
        mix: rangeforge_core::mix::MixPlan::default(),
    };
    let tags = Tags {
        title: Some("The Rain Song".to_string()),
        artist: Some("Led Zeppelin".to_string()),
        album: Some("Houses of the Holy".to_string()),
        album_artist: Some("Led Zeppelin".to_string()),
        track: Some(2),
        track_total: Some(8),
        genre: Some("Classic Rock, Hard Rock, Radio".to_string()),
        date: Some("1973".to_string()),
    };
    let plan = TrackPlan {
        tags,
        ..plan(0.0, "tagged.mka")
    };
    let written = export_whole(&ffmpeg, &input, plan, &opts).unwrap();

    let metadata = Command::new(ffmpeg.path())
        .args(["-v", "error", "-i"])
        .arg(&written[0].path)
        .args(["-f", "ffmetadata", "-"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&metadata.stdout).to_lowercase();
    for expected in [
        "title=the rain song",
        "artist=led zeppelin",
        "album=houses of the holy",
        "genre=classic rock, hard rock, radio",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }
}

#[test]
fn tags_written_to_a_file_can_be_read_back() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("in.wav", 1, 200);
    let tags = Tags {
        title: Some("The Rain Song".to_string()),
        artist: Some("Led Zeppelin".to_string()),
        album: Some("Houses of the Holy".to_string()),
        album_artist: Some("Led Zeppelin".to_string()),
        track: Some(2),
        track_total: Some(8),
        genre: Some("Rock".to_string()),
        date: Some("1973".to_string()),
    };
    for (codec, name) in [(Codec::Flac, "tagged.flac"), (Codec::Alac, "tagged.m4a")] {
        let opts = options(codec, serialize(), dir.path());
        let plan = TrackPlan {
            tags: tags.clone(),
            ..plan(0.0, name)
        };
        let written = export_whole(&ffmpeg, &input, plan, &opts).unwrap();

        let found = rangeforge_audio::tags::read_tags(&written[0].path).unwrap();
        assert_eq!(found.title.as_deref(), Some("The Rain Song"), "{name}");
        assert_eq!(found.artist.as_deref(), Some("Led Zeppelin"), "{name}");
        assert_eq!(found.album.as_deref(), Some("Houses of the Holy"), "{name}");
        assert_eq!(
            found.album_artist.as_deref(),
            Some("Led Zeppelin"),
            "{name}"
        );
        assert_eq!(found.genre.as_deref(), Some("Rock"), "{name}");
        assert_eq!(found.date.as_deref(), Some("1973"), "{name}");
        assert_eq!(found.track, Some(2), "{name}");
    }

    // A file with no tags gives back nothing, not an error.
    let opts = options(Codec::Flac, serialize(), dir.path());
    let written = export_whole(&ffmpeg, &input, plan(0.0, "bare.flac"), &opts).unwrap();
    assert!(
        rangeforge_audio::tags::read_tags(&written[0].path)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn single_files_carry_their_tags_over() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("in.wav", 2, 200);

    // First make a tagged FLAC, to stand in for a library file that already has tags.
    let tagged = TrackPlan {
        gain_db: 0.0,
        file_name: "tagged.flac".to_string(),
        tags: Tags {
            title: Some("Old Title".to_string()),
            artist: Some("Original Artist".to_string()),
            ..Tags::default()
        },
        skip: false,
    };
    let make = options(Codec::Flac, serialize(), dir.path());
    let source = export_whole(&ffmpeg, &input, tagged, &make)
        .unwrap()
        .remove(0)
        .path;

    // Now convert it, copying its tags, with a new title on top.
    let mut convert = options(Codec::Alac, serialize(), dir.path());
    convert.copy_source_tags = true;
    let new_title = TrackPlan {
        gain_db: -1.0,
        file_name: "converted.m4a".to_string(),
        tags: Tags {
            title: Some("New Title".to_string()),
            ..Tags::default()
        },
        skip: false,
    };
    let written = export_whole(&ffmpeg, &source, new_title, &convert).unwrap();

    let metadata = Command::new(ffmpeg.path())
        .args(["-v", "error", "-i"])
        .arg(&written[0].path)
        .args(["-f", "ffmetadata", "-"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&metadata.stdout);
    assert!(
        text.contains("artist=Original Artist"),
        "artist carried over:\n{text}"
    );
    assert!(
        text.contains("title=New Title"),
        "the new title won:\n{text}"
    );
    assert!(!text.contains("Old Title"), "{text}");
}

/// Writes about a second of DSD64: a 1 kHz tone from a simple one-bit modulator, in a DSF file
/// with `channels` channels.
fn write_dsf(path: &Path, channels: u32) {
    const RATE: u32 = 2_822_400;
    const BLOCK: usize = 4096;
    const BLOCKS: usize = 86;
    let bits = BLOCKS * BLOCK * 8;
    // One block-sized run of bytes per channel, channel after channel, block after block.
    let mut data = vec![0_u8; channels as usize * BLOCKS * BLOCK];
    for channel in 0..channels as usize {
        let (mut integrator, mut out) = (0.0_f64, -1.0_f64);
        for n in 0..bits {
            #[allow(clippy::cast_precision_loss)]
            let x = 0.5 * (std::f64::consts::TAU * 1000.0 * n as f64 / f64::from(RATE)).sin();
            integrator += x - out;
            out = if integrator >= 0.0 { 1.0 } else { -1.0 };
            if out > 0.0 {
                // Bits are stored lowest first within each byte.
                let (block, within) = (n / (BLOCK * 8), n % (BLOCK * 8));
                let at = (block * channels as usize + channel) * BLOCK + within / 8;
                data[at] |= 1 << (within % 8);
            }
        }
    }
    let mut bytes = Vec::new();
    bytes.extend(b"DSD ");
    bytes.extend(28_u64.to_le_bytes());
    bytes.extend((92 + data.len() as u64).to_le_bytes());
    bytes.extend(0_u64.to_le_bytes());
    bytes.extend(b"fmt ");
    bytes.extend(52_u64.to_le_bytes());
    bytes.extend(1_u32.to_le_bytes());
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend((if channels == 2 { 2_u32 } else { 7 }).to_le_bytes());
    bytes.extend(channels.to_le_bytes());
    bytes.extend(RATE.to_le_bytes());
    bytes.extend(1_u32.to_le_bytes());
    bytes.extend((bits as u64).to_le_bytes());
    bytes.extend(u32::try_from(BLOCK).unwrap().to_le_bytes());
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend((12 + data.len() as u64).to_le_bytes());
    bytes.extend(data);
    fs::write(path, bytes).unwrap();
}

#[test]
fn dsd_is_measured_at_a_fixed_rate_with_every_channel() {
    let Some(_ffmpeg) = ffmpeg() else { return };
    let dir = tempdir().unwrap();
    for channels in [2, 6] {
        let input = dir.path().join(format!("tone{channels}.dsf"));
        write_dsf(&input, channels);
        let measured = measure_whole_file(&input, MixPlan::default()).unwrap();
        assert_eq!(measured.sample_rate, 88_200);
        assert_eq!(measured.channels, channels as usize);
        let seconds = measured.duration_seconds();
        assert!((0.98..1.02).contains(&seconds), "{seconds} s");
        let lufs = measured.tracks[0].integrated_lufs;
        assert!(lufs.is_finite() && lufs > -40.0, "{lufs} LUFS");
    }
}

#[test]
fn dsd_is_written_at_the_asked_rate_and_depth() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let dir = tempdir().unwrap();
    let input = dir.path().join("tone.dsf");
    write_dsf(&input, 2);
    for rate in [
        SampleRate::Hz44100,
        SampleRate::Hz88200,
        SampleRate::Hz176400,
    ] {
        for codec in [Codec::Alac, Codec::Flac] {
            let format = OutputFormat::new(codec, BitDepth::TwentyFour).with_sample_rate(rate);
            let opts = Options {
                format,
                strategy: serialize(),
                directory: dir.path().to_path_buf(),
                copy_source_tags: false,
                mix: rangeforge_core::mix::MixPlan::default(),
            };
            let name = format!(
                "out-{}-{}.{}",
                codec.name(),
                rate.name(),
                format.extension()
            );
            let written = export_whole(&ffmpeg, &input, plan(-3.0, &name), &opts).unwrap();
            assert_eq!(
                probed_rate(&ffmpeg, &written[0].path),
                rate.hz().unwrap(),
                "{name}"
            );
            // A tone that was written is not silence, and the gain took it down 3 dB.
            let written_lufs = loudness(&written[0].path);
            let source_lufs = measure_whole_file(&input, MixPlan::default())
                .unwrap()
                .tracks[0]
                .integrated_lufs;
            assert!(
                (written_lufs - (source_lufs - 3.0)).abs() < 0.3,
                "{name}: {written_lufs} against {source_lufs}"
            );
        }
    }
}

/// Adds an ID3v2.3 tag to the end of a DSF file, which is where DSF keeps its tags, and points
/// the header at it.
fn add_id3(path: &Path, frames: &[(&str, &str)]) {
    let mut body = Vec::new();
    for (id, text) in frames {
        body.extend(id.as_bytes());
        body.extend((u32::try_from(text.len()).unwrap() + 1).to_be_bytes());
        body.extend([0, 0]);
        body.push(0); // Latin-1
        body.extend(text.as_bytes());
    }
    // The tag's size is written seven bits to a byte.
    let size = u32::try_from(body.len()).unwrap();
    let syncsafe = [
        u8::try_from((size >> 21) & 0x7f).unwrap(),
        u8::try_from((size >> 14) & 0x7f).unwrap(),
        u8::try_from((size >> 7) & 0x7f).unwrap(),
        u8::try_from(size & 0x7f).unwrap(),
    ];
    let mut bytes = fs::read(path).unwrap();
    let pointer = bytes.len() as u64;
    bytes[20..28].copy_from_slice(&pointer.to_le_bytes());
    bytes.extend(b"ID3");
    bytes.extend([3, 0, 0]);
    bytes.extend(syncsafe);
    bytes.extend(body);
    let total = bytes.len() as u64;
    bytes[12..20].copy_from_slice(&total.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

#[test]
fn the_tags_of_a_dsd_file_are_read() {
    let Some(_ffmpeg) = ffmpeg() else { return };
    let dir = tempdir().unwrap();
    let input = dir.path().join("tagged.dsf");
    write_dsf(&input, 2);
    add_id3(
        &input,
        &[
            ("TIT2", "Total Eclipse"),
            ("TPE1", "Alan Parsons Project"),
            ("TALB", "I Robot"),
            ("TCON", "Progressive Rock"),
            ("TRCK", "9/10"),
            ("TYER", "1977"),
        ],
    );
    let tags = rangeforge_audio::tags::read_tags(&input).unwrap();
    assert_eq!(tags.title.as_deref(), Some("Total Eclipse"));
    assert_eq!(tags.artist.as_deref(), Some("Alan Parsons Project"));
    assert_eq!(tags.album.as_deref(), Some("I Robot"));
    assert_eq!(tags.genre.as_deref(), Some("Progressive Rock"));
    assert_eq!(tags.track, Some(9));
    assert_eq!(tags.date.as_deref(), Some("1977"));

    // And a DSD file with no tags simply has none.
    let bare = dir.path().join("bare.dsf");
    write_dsf(&bare, 2);
    assert!(rangeforge_audio::tags::read_tags(&bare).unwrap().is_empty());
}

// ---- mixing channels while exporting ----

const MASK_51: u32 = 0x3F; // front left and right, centre, LFE, back left and right

/// A 16-bit WAVE file with an explicit channel mask, which is how more than two channels are
/// stored. `sample(channel, frame)` gives each sample from -1.0 to 1.0.
fn write_surround_wav(
    path: &Path,
    channels: u16,
    mask: u32,
    frames: usize,
    sample: impl Fn(usize, usize) -> f32,
) {
    let rate: u32 = 44_100;
    let mut data = Vec::new();
    for frame in 0..frames {
        for channel in 0..usize::from(channels) {
            #[allow(clippy::cast_possible_truncation)]
            let value = (sample(channel, frame).clamp(-1.0, 1.0) * 32767.0).round() as i16;
            data.extend_from_slice(&value.to_le_bytes());
        }
    }
    let align = channels * 2;
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&0xFFFE_u16.to_le_bytes());
    fmt.extend_from_slice(&channels.to_le_bytes());
    fmt.extend_from_slice(&rate.to_le_bytes());
    fmt.extend_from_slice(&(rate * u32::from(align)).to_le_bytes());
    fmt.extend_from_slice(&align.to_le_bytes());
    fmt.extend_from_slice(&16_u16.to_le_bytes());
    fmt.extend_from_slice(&22_u16.to_le_bytes());
    fmt.extend_from_slice(&16_u16.to_le_bytes());
    fmt.extend_from_slice(&mask.to_le_bytes());
    fmt.extend_from_slice(&[
        1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71,
    ]);
    let mut file = Vec::new();
    file.extend_from_slice(b"RIFF");
    file.extend_from_slice(
        &(4 + 8 + u32::try_from(fmt.len()).unwrap() + 8 + u32::try_from(data.len()).unwrap())
            .to_le_bytes(),
    );
    file.extend_from_slice(b"WAVEfmt ");
    file.extend_from_slice(&u32::try_from(fmt.len()).unwrap().to_le_bytes());
    file.extend_from_slice(&fmt);
    file.extend_from_slice(b"data");
    file.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    file.extend_from_slice(&data);
    fs::write(path, file).unwrap();
}

/// A different tone in each channel of a 5.1 file, 3 seconds long.
fn surround_source(dir: &Path) -> PathBuf {
    let path = dir.join("surround.wav");
    let hertz = [300.0_f32, 400.0, 500.0, 60.0, 700.0, 800.0];
    write_surround_wav(&path, 6, MASK_51, 44_100 * 3, |channel, frame| {
        #[allow(clippy::cast_precision_loss)]
        let t = frame as f32 / 44_100.0;
        0.25 * (std::f32::consts::TAU * hertz[channel] * t).sin()
    });
    path
}

fn mixed_options(codec: Codec, bits: BitDepth, channels: Channels, dir: &Path) -> Options {
    let mut opts = options(codec, serialize(), dir);
    opts.format = OutputFormat::new(codec, bits);
    opts.mix = MixPlan {
        channels,
        style: Downmix::Standard,
    };
    opts
}

#[test]
fn a_5_1_file_is_written_as_stereo_and_the_gain_comes_after_the_mix() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let dir = tempdir().unwrap();
    let input = surround_source(dir.path());
    let (_, source) = decode_all(&input);
    let opts = mixed_options(
        Codec::Flac,
        BitDepth::TwentyFour,
        Channels::Stereo,
        dir.path(),
    );

    // 6.0206 dB down is exactly half, applied to the mixed sound.
    let written = export_whole(&ffmpeg, &input, plan(-6.0206, "stereo.flac"), &opts).unwrap();
    let (info, out) = decode_all(&written[0].path);
    assert_eq!((info.channels, info.sample_rate), (2, 44_100));
    assert_eq!(out.len(), 44_100 * 3 * 2);
    for frame in (0..44_100 * 3).step_by(7) {
        let s = |channel: usize| source[frame * 6 + channel];
        let left = 0.5 * (s(0) + MINUS_3_DB * s(2) + MINUS_3_DB * s(4));
        let right = 0.5 * (s(1) + MINUS_3_DB * s(2) + MINUS_3_DB * s(5));
        assert!((out[frame * 2] - left).abs() < 2e-4, "left, frame {frame}");
        assert!(
            (out[frame * 2 + 1] - right).abs() < 2e-4,
            "right, frame {frame}"
        );
    }
}

#[test]
fn what_is_written_measures_as_the_table_said_it_would() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let dir = tempdir().unwrap();
    let input = surround_source(dir.path());
    let mix = MixPlan {
        channels: Channels::Stereo,
        style: Downmix::Standard,
    };
    // What the program shows before the run: loudness and peak of the mixed sound.
    let shown = measure_whole_file(&input, mix).unwrap().tracks[0];
    // The same sound after being written at 0 dB, measured as the plain file it now is.
    let opts = mixed_options(
        Codec::Flac,
        BitDepth::TwentyFour,
        Channels::Stereo,
        dir.path(),
    );
    let written = export_whole(&ffmpeg, &input, plan(0.0, "stereo.flac"), &opts).unwrap();
    let after = measure_whole_file(&written[0].path, MixPlan::default())
        .unwrap()
        .tracks[0];
    assert!(
        (shown.integrated_lufs - after.integrated_lufs).abs() < 0.01,
        "{shown:?} {after:?}"
    );
    assert!(
        (shown.true_peak_dbtp - after.true_peak_dbtp).abs() < 0.05,
        "{shown:?} {after:?}"
    );
}

#[test]
fn quad_is_written_with_the_centre_folded_in_and_alac_refuses_it_cleanly() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let dir = tempdir().unwrap();
    let input = surround_source(dir.path());
    let (_, source) = decode_all(&input);

    let opts = mixed_options(
        Codec::Flac,
        BitDepth::TwentyFour,
        Channels::Quad,
        dir.path(),
    );
    let written = export_whole(&ffmpeg, &input, plan(0.0, "quad.flac"), &opts).unwrap();
    let (info, quad) = decode_all(&written[0].path);
    assert_eq!(info.channels, 4);
    for frame in (0..44_100 * 3).step_by(11) {
        let s = |channel: usize| source[frame * 6 + channel];
        let at = |channel: usize| quad[frame * 4 + channel];
        assert!(
            (at(0) - (s(0) + MINUS_3_DB * s(2))).abs() < 2e-4,
            "front left"
        );
        assert!(
            (at(1) - (s(1) + MINUS_3_DB * s(2))).abs() < 2e-4,
            "front right"
        );
        assert!((at(2) - s(4)).abs() < 2e-4, "back left");
        assert!((at(3) - s(5)).abs() < 2e-4, "back right");
    }

    // ALAC cannot hold quad: a plain error, and nothing left behind.
    let before = file_names(dir.path());
    let alac = mixed_options(Codec::Alac, BitDepth::Source, Channels::Quad, dir.path());
    let refused = export_whole(&ffmpeg, &input, plan(0.0, "quad.m4a"), &alac);
    assert!(
        matches!(refused, Err(ExportError::QuadNeedsAnotherCodec { .. })),
        "{refused:?}"
    );
    assert_eq!(file_names(dir.path()), before, "no half-written file");
}

#[test]
fn a_stereo_file_asked_for_stereo_stays_bit_exact_and_mono_is_the_average() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let (dir, input) = with_source("in.wav", 2, 200);
    let (_, original) = decode_all(&input);

    let same = mixed_options(Codec::Wav, BitDepth::Source, Channels::Stereo, dir.path());
    let written = export_whole(&ffmpeg, &input, plan(0.0, "same.wav"), &same).unwrap();
    assert_eq!(
        decode_all(&written[0].path).1,
        original,
        "nothing to mix, nothing changed"
    );

    let mono = mixed_options(
        Codec::Flac,
        BitDepth::TwentyFour,
        Channels::Mono,
        dir.path(),
    );
    let written = export_whole(&ffmpeg, &input, plan(0.0, "mono.flac"), &mono).unwrap();
    let (info, out) = decode_all(&written[0].path);
    assert_eq!(info.channels, 1);
    for frame in (0..original.len() / 2).step_by(13) {
        let average = f32::midpoint(original[frame * 2], original[frame * 2 + 1]);
        assert!((out[frame] - average).abs() < 2e-4, "frame {frame}");
    }
}
