//! Drawing the interactive view. It only reads the [`App`]; it never changes it.

use ratatui::backend::TestBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, List, ListItem, Paragraph, Row, Table};
use ratatui::{Frame, Terminal};

use rangeforge_session::controls::{ConflictKind, Focus, Mode, WhereKind};
use rangeforge_session::model::{App, Group, RunState, Status, TrackRow};

const LOG_WIDTH: u16 = 36;
/// Seven lines of controls plus the border.
const CONTROLS_HEIGHT: u16 = 9;

/// Draws the whole screen: the table, the controls strip, and the log panel if it is open.
pub fn draw(frame: &mut Frame, app: &App) {
    let [main, controls] =
        Layout::vertical([Constraint::Min(6), Constraint::Length(CONTROLS_HEIGHT)])
            .areas(frame.area());
    if app.show_log {
        let [table, log] =
            Layout::horizontal([Constraint::Min(60), Constraint::Length(LOG_WIDTH)]).areas(main);
        draw_table(frame, app, table);
        draw_log(frame, app, log);
    } else {
        draw_table(frame, app, main);
    }
    draw_controls(frame, app, controls);
}

/// Renders one frame as plain text, without a real terminal.
pub fn render_to_string(app: &App, width: u16, height: u16) -> String {
    // The in-memory test backend cannot fail; its error type has no values.
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).unwrap_or_else(|never| match never {});
    terminal
        .draw(|frame| draw(frame, app))
        .unwrap_or_else(|never| match never {});
    let buffer = terminal.backend().buffer();
    let lines: Vec<String> = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect();
    lines.join("\n")
}

fn number_cell(text: String) -> Cell<'static> {
    Cell::from(Line::from(text).right_aligned())
}

fn header() -> Row<'static> {
    Row::new(vec![
        Cell::from("Name"),
        Cell::from("Codec"),
        number_cell("Loudness (LUFS)".to_string()),
        number_cell("True Peak (dBTP)".to_string()),
        number_cell("Adjust (LUFS)".to_string()),
        number_cell("Range (LRA)".to_string()),
        Cell::from("  Tags"),
    ])
    .bold()
    .bottom_margin(1)
}

/// What a file's row says in the Loudness column: how far measuring or writing has got.
fn progress_cell(group: &Group) -> Cell<'static> {
    let right = |text: String| Cell::from(Line::from(text).right_aligned());
    match &group.run {
        RunState::Queued => right("queued".to_string()).dim(),
        RunState::Encoding { index, total } => {
            right(format!("encoding {}/{total}", index + 1)).yellow()
        }
        RunState::Saved => right("saved".to_string()).green(),
        RunState::Idle | RunState::Failed(_) => match group.status {
            Status::Pending => right("measuring...".to_string()).dim(),
            _ => Cell::from(""),
        },
    }
}

/// The album or file row, plus a line of its own for each failure so the message has room.
fn group_rows(group: &Group) -> Vec<Row<'static>> {
    let mut rows = vec![Row::new(vec![
        Cell::from(group.name.clone()).bold(),
        Cell::from(group.codec_label()),
        progress_cell(group),
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
    ])];
    if let Status::Failed(message) = &group.status {
        rows.push(Row::new(vec![Cell::from(format!("  ! {message}")).red()]));
    }
    if let RunState::Failed(message) = &group.run {
        rows.push(Row::new(vec![
            Cell::from(format!("  ! could not write: {message}")).red(),
        ]));
    }
    rows
}

