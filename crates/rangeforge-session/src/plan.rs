//! Working out each song's gain and what to write for it. Shared by `rangeforge run` and the
//! interactive view, so both always make the same plan.

use std::path::{Path, PathBuf};

use rangeforge_core::cue::{CueSheet, CueTrack};
use rangeforge_core::identify::{Identity, IdentitySource};
use rangeforge_core::planner::{GainPlan, Measurement, PlanError, Target, plan_gain};
use rangeforge_export::format::OutputFormat;
use rangeforge_export::pipeline::TrackPlan;
use rangeforge_export::tags::Tags;
use rangeforge_export::template::NameValues;

use crate::load::Job;
use crate::naming::NameTemplates;

/// One song with its measurement and the gain chosen for it.
#[derive(Debug, Clone)]
pub struct Planned {
    pub label: String,
    pub measurement: Measurement,
    pub gain: Result<GainPlan, PlanError>,
}

/// The name of a file without its extension.
pub fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
}

/// The label of each song in a job: `7 - No Quarter` for a CUE track, or the file's name.
pub fn labels(job: &Job) -> Vec<String> {
    match &job.cue_file {
        Some(file) => file
            .tracks
            .iter()
            .map(|track| {
                format!(
                    "{} - {}",
                    track.number,
                    track.title.as_deref().unwrap_or("(untitled)")
                )
            })
            .collect(),
        None => vec![file_stem(&job.audio)],
    }
}

/// Pairs each measurement with its label and works out its gain for `target`.
pub fn plan_tracks(job: &Job, measurements: &[Measurement], target: Target) -> Vec<Planned> {
    labels(job)
        .into_iter()
        .zip(measurements)
        .map(|(label, &measurement)| Planned {
            label,
            measurement,
            gain: plan_gain(measurement, target),
        })
        .collect()
}

/// The tags to write for one track of a CUE album. The sheet gives the starting values and the
/// track number and total, which come from the rip and stay as they are. Names the user typed
/// in (the identity is `Hand`) replace the sheet's title, artist, album and album artist.
pub fn cue_tags(
    sheet: &CueSheet,
    track: &CueTrack,
    total_tracks: usize,
    identity: Option<&Identity>,
) -> Tags {
    let mut tags = Tags::for_cue_track(sheet, track, total_tracks);
    if let Some(identity) = identity.filter(|identity| identity.source == IdentitySource::Hand) {
        tags.title.clone_from(&identity.title);
        tags.artist.clone_from(&identity.artist);
        tags.album.clone_from(&identity.album);
        tags.album_artist.clone_from(&identity.album_artist);
    }
    tags
}

