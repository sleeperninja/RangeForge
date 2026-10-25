//! Who a song is: its artist, title, album and so on, and where that knowledge came from.
//!
//! The best source is the file's own tags, or a CUE sheet's performer and title lines. When
//! neither exists, the file's name and the folders it sits in often say enough, as in
//! `Led Zeppelin/Houses of the Holy/07 - No Quarter.flac`. That last kind is a guess, and an
//! [`Identity`] says so, so the screen can show it as one.

use std::path::Path;

use crate::cue::{CueSheet, CueTrack};

/// Where an identity came from, from most to least trustworthy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentitySource {
    /// The performer and title lines of a CUE sheet.
    CueSheet,
    /// Tags stored in the audio file.
    FileTags,
    /// Worked out from the file's name and folders. A guess.
    FileName,
}

/// What is known about one song. Anything unknown is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub source: IdentitySource,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    /// As written in the source: a year, or a full date.
    pub date: Option<String>,
    pub track: Option<u32>,
}

/// Words a folder might be called that say nothing about an artist.
const GENERIC_FOLDERS: [&str; 14] = [
    "music",
    "downloads",
    "audio",
    "flac",
    "alac",
    "itunes",
    "media",
    "testdata",
    "desktop",
    "documents",
    "new folder",
    "temp",
    "tmp",
    "various artists",
];

impl Identity {
    /// An identity that knows nothing yet.
    #[must_use]
    pub fn unknown(source: IdentitySource) -> Self {
        Self {
            source,
            artist: None,
            title: None,
            album: None,
            album_artist: None,
            genre: None,
            date: None,
            track: None,
        }
    }

    /// One track of a CUE album. A track's own performer wins over the album's.
    #[must_use]
    pub fn from_cue(sheet: &CueSheet, track: &CueTrack) -> Self {
        Self {
            source: IdentitySource::CueSheet,
            artist: track.performer.clone().or_else(|| sheet.performer.clone()),
            title: track.title.clone(),
            album: sheet.title.clone(),
            album_artist: sheet.performer.clone(),
            genre: sheet.genre.clone(),
            date: sheet.date.clone(),
            track: Some(track.number),
        }
    }

    /// True when there is enough to look the song up: an artist and a title.
    #[must_use]
    pub fn can_look_up(&self) -> bool {
        self.artist.is_some() && self.title.is_some()
    }

    /// Works out what it can from a file's name and the folders above it. See the tests for
    /// the patterns it understands.
    #[must_use]
    pub fn guess_from_path(path: &Path) -> Self {
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        let folder = |level: usize| -> Option<String> {
            path.ancestors()
                .nth(level)
                .and_then(|dir| dir.file_name())
                .and_then(|name| name.to_str())
                .map(str::to_string)
        };

        let mut identity = Self::unknown(IdentitySource::FileName);
        let stem = tidy(stem);
        let (track, rest) = split_track(&stem);
        identity.track = track;
        match rest.split_once(" - ") {
            Some((artist, title)) if !artist.trim().is_empty() && !title.trim().is_empty() => {
                identity.artist = Some(artist.trim().to_string());
                identity.title = Some(title.trim().to_string());
            }
            _ if !rest.trim().is_empty() => identity.title = Some(rest.trim().to_string()),
            _ => {}
        }

        // The folder is the album, and often says the artist too.
        if let Some(parent) = folder(1) {
            let named = album_folder(&tidy(&parent));
            identity.album = Some(named.album).filter(|album| !album.is_empty());
            identity.date = named.date;
            if identity.artist.is_none() {
                identity.artist = named.artist.or_else(|| {
                    folder(2)
                        .map(|grandparent| tidy(&grandparent))
                        .filter(|name| is_artist_folder(name))
                });
            }
        }
        identity
    }
}

/// Folder names that could be an artist.
fn is_artist_folder(name: &str) -> bool {
    let lower = name.to_lowercase();
    !name.is_empty()
        && !GENERIC_FOLDERS.contains(&lower.as_str())
        // A drive letter such as "C:" is not an artist.
        && !name.ends_with(':')
}

