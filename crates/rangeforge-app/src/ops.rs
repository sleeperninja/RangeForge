//! What the window can do, as plain functions.
//!
//! Each function changes the shared screen state and returns the new state for the window to
//! display. Nothing here knows about Tauri, so all of it can be tested directly. The thin
//! wrappers in `main.rs` turn these into commands and forward progress to the window.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use rangeforge_audio::cache::Cache;
use rangeforge_export::format::{BitDepth, Codec};
use rangeforge_session::controls::PresetList;
use rangeforge_session::load::Job;
use rangeforge_session::model::{App, MeasureOutcome, RunEvent, RunRequest, RunSpec, Settings};
use rangeforge_session::presets;
use rangeforge_session::snapshot::{Snapshot, strategy_from_id};
use serde::Serialize;

/// The screen state, shared between the window's commands and the background threads.
pub struct Shared {
    app: Mutex<App>,
    presets_path: Option<PathBuf>,
    cache: Option<Cache>,
}

impl Shared {
    /// Starts with an empty list and the saved presets (or the built-in ones).
    pub fn new(presets_path: Option<PathBuf>, cache: Option<Cache>) -> Self {
        let mut app = App::new(Vec::new(), Vec::new(), Vec::new(), Settings::default());
        let stored = presets_path.as_deref().map_or_else(
            || presets::Loaded {
                presets: presets::built_in(),
                notes: Vec::new(),
            },
            presets::load,
        );
        for note in stored.notes {
            app.log(note);
        }
        app.presets = PresetList::new(stored.presets);
        app.presets.select_matching(&app.settings, app.output);
        Self {
            app: Mutex::new(app),
            presets_path,
            cache,
        }
    }

    /// Where measurements are remembered.
    pub fn cache(&self) -> Option<&Cache> {
        self.cache.as_ref()
    }

    fn lock(&self) -> MutexGuard<'_, App> {
        // A thread that panicked while holding the lock leaves the state readable.
        self.app.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The answer to nearly every command: the new screen, and a message if something was
/// refused.
#[derive(Debug, Clone, Serialize)]
pub struct Reply {
    pub state: Snapshot,
    pub error: Option<String>,
}

/// The answer to pressing Run.
#[derive(Debug, Clone, Serialize)]
pub struct RunReply {
    /// `started`, `needs_confirmation` or `refused`.
    pub outcome: &'static str,
    pub message: Option<String>,
    pub state: Snapshot,
}

/// Saves the presets if they changed, then describes the screen.
fn reply(shared: &Shared, app: &mut App, error: Option<String>) -> Reply {
    if app.take_presets_changed()
        && let Some(path) = &shared.presets_path
        && let Err(problem) = presets::save(path, &app.presets.items)
    {
        app.log(format!("Could not save presets: {problem:#}"));
    }
    Reply {
        state: app.snapshot(),
        error,
    }
}

fn with_app(shared: &Shared, change: impl FnOnce(&mut App) -> Result<(), String>) -> Reply {
    let mut app = shared.lock();
    let error = change(&mut app).err();
    reply(shared, &mut app, error)
}

fn finite(value: f64, what: &str) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{what} must be a number"))
    }
}

pub fn state(shared: &Shared) -> Reply {
    with_app(shared, |_| Ok(()))
}

pub fn set_target(shared: &Shared, lufs: f64) -> Reply {
    with_app(shared, |app| {
        app.set_target(finite(lufs, "The target loudness")?);
        Ok(())
    })
}

pub fn set_ceiling(shared: &Shared, dbtp: f64) -> Reply {
    with_app(shared, |app| {
        app.set_ceiling(finite(dbtp, "The peak ceiling")?);
        Ok(())
    })
}

pub fn set_peak_protection(shared: &Shared, on: bool) -> Reply {
    with_app(shared, |app| {
        app.set_peak_protection(on);
        Ok(())
    })
}

pub fn set_codec(shared: &Shared, id: &str) -> Reply {
    with_app(shared, |app| {
        let codec = Codec::from_name(id).ok_or_else(|| format!("Unknown format {id:?}"))?;
        app.set_codec(codec);
        Ok(())
    })
}

