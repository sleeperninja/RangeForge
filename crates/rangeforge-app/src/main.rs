// Release builds are a window program, not a console program.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Tauri commands must take their arguments by value, and are async so a slow folder on a
// network drive cannot freeze the window.
#![allow(clippy::needless_pass_by_value, clippy::unused_async)]

//! The `RangeForge` desktop window.
//!
//! The page in `frontend/` shows the screen and sends actions. All the rules live in
//! `rangeforge-session`, shared with the terminal view; [`ops`] turns them into plain functions
//! and this file wraps those as commands for the page.

mod credentials;
mod ops;

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

use ops::{Locations, Reply, RunReply, Shared};
use rangeforge_export::ffmpeg::Ffmpeg;
use rangeforge_lookup::billboard::{HttpFetcher, unix_now};
use rangeforge_lookup::lastfm::{HttpGet, LastFm};
use rangeforge_lookup::tagcache::Subject;
use rangeforge_session::load::Job;
use rangeforge_session::model::{Names, RunEvent};
use rangeforge_session::naming::NamingPreview;
use rangeforge_session::{engine, paths, runner, secrets};
use tauri::{AppHandle, Emitter, Manager, State};

type Session<'a> = State<'a, Arc<Shared>>;

/// Sends the new screen to the page.
fn emit(app: &AppHandle, reply: &Reply) {
    // If the window is gone there is nobody to tell.
    let _ = app.emit("state", reply);
}

/// Measures `jobs` in the background and tells the page as each file finishes. A measured
/// single file brings its tags, which may be new questions for Last.fm.
fn watch_measuring(app: &AppHandle, shared: &Arc<Shared>, jobs: Vec<Job>) {
    if jobs.is_empty() {
        return;
    }
    let outcomes = engine::start(jobs, shared.cache());
    let (app, shared) = (app.clone(), Arc::clone(shared));
    thread::spawn(move || {
        for outcome in outcomes {
            emit(&app, &ops::apply_outcome(&shared, outcome));
            watch_genres(&app, &shared);
        }
    });
}

/// Asks Last.fm for genres in the background, if it is switched on and there is something to
/// ask, and tells the page as answers come in. Cheap to call whenever something changes.
fn watch_genres(app: &AppHandle, shared: &Arc<Shared>) {
    if !ops::genres_wanted(shared) {
        return;
    }
    let (app, shared) = (app.clone(), Arc::clone(shared));
    thread::spawn(move || {
        let Some(key) = shared.lastfm_key() else {
            return;
        };
        let client = LastFm::new(HttpGet, key.value);
        let ask = |subject: &Subject| match subject {
            Subject::Album { artist, album } => client.album_tags(artist, album),
            Subject::Track { artist, title } => client.track_tags(artist, title),
        };
        // Last.fm asks for no more than about five requests a second.
        ops::update_genres(
            &shared,
            &ask,
            unix_now(),
            Duration::from_millis(250),
            &mut |reply| emit(&app, &reply),
        );
    });
}

/// Gets the Hot 100 list ready in the background (the saved copy first, then a check for a
/// newer one) and tells the page after each step. Does nothing while lookups are off.
fn watch_radio(app: &AppHandle, shared: &Arc<Shared>) {
    let (app, shared) = (app.clone(), Arc::clone(shared));
    thread::spawn(move || {
        ops::update_radio(&shared, &HttpFetcher, unix_now(), &mut |reply| {
            emit(&app, &reply);
        });
    });
}

/// Tells the page about each step of a run.
fn watch_run(app: &AppHandle, shared: &Arc<Shared>, events: Receiver<RunEvent>) {
    let (app, shared) = (app.clone(), Arc::clone(shared));
    thread::spawn(move || {
        for event in events {
            emit(&app, &ops::apply_run_event(&shared, event));
        }
    });
}

#[tauri::command]
async fn get_state(state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::state(&state))
}