fn track_row(app: &App, track: &TrackRow) -> Row<'static> {
    let measurement = track.measurement;
    let or_dash = |text: Option<String>| text.unwrap_or_else(|| "-".to_string());
    let loudness = or_dash(measurement.map(|m| format!("{:.2}", m.integrated_lufs)));
    let peak = or_dash(measurement.map(|m| format!("{:.2}", m.true_peak_dbtp)));
    let range = or_dash(measurement.map(|m| format!("{:.1}", m.loudness_range_lu)));

    let adjust = match app.plan(track) {
        None => Cell::from(Line::from("-").right_aligned()),
        Some(Err(_)) => Cell::from(Line::from("n/a").right_aligned()),
        Some(Ok(plan)) => {
            let marker = if plan.limited_by_peak { "*" } else { "" };
            let cell = number_cell(format!("{:+.2}{marker}", plan.gain_db));
            if plan.limited_by_peak {
                cell.style(Style::new().fg(Color::Yellow))
            } else {
                cell
            }
        }
    };

    // A tick marks a song whose output file has been written; a dash, one left out of the run.
    let (mark, name_style) = if !track.included {
        ("\u{2013}", Style::new().add_modifier(Modifier::DIM))
    } else if track.saved {
        ("\u{2713}", Style::new().fg(Color::Green))
    } else {
        ("\u{21b3}", Style::new())
    };
    Row::new(vec![
        Cell::from(format!("{mark} {}", track.label)).style(name_style),
        Cell::from(app.output.codec.name()),
        number_cell(loudness),
        number_cell(peak),
        adjust,
        number_cell(range),
        Cell::from(""),
    ])
}

fn draw_table(frame: &mut Frame, app: &App, area: Rect) {
    let rows = app.groups.iter().flat_map(|group| {
        group_rows(group)
            .into_iter()
            .chain(group.tracks.iter().map(|track| track_row(app, track)))
    });
    // The number columns are just wide enough for their headings. Name gets most of the
    // spare width, as in the mockup; Tags gets the rest.
    let widths = [
        Constraint::Fill(4),
        Constraint::Length(10),
        Constraint::Length(15),
        Constraint::Length(16),
        Constraint::Length(13),
        Constraint::Length(11),
        Constraint::Fill(1),
    ];
    let table = Table::new(rows.skip(app.scroll), widths)
        .header(header())
        .block(Block::bordered().title(" RangeForge "));
    frame.render_widget(table, area);
}

/// A value the user can change, shown in brackets, and highlighted when it has the keyboard.
fn control(text: impl Into<String>, focused: bool) -> Span<'static> {
    let text = format!("[{}]", text.into());
    if focused {
        Span::styled(
            text,
            Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD),
        )
    } else {
        Span::styled(text, Style::new().add_modifier(Modifier::BOLD))
    }
}

fn label(text: &str) -> Span<'static> {
    Span::from(text.to_string()).dim()
}

fn volume_line(app: &App) -> Line<'static> {
    let settings = app.settings;
    let protection = if settings.peak_protection {
        "on"
    } else {
        "off"
    };
    Line::from(vec![
        label("Volume       "),
        Span::from("Target Loudness "),
        control(
            format!("{:.1}", settings.target_lufs),
            app.focus == Focus::Target,
        ),
        Span::from(" LUFS     Peak Protection "),
        control(protection, app.focus == Focus::PeakProtection),
        Span::from("   Ceiling "),
        control(
            format!("{:.1}", settings.peak_ceiling_dbtp),
            app.focus == Focus::Ceiling,
        ),
        Span::from(" dBTP"),
    ])
}

fn output_line(app: &App) -> Line<'static> {
    let output = app.output;
    Line::from(vec![
        label("Output       "),
        Span::from("Codec "),
        control(output.codec.name(), app.focus == Focus::Codec),
        Span::from(format!("   Container {}", output.format().extension())),
        Span::from("   Bit Depth "),
        control(output.bit_depth.label(), app.focus == Focus::BitDepth),
        label(&format!(
            "   Sample Rate {}   Channels {}",
            output.sample_rate.label().to_lowercase(),
            output.mix.channels.name()
        )),
    ])
}

