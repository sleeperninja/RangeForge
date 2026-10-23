//! Drawing the interactive view. It only reads the [`App`]; it never changes it.

use ratatui::backend::TestBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, List, ListItem, Paragraph, Row, Table};
use ratatui::{Frame, Terminal};

use super::model::{App, Group, OUTPUT_CODEC, Status, TrackRow};

const LOG_WIDTH: u16 = 36;
const CONTROLS_HEIGHT: u16 = 5;

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

/// The album or file row, plus a line of its own for a failure so the message has room.
fn group_rows(group: &Group) -> Vec<Row<'static>> {
    // While a file is being measured, its row says so where the numbers will appear.
    let loudness = match group.status {
        Status::Pending => Cell::from(Line::from("measuring...").right_aligned()).dim(),
        _ => Cell::from(""),
    };
    let mut rows = vec![Row::new(vec![
        Cell::from(group.name.clone()).bold(),
        Cell::from(group.codec_label()),
        loudness,
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
    ])];
    if let Status::Failed(message) = &group.status {
        rows.push(Row::new(vec![Cell::from(format!("  ! {message}")).red()]));
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

    Row::new(vec![
        Cell::from(format!("\u{21b3} {}", track.label)),
        Cell::from(OUTPUT_CODEC),
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

fn draw_controls(frame: &mut Frame, app: &App, area: Rect) {
    let settings = app.settings;
    let pending = app.pending();
    let progress = if pending == 0 {
        "all measured".to_string()
    } else {
        format!("measuring {pending} of {} file(s)...", app.groups.len())
    };
    let limited = if app.any_limited() {
        "   * limited by peak protection"
    } else {
        ""
    };
    let protection = if settings.peak_protection {
        format!("on, {:.1} dBTP", settings.peak_ceiling_dbtp)
    } else {
        "off".to_string()
    };

    let lines = vec![
        Line::from(vec![
            Span::from(format!("Run ({})", app.queued())).bold(),
            Span::from(format!("   {progress}{limited}")),
        ]),
        Line::from(vec![
            Span::from("Target Loudness "),
            Span::from(format!("{:.1} LUFS", settings.target_lufs)).bold(),
            Span::from("  (\u{2190}/\u{2192})      Peak Protection "),
            Span::from(protection).bold(),
            Span::from("  (p, [ and ])"),
        ]),
        Line::from(
            "\u{2191}/\u{2193} PgUp/PgDn scroll   l logs   Enter run (not built yet)   q quit",
        )
        .dim(),
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
    use crate::ui::model::{Kind, Origin, Settings};
    use rangeforge_core::planner::Measurement;

    fn measured(lufs: f64, peak: f64, range: f64) -> Measurement {
        Measurement {
            integrated_lufs: lufs,
            true_peak_dbtp: peak,
            loudness_range_lu: range,
        }
    }

    fn track(label: &str, measurement: Option<Measurement>) -> TrackRow {
        TrackRow {
            label: label.to_string(),
            measurement,
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
        };
        let single = Group {
            name: "14 - Shot at the Night.flac".to_string(),
            kind: Kind::SingleFile,
            source_extension: "flac".to_string(),
            tracks: vec![track("14 - Shot at the Night", None)],
            status: Status::Pending,
        };
        let failed = Group {
            name: "Missing Album".to_string(),
            kind: Kind::CueAlbum,
            source_extension: String::new(),
            tracks: vec![track("1 - Gone", None)],
            status: Status::Failed("audio file not found: gone.flac".to_string()),
        };
        App::new(
            vec![album, single, failed],
            vec!["Added LZ-HoTH.flac".to_string()],
            Settings::default(),
        )
    }

    fn render(app: &App) -> String {
        render_to_string(app, 130, 22)
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
    fn controls_show_the_settings() {
        let screen = render(&sample_app());
        assert!(screen.contains("Target Loudness -22.0 LUFS"), "{screen}");
        assert!(screen.contains("on, -0.6 dBTP"), "{screen}");
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
        for (width, height) in [(20, 6), (40, 10), (1, 1), (80, 7)] {
            let _ = render_to_string(&app, width, height);
        }
    }
}