#[tauri::command]
async fn add_paths(
    paths: Vec<String>,
    state: Session<'_>,
    app: AppHandle,
) -> Result<Reply, String> {
    let (reply, jobs) = ops::add_paths(&state, &paths);
    watch_measuring(&app, state.inner(), jobs);
    // Songs from a CUE sheet are known at once and can be asked about straight away.
    watch_genres(&app, state.inner());
    Ok(reply)
}

#[tauri::command]
async fn clear_list(state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::clear_list(&state))
}

#[tauri::command]
async fn set_target(value: f64, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_target(&state, value))
}

#[tauri::command]
async fn set_ceiling(value: f64, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_ceiling(&state, value))
}

#[tauri::command]
async fn set_peak_protection(on: bool, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_peak_protection(&state, on))
}

#[tauri::command]
async fn set_codec(id: String, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_codec(&state, &id))
}

#[tauri::command]
async fn set_container(id: String, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_container(&state, &id))
}

#[tauri::command]
async fn set_sample_rate(id: String, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_sample_rate(&state, &id))
}

#[tauri::command]
async fn set_dsd_format(field: String, id: String, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_dsd_format(&state, &field, &id))
}

#[tauri::command]
async fn set_confirm_destructive(on: bool, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_confirm_destructive(&state, on))
}

#[tauri::command]
async fn set_bit_depth(id: String, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_bit_depth(&state, &id))
}

#[tauri::command]
async fn select_preset(index: usize, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::select_preset(&state, index))
}

#[tauri::command]
async fn save_preset(name: String, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::save_preset(&state, &name))
}

#[tauri::command]
async fn delete_preset(state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::delete_preset(&state))
}

#[tauri::command]
async fn set_place(id: String, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_place(&state, &id))
}

#[tauri::command]
async fn set_place_text(text: String, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_place_text(&state, text))
}

#[tauri::command]
async fn set_conflict(id: String, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_conflict(&state, &id))
}

#[tauri::command]
async fn set_conflict_text(text: String, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_conflict_text(&state, text))
}

#[tauri::command]
async fn set_delete_original(on: bool, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_delete_original(&state, on))
}

#[tauri::command]
async fn set_naming(album: String, single: String, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_naming(&state, &album, &single))
}

#[tauri::command]
async fn preview_naming(
    album: String,
    single: String,
    state: Session<'_>,
) -> Result<NamingPreview, String> {
    Ok(ops::preview_naming(&state, &album, &single))
}

#[tauri::command]
async fn set_metadata(on: bool, state: Session<'_>, app: AppHandle) -> Result<Reply, String> {
    let reply = ops::set_metadata(&state, on);
    if on {
        watch_radio(&app, state.inner());
        watch_genres(&app, state.inner());
    }
    Ok(reply)
}

#[tauri::command]
async fn set_lastfm_enabled(on: bool, state: Session<'_>, app: AppHandle) -> Result<Reply, String> {
    let reply = ops::set_lastfm_enabled(&state, on);
    if on {
        watch_genres(&app, state.inner());
    }
    Ok(reply)
}

#[tauri::command]
async fn set_lastfm_key(key: String, state: Session<'_>, app: AppHandle) -> Result<Reply, String> {
    let reply = ops::set_lastfm_key(&state, &key);
    watch_genres(&app, state.inner());
    Ok(reply)
}

#[tauri::command]
async fn clear_completed(state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::clear_completed(&state))
}

#[tauri::command]
async fn get_lastfm_tags(
    group: usize,
    track: Option<usize>,
    state: Session<'_>,
    app: AppHandle,
) -> Result<Reply, String> {
    let reply = ops::request_lastfm(&state, group, track);
    watch_genres(&app, state.inner());
    Ok(reply)
}

#[tauri::command]
async fn add_tag(
    group: usize,
    track: Option<usize>,
    text: String,
    state: Session<'_>,
) -> Result<Reply, String> {
    Ok(ops::add_tag(&state, group, track, &text))
}