fn preset_line(app: &App) -> Line<'static> {
    let name = app
        .presets
        .selected_preset()
        .map_or_else(|| "custom".to_string(), |preset| preset.name.clone());
    let mut spans = vec![
        label("Preset       "),
        control(name, app.focus == Focus::Preset),
    ];
    if app.preset_modified() {
        spans.push(Span::from(" modified").yellow());
    }
    // Save is only on offer when there is something to save, as in the mockup.
    let can_save = app.can_save_preset();
    let save = Span::from("   s save");
    spans.push(if can_save { save } else { save.dim() });
    spans.push(if app.presets.selected.is_some() {
        Span::from("   d delete")
    } else {
        Span::from("   d delete").dim()
    });
    Line::from(spans)
}

/// One radio choice: filled when selected, bold when it also has the keyboard.
fn radio(text: &str, selected: bool, focused: bool) -> Span<'static> {
    let mark = if selected { "(\u{2022})" } else { "( )" };
    let text = format!("{mark} {text}");
    if selected && focused {
        Span::styled(
            text,
            Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD),
        )
    } else if selected {
        Span::styled(text, Style::new().add_modifier(Modifier::BOLD))
    } else {
        Span::from(text).dim()
    }
}

/// Where the finished files go.
fn place_line(app: &App) -> Line<'static> {
    let strategy = &app.strategy;
    let mut spans = vec![label("Save files    ")];
    for kind in WhereKind::ALL {
        let selected = strategy.where_kind == kind;
        spans.push(radio(kind.label(), selected, app.focus == Focus::Where));
        if selected && let Some(text) = strategy.where_text() {
            let text = if text.is_empty() {
                "choose a folder"
            } else {
                text
            };
            spans.push(Span::from(" "));
            spans.push(control(text.to_string(), app.focus == Focus::WhereText));
        }
        spans.push(Span::from("  "));
    }
    Line::from(spans)
}

/// What to do about a name that is taken, and whether to delete the original.
fn conflict_line(app: &App) -> Line<'static> {
    let strategy = &app.strategy;
    let mut spans = vec![label("If name taken ")];
    for kind in ConflictKind::ALL {
        let selected = strategy.conflict_kind == kind;
        spans.push(radio(kind.label(), selected, app.focus == Focus::Conflict));
        if selected && let Some(text) = strategy.conflict_text() {
            spans.push(Span::from(" "));
            spans.push(control(text.to_string(), app.focus == Focus::ConflictText));
        }
        spans.push(Span::from("  "));
    }
    spans.push(Span::from("Delete original "));
    spans.push(control(
        if strategy.delete_original {
            "yes"
        } else {
            "no"
        },
        app.focus == Focus::DeleteOriginal,
    ));
    Line::from(spans)
}

/// The bottom line: what the keys are doing, a problem to fix, or the keys that work.
fn prompt_line(app: &App) -> Line<'static> {
    match &app.mode {
        Mode::Editing(buffer) => Line::from(vec![
            Span::from("Editing: ").bold(),
            Span::from(format!("{buffer}\u{2588}")),
            Span::from("    Enter accept, Esc cancel").dim(),
        ]),
        Mode::NamingPreset(buffer) => Line::from(vec![
            Span::from("Preset name: ").bold(),
            Span::from(format!("{buffer}\u{2588}")),
            Span::from("    Enter save, Esc cancel").dim(),
        ]),
        Mode::ConfirmRun => Line::from(
            "This overwrites files or deletes originals. Run anyway? y = yes, any other key cancels",
        )
        .yellow()
        .bold(),
        Mode::Normal => match app.strategy.to_strategy() {
            Err(reason) => Line::from(format!("File Strategy: {reason}")).red(),
            Ok(_) => Line::from(
                "Tab next control   \u{2190}/\u{2192} change   e edit text   Enter run   l logs   \u{2191}/\u{2193} scroll   q quit",
            )
            .dim(),
        },
    }
}

