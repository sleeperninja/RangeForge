//! Working out who a single file's song is: its own tags when it has them, otherwise what its
//! name and folders suggest.
//!
//! Songs cut from a CUE album are identified from the sheet instead (see
//! [`Identity::from_cue`]).

use std::path::Path;

use rangeforge_audio::tags::{FileTags, read_tags};
use rangeforge_core::identify::{Identity, IdentitySource};

/// Combines what a file's tags say with what its name suggests. The tags win wherever they have
/// something to say; the guess only fills the gaps. A file with no usable tags is just the guess.
#[must_use]
pub fn merge_tags(guess: Identity, tags: &FileTags) -> Identity {
    if tags.is_empty() {
        return guess;
    }
    Identity {
        source: IdentitySource::FileTags,
        artist: tags.artist.clone().or(guess.artist),
        title: tags.title.clone().or(guess.title),
        album: tags.album.clone().or(guess.album),
        album_artist: tags.album_artist.clone(),
        genre: tags.genre.clone(),
        date: tags.date.clone().or(guess.date),
        track: tags.track.or(guess.track),
    }
}

/// Who the song in this file is. Reads only the start of the file; a file whose tags cannot be
/// read is identified by its name.
#[must_use]
pub fn identify_file(path: &Path) -> Identity {
    let guess = Identity::guess_from_path(path);
    match read_tags(path) {
        Ok(tags) => merge_tags(guess, &tags),
        Err(_) => guess,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guess() -> Identity {
        Identity::guess_from_path(Path::new(
            "/music/Led Zeppelin/1973. Houses of the Holy/07 - No Quarter.flac",
        ))
    }

    #[test]
    fn tags_win_and_the_guess_fills_the_gaps() {
        let tags = FileTags {
            title: Some("No Quarter (Remaster)".to_string()),
            genre: Some("Rock".to_string()),
            ..FileTags::default()
        };
        let merged = merge_tags(guess(), &tags);
        assert_eq!(merged.source, IdentitySource::FileTags);
        assert_eq!(merged.title.as_deref(), Some("No Quarter (Remaster)"));
        assert_eq!(merged.genre.as_deref(), Some("Rock"));
        // Not in the tags, so taken from the folders.
        assert_eq!(merged.artist.as_deref(), Some("Led Zeppelin"));
        assert_eq!(merged.album.as_deref(), Some("Houses of the Holy"));
        assert_eq!(merged.track, Some(7));
        assert_eq!(merged.date.as_deref(), Some("1973"));
    }

    #[test]
    fn a_file_without_tags_is_just_the_guess() {
        let merged = merge_tags(guess(), &FileTags::default());
        assert_eq!(merged, guess());
        assert_eq!(merged.source, IdentitySource::FileName);
    }

    #[test]
    fn a_file_that_cannot_be_read_falls_back_to_its_name() {
        let identity = identify_file(Path::new("/nowhere/Killers/Direct Hits/14 - Shot.flac"));
        assert_eq!(identity.source, IdentitySource::FileName);
        assert_eq!(identity.title.as_deref(), Some("Shot"));
    }
}
