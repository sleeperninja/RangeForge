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
    /// Typed in by the user. Nothing replaces it.
    Hand,
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
        // A name made by a machine, such as `00724352531929_001_Doves_Firesite_GBDGG0000013`,
        // has underscores and no spaces, or (when a store keeps the title's own spaces, as in
        // `..._001_Doves_Some Cities_GBDGG0400080`) at least a barcode or an ISRC. Those codes
        // are not part of the song, and what is left is separated by underscores.
        let (without_codes, found_codes) = strip_release_codes(stem);
        let machine_made = found_codes || !stem.trim().contains(' ');
        let fields = if found_codes {
            without_codes.replace('_', " ")
        } else {
            without_codes
        };
        let stem = tidy(&fields);
        let (track, rest) = split_track(&stem);
        identity.track = track;
        let mut artist_from_name = false;
        match rest.split_once(" - ") {
            Some((artist, title)) if !artist.trim().is_empty() && !title.trim().is_empty() => {
                artist_from_name = true;
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
                        .map(|grandparent| without_notes(&tidy(&grandparent)))
                        .filter(|name| is_artist_folder(name))
                });
            }
        }

        // Such a name runs the artist and the title together (`Doves_Firesite`), and only the
        // folder tells where one ends. Take the artist off the front of the title.
        if machine_made
            && !artist_from_name
            && let (Some(artist), Some(title)) = (&identity.artist, &identity.title)
            && let Some(rest) = without_artist_prefix(title, artist)
        {
            identity.title = Some(rest);
        }
        identity
    }
}

/// A name's barcode and ISRC, when it is an underscore-joined name that has them, and whether
/// either was found. The barcode (8 to 14 digits) leads and is followed by the track number;
/// the ISRC (two letters, three letters or digits, seven digits) ends. The fields in between
/// may hold spaces: some stores keep the title's own.
fn strip_release_codes(stem: &str) -> (String, bool) {
    let stem = stem.trim();
    let mut parts: Vec<&str> = stem.split('_').collect();
    let mut found = false;
    let barcode =
        |part: &str| (8..=14).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_digit());
    let short_number =
        |part: &str| (1..=3).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_digit());
    if parts.len() >= 3 && barcode(parts[0]) && short_number(parts[1]) {
        parts.remove(0);
        found = true;
    }
    let isrc = |part: &str| {
        let bytes = part.as_bytes();
        bytes.len() == 12
            && bytes[..2].iter().all(u8::is_ascii_uppercase)
            && bytes[2..5]
                .iter()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            && bytes[5..].iter().all(u8::is_ascii_digit)
    };
    if parts.len() >= 2 && parts.last().is_some_and(|last| isrc(last)) {
        parts.pop();
        found = true;
    }
    (parts.join("_"), found)
}

/// `text` without `(...)` and `[...]` notes at its end: `Doves (lossless downloads)` is `Doves`.
/// Text that is nothing but a note is left as it is.
fn without_notes(text: &str) -> String {
    let mut text = text.trim().to_string();
    while let Some(note) = trailing_note(&text) {
        let shorter = text[..note.start].trim_end().to_string();
        if shorter.is_empty() {
            break;
        }
        text = shorter;
    }
    text
}

