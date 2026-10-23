//! Loudness measurement: decode once, measure each track.

use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;

use ebur128::{EbuR128, Mode};
use rangeforge_core::cue::Segment;
use rangeforge_core::planner::Measurement;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::AudioError;

/// What a pass over a file produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    /// One entry per requested track, in order.
    pub tracks: Vec<Measurement>,
    pub sample_rate: u32,
    pub channels: usize,
    /// Every sample frame decoded, including audio outside any track.
    pub total_sample_frames: u64,
}

impl Measured {
    /// Length of the decoded audio in seconds.
    pub fn duration_seconds(&self) -> f64 {
        // Precision loss only matters beyond 2^53 sample frames, far past any real file.
        #[allow(clippy::cast_precision_loss)]
        let frames = self.total_sample_frames as f64;
        frames / f64::from(self.sample_rate)
    }
}

/// Measures a whole file as one track.
pub fn measure_whole_file(path: &Path) -> Result<Measured, AudioError> {
    run(path, |_| {
        vec![Span {
            start: 0,
            end: None,
        }]
    })
}

/// Measures each CUE segment of one audio file. The segments' last track runs to the end of
/// the file.
pub fn measure_segments(path: &Path, segments: &[Segment<'_>]) -> Result<Measured, AudioError> {
    run(path, |rate| {
        segments
            .iter()
            .map(|segment| Span {
                start: segment.start.to_sample_frames(rate),
                end: segment.end.map(|end| end.to_sample_frames(rate)),
            })
            .collect()
    })
}

/// A range of sample frames. `end` is exclusive; `None` means the end of the file.
#[derive(Debug, Clone, Copy)]
struct Span {
    start: u64,
    end: Option<u64>,
}

fn run(path: &Path, spans_for_rate: impl FnOnce(u32) -> Vec<Span>) -> Result<Measured, AudioError> {
    let file = File::open(path).map_err(|source| AudioError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    let stream = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe().format(
        &hint,
        stream,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(AudioError::NoAudioStream)?;
    let track_id = track.id;
    let params = track.codec_params.clone();

    let mut decoder = symphonia::default::get_codecs().make(&params, &DecoderOptions::default())?;
    let mut spans_for_rate = Some(spans_for_rate);
    // The meters are built from the first decoded block, because not every format states
    // its sample rate and channel layout up front (ALAC inside MP4, for one).
    let mut meters: Option<Meters> = None;
    let mut buffer: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            // Symphonia reports the end of the stream as an unexpected end of file.
            Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                break;
            }
            Err(error) => return Err(error.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        // A corrupt packet is an error, not something to skip: skipping would quietly
        // change the measured loudness.
        let block = decoder.decode(&packet)?;
        let spec = *block.spec();
        if meters.is_none() {
            let make_spans = spans_for_rate.take().ok_or(AudioError::NoAudioStream)?;
            meters = Some(Meters::new(
                spec.channels.count(),
                spec.rate,
                make_spans(spec.rate),
            )?);
        }
        let Some(active) = meters.as_mut() else {
            continue;
        };
        let samples =
            buffer.get_or_insert_with(|| SampleBuffer::<f32>::new(block.capacity() as u64, spec));
        samples.copy_interleaved_ref(block);
        active.feed(samples.samples())?;
    }

    // No decoded audio at all means there is nothing to measure.
    let meters = meters.ok_or(AudioError::NoAudioStream)?;
    let (sample_rate, channels) = (meters.sample_rate, meters.channels);
    let (tracks, total_sample_frames) = meters.finish()?;
    Ok(Measured {
        tracks,
        sample_rate,
        channels,
        total_sample_frames,
    })
}

/// One loudness meter per track, fed in order as the audio streams past.
struct Meters {
    channels: usize,
    sample_rate: u32,
    spans: Vec<Span>,
    /// Which span is being measured.
    index: usize,
    /// Sample frames consumed so far.
    position: u64,
    meter: Option<EbuR128>,
    results: Vec<Measurement>,
}

impl Meters {
    fn new(channels: usize, sample_rate: u32, spans: Vec<Span>) -> Result<Self, AudioError> {
        let mut meters = Self {
            channels,
            sample_rate,
            spans,
            index: 0,
            position: 0,
            meter: None,
            results: Vec::new(),
        };
        meters.open_current()?;
        Ok(meters)
    }

    fn open_current(&mut self) -> Result<(), AudioError> {
        if self.index < self.spans.len() {
            let channels = u32::try_from(self.channels).unwrap_or(u32::MAX);
            self.meter = Some(EbuR128::new(
                channels,
                self.sample_rate,
                Mode::I | Mode::LRA | Mode::TRUE_PEAK,
            )?);
        }
        Ok(())
    }

    fn close_current(&mut self) -> Result<(), AudioError> {
        if let Some(meter) = self.meter.take() {
            let mut peak = 0.0_f64;
            for channel in 0..u32::try_from(self.channels).unwrap_or(u32::MAX) {
                peak = peak.max(meter.true_peak(channel)?);
            }
            self.results.push(Measurement {
                integrated_lufs: meter.loudness_global()?,
                true_peak_dbtp: 20.0 * peak.log10(),
                loudness_range_lu: meter.loudness_range()?,
            });
        }
        Ok(())
    }

    /// Feeds interleaved samples, splitting them at track boundaries.
    fn feed(&mut self, samples: &[f32]) -> Result<(), AudioError> {
        let frames = samples.len() / self.channels;
        let mut offset = 0_usize;

        while offset < frames && self.index < self.spans.len() {
            let span = self.spans[self.index];
            let available = (frames - offset) as u64;

            // Audio before this track starts belongs to no track (for example a first-track
            // offset in the CUE sheet), so it is skipped.
            if self.position < span.start {
                let skip = (span.start - self.position).min(available);
                self.position += skip;
                offset += usize::try_from(skip).unwrap_or(frames);
                continue;
            }

            let take = span.end.map_or(available, |end| {
                end.saturating_sub(self.position).min(available)
            });
            if take > 0 {
                let count = usize::try_from(take).unwrap_or(frames);
                let chunk = &samples[offset * self.channels..(offset + count) * self.channels];
                if let Some(meter) = self.meter.as_mut() {
                    meter.add_frames_f32(chunk)?;
                }
                self.position += take;
                offset += count;
            }

            if span.end.is_some_and(|end| self.position >= end) {
                self.close_current()?;
                self.index += 1;
                self.open_current()?;
            }
        }

        // Audio after the last track still counts towards the position and total.
        self.position += (frames - offset) as u64;
        Ok(())
    }

    fn finish(mut self) -> Result<(Vec<Measurement>, u64), AudioError> {
        // A final span without an end runs to the end of the file.
        if self.index < self.spans.len() && self.spans[self.index].end.is_none() {
            self.close_current()?;
            self.index += 1;
        }
        if self.index < self.spans.len() {
            return Err(AudioError::EndedEarly {
                missing: self.spans.len() - self.index,
            });
        }
        Ok((self.results, self.position))
    }
}
