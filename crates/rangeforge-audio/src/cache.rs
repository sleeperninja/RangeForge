//! On-disk cache of measurements, so a file measured once is never decoded again.
//!
//! A file is recognised by a fingerprint of its content (size plus a hash of three slices),
//! not by its name or date. Renaming a file, copying it to another folder, or moving it to
//! the NAS therefore keeps its cached results. Results are stored per way of splitting the
//! file, so a different CUE layout gets its own measurements.
//!
//! The cache never makes a measurement fail: anything unreadable, corrupt or out of date is
//! treated as a miss and replaced.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use rangeforge_core::cue::Segment;
use rangeforge_core::planner::Measurement;
use serde::{Deserialize, Serialize};

use crate::AudioError;
use crate::measure::{self, Measured};

/// Bump this when the decoder or meter changes in a way that could change results. Every
/// older cache entry is then ignored.
const ANALYSIS_VERSION: u32 = 1;

/// How much of the file is hashed at each of the three sample points.
const SLICE: u64 = 64 * 1024;

/// Where a result came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Read from the cache. The file was not decoded.
    Cache,
    /// Measured by decoding the file. `save_error` is set if the result could not be stored.
    Decoded { save_error: Option<String> },
}

/// A directory of cached measurements.
#[derive(Debug, Clone)]
pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    /// Uses `dir` for storage. It is created when the first result is saved.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Measures a whole file as one track, using the cache when possible.
    pub fn measure_whole_file(&self, path: &Path) -> Result<(Measured, Source), AudioError> {
        self.through_cache(path, "whole".to_string(), || {
            measure::measure_whole_file(path)
        })
    }

    /// Measures each CUE segment of a file, using the cache when possible.
    pub fn measure_segments(
        &self,
        path: &Path,
        segments: &[Segment<'_>],
    ) -> Result<(Measured, Source), AudioError> {
        self.through_cache(path, spans_key(segments), || {
            measure::measure_segments(path, segments)
        })
    }

    fn through_cache(
        &self,
        path: &Path,
        key: String,
        measure: impl FnOnce() -> Result<Measured, AudioError>,
    ) -> Result<(Measured, Source), AudioError> {
        let (size, digest) = fingerprint(path)?;
        let entry_path = self.dir.join(format!("{digest}.json"));
        let mut entry = load(&entry_path, size).unwrap_or_else(|| Entry {
            version: ANALYSIS_VERSION,
            size,
            results: BTreeMap::new(),
        });

        if let Some(found) = entry.results.get(&key) {
            return Ok((found.to_measured(), Source::Cache));
        }

        let measured = measure()?;
        entry.results.insert(key, CachedResult::from(&measured));
        let save_error = save(&self.dir, &entry_path, &entry)
            .err()
            .map(|error| error.to_string());
        Ok((measured, Source::Decoded { save_error }))
    }
}

