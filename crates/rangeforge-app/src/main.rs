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

mod ops;

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::thread;

use ops::{Reply, RunReply, Shared};
use rangeforge_audio::cache::Cache;
use rangeforge_export::ffmpeg::Ffmpeg;
use rangeforge_session::load::Job;
use rangeforge_session::model::RunEvent;
use rangeforge_session::{engine, paths, runner};
use tauri::{AppHandle, Emitter, Manager, State};

type Session<'a> = State<'a, Arc<Shared>>;

/// Sends the new screen to the page.
fn emit(app: &AppHandle, reply: &Reply) {
    // If the window is gone there is nobody to tell.
    let _ = app.emit("state", reply);
}

/// Measures `jobs` in the background and tells the page as each file finishes.
fn watch_measuring(app: &AppHandle, shared: &Arc<Shared>, jobs: Vec<Job>) {
    if jobs.is_empty() {
        return;
    }
    let outcomes = engine::start(jobs, shared.cache());
    let (app, shared) = (app.clone(), Arc::clone(shared));
    thread::spawn(move || {
        for outcome in outcomes {
            emit(&app, &ops::apply_outcome(&shared, outcome));
        }
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
async fn set_strategy(id: String, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_strategy(&state, &id))
}

#[tauri::command]
async fn set_strategy_text(text: String, state: Session<'_>) -> Result<Reply, String> {
    Ok(ops::set_strategy_text(&state, text))
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
    let shared = Arc::new(Shared::new(
        paths::presets_path(None),
        paths::default_cache_dir().map(Cache::new),
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
            set_bit_depth,
            select_preset,
            save_preset,
            delete_preset,
            set_strategy,
            set_strategy_text,
            run,
        ])
        .setup(|app| {
            // Files named on the command line open straight away, which also makes "Open with"
            // from the file manager work.
            let args: Vec<String> = std::env::args_os()
                .skip(1)
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            if !args.is_empty() {
                let shared = Arc::clone(app.state::<Arc<Shared>>().inner());
                let (_, jobs) = ops::add_paths(&shared, &args);
                watch_measuring(app.handle(), &shared, jobs);
            }
            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("RangeForge could not open its window: {error}");
        std::process::exit(1);
    }
}