/// What to write for each song, named by `names`, and the folder the source lives in.
pub fn track_plans(
    job: &Job,
    planned: &[Planned],
    format: OutputFormat,
    names: &NameTemplates,
    identities: &[Option<Identity>],
) -> (Vec<TrackPlan>, PathBuf) {
    let extension = format.extension();
    let source_dir = job
        .audio
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    // A song that cannot be measured (silence) is passed through untouched.
    let gain_of = |planned: &Planned| planned.gain.as_ref().map_or(0.0, |plan| plan.gain_db);

    let source_name = file_stem(&job.audio);
    let plans = match (&job.cue_file, &job.sheet) {
        (Some(file), Some(sheet)) => file
            .tracks
            .iter()
            .zip(planned)
            .enumerate()
            .map(|(index, (track, planned))| {
                let identity = identities.get(index).and_then(Option::as_ref);
                let tags = cue_tags(sheet, track, file.tracks.len(), identity);
                let values = NameValues::from_tags(&tags, &source_name);
                TrackPlan {
                    gain_db: gain_of(planned),
                    file_name: names.album.file_name(&values, extension),
                    tags,
                    skip: false,
                }
            })
            .collect(),
        _ => planned
            .iter()
            .enumerate()
            .map(|(index, planned)| {
                // A single file is named from what is known about its song, if anything is.
                let values = match identities.get(index).and_then(Option::as_ref) {
                    Some(identity) => NameValues::from_identity(identity, &source_name),
                    None => NameValues {
                        name: Some(source_name.clone()),
                        ..NameValues::default()
                    },
                };
                TrackPlan {
                    gain_db: gain_of(planned),
                    file_name: names.single.file_name(&values, extension),
                    tags: Tags::default(),
                    skip: false,
                }
            })
            .collect(),
    };
    (plans, source_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rangeforge_core::cue;
    use rangeforge_export::format::{BitDepth, Codec};

    const SHEET: &str = "PERFORMER \"The Band\"\nTITLE \"Album\"\nFILE \"in.flac\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"First\"\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    TITLE \"Second\"\n    INDEX 01 01:00:00\n";

    fn cue_job() -> Job {
        let sheet = cue::parse(SHEET).unwrap();
        Job {
            group: 0,
            audio: PathBuf::from("music/album/in.flac"),
            cue_file: Some(sheet.files[0].clone()),
            sheet: Some(sheet),
            mix: rangeforge_core::mix::MixPlan::default(),
        }
    }

    fn single_job() -> Job {
        Job {
            group: 0,
            audio: PathBuf::from("music/14 - Shot at the Night.flac"),
            cue_file: None,
            sheet: None,
            mix: rangeforge_core::mix::MixPlan::default(),
        }
    }

    fn measurement(lufs: f64, peak: f64) -> Measurement {
        Measurement {
            integrated_lufs: lufs,
            true_peak_dbtp: peak,
            loudness_range_lu: 8.0,
        }
    }

    const TARGET: Target = Target {
        loudness_lufs: -22.0,
        peak_ceiling_dbtp: Some(-0.6),
    };

    fn alac() -> OutputFormat {
        OutputFormat::new(Codec::Alac, BitDepth::Source)
    }

    #[test]
    fn cue_songs_are_labelled_by_number_and_title() {
        assert_eq!(labels(&cue_job()), ["1 - First", "2 - Second"]);
        assert_eq!(labels(&single_job()), ["14 - Shot at the Night"]);
    }

    #[test]
    fn gains_follow_the_target() {
        let measurements = [measurement(-14.2, -1.65), measurement(-19.53, -1.0)];
        let planned = plan_tracks(&cue_job(), &measurements, TARGET);
        assert_eq!(planned.len(), 2);
        let first = planned[0].gain.as_ref().unwrap().gain_db;
        assert!((first - -7.8).abs() < 1e-9);
    }

    #[test]
    fn cue_albums_get_numbered_tagged_plans_next_to_the_image() {
        let job = cue_job();
        let planned = plan_tracks(
            &job,
            &[measurement(-20.0, -2.0), measurement(-20.0, -2.0)],
            TARGET,
        );
        let (plans, dir) = track_plans(&job, &planned, alac(), &NameTemplates::default(), &[]);

        assert_eq!(dir, PathBuf::from("music/album"));
        assert_eq!(plans[0].file_name, "01 - First.m4a");
        assert_eq!(plans[1].file_name, "02 - Second.m4a");
        assert_eq!(plans[1].tags.track, Some(2));
        assert_eq!(plans[1].tags.album.as_deref(), Some("Album"));
    }

    #[test]
    fn names_typed_in_replace_a_cue_sheets_but_the_rip_keeps_the_numbers() {
        use rangeforge_core::identify::IdentitySource;

        let job = cue_job();
        let planned = plan_tracks(
            &job,
            &[measurement(-20.0, -2.0), measurement(-20.0, -2.0)],
            TARGET,
        );
        let sheet = job.sheet.as_ref().unwrap();
        let tracks = &job.cue_file.as_ref().unwrap().tracks;
        // The sheet's own values are the identity until the user changes something.
        let mut typed = Identity::from_cue(sheet, &tracks[1]);
        assert_eq!(typed.source, IdentitySource::CueSheet);
        let (plans, _) = track_plans(
            &job,
            &planned,
            alac(),
            &NameTemplates::default(),
            &[None, Some(typed.clone())],
        );
        assert_eq!(plans[1].tags.title.as_deref(), Some("Second"));

        // Typed in: title, artist, album and album artist change; the number does not, and
        // neither does the other track.
        typed.title = Some("Second (Remastered)".to_string());
        typed.artist = Some("Someone Else".to_string());
        typed.album = Some("Right Album".to_string());
        typed.album_artist = Some("Right Band".to_string());
        typed.track = Some(9);
        typed.source = IdentitySource::Hand;
        let (plans, _) = track_plans(
            &job,
            &planned,
            alac(),
            &NameTemplates::default(),
            &[None, Some(typed)],
        );
        assert_eq!(plans[1].tags.title.as_deref(), Some("Second (Remastered)"));
        assert_eq!(plans[1].tags.artist.as_deref(), Some("Someone Else"));
        assert_eq!(plans[1].tags.album.as_deref(), Some("Right Album"));
        assert_eq!(plans[1].tags.album_artist.as_deref(), Some("Right Band"));
        assert_eq!(plans[1].tags.track, Some(2), "the rip's number stays");
        assert_eq!(plans[1].tags.track_total, Some(2));
        assert_eq!(plans[1].file_name, "02 - Second (Remastered).m4a");
        assert_eq!(plans[0].tags.title.as_deref(), Some("First"));
        assert_eq!(plans[0].tags.album.as_deref(), Some("Album"));
    }

    #[test]
    fn single_files_keep_their_name_and_their_own_tags() {
        let job = single_job();
        let planned = plan_tracks(&job, &[measurement(-20.0, -2.0)], TARGET);
        let (plans, dir) = track_plans(&job, &planned, alac(), &NameTemplates::default(), &[]);

        assert_eq!(dir, PathBuf::from("music"));
        assert_eq!(plans[0].file_name, "14 - Shot at the Night.m4a");
        assert_eq!(plans[0].tags, Tags::default());
    }

    #[test]
    fn the_patterns_decide_the_names() {
        use crate::naming::NameSettings;
        let names = NameSettings {
            album: "{artist} - {album} - {track:02} {title}".to_string(),
            single: "{name} (balanced)".to_string(),
        }
        .templates()
        .unwrap();

        let job = cue_job();
        let planned = plan_tracks(
            &job,
            &[measurement(-20.0, -2.0), measurement(-20.0, -2.0)],
            TARGET,
        );
        let (plans, _) = track_plans(&job, &planned, alac(), &names, &[]);
        assert_eq!(plans[0].file_name, "The Band - Album - 01 First.m4a");

        let job = single_job();
        let planned = plan_tracks(&job, &[measurement(-20.0, -2.0)], TARGET);
        let (plans, _) = track_plans(&job, &planned, alac(), &names, &[]);
        assert_eq!(plans[0].file_name, "14 - Shot at the Night (balanced).m4a");
    }

    #[test]
    fn a_silent_song_passes_through_untouched() {
        let job = single_job();
        let planned = plan_tracks(
            &job,
            &[measurement(f64::NEG_INFINITY, f64::NEG_INFINITY)],
            TARGET,
        );
        assert!(planned[0].gain.is_err());
        let (plans, _) = track_plans(&job, &planned, alac(), &NameTemplates::default(), &[]);
        assert!(plans[0].gain_db.abs() < f64::EPSILON);
    }
}
