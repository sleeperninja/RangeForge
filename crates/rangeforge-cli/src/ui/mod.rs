//! The interactive terminal view: a table of measurements with live volume targets, the
//! mockup's settings panels, and a Run button.
//!
//! The layout follows the mockups in `docs/mockups`. All the rules live in
//! `rangeforge-session`, shared with the desktop app. This module only draws the screen
//! ([`view`]) and turns terminal key presses into the shared [`Key`] type.

mod view;

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use rangeforge_audio::cache::Cache;
use rangeforge_export::ffmpeg::Ffmpeg;
use rangeforge_session::controls::{Mode, OutputSettings, PresetList, StrategySettings};
use rangeforge_session::load::{self, Job};
use rangeforge_session::model::{App, Key, MeasureOutcome, RunEvent, RunSpec, Settings};
use rangeforge_session::{engine, presets, runner};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

/// Everything the view needs to start.
#[derive(Debug)]
pub struct Options {
    pub paths: Vec<PathBuf>,
    pub settings: Settings,
    pub output: OutputSettings,
    pub strategy: StrategySettings,
    pub cache: Option<Cache>,
    pub show_log: bool,
    /// Where presets are kept, or `None` to use the built-in ones and never save.
    pub presets_path: Option<PathBuf>,
    /// For the snapshot only: press Run once measuring is done and wait for it to finish.
    pub run: bool,
    /// For the snapshot only: confirm a strategy that overwrites files.
    pub yes: bool,
}

/// Turns a terminal key code into the shared key type. Keys the screen does not use give
/// `None`.
fn translate(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Backspace => Key::Backspace,
        _ => return None,
    })
}

/// Loads the paths and sets the screen up with the starting settings and the saved presets.
/// Returns the app and the measuring jobs to start.
fn prepare(options: &Options) -> (App, Vec<Job>) {
    let loaded = load::load(&options.paths);
    let jobs = loaded.jobs.clone();
    let mut app = App::new(loaded.groups, loaded.jobs, loaded.log, options.settings);
    app.show_log = options.show_log;
    app.output = options.output;
    app.strategy = options.strategy.clone();

    let stored = options.presets_path.as_deref().map_or_else(
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
    (app, jobs)
}

/// Opens the interactive view and runs until the user quits.
pub fn run(options: &Options) -> Result<()> {
    let (mut app, jobs) = prepare(options);
    let outcomes = engine::start(jobs, options.cache.as_ref());

    let mut terminal = ratatui::init();
    let result = event_loop(
        &mut terminal,
        &mut app,
        &outcomes,
        options.presets_path.as_deref(),
    );
    ratatui::restore();
    result
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    outcomes: &Receiver<MeasureOutcome>,
    presets_path: Option<&Path>,
) -> Result<()> {
    let mut writing: Option<Receiver<RunEvent>> = None;
    while !app.should_quit {
        terminal.draw(|frame| view::draw(frame, app))?;
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && let Some(key) = translate(key.code)
        {
            app.handle_key(key);
        }
        if let Some(spec) = app.take_run_spec() {
            writing = start_writing(app, spec);
        }
        if app.take_presets_changed() {
            save_presets(app, presets_path);
        }
        while let Ok(outcome) = outcomes.try_recv() {
            app.apply(outcome);
        }
        if let Some(events) = &writing {
            while let Ok(event) = events.try_recv() {
                app.apply_run_event(event);
            }
        }
    }
    Ok(())
}

/// Starts writing in the background, or tells the user why it cannot.
fn start_writing(app: &mut App, spec: RunSpec) -> Option<Receiver<RunEvent>> {
    match Ffmpeg::locate() {
        Ok(ffmpeg) => Some(runner::start(spec, ffmpeg)),
        Err(error) => {
            app.abort_run(&error.to_string());
            None
        }
    }
}

fn save_presets(app: &mut App, path: Option<&Path>) {
    if let Some(path) = path
        && let Err(error) = presets::save(path, &app.presets.items)
    {
        app.log(format!("Could not save presets: {error:#}"));
    }
}

/// Loads and measures everything, then renders one frame as text. This is the same screen
/// as the interactive view, without needing a terminal, so it can be piped, tested and
/// shared. With `run` set, it also presses Run and waits for the files to be written, going
/// through exactly the same checks and confirmation as a real key press.
pub fn snapshot(options: &Options, width: u16, height: u16) -> Result<String> {
    let (mut app, jobs) = prepare(options);
    let expected = jobs.len();
    let outcomes = engine::start(jobs, options.cache.as_ref());
    for _ in 0..expected {
        let outcome = outcomes
            .recv()
            .context("a measuring thread stopped unexpectedly")?;
        app.apply(outcome);
    }
    if options.run {
        run_and_wait(&mut app, options.yes)?;
    }
    Ok(view::render_to_string(&app, width, height))
}

/// Presses Run, answers the confirmation if there is one, and waits for the run to finish.
fn run_and_wait(app: &mut App, confirm: bool) -> Result<()> {
    app.handle_key(Key::Enter);
    if app.mode == Mode::ConfirmRun {
        if !confirm {
            bail!("this File Strategy overwrites files or deletes originals. Add --yes to confirm");
        }
        app.handle_key(Key::Char('y'));
    }
    let Some(spec) = app.take_run_spec() else {
        bail!(
            "nothing was run: {}",
            app.log.last().map_or("no reason given", String::as_str)
        );
    };
    let events = runner::start(spec, Ffmpeg::locate()?);
    loop {
        let event = events
            .recv()
            .context("the writing thread stopped unexpectedly")?;
        let done = event == RunEvent::AllDone;
        app.apply_run_event(event);
        if done {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_keys_become_shared_keys() {
        assert_eq!(translate(KeyCode::Char('q')), Some(Key::Char('q')));
        assert_eq!(translate(KeyCode::Enter), Some(Key::Enter));
        assert_eq!(translate(KeyCode::BackTab), Some(Key::BackTab));
        assert_eq!(translate(KeyCode::PageDown), Some(Key::PageDown));
        // Keys the screen has no use for are ignored.
        assert_eq!(translate(KeyCode::F(5)), None);
        assert_eq!(translate(KeyCode::Insert), None);
    }
}
