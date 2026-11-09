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

use rangeforge_core::identify::IdentitySource;
use rangeforge_core::mix::{Channels, Downmix};
use rangeforge_export::format::{BitDepth, Codec, Container, FLAC_LEVELS};

use crate::controls::{ConflictKind, OutputSettings, WhereKind, sample_rates_for};
use crate::dsd::DsdFormat;
use crate::model::{App, Group, Kind, RunState, Status, TrackRow};
use crate::naming::{self, NamingPreview};
use crate::secrets::KeySource;

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
    /// Labels from outside lookups, shown in the Tags column.
    pub tags: Vec<TagChip>,
    /// Who the song is, and how that was found out.
    pub identity: Option<IdentityView>,
    /// What a single file will be written as, without its extension, when the file name
    /// pattern makes that something other than its own name.
    pub output_name: Option<String>,
    /// That name rests on a guess from the file's name and folders, not on its tags or on
    /// names typed in by the user.
    pub output_guessed: bool,
}

/// Who a song is, for the window to show on hover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityView {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    /// Where it came from: `hand`, `cue`, `tags`, or `name` (a guess from the file name).
    pub source: &'static str,
}

/// One label in the Tags column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagChip {
    pub text: String,
    /// What colour to give it: `genre` (plain) for the whole album, `song` (pink) for what
    /// stands out about one song, `radio` (blue) for the Hot 100.
    pub kind: &'static str,
    /// What to say when the pointer rests on it.
    pub title: String,
}

/// An album or a single file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupView {
    pub name: String,
    /// What the Codec column says on this row: `Split(cue)` or the source format in brackets.
    pub codec_label: String,
    /// The codec this group's songs will be written in. Differs from the main Output Format
    /// for DSD files when the DSD Format setting names a codec of its own.
    pub output_codec: String,
    /// The folder the audio file is in. Songs only gather under one album heading if they share
    /// it, so a stereo and a six-channel folder of the same album stay apart.
    pub folder: String,
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
    /// False when the chosen output can hold sound above full scale (32-bit float WAV): the
    /// protection does not apply, and the window shows it unticked and dimmed.
    pub protection_applies: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutputView {
    pub codec: String,
    /// `native` or `mka`.
    pub container: String,
    /// `original`, or the rate in hertz.
    pub sample_rate: String,
    pub bit_depth: String,
    /// `original`, `quad`, `stereo` or `mono`.
    pub channels: String,
    /// `standard`, `bass` or `front`; it matters when the channels are not original.
    pub downmix: String,
    /// The lossy bitrate, such as `320` or `v0`. Only a lossy codec uses it.
    pub bitrate: String,
    /// FLAC's compression level, `0` to `8`. Only FLAC uses it.
    pub flac_level: u8,
    /// The ending of the files that will be written: `m4a`, `flac`, `wav` or `mka`.
    pub extension: &'static str,
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
    /// The user wants to be asked before such a run. "Don't ask again" turns it off, and
    /// Settings turns it back on. `needs_confirmation` says whether this run is one.
    pub confirm_first: bool,
}

/// What DSD files fall back to where the Output Format says source. Everything else about a DSD
/// file follows the Output Format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DsdView {
    pub sample_rate: String,
    pub bit_depth: String,
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

/// A variable a file name pattern can use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariableView {
    /// As typed in a pattern, braces included: `{title}`.
    pub key: String,
    pub meaning: String,
}

/// The file name patterns, for the Settings window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NamingView {
    pub album: String,
    pub single: String,
    /// What the two saved patterns produce for a made-up song.
    pub example: NamingPreview,
    pub variables: Vec<VariableView>,
}

/// The outside lookups, for the main screen's switch and the Settings window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MetadataView {
    /// The "Metadata API" switch.
    pub enabled: bool,
    /// "Use Last.fm" beside its key.
    pub lastfm_enabled: bool,
    /// Whether a Last.fm key is available: `saved`, `environment` or `none`. The key itself is
    /// never sent to the window.
    pub lastfm_key: &'static str,
    /// How many songs the Hot 100 index holds, once it has loaded.
    pub radio_songs: Option<usize>,
}