/// The title with the artist's name taken off its front (`Doves Firesite` by Doves is
/// `Firesite`), or `None` if it does not start with the artist or would be left empty.
fn without_artist_prefix(title: &str, artist: &str) -> Option<String> {
    let head = title.get(..artist.len())?;
    let rest = title.get(artist.len()..)?;
    if !head.eq_ignore_ascii_case(artist) || !rest.starts_with(' ') {
        return None;
    }
    let rest = rest.trim();
    (!rest.is_empty()).then(|| rest.to_string())
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
    fn a_vinyl_download_name_gives_track_title_and_the_folders_give_the_rest() {
        let folder = "/audio/HiRes Backup/Doves (lossless downloads from vinyls)/2000 - Lost Souls";
        for (file, track, title) in [
            (
                "00724352531929_001_Doves_Firesite_GBDGG0000013.wav",
                1,
                "Firesite",
            ),
            (
                "00724352531929_002_Doves_Here_It_Comes_GBDGG0000014.wav",
                2,
                "Here It Comes",
            ),
            (
                "00724352531929_009_Doves_The_Man_Who_Told_Everything_GBDGG0000021.wav",
                9,
                "The Man Who Told Everything",
            ),
            (
                "00724352531929_012_Doves_A_House_GBDGG0000023.wav",
                12,
                "A House",
            ),
        ] {
            let identity = guess(&format!("{folder}/{file}"));
            assert_eq!(
                fields(&identity),
                (Some("Doves"), Some(title), Some("Lost Souls"), Some(track)),
                "{file}"
            );
            assert_eq!(identity.date.as_deref(), Some("2000"), "{file}");
            assert!(identity.can_look_up(), "{file}");
        }
    }

    #[test]
    fn a_store_that_keeps_the_titles_own_spaces_is_read_too() {
        // Underscores between the fields, real spaces inside the title.
        let folder = "/audio/Doves (lossless downloads from vinyls)/2005 - Some Cities";
        for (file, track, title) in [
            (
                "00724387460928_001_Doves_Some Cities_GBDGG0400080.m4a",
                1,
                "Some Cities",
            ),
            (
                "00724387460928_002_Doves_Black And White Town_GBDGG0400054.m4a",
                2,
                "Black And White Town",
            ),
            (
                "00724387460928_006_Doves_Walk In Fire_GBDGG0400081.m4a",
                6,
                "Walk In Fire",
            ),
            (
                "00724387460928_010_Doves_Sky Starts Falling_GBDGG0400060.m4a",
                10,
                "Sky Starts Falling",
            ),
        ] {
            let identity = guess(&format!("{folder}/{file}"));
            assert_eq!(
                fields(&identity),
                (Some("Doves"), Some(title), Some("Some Cities"), Some(track)),
                "{file}"
            );
            assert_eq!(identity.date.as_deref(), Some("2005"), "{file}");
        }
        // The files that were tagged by name parse as before.
        let plain = guess(&format!("{folder}/04 - Snowden.m4a"));
        assert_eq!(plain.title.as_deref(), Some("Snowden"));
        assert_eq!(plain.track, Some(4));
        // A name with spaces and no codes is the person's own and is left whole.
        let own = guess(&format!("{folder}/Doves_Some Cities.m4a"));
        assert_eq!(own.title.as_deref(), Some("Doves_Some Cities"));
    }

    #[test]
    fn release_codes_and_the_artist_prefix_only_go_from_machine_made_names() {
        // A name with spaces is the person's own: nothing is stripped from it.
        let spaced = guess("/m/Doves/Lost Souls/01 Doves Firesite.wav");
        assert_eq!(spaced.title.as_deref(), Some("Doves Firesite"));
        // A song named for the artist keeps its name.
        let same = guess("/m/Doves/Lost Souls/00724352531929_003_Doves_Doves_GBDGG0000015.wav");
        assert_eq!(same.title.as_deref(), Some("Doves"));
        let alone = guess("/m/Doves/Lost Souls/01_Doves.wav");
        assert_eq!(alone.title.as_deref(), Some("Doves"));
        // Several words of artist, and an ISRC with no barcode before it.
        let two =
            guess("/m/Led Zeppelin (lossless)/IV/003_Led_Zeppelin_Black_Dog_GBAAA0000001.flac");
        assert_eq!(two.artist.as_deref(), Some("Led Zeppelin"));
        assert_eq!(two.title.as_deref(), Some("Black Dog"));
        // Something that only looks like a code is left in: eleven characters is no ISRC.
        let near = guess("/m/A/B/01_Song_GBDGG000001.flac");
        assert_eq!(near.title.as_deref(), Some("Song GBDGG000001"));
        // A folder that is nothing but a note is not turned into an empty artist.
        assert_eq!(without_notes("(lossless)"), "(lossless)");
        assert_eq!(without_notes("Doves [FLAC] (1999)"), "Doves");
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
