//! Reading CUE sheets and measuring files, with or without the cache.

use std::path::Path;

use anyhow::{Context, Result};
use rangeforge_audio::cache::{Cache, Source};
use rangeforge_audio::measure::{self, Measured};
use rangeforge_core::cue::{self, CueSheet, Segment};

/// Reads and parses a CUE sheet.
pub fn read_sheet(path: &Path) -> Result<CueSheet> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    // Many CUE sheets are not UTF-8. Lossy decoding keeps them readable; proper
    // encoding detection comes later.
    let text = String::from_utf8_lossy(&bytes);
    cue::parse(&text).with_context(|| format!("parsing {}", path.display()))
}

/// DSD files are opened through FFmpeg, so find it first. Finding it also tells the audio reader
/// where it is. A missing FFmpeg shows up as the reader's own plain-words error.
fn prepare(audio: &Path) {
    if rangeforge_audio::dsd::is_dsd(audio) {
        let _ = rangeforge_export::ffmpeg::Ffmpeg::locate();
    }
}

/// Measures a whole file as one track.
pub fn measure_whole(audio: &Path, cache: Option<&Cache>) -> Result<(Measured, Source)> {
    prepare(audio);
    Ok(match cache {
        Some(cache) => cache.measure_whole_file(audio)?,
        None => (
            measure::measure_whole_file(audio)?,
            Source::Decoded { save_error: None },
        ),
    })
}

/// Measures each CUE segment of a file.
pub fn measure_split(
    audio: &Path,
    segments: &[Segment<'_>],
    cache: Option<&Cache>,
) -> Result<(Measured, Source)> {
    prepare(audio);
    Ok(match cache {
        Some(cache) => cache.measure_segments(audio, segments)?,
        None => (
            measure::measure_segments(audio, segments)?,
            Source::Decoded { save_error: None },
        ),
    })
}
