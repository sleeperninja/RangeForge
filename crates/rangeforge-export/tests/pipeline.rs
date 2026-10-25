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
use rangeforge_export::ExportError;
use rangeforge_export::ffmpeg::Ffmpeg;
use rangeforge_export::format::{BitDepth, Codec, OutputFormat};
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
    measure_whole_file(path).unwrap().tracks[0].integrated_lufs
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
    let before = measure_whole_file(&input).unwrap().tracks[0];

    for gain in [-6.0, -2.5, 3.0] {
        let opts = options(Codec::Alac, serialize(), dir.path());
        let written = export_whole(&ffmpeg, &input, plan(gain, "out.m4a"), &opts).unwrap();
        let after = measure_whole_file(&written[0].path).unwrap().tracks[0];

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
