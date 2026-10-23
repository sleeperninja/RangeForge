//! The state behind the interactive view.
//!
//! Nothing here draws or spawns threads, so all of it can be tested directly.

use std::time::Duration;

use rangeforge_core::planner::{self, GainPlan, Measurement, PlanError, Target};
use ratatui::crossterm::event::KeyCode;

/// The output codec shown on track rows. A placeholder until the Output Format controls
/// exist.
pub const OUTPUT_CODEC: &str = "alac";

const TARGET_STEP: f64 = 0.5;
const CEILING_STEP: f64 = 0.1;
const TARGET_LIMITS: (f64, f64) = (-60.0, 0.0);
const CEILING_LIMITS: (f64, f64) = (-6.0, 0.0);
const PAGE: usize = 10;

fn tenths(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// The volume settings the user can change.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub target_lufs: f64,
    pub peak_protection: bool,
    pub peak_ceiling_dbtp: f64,
}

impl Default for Settings {
    /// The defaults shown in the mockup.
    fn default() -> Self {
        Self {
            target_lufs: -22.0,
            peak_protection: true,
            peak_ceiling_dbtp: -0.6,
        }
    }
}

impl Settings {
    /// Builds settings, keeping values inside their allowed ranges.
    pub fn new(target_lufs: f64, peak_protection: bool, peak_ceiling_dbtp: f64) -> Self {
        Self {
            target_lufs: tenths(target_lufs).clamp(TARGET_LIMITS.0, TARGET_LIMITS.1),
            peak_protection,
            peak_ceiling_dbtp: tenths(peak_ceiling_dbtp).clamp(CEILING_LIMITS.0, CEILING_LIMITS.1),
        }
    }

    /// What the planner needs.
    pub fn target(&self) -> Target {
        Target {
            loudness_lufs: self.target_lufs,
            peak_ceiling_dbtp: self.peak_protection.then_some(self.peak_ceiling_dbtp),
        }
    }

    /// Moves the target loudness by `steps` of half a LU.
    pub fn nudge_target(&mut self, steps: i32) {
        self.target_lufs = tenths(self.target_lufs + f64::from(steps) * TARGET_STEP)
            .clamp(TARGET_LIMITS.0, TARGET_LIMITS.1);
    }

    /// Moves the peak ceiling by `steps` of a tenth of a dB.
    pub fn nudge_ceiling(&mut self, steps: i32) {
        self.peak_ceiling_dbtp = tenths(self.peak_ceiling_dbtp + f64::from(steps) * CEILING_STEP)
            .clamp(CEILING_LIMITS.0, CEILING_LIMITS.1);
    }
}

/// Where a finished measurement came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Cache,
    Decoded,
}

/// How far along a file is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Pending,
    Done(Origin),
    Failed(String),
}

/// What kind of row group this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    CueAlbum,
    SingleFile,
}

/// One song.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackRow {
    pub label: String,
    pub measurement: Option<Measurement>,
}

/// An album, or a single file, with its songs.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub name: String,
    pub kind: Kind,
    /// The audio file's extension, such as `flac`.
    pub source_extension: String,
    pub tracks: Vec<TrackRow>,
    pub status: Status,
}

impl Group {
    /// The Codec column text for the parent row.
    pub fn codec_label(&self) -> String {
        match self.kind {
            Kind::CueAlbum => "Split(cue)".to_string(),
            Kind::SingleFile => format!("({})", self.source_extension),
        }
    }
}

/// A finished measurement job, sent back from a worker thread.
#[derive(Debug)]
pub struct MeasureOutcome {
    pub group: usize,
    pub result: Result<(Vec<Measurement>, Origin), String>,
    pub elapsed: Duration,
}

#[derive(Debug)]
pub struct App {
    pub groups: Vec<Group>,
    pub settings: Settings,
    pub show_log: bool,
    pub log: Vec<String>,
    /// The first table row shown.
    pub scroll: usize,
    pub should_quit: bool,
}

impl App {
    pub fn new(groups: Vec<Group>, log: Vec<String>, settings: Settings) -> Self {
        Self {
            groups,
            settings,
            show_log: false,
            log,
            scroll: 0,
            should_quit: false,
        }
    }

    /// Files still being measured.
    pub fn pending(&self) -> usize {
        self.groups
            .iter()
            .filter(|group| group.status == Status::Pending)
            .count()
    }

    /// Songs that a run would process: everything except files that failed.
    pub fn queued(&self) -> usize {
        self.groups
            .iter()
            .filter(|group| !matches!(group.status, Status::Failed(_)))
            .map(|group| group.tracks.len())
            .sum()
    }

    /// Total table rows: one per group, one per song, and one extra per failed group for its
    /// message.
    pub fn row_count(&self) -> usize {
        self.groups
            .iter()
            .map(|group| {
                let message = usize::from(matches!(group.status, Status::Failed(_)));
                1 + message + group.tracks.len()
            })
            .sum()
    }

