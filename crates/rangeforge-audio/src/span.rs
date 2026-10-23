//! Cutting a stream of audio into tracks.
//!
//! Audio arrives in blocks of arbitrary size, and track boundaries rarely fall on block
//! boundaries. [`Splitter`] takes each block and reports which part belongs to which track, so
//! that measuring and exporting cut the audio in exactly the same places.

use crate::AudioError;

/// A range of sample frames. `end` is exclusive; `None` means the end of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u64,
    pub end: Option<u64>,
}

/// What happened while splitting a block.
#[derive(Debug, Clone, Copy)]
pub enum Event<'a> {
    /// The track with this index begins.
    Start(usize),
    /// Interleaved samples belonging to the track with this index.
    Samples(usize, &'a [f32]),
    /// The track with this index is complete.
    End(usize),
}

/// Walks a list of spans in order as audio streams past.
#[derive(Debug)]
pub struct Splitter {
    spans: Vec<Span>,
    channels: usize,
    /// The span being filled.
    index: usize,
    started: bool,
    /// Sample frames seen so far.
    position: u64,
}

impl Splitter {
    pub fn new(spans: Vec<Span>, channels: usize) -> Self {
        Self {
            spans,
            channels: channels.max(1),
            index: 0,
            started: false,
            position: 0,
        }
    }