#[tauri::command]
async fn remove_tag(
    group: usize,
    track: usize,
    text: String,
    state: Session<'_>,
) -> Result<Reply, String> {
    Ok(ops::remove_tag(&state, group, track, &text))
}

/// The names typed into the edit window.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NamesInput {
    artist: String,
    title: String,
    album: String,
    album_artist: String,
}

/// Types in names by hand, then lets Last.fm be asked about the new ones if lookups are on.
#[tauri::command]
async fn set_names(
    group: usize,
    track: Option<usize>,
    names: NamesInput,
    state: Session<'_>,
    app: AppHandle,
) -> Result<Reply, String> {
    let names = Names {
        artist: names.artist,
        title: names.title,
        album: names.album,
        album_artist: names.album_artist,
    };
    let reply = ops::set_names(&state, group, track, &names);
    watch_genres(&app, state.inner());
    Ok(reply)
}

#[tauri::command]
async fn remove_group(group: usize, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::remove_group(&state, group))
}

#[tauri::command]
async fn set_included(
    group: usize,
    track: Option<usize>,
    on: bool,
    state: Session<'_>,
) -> Result<Reply, String> {
    Ok(ops::set_included(&state, group, track, on))
}

#[tauri::command]
async fn run(confirmed: bool, state: Session<'_>, app: AppHandle) -> Result<RunReply, String> {
    let (reply, spec) = ops::begin_run(&state, confirmed);
    let Some(spec) = spec else {
        return Ok(reply);
    };
    match Ffmpeg::locate() {
        Ok(ffmpeg) => {
            watch_run(&app, state.inner(), runner::start(spec, ffmpeg));
            Ok(reply)
        }
        Err(error) => Ok(ops::abort_run(&state, &error.to_string())),
    }
}

fn main() {
    // A developer's `.env` file (with LASTFM_API in it) is read in debug builds only. A release
    // build uses the key saved in Settings, or one set in the environment.
    #[cfg(debug_assertions)]
    {
        let _ = dotenvy::dotenv();
    }
    let shared = Arc::new(Shared::new(
        Locations {
            presets: paths::presets_path(None),
            settings: paths::settings_path(None),
            hot100: paths::hot100_path(),
            lastfm_cache: paths::lastfm_cache_path(),
            cache: paths::default_cache_dir(),
        },
        Box::new(credentials::OsStore),
        std::env::var(secrets::LASTFM_ENV).ok(),
    ));

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(shared)
        .invoke_handler(tauri::generate_handler![
            get_state,
            add_paths,
            clear_list,
            set_target,
            set_ceiling,
            set_peak_protection,
            set_codec,
            set_container,
            set_sample_rate,
            set_bit_depth,
            set_dsd_format,
            set_confirm_destructive,
            select_preset,
            save_preset,
            delete_preset,
            set_place,
            set_place_text,
            set_conflict,
            set_conflict_text,
            set_delete_original,
            set_naming,
            preview_naming,
            set_metadata,
            set_lastfm_enabled,
            set_lastfm_key,
            clear_completed,
            remove_group,
            get_lastfm_tags,
            add_tag,
            remove_tag,
            set_names,
            set_included,
            run,
        ])
        .setup(|app| {
            // Files named on the command line open straight away, which also makes "Open with"
            // from the file manager work.
            let args: Vec<String> = std::env::args_os()
                .skip(1)
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            let shared = Arc::clone(app.state::<Arc<Shared>>().inner());
            if !args.is_empty() {
                let (_, jobs) = ops::add_paths(&shared, &args);
                watch_measuring(app.handle(), &shared, jobs);
            }
            // If lookups were left on last time, get the Hot 100 list ready.
            watch_radio(app.handle(), &shared);
            watch_genres(app.handle(), &shared);
            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("RangeForge could not open its window: {error}");
        std::process::exit(1);
    }
}
