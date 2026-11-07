//! Turns paths given on the command line into table rows and measurement jobs.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use rangeforge_core::cue::{CueFile, CueSheet};
use rangeforge_core::identify::Identity;

use crate::measuring::read_sheet;
use crate::model::{Group, Kind, RunState, Status, TrackRow};

const AUDIO_EXTENSIONS: [&str; 9] = [
    "flac", "wav", "m4a", "mp3", "ogg", "aac", "aiff", "dsf", "dff",
];

/// One file to measure.
#[derive(Debug, Clone)]
pub struct Job {
    /// Index into the list of groups.
    pub group: usize,
    pub audio: PathBuf,
    /// The CUE layout, for an album. `None` measures the whole file as one song.
    pub cue_file: Option<CueFile>,
    /// The whole CUE sheet, which holds the album-level details used for tags.
    pub sheet: Option<CueSheet>,
}

/// What loading produced.
#[derive(Debug, Default)]
pub struct Loaded {
    pub groups: Vec<Group>,
    pub jobs: Vec<Job>,
    pub log: Vec<String>,
    /// The index the first of these groups will have in the screen's list, so jobs can point
    /// at the right group when files are added to a list that already has some.
    pub first_group: usize,
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn is_cue(path: &Path) -> bool {
    extension_of(path) == "cue"
}

fn is_audio(path: &Path) -> bool {
    AUDIO_EXTENSIONS.contains(&extension_of(path).as_str())
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Finds the audio file a CUE sheet refers to. Sheets often name a `.wav` that has since
/// become a `.flac`, so if the exact name is missing, the same name with another audio
/// extension is accepted.
pub fn resolve_audio(cue_dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = cue_dir.join(name);
    if exact.is_file() {
        return Some(exact);
    }
    let stem = Path::new(name).file_stem()?;
    AUDIO_EXTENSIONS.iter().find_map(|extension| {
        let mut candidate: OsString = stem.to_os_string();
        candidate.push(".");
        candidate.push(extension);
        let path = cue_dir.join(candidate);
        path.is_file().then_some(path)
    })
}

/// How many folders down from a dropped folder are searched. An album ripped from a disc often
/// sits three deep: `Artist - Album / 1977 - Album / 6ch`. Four covers a disc folder as well.
const MAX_DEPTH: usize = 4;

/// The folder this program's own output goes in, unless the user named another. It is never
/// searched, so output that was already written is not added again.
const OUTPUT_FOLDER: &str = "RangeForge";

/// Replaces folders with what they contain: their CUE sheets, or if there are none, their
/// audio files, and the same for the folders inside them.
fn expand(inputs: &[PathBuf], log: &mut Vec<String>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for input in inputs {
        if input.is_dir() {
            collect_folder(input, 0, log, &mut paths);
        } else {
            paths.push(input.clone());
        }
    }
    paths
}

/// Adds what is in one folder, then goes into its subfolders (in name order) up to the depth
/// limit. Subfolders that are links are not followed, so a loop cannot trap the search.
fn collect_folder(dir: &Path, depth: usize, log: &mut Vec<String>, paths: &mut Vec<PathBuf>) {
    let mut entries: Vec<(PathBuf, bool)> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|e| {
                let is_folder = e.file_type().is_ok_and(|kind| kind.is_dir());
                (e.path(), is_folder)
            })
            .collect(),
        Err(error) => {
            log.push(format!("Could not read folder {}: {error}", dir.display()));
            return;
        }
    };
    entries.sort();
    let files: Vec<&PathBuf> = entries
        .iter()
        .filter(|(_, is_folder)| !is_folder)
        .map(|(path, _)| path)
        .collect();
    let cues: Vec<&PathBuf> = files.iter().copied().filter(|p| is_cue(p)).collect();
    if cues.is_empty() {
        paths.extend(files.into_iter().filter(|p| is_audio(p)).cloned());
    } else {
        paths.extend(cues.into_iter().cloned());
    }
    if depth >= MAX_DEPTH {
        return;
    }
    for (path, _) in entries.iter().filter(|(_, is_folder)| *is_folder) {
        let name = file_name(path);
        if name.starts_with('.') || name.eq_ignore_ascii_case(OUTPUT_FOLDER) {
            continue;
        }
        collect_folder(path, depth + 1, log, paths);
    }
}

