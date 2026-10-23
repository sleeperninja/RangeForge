//! Checks the CUE reader against a real Exact Audio Copy rip.
//!
//! The fixture is the CUE sheet for "Houses of the Holy" (Atlantic 19130-2). The expected
//! lengths come from individual ALAC tracks cut from the same rip, measured with ffprobe,
//! so they are ground truth for where each track starts and ends.

use rangeforge_core::cue::{self, Frames};

const SHEET: &str = include_str!("fixtures/houses-of-the-holy.cue");

/// 2460.200 s, from ffprobe on the single-file rip.
const FILE_END: Frames = Frames(184_515);

#[test]
fn reads_the_whole_album() {
    let sheet = cue::parse(SHEET).unwrap();
    assert_eq!(sheet.title.as_deref(), Some("Houses of the Holy"));
    assert_eq!(sheet.performer.as_deref(), Some("Led Zeppelin"));
    assert_eq!(sheet.disc_id.as_deref(), Some("63099C08"));
    assert_eq!(sheet.files.len(), 1);
    assert_eq!(sheet.files[0].name, "Houses of the Holy.flac");
    assert_eq!(sheet.files[0].tracks.len(), 8);
}

#[test]
fn keeps_pregaps_and_the_first_track_offset() {
    let sheet = cue::parse(SHEET).unwrap();
    let tracks = &sheet.files[0].tracks;
    // Track 1 has INDEX 00 at 00:00:00 and INDEX 01 at 00:00:33.
    assert_eq!(tracks[0].pregap, Some(Frames(0)));
    assert_eq!(tracks[0].start, Frames(33));
    // Track 2 has no INDEX 00.
    assert_eq!(tracks[1].pregap, None);
    // Track 4: INDEX 00 at 18:00:23, INDEX 01 at 18:00:28.
    assert_eq!(tracks[3].pregap, Some(Frames(81_023)));
    assert_eq!(tracks[3].start, Frames(81_028));
}

#[test]
fn track_lengths_match_the_individual_alac_files() {
    let sheet = cue::parse(SHEET).unwrap();
    let segments = sheet.files[0].segments();
    // (track number, length in CD frames, ffprobe duration of the matching .m4a)
    let ground_truth = [
        (3, 21_765, 290.200),
        (6, 19_850, 264.667),
        (7, 31_777, 423.693),
        (8, 20_310, 270.800),
    ];
    for (number, frames, seconds) in ground_truth {
        let segment = segments
            .iter()
            .find(|segment| segment.track.number == number)
            .unwrap();
        let length = segment.length(FILE_END);
        assert_eq!(length, Frames(frames), "track {number}");
        assert!(
            (length.as_seconds() - seconds).abs() < 0.001,
            "track {number}: {} s against {seconds} s",
            length.as_seconds()
        );
    }
}

#[test]
fn tracks_are_contiguous_and_cover_the_file() {
    let sheet = cue::parse(SHEET).unwrap();
    let segments = sheet.files[0].segments();
    for pair in segments.windows(2) {
        assert_eq!(pair[0].end, Some(pair[1].start));
    }
    let total: u32 = segments.iter().map(|s| s.length(FILE_END).0).sum();
    // Everything from track 1's INDEX 01 to the end of the file.
    assert_eq!(total, FILE_END.0 - 33);
}
