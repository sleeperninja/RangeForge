// Helpers outside `#[test]` functions may unwrap: a failure here is a failed test.
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Checks the export pipeline against a real album kept in `testdata/` (not in git).
//!
//! These tests read real audio, so they are skipped by default and on CI. Run them with:
//!
//! ```text
//! cargo test -p rangeforge-export --release --test real_album -- --ignored
//! ```
//!
//! The album is "Houses of the Holy" (Atlantic 19130-2), ripped with Exact Audio Copy: one FLAC
//! plus its CUE sheet, and tracks 3, 6, 7 and 8 as separate ALAC files made from the same rip.

use std::fs;
use std::path::{Path, PathBuf};

use rangeforge_audio::AudioError;
use rangeforge_audio::decode::for_each_block;
use rangeforge_audio::measure::measure_segments;
use rangeforge_core::cue;
use rangeforge_core::planner::{Target, plan_gain};
use rangeforge_export::ffmpeg::Ffmpeg;
use rangeforge_export::format::{BitDepth, Codec, OutputFormat};
use rangeforge_export::naming::track_file_name;
use rangeforge_export::pipeline::{Options, TrackPlan, export_file};
use rangeforge_export::strategy::Strategy;
use rangeforge_export::tags::Tags;
use tempfile::tempdir;

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

fn separate_track(dir: &Path, number: &str) -> PathBuf {
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

fn decode_all(path: &Path) -> Vec<f32> {
    let mut samples = Vec::new();
    for_each_block::<AudioError, _>(path, |_, block| {
        samples.extend_from_slice(block);
        Ok(())
    })
    .unwrap();
    samples
}

/// Exports the whole album with `gain_for` choosing each track's gain.
fn export_album(out_dir: &Path, gain_for: impl Fn(usize) -> f64) -> Vec<PathBuf> {
    let ffmpeg = Ffmpeg::locate().expect("FFmpeg is needed for this test");
    let dir = album_dir();
    let sheet =
        cue::parse(&fs::read_to_string(dir.join("Houses of the Holy.cue")).unwrap()).unwrap();
    let file = &sheet.files[0];
    let segments = file.segments();
    let plans: Vec<TrackPlan> = file
        .tracks
        .iter()
        .enumerate()
        .map(|(index, track)| TrackPlan {
            gain_db: gain_for(index),
            file_name: track_file_name(track.number, track.title.as_deref(), "m4a"),
            tags: Tags::for_cue_track(&sheet, track, file.tracks.len()),
        })
        .collect();
    let options = Options {
        format: OutputFormat::new(Codec::Alac, BitDepth::Source),
        strategy: Strategy::SaveTo {
            directory: out_dir.to_path_buf(),
        },
        directory: dir.clone(),
        copy_source_tags: false,
    };
    export_file(
        &ffmpeg,
        &dir.join("Houses of the Holy.flac"),
        Some(&segments),
        &plans,
        &options,
        &mut |_| {},
    )
    .unwrap()
    .into_iter()
    .map(|written| written.path)
    .collect()
}

#[test]
#[ignore = "reads real audio from testdata/, which is not in git"]
fn tracks_cut_from_the_flac_are_sample_exact_copies_of_the_separate_alac_files() {
    let out = tempdir().unwrap();
    let written = export_album(out.path(), |_| 0.0);
    assert_eq!(written.len(), 8);

    let dir = album_dir();
    for number in [3_usize, 6, 7, 8] {
        let ours = decode_all(&written[number - 1]);
        let theirs = decode_all(&separate_track(&dir, &format!("{number:02}")));
        assert_eq!(ours.len(), theirs.len(), "track {number}: length");
        assert!(ours == theirs, "track {number}: samples differ");
    }
}

#[test]
#[ignore = "reads real audio from testdata/, which is not in git"]
fn every_track_lands_on_the_target_or_the_peak_ceiling() {
    let dir = album_dir();
    let sheet =
        cue::parse(&fs::read_to_string(dir.join("Houses of the Holy.cue")).unwrap()).unwrap();
    let segments = sheet.files[0].segments();
    let measured = measure_segments(&dir.join("Houses of the Holy.flac"), &segments).unwrap();

    // A target loud enough that some quiet tracks must be held back by the peak ceiling.
    let target = Target {
        loudness_lufs: -16.0,
        peak_ceiling_dbtp: Some(-0.6),
    };
    let plans: Vec<_> = measured
        .tracks
        .iter()
        .map(|m| plan_gain(*m, target).unwrap())
        .collect();
    assert!(
        plans.iter().any(|p| p.limited_by_peak),
        "the test needs at least one limited track"
    );
    assert!(
        plans.iter().any(|p| !p.limited_by_peak),
        "and at least one unlimited track"
    );

    let out = tempdir().unwrap();
    let written = export_album(out.path(), |index| plans[index].gain_db);

    for (index, path) in written.iter().enumerate() {
        let after = rangeforge_audio::measure::measure_whole_file(path)
            .unwrap()
            .tracks[0];
        let plan = &plans[index];
        assert!(
            (after.integrated_lufs - plan.resulting_lufs).abs() < 0.05,
            "track {}: {} LUFS, planned {}",
            index + 1,
            after.integrated_lufs,
            plan.resulting_lufs
        );
        assert!(
            after.true_peak_dbtp <= -0.55,
            "track {}: true peak {} is over the ceiling",
            index + 1,
            after.true_peak_dbtp
        );
        if plan.limited_by_peak {
            assert!(
                (after.true_peak_dbtp - -0.6).abs() < 0.1,
                "track {}: limited, so the peak should sit on the ceiling, got {}",
                index + 1,
                after.true_peak_dbtp
            );
            assert!(
                after.integrated_lufs < -16.0 + 0.05,
                "limited tracks stay below the target"
            );
        } else {
            assert!(
                (after.integrated_lufs - -16.0).abs() < 0.05,
                "unlimited tracks hit the target"
            );
        }
    }
}