    /// Sample frames seen so far, including audio outside every span.
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Splits one block of interleaved samples, calling `sink` for each event in order.
    ///
    /// Audio before a span starts belongs to no track (for example the first-track offset in
    /// a CUE sheet) and is skipped.
    pub fn split<E>(
        &mut self,
        samples: &[f32],
        mut sink: impl FnMut(Event<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        let frames = samples.len() / self.channels;
        let mut offset = 0_usize;

        while self.index < self.spans.len() {
            let span = self.spans[self.index];
            let remaining = (frames - offset) as u64;

            if !self.started {
                if self.position < span.start {
                    let skip = (span.start - self.position).min(remaining);
                    self.position += skip;
                    offset += usize::try_from(skip).unwrap_or(frames);
                    if self.position < span.start {
                        break;
                    }
                }
                sink(Event::Start(self.index))?;
                self.started = true;
            }

            let remaining = (frames - offset) as u64;
            let take = span.end.map_or(remaining, |end| {
                end.saturating_sub(self.position).min(remaining)
            });
            if take > 0 {
                let count = usize::try_from(take).unwrap_or(frames);
                let chunk = &samples[offset * self.channels..(offset + count) * self.channels];
                sink(Event::Samples(self.index, chunk))?;
                self.position += take;
                offset += count;
            }

            if span.end.is_some_and(|end| self.position >= end) {
                sink(Event::End(self.index))?;
                self.index += 1;
                self.started = false;
            } else {
                break;
            }
        }

        // Audio after the last span still counts towards the position.
        self.position += (frames - offset) as u64;
        Ok(())
    }

    /// Called when the stream ends. A last span with no end runs to the end of the file, so it
    /// completes here. Any other unfinished span means the audio was shorter than expected.
    pub fn finish<E: From<AudioError>>(
        &mut self,
        mut sink: impl FnMut(Event<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        if self.index < self.spans.len() && self.started && self.spans[self.index].end.is_none() {
            sink(Event::End(self.index))?;
            self.index += 1;
            self.started = false;
        }
        if self.index < self.spans.len() {
            return Err(AudioError::EndedEarly {
                missing: self.spans.len() - self.index,
            }
            .into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `blocks` (mono, one value per frame) through a splitter and records what each
    /// track received.
    fn collect(spans: Vec<Span>, blocks: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, AudioError> {
        let count = spans.len();
        let mut splitter = Splitter::new(spans, 1);
        let mut tracks: Vec<Vec<f32>> = vec![Vec::new(); count];
        let mut log: Vec<String> = Vec::new();
        for block in blocks {
            splitter.split(block, |event| {
                match event {
                    Event::Start(i) => log.push(format!("start {i}")),
                    Event::Samples(i, data) => tracks[i].extend_from_slice(data),
                    Event::End(i) => log.push(format!("end {i}")),
                }
                Ok::<(), AudioError>(())
            })?;
        }
        splitter.finish(|event| {
            if let Event::End(i) = event {
                log.push(format!("end {i}"));
            }
            Ok::<(), AudioError>(())
        })?;
        // Every track must have started and ended exactly once, in order.
        for i in 0..count {
            let starts = log.iter().filter(|l| **l == format!("start {i}")).count();
            let ends = log.iter().filter(|l| **l == format!("end {i}")).count();
            assert_eq!((starts, ends), (1, 1), "track {i}: {log:?}");
        }
        Ok(tracks)
    }

    fn ramp(from: u32, to: u32) -> Vec<f32> {
        (from..to)
            .map(|n| f32::from(u16::try_from(n).unwrap()))
            .collect()
    }

    fn span(start: u64, end: Option<u64>) -> Span {
        Span { start, end }
    }

    #[test]
    fn splits_at_boundaries_whatever_the_block_size() {
        let spans = vec![span(0, Some(10)), span(10, Some(25)), span(25, None)];
        let expected = vec![ramp(0, 10), ramp(10, 25), ramp(25, 40)];

        // The same audio in blocks of every awkward size must produce the same tracks.
        for size in [1_u32, 3, 7, 10, 11, 40, 100] {
            let blocks: Vec<Vec<f32>> = (0..40)
                .step_by(size as usize)
                .map(|from| ramp(from, (from + size).min(40)))
                .collect();
            assert_eq!(
                collect(spans.clone(), &blocks).unwrap(),
                expected,
                "size {size}"
            );
        }
    }

    #[test]
    fn skips_audio_before_the_first_track() {
        let spans = vec![span(4, Some(10)), span(10, None)];
        let blocks = vec![ramp(0, 7), ramp(7, 20)];
        let tracks = collect(spans, &blocks).unwrap();
        assert_eq!(tracks[0], ramp(4, 10));
        assert_eq!(tracks[1], ramp(10, 20));
    }

    #[test]
    fn leaves_gaps_between_tracks_out() {
        let spans = vec![span(0, Some(5)), span(8, Some(12))];
        let tracks = collect(spans, &[ramp(0, 15)]).unwrap();
        assert_eq!(tracks[0], ramp(0, 5));
        assert_eq!(tracks[1], ramp(8, 12));
    }

    #[test]
    fn a_stream_that_ends_early_is_an_error() {
        let result = collect(vec![span(0, Some(10)), span(10, Some(30))], &[ramp(0, 20)]);
        assert!(matches!(result, Err(AudioError::EndedEarly { missing: 1 })));
    }

    #[test]
    fn a_last_track_starting_past_the_end_is_an_error() {
        let result = collect(vec![span(0, Some(10)), span(50, None)], &[ramp(0, 20)]);
        assert!(matches!(result, Err(AudioError::EndedEarly { missing: 1 })));
    }

    #[test]
    fn counts_every_frame_including_ignored_ones() {
        let mut splitter = Splitter::new(vec![span(2, Some(4))], 1);
        for block in [ramp(0, 3), ramp(3, 9)] {
            splitter
                .split(&block, |_| Ok::<(), AudioError>(()))
                .unwrap();
        }
        assert_eq!(splitter.position(), 9);
    }

    #[test]
    fn stereo_frames_stay_together() {
        // Three stereo frames: (0,1) (2,3) (4,5). The track covers frames 1 and 2.
        let mut splitter = Splitter::new(vec![span(1, Some(3))], 2);
        let mut got: Vec<f32> = Vec::new();
        splitter
            .split(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0], |event| {
                if let Event::Samples(_, data) = event {
                    got.extend_from_slice(data);
                }
                Ok::<(), AudioError>(())
            })
            .unwrap();
        assert_eq!(got, vec![2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn a_zero_length_track_starts_and_ends() {
        let tracks = collect(vec![span(5, Some(5)), span(5, None)], &[ramp(0, 10)]).unwrap();
        assert_eq!(tracks[0], Vec::<f32>::new());
        assert_eq!(tracks[1], ramp(5, 10));
    }
}
