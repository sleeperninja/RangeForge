//! Reading the tags stored in an audio file: title, artist, album and so on.
//!
//! Only the start of the file is read, which is where tags live in FLAC, MP4 and MP3, so this
//! is cheap even on a network drive. It does not decode any audio.

use std::fs::File;
use std::path::Path;

use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardTagKey};
use symphonia::core::probe::Hint;

use crate::AudioError;

/// The tags a file carries. Anything it does not have is `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    /// As stored: a year or a full date.
    pub date: Option<String>,
    pub track: Option<u32>,
}

impl FileTags {
    /// True if the file has nothing worth using.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

fn keep_first(slot: &mut Option<String>, value: String) {
    if slot.is_none() {
        *slot = Some(value);
    }
}

/// Adds what one block of metadata says. A tag already found is not overwritten, so the
/// block read first wins.
fn collect(revision: &MetadataRevision, tags: &mut FileTags) {
    for tag in revision.tags() {
        let Some(key) = tag.std_key else {
            continue;
        };
        let value = tag.value.to_string().trim().to_string();
        if value.is_empty() {
            continue;
        }
        match key {
            StandardTagKey::TrackTitle => keep_first(&mut tags.title, value),
            StandardTagKey::Artist => keep_first(&mut tags.artist, value),
            StandardTagKey::Album => keep_first(&mut tags.album, value),
            StandardTagKey::AlbumArtist => keep_first(&mut tags.album_artist, value),
            StandardTagKey::Genre => keep_first(&mut tags.genre, value),
            StandardTagKey::Date | StandardTagKey::ReleaseDate => keep_first(&mut tags.date, value),
            // Often written `7/12`: the number is what comes before the slash.
            StandardTagKey::TrackNumber if tags.track.is_none() => {
                let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
                tags.track = digits.parse().ok().filter(|n| *n > 0);
            }
            _ => {}
        }
    }
}

/// Reads the tags of the audio file at `path`.
pub fn read_tags(path: &Path) -> Result<FileTags, AudioError> {
    let file = File::open(path).map_err(|source| AudioError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    let stream = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }
    let mut probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(AudioError::from)?;

    let mut tags = FileTags::default();
    // Tags inside the format itself (FLAC, MP4) come first, then any in front of it (ID3).
    if let Some(revision) = probed.format.metadata().current() {
        collect(revision, &mut tags);
    }
    if let Some(revision) = probed.metadata.get().as_ref().and_then(|m| m.current()) {
        collect(revision, &mut tags);
    }
    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_audio_is_an_error_and_a_missing_one_says_so() {
        let dir = std::env::temp_dir();
        let missing = dir.join("rangeforge-no-such-file.flac");
        assert!(matches!(read_tags(&missing), Err(AudioError::Open { .. })));
    }

    #[test]
    fn nothing_found_counts_as_empty() {
        assert!(FileTags::default().is_empty());
        let tagged = FileTags {
            title: Some("x".to_string()),
            ..FileTags::default()
        };
        assert!(!tagged.is_empty());
    }
}
