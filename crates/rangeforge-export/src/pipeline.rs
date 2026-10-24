//! Exporting one audio file: decode once, apply each track's gain, encode, then place the
//! results.
//!
//! The order is deliberate. Every track is encoded to a temporary file first. Only when the
//! whole source has been read and every encode has succeeded are the temporary files moved
//! into place under the chosen [`Strategy`]. If anything fails before that, the temporary
//! files are deleted and the original files have not been touched.

use std::fs;
use std::path::{Path, PathBuf};

use rangeforge_audio::decode::{StreamInfo, for_each_block};
use rangeforge_audio::measure::spans_for;
use rangeforge_audio::span::{Event, Span, Splitter};
use rangeforge_core::cue::Segment;

use crate::ExportError;
use crate::encode::{EncodeSpec, Encoder};
use crate::ffmpeg::Ffmpeg;
use crate::format::OutputFormat;
use crate::strategy::{Resolved, Strategy, resolve};
use crate::tags::Tags;

/// What to do with one track.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackPlan {
    /// The volume change in dB. Negative is quieter.
    pub gain_db: f64,
    /// The output file's name, before the File Strategy adjusts it.
    pub file_name: String,
    pub tags: Tags,
}

/// How to write the files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub format: OutputFormat,
    pub strategy: Strategy,
    /// The folder files go in unless the strategy names another.
    pub directory: PathBuf,
    /// Copy the source file's own tags first. Right for single files, which carry their own
    /// tags; wrong for a CUE image, whose tags describe the whole album.
    pub copy_source_tags: bool,
}

/// One finished output file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub index: usize,
    pub path: PathBuf,
    /// An existing file at `path` was replaced.
    pub overwrote: bool,
    /// An existing file was renamed to this to make room.
    pub renamed_existing_to: Option<PathBuf>,
}

/// Progress, for showing to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Encoding of track `index` (counting from 0) of `total` has begun.
    Encoding { index: usize, total: usize },
    /// Track `index` has been placed at `path`.
    Saved { index: usize, path: PathBuf },
}

/// Where each track would be written, without writing anything.
pub fn preview(options: &Options, tracks: &[TrackPlan]) -> Result<Vec<Resolved>, ExportError> {
    tracks
        .iter()
        .map(|track| resolve(&options.strategy, &options.directory, &track.file_name))
        .collect()
}