    /// The volume change for a song under the current settings, once it is measured.
    pub fn plan(&self, track: &TrackRow) -> Option<Result<GainPlan, PlanError>> {
        track
            .measurement
            .map(|measurement| planner::plan_gain(measurement, self.settings.target()))
    }

    /// True if peak protection reduced the gain of any song.
    pub fn any_limited(&self) -> bool {
        self.groups
            .iter()
            .flat_map(|group| &group.tracks)
            .filter_map(|track| self.plan(track))
            .any(|plan| plan.is_ok_and(|plan| plan.limited_by_peak))
    }

    fn log(&mut self, line: String) {
        self.log.push(line);
    }

    /// Records a finished measurement.
    pub fn apply(&mut self, outcome: MeasureOutcome) {
        let Some(group) = self.groups.get_mut(outcome.group) else {
            return;
        };
        let name = group.name.clone();
        let line = match outcome.result {
            Ok((measurements, origin)) if measurements.len() == group.tracks.len() => {
                for (track, measurement) in group.tracks.iter_mut().zip(measurements) {
                    track.measurement = Some(measurement);
                }
                group.status = Status::Done(origin);
                match origin {
                    Origin::Cache => format!("Loudness scan: {name} (cached)"),
                    Origin::Decoded => format!(
                        "Loudness scan: {name} ({:.1} s)",
                        outcome.elapsed.as_secs_f64()
                    ),
                }
            }
            Ok((measurements, _)) => {
                let message = format!(
                    "measured {} track(s) but expected {}",
                    measurements.len(),
                    group.tracks.len()
                );
                group.status = Status::Failed(message.clone());
                format!("Failed: {name}: {message}")
            }
            Err(message) => {
                group.status = Status::Failed(message.clone());
                format!("Failed: {name}: {message}")
            }
        };
        self.log(line);
    }

    fn scroll_down(&mut self, rows: usize) {
        let last = self.row_count().saturating_sub(1);
        self.scroll = (self.scroll + rows).min(last);
    }

    fn scroll_up(&mut self, rows: usize) {
        self.scroll = self.scroll.saturating_sub(rows);
    }

