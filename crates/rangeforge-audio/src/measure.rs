//! Loudness measurement: decode once, measure each track.

use std::path::Path;

use ebur128::{EbuR128, Mode};
use rangeforge_core::cue::Segment;
use rangeforge_core::mix::MixPlan;
use rangeforge_core::planner::Measurement;

use crate::AudioError;
use crate::decode::{StreamInfo, for_each_block_mixed};
use crate::dsd;
use crate::span::{Event, Span, Splitter};

/// What a pass over a file produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    /// One entry per requested track, in order.
    pub tracks: Vec<Measurement>,
    pub sample_rate: u32,
    /// The channels that were measured: after any mix, so the file's own count when none.
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

/// Measures a whole file as one track, with its channels mixed as `mix` says first. Loudness and
/// peak are those of the mixed sound, which is what would be written.
pub fn measure_whole_file(path: &Path, mix: MixPlan) -> Result<Measured, AudioError> {
    run(path, mix, |_| {
        vec![Span {
            start: 0,
            end: None,
        }]
    })
}

/// Measures each CUE segment of one audio file, with its channels mixed as `mix` says first.
/// The segments' last track runs to the end of the file.
pub fn measure_segments(
    path: &Path,
    segments: &[Segment<'_>],
    mix: MixPlan,
) -> Result<Measured, AudioError> {
    run(path, mix, |rate| spans_for(segments, rate))
}

/// Converts CUE segments to sample ranges at `sample_rate`.
pub fn spans_for(segments: &[Segment<'_>], sample_rate: u32) -> Vec<Span> {
    segments
        .iter()
        .map(|segment| Span {
            start: segment.start.to_sample_frames(sample_rate),
            end: segment.end.map(|end| end.to_sample_frames(sample_rate)),
        })
        .collect()
}

fn run(
    path: &Path,
    mix: MixPlan,
    spans_for_rate: impl FnOnce(u32) -> Vec<Span>,
) -> Result<Measured, AudioError> {
    let mut spans_for_rate = Some(spans_for_rate);
    let mut meters: Option<Meters> = None;

    let info = for_each_block_mixed(
        path,
        dsd::ANALYSIS_RATE,
        mix,
        |info: &StreamInfo, samples: &[f32]| {
            if meters.is_none() {
                let make_spans = spans_for_rate.take().ok_or(AudioError::NoAudioStream)?;
                meters = Some(Meters::new(info, make_spans(info.sample_rate)));
            }
            match meters.as_mut() {
                Some(active) => active.feed(samples),
                None => Ok(()),
            }
        },
    )?;

    let meters = meters.ok_or(AudioError::NoAudioStream)?;
    let (tracks, total_sample_frames) = meters.finish()?;
    Ok(Measured {
        tracks,
        sample_rate: info.sample_rate,
        channels: info.channels,
        total_sample_frames,
    })
}

/// One loudness meter per track, fed in order as the audio streams past.
struct Meters {
    channels: u32,
    sample_rate: u32,
    splitter: Splitter,
    meter: Option<EbuR128>,
    results: Vec<Measurement>,
}

impl Meters {
    fn new(info: &StreamInfo, spans: Vec<Span>) -> Self {
        Self {
            channels: u32::try_from(info.channels).unwrap_or(u32::MAX),
            sample_rate: info.sample_rate,
            splitter: Splitter::new(spans, info.channels),
            meter: None,
            results: Vec::new(),
        }
    }

    fn feed(&mut self, samples: &[f32]) -> Result<(), AudioError> {
        let (channels, sample_rate) = (self.channels, self.sample_rate);
        let (meter, results) = (&mut self.meter, &mut self.results);
        self.splitter.split(samples, |event| {
            handle(event, channels, sample_rate, meter, results)
        })
    }

    fn finish(mut self) -> Result<(Vec<Measurement>, u64), AudioError> {
        let (channels, sample_rate) = (self.channels, self.sample_rate);
        let (meter, results) = (&mut self.meter, &mut self.results);
        self.splitter
            .finish(|event| handle(event, channels, sample_rate, meter, results))?;
        Ok((self.results, self.splitter.position()))
    }
}

/// Reacts to one track event: open a meter, feed it, or close it into a result.
fn handle(
    event: Event<'_>,
    channels: u32,
    sample_rate: u32,
    meter: &mut Option<EbuR128>,
    results: &mut Vec<Measurement>,
) -> Result<(), AudioError> {
    match event {
        Event::Start(_) => {
            *meter = Some(EbuR128::new(
                channels,
                sample_rate,
                Mode::I | Mode::LRA | Mode::TRUE_PEAK,
            )?);
        }
        Event::Samples(_, samples) => {
            if let Some(meter) = meter.as_mut() {
                meter.add_frames_f32(samples)?;
            }
        }
        Event::End(_) => {
            if let Some(meter) = meter.take() {
                let mut peak = 0.0_f64;
                for channel in 0..channels {
                    peak = peak.max(meter.true_peak(channel)?);
                }
                results.push(Measurement {
                    integrated_lufs: meter.loudness_global()?,
                    true_peak_dbtp: 20.0 * peak.log10(),
                    loudness_range_lu: meter.loudness_range()?,
                });
            }
        }
    }
    Ok(())
}
