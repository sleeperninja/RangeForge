//! The interactive terminal view: a table of measurements with live volume targets.
//!
//! The layout follows the mockups in `docs/mockups`. The state lives in [`model`], drawing
//! in [`view`], loading in [`load`] and background measuring in [`engine`], so each part can
//! be tested without a terminal.

mod engine;
pub mod load;
pub mod model;
mod view;

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use anyhow::{Context, Result};
use rangeforge_audio::cache::Cache;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use model::{App, MeasureOutcome, Settings};

/// Everything the view needs to start.
#[derive(Debug)]
pub struct Options {
    pub paths: Vec<PathBuf>,
    pub settings: Settings,
    pub cache: Option<Cache>,
    pub show_log: bool,
}

/// Opens the interactive view and runs until the user quits.
pub fn run(options: &Options) -> Result<()> {
    let loaded = load::load(&options.paths);
    let mut app = App::new(loaded.groups, loaded.log, options.settings);
    app.show_log = options.show_log;
    let outcomes = engine::start(loaded.jobs, options.cache.as_ref());

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app, &outcomes);
    ratatui::restore();
    result
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    outcomes: &Receiver<MeasureOutcome>,
) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| view::draw(frame, app))?;
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key.code);
        }
        while let Ok(outcome) = outcomes.try_recv() {
            app.apply(outcome);
        }
    }
    Ok(())
}

/// Loads and measures everything, then renders one frame as text. This is the same screen
/// as the interactive view, without needing a terminal, so it can be piped, tested and
/// shared.
pub fn snapshot(options: &Options, width: u16, height: u16) -> Result<String> {
    let loaded = load::load(&options.paths);
    let expected = loaded.jobs.len();
    let mut app = App::new(loaded.groups, loaded.log, options.settings);
    app.show_log = options.show_log;
    let outcomes = engine::start(loaded.jobs, options.cache.as_ref());
    for _ in 0..expected {
        let outcome = outcomes
            .recv()
            .context("a measuring thread stopped unexpectedly")?;
        app.apply(outcome);
    }
    Ok(view::render_to_string(&app, width, height))
}