pub fn set_bit_depth(shared: &Shared, id: &str) -> Reply {
    with_app(shared, |app| {
        let depth = BitDepth::from_name(id).ok_or_else(|| format!("Unknown bit depth {id:?}"))?;
        app.set_bit_depth(depth);
        Ok(())
    })
}

pub fn select_preset(shared: &Shared, index: usize) -> Reply {
    with_app(shared, |app| {
        app.select_preset(index);
        Ok(())
    })
}

pub fn save_preset(shared: &Shared, name: &str) -> Reply {
    with_app(shared, |app| app.save_preset(name))
}

pub fn delete_preset(shared: &Shared) -> Reply {
    with_app(shared, App::delete_preset)
}

pub fn set_strategy(shared: &Shared, id: &str) -> Reply {
    with_app(shared, |app| {
        let kind = strategy_from_id(id).ok_or_else(|| format!("Unknown File Strategy {id:?}"))?;
        app.set_strategy_kind(kind);
        Ok(())
    })
}

pub fn set_strategy_text(shared: &Shared, text: String) -> Reply {
    with_app(shared, |app| {
        app.set_strategy_text(text);
        Ok(())
    })
}

pub fn clear_list(shared: &Shared) -> Reply {
    with_app(shared, App::clear)
}

/// Adds files, CUE sheets or folders. The caller starts measuring the returned jobs.
pub fn add_paths(shared: &Shared, paths: &[String]) -> (Reply, Vec<Job>) {
    let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let mut app = shared.lock();
    let jobs = app.add_paths(&paths);
    (reply(shared, &mut app, None), jobs)
}

/// Records a finished measurement.
pub fn apply_outcome(shared: &Shared, outcome: MeasureOutcome) -> Reply {
    let mut app = shared.lock();
    app.apply(outcome);
    reply(shared, &mut app, None)
}

/// Records progress of a run.
pub fn apply_run_event(shared: &Shared, event: RunEvent) -> Reply {
    let mut app = shared.lock();
    app.apply_run_event(event);
    reply(shared, &mut app, None)
}

/// The user pressed Run. When it can start, the run is returned for the caller to start (after
/// it has found FFmpeg).
pub fn begin_run(shared: &Shared, confirmed: bool) -> (RunReply, Option<RunSpec>) {
    let mut app = shared.lock();
    let request = app.begin_run(confirmed);
    let spec = match request {
        RunRequest::Started => app.take_run_spec(),
        _ => None,
    };
    let (outcome, message) = match request {
        RunRequest::Started => ("started", None),
        RunRequest::NeedsConfirmation => ("needs_confirmation", None),
        RunRequest::Refused(reason) => ("refused", Some(reason)),
    };
    let state = reply(shared, &mut app, None).state;
    (
        RunReply {
            outcome,
            message,
            state,
        },
        spec,
    )
}

