//! What the window can do, as plain functions.
//!
//! Each function changes the shared screen state and returns the new state for the window to
//! display. Nothing here knows about Tauri, so all of it can be tested directly. The thin
//! wrappers in `main.rs` turn these into commands and forward progress to the window.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rangeforge_audio::cache::Cache;
use rangeforge_export::format::{BitDepth, Codec, Container, SampleRate};
use rangeforge_lookup::billboard::{self, Fetcher, Refresh};
use rangeforge_lookup::lastfm::{LastFmError, RawTag};
use rangeforge_lookup::tagcache::{Subject, TagCache};
use rangeforge_session::controls::PresetList;
use rangeforge_session::load::Job;
use rangeforge_session::model::{
    App, MeasureOutcome, Names, RunEvent, RunRequest, RunSpec, Settings,
};
use rangeforge_session::presets;
use rangeforge_session::secrets::{self, SecretStore};
use rangeforge_session::snapshot::{Snapshot, conflict_from_id, place_from_id};
use rangeforge_session::{appsettings, naming};
use serde::Serialize;

/// How long a check of the Hot 100 file stays good before the server is asked again.
const RADIO_CHECK_EVERY: Duration = Duration::from_secs(12 * 60 * 60);

/// Where the program keeps its files.
#[derive(Debug, Default, Clone)]
pub struct Locations {
    pub presets: Option<PathBuf>,
    pub settings: Option<PathBuf>,
    /// The small local copy of the Hot 100 history.
    pub hot100: Option<PathBuf>,
    /// What Last.fm has said about albums and songs.
    pub lastfm_cache: Option<PathBuf>,
    pub cache: Option<PathBuf>,
}

/// The screen state, shared between the window's commands and the background threads.
pub struct Shared {
    app: Mutex<App>,
    presets_path: Option<PathBuf>,
    settings_path: Option<PathBuf>,
    hot100_path: Option<PathBuf>,
    lastfm_cache_path: Option<PathBuf>,
    cache: Option<Cache>,
    /// Where API keys are kept.
    store: Box<dyn SecretStore>,
    /// The environment's Last.fm key, read once at start-up.
    env_lastfm: Option<String>,
    /// A Hot 100 update is already running.
    radio_busy: AtomicBool,
    /// Last.fm is already being asked.
    genres_busy: AtomicBool,
}

