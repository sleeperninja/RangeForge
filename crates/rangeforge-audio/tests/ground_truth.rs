// Helpers outside `#[test]` functions may unwrap: a failure here is a failed test.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Checks measurements against a real album kept in `testdata/` (not in git).
//!
//! These tests read real audio, so they are skipped by default and on CI. Run them with:
//!
//! ```text
//! cargo test -p rangeforge-audio --release -- --ignored
//! ```
//!
//! The album is "Houses of the Holy" (Atlantic 19130-2), ripped with Exact Audio Copy:
//! one FLAC plus its CUE sheet, and tracks 3, 6, 7 and 8 as separate ALAC files.

use std::fs;
use std::path::PathBuf;

use rangeforge_audio::measure::{measure_segments, measure_whole_file};
use rangeforge_core::cue;
use rangeforge_core::planner::Measurement;

fn album_dir() -> PathBuf {
    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    fs::read_dir(&testdata)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", testdata.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("1973. Houses"))
        })
        .expect("testdata/ has no folder starting with \"1973. Houses\"")
}

fn track_file(dir: &std::path::Path, number: &str) -> PathBuf {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            let is_m4a = path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("m4a"));
            let numbered = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&format!("{number} ")));
            is_m4a && numbered
        })
        .unwrap_or_else(|| panic!("no .m4a for track {number}"))
}

fn assert_close(label: &str, actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{label}: {actual} against {expected} (allowed difference {tolerance})"
    );
}

fn measure_album(dir: &std::path::Path) -> Vec<Measurement> {
    let sheet =
        cue::parse(&fs::read_to_string(dir.join("Houses of the Holy.cue")).unwrap()).unwrap();
    let segments = sheet.files[0].segments();
    measure_segments(&dir.join("Houses of the Holy.flac"), &segments)
        .unwrap()
        .tracks
}

#[test]
#[ignore = "reads real audio from testdata/, which is not in git"]
fn tracks_cut_from_the_flac_match_the_separate_alac_files() {
    let dir = album_dir();
    let album = measure_album(&dir);
    assert_eq!(album.len(), 8);

    for number in [3_usize, 6, 7, 8] {
        let separate = measure_whole_file(&track_file(&dir, &format!("{number:02}")))
            .unwrap()
            .tracks[0];
        let cut = album[number - 1];
        let label = format!("track {number}");
        assert_close(
            &format!("{label} LUFS"),
            cut.integrated_lufs,
            separate.integrated_lufs,
            0.01,
        );
        assert_close(
            &format!("{label} dBTP"),
            cut.true_peak_dbtp,
            separate.true_peak_dbtp,
            0.01,
        );
        assert_close(
            &format!("{label} LU"),
            cut.loudness_range_lu,
            separate.loudness_range_lu,
            0.01,
        );
    }
}

/// FFmpeg's `ebur128` filter, run on the same four tracks. FFmpeg prints one decimal, so
/// loudness and peak are allowed 0.1. Loudness range is deliberately not compared: it
/// differs from FFmpeg by up to 0.4 LU on some tracks (see decision D-04).
#[test]
#[ignore = "reads real audio from testdata/, which is not in git"]
fn loudness_and_peak_agree_with_ffmpeg() {
    let dir = album_dir();
    let album = measure_album(&dir);
    // (track, FFmpeg LUFS, FFmpeg dBTP)
    let reference = [
        (3_usize, -18.4, -2.4),
        (6, -17.4, -2.3),
        (7, -19.1, -2.4),
        (8, -15.4, -2.2),
    ];
    for (number, lufs, peak) in reference {
        let measured = album[number - 1];
        assert_close(
            &format!("track {number} LUFS"),
            measured.integrated_lufs,
            lufs,
            0.1,
        );
        assert_close(
            &format!("track {number} dBTP"),
            measured.true_peak_dbtp,
            peak,
            0.1,
        );
    }
}
