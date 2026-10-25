//! Remembering what Last.fm said, so an album or song is only ever asked about once.
//!
//! Answers are kept in one small JSON file. A question Last.fm could not answer for lack of an
//! entry (no tags) is remembered too, for a shorter time, so a collection full of obscure albums
//! does not ask again at every start. Failures (no network, a bad key) are never remembered.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::lastfm::RawTag;
use crate::normalize::normalize;

/// Bump this if the file's layout changes in a way older versions cannot read.
const VERSION: u32 = 1;

/// How long tags are trusted. What listeners call an album hardly changes.
pub const KEEP_TAGS: Duration = Duration::from_secs(60 * 60 * 24 * 90);
/// How long "Last.fm has no tags for this" is trusted. Shorter, since tags do appear later.
pub const KEEP_NOTHING: Duration = Duration::from_secs(60 * 60 * 24 * 14);

/// What is being asked about.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Subject {
    Album { artist: String, album: String },
    Track { artist: String, title: String },
}

impl Subject {
    /// The text two spellings of the same thing share, such as `Mak'er` and `Maker`.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Album { artist, album } => {
                format!("album\u{1}{}\u{1}{}", normalize(artist), normalize(album))
            }
            Self::Track { artist, title } => {
                format!("track\u{1}{}\u{1}{}", normalize(artist), normalize(title))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    /// When it was asked, in seconds since 1970.
    asked: u64,
    tags: Vec<RawTag>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    version: u32,
    entries: HashMap<String, Entry>,
}

/// The remembered answers.
#[derive(Debug, Default)]
pub struct TagCache {
    entries: HashMap<String, Entry>,
}

impl TagCache {
    /// Reads the cache at `path`. A missing or damaged file is an empty cache: everything is
    /// just asked again.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let entries = fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Stored>(&bytes).ok())
            .filter(|stored| stored.version == VERSION)
            .map(|stored| stored.entries)
            .unwrap_or_default();
        Self { entries }
    }

    /// Writes the cache through a temporary file, so a crash cannot leave half a file behind.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let stored = Stored {
            version: VERSION,
            entries: self.entries.clone(),
        };
        let json = serde_json::to_vec(&stored).context("writing the Last.fm cache")?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, json).with_context(|| format!("writing {}", temporary.display()))?;
        fs::rename(&temporary, path).with_context(|| format!("saving {}", path.display()))?;
        Ok(())
    }

    /// What was remembered about `subject`, if it is recent enough to trust.
    #[must_use]
    pub fn get(&self, subject: &Subject, now: u64) -> Option<&[RawTag]> {
        let entry = self.entries.get(&subject.key())?;
        let keep = if entry.tags.is_empty() {
            KEEP_NOTHING
        } else {
            KEEP_TAGS
        };
        (now.saturating_sub(entry.asked) < keep.as_secs()).then_some(entry.tags.as_slice())
    }

    /// Remembers an answer.
    pub fn put(&mut self, subject: &Subject, now: u64, tags: Vec<RawTag>) {
        self.entries
            .insert(subject.key(), Entry { asked: now, tags });
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn album(artist: &str, album: &str) -> Subject {
        Subject::Album {
            artist: artist.to_string(),
            album: album.to_string(),
        }
    }

    fn rock() -> Vec<RawTag> {
        vec![RawTag {
            name: "rock".to_string(),
            count: 100,
        }]
    }

    const DAY: u64 = 60 * 60 * 24;

    #[test]
    fn spellings_of_the_same_thing_share_an_entry() {
        assert_eq!(
            album("Led Zeppelin", "D'yer Mak'er").key(),
            album("led zeppelin", "Dyer Maker").key()
        );
        assert_ne!(
            album("A", "B").key(),
            Subject::Track {
                artist: "A".to_string(),
                title: "B".to_string()
            }
            .key(),
            "an album and a song with the same words are different questions"
        );
    }

    #[test]
    fn an_answer_is_trusted_for_a_while_and_then_asked_again() {
        let mut cache = TagCache::default();
        let subject = album("Led Zeppelin", "Houses of the Holy");
        assert_eq!(cache.get(&subject, 1_000), None);

        cache.put(&subject, 1_000, rock());
        assert_eq!(
            cache.get(&subject, 1_000 + DAY).map(<[RawTag]>::len),
            Some(1)
        );
        assert_eq!(
            cache.get(&subject, 1_000 + 89 * DAY).map(<[RawTag]>::len),
            Some(1)
        );
        assert_eq!(cache.get(&subject, 1_000 + 91 * DAY), None);
    }

    #[test]
    fn no_tags_is_remembered_but_not_for_as_long() {
        let mut cache = TagCache::default();
        let subject = album("Obscure", "Nothing");
        cache.put(&subject, 1_000, Vec::new());
        assert_eq!(cache.get(&subject, 1_000 + 13 * DAY), Some(&[][..]));
        assert_eq!(cache.get(&subject, 1_000 + 15 * DAY), None);
    }

    #[test]
    fn the_cache_survives_a_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("deeper").join("lastfm.json");
        let mut cache = TagCache::default();
        cache.put(&album("A", "B"), 5_000, rock());
        cache.save(&path).unwrap();

        let loaded = TagCache::load(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded.get(&album("a", "b"), 5_001).map(<[RawTag]>::len),
            Some(1)
        );
        let names: Vec<String> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["lastfm.json"], "no temporary file is left");
    }

    #[test]
    fn a_missing_or_damaged_file_is_an_empty_cache() {
        let dir = tempdir().unwrap();
        assert!(TagCache::load(&dir.path().join("none.json")).is_empty());
        let path = dir.path().join("bad.json");
        fs::write(&path, b"{ not json").unwrap();
        assert!(TagCache::load(&path).is_empty());
        fs::write(&path, br#"{"version": 99, "entries": {}}"#).unwrap();
        assert!(TagCache::load(&path).is_empty());
    }
}