/// The CUE sheet that goes with an audio file: one with the same name beside it, that names
/// this file. Dropping the audio file then does the same as dropping the folder.
fn cue_beside(audio: &Path) -> Option<PathBuf> {
    let wanted = audio.file_name()?.to_string_lossy().to_lowercase();
    ["cue", "CUE"]
        .iter()
        .map(|extension| audio.with_extension(extension))
        .find(|cue| cue.is_file())
        .filter(|cue| {
            let Ok(sheet) = read_sheet(cue) else {
                return false;
            };
            let dir = cue.parent().unwrap_or_else(|| Path::new("."));
            sheet.files.iter().any(|file| {
                resolve_audio(dir, &file.name)
                    .and_then(|found| {
                        found
                            .file_name()
                            .map(|n| n.to_string_lossy().to_lowercase())
                    })
                    .is_some_and(|name| name == wanted)
            })
        })
}

/// Swaps each audio file that has a CUE sheet beside it for that sheet, and drops repeats, so
/// an album's audio and its sheet dropped together are added once.
fn with_cues_found(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut result: Vec<PathBuf> = Vec::new();
    for path in paths {
        let path = if is_audio(&path) {
            cue_beside(&path).unwrap_or(path)
        } else {
            path
        };
        if !result.contains(&path) {
            result.push(path);
        }
    }
    result
}

/// Loads everything named in `inputs`. Problems become log lines or failed rows, never
/// errors, so one bad path does not stop the rest.
pub fn load(inputs: &[PathBuf]) -> Loaded {
    load_from(inputs, 0)
}

/// Like [`load`], for adding to a list that already holds `first_group` groups.
pub fn load_from(inputs: &[PathBuf], first_group: usize) -> Loaded {
    let mut loaded = Loaded {
        first_group,
        ..Loaded::default()
    };
    let paths = with_cues_found(expand(inputs, &mut loaded.log));
    for path in paths {
        if is_cue(&path) {
            add_cue(&path, &mut loaded);
        } else if is_audio(&path) {
            add_single(&path, &mut loaded);
        } else {
            loaded.log.push(format!(
                "Skipped {}: not an audio or CUE file",
                file_name(&path)
            ));
        }
    }
    loaded
}

fn album_name(sheet: &CueSheet, cue_path: &Path) -> String {
    match (sheet.performer.as_deref(), sheet.title.as_deref()) {
        (Some(performer), Some(title)) => format!("{performer} - {title}"),
        (None, Some(title)) => title.to_string(),
        _ => cue_path.file_stem().map_or_else(
            || "(untitled)".to_string(),
            |s| s.to_string_lossy().into_owned(),
        ),
    }
}

fn add_cue(cue_path: &Path, loaded: &mut Loaded) {
    let sheet = match read_sheet(cue_path) {
        Ok(sheet) => sheet,
        Err(error) => {
            loaded
                .log
                .push(format!("Could not read {}: {error:#}", file_name(cue_path)));
            return;
        }
    };
    let dir = cue_path.parent().unwrap_or_else(|| Path::new("."));
    let base_name = album_name(&sheet, cue_path);

    for file in &sheet.files {
        let name = if sheet.files.len() > 1 {
            format!("{base_name} [{}]", file.name)
        } else {
            base_name.clone()
        };
        let tracks = file
            .tracks
            .iter()
            .map(|track| TrackRow {
                identity: Some(Identity::from_cue(&sheet, track)),
                ..TrackRow::new(format!(
                    "{} - {}",
                    track.number,
                    track.title.as_deref().unwrap_or("(untitled)")
                ))
            })
            .collect();
        let resolved = resolve_audio(dir, &file.name);
        let mut group = Group {
            name,
            kind: Kind::CueAlbum,
            source_extension: resolved.as_deref().map(extension_of).unwrap_or_default(),
            tracks,
            status: Status::Pending,
            run: RunState::Idle,
        };

        let index = loaded.first_group + loaded.groups.len();
        if let Some(audio) = resolved {
            loaded.log.push(format!("Added {}", file_name(&audio)));
            loaded
                .log
                .push(format!("Cue detected: {}", file_name(cue_path)));
            loaded.jobs.push(Job {
                group: index,
                audio,
                cue_file: Some(file.clone()),
                sheet: Some(sheet.clone()),
            });
        } else {
            let message = format!("audio file not found: {}", file.name);
            loaded
                .log
                .push(format!("Failed: {}: {message}", group.name));
            group.status = Status::Failed(message);
        }
        loaded.groups.push(group);
    }
}

