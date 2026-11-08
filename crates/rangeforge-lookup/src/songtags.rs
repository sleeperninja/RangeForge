//! Song tags the user copied from a song's Last.fm tags page.
//!
//! Last.fm's API has no tags for songs, and its tags page sits behind a bot check, so a person
//! reads the page in a browser and pastes the list in. What was pasted is kept in its own small
//! JSON file, apart from the cache of Last.fm's answers: that one may be cleared at any time,
//! while this is the user's own work and is only ever changed by the user.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::lastfm::RawTag;
use crate::tagcache::Subject;

/// Bump this if the file's layout changes in a way older versions cannot read.
const VERSION: u32 = 1;

/// The most tags kept for one song. A tags page lists far more than anyone needs.
pub const MOST_TAGS: usize = 40;

/// Splits pasted text into tag names. Commas, semicolons, line breaks, tabs, `|` and bullet
/// characters all separate tags, so a list copied from a web page, a spreadsheet or another
/// program's tag field all work. Empty pieces are dropped, a name repeated in other letters
/// is kept once, and the order is kept: on Last.fm's page the most used tag comes first.
#[must_use]
pub fn split_pasted(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for piece in text.split(|c: char| {
        matches!(
            c,
            ',' | ';' | '\n' | '\r' | '\t' | '|' | '•' | '·' | '▪' | '●'
        )
    }) {
        // A list marker such as "- reggae" or "* reggae" is not part of the name.
        let piece = piece.trim();
        let piece = piece
            .strip_prefix("- ")
            .or_else(|| piece.strip_prefix("* "))
            .unwrap_or(piece);
        let name = piece.split_whitespace().collect::<Vec<_>>().join(" ");
        if !name.is_empty() && !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            names.push(name);
        }
    }
    names.truncate(MOST_TAGS);
    names
}