/// Exports one audio file. With `segments`, each CUE track is cut out and written; without,
/// the whole file is written as a single track.
pub fn export_file(
    ffmpeg: &Ffmpeg,
    input: &Path,
    segments: Option<&[Segment<'_>]>,
    tracks: &[TrackPlan],
    options: &Options,
    on_progress: &mut dyn FnMut(&Progress),
) -> Result<Vec<Written>, ExportError> {
    let expected = segments.map_or(1, <[Segment<'_>]>::len);
    if tracks.len() != expected {
        return Err(ExportError::TrackCountMismatch {
            expected: tracks.len(),
            found: expected,
        });
    }

    let mut run = Run {
        ffmpeg,
        input,
        segments,
        tracks,
        options,
        on_progress,
        splitter: None,
        info: None,
        encoder: None,
        current_temp: None,
        staged: Vec::new(),
    };

    let encoded = for_each_block::<ExportError, _>(input, |info, samples| run.feed(info, samples))
        .and_then(|_| run.finish_stream());
    if let Err(error) = encoded {
        run.discard_staged();
        return Err(error);
    }
    run.place_files()
}

struct Staged {
    index: usize,
    temp: PathBuf,
}

/// The state of one export while the audio streams through.
struct Run<'a> {
    ffmpeg: &'a Ffmpeg,
    input: &'a Path,
    segments: Option<&'a [Segment<'a>]>,
    tracks: &'a [TrackPlan],
    options: &'a Options,
    on_progress: &'a mut dyn FnMut(&Progress),
    splitter: Option<Splitter>,
    info: Option<StreamInfo>,
    encoder: Option<Encoder>,
    current_temp: Option<PathBuf>,
    staged: Vec<Staged>,
}

impl Run<'_> {
    fn feed(&mut self, info: &StreamInfo, samples: &[f32]) -> Result<(), ExportError> {
        if self.splitter.is_none() {
            let spans = match self.segments {
                Some(segments) => spans_for(segments, info.sample_rate),
                None => vec![Span {
                    start: 0,
                    end: None,
                }],
            };
            self.splitter = Some(Splitter::new(spans, info.channels));
            self.info = Some(*info);
        }
        // The splitter is lent out for the call, because handling an event needs `self`.
        let Some(mut splitter) = self.splitter.take() else {
            return Ok(());
        };
        let result = splitter.split(samples, |event| self.on_event(event));
        self.splitter = Some(splitter);
        result
    }

    fn finish_stream(&mut self) -> Result<(), ExportError> {
        let Some(mut splitter) = self.splitter.take() else {
            return Err(rangeforge_audio::AudioError::NoAudioStream.into());
        };
        let result = splitter.finish(|event| self.on_event(event));
        self.splitter = Some(splitter);
        result
    }

    fn on_event(&mut self, event: Event<'_>) -> Result<(), ExportError> {
        match event {
            Event::Start(index) => self.start_track(index),
            Event::Samples(_, samples) => match self.encoder.as_mut() {
                Some(encoder) => encoder.write(samples),
                None => Ok(()),
            },
            Event::End(index) => self.end_track(index),
        }
    }

    /// Where a track is written while it is being encoded: next to its final home, so that
    /// moving it into place is a rename and not a copy.
    fn temp_path(&self, plan: &TrackPlan) -> PathBuf {
        self.options
            .strategy
            .directory(&self.options.directory)
            .join(format!(".{}.rangeforge-part", plan.file_name))
    }

    fn start_track(&mut self, index: usize) -> Result<(), ExportError> {
        let (Some(info), Some(plan)) = (self.info, self.tracks.get(index)) else {
            return Ok(());
        };
        let temp = self.temp_path(plan);
        if let Some(parent) = temp.parent() {
            fs::create_dir_all(parent).map_err(ExportError::io("creating", parent))?;
        }
        let spec = EncodeSpec {
            info,
            format: self.options.format,
            gain_db: plan.gain_db,
            output: &temp,
            tags: &plan.tags,
            metadata_from: self.options.copy_source_tags.then_some(self.input),
        };
        self.encoder = Some(Encoder::start(self.ffmpeg, index, &spec)?);
        self.current_temp = Some(temp);
        (self.on_progress)(&Progress::Encoding {
            index,
            total: self.tracks.len(),
        });
        Ok(())
    }

    fn end_track(&mut self, index: usize) -> Result<(), ExportError> {
        if let (Some(encoder), Some(temp)) = (self.encoder.take(), self.current_temp.take()) {
            encoder.finish()?;
            self.staged.push(Staged { index, temp });
        }
        Ok(())
    }

    /// Deletes every temporary file. Used when something went wrong.
    fn discard_staged(&mut self) {
        self.encoder = None;
        if let Some(temp) = self.current_temp.take() {
            let _ = fs::remove_file(temp);
        }
        for staged in self.staged.drain(..) {
            let _ = fs::remove_file(staged.temp);
        }
    }

    /// Moves the finished temporary files into place, following the File Strategy.
    fn place_files(&mut self) -> Result<Vec<Written>, ExportError> {
        let staged = std::mem::take(&mut self.staged);
        let mut written = Vec::with_capacity(staged.len());
        for (position, item) in staged.iter().enumerate() {
            match self.place(item) {
                Ok(done) => {
                    (self.on_progress)(&Progress::Saved {
                        index: done.index,
                        path: done.path.clone(),
                    });
                    written.push(done);
                }
                Err(error) => {
                    // Files already placed stay; everything not yet placed is cleaned up.
                    for left in &staged[position..] {
                        let _ = fs::remove_file(&left.temp);
                    }
                    return Err(error);
                }
            }
        }
        Ok(written)
    }

    fn place(&self, item: &Staged) -> Result<Written, ExportError> {
        let plan = &self.tracks[item.index];
        let resolved = resolve(
            &self.options.strategy,
            &self.options.directory,
            &plan.file_name,
        )?;
        if let Some(old) = &resolved.move_existing_to {
            fs::rename(&resolved.final_path, old)
                .map_err(ExportError::io("renaming", &resolved.final_path))?;
        }
        if let Err(source) = fs::rename(&item.temp, &resolved.final_path) {
            // Put a renamed original back rather than leave it under the wrong name.
            if let Some(old) = &resolved.move_existing_to {
                let _ = fs::rename(old, &resolved.final_path);
            }
            return Err(ExportError::Io {
                action: "placing",
                path: resolved.final_path,
                source,
            });
        }
        Ok(Written {
            index: item.index,
            path: resolved.final_path,
            overwrote: resolved.overwrites,
            renamed_existing_to: resolved.move_existing_to,
        })
    }
}