impl Shared {
    /// Starts with an empty list and the saved presets and settings (or the defaults).
    /// `env_lastfm` is the Last.fm key from the environment, used when none is saved.
    pub fn new(
        locations: Locations,
        store: Box<dyn SecretStore>,
        env_lastfm: Option<String>,
    ) -> Self {
        let Locations {
            presets: presets_path,
            settings: settings_path,
            hot100: hot100_path,
            lastfm_cache: lastfm_cache_path,
            cache,
        } = locations;
        let mut app = App::new(Vec::new(), Vec::new(), Vec::new(), Settings::default());
        if let Some(path) = &settings_path {
            let loaded = appsettings::load(path);
            app.apply_saved_settings(loaded.settings);
            for note in loaded.notes {
                app.log(note);
            }
        }
        match secrets::lastfm_key(store.as_ref(), env_lastfm.clone()) {
            Ok(key) => {
                app.lastfm_key = key.map_or(secrets::KeySource::None, |key| key.source);
            }
            Err(problem) => app.log(format!("Could not read the saved Last.fm key: {problem:#}")),
        }
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
            settings_path,
            hot100_path,
            lastfm_cache_path,
            cache: cache.map(Cache::new),
            store,
            env_lastfm,
            radio_busy: AtomicBool::new(false),
            genres_busy: AtomicBool::new(false),
        }
    }

    /// Where measurements are remembered.
    pub fn cache(&self) -> Option<&Cache> {
        self.cache.as_ref()
    }

    /// The Last.fm key to use, if there is one. Used by the code that asks Last.fm; it never
    /// goes to the window.
    pub fn lastfm_key(&self) -> Option<secrets::Key> {
        secrets::lastfm_key(self.store.as_ref(), self.env_lastfm.clone())
            .ok()
            .flatten()
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
    if app.take_settings_changed()
        && let Some(path) = &shared.settings_path
        && let Err(problem) = appsettings::save(path, &app.saved_settings())
    {
        app.log(format!("Could not save settings: {problem:#}"));
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

pub fn set_container(shared: &Shared, id: &str) -> Reply {
    with_app(shared, |app| {
        let container =
            Container::from_name(id).ok_or_else(|| format!("Unknown container {id:?}"))?;
        app.set_container(container);
        Ok(())
    })
}

pub fn set_sample_rate(shared: &Shared, id: &str) -> Reply {
    with_app(shared, |app| {
        let rate =
            SampleRate::from_name(id).ok_or_else(|| format!("Unknown sample rate {id:?}"))?;
        app.set_sample_rate(rate)
    })
}

/// Sets one part of the DSD Format: `codec`, `container`, `sample_rate` or `bit_depth`. For the
/// first two, `same` follows the main Output Format.
pub fn set_dsd_format(shared: &Shared, field: &str, id: &str) -> Reply {
    with_app(shared, |app| {
        let same = id == "same";
        match field {
            "codec" => {
                let codec = if same {
                    None
                } else {
                    Some(Codec::from_name(id).ok_or_else(|| format!("Unknown codec {id:?}"))?)
                };
                app.set_dsd_codec(codec);
                Ok(())
            }
            "container" => {
                let container = if same {
                    None
                } else {
                    Some(
                        Container::from_name(id)
                            .ok_or_else(|| format!("Unknown container {id:?}"))?,
                    )
                };
                app.set_dsd_container(container);
                Ok(())
            }
            "sample_rate" => app.set_dsd_sample_rate(
                SampleRate::from_name(id).ok_or_else(|| format!("Unknown sample rate {id:?}"))?,
            ),
            "bit_depth" => app.set_dsd_bit_depth(
                BitDepth::from_name(id).ok_or_else(|| format!("Unknown bit depth {id:?}"))?,
            ),
            other => Err(format!("Unknown DSD setting {other:?}")),
        }
    })
}

/// Turns the question before an overwriting or deleting run on or off.
pub fn set_confirm_destructive(shared: &Shared, on: bool) -> Reply {
    with_app(shared, |app| {
        app.set_confirm_destructive(on);
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

pub fn set_place(shared: &Shared, id: &str) -> Reply {
    with_app(shared, |app| {
        let kind = place_from_id(id).ok_or_else(|| format!("Unknown place {id:?}"))?;
        app.set_where(kind);
        Ok(())
    })
}

pub fn set_place_text(shared: &Shared, text: String) -> Reply {
    with_app(shared, |app| {
        app.set_where_text(text);
        Ok(())
    })
}

pub fn set_conflict(shared: &Shared, id: &str) -> Reply {
    with_app(shared, |app| {
        let kind = conflict_from_id(id).ok_or_else(|| format!("Unknown choice {id:?}"))?;
        app.set_conflict(kind);
        Ok(())
    })
}

pub fn set_conflict_text(shared: &Shared, text: String) -> Reply {
    with_app(shared, |app| {
        app.set_conflict_text(text);
        Ok(())
    })
}

pub fn set_delete_original(shared: &Shared, on: bool) -> Reply {
    with_app(shared, |app| {
        app.set_delete_original(on);
        Ok(())
    })
}

/// Sets the file name patterns, which are kept between sessions.
pub fn set_naming(shared: &Shared, album: &str, single: &str) -> Reply {
    with_app(shared, |app| app.set_naming(album, single))
}

/// What the patterns would produce, without keeping them, for the Settings window to show
/// while the user types.
pub fn preview_naming(shared: &Shared, album: &str, single: &str) -> naming::NamingPreview {
    let extension = shared.lock().output.format().extension();
    naming::preview(album, single, extension)
}

/// The "Metadata API" switch: whether any outside lookup happens at all.
pub fn set_metadata(shared: &Shared, on: bool) -> Reply {
    with_app(shared, |app| {
        app.set_metadata_enabled(on);
        Ok(())
    })
}

/// "Use Last.fm", beside its key.
pub fn set_lastfm_enabled(shared: &Shared, on: bool) -> Reply {
    with_app(shared, |app| {
        app.set_lastfm_enabled(on);
        Ok(())
    })
}

/// Keeps the Last.fm key in the system's credential store, or forgets it when `key` is empty.
/// The key is never logged or sent back to the window.
pub fn set_lastfm_key(shared: &Shared, key: &str) -> Reply {
    let key = key.trim();
    let stored = if key.is_empty() {
        shared.store.delete(secrets::LASTFM)
    } else {
        shared.store.set(secrets::LASTFM, key)
    };
    let forgot = key.is_empty();
    with_app(shared, |app| {
        stored.map_err(|problem| {
            format!("Could not keep the key in the system's credential store: {problem:#}")
        })?;
        app.lastfm_key = shared
            .lastfm_key()
            .map_or(secrets::KeySource::None, |k| k.source);
        app.log(if forgot {
            "Forgot the saved Last.fm key"
        } else {
            "Saved the Last.fm key"
        });
        Ok(())
    })
}

/// Loads the saved Hot 100 list from disk, which is quick, and gives it to the screen.
fn load_radio(shared: &Shared) -> Option<Reply> {
    let saved = billboard::load(shared.hot100_path.as_deref()?)?;
    let mut app = shared.lock();
    app.log(format!("Hot 100 list ready ({} songs)", saved.hot100.len()));
    app.set_radio(Some(Arc::new(saved.hot100)));
    Some(reply(shared, &mut app, None))
}

fn log_and_describe(shared: &Shared, line: String) -> Reply {
    let mut app = shared.lock();
    app.log(line);
    reply(shared, &mut app, None)
}

/// Gets the Hot 100 list ready: loads the copy on disk right away, then asks whether the chart
/// has changed and downloads it only if so. Slow when a download is needed, so run it on a
/// background thread; `show` is called with the new screen after each step. Does nothing if an
/// update is already running, or if lookups are off.
pub fn update_radio(shared: &Shared, fetcher: &dyn Fetcher, now: u64, show: &mut dyn FnMut(Reply)) {
    if !shared.lock().metadata.enabled || shared.radio_busy.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(reply) = load_radio(shared) {
        show(reply);
    }
    if let Some(path) = &shared.hot100_path {
        match billboard::refresh(path, fetcher, now, RADIO_CHECK_EVERY) {
            Ok(Refresh::Updated { songs }) => {
                show(log_and_describe(
                    shared,
                    format!("Downloaded a newer Hot 100 list ({songs} songs)"),
                ));
                if let Some(reply) = load_radio(shared) {
                    show(reply);
                }
            }
            Ok(Refresh::Recent | Refresh::Unchanged) => {}
            Err(problem) => show(log_and_describe(
                shared,
                format!("Could not update the Hot 100 list, using the copy on disk: {problem:#}"),
            )),
        }
    }
    shared.radio_busy.store(false, Ordering::SeqCst);
}

/// True when there is something to ask Last.fm, so the caller can start [`update_genres`].
pub fn genres_wanted(shared: &Shared) -> bool {
    shared.lock().has_questions()
}

/// Describes the screen as it is, after a background step.
fn describe(shared: &Shared) -> Reply {
    let mut app = shared.lock();
    reply(shared, &mut app, None)
}

/// How many answers arrive between redraws of the list, so a big collection does not redraw it
/// for every single song.
const SHOW_EVERY: usize = 4;

/// Asks Last.fm what listeners call the albums and songs on the screen, remembering each answer
/// on disk so none is asked twice. Slow, so run it on a background thread. `ask` makes one
/// request; `pause` is waited between requests to stay within Last.fm's limits; `show` is called
/// with the new screen as answers come in.
///
/// If Last.fm cannot be reached or does not accept the key, it stops and says so, and the
/// questions are left to be asked next time. It does nothing if another call is already asking.
pub fn update_genres(
    shared: &Shared,
    ask: &dyn Fn(&Subject) -> Result<Vec<RawTag>, LastFmError>,
    now: u64,
    pause: Duration,
    show: &mut dyn FnMut(Reply),
) {
    loop {
        if shared.genres_busy.swap(true, Ordering::SeqCst) {
            return;
        }
        let stopped = ask_all(shared, ask, now, pause, show);
        shared.genres_busy.store(false, Ordering::SeqCst);
        // Questions can arrive while the last answers are being processed (a file measured, a
        // key saved), so look once more before giving up the job. After a failure, wait for
        // the next trigger instead of failing again at once.
        if stopped || !shared.lock().has_questions() {
            return;
        }
    }
}

/// Does the asking. Returns true if it stopped early because something went wrong.
fn ask_all(
    shared: &Shared,
    ask: &dyn Fn(&Subject) -> Result<Vec<RawTag>, LastFmError>,
    now: u64,
    pause: Duration,
    show: &mut dyn FnMut(Reply),
) -> bool {
    let mut cache = shared
        .lastfm_cache_path
        .as_deref()
        .map(TagCache::load)
        .unwrap_or_default();
    let (mut since_shown, mut asked_online, mut stopped) = (0, false, false);

    'rounds: loop {
        let questions = shared.lock().take_questions();
        if questions.is_empty() {
            break;
        }
        for subject in questions {
            // Something asked for by hand is asked afresh, not answered from the saved copy.
            let by_hand = shared.lock().was_requested(&subject);
            let remembered = if by_hand {
                None
            } else {
                cache.get(&subject, now).map(<[RawTag]>::to_vec)
            };
            let tags = if let Some(tags) = remembered {
                tags
            } else {
                if asked_online {
                    std::thread::sleep(pause);
                }
                asked_online = true;
                let mut answer = ask(&subject);
                if answer == Err(LastFmError::RateLimited) {
                    // Wait a good while and try this one again, once.
                    std::thread::sleep(pause * 8);
                    answer = ask(&subject);
                }
                match answer {
                    Ok(tags) => {
                        cache.put(&subject, now, tags.clone());
                        shared.lock().log(lookup_line(&subject, &tags));
                        tags
                    }
                    Err(error @ (LastFmError::InvalidKey | LastFmError::RateLimited)) => {
                        let advice = match error {
                            LastFmError::InvalidKey => {
                                "Last.fm does not accept the API key. Check it in Settings"
                            }
                            _ => "Last.fm says to slow down. Genres will carry on next time",
                        };
                        stop(shared, advice, show);
                        stopped = true;
                        break 'rounds;
                    }
                    Err(LastFmError::Network(why)) => {
                        stop(shared, &format!("Could not reach Last.fm: {why}"), show);
                        stopped = true;
                        break 'rounds;
                    }
                    // An answer that makes no sense: leave this one without genres.
                    Err(LastFmError::Unexpected(why)) => {
                        shared.lock().log(format!(
                            "Last.fm gave an answer that could not be used for {}: {why}",
                            describe_subject(&subject)
                        ));
                        Vec::new()
                    }
                }
            };
            shared.lock().apply_answer(&subject, tags);
            since_shown += 1;
            if since_shown >= SHOW_EVERY {
                since_shown = 0;
                show(describe(shared));
            }
        }
    }

    if let Some(path) = &shared.lastfm_cache_path
        && !cache.is_empty()
        && let Err(problem) = cache.save(path)
    {
        shared
            .lock()
            .log(format!("Could not save the Last.fm answers: {problem:#}"));
    }
    show(describe(shared));
    stopped
}

/// The question as it was put to Last.fm, for the log: the exact names that were sent.
fn describe_subject(subject: &Subject) -> String {
    match subject {
        Subject::Album { artist, album } => format!("album “{album}” by “{artist}”"),
        Subject::Track { artist, title } => format!("song “{title}” by “{artist}”"),
    }
}

/// What the log says after Last.fm answered.
fn lookup_line(subject: &Subject, tags: &[RawTag]) -> String {
    let what = describe_subject(subject);
    if tags.is_empty() {
        return format!(
            "Last.fm has no tags for {what}. If the names look wrong, fix them in the file's tags."
        );
    }
    let top: Vec<&str> = tags.iter().take(6).map(|tag| tag.name.as_str()).collect();
    format!("Last.fm: {what} → {}", top.join(", "))
}

/// Stops asking: says why in the log, and lets the unanswered questions be asked again later.
fn stop(shared: &Shared, why: &str, show: &mut dyn FnMut(Reply)) {
    {
        let mut app = shared.lock();
        app.log(why.to_string());
        app.forget_asked();
    }
    show(describe(shared));
}

/// Marks a song (or a row's album) to be asked about at Last.fm, whatever the automatic lookup
/// switch says. The caller then starts the asking.
pub fn request_lastfm(shared: &Shared, group: usize, track: Option<usize>) -> Reply {
    with_app(shared, |app| app.request_lastfm(group, track))
}

/// Adds a tag by hand to a song, or to every song of an album when no song is given.
pub fn add_tag(shared: &Shared, group: usize, track: Option<usize>, text: &str) -> Reply {
    with_app(shared, |app| app.add_tag(group, track, text))
}

/// Takes a tag away from a song by hand.
pub fn remove_tag(shared: &Shared, group: usize, track: usize, text: &str) -> Reply {
    with_app(shared, |app| app.remove_tag(group, track, text))
}

/// Types in a song's names, or an album's, replacing what was known.
pub fn set_names(shared: &Shared, group: usize, track: Option<usize>, names: &Names) -> Reply {
    with_app(shared, |app| app.set_names(group, track, names))
}

pub fn clear_list(shared: &Shared) -> Reply {
    with_app(shared, App::clear)
}

/// Removes the albums and files that have been written from the list.
pub fn clear_completed(shared: &Shared) -> Reply {
    with_app(shared, |app| app.clear_completed().map(|_| ()))
}

pub fn remove_group(shared: &Shared, group: usize) -> Reply {
    with_app(shared, |app| app.remove_group(group))
}

/// Switches one song on or off, or with no song given, every song of the group.
pub fn set_included(shared: &Shared, group: usize, track: Option<usize>, on: bool) -> Reply {
    with_app(shared, |app| {
        match track {
            Some(track) => app.set_track_included(group, track, on),
            None => app.set_group_included(group, on),
        }
        Ok(())
    })
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
    use rangeforge_lookup::billboard::Fetched;
    use rangeforge_session::secrets::MemoryStore;
    use rangeforge_session::{engine, runner};
    use tempfile::tempdir;

    fn locations(dir: &Path) -> Locations {
        Locations {
            presets: Some(dir.join("presets.json")),
            settings: Some(dir.join("settings.json")),
            hot100: Some(dir.join("hot100.json")),
            lastfm_cache: Some(dir.join("lastfm.json")),
            cache: None,
        }
    }

    fn shared_with_presets(dir: &Path) -> Shared {
        Shared::new(locations(dir), Box::<MemoryStore>::default(), None)
    }

    /// A credential store that several `Shared` can use in turn, like the real one.
    #[derive(Clone, Default)]
    struct SharedStore(Arc<MemoryStore>);

    impl SecretStore for SharedStore {
        fn get(&self, name: &str) -> anyhow::Result<Option<String>> {
            self.0.get(name)
        }
        fn set(&self, name: &str, value: &str) -> anyhow::Result<()> {
            self.0.set(name, value)
        }
        fn delete(&self, name: &str) -> anyhow::Result<()> {
            self.0.delete(name)
        }
    }

    /// A credential store that refuses everything, like one that is locked.
    struct LockedStore;

    impl SecretStore for LockedStore {
        fn get(&self, _: &str) -> anyhow::Result<Option<String>> {
            Err(anyhow::anyhow!("locked"))
        }
        fn set(&self, _: &str, _: &str) -> anyhow::Result<()> {
            Err(anyhow::anyhow!("locked"))
        }
        fn delete(&self, _: &str) -> anyhow::Result<()> {
            Err(anyhow::anyhow!("locked"))
        }
    }

    const CHART: &str = r#"[{"date": "1973-10-20", "data": [
        {"song": "The Rain Song", "artist": "Led Zeppelin", "this_week": 14, "last_week": null, "peak_position": 14, "weeks_on_chart": 1}
    ]}]"#;

    /// Hands out the chart each time it is asked, and counts the requests.
    struct FakeChart {
        asked: std::cell::Cell<usize>,
        fail: bool,
    }

    impl FakeChart {
        fn new(fail: bool) -> Self {
            Self {
                asked: std::cell::Cell::new(0),
                fail,
            }
        }
    }

    impl Fetcher for FakeChart {
        fn get(&self, _url: &str, _etag: Option<&str>) -> anyhow::Result<Fetched> {
            self.asked.set(self.asked.get() + 1);
            if self.fail {
                anyhow::bail!("no network");
            }
            Ok(Fetched::Body {
                body: Box::new(std::io::Cursor::new(CHART.as_bytes().to_vec())),
                etag: Some("\"v1\"".to_string()),
            })
        }
    }

    const ALBUM_SHEET: &str = "PERFORMER \"Led Zeppelin\"\nTITLE \"Houses of the Holy\"\nFILE \"album.flac\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"The Rain Song\"\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    TITLE \"No Quarter\"\n    INDEX 01 05:00:00\n";

    /// A Last.fm that is on and has a key, with one two-song album on the screen.
    fn shared_with_album(dir: &Path, store: &SharedStore) -> Shared {
        fs::write(dir.join("album.cue"), ALBUM_SHEET).unwrap();
        fs::write(dir.join("album.flac"), b"").unwrap();
        let shared = Shared::new(locations(dir), Box::new(store.clone()), None);
        add_paths(&shared, &[dir.join("album.cue").display().to_string()]);
        set_metadata(&shared, true);
        set_lastfm_enabled(&shared, true);
        set_lastfm_key(&shared, "a-key");
        shared
    }

    fn tags(names: &[(&str, u32)]) -> Vec<RawTag> {
        names
            .iter()
            .map(|(name, count)| RawTag {
                name: (*name).to_string(),
                count: *count,
            })
            .collect()
    }

    /// Answers like Last.fm would for the test album, and counts how often it was asked.
    fn lastfm(
        asked: &std::cell::Cell<usize>,
    ) -> impl Fn(&Subject) -> Result<Vec<RawTag>, LastFmError> + '_ {
        move |subject| {
            asked.set(asked.get() + 1);
            Ok(match subject {
                Subject::Album { .. } => {
                    tags(&[("classic rock", 100), ("hard rock", 80), ("seen live", 70)])
                }
                Subject::Track { title, .. } if title == "The Rain Song" => {
                    tags(&[("ballad", 100), ("classic rock", 60)])
                }
                Subject::Track { .. } => Vec::new(),
            })
        }
    }

    fn chip_texts(reply: &Reply, track: usize) -> Vec<String> {
        reply.state.groups[0].tracks[track]
            .tags
            .iter()
            .map(|chip| chip.text.clone())
            .collect()
    }

    const NO_PAUSE: Duration = Duration::ZERO;

    #[test]
    fn genres_are_asked_once_shown_remembered_and_never_asked_again() {
        let dir = tempdir().unwrap();
        let store = SharedStore::default();
        let shared = shared_with_album(dir.path(), &store);
        assert!(genres_wanted(&shared));

        let asked = std::cell::Cell::new(0);
        let mut screens = Vec::new();
        update_genres(&shared, &lastfm(&asked), 1_000, NO_PAUSE, &mut |r| {
            screens.push(r);
        });

        // One album and two songs.
        assert_eq!(asked.get(), 3);
        let last = screens.last().unwrap();
        assert_eq!(chip_texts(last, 0), ["Classic Rock", "Hard Rock", "Ballad"]);
        assert_eq!(chip_texts(last, 1), ["Classic Rock", "Hard Rock"]);
        assert!(!genres_wanted(&shared), "everything is answered");

        // The next start finds the answers on disk and asks nobody.
        let again = shared_with_album(dir.path(), &store);
        let asked_again = std::cell::Cell::new(0);
        let mut screens = Vec::new();
        update_genres(&again, &lastfm(&asked_again), 1_500, NO_PAUSE, &mut |r| {
            screens.push(r);
        });
        assert_eq!(asked_again.get(), 0);
        assert_eq!(
            chip_texts(screens.last().unwrap(), 0),
            ["Classic Rock", "Hard Rock", "Ballad"]
        );
    }

    #[test]
    fn nothing_is_asked_while_last_fm_is_off_or_has_no_key() {
        let dir = tempdir().unwrap();
        let store = SharedStore::default();
        let shared = shared_with_album(dir.path(), &store);
        set_lastfm_enabled(&shared, false);
        let asked = std::cell::Cell::new(0);
        update_genres(&shared, &lastfm(&asked), 1_000, NO_PAUSE, &mut |_| {});
        assert_eq!(asked.get(), 0);

        set_lastfm_enabled(&shared, true);
        set_lastfm_key(&shared, "");
        assert!(!genres_wanted(&shared), "no key");
        update_genres(&shared, &lastfm(&asked), 1_000, NO_PAUSE, &mut |_| {});
        assert_eq!(asked.get(), 0);
    }

    #[test]
    fn a_rejected_key_stops_the_asking_says_so_and_leaves_the_questions_for_later() {
        let dir = tempdir().unwrap();
        let shared = shared_with_album(dir.path(), &SharedStore::default());
        let mut screens = Vec::new();
        let calls = std::cell::Cell::new(0);
        let rejected = |_: &Subject| {
            calls.set(calls.get() + 1);
            Err(LastFmError::InvalidKey)
        };
        update_genres(&shared, &rejected, 1_000, NO_PAUSE, &mut |r| {
            screens.push(r);
        });

        assert_eq!(
            calls.get(),
            1,
            "it does not keep trying a key that is refused"
        );
        let log = &screens.last().unwrap().state.log;
        assert!(
            log.iter()
                .any(|l| l.contains("does not accept the API key"))
        );
        assert!(
            !log.iter().any(|l| l.contains("a-key")),
            "the key is never logged"
        );
        assert!(
            genres_wanted(&shared),
            "the questions are still there for next time"
        );
    }

    #[test]
    fn a_network_failure_stops_the_asking_and_the_program_carries_on() {
        let dir = tempdir().unwrap();
        let shared = shared_with_album(dir.path(), &SharedStore::default());
        let mut screens = Vec::new();
        let offline = |_: &Subject| Err(LastFmError::Network("connection refused".to_string()));
        update_genres(&shared, &offline, 1_000, NO_PAUSE, &mut |r| {
            screens.push(r);
        });

        let log = &screens.last().unwrap().state.log;
        assert!(
            log.iter()
                .any(|l| l.contains("Could not reach Last.fm: connection refused"))
        );
        assert!(genres_wanted(&shared));
        // And the next attempt works once the network is back.
        let asked = std::cell::Cell::new(0);
        update_genres(&shared, &lastfm(&asked), 1_000, NO_PAUSE, &mut |_| {});
        assert_eq!(asked.get(), 3);
        assert!(!genres_wanted(&shared));
    }

    #[test]
    fn being_told_to_slow_down_waits_and_tries_that_question_again() {
        let dir = tempdir().unwrap();
        let shared = shared_with_album(dir.path(), &SharedStore::default());
        let calls = std::cell::Cell::new(0);
        let slow_once = |_: &Subject| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                Err(LastFmError::RateLimited)
            } else {
                Ok(tags(&[("rock", 100)]))
            }
        };
        update_genres(&shared, &slow_once, 1_000, NO_PAUSE, &mut |_| {});
        assert_eq!(calls.get(), 4, "three questions and one repeat");
        assert!(!genres_wanted(&shared));
    }

    #[test]
    fn an_answer_that_makes_no_sense_leaves_that_song_without_genres_and_the_rest_carry_on() {
        let dir = tempdir().unwrap();
        let shared = shared_with_album(dir.path(), &SharedStore::default());
        let odd = |subject: &Subject| match subject {
            Subject::Track { title, .. } if title == "No Quarter" => {
                Err(LastFmError::Unexpected("gibberish".to_string()))
            }
            _ => Ok(tags(&[("rock", 100)])),
        };
        let mut screens = Vec::new();
        update_genres(&shared, &odd, 1_000, NO_PAUSE, &mut |r| {
            screens.push(r);
        });
        assert_eq!(chip_texts(screens.last().unwrap(), 0), ["Rock"]);
        assert!(!genres_wanted(&shared), "it does not loop on the bad one");
    }

    #[test]
    fn a_sample_rate_the_codec_cannot_write_is_refused_or_put_back() {
        let dir = tempdir().unwrap();
        let shared = shared_with_presets(dir.path());

        // AAC stops at 96 kHz: asking for more is refused with a reason.
        set_codec(&shared, "aac");
        let reply = set_sample_rate(&shared, "192000");
        assert!(
            reply
                .error
                .unwrap()
                .contains("cannot be written at 192 kHz")
        );
        assert_eq!(reply.state.output.sample_rate, "original");

        // A rate chosen for ALAC goes back to original when the codec cannot keep it.
        set_codec(&shared, "alac");
        assert_eq!(
            set_sample_rate(&shared, "192000").state.output.sample_rate,
            "192000"
        );
        let reply = set_codec(&shared, "aac");
        assert_eq!(reply.state.output.sample_rate, "original");
        assert!(
            reply
                .state
                .log
                .iter()
                .any(|line| line.contains("back to original"))
        );

        // Nonsense is refused and changes nothing.
        assert!(
            set_container(&shared, "ogg")
                .error
                .unwrap()
                .contains("Unknown container")
        );
        assert!(
            set_sample_rate(&shared, "22050")
                .error
                .unwrap()
                .contains("Unknown sample rate")
        );
    }

    #[test]
    fn the_container_and_rate_are_remembered_in_a_saved_preset() {
        let dir = tempdir().unwrap();
        let shared = shared_with_presets(dir.path());
        set_container(&shared, "mka");
        set_sample_rate(&shared, "88200");
        let reply = save_preset(&shared, "Archive");
        assert_eq!(reply.error, None);

        // A fresh start finds it on disk, and choosing it brings both back.
        let again = shared_with_presets(dir.path());
        let names = state(&again).state.presets.names;
        let index = names.iter().position(|n| n == "Archive").unwrap();
        let chosen = select_preset(&again, index);
        assert_eq!(chosen.state.output.container, "mka");
        assert_eq!(chosen.state.output.sample_rate, "88200");
    }

    #[test]
    fn the_lookup_switches_start_off_and_are_kept() {
        let dir = tempdir().unwrap();
        let shared = shared_with_presets(dir.path());
        let reply = state(&shared);
        assert!(!reply.state.metadata.enabled);
        assert!(!reply.state.metadata.lastfm_enabled);

        assert!(set_metadata(&shared, true).state.metadata.enabled);
        assert!(
            set_lastfm_enabled(&shared, true)
                .state
                .metadata
                .lastfm_enabled
        );

        let reloaded = state(&shared_with_presets(dir.path()));
        assert!(reloaded.state.metadata.enabled);
        assert!(reloaded.state.metadata.lastfm_enabled);
    }

    #[test]
    fn the_last_fm_key_is_saved_in_the_store_and_never_comes_back() {
        let dir = tempdir().unwrap();
        let store = SharedStore::default();
        let shared = Shared::new(locations(dir.path()), Box::new(store.clone()), None);
        assert_eq!(state(&shared).state.metadata.lastfm_key, "none");

        let reply = set_lastfm_key(&shared, "  abc123secret  ");
        assert_eq!(reply.error, None);
        assert_eq!(reply.state.metadata.lastfm_key, "saved");
        assert_eq!(
            store.get(secrets::LASTFM).unwrap().as_deref(),
            Some("abc123secret"),
            "kept without the spaces"
        );
        assert_eq!(shared.lastfm_key().unwrap().value, "abc123secret");

        // Neither the screen nor the settings file nor the log can hold it.
        let json = serde_json::to_string(&reply).unwrap();
        assert!(!json.contains("abc123secret"), "{json}");
        let settings = fs::read_to_string(dir.path().join("settings.json")).unwrap_or_default();
        assert!(!settings.contains("abc123secret"));

        // A fresh start sees a key is there, from the store.
        let again = Shared::new(locations(dir.path()), Box::new(store.clone()), None);
        assert_eq!(state(&again).state.metadata.lastfm_key, "saved");

        // An empty key forgets it.
        let reply = set_lastfm_key(&shared, "   ");
        assert_eq!(reply.state.metadata.lastfm_key, "none");
        assert_eq!(store.get(secrets::LASTFM).unwrap(), None);
    }

    #[test]
    fn the_environment_supplies_a_key_when_none_is_saved() {
        let dir = tempdir().unwrap();
        let shared = Shared::new(
            locations(dir.path()),
            Box::<MemoryStore>::default(),
            Some("from-environment".to_string()),
        );
        assert_eq!(state(&shared).state.metadata.lastfm_key, "environment");

        // A saved key takes over, and forgetting it falls back to the environment again.
        assert_eq!(
            set_lastfm_key(&shared, "saved-one")
                .state
                .metadata
                .lastfm_key,
            "saved"
        );
        assert_eq!(
            set_lastfm_key(&shared, "").state.metadata.lastfm_key,
            "environment"
        );
    }

    #[test]
    fn a_locked_store_is_reported_not_hidden() {
        let dir = tempdir().unwrap();
        let shared = Shared::new(locations(dir.path()), Box::new(LockedStore), None);
        let reply = set_lastfm_key(&shared, "abc123secret");
        let error = reply.error.unwrap();
        assert!(error.contains("credential store"), "{error}");
        assert!(!error.contains("abc123secret"));
        assert_eq!(reply.state.metadata.lastfm_key, "none");
        // And the start-up check of a locked store did not stop the program.
        assert!(
            state(&shared)
                .state
                .log
                .iter()
                .any(|line| line.contains("saved Last.fm key"))
        );
    }

    #[test]
    fn the_hot_100_list_waits_for_the_switch_and_then_loads_and_updates() {
        let dir = tempdir().unwrap();
        let shared = shared_with_presets(dir.path());
        let chart = FakeChart::new(false);
        let mut screens = Vec::new();

        // Lookups are off: nobody is asked.
        update_radio(&shared, &chart, 1_000, &mut |reply| screens.push(reply));
        assert_eq!(chart.asked.get(), 0);
        assert!(screens.is_empty());

        // On, with nothing saved yet: it downloads once, then loads it.
        set_metadata(&shared, true);
        update_radio(&shared, &chart, 1_000, &mut |reply| screens.push(reply));
        assert_eq!(chart.asked.get(), 1);
        let last = screens.last().unwrap();
        assert_eq!(last.state.metadata.radio_songs, Some(1));
        assert!(
            last.state
                .log
                .iter()
                .any(|l| l.contains("Downloaded a newer"))
        );

        // Soon after, the saved copy is loaded and nobody is asked again.
        let again = shared_with_presets(dir.path());
        set_metadata(&again, true);
        let mut screens = Vec::new();
        update_radio(&again, &chart, 1_060, &mut |reply| screens.push(reply));
        assert_eq!(chart.asked.get(), 1, "checked recently");
        assert_eq!(screens[0].state.metadata.radio_songs, Some(1));
    }

    #[test]
    fn a_failed_hot_100_download_is_reported_and_the_program_carries_on() {
        let dir = tempdir().unwrap();
        let shared = shared_with_presets(dir.path());
        set_metadata(&shared, true);
        let mut screens = Vec::new();

        update_radio(&shared, &FakeChart::new(true), 1_000, &mut |reply| {
            screens.push(reply);
        });

        let last = screens.last().unwrap();
        assert_eq!(last.state.metadata.radio_songs, None);
        assert!(
            last.state
                .log
                .iter()
                .any(|l| l.contains("Could not update the Hot 100 list"))
        );
        // The update is not left marked as running, so a later try works.
        let mut again = Vec::new();
        update_radio(&shared, &FakeChart::new(false), 1_000, &mut |reply| {
            again.push(reply);
        });
        assert_eq!(again.last().unwrap().state.metadata.radio_songs, Some(1));
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
        assert_eq!(set_codec(&shared, "flac").state.output.extension, "flac");
        assert_eq!(set_bit_depth(&shared, "24").state.output.bit_depth, "24");
        assert_eq!(set_container(&shared, "mka").state.output.extension, "mka");
        assert_eq!(
            set_sample_rate(&shared, "96000").state.output.sample_rate,
            "96000"
        );
        assert_eq!(
            set_conflict(&shared, "rename_existing")
                .state
                .strategy
                .conflict,
            "rename_existing"
        );
        let reply = set_conflict_text(&shared, "_backup".to_string());
        assert_eq!(reply.state.strategy.old_suffix, "_backup");
        assert_eq!(
            set_place(&shared, "subfolder").state.strategy.place,
            "subfolder"
        );
        let reply = set_place_text(&shared, "Balanced".to_string());
        assert_eq!(reply.state.strategy.subfolder, "Balanced");
        assert!(
            set_delete_original(&shared, true)
                .state
                .strategy
                .delete_original
        );
    }

    #[test]
    fn nonsense_is_refused_with_a_reason_and_changes_nothing() {
        let dir = tempdir().unwrap();
        let shared = shared_with_presets(dir.path());

        let reply = set_codec(&shared, "mp3");
        assert!(reply.error.unwrap().contains("Unknown format"));
        assert_eq!(reply.state.output.codec, "alac");

        assert!(set_bit_depth(&shared, "32").error.is_some());
        assert!(set_place(&shared, "shred").error.is_some());
        assert!(set_conflict(&shared, "shred").error.is_some());
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
        assert!(dir.path().join("song.m4a").exists());
        assert!(song.exists(), "the source is untouched");
    }

    #[test]
    fn name_patterns_are_checked_previewed_and_kept_between_sessions() {
        let dir = tempdir().unwrap();
        let shared = shared_with_presets(dir.path());

        let preview = preview_naming(&shared, "{artist} - {title}", "{name}");
        assert_eq!(
            preview.album_example.as_deref(),
            Some("Led Zeppelin - No Quarter.m4a")
        );
        assert_eq!(
            state(&shared).state.naming.album,
            "{track:02} - {title}",
            "a preview keeps nothing"
        );

        let reply = set_naming(&shared, "{nope}", "{name}");
        assert!(reply.error.unwrap().contains("not a known variable"));
        assert_eq!(reply.state.naming.album, "{track:02} - {title}");

        let reply = set_naming(&shared, "{artist} - {title}", "{name} (b)");
        assert_eq!(reply.error, None);
        assert_eq!(reply.state.naming.single, "{name} (b)");

        // A fresh start finds them on disk.
        let reloaded = state(&shared_with_presets(dir.path()));
        assert_eq!(reloaded.state.naming.album, "{artist} - {title}");
        assert_eq!(reloaded.state.naming.single, "{name} (b)");
    }

    #[test]
    fn finished_albums_can_be_cleared_and_songs_left_out() {
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

        let reply = set_included(&shared, 0, Some(0), false);
        assert!(!reply.state.groups[0].tracks[0].included);
        assert_eq!(reply.state.queued_songs, 0);
        let reply = set_included(&shared, 0, None, true);
        assert!(reply.state.groups[0].tracks[0].included);

        // Nothing is finished yet, so nothing is cleared.
        assert_eq!(clear_completed(&shared).state.groups.len(), 1);

        let reply = remove_group(&shared, 0);
        assert_eq!(reply.error, None);
        assert_eq!(reply.state.groups.len(), 0);
    }

    #[test]
    fn the_dsd_format_the_file_strategy_and_the_question_survive_a_restart() {
        let dir = tempdir().unwrap();
        let shared = shared_with_presets(dir.path());
        assert_eq!(set_dsd_format(&shared, "sample_rate", "176400").error, None);
        assert_eq!(set_dsd_format(&shared, "bit_depth", "16").error, None);
        assert_eq!(set_dsd_format(&shared, "codec", "flac").error, None);
        assert_eq!(set_dsd_format(&shared, "container", "mka").error, None);
        set_place(&shared, "subfolder");
        set_place_text(&shared, "Balanced".to_string());
        set_conflict(&shared, "overwrite");
        set_delete_original(&shared, true);
        assert_eq!(set_confirm_destructive(&shared, false).error, None);

        let again = shared_with_presets(dir.path());
        let snapshot = state(&again).state;
        assert_eq!(snapshot.dsd.sample_rate, "176400");
        assert_eq!(snapshot.dsd.bit_depth, "16");
        assert_eq!(snapshot.dsd.codec, "flac");
        assert_eq!(snapshot.dsd.container, "mka");
        assert_eq!(snapshot.strategy.place, "subfolder");
        assert_eq!(snapshot.strategy.subfolder, "Balanced");
        assert_eq!(snapshot.strategy.conflict, "overwrite");
        assert!(snapshot.strategy.delete_original);
        assert!(!snapshot.strategy.confirm_first);
    }

    #[test]
    fn dsd_has_no_original_rate_or_source_depth_and_unknown_names_are_refused() {
        let dir = tempdir().unwrap();
        let shared = shared_with_presets(dir.path());
        assert!(
            set_dsd_format(&shared, "sample_rate", "original")
                .error
                .is_some()
        );
        assert!(
            set_dsd_format(&shared, "bit_depth", "source")
                .error
                .is_some()
        );
        assert!(set_dsd_format(&shared, "codec", "mp3").error.is_some());
        assert!(set_dsd_format(&shared, "volume", "11").error.is_some());
        let snapshot = state(&shared).state;
        assert_eq!(snapshot.dsd.sample_rate, "88200");
        assert_eq!(snapshot.dsd.bit_depth, "24");
        let rates: Vec<&str> = snapshot
            .options
            .dsd_sample_rates
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert!(!rates.contains(&"original"), "{rates:?}");
    }

    #[test]
    fn overwriting_asks_the_window_to_confirm_before_anything_runs() {
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
        set_conflict(&shared, "overwrite");

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