/// Underscores stand for spaces in names made without them, such as `Led_Zeppelin_-_Black_Dog`.
fn tidy(text: &str) -> String {
    let text = text.trim();
    let text = if text.contains(' ') {
        text.to_string()
    } else {
        text.replace('_', " ")
    };
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Splits a leading track number off a name: `03 - Title`, `03. Title`, `3-Title`,
/// `03 Title`. A number alone is not a track (it may be the title), and a bare `99 Luftballons`
/// is left whole: with only a space after it, the number needs a leading zero to count.
fn split_track(name: &str) -> (Option<u32>, String) {
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 3 {
        return (None, name.to_string());
    }
    let after = &name[digits.len()..];
    let rest = after.trim_start_matches([' ', '.', '-', ')', '_']).trim();
    if rest.is_empty() {
        return (None, name.to_string());
    }
    let explicit_separator = after.trim_start().starts_with(['.', '-', ')', '_']);
    let spaced_only = after.starts_with(' ') && !explicit_separator;
    if spaced_only && !digits.starts_with('0') {
        return (None, name.to_string());
    }
    if !explicit_separator && !spaced_only {
        return (None, name.to_string());
    }
    (digits.parse().ok(), rest.to_string())
}

/// What a folder name says about an album.
struct AlbumFolder {
    artist: Option<String>,
    album: String,
    date: Option<String>,
}

/// Reads folder names such as `Led Zeppelin - Houses of the Holy (1973) [FLAC]` and
/// `1973. Houses of the Holy`.
fn album_folder(name: &str) -> AlbumFolder {
    let mut date = None;
    let mut text = name.trim().to_string();

    // Notes in brackets at the end: a year is kept, anything else (`[FLAC]`) is dropped.
    while let Some(note) = trailing_note(&text) {
        if note.inner.len() == 4 && note.inner.chars().all(|c| c.is_ascii_digit()) {
            date = Some(note.inner);
        }
        text.truncate(note.start);
        text = text.trim_end().to_string();
    }

    // A year in front: `1973. Album`, `1973 - Album`, `1973 Album`.
    let year: String = text.chars().take_while(char::is_ascii_digit).collect();
    if year.len() == 4 {
        let rest = text[4..].trim_start_matches([' ', '.', '-', '_']).trim();
        if !rest.is_empty() && text[4..].starts_with([' ', '.', '-', '_']) {
            date = date.or(Some(year));
            text = rest.to_string();
        }
    }

    match text.split_once(" - ") {
        Some((artist, album)) if !artist.trim().is_empty() && !album.trim().is_empty() => {
            AlbumFolder {
                artist: Some(artist.trim().to_string()),
                album: album.trim().to_string(),
                date,
            }
        }
        _ => AlbumFolder {
            artist: None,
            album: text,
            date,
        },
    }
}

struct Note {
    start: usize,
    inner: String,
}

/// A `(...)` or `[...]` group that ends the text.
fn trailing_note(text: &str) -> Option<Note> {
    let trimmed = text.trim_end();
    let close = trimmed.chars().last()?;
    let open = match close {
        ')' => '(',
        ']' => '[',
        _ => return None,
    };
    let start = trimmed.rfind(open)?;
    Some(Note {
        start,
        inner: trimmed[start + 1..trimmed.len() - 1].trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cue;

    fn guess(path: &str) -> Identity {
        Identity::guess_from_path(Path::new(path))
    }

    fn fields(identity: &Identity) -> (Option<&str>, Option<&str>, Option<&str>, Option<u32>) {
        (
            identity.artist.as_deref(),
            identity.title.as_deref(),
            identity.album.as_deref(),
            identity.track,
        )
    }

    #[test]
    fn artist_dash_title() {
        let identity = guess("/stuff/Led Zeppelin - Black Dog.flac");
        assert_eq!(
            fields(&identity),
            (Some("Led Zeppelin"), Some("Black Dog"), Some("stuff"), None)
        );
        assert_eq!(identity.source, IdentitySource::FileName);
    }

    #[test]
    fn a_track_number_then_the_title_with_the_folders_giving_album_and_artist() {
        for name in [
            "07 - No Quarter.flac",
            "07. No Quarter.flac",
            "07 No Quarter.flac",
            "7-No Quarter.flac",
            "07_No_Quarter.flac",
        ] {
            let identity = guess(&format!("/music/Led Zeppelin/Houses of the Holy/{name}"));
            assert_eq!(
                fields(&identity),
                (
                    Some("Led Zeppelin"),
                    Some("No Quarter"),
                    Some("Houses of the Holy"),
                    Some(7)
                ),
                "{name}"
            );
        }
    }

    #[test]
    fn a_track_number_then_artist_and_title() {
        let identity = guess("/x/y/03 - Led Zeppelin - Immigrant Song.flac");
        assert_eq!(
            fields(&identity),
            (
                Some("Led Zeppelin"),
                Some("Immigrant Song"),
                Some("y"),
                Some(3)
            )
        );
    }

    #[test]
    fn a_number_that_is_part_of_the_title_stays_in_it() {
        assert_eq!(
            guess("/m/99 Luftballons.flac").title.as_deref(),
            Some("99 Luftballons")
        );
        assert_eq!(guess("/m/1979.flac").title.as_deref(), Some("1979"));
        assert_eq!(guess("/m/7.flac").title.as_deref(), Some("7"));
        assert_eq!(guess("/m/99 Luftballons.flac").track, None);
        // But with a clear separator it is a track.
        assert_eq!(guess("/m/99 - Luftballons.flac").track, Some(99));
    }

    #[test]
    fn an_album_folder_can_name_the_artist_the_year_and_notes() {
        let identity = guess(
            "/m/Led Zeppelin - Houses of the Holy (1973) [FLAC]/01 - The Song Remains the Same.flac",
        );
        assert_eq!(identity.artist.as_deref(), Some("Led Zeppelin"));
        assert_eq!(identity.album.as_deref(), Some("Houses of the Holy"));
        assert_eq!(identity.date.as_deref(), Some("1973"));
        assert_eq!(identity.track, Some(1));
    }

    #[test]
    fn a_year_in_front_of_the_album_is_a_date_and_the_artist_comes_from_above() {
        let identity = guess("/m/Led Zeppelin/1973. Houses of the Holy/01 - The Rain Song.flac");
        assert_eq!(identity.artist.as_deref(), Some("Led Zeppelin"));
        assert_eq!(identity.album.as_deref(), Some("Houses of the Holy"));
        assert_eq!(identity.date.as_deref(), Some("1973"));
        for text in ["/m/A/1973 - Album/x.flac", "/m/A/1973 Album/x.flac"] {
            assert_eq!(guess(text).album.as_deref(), Some("Album"), "{text}");
        }
    }

    #[test]
    fn folders_that_say_nothing_about_an_artist_are_not_one() {
        assert_eq!(guess("/Music/Some Album/01 - Song.flac").artist, None);
        assert_eq!(guess("C:/Downloads/Some Album/01 - Song.flac").artist, None);
        assert_eq!(guess("C:/Some Album/01 - Song.flac").artist, None);
    }

    #[test]
    fn underscores_stand_for_spaces_only_when_there_are_no_spaces() {
        let identity = guess("/m/Led_Zeppelin_-_Black_Dog.flac");
        assert_eq!(identity.artist.as_deref(), Some("Led Zeppelin"));
        assert_eq!(identity.title.as_deref(), Some("Black Dog"));
        assert_eq!(
            guess("/m/Snake_Eyes - Part 2.flac").title.as_deref(),
            Some("Part 2"),
            "a name that already has spaces keeps its underscores"
        );
    }

    #[test]
    fn a_bare_name_is_only_a_title() {
        let identity = guess("/tmp/song.flac");
        assert_eq!(identity.title.as_deref(), Some("song"));
        assert_eq!(identity.artist, None);
        assert!(!identity.can_look_up());
    }

    #[test]
    fn a_guess_with_an_artist_and_a_title_can_be_looked_up() {
        assert!(guess("/m/Killers/Direct Hits/14 - Shot at the Night.flac").can_look_up());
    }

    #[test]
    fn a_cue_track_knows_what_its_sheet_says() {
        let sheet = cue::parse(
            "REM GENRE Rock\nREM DATE 1973\nPERFORMER \"Led Zeppelin\"\nTITLE \"Houses of the Holy\"\nFILE \"a.flac\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"The Rain Song\"\n    PERFORMER \"Robert Plant\"\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    TITLE \"No Quarter\"\n    INDEX 01 05:00:00\n",
        )
        .unwrap();
        let tracks = &sheet.files[0].tracks;

        let first = Identity::from_cue(&sheet, &tracks[0]);
        assert_eq!(first.source, IdentitySource::CueSheet);
        assert_eq!(first.artist.as_deref(), Some("Robert Plant"));
        assert_eq!(first.album_artist.as_deref(), Some("Led Zeppelin"));

        let second = Identity::from_cue(&sheet, &tracks[1]);
        assert_eq!(
            fields(&second),
            (
                Some("Led Zeppelin"),
                Some("No Quarter"),
                Some("Houses of the Holy"),
                Some(2)
            ),
            "the album's performer fills in"
        );
        assert_eq!(second.genre.as_deref(), Some("Rock"));
        assert_eq!(second.date.as_deref(), Some("1973"));
    }
}
