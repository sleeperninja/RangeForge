//! The whole screen as plain data, for a window to display.
//!
//! A window cannot look inside [`App`], so it asks for a [`Snapshot`]: every row, every
//! setting, and every list of choices, already worked out. All numbers a window shows (the
//! gain of each song, whether peak protection limited it) are computed here, by the same code
//! the terminal view uses, so the two cannot disagree.
//!
//! Numbers that are not finite, such as the loudness of silence, become `None`, because JSON
//! cannot hold infinity.

use serde::Serialize;

use rangeforge_export::format::{BitDepth, Codec};

use crate::controls::{ConflictKind, WhereKind};
use crate::model::{App, Group, Kind, RunState, Status, TrackRow};

fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

/// One song as a window shows it. The flags are independent facts a window shows separately.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackView {
    pub label: String,
    pub loudness_lufs: Option<f64>,
    pub true_peak_dbtp: Option<f64>,
    pub range_lu: Option<f64>,
    /// The gain that will be applied, in dB, once the song is measured.
    pub adjust_db: Option<f64>,
    /// Peak protection held the gain back, so the song stays below the target.
    pub limited: bool,
    /// The song is measured but cannot be adjusted, such as silence.
    pub unadjustable: bool,
    /// Its output file has been written.
    pub saved: bool,
    /// It is part of the next run. The user can switch songs off.
    pub included: bool,
}

/// An album or a single file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupView {
    pub name: String,
    /// What the Codec column says on this row: `Split(cue)` or the source format in brackets.
    pub codec_label: String,
    /// `cue` or `single`.
    pub kind: &'static str,
    pub measuring: bool,
    /// Why measuring failed, if it did.
    pub failure: Option<String>,
    /// How far writing has got: `queued`, `encoding 3/8`, `saved`, or empty.
    pub run_text: String,
    pub run_state: &'static str,
    /// Why writing failed, if it did.
    pub run_failure: Option<String>,
    pub tracks: Vec<TrackView>,
}

/// A choice in a list: its saved name and what to show.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choice {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VolumeView {
    pub target_lufs: f64,
    pub peak_protection: bool,
    pub ceiling_dbtp: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutputView {
    pub codec: String,
    pub container: &'static str,
    pub bit_depth: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StrategyView {
    /// Where files go: `next`, `subfolder` or `folder`.
    pub place: String,
    pub subfolder: String,
    pub folder: String,
    /// What to do about a name that is taken: `number`, `rename_existing` or `overwrite`.
    pub conflict: String,
    pub number_suffix: String,
    pub old_suffix: String,
    pub delete_original: bool,
    /// Why the current choice cannot be used yet, such as a missing folder.
    pub problem: Option<String>,
    /// Running with this choice should ask the user first.
    pub needs_confirmation: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PresetsView {
    pub names: Vec<String>,
    pub selected: Option<usize>,
    /// The settings have been changed from the selected preset.
    pub modified: bool,
    pub can_save: bool,
    pub can_delete: bool,
}

/// The lists of choices a window offers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Options {
    pub codecs: Vec<Choice>,
    pub bit_depths: Vec<Choice>,
    pub places: Vec<Choice>,
    pub conflicts: Vec<Choice>,
}

/// The whole screen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot {
    pub groups: Vec<GroupView>,
    pub volume: VolumeView,
    pub output: OutputView,
    pub strategy: StrategyView,
    pub presets: PresetsView,
    pub options: Options,
    pub running: bool,
    /// Files still being measured, and how many there are in all.
    pub pending_files: usize,
    pub total_files: usize,
    /// Songs a run would write.
    pub queued_songs: usize,
    /// Some song is held back by peak protection.
    pub any_limited: bool,
    /// Why Run cannot be pressed now, or `None` when it can.
    pub run_blocker: Option<String>,
    pub log: Vec<String>,
}

fn place_id(kind: WhereKind) -> &'static str {
    match kind {
        WhereKind::NextToOriginal => "next",
        WhereKind::Subfolder => "subfolder",
        WhereKind::Folder => "folder",
    }
}