    /// Reacts to a key press.
    pub fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Left => self.settings.nudge_target(-1),
            KeyCode::Right => self.settings.nudge_target(1),
            KeyCode::Char('[') => self.settings.nudge_ceiling(-1),
            KeyCode::Char(']') => self.settings.nudge_ceiling(1),
            KeyCode::Char('p') => self.settings.peak_protection = !self.settings.peak_protection,
            KeyCode::Char('l') => self.show_log = !self.show_log,
            KeyCode::Up => self.scroll_up(1),
            KeyCode::Down => self.scroll_down(1),
            KeyCode::PageUp => self.scroll_up(PAGE),
            KeyCode::PageDown => self.scroll_down(PAGE),
            KeyCode::Home => self.scroll = 0,
            KeyCode::End => self.scroll = self.row_count().saturating_sub(1),
            KeyCode::Enter => {
                self.log("Run is not built yet (roadmap phase 3)".to_string());
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measurement(lufs: f64, peak: f64) -> Measurement {
        Measurement {
            integrated_lufs: lufs,
            true_peak_dbtp: peak,
            loudness_range_lu: 10.0,
        }
    }

    fn row(label: &str, measurement: Option<Measurement>) -> TrackRow {
        TrackRow {
            label: label.to_string(),
            measurement,
        }
    }

    fn album(rows: Vec<TrackRow>) -> Group {
        Group {
            name: "Led Zeppelin - Houses of the Holy".to_string(),
            kind: Kind::CueAlbum,
            source_extension: "flac".to_string(),
            tracks: rows,
            status: Status::Pending,
        }
    }

    fn app_with(groups: Vec<Group>) -> App {
        App::new(groups, Vec::new(), Settings::default())
    }

    #[test]
    fn defaults_match_the_mockup() {
        let settings = Settings::default();
        assert!((settings.target_lufs - -22.0).abs() < 1e-9);
        assert!((settings.peak_ceiling_dbtp - -0.6).abs() < 1e-9);
        assert!(settings.peak_protection);
    }

    #[test]
    fn nudging_stays_tidy_and_inside_its_limits() {
        let mut settings = Settings::default();
        for _ in 0..7 {
            settings.nudge_ceiling(1);
        }
        // Seven steps of 0.1 from -0.6 would be 0.1 without a limit; 0.0 is the maximum.
        assert!(settings.peak_ceiling_dbtp.abs() < 1e-9);

        settings.nudge_target(1);
        assert!((settings.target_lufs - -21.5).abs() < 1e-9);
        for _ in 0..200 {
            settings.nudge_target(1);
        }
        assert!(settings.target_lufs.abs() < 1e-9);
        for _ in 0..300 {
            settings.nudge_target(-1);
        }
        assert!((settings.target_lufs - -60.0).abs() < 1e-9);
    }

    #[test]
    fn repeated_nudges_do_not_drift() {
        let mut settings = Settings::default();
        for _ in 0..30 {
            settings.nudge_ceiling(-1);
            settings.nudge_ceiling(1);
        }
        assert!((settings.peak_ceiling_dbtp - -0.6).abs() < 1e-12);
    }

    #[test]
    fn new_settings_are_clamped() {
        let settings = Settings::new(-99.0, true, 5.0);
        assert!((settings.target_lufs - -60.0).abs() < 1e-9);
        assert!(settings.peak_ceiling_dbtp.abs() < 1e-9);
    }

    #[test]
    fn plans_follow_the_settings_and_flag_peak_limits() {
        let mut app = app_with(vec![album(vec![
            row("1 - quiet", Some(measurement(-20.0, -1.0))),
            row("2 - loud", Some(measurement(-14.2, -1.65))),
            row("3 - pending", None),
        ])]);

        let plan = app.plan(&app.groups[0].tracks[1]).unwrap().unwrap();
        assert!((plan.gain_db - -7.8).abs() < 1e-9);
        assert!(app.plan(&app.groups[0].tracks[2]).is_none());
        assert!(!app.any_limited());

        // Raise the target enough that the quiet song would need more than its headroom.
        app.settings.target_lufs = -14.0;
        assert!(app.any_limited());
        // Turning protection off removes the limit.
        app.handle_key(KeyCode::Char('p'));
        assert!(!app.any_limited());
    }

    #[test]
    fn applying_a_result_fills_the_rows_and_logs_it() {
        let mut app = app_with(vec![album(vec![row("1 - a", None), row("2 - b", None)])]);
        assert_eq!(app.pending(), 1);

        app.apply(MeasureOutcome {
            group: 0,
            result: Ok((
                vec![measurement(-15.0, -3.0), measurement(-20.7, -1.9)],
                Origin::Cache,
            )),
            elapsed: Duration::ZERO,
        });

        assert_eq!(app.pending(), 0);
        assert_eq!(app.groups[0].status, Status::Done(Origin::Cache));
        assert!(app.groups[0].tracks.iter().all(|t| t.measurement.is_some()));
        assert!(app.log.last().unwrap().contains("(cached)"));
    }

    #[test]
    fn a_failed_or_mismatched_result_marks_the_group_failed() {
        let mut app = app_with(vec![
            album(vec![row("1 - a", None)]),
            album(vec![row("1 - a", None), row("2 - b", None)]),
        ]);
        app.apply(MeasureOutcome {
            group: 0,
            result: Err("file not found".to_string()),
            elapsed: Duration::ZERO,
        });
        app.apply(MeasureOutcome {
            group: 1,
            result: Ok((vec![measurement(-15.0, -3.0)], Origin::Decoded)),
            elapsed: Duration::ZERO,
        });

        assert_eq!(
            app.groups[0].status,
            Status::Failed("file not found".into())
        );
        assert!(matches!(app.groups[1].status, Status::Failed(_)));
        // Failed files are not part of a run.
        assert_eq!(app.queued(), 0);
    }

    #[test]
    fn counts_rows_and_queued_songs() {
        let mut app = app_with(vec![
            album(vec![row("1", None), row("2", None), row("3", None)]),
            album(vec![row("1", None)]),
        ]);
        assert_eq!(app.row_count(), 6);
        assert_eq!(app.queued(), 4);

        // A failed album still lists its songs, plus one line for the message, but a run
        // skips it.
        app.groups[1].status = Status::Failed("gone".to_string());
        assert_eq!(app.row_count(), 7);
        assert_eq!(app.queued(), 3);
    }

    #[test]
    fn keys_drive_the_settings_and_the_view() {
        let mut app = app_with(vec![album(
            (0..30).map(|n| row(&n.to_string(), None)).collect(),
        )]);

        app.handle_key(KeyCode::Right);
        assert!((app.settings.target_lufs - -21.5).abs() < 1e-9);
        app.handle_key(KeyCode::Left);
        app.handle_key(KeyCode::Left);
        assert!((app.settings.target_lufs - -22.5).abs() < 1e-9);

        app.handle_key(KeyCode::Char('l'));
        assert!(app.show_log);

        app.handle_key(KeyCode::PageDown);
        assert_eq!(app.scroll, 10);
        app.handle_key(KeyCode::End);
        assert_eq!(app.scroll, 30);
        app.handle_key(KeyCode::Down);
        assert_eq!(app.scroll, 30, "cannot scroll past the last row");
        app.handle_key(KeyCode::Home);
        assert_eq!(app.scroll, 0);
        app.handle_key(KeyCode::Up);
        assert_eq!(app.scroll, 0);

        app.handle_key(KeyCode::Enter);
        assert!(app.log.last().unwrap().contains("not built yet"));

        assert!(!app.should_quit);
        app.handle_key(KeyCode::Char('q'));
        assert!(app.should_quit);
    }

    #[test]
    fn codec_labels_follow_the_mockup() {
        let mut group = album(Vec::new());
        assert_eq!(group.codec_label(), "Split(cue)");
        group.kind = Kind::SingleFile;
        assert_eq!(group.codec_label(), "(flac)");
    }
}