/// The lists of choices a window offers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Options {
    pub codecs: Vec<Choice>,
    pub bit_depths: Vec<Choice>,
    /// The usual file for the chosen codec, and `mka`.
    pub containers: Vec<Choice>,
    /// Only the sample rates the chosen codec can write.
    pub sample_rates: Vec<Choice>,
    /// How many channels to write. Quad is left out for a codec that cannot hold it.
    pub channels: Vec<Choice>,
    pub downmixes: Vec<Choice>,
    /// The bitrates the chosen codec offers; empty for a lossless one, whose picker is dimmed.
    pub bitrates: Vec<Choice>,
    /// FLAC's compression levels, 0 (largest) to 8 (smallest).
    pub flac_levels: Vec<Choice>,
    pub places: Vec<Choice>,
    pub conflicts: Vec<Choice>,
    /// For the DSD Format: the Output Format's own choice, or a codec of its own.
    /// Real rates only: DSD has no original rate to keep.
    pub dsd_sample_rates: Vec<Choice>,
    pub dsd_bit_depths: Vec<Choice>,
}

/// The whole screen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot {
    pub groups: Vec<GroupView>,
    pub volume: VolumeView,
    pub output: OutputView,
    pub strategy: StrategyView,
    pub dsd: DsdView,
    pub presets: PresetsView,
    pub naming: NamingView,
    pub metadata: MetadataView,
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

pub fn place_id(kind: WhereKind) -> &'static str {
    match kind {
        WhereKind::NextToOriginal => "next",
        WhereKind::Subfolder => "subfolder",
        WhereKind::Folder => "folder",
    }
}

pub fn conflict_id(kind: ConflictKind) -> &'static str {
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

/// The labels for the Tags column, in the order they are written: the album's genres (plain),
/// what stands out about the song (pink), then Radio (blue).
fn chips(app: &App, track: &TrackRow) -> Vec<TagChip> {
    let genres = app.song_genres(track);
    let mut chips: Vec<TagChip> = genres
        .album
        .into_iter()
        .map(|text| TagChip {
            text,
            kind: "genre",
            title: "A genre of the whole album".to_string(),
        })
        .chain(genres.song.into_iter().map(|text| TagChip {
            text,
            kind: "song",
            title: "A genre that stands out for this song".to_string(),
        }))
        .collect();
    chips.extend(genres.mine.into_iter().map(|text| TagChip {
        text,
        kind: "mine",
        title: "Added by you".to_string(),
    }));
    if let Some(hit) = track.radio.filter(|_| app.shows_radio(track)) {
        chips.push(TagChip {
            text: "Radio".to_string(),
            kind: "radio",
            title: format!(
                "Reached #{} on the Billboard Hot 100 ({})",
                hit.peak, hit.year
            ),
        });
    }
    chips
}

fn track_view(
    app: &App,
    group_index: usize,
    group: &Group,
    track_index: usize,
    track: &TrackRow,
) -> TrackView {
    let (adjust_db, limited, unadjustable) = match app.plan(track) {
        Some(Ok(plan)) => (finite(plan.gain_db), plan.limited_by_peak, false),
        Some(Err(_)) => (None, false, true),
        None => (None, false, false),
    };
    let output_name = app.output_stem(group_index, group, track_index, track);
    let output_guessed = output_name.is_some()
        && track
            .identity
            .as_ref()
            .is_some_and(|identity| identity.source == IdentitySource::FileName);
    TrackView {
        label: track.label.clone(),
        output_name,
        output_guessed,
        loudness_lufs: track.measurement.and_then(|m| finite(m.integrated_lufs)),
        true_peak_dbtp: track.measurement.and_then(|m| finite(m.true_peak_dbtp)),
        range_lu: track.measurement.and_then(|m| finite(m.loudness_range_lu)),
        adjust_db,
        limited,
        unadjustable,
        saved: track.saved,
        included: track.included,
        tags: chips(app, track),
        identity: track.identity.as_ref().map(|identity| IdentityView {
            artist: identity.artist.clone(),
            title: identity.title.clone(),
            album: identity.album.clone(),
            album_artist: identity.album_artist.clone(),
            source: match identity.source {
                IdentitySource::Hand => "hand",
                IdentitySource::CueSheet => "cue",
                IdentitySource::FileTags => "tags",
                IdentitySource::FileName => "name",
            },
        }),
    }
}

fn group_view(app: &App, index: usize, group: &Group) -> GroupView {
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
        output_codec: app
            .dsd
            .format_for(
                app.output.format(),
                std::path::Path::new(&format!("x.{}", group.source_extension)),
            )
            .codec
            .name()
            .to_string(),
        folder: app
            .jobs
            .iter()
            .find(|job| job.group == index)
            .and_then(|job| job.audio.parent())
            .map(|folder| folder.display().to_string())
            .unwrap_or_default(),
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
            .enumerate()
            .map(|(track_index, track)| track_view(app, index, group, track_index, track))
            .collect(),
    }
}