fn add_single(path: &Path, loaded: &mut Loaded) {
    let name = file_name(path);
    let stem = path
        .file_stem()
        .map_or_else(|| name.clone(), |s| s.to_string_lossy().into_owned());
    loaded.jobs.push(Job {
        group: loaded.first_group + loaded.groups.len(),
        audio: path.to_path_buf(),
        cue_file: None,
        sheet: None,
    });
    loaded.groups.push(Group {
        name: name.clone(),
        kind: Kind::SingleFile,
        source_extension: extension_of(path),
        // Until the file's tags are read (with its measurement), its name is the best guide.
        tracks: vec![TrackRow {
            identity: Some(Identity::guess_from_path(path)),
            ..TrackRow::new(stem)
        }],
        status: Status::Pending,
        run: RunState::Idle,
    });
    loaded.log.push(format!("Added {name}"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const CUE: &str = "PERFORMER \"Led Zeppelin\"\nTITLE \"Houses of the Holy\"\nFILE \"album.wav\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"The Song Remains the Same\"\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    TITLE \"The Rain Song\"\n    INDEX 01 05:30:30\n";

    fn touch(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn a_cue_becomes_an_album_with_one_job() {
        let dir = tempdir().unwrap();
        let cue = touch(dir.path(), "album.cue", CUE);
        touch(dir.path(), "album.wav", "");

        let loaded = load(&[cue]);
        assert_eq!(loaded.groups.len(), 1);
        let group = &loaded.groups[0];
        assert_eq!(group.name, "Led Zeppelin - Houses of the Holy");
        assert_eq!(group.kind, Kind::CueAlbum);
        assert_eq!(group.status, Status::Pending);
        assert_eq!(group.tracks[0].label, "1 - The Song Remains the Same");
        assert_eq!(group.tracks[1].label, "2 - The Rain Song");
        assert_eq!(loaded.jobs.len(), 1);
        assert_eq!(loaded.jobs[0].group, 0);
        assert!(loaded.jobs[0].cue_file.is_some());
    }

    #[test]
    fn a_renamed_audio_file_is_found_by_its_stem() {
        let dir = tempdir().unwrap();
        let cue = touch(dir.path(), "album.cue", CUE);
        // The sheet names album.wav, but the file is now a FLAC.
        touch(dir.path(), "album.flac", "");

        let loaded = load(&[cue]);
        assert_eq!(loaded.groups[0].source_extension, "flac");
        assert_eq!(loaded.jobs.len(), 1);
        assert!(loaded.jobs[0].audio.ends_with("album.flac"));
    }

    #[test]
    fn stems_with_dots_are_matched_whole() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "Vol.2 Live.flac", "");
        let found = resolve_audio(dir.path(), "Vol.2 Live.wav").unwrap();
        assert!(found.ends_with("Vol.2 Live.flac"));
    }

    #[test]
    fn missing_audio_gives_a_failed_row_and_no_job() {
        let dir = tempdir().unwrap();
        let cue = touch(dir.path(), "album.cue", CUE);

        let loaded = load(&[cue]);
        assert!(loaded.jobs.is_empty());
        assert!(matches!(loaded.groups[0].status, Status::Failed(_)));
        assert!(
            loaded
                .log
                .iter()
                .any(|line| line.contains("audio file not found"))
        );
    }

    #[test]
    fn a_single_audio_file_becomes_a_one_song_group() {
        let dir = tempdir().unwrap();
        let song = touch(dir.path(), "14 - Shot at the Night.flac", "");

        let loaded = load(&[song]);
        let group = &loaded.groups[0];
        assert_eq!(group.kind, Kind::SingleFile);
        assert_eq!(group.codec_label(), "(flac)");
        assert_eq!(group.tracks.len(), 1);
        assert_eq!(group.tracks[0].label, "14 - Shot at the Night");
        assert!(loaded.jobs[0].cue_file.is_none());
    }

    fn nested(root: &Path, path: &str) -> PathBuf {
        let full = root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(&full, "").unwrap();
        full
    }

    #[test]
    fn a_dropped_folder_is_searched_down_through_its_folders() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("Alan Parsons Project - I Robot");
        nested(&root, "1977 - I Robot/6ch/01 - I Robot.dsf");
        nested(&root, "1977 - I Robot/6ch/02 - Breakdown.dsf");
        nested(&root, "1977 - I Robot/2ch/01 - I Robot.dsf");
        nested(&root, "cover.jpg");

        let loaded = load(&[root]);

        assert_eq!(loaded.groups.len(), 3);
        assert_eq!(loaded.jobs.len(), 3);
        assert!(
            loaded
                .jobs
                .iter()
                .all(|job| job.audio.extension().unwrap() == "dsf")
        );
    }

    #[test]
    fn each_folder_uses_its_own_cue_sheet_or_else_its_own_audio() {
        let dir = tempdir().unwrap();
        nested(dir.path(), "Disc 1/album.cue");
        fs::write(dir.path().join("Disc 1/album.cue"), CUE).unwrap();
        nested(dir.path(), "Disc 1/album.wav");
        nested(dir.path(), "Disc 1/stray.flac");
        nested(dir.path(), "Disc 2/loose.flac");

        let loaded = load(&[dir.path().to_path_buf()]);

        let kinds: Vec<_> = loaded.groups.iter().map(|g| g.kind).collect();
        assert_eq!(kinds, [Kind::CueAlbum, Kind::SingleFile]);
    }

    #[test]
    fn the_search_stops_four_folders_down_and_skips_hidden_and_output_folders() {
        let dir = tempdir().unwrap();
        nested(dir.path(), "a/b/c/d/four.flac");
        nested(dir.path(), "a/b/c/d/e/five.flac");
        nested(dir.path(), ".hidden/secret.flac");
        nested(dir.path(), "RangeForge/earlier output.flac");
        nested(dir.path(), "rangeforge/lower case.flac");

        let loaded = load(&[dir.path().to_path_buf()]);

        let names: Vec<_> = loaded.groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["four.flac"]);
    }

    #[test]
    fn a_folder_uses_its_cue_sheets_and_otherwise_its_audio() {
        let with_cue = tempdir().unwrap();
        touch(with_cue.path(), "album.cue", CUE);
        touch(with_cue.path(), "album.flac", "");
        touch(with_cue.path(), "notes.txt", "");
        let loaded = load(&[with_cue.path().to_path_buf()]);
        assert_eq!(loaded.groups.len(), 1);
        assert_eq!(loaded.groups[0].kind, Kind::CueAlbum);

        let without = tempdir().unwrap();
        touch(without.path(), "b.flac", "");
        touch(without.path(), "a.mp3", "");
        touch(without.path(), "cover.jpg", "");
        let loaded = load(&[without.path().to_path_buf()]);
        let names: Vec<&str> = loaded.groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["a.mp3", "b.flac"]);
    }

    #[test]
    fn unknown_files_and_bad_cues_are_logged_not_fatal() {
        let dir = tempdir().unwrap();
        let text = touch(dir.path(), "readme.txt", "");
        let bad = touch(dir.path(), "broken.cue", "this is not a cue sheet");
        let song = touch(dir.path(), "song.flac", "");

        let loaded = load(&[text, bad, song]);
        assert_eq!(loaded.groups.len(), 1);
        assert!(loaded.log.iter().any(|l| l.contains("Skipped readme.txt")));
        assert!(
            loaded
                .log
                .iter()
                .any(|l| l.contains("Could not read broken.cue"))
        );
    }

    #[test]
    fn job_indexes_follow_group_order() {
        let dir = tempdir().unwrap();
        let cue = touch(dir.path(), "album.cue", CUE);
        touch(dir.path(), "album.wav", "");
        let song = touch(dir.path(), "song.flac", "");

        let loaded = load(&[cue, song]);
        let indexes: Vec<usize> = loaded.jobs.iter().map(|job| job.group).collect();
        assert_eq!(indexes, [0, 1]);
    }

    #[test]
    fn dropping_the_audio_file_finds_the_cue_sheet_beside_it() {
        let dir = tempdir().unwrap();
        let cue = touch(dir.path(), "album.cue", CUE);
        let audio = touch(dir.path(), "album.flac", "");

        let loaded = load(std::slice::from_ref(&audio));
        assert_eq!(loaded.groups.len(), 1);
        assert_eq!(loaded.groups[0].kind, Kind::CueAlbum);
        assert!(loaded.jobs[0].cue_file.is_some());

        // The audio and its sheet together are one album, not two.
        let loaded = load(&[audio, cue]);
        assert_eq!(loaded.groups.len(), 1);
        assert_eq!(loaded.jobs.len(), 1);
    }

    #[test]
    fn a_cue_sheet_for_a_different_file_is_not_used() {
        let dir = tempdir().unwrap();
        // The sheet is named after this file but describes album.wav, which is not here.
        touch(dir.path(), "song.cue", CUE);
        let song = touch(dir.path(), "song.flac", "");

        let loaded = load(&[song]);
        assert_eq!(loaded.groups[0].kind, Kind::SingleFile);
    }

    #[test]
    fn files_added_later_point_at_the_right_groups() {
        let dir = tempdir().unwrap();
        let cue = touch(dir.path(), "album.cue", CUE);
        touch(dir.path(), "album.wav", "");
        let song = touch(dir.path(), "song.flac", "");

        // The list already holds three groups, so these become groups 3 and 4.
        let loaded = load_from(&[cue, song], 3);
        let indexes: Vec<usize> = loaded.jobs.iter().map(|job| job.group).collect();
        assert_eq!(indexes, [3, 4]);
        assert_eq!(
            loaded.groups.len(),
            2,
            "the groups themselves are not offset"
        );
    }
}
