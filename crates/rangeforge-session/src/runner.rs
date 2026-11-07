//! A worker thread that writes the files for a run, so the screen stays responsive.
//!
//! Files are written one source at a time. Each song inside a source is encoded by its own
//! FFmpeg process, fed from a single pass over the source.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use rangeforge_core::cue::CueFile;
use rangeforge_export::ffmpeg::Ffmpeg;
use rangeforge_export::pipeline::{Options, Progress, export_file};

use crate::model::{RunEvent, RunItem, RunSpec};

/// Starts writing the files in `spec` in the background. Progress arrives on the returned
/// receiver, ending with [`RunEvent::AllDone`].
pub fn start(spec: RunSpec, ffmpeg: Ffmpeg) -> Receiver<RunEvent> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for item in &spec.items {
            let outcome = write_item(item, &spec, &ffmpeg, &sender);
            let event = match outcome {
                Ok(()) => RunEvent::Finished { group: item.group },
                Err(message) => RunEvent::Failed {
                    group: item.group,
                    message,
                },
            };
            if sender.send(event).is_err() {
                return;
            }
        }
        let _ = sender.send(RunEvent::AllDone);
    });
    receiver
}

fn write_item(
    item: &RunItem,
    spec: &RunSpec,
    ffmpeg: &Ffmpeg,
    sender: &Sender<RunEvent>,
) -> Result<(), String> {
    let options = Options {
        format: item.format,
        strategy: spec.strategy.clone(),
        directory: item.directory.clone(),
        // A single file carries its own tags. A CUE image's tags describe the whole album.
        copy_source_tags: item.job.cue_file.is_none(),
    };
    let segments = item.job.cue_file.as_ref().map(CueFile::segments);
    export_file(
        ffmpeg,
        &item.job.audio,
        segments.as_deref(),
        &item.plans,
        &options,
        &mut |progress| {
            // The window may have been closed; if so there is nobody to tell.
            let _ = match progress {
                Progress::Encoding { index, total } => sender.send(RunEvent::Encoding {
                    group: item.group,
                    index: *index,
                    total: *total,
                }),
                Progress::Saved { index, path } => sender.send(RunEvent::Saved {
                    group: item.group,
                    index: *index,
                    path: path.clone(),
                }),
                Progress::RemovedOriginal { path } => sender.send(RunEvent::RemovedOriginal {
                    group: item.group,
                    path: path.clone(),
                }),
            };
        },
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    use rangeforge_export::format::{BitDepth, Codec, OutputFormat};
    use rangeforge_export::pipeline::TrackPlan;
    use rangeforge_export::strategy::Strategy;
    use rangeforge_export::tags::Tags;
    use tempfile::tempdir;

    use crate::load::Job;

    fn ffmpeg() -> Option<Ffmpeg> {
        match Ffmpeg::locate() {
            Ok(ffmpeg) => Some(ffmpeg),
            Err(error) => {
                // On a build server FFmpeg is installed on purpose, so a missing one is a
                // failure there and not a reason to skip.
                assert!(
                    std::env::var_os("CI").is_none(),
                    "FFmpeg is required in CI: {error}"
                );
                eprintln!("skipping: {error}");
                None
            }
        }
    }

    /// A one-second stereo 16-bit WAV of a repeating ramp.
    fn write_wav(path: &Path) {
        let rate: u32 = 44_100;
        let data_len = rate * 4;
        let mut bytes = Vec::new();
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
        for frame in 0..rate {
            let ramp = i16::try_from(i32::try_from(frame % 200).unwrap() * 100 - 10_000).unwrap();
            bytes.extend_from_slice(&ramp.to_le_bytes());
            bytes.extend_from_slice(&ramp.to_le_bytes());
        }
        fs::write(path, bytes).unwrap();
    }

    fn item(group: usize, audio: PathBuf, file_name: &str) -> RunItem {
        let directory = audio.parent().unwrap().to_path_buf();
        RunItem {
            group,
            job: Job {
                group,
                audio,
                cue_file: None,
                sheet: None,
            },
            plans: vec![TrackPlan {
                gain_db: -3.0,
                file_name: file_name.to_string(),
                tags: Tags::default(),
                skip: false,
            }],
            directory,
            format: OutputFormat::new(Codec::Alac, BitDepth::Source),
        }
    }

    fn collect(receiver: &Receiver<RunEvent>) -> Vec<RunEvent> {
        let mut events = Vec::new();
        while let Ok(event) = receiver.recv_timeout(std::time::Duration::from_secs(60)) {
            let done = event == RunEvent::AllDone;
            events.push(event);
            if done {
                break;
            }
        }
        events
    }

    #[test]
    fn a_run_reports_each_step_and_writes_the_files() {
        let Some(ffmpeg) = ffmpeg() else { return };
        let dir = tempdir().unwrap();
        let first = dir.path().join("one.wav");
        let second = dir.path().join("two.wav");
        write_wav(&first);
        write_wav(&second);
        let spec = RunSpec {
            items: vec![item(0, first, "one.m4a"), item(1, second, "two.m4a")],
            format: OutputFormat::new(Codec::Alac, BitDepth::Source),
            strategy: Strategy::safe(),
        };

        let events = collect(&start(spec, ffmpeg));

        let expected = |group: usize, name: &str| {
            [
                RunEvent::Encoding {
                    group,
                    index: 0,
                    total: 1,
                },
                RunEvent::Saved {
                    group,
                    index: 0,
                    path: dir.path().join(name),
                },
                RunEvent::Finished { group },
            ]
        };
        let mut all = Vec::new();
        all.extend(expected(0, "one.m4a"));
        all.extend(expected(1, "two.m4a"));
        all.push(RunEvent::AllDone);
        assert_eq!(events, all);

        assert!(dir.path().join("one.m4a").exists());
        assert!(dir.path().join("two.m4a").exists());
        assert!(
            dir.path().join("one.wav").exists(),
            "the sources are untouched"
        );
    }

    #[test]
    fn one_failure_does_not_stop_the_rest() {
        let Some(ffmpeg) = ffmpeg() else { return };
        let dir = tempdir().unwrap();
        let bad = dir.path().join("bad.wav");
        let good = dir.path().join("good.wav");
        fs::write(&bad, b"not audio").unwrap();
        write_wav(&good);
        let spec = RunSpec {
            items: vec![item(0, bad, "bad.m4a"), item(1, good, "good.m4a")],
            format: OutputFormat::new(Codec::Alac, BitDepth::Source),
            strategy: Strategy::safe(),
        };

        let events = collect(&start(spec, ffmpeg));

        assert!(
            events
                .iter()
                .any(|e| matches!(e, RunEvent::Failed { group: 0, .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, RunEvent::Finished { group: 1 }))
        );
        assert_eq!(events.last(), Some(&RunEvent::AllDone));
        assert!(dir.path().join("good.m4a").exists());
        assert!(!dir.path().join("bad.m4a").exists());
    }
}