fn conflict_id(kind: ConflictKind) -> &'static str {
    match kind {
        ConflictKind::Number => "number",
        ConflictKind::RenameExisting => "rename_existing",
        ConflictKind::Overwrite => "overwrite",
    }
}

/// The place for files with this id, as used by a window.
#[must_use]
pub fn place_from_id(id: &str) -> Option<WhereKind> {
    WhereKind::ALL
        .into_iter()
        .find(|kind| place_id(*kind) == id)
}

/// The way of handling a taken name with this id, as used by a window.
#[must_use]
pub fn conflict_from_id(id: &str) -> Option<ConflictKind> {
    ConflictKind::ALL
        .into_iter()
        .find(|kind| conflict_id(*kind) == id)
}

fn track_view(app: &App, track: &TrackRow) -> TrackView {
    let (adjust_db, limited, unadjustable) = match app.plan(track) {
        Some(Ok(plan)) => (finite(plan.gain_db), plan.limited_by_peak, false),
        Some(Err(_)) => (None, false, true),
        None => (None, false, false),
    };
    TrackView {
        label: track.label.clone(),
        loudness_lufs: track.measurement.and_then(|m| finite(m.integrated_lufs)),
        true_peak_dbtp: track.measurement.and_then(|m| finite(m.true_peak_dbtp)),
        range_lu: track.measurement.and_then(|m| finite(m.loudness_range_lu)),
        adjust_db,
        limited,
        unadjustable,
        saved: track.saved,
        included: track.included,
    }
}

fn group_view(app: &App, group: &Group) -> GroupView {
    let (run_text, run_state, run_failure) = match &group.run {
        RunState::Idle => (String::new(), "idle", None),
        RunState::Queued => ("queued".to_string(), "queued", None),
        RunState::Encoding { index, total } => {
            (format!("encoding {}/{total}", index + 1), "encoding", None)
        }
        RunState::Saved => ("saved".to_string(), "saved", None),
        RunState::Failed(message) => (String::new(), "failed", Some(message.clone())),
    };
    GroupView {
        name: group.name.clone(),
        codec_label: group.codec_label(),
        kind: match group.kind {
            Kind::CueAlbum => "cue",
            Kind::SingleFile => "single",
        },
        measuring: group.status == Status::Pending,
        failure: match &group.status {
            Status::Failed(message) => Some(message.clone()),
            _ => None,
        },
        run_text,
        run_state,
        run_failure,
        tracks: group
            .tracks
            .iter()
            .map(|track| track_view(app, track))
            .collect(),
    }
}

fn options() -> Options {
    Options {
        codecs: Codec::ALL
            .into_iter()
            .map(|codec| Choice {
                id: codec.name().to_string(),
                label: codec.name().to_string(),
            })
            .collect(),
        bit_depths: BitDepth::ALL
            .into_iter()
            .map(|depth| Choice {
                id: depth.name().to_string(),
                label: depth.label().to_string(),
            })
            .collect(),
        places: WhereKind::ALL
            .into_iter()
            .map(|kind| Choice {
                id: place_id(kind).to_string(),
                label: kind.label().to_string(),
            })
            .collect(),
        conflicts: ConflictKind::ALL
            .into_iter()
            .map(|kind| Choice {
                id: conflict_id(kind).to_string(),
                label: kind.label().to_string(),
            })
            .collect(),
    }
}