/// Identifies one way of splitting a file. Positions are CUE frames, so the key does not
/// depend on the sample rate, which is not known until the file is decoded.
fn spans_key(segments: &[Segment<'_>]) -> String {
    segments
        .iter()
        .map(|segment| {
            let end = segment
                .end
                .map_or_else(String::new, |end| end.0.to_string());
            format!("{}-{end}", segment.start.0)
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// File size and a hash of the size plus three slices (start, middle, end).
fn fingerprint(path: &Path) -> Result<(u64, String), AudioError> {
    let open_error = |source: io::Error| AudioError::Open {
        path: path.to_path_buf(),
        source,
    };
    let mut file = File::open(path).map_err(open_error)?;
    let size = file.metadata().map_err(open_error)?.len();

    let mut hasher = blake3::Hasher::new();
    hasher.update(&size.to_le_bytes());
    let middle = (size / 2).saturating_sub(SLICE / 2);
    let end = size.saturating_sub(SLICE);
    for offset in [0, middle, end] {
        file.seek(SeekFrom::Start(offset)).map_err(open_error)?;
        let mut slice = Vec::new();
        (&mut file)
            .take(SLICE)
            .read_to_end(&mut slice)
            .map_err(open_error)?;
        hasher.update(&slice);
    }
    Ok((size, hasher.finalize().to_hex().to_string()))
}

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    version: u32,
    size: u64,
    results: BTreeMap<String, CachedResult>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CachedResult {
    sample_rate: u32,
    channels: usize,
    total_sample_frames: u64,
    tracks: Vec<CachedTrack>,
}

/// JSON cannot hold infinity, which is what silence measures as. A missing value stands for
/// a non-finite one.
#[derive(Debug, Serialize, Deserialize)]
struct CachedTrack {
    integrated_lufs: Option<f64>,
    true_peak_dbtp: Option<f64>,
    loudness_range_lu: Option<f64>,
}

fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

impl From<&Measured> for CachedResult {
    fn from(measured: &Measured) -> Self {
        Self {
            sample_rate: measured.sample_rate,
            channels: measured.channels,
            total_sample_frames: measured.total_sample_frames,
            tracks: measured
                .tracks
                .iter()
                .map(|track| CachedTrack {
                    integrated_lufs: finite(track.integrated_lufs),
                    true_peak_dbtp: finite(track.true_peak_dbtp),
                    loudness_range_lu: finite(track.loudness_range_lu),
                })
                .collect(),
        }
    }
}

impl CachedResult {
    fn to_measured(&self) -> Measured {
        Measured {
            tracks: self
                .tracks
                .iter()
                .map(|track| Measurement {
                    integrated_lufs: track.integrated_lufs.unwrap_or(f64::NEG_INFINITY),
                    true_peak_dbtp: track.true_peak_dbtp.unwrap_or(f64::NEG_INFINITY),
                    loudness_range_lu: track.loudness_range_lu.unwrap_or(0.0),
                })
                .collect(),
            sample_rate: self.sample_rate,
            channels: self.channels,
            total_sample_frames: self.total_sample_frames,
        }
    }
}

/// Reads an entry, or `None` if it is missing, unreadable, from another version, or for a
/// file of a different size.
fn load(entry_path: &Path, size: u64) -> Option<Entry> {
    let bytes = fs::read(entry_path).ok()?;
    let entry: Entry = serde_json::from_slice(&bytes).ok()?;
    (entry.version == ANALYSIS_VERSION && entry.size == size).then_some(entry)
}

/// Writes an entry by writing a temporary file and renaming it, so a crash cannot leave a
/// half-written entry behind.
fn save(dir: &Path, entry_path: &Path, entry: &Entry) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let temporary = entry_path.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(entry).map_err(io::Error::other)?,
    )?;
    fs::rename(&temporary, entry_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rangeforge_core::cue;
    use tempfile::tempdir;

    /// Writes a short stereo 16-bit 44.1 kHz WAV. `amplitude` of 0 is silence; the sound is a
    /// repeating ramp, which is enough to be measurable.
    fn write_wav(path: &Path, seconds: u32, amplitude: i32) {
        let rate: u32 = 44_100;
        let channels: u16 = 2;
        let frames = rate * seconds;
        let data_len = frames * u32::from(channels) * 2;
        let mut bytes = Vec::with_capacity(44 + usize::try_from(data_len).unwrap());
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for frame in 0..frames {
            let ramp = i32::try_from(frame % 200).unwrap() - 100;
            let sample = i16::try_from(ramp * amplitude).unwrap();
            for _ in 0..channels {
                bytes.extend_from_slice(&sample.to_le_bytes());
            }
        }
        fs::write(path, bytes).unwrap();
    }

    fn entry_files(dir: &Path) -> usize {
        fs::read_dir(dir).map_or(0, |entries| {
            entries
                .filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                .count()
        })
    }

    #[test]
    fn second_measurement_comes_from_the_cache() {
        let audio = tempdir().unwrap();
        let store = tempdir().unwrap();
        let file = audio.path().join("a.wav");
        write_wav(&file, 3, 200);
        let cache = Cache::new(store.path());

        let (first, source) = cache.measure_whole_file(&file).unwrap();
        assert_eq!(source, Source::Decoded { save_error: None });
        let (second, source) = cache.measure_whole_file(&file).unwrap();
        assert_eq!(source, Source::Cache);
        assert_eq!(first, second);
        assert_eq!(entry_files(store.path()), 1);
    }

    #[test]
    fn same_size_but_different_audio_is_a_miss() {
        let audio = tempdir().unwrap();
        let store = tempdir().unwrap();
        let file = audio.path().join("a.wav");
        let cache = Cache::new(store.path());

        write_wav(&file, 3, 100);
        let (quiet, _) = cache.measure_whole_file(&file).unwrap();
        write_wav(&file, 3, 200);
        let (loud, source) = cache.measure_whole_file(&file).unwrap();

        assert_eq!(source, Source::Decoded { save_error: None });
        assert!(loud.tracks[0].integrated_lufs > quiet.tracks[0].integrated_lufs + 3.0);
    }

    #[test]
    fn renamed_or_copied_files_keep_their_cached_results() {
        let audio = tempdir().unwrap();
        let store = tempdir().unwrap();
        let original = audio.path().join("a.wav");
        let renamed = audio.path().join("a_old.wav");
        write_wav(&original, 3, 200);
        let cache = Cache::new(store.path());

        let (first, _) = cache.measure_whole_file(&original).unwrap();
        fs::rename(&original, &renamed).unwrap();
        let (second, source) = cache.measure_whole_file(&renamed).unwrap();

        assert_eq!(source, Source::Cache);
        assert_eq!(first, second);
    }

    #[test]
    fn each_way_of_splitting_a_file_is_cached_separately() {
        let audio = tempdir().unwrap();
        let store = tempdir().unwrap();
        let file = audio.path().join("a.wav");
        write_wav(&file, 3, 200);
        let cache = Cache::new(store.path());

        let one = cue::parse(
            "FILE \"a.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nINDEX 01 00:01:00\n",
        )
        .unwrap();
        let other = cue::parse(
            "FILE \"a.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nINDEX 01 00:02:00\n",
        )
        .unwrap();
        let one = one.files[0].segments();
        let other = other.files[0].segments();

        let (split, source) = cache.measure_segments(&file, &one).unwrap();
        assert!(matches!(source, Source::Decoded { .. }));
        assert_eq!(split.tracks.len(), 2);
        let (_, source) = cache.measure_segments(&file, &one).unwrap();
        assert_eq!(source, Source::Cache);

        // A different split and the whole file are both misses, but share one entry file.
        let (_, source) = cache.measure_segments(&file, &other).unwrap();
        assert!(matches!(source, Source::Decoded { .. }));
        let (_, source) = cache.measure_whole_file(&file).unwrap();
        assert!(matches!(source, Source::Decoded { .. }));
        assert_eq!(entry_files(store.path()), 1);
    }

    #[test]
    fn a_corrupt_or_outdated_entry_is_replaced() {
        let audio = tempdir().unwrap();
        let store = tempdir().unwrap();
        let file = audio.path().join("a.wav");
        write_wav(&file, 3, 200);
        let cache = Cache::new(store.path());
        cache.measure_whole_file(&file).unwrap();

        let (size, digest) = fingerprint(&file).unwrap();
        let entry_path = store.path().join(format!("{digest}.json"));

        fs::write(&entry_path, b"{ this is not json").unwrap();
        let (_, source) = cache.measure_whole_file(&file).unwrap();
        assert!(matches!(source, Source::Decoded { .. }));
        assert!(load(&entry_path, size).is_some(), "the entry was healed");

        let stale = Entry {
            version: ANALYSIS_VERSION + 1,
            size,
            results: BTreeMap::new(),
        };
        fs::write(&entry_path, serde_json::to_vec(&stale).unwrap()).unwrap();
        let (_, source) = cache.measure_whole_file(&file).unwrap();
        assert!(matches!(source, Source::Decoded { .. }));
    }

    #[test]
    fn silence_survives_the_cache() {
        let audio = tempdir().unwrap();
        let store = tempdir().unwrap();
        let file = audio.path().join("silent.wav");
        write_wav(&file, 3, 0);
        let cache = Cache::new(store.path());

        let (first, _) = cache.measure_whole_file(&file).unwrap();
        let (second, source) = cache.measure_whole_file(&file).unwrap();
        assert_eq!(source, Source::Cache);
        assert!(first.tracks[0].integrated_lufs.is_infinite());
        assert_eq!(first, second);
    }

    #[test]
    fn an_unwritable_cache_does_not_fail_the_measurement() {
        let audio = tempdir().unwrap();
        let store = tempdir().unwrap();
        let file = audio.path().join("a.wav");
        write_wav(&file, 3, 200);
        // A file where the cache directory should be makes saving impossible.
        let blocker = store.path().join("blocked");
        fs::write(&blocker, b"not a directory").unwrap();
        let cache = Cache::new(&blocker);

        let (measured, source) = cache.measure_whole_file(&file).unwrap();
        assert_eq!(measured.tracks.len(), 1);
        assert!(matches!(
            source,
            Source::Decoded {
                save_error: Some(_)
            }
        ));
    }

    #[test]
    fn spans_keys_distinguish_splits() {
        let sheet = cue::parse(
            "FILE \"a.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:33\nTRACK 02 AUDIO\nINDEX 01 05:30:30\n",
        )
        .unwrap();
        assert_eq!(spans_key(&sheet.files[0].segments()), "33-24780,24780-");
    }
}