#[allow(clippy::too_many_lines)] // one list of choices after another
fn options(output: OutputSettings) -> Options {
    Options {
        codecs: Codec::ALL
            .into_iter()
            .map(|codec| Choice {
                id: codec.name().to_string(),
                label: codec.name().to_string(),
            })
            .collect(),
        // The usual file's ending for the chosen codec, and mka.
        containers: Container::ALL
            .into_iter()
            .map(|container| Choice {
                id: container.name().to_string(),
                label: container.label(output.codec).to_string(),
            })
            .collect(),
        // Only the rates the chosen codec can write.
        channels: Channels::ALL
            .into_iter()
            .filter(|channels| *channels != Channels::Quad || output.codec.can_write_quad())
            .map(|channels| Choice {
                id: channels.name().to_string(),
                label: channels.label().to_string(),
            })
            .collect(),
        downmixes: Downmix::ALL
            .into_iter()
            .map(|style| Choice {
                id: style.name().to_string(),
                label: style.label().to_string(),
            })
            .collect(),
        bitrates: output
            .codec
            .bitrates()
            .iter()
            .map(|rate| Choice {
                id: rate.name(),
                label: rate.label(),
            })
            .collect(),
        flac_levels: FLAC_LEVELS
            .into_iter()
            .map(|level| Choice {
                id: level.to_string(),
                label: match level {
                    0 => "0 (largest)".to_string(),
                    5 => "5 (usual)".to_string(),
                    8 => "8 (smallest)".to_string(),
                    other => other.to_string(),
                },
            })
            .collect(),
        sample_rates: sample_rates_for(output.codec)
            .into_iter()
            .map(|rate| Choice {
                id: rate.name().to_string(),
                label: rate.label().to_string(),
            })
            .collect(),
        bit_depths: BitDepth::ALL
            .into_iter()
            .filter(|depth| depth.fits(output.codec))
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
        dsd_sample_rates: DsdFormat::SAMPLE_RATES
            .into_iter()
            .map(|rate| Choice {
                id: rate.name().to_string(),
                label: rate.label().to_string(),
            })
            .collect(),
        dsd_bit_depths: DsdFormat::BIT_DEPTHS
            .into_iter()
            .map(|depth| Choice {
                id: depth.name().to_string(),
                label: depth.label().to_string(),
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
                .enumerate()
                .map(|(index, group)| group_view(self, index, group))
                .collect(),
            volume: VolumeView {
                target_lufs: self.settings.target_lufs,
                peak_protection: self.settings.peak_protection,
                ceiling_dbtp: self.settings.peak_ceiling_dbtp,
                protection_applies: !self.output.format().keeps_overs(),
            },
            output: OutputView {
                codec: self.output.codec.name().to_string(),
                container: self.output.container.name().to_string(),
                sample_rate: self.output.shown_sample_rate().name().to_string(),
                bit_depth: self.output.bit_depth.name().to_string(),
                channels: self.output.mix.channels.name().to_string(),
                downmix: self.output.mix.style.name().to_string(),
                bitrate: self.output.bitrate.name(),
                flac_level: self.output.flac_level,
                extension: self.output.format().extension(),
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
                confirm_first: self.confirm_destructive,
            },
            dsd: DsdView {
                sample_rate: self.dsd.sample_rate.name().to_string(),
                bit_depth: self.dsd.bit_depth.name().to_string(),
            },
            presets: PresetsView {
                names: self.presets.items.iter().map(|p| p.name.clone()).collect(),
                selected: self.presets.selected,
                modified: self.preset_modified(),
                can_save: self.can_save_preset(),
                can_delete: self.presets.selected.is_some(),
            },
            naming: NamingView {
                album: self.naming.album.clone(),
                single: self.naming.single.clone(),
                example: naming::preview(
                    &self.naming.album,
                    &self.naming.single,
                    self.output.format().extension(),
                ),
                variables: naming::variables()
                    .into_iter()
                    .map(|(key, meaning)| VariableView { key, meaning })
                    .collect(),
            },
            metadata: MetadataView {
                enabled: self.metadata.enabled,
                lastfm_enabled: self.metadata.lastfm_enabled,
                lastfm_key: match self.lastfm_key {
                    KeySource::Saved => "saved",
                    KeySource::Environment => "environment",
                    KeySource::None => "none",
                },
                radio_songs: self.radio_songs(),
            },
            options: options(self.output),
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
    use rangeforge_export::format::SampleRate;

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
    fn a_single_file_shows_the_name_the_pattern_will_give_it() {
        use rangeforge_core::identify::Identity;
        use std::path::Path;

        let path = Path::new(
            "/audio/Doves (lossless downloads from vinyls)/2005 - Some Cities/00724387460928_001_Doves_Some Cities_GBDGG0400080.m4a",
        );
        let label = "00724387460928_001_Doves_Some Cities_GBDGG0400080";
        let single = Group {
            name: format!("{label}.m4a"),
            kind: Kind::SingleFile,
            source_extension: "m4a".to_string(),
            tracks: vec![TrackRow {
                identity: Some(Identity::guess_from_path(path)),
                ..track(label, Some(measured(-20.0, -3.0, 8.0)))
            }],
            status: Status::Done(Origin::Decoded),
            run: RunState::Idle,
        };
        let mut app = App::new(vec![single], Vec::new(), Vec::new(), Settings::default());
        let name = |app: &App| app.snapshot().groups[0].tracks[0].clone();

        // The usual pattern keeps the file's own name, so there is nothing to show.
        assert_eq!(name(&app).output_name, None);
        assert!(!name(&app).output_guessed);

        // A pattern from the tags shows the name the run will write, and says it is a guess
        // while it rests on the file name.
        app.set_naming(
            "{track:02} - {title}",
            "{artist}-{album}_{track:02}-{title}",
        )
        .unwrap();
        let shown = name(&app);
        assert_eq!(
            shown.output_name.as_deref(),
            Some("Doves-Some Cities_01-Some Cities")
        );
        assert!(shown.output_guessed);
        assert_eq!(shown.label, label, "the original name is still known");

        // Names typed in by hand are no guess.
        let typed = crate::model::Names {
            title: "Some Cities".to_string(),
            artist: "Doves".to_string(),
            album: "Some Cities".to_string(),
            // Typing in something new is what makes it the user's own name; typing in what was
            // already guessed changes nothing.
            album_artist: "Doves".to_string(),
        };
        app.set_names(
            &[crate::model::Pick {
                group: 0,
                track: Some(0),
            }],
            &typed,
            true,
        )
        .unwrap();
        assert!(!name(&app).output_guessed);
        assert!(name(&app).output_name.is_some());

        // A song cut from a CUE sheet keeps its label.
        let cue = sample_app().snapshot();
        assert!(cue.groups[0].tracks.iter().all(|t| t.output_name.is_none()));
    }

    #[test]
    fn a_cue_track_shows_the_name_it_will_be_written_as_with_typed_in_names_on_top() {
        use crate::load::Job;
        use crate::model::{Names, Pick};
        use rangeforge_core::cue;
        use rangeforge_core::identify::Identity;
        use std::path::PathBuf;

        let sheet = cue::parse(
            "PERFORMER \"The Band\"
TITLE \"Album\"
FILE \"in.flac\" WAVE
  TRACK 01 AUDIO
    TITLE \"First\"
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    TITLE \"Second\"
    INDEX 01 01:00:00
",
        )
        .unwrap();
        let file = sheet.files[0].clone();
        let tracks: Vec<TrackRow> = file
            .tracks
            .iter()
            .map(|t| TrackRow {
                identity: Some(Identity::from_cue(&sheet, t)),
                ..track(
                    &format!("{} - {}", t.number, t.title.clone().unwrap()),
                    Some(measured(-20.0, -3.0, 8.0)),
                )
            })
            .collect();
        let group = Group {
            name: "The Band - Album".to_string(),
            kind: Kind::CueAlbum,
            source_extension: "flac".to_string(),
            tracks,
            status: Status::Done(Origin::Decoded),
            run: RunState::Idle,
        };
        let job = Job {
            group: 0,
            audio: PathBuf::from("music/in.flac"),
            cue_file: Some(file),
            sheet: Some(sheet),
            mix: rangeforge_core::mix::MixPlan::default(),
        };
        let mut app = App::new(vec![group], vec![job], Vec::new(), Settings::default());
        let names = |app: &App| -> Vec<Option<String>> {
            app.snapshot().groups[0]
                .tracks
                .iter()
                .map(|t| t.output_name.clone())
                .collect()
        };

        // The label says "1 - First"; the file will be "01 - First".
        assert_eq!(
            names(&app),
            [
                Some("01 - First".to_string()),
                Some("02 - Second".to_string())
            ]
        );
        assert!(
            app.snapshot().groups[0]
                .tracks
                .iter()
                .all(|t| !t.output_guessed)
        );

        // A title typed in changes that song's name and keeps the rip's number.
        let typed = Names {
            title: "Second (Remastered)".to_string(),
            ..Names::default()
        };
        app.set_names(
            &[Pick {
                group: 0,
                track: Some(1),
            }],
            &typed,
            true,
        )
        .unwrap();
        assert_eq!(
            names(&app),
            [
                Some("01 - First".to_string()),
                Some("02 - Second (Remastered)".to_string())
            ]
        );
    }

    #[test]
    fn the_channel_choices_follow_the_codec_and_the_panel_says_what_is_chosen() {
        let mut app = sample_app();
        let ids = |choices: &[Choice]| choices.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        let snapshot = app.snapshot();
        assert_eq!(
            (
                snapshot.output.channels.as_str(),
                snapshot.output.downmix.as_str()
            ),
            ("original", "standard")
        );
        // ALAC, as the app starts, cannot hold quad, so it is not offered.
        assert_eq!(
            ids(&snapshot.options.channels),
            ["original", "stereo", "mono"]
        );
        assert_eq!(
            ids(&snapshot.options.downmixes),
            ["standard", "bass", "front"]
        );

        app.set_codec(Codec::Flac).unwrap();
        assert_eq!(
            ids(&app.snapshot().options.channels),
            ["original", "quad", "stereo", "mono"]
        );
        app.set_channels(Channels::Stereo).unwrap();
        app.set_downmix(rangeforge_core::mix::Downmix::WithBass)
            .unwrap();
        let snapshot = app.snapshot();
        assert_eq!(
            (
                snapshot.output.channels.as_str(),
                snapshot.output.downmix.as_str()
            ),
            ("stereo", "bass")
        );
        serde_json::to_string(&snapshot).unwrap();
    }

    #[test]
    fn the_settings_panels_are_described() {
        let snapshot = sample_app().snapshot();
        assert!((snapshot.volume.target_lufs - -22.0).abs() < 1e-9);
        assert!(snapshot.volume.peak_protection);
        assert_eq!(snapshot.output.codec, "alac");
        assert_eq!(snapshot.output.container, "native");
        assert_eq!(snapshot.output.extension, "m4a");
        assert_eq!(snapshot.output.sample_rate, "original");
        assert_eq!(snapshot.output.bit_depth, "source");
        assert_eq!(snapshot.strategy.place, "next");
        assert_eq!(snapshot.strategy.conflict, "number");
        assert_eq!(snapshot.strategy.number_suffix, "_00");
        assert_eq!(snapshot.strategy.subfolder, "_forged");
        assert!(!snapshot.strategy.delete_original);
        assert_eq!(snapshot.strategy.problem, None);
        assert!(!snapshot.strategy.needs_confirmation);
        assert_eq!(snapshot.log, ["Added x"]);
    }

    #[test]
    fn thirty_two_bit_float_needs_no_peak_protection_and_the_setting_comes_back() {
        let mut app = sample_app();
        app.set_codec(Codec::Wav).unwrap();
        assert!(app.snapshot().volume.protection_applies);
        app.set_bit_depth(BitDepth::Float32).unwrap();
        let view = app.snapshot().volume;
        assert!(!view.protection_applies);
        assert!(view.peak_protection, "the setting itself is not touched");
        // The planner has no ceiling for the float file, and has one again afterwards.
        let target = |app: &App| {
            app.settings
                .target_for(app.output.format().keeps_overs())
                .peak_ceiling_dbtp
        };
        assert_eq!(target(&app), None);
        app.set_bit_depth(BitDepth::TwentyFour).unwrap();
        assert!(app.snapshot().volume.protection_applies);
        assert_eq!(target(&app), Some(-0.6));
    }

    #[test]
    fn thirty_two_bit_float_is_offered_for_wav_alone() {
        let mut app = sample_app();
        let depths = |app: &App| {
            app.snapshot()
                .options
                .bit_depths
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(depths(&app), ["source", "16", "24"]);
        app.set_codec(Codec::Wav).unwrap();
        assert_eq!(depths(&app), ["source", "16", "24", "32f"]);
        app.set_bit_depth(BitDepth::Float32).unwrap();
        assert_eq!(app.snapshot().output.bit_depth, "32f");
        // Another codec refuses it, and leaving WAV puts the depth back to the source's.
        app.set_codec(Codec::Flac).unwrap();
        assert_eq!(app.snapshot().output.bit_depth, "source");
        assert!(app.set_bit_depth(BitDepth::Float32).is_err());
    }

    #[test]
    fn the_choices_to_offer_come_from_the_session() {
        let options = sample_app().snapshot().options;
        let ids = |choices: &[Choice]| choices.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        assert_eq!(
            ids(&options.codecs),
            ["alac", "flac", "wav", "aac", "mp3", "opus"]
        );
        assert_eq!(ids(&options.bit_depths), ["source", "16", "24"]);
        assert_eq!(ids(&options.places), ["next", "subfolder", "folder"]);
        assert_eq!(
            ids(&options.conflicts),
            ["number", "rename_existing", "overwrite"]
        );
        assert_eq!(options.conflicts[1].label, "Rename existing file");
        assert_eq!(options.bit_depths[1].label, "16-bit");
    }

    #[test]
    fn the_containers_and_rates_offered_follow_the_chosen_codec() {
        let mut app = sample_app();
        let options = app.snapshot().options;
        let ids = |choices: &[Choice]| choices.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        let labels =
            |choices: &[Choice]| choices.iter().map(|c| c.label.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&options.containers), ["native", "mka"]);
        assert_eq!(labels(&options.containers), ["m4a", "mka"]);
        assert_eq!(options.sample_rates.len(), 7);
        assert_eq!(options.sample_rates[0].label, "source");
        assert_eq!(options.sample_rates[3].label, "88.2 kHz");

        app.set_codec(Codec::Flac).unwrap();
        assert_eq!(labels(&app.snapshot().options.containers), ["flac", "mka"]);

        app.set_codec(Codec::Aac).unwrap();
        let rates = ids(&app.snapshot().options.sample_rates);
        assert_eq!(rates, ["original", "44100", "48000", "88200", "96000"]);
    }

    #[test]
    fn the_chosen_container_and_rate_show_and_name_the_files() {
        let mut app = sample_app();
        app.set_container(Container::Mka);
        app.set_sample_rate(SampleRate::Hz96000).unwrap();
        let output = app.snapshot().output;
        assert_eq!(output.container, "mka");
        assert_eq!(output.sample_rate, "96000");
        assert_eq!(output.extension, "mka");
        // The naming example uses the same ending.
        let example = app.snapshot().naming.example.album_example.unwrap();
        assert_eq!(example.rsplit('.').next(), Some("mka"), "{example}");
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
    fn the_name_patterns_come_with_examples_and_the_variable_list() {
        let mut app = sample_app();
        let naming = app.snapshot().naming;
        assert_eq!(naming.album, "{track:02} - {title}");
        assert_eq!(naming.single, "{name}");
        assert_eq!(
            naming.example.album_example.as_deref(),
            Some("07 - No Quarter.m4a")
        );
        assert_eq!(naming.variables.len(), 8);
        assert!(naming.variables.iter().any(|v| v.key == "{albumartist}"));

        // The example follows the chosen format's extension.
        app.set_codec(Codec::Flac).unwrap();
        app.set_naming("{artist} - {title}", "{name}").unwrap();
        let naming = app.snapshot().naming;
        assert_eq!(
            naming.example.album_example.as_deref(),
            Some("Led Zeppelin - No Quarter.flac")
        );
    }

    #[test]
    fn the_radio_tag_and_the_lookup_switches_reach_the_window() {
        let mut app = sample_app();
        let snapshot = app.snapshot();
        assert!(!snapshot.metadata.enabled);
        assert!(!snapshot.metadata.lastfm_enabled);
        assert_eq!(snapshot.metadata.lastfm_key, "none");
        assert_eq!(snapshot.metadata.radio_songs, None);
        assert_eq!(snapshot.groups[0].tracks[1].tags, Vec::<TagChip>::new());

        app.set_metadata_enabled(true);
        app.lastfm_key = KeySource::Environment;
        app.groups[0].tracks[1].radio = Some(crate::model::RadioHit {
            peak: 14,
            year: 1973,
        });
        let snapshot = app.snapshot();
        assert!(snapshot.metadata.enabled);
        assert_eq!(snapshot.metadata.lastfm_key, "environment");
        let chip = &snapshot.groups[0].tracks[1].tags[0];
        assert_eq!(chip.text, "Radio");
        assert_eq!(chip.kind, "radio");
        assert_eq!(chip.title, "Reached #14 on the Billboard Hot 100 (1973)");
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