fn draw_controls(frame: &mut Frame, app: &App, area: Rect) {
    let pending = app.pending();
    let status = if app.running {
        "writing files...".to_string()
    } else if pending == 0 {
        "all measured".to_string()
    } else {
        format!("measuring {pending} of {} file(s)...", app.groups.len())
    };
    let limited = if app.any_limited() {
        "   * limited by peak protection"
    } else {
        ""
    };
    let run_line = Line::from(vec![
        Span::from(format!("Run ({})", app.queued())).bold(),
        Span::from(format!("   {status}{limited}")),
    ]);

    let lines = vec![
        run_line,
        volume_line(app),
        output_line(app),
        preset_line(app),
        place_line(app),
        conflict_line(app),
        prompt_line(app),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Controls ")),
        area,
    );
}

fn draw_log(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::bordered().title(" DevLog ");
    let height = usize::from(block.inner(area).height);
    let skip = app.log.len().saturating_sub(height);
    let items: Vec<ListItem> = app
        .log
        .iter()
        .skip(skip)
        .map(|line| ListItem::new(format!("- {line}")))
        .collect();
    frame.render_widget(List::new(items).block(block), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rangeforge_core::planner::Measurement;
    use rangeforge_export::format::{BitDepth, Codec};
    use rangeforge_session::controls::PresetList;
    use rangeforge_session::model::{Kind, Origin, RunEvent, Settings};
    use rangeforge_session::presets::built_in;
    use std::path::PathBuf;

    fn measured(lufs: f64, peak: f64, range: f64) -> Measurement {
        Measurement {
            integrated_lufs: lufs,
            true_peak_dbtp: peak,
            loudness_range_lu: range,
        }
    }

    fn track(label: &str, measurement: Option<Measurement>) -> TrackRow {
        TrackRow {
            measurement,
            ..TrackRow::new(label.to_string())
        }
    }

    fn sample_app() -> App {
        let album = Group {
            name: "Led Zeppelin - Houses of the Holy".to_string(),
            kind: Kind::CueAlbum,
            source_extension: "flac".to_string(),
            tracks: vec![
                track(
                    "1 - The Song Remains the Same",
                    Some(measured(-14.2, -1.65, 11.0)),
                ),
                track("2 - The Rain Song", Some(measured(-19.53, -1.0, 14.0))),
            ],
            status: Status::Done(Origin::Decoded),
            run: RunState::Idle,
        };
        let single = Group {
            name: "14 - Shot at the Night.flac".to_string(),
            kind: Kind::SingleFile,
            source_extension: "flac".to_string(),
            tracks: vec![track("14 - Shot at the Night", None)],
            status: Status::Pending,
            run: RunState::Idle,
        };
        let failed = Group {
            name: "Missing Album".to_string(),
            kind: Kind::CueAlbum,
            source_extension: String::new(),
            tracks: vec![track("1 - Gone", None)],
            status: Status::Failed("audio file not found: gone.flac".to_string()),
            run: RunState::Idle,
        };
        let mut app = App::new(
            vec![album, single, failed],
            Vec::new(),
            vec!["Added LZ-HoTH.flac".to_string()],
            Settings::default(),
        );
        app.presets = PresetList::new(built_in());
        app.presets.select_matching(&app.settings, app.output);
        app
    }

    fn render(app: &App) -> String {
        render_to_string(app, 140, 26)
    }

    #[test]
    fn shows_the_table_like_the_mockup() {
        let screen = render(&sample_app());
        for expected in [
            "RangeForge",
            "Loudness (LUFS)",
            "Range (LRA)",
            "Led Zeppelin - Houses of the Holy",
            "Split(cue)",
            "(flac)",
            "\u{21b3} 1 - The Song Remains the Same",
            "alac",
            "-14.20",
            "-7.80",
            "-2.47",
        ] {
            assert!(
                screen.contains(expected),
                "missing {expected:?} in:\n{screen}"
            );
        }
    }

    #[test]
    fn shows_progress_failures_and_the_run_count() {
        let screen = render(&sample_app());
        assert!(screen.contains("measuring..."), "{screen}");
        assert!(
            screen.contains("! audio file not found: gone.flac"),
            "{screen}"
        );
        assert!(screen.contains("measuring 1 of 3 file(s)"), "{screen}");
        // Two measured songs plus the pending single; the failed album is excluded.
        assert!(screen.contains("Run (3)"), "{screen}");
    }

    #[test]
    fn the_controls_show_every_panel_from_the_mockup() {
        let screen = render(&sample_app());
        for expected in [
            "Target Loudness [-22.0] LUFS",
            "Peak Protection [on]",
            "Ceiling [-0.6] dBTP",
            "Codec [alac]",
            "Container m4a",
            "Bit Depth [source]",
            "Sample Rate original",
            "Channels original",
            "Preset       [iTunes Lossless]",
            "(\u{2022}) Next to the original",
            "( ) In a subfolder of the original's folder",
            "( ) All in one folder",
            "(\u{2022}) Add a number to the new file [_00]",
            "( ) Rename the existing file",
            "( ) Overwrite the existing file",
            "Delete original [no]",
        ] {
            assert!(
                screen.contains(expected),
                "missing {expected:?} in:\n{screen}"
            );
        }
    }

    #[test]
    fn the_output_panel_follows_the_codec_and_depth() {
        let mut app = sample_app();
        app.output.codec = Codec::Flac;
        app.output.bit_depth = BitDepth::TwentyFour;
        let screen = render(&app);
        assert!(screen.contains("Codec [flac]"), "{screen}");
        assert!(screen.contains("Container flac"), "{screen}");
        assert!(screen.contains("Bit Depth [24-bit]"), "{screen}");
        // Song rows name the codec they will be written in.
        assert!(screen.contains("flac"), "{screen}");
        assert!(!screen.contains("alac"), "{screen}");
    }

    #[test]
    fn a_changed_preset_says_so_and_a_custom_one_is_called_custom() {
        let mut app = sample_app();
        assert!(!render(&app).contains("modified"));
        app.settings.nudge_target(1);
        assert!(render(&app).contains("[iTunes Lossless] modified"));

        app.presets.selected = None;
        assert!(render(&app).contains("Preset       [custom]"));
    }

    #[test]
    fn the_strategy_panel_shows_the_field_of_the_selected_choice() {
        let mut app = sample_app();
        app.strategy.conflict_kind = ConflictKind::RenameExisting;
        let screen = render(&app);
        assert!(
            screen.contains("(\u{2022}) Rename the existing file [_old]"),
            "{screen}"
        );
        assert!(
            screen.contains("( ) Add a number to the new file"),
            "{screen}"
        );

        app.strategy.where_kind = WhereKind::Folder;
        assert!(render(&app).contains("[choose a folder]"));
        app.strategy.folder = "D:\\music\\out".to_string();
        assert!(render(&app).contains("(\u{2022}) All in one folder [D:\\music\\out]"));

        app.strategy.where_kind = WhereKind::Subfolder;
        assert!(
            render(&app)
                .contains("(\u{2022}) In a subfolder of the original's folder [RangeForge]")
        );

        app.strategy.conflict_kind = ConflictKind::Overwrite;
        app.strategy.delete_original = true;
        let screen = render(&app);
        assert!(
            screen.contains("(\u{2022}) Overwrite the existing file"),
            "{screen}"
        );
        assert!(screen.contains("Delete original [yes]"), "{screen}");
        assert!(!screen.contains("[_00]"), "{screen}");
    }

    #[test]
    fn a_song_left_out_is_dimmed_with_a_dash() {
        let mut app = sample_app();
        app.groups[0].tracks[1].included = false;
        let screen = render(&app);
        assert!(screen.contains("\u{2013} 2 - The Rain Song"), "{screen}");
        assert!(screen.contains("\u{21b3} 1 - The Song Remains"), "{screen}");
    }

    #[test]
    fn a_strategy_that_cannot_work_is_explained_in_red_text() {
        let mut app = sample_app();
        app.strategy.number_suffix = "_new".to_string();
        let screen = render(&app);
        assert!(
            screen.contains("File Strategy: the serial suffix must end in digits"),
            "{screen}"
        );
    }

    #[test]
    fn the_bottom_line_follows_the_mode() {
        let mut app = sample_app();
        assert!(render(&app).contains("Tab next control"));

        app.mode = Mode::Editing("_07".to_string());
        assert!(render(&app).contains("Editing: _07\u{2588}"));

        app.mode = Mode::NamingPreset("Car".to_string());
        assert!(render(&app).contains("Preset name: Car\u{2588}"));

        app.mode = Mode::ConfirmRun;
        assert!(render(&app).contains("Run anyway? y = yes"));
        // (The prompt names both ways a run can destroy something.)
    }

    #[test]
    fn rows_show_how_far_a_run_has_got() {
        let mut app = sample_app();
        app.groups[0].run = RunState::Queued;
        assert!(render(&app).contains("queued"));

        app.groups[0].run = RunState::Encoding { index: 1, total: 2 };
        app.running = true;
        let screen = render(&app);
        assert!(screen.contains("encoding 2/2"), "{screen}");
        assert!(screen.contains("writing files..."), "{screen}");

        app.apply_run_event(RunEvent::Saved {
            group: 0,
            index: 0,
            path: PathBuf::from("out/01.m4a"),
        });
        app.apply_run_event(RunEvent::Finished { group: 0 });
        let screen = render(&app);
        assert!(screen.contains("saved"), "{screen}");
        assert!(
            screen.contains("\u{2713} 1 - The Song Remains the Same"),
            "{screen}"
        );
        assert!(
            screen.contains("\u{21b3} 2 - The Rain Song"),
            "the unsaved song is not ticked"
        );
    }

    #[test]
    fn a_failed_write_gets_its_own_line() {
        let mut app = sample_app();
        app.groups[0].run = RunState::Failed("FFmpeg exited with 1".to_string());
        let screen = render(&app);
        assert!(
            screen.contains("! could not write: FFmpeg exited with 1"),
            "{screen}"
        );
    }

    #[test]
    fn changing_the_target_changes_the_adjust_column() {
        let mut app = sample_app();
        app.settings.target_lufs = -20.0;
        let screen = render(&app);
        // -20 minus -14.2 is -5.8, and minus -19.53 is -0.47.
        assert!(screen.contains("-5.80"), "{screen}");
        assert!(screen.contains("-0.47"), "{screen}");
        assert!(!screen.contains("-7.80"), "{screen}");
    }

    #[test]
    fn peak_limited_songs_are_starred_and_explained() {
        let mut app = sample_app();
        app.settings.target_lufs = -14.0;
        let screen = render(&app);
        assert!(screen.contains("limited by peak protection"), "{screen}");
        // The quiet song would need +5.53 dB but only has 0.40 dB of headroom.
        assert!(screen.contains("+0.40*"), "{screen}");
    }

    #[test]
    fn the_log_panel_appears_on_request() {
        let mut app = sample_app();
        assert!(!render(&app).contains("DevLog"));
        app.show_log = true;
        let screen = render(&app);
        assert!(screen.contains("DevLog"), "{screen}");
        assert!(screen.contains("- Added LZ-HoTH.flac"), "{screen}");
    }

    #[test]
    fn scrolling_hides_the_top_rows() {
        let mut app = sample_app();
        app.scroll = 1;
        let screen = render(&app);
        assert!(
            !screen.contains("Led Zeppelin - Houses of the Holy"),
            "{screen}"
        );
        assert!(screen.contains("The Song Remains the Same"), "{screen}");
    }

    #[test]
    fn a_tiny_window_does_not_panic() {
        let app = sample_app();
        for (width, height) in [(20, 6), (40, 10), (1, 1), (80, 7), (100, 9)] {
            let _ = render_to_string(&app, width, height);
        }
    }
}