/// Gives tags in their order the popularity the genre rules expect: 100 for the first, 5 fewer
/// for each after, never below 10. The page gives no counts, but its order is most-used first.
#[must_use]
pub fn ranked(names: &[String]) -> Vec<RawTag> {
    names
        .iter()
        .enumerate()
        .map(|(place, name)| RawTag {
            name: name.clone(),
            count: 100u32
                .saturating_sub(u32::try_from(place).unwrap_or(u32::MAX).saturating_mul(5))
                .max(10),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    artist: String,
    title: String,
    /// The tags in the order they were pasted.
    tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    version: u32,
    songs: BTreeMap<String, Entry>,
}

/// The song tags the user has pasted.
#[derive(Debug, Default)]
pub struct SongTags {
    songs: BTreeMap<String, Entry>,
}

impl SongTags {
    /// Reads the file at `path`. A missing or damaged file is simply empty.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let songs = fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Stored>(&bytes).ok())
            .filter(|stored| stored.version == VERSION)
            .map(|stored| stored.songs)
            .unwrap_or_default();
        Self { songs }
    }

    /// Writes the file through a temporary one, so a crash cannot leave half a file behind.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let stored = Stored {
            version: VERSION,
            songs: self.songs.clone(),
        };
        let json = serde_json::to_vec_pretty(&stored).context("writing the song tags")?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, json).with_context(|| format!("writing {}", temporary.display()))?;
        fs::rename(&temporary, path).with_context(|| format!("saving {}", path.display()))?;
        Ok(())
    }

    /// Keeps these tags for a song, replacing any kept before. No tags forgets the song.
    pub fn set(&mut self, artist: &str, title: &str, tags: Vec<String>) {
        let subject = Subject::Track {
            artist: artist.to_string(),
            title: title.to_string(),
        };
        if tags.is_empty() {
            self.songs.remove(&subject.key());
        } else {
            self.songs.insert(
                subject.key(),
                Entry {
                    artist: artist.to_string(),
                    title: title.to_string(),
                    tags,
                },
            );
        }
    }

    /// Every song with kept tags, as the question it answers and its ranked tags.
    #[must_use]
    pub fn answers(&self) -> Vec<(Subject, Vec<RawTag>)> {
        self.songs
            .values()
            .map(|entry| {
                (
                    Subject::Track {
                        artist: entry.artist.clone(),
                        title: entry.title.clone(),
                    },
                    ranked(&entry.tags),
                )
            })
            .collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.songs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn commas_semicolons_and_line_breaks_all_separate_tags() {
        let expected = ["classic rock", "rock", "reggae", "hard rock"];
        assert_eq!(
            split_pasted("classic rock, rock, reggae, hard rock"),
            expected
        );
        assert_eq!(
            split_pasted("classic rock; rock ;reggae;hard rock"),
            expected
        );
        assert_eq!(
            split_pasted("classic rock\nrock\r\nreggae\n\n  hard rock  \n"),
            expected
        );
        assert_eq!(
            split_pasted("classic rock, rock; reggae\nhard rock"),
            expected,
            "mixed separators in one paste"
        );
        assert_eq!(
            split_pasted("• classic rock • rock\t| reggae | hard rock"),
            expected
        );
        assert_eq!(
            split_pasted("- classic rock\n* rock\n- reggae\n- hard rock"),
            expected
        );
    }

    #[test]
    fn a_hyphen_inside_a_tag_stays_and_repeats_and_blanks_go() {
        assert_eq!(
            split_pasted("hip-hop, Hip-Hop, , ;  trip  hop"),
            ["hip-hop", "trip hop"]
        );
        assert_eq!(split_pasted("  \n ; , "), Vec::<String>::new());
    }

    #[test]
    fn only_so_many_tags_are_kept() {
        let long: Vec<String> = (0..100).map(|n| format!("tag {n}")).collect();
        assert_eq!(split_pasted(&long.join(", ")).len(), MOST_TAGS);
    }

    #[test]
    fn order_becomes_a_falling_popularity_that_stops_at_ten() {
        let names: Vec<String> = (0..30).map(|n| format!("t{n}")).collect();
        let tags = ranked(&names);
        assert_eq!(tags[0].count, 100);
        assert_eq!(tags[1].count, 95);
        assert_eq!(tags[29].count, 10);
        assert_eq!(ranked(&[]), []);
    }

    #[test]
    fn pasted_tags_survive_a_round_trip_and_spellings_share_an_entry() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sub").join("song-tags.json");
        let mut kept = SongTags::default();
        kept.set(
            "Led Zeppelin",
            "D\u{2019}yer Mak\u{2019}er",
            vec!["classic rock".to_string(), "reggae".to_string()],
        );
        // The same song spelled with plain apostrophes replaces rather than adds.
        kept.set(
            "led zeppelin",
            "D'yer Mak'er",
            vec!["reggae".to_string(), "rock".to_string()],
        );
        assert_eq!(kept.len(), 1);
        kept.save(&path).unwrap();

        let loaded = SongTags::load(&path);
        let answers = loaded.answers();
        assert_eq!(answers.len(), 1);
        let names: Vec<&str> = answers[0].1.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["reggae", "rock"]);
        assert_eq!(answers[0].1[0].count, 100);

        // Pasting nothing forgets the song.
        let mut loaded = loaded;
        loaded.set("Led Zeppelin", "D'yer Mak'er", Vec::new());
        assert!(loaded.is_empty());
    }

    #[test]
    fn a_missing_or_damaged_file_is_empty_and_no_temporary_file_is_left() {
        let dir = tempdir().unwrap();
        assert!(SongTags::load(&dir.path().join("none.json")).is_empty());
        let path = dir.path().join("bad.json");
        fs::write(&path, b"{ not json").unwrap();
        assert!(SongTags::load(&path).is_empty());
        fs::write(&path, br#"{"version": 99, "songs": {}}"#).unwrap();
        assert!(SongTags::load(&path).is_empty());

        let good = dir.path().join("good.json");
        SongTags::default().save(&good).unwrap();
        let mut names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["bad.json", "good.json"]);
    }
}