impl App {
    /// The whole screen as plain data.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            groups: self
                .groups
                .iter()
                .map(|group| group_view(self, group))
                .collect(),
            volume: VolumeView {
                target_lufs: self.settings.target_lufs,
                peak_protection: self.settings.peak_protection,
                ceiling_dbtp: self.settings.peak_ceiling_dbtp,
            },
            output: OutputView {
                codec: self.output.codec.name().to_string(),
                container: self.output.codec.extension(),
                bit_depth: self.output.bit_depth.name().to_string(),
            },
            strategy: StrategyView {
                place: place_id(self.strategy.where_kind).to_string(),
                subfolder: self.strategy.subfolder.clone(),
                folder: self.strategy.folder.clone(),
                conflict: conflict_id(self.strategy.conflict_kind).to_string(),
                number_suffix: self.strategy.number_suffix.clone(),
                old_suffix: self.strategy.old_suffix.clone(),
                delete_original: self.strategy.delete_original,
                problem: self.strategy.to_strategy().err(),
                needs_confirmation: self.strategy.needs_confirmation(),
            },
            presets: PresetsView {
                names: self.presets.items.iter().map(|p| p.name.clone()).collect(),
                selected: self.presets.selected,
                modified: self.preset_modified(),
                can_save: self.can_save_preset(),
                can_delete: self.presets.selected.is_some(),
            },
            options: options(),
            running: self.running,
            pending_files: self.pending(),
            total_files: self.groups.len(),
            queued_songs: self.queued(),
            any_limited: self.any_limited(),
            run_blocker: self.run_blocker(),
            log: self.log.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::PresetList;
    use crate::model::{Origin, Settings};
    use crate::presets::built_in;
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
                track("2 - The Rain Song", Some(measured(-20.66, -1.91, 12.2))),
                track(
                    "3 - Silence",
                    Some(measured(f64::NEG_INFINITY, f64::NEG_INFINITY, 0.0)),
                ),
                track("4 - Pending", None),
            ],
            status: Status::Done(Origin::Decoded),
            run: RunState::Idle,
        };
        let mut app = App::new(
            vec![album],
            Vec::new(),
            vec!["Added x".to_string()],
            Settings::default(),
        );
        app.presets = PresetList::new(built_in());
        app.presets.select_matching(&app.settings, app.output);
        app
    }

    #[test]
    fn songs_carry_their_measurements_and_gains() {
        let snapshot = sample_app().snapshot();
        let tracks = &snapshot.groups[0].tracks;

        assert_eq!(tracks[0].loudness_lufs, Some(-14.2));
        assert!((tracks[0].adjust_db.unwrap() - -7.8).abs() < 1e-9);
        assert!(!tracks[0].limited);
        assert_eq!(tracks[0].range_lu, Some(11.0));
    }

    #[test]
    fn a_song_held_back_by_peak_protection_is_flagged() {
        let mut app = sample_app();
        app.set_target(-16.0);
        let snapshot = app.snapshot();
        // The Rain Song wants +4.66 dB but has only 1.31 dB of headroom.
        let rain = &snapshot.groups[0].tracks[1];
        assert!(rain.limited);
        assert!((rain.adjust_db.unwrap() - 1.31).abs() < 1e-9);
        assert!(snapshot.any_limited);
    }

    #[test]
    fn silence_and_unmeasured_songs_become_empty_not_infinite() {
        let snapshot = sample_app().snapshot();
        let silence = &snapshot.groups[0].tracks[2];
        assert_eq!(silence.loudness_lufs, None);
        assert_eq!(silence.true_peak_dbtp, None);
        assert!(silence.unadjustable);

        let pending = &snapshot.groups[0].tracks[3];
        assert_eq!(pending.loudness_lufs, None);
        assert_eq!(pending.adjust_db, None);
        assert!(!pending.unadjustable);
        // Everything must survive being turned into JSON.
        serde_json::to_string(&snapshot).unwrap();
    }

    #[test]
    fn the_settings_panels_are_described() {
        let snapshot = sample_app().snapshot();
        assert!((snapshot.volume.target_lufs - -22.0).abs() < 1e-9);
        assert!(snapshot.volume.peak_protection);
        assert_eq!(snapshot.output.codec, "alac");
        assert_eq!(snapshot.output.container, "m4a");
        assert_eq!(snapshot.output.bit_depth, "source");
        assert_eq!(snapshot.strategy.place, "next");
        assert_eq!(snapshot.strategy.conflict, "number");
        assert_eq!(snapshot.strategy.number_suffix, "_00");
        assert_eq!(snapshot.strategy.subfolder, "RangeForge");
        assert!(!snapshot.strategy.delete_original);
        assert_eq!(snapshot.strategy.problem, None);
        assert!(!snapshot.strategy.needs_confirmation);
        assert_eq!(snapshot.log, ["Added x"]);
    }

    #[test]
    fn the_choices_to_offer_come_from_the_session() {
        let options = sample_app().snapshot().options;
        let ids = |choices: &[Choice]| choices.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&options.codecs), ["alac", "flac", "wav", "aac"]);
        assert_eq!(ids(&options.bit_depths), ["source", "16", "24"]);
        assert_eq!(ids(&options.places), ["next", "subfolder", "folder"]);
        assert_eq!(
            ids(&options.conflicts),
            ["number", "rename_existing", "overwrite"]
        );
        assert_eq!(options.conflicts[1].label, "Rename the existing file");
        assert_eq!(options.bit_depths[1].label, "16-bit");
    }

    #[test]
    fn strategy_ids_round_trip() {
        for kind in WhereKind::ALL {
            assert_eq!(place_from_id(place_id(kind)), Some(kind));
        }
        for kind in ConflictKind::ALL {
            assert_eq!(conflict_from_id(conflict_id(kind)), Some(kind));
        }
        assert_eq!(place_from_id("nonsense"), None);
        assert_eq!(conflict_from_id("nonsense"), None);
    }

    #[test]
    fn presets_report_what_can_be_done() {
        let mut app = sample_app();
        let presets = app.snapshot().presets;
        assert_eq!(presets.names.len(), 5);
        assert_eq!(presets.selected, Some(1));
        assert!(!presets.modified);
        assert!(!presets.can_save, "nothing has changed yet");
        assert!(presets.can_delete);

        app.set_target(-20.0);
        let presets = app.snapshot().presets;
        assert!(presets.modified);
        assert!(presets.can_save);
    }

    #[test]
    fn a_strategy_that_cannot_work_says_why_and_blocks_run() {
        let mut app = sample_app();
        app.set_where(WhereKind::Folder);
        let snapshot = app.snapshot();
        assert_eq!(
            snapshot.strategy.problem.as_deref(),
            Some("choose a folder to save to")
        );

        app.set_where(WhereKind::NextToOriginal);
        app.set_conflict(ConflictKind::Overwrite);
        assert!(app.snapshot().strategy.needs_confirmation);
    }

    #[test]
    fn songs_report_whether_they_are_switched_on() {
        let mut app = sample_app();
        assert!(app.snapshot().groups[0].tracks[1].included);
        app.set_track_included(0, 1, false);
        let snapshot = app.snapshot();
        assert!(!snapshot.groups[0].tracks[1].included);
        assert_eq!(snapshot.queued_songs, 3);
    }

    #[test]
    fn run_progress_is_described() {
        let mut app = sample_app();
        app.groups[0].run = RunState::Encoding { index: 1, total: 4 };
        app.running = true;
        let snapshot = app.snapshot();
        assert_eq!(snapshot.groups[0].run_text, "encoding 2/4");
        assert_eq!(snapshot.groups[0].run_state, "encoding");
        assert!(snapshot.running);
        assert_eq!(
            snapshot.run_blocker.as_deref(),
            Some("A run is already in progress")
        );

        app.groups[0].run = RunState::Failed("FFmpeg failed".to_string());
        let snapshot = app.snapshot();
        assert_eq!(
            snapshot.groups[0].run_failure.as_deref(),
            Some("FFmpeg failed")
        );
        assert_eq!(snapshot.groups[0].run_state, "failed");
    }

    #[test]
    fn counts_and_groups_describe_the_list() {
        let mut app = sample_app();
        app.groups[0].status = Status::Pending;
        let snapshot = app.snapshot();
        assert!(snapshot.groups[0].measuring);
        assert_eq!(snapshot.pending_files, 1);
        assert_eq!(snapshot.total_files, 1);
        assert_eq!(snapshot.groups[0].codec_label, "Split(cue)");
        assert_eq!(snapshot.groups[0].kind, "cue");

        app.groups[0].status = Status::Failed("gone".to_string());
        assert_eq!(app.snapshot().groups[0].failure.as_deref(), Some("gone"));
    }
}