/// A run was prepared but could not start, such as when FFmpeg is missing.
pub fn abort_run(shared: &Shared, reason: &str) -> RunReply {
    let mut app = shared.lock();
    app.abort_run(reason);
    let state = reply(shared, &mut app, None).state;
    RunReply {
        outcome: "refused",
        message: Some(format!("Cannot run: {reason}")),
        state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::time::Duration;

    use rangeforge_export::ffmpeg::Ffmpeg;
    use rangeforge_session::{engine, runner};
    use tempfile::tempdir;

    fn shared_with_presets(dir: &Path) -> Shared {
        Shared::new(Some(dir.join("presets.json")), None)
    }

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

    /// A two-second stereo 16-bit WAV of a repeating ramp.
    fn write_wav(path: &Path) {
        let rate: u32 = 44_100;
        let frames = rate * 2;
        let data_len = frames * 4;
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
        for frame in 0..frames {
            let ramp = i16::try_from(i32::try_from(frame % 200).unwrap() * 100 - 10_000).unwrap();
            bytes.extend_from_slice(&ramp.to_le_bytes());
            bytes.extend_from_slice(&ramp.to_le_bytes());
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn it_starts_empty_with_the_built_in_presets() {
        let dir = tempdir().unwrap();
        let reply = state(&shared_with_presets(dir.path()));
        assert_eq!(reply.error, None);
        assert_eq!(reply.state.groups.len(), 0);
        assert_eq!(reply.state.presets.names.len(), 5);
        assert_eq!(reply.state.presets.selected, Some(1));
        assert_eq!(reply.state.run_blocker.as_deref(), Some("Nothing to run"));
    }

    #[test]
    fn settings_changes_come_back_in_the_new_state() {
        let dir = tempdir().unwrap();
        let shared = shared_with_presets(dir.path());

        assert!((set_target(&shared, -18.0).state.volume.target_lufs - -18.0).abs() < 1e-9);
        assert!(
            !set_peak_protection(&shared, false)
                .state
                .volume
                .peak_protection
        );
        assert!((set_ceiling(&shared, -1.0).state.volume.ceiling_dbtp - -1.0).abs() < 1e-9);
        assert_eq!(set_codec(&shared, "flac").state.output.codec, "flac");
        assert_eq!(set_codec(&shared, "flac").state.output.container, "flac");
        assert_eq!(set_bit_depth(&shared, "24").state.output.bit_depth, "24");
        assert_eq!(
            set_strategy(&shared, "rename_old").state.strategy.kind,
            "rename_old"
        );
        let reply = set_strategy_text(&shared, "_backup".to_string());
        assert_eq!(reply.state.strategy.old_suffix, "_backup");
    }

    #[test]
    fn nonsense_is_refused_with_a_reason_and_changes_nothing() {
        let dir = tempdir().unwrap();
        let shared = shared_with_presets(dir.path());

        let reply = set_codec(&shared, "mp3");
        assert!(reply.error.unwrap().contains("Unknown format"));
        assert_eq!(reply.state.output.codec, "alac");

        assert!(set_bit_depth(&shared, "32").error.is_some());
        assert!(set_strategy(&shared, "shred").error.is_some());
        assert!(
            set_target(&shared, f64::NAN)
                .error
                .unwrap()
                .contains("must be a number")
        );
        assert!((state(&shared).state.volume.target_lufs - -22.0).abs() < 1e-9);
    }

    #[test]
    fn presets_are_saved_to_disk_when_they_change() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("presets.json");
        let shared = shared_with_presets(dir.path());

        assert!(
            save_preset(&shared, "Car")
                .error
                .unwrap()
                .contains("nothing to save")
        );
        set_target(&shared, -17.0);
        let reply = save_preset(&shared, "Car");
        assert_eq!(reply.error, None);
        assert!(reply.state.presets.names.contains(&"Car".to_string()));

        // A fresh start finds it on disk.
        let reloaded = state(&shared_with_presets(dir.path()));
        assert!(reloaded.state.presets.names.contains(&"Car".to_string()));
        assert!(path.exists());

        let reply = delete_preset(&shared);
        assert_eq!(reply.error, None);
        assert!(!reply.state.presets.names.contains(&"Car".to_string()));
        assert!(
            !state(&shared_with_presets(dir.path()))
                .state
                .presets
                .names
                .contains(&"Car".to_string())
        );
    }

    #[test]
    fn selecting_a_preset_applies_it() {
        let dir = tempdir().unwrap();
        let shared = shared_with_presets(dir.path());
        let reply = select_preset(&shared, 2);
        assert_eq!(reply.state.presets.selected, Some(2));
        assert_eq!(reply.state.output.codec, "flac");
        assert_eq!(reply.state.output.bit_depth, "16");
    }

    #[test]
    fn files_are_added_and_measured_and_can_be_written() {
        let Some(ffmpeg) = ffmpeg() else { return };
        let dir = tempdir().unwrap();
        let song = dir.path().join("song.wav");
        write_wav(&song);
        let shared = shared_with_presets(dir.path());

        // Drop a file on the window.
        let (reply, jobs) = add_paths(&shared, &[song.display().to_string()]);
        assert_eq!(reply.state.groups.len(), 1);
        assert!(reply.state.groups[0].measuring);
        assert_eq!(jobs.len(), 1);
        assert_eq!(
            reply.state.run_blocker.as_deref(),
            Some("Measuring is not finished yet")
        );

        // Measuring finishes in the background.
        let outcomes = engine::start(jobs, shared.cache());
        let outcome = outcomes.recv_timeout(Duration::from_secs(60)).unwrap();
        let reply = apply_outcome(&shared, outcome);
        assert!(!reply.state.groups[0].measuring);
        let track = &reply.state.groups[0].tracks[0];
        assert!(track.loudness_lufs.is_some());
        assert!(track.adjust_db.is_some());
        assert_eq!(reply.state.run_blocker, None);

        // Run it, writing next to the source.
        let (run, spec) = begin_run(&shared, false);
        assert_eq!(run.outcome, "started");
        let events = runner::start(spec.unwrap(), ffmpeg);
        loop {
            let event = events.recv_timeout(Duration::from_secs(60)).unwrap();
            let done = event == RunEvent::AllDone;
            let reply = apply_run_event(&shared, event);
            if done {
                assert!(!reply.state.running);
                assert_eq!(reply.state.groups[0].run_state, "saved");
                assert!(reply.state.groups[0].tracks[0].saved);
                break;
            }
        }
        assert!(dir.path().join("song_00.m4a").exists());
        assert!(song.exists(), "the source is untouched");
    }

    #[test]
    fn replace_asks_the_window_to_confirm_before_anything_runs() {
        let dir = tempdir().unwrap();
        let song = dir.path().join("song.wav");
        write_wav(&song);
        let shared = shared_with_presets(dir.path());
        let (_, jobs) = add_paths(&shared, &[song.display().to_string()]);
        let outcomes = engine::start(jobs, shared.cache());
        apply_outcome(
            &shared,
            outcomes.recv_timeout(Duration::from_secs(60)).unwrap(),
        );
        set_strategy(&shared, "replace");

        let (run, spec) = begin_run(&shared, false);
        assert_eq!(run.outcome, "needs_confirmation");
        assert!(spec.is_none());
        assert!(!run.state.running);

        let (run, spec) = begin_run(&shared, true);
        assert_eq!(run.outcome, "started");
        assert!(spec.is_some());
        assert!(run.state.running);
    }

    #[test]
    fn a_refused_run_says_why() {
        let dir = tempdir().unwrap();
        let shared = shared_with_presets(dir.path());
        let (run, spec) = begin_run(&shared, true);
        assert_eq!(run.outcome, "refused");
        assert_eq!(run.message.as_deref(), Some("Nothing to run"));
        assert!(spec.is_none());
    }

    #[test]
    fn a_run_that_cannot_start_is_put_right() {
        let dir = tempdir().unwrap();
        let song = dir.path().join("song.wav");
        write_wav(&song);
        let shared = shared_with_presets(dir.path());
        let (_, jobs) = add_paths(&shared, &[song.display().to_string()]);
        let outcomes = engine::start(jobs, shared.cache());
        apply_outcome(
            &shared,
            outcomes.recv_timeout(Duration::from_secs(60)).unwrap(),
        );
        let (_, spec) = begin_run(&shared, false);
        assert!(spec.is_some());

        let reply = abort_run(&shared, "FFmpeg was not found");
        assert_eq!(reply.outcome, "refused");
        assert!(!reply.state.running);
        assert_eq!(reply.state.groups[0].run_state, "idle");
        assert!(reply.message.unwrap().contains("FFmpeg was not found"));
    }

    #[test]
    fn the_list_can_be_cleared() {
        let dir = tempdir().unwrap();
        let song = dir.path().join("song.wav");
        write_wav(&song);
        let shared = shared_with_presets(dir.path());
        add_paths(&shared, &[song.display().to_string()]);
        assert_eq!(state(&shared).state.groups.len(), 1);
        let reply = clear_list(&shared);
        assert_eq!(reply.error, None);
        assert_eq!(reply.state.groups.len(), 0);
    }

    #[test]
    fn replies_can_be_turned_into_json_for_the_window() {
        let dir = tempdir().unwrap();
        let json = serde_json::to_string(&state(&shared_with_presets(dir.path()))).unwrap();
        assert!(json.contains("\"state\""));
        assert!(json.contains("\"error\":null"));
    }
}
