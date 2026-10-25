//! A song's genres: what its file already says, what Last.fm's listeners call its album, what
//! stands out about the song itself, and finally Radio.
//!
//! This is shown as chips in the Tags column, and written to the genre tag (comma separated,
//! Radio last) when files are written.

use std::collections::HashMap;

use rangeforge_core::identify::Identity;
use rangeforge_lookup::genres::{Rules, pick};
use rangeforge_lookup::lastfm::RawTag;
use rangeforge_lookup::tagcache::Subject;

/// What Last.fm has said so far, by question.
#[derive(Debug, Default)]
pub struct Answers {
    by_key: HashMap<String, Vec<RawTag>>,
}

impl Answers {
    pub fn insert(&mut self, subject: &Subject, tags: Vec<RawTag>) {
        self.by_key.insert(subject.key(), tags);
    }

    #[must_use]
    pub fn get(&self, subject: &Subject) -> Option<&[RawTag]> {
        self.by_key.get(&subject.key()).map(Vec::as_slice)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }
}

/// The genres of one song, grouped the way the screen colours them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SongGenres {
    /// Already in the file or CUE sheet, and the album's, from Last.fm. Plain chips.
    pub album: Vec<String>,
    /// What stands out about this song alone. Pink chips.
    pub song: Vec<String>,
}

impl SongGenres {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.album.is_empty() && self.song.is_empty()
    }
}

/// The album a song belongs to, as Last.fm is asked about it: the album's artist if known, or
/// else the song's.
fn album_subject(identity: &Identity) -> Option<Subject> {
    let artist = identity
        .album_artist
        .as_deref()
        .or(identity.artist.as_deref())?;
    let album = identity.album.as_deref()?;
    Some(Subject::Album {
        artist: artist.to_string(),
        album: album.to_string(),
    })
}

fn track_subject(identity: &Identity) -> Option<Subject> {
    Some(Subject::Track {
        artist: identity.artist.clone()?,
        title: identity.title.clone()?,
    })
}

/// What Last.fm would need to be asked about this song: its album and the song itself, as far as
/// the identity knows them.
#[must_use]
pub fn questions(identity: &Identity) -> Vec<Subject> {
    album_subject(identity)
        .into_iter()
        .chain(track_subject(identity))
        .collect()
}

/// Splits a genre tag that holds several: `Rock, Hard Rock` or `Rock; Blues`.
#[must_use]
pub fn split_genres(text: &str) -> Vec<String> {
    text.split([',', ';'])
        .map(str::trim)
        .filter(|genre| !genre.is_empty())
        .map(str::to_string)
        .collect()
}

fn add_new(list: &mut Vec<String>, more: impl IntoIterator<Item = String>) {
    for genre in more {
        if !list.iter().any(|g| g.eq_ignore_ascii_case(&genre)) {
            list.push(genre);
        }
    }
}

/// The genres for a song, from what is known so far. Whatever Last.fm has not answered yet is
/// simply missing, and appears when it does.
#[must_use]
pub fn song_genres(identity: &Identity, answers: &Answers) -> SongGenres {
    let mut album = Vec::new();
    // What the file or sheet already says comes first, as the person who tagged it chose.
    if let Some(existing) = &identity.genre {
        add_new(&mut album, split_genres(existing));
    }
    let artist = identity
        .album_artist
        .as_deref()
        .or(identity.artist.as_deref())
        .unwrap_or_default();
    if let Some(tags) =
        album_subject(identity).and_then(|subject| answers.get(&subject).map(<[RawTag]>::to_vec))
    {
        let picked = pick(&tags, artist, &album, Rules::ALBUM);
        add_new(&mut album, picked);
    }

    let mut song = Vec::new();
    if let (Some(subject), Some(artist)) = (track_subject(identity), identity.artist.as_deref())
        && let Some(tags) = answers.get(&subject)
    {
        song = pick(tags, artist, &album, Rules::SONG);
    }
    SongGenres { album, song }
}

/// The text for the genre tag: the genres in order, then Radio if the song charted, joined with
/// a comma and a space. `None` when there is nothing to say, so an existing tag is left alone.
#[must_use]
pub fn genre_tag(genres: &SongGenres, radio: bool) -> Option<String> {
    let mut all = genres.album.clone();
    add_new(&mut all, genres.song.clone());
    if radio {
        add_new(&mut all, ["Radio".to_string()]);
    }
    (!all.is_empty()).then(|| all.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rangeforge_core::identify::IdentitySource;

    fn tag(name: &str, count: u32) -> RawTag {
        RawTag {
            name: name.to_string(),
            count,
        }
    }

    fn identity() -> Identity {
        Identity {
            artist: Some("Led Zeppelin".to_string()),
            title: Some("The Crunge".to_string()),
            album: Some("Houses of the Holy".to_string()),
            album_artist: Some("Led Zeppelin".to_string()),
            ..Identity::unknown(IdentitySource::CueSheet)
        }
    }

    fn answers() -> Answers {
        let mut answers = Answers::default();
        answers.insert(
            &album_subject(&identity()).unwrap(),
            vec![
                tag("classic rock", 100),
                tag("hard rock", 80),
                tag("seen live", 70),
            ],
        );
        answers.insert(
            &track_subject(&identity()).unwrap(),
            vec![
                tag("funk rock", 100),
                tag("hard rock", 90),
                tag("classic rock", 60),
            ],
        );
        answers
    }

    #[test]
    fn album_genres_are_plain_and_song_extras_are_the_ones_that_differ() {
        let genres = song_genres(&identity(), &answers());
        assert_eq!(genres.album, ["Classic Rock", "Hard Rock"]);
        assert_eq!(genres.song, ["Funk Rock"]);
    }

    #[test]
    fn what_the_file_already_says_comes_first_and_is_not_repeated() {
        let mut known = identity();
        known.genre = Some("Rock; Hard Rock".to_string());
        let genres = song_genres(&known, &answers());
        assert_eq!(genres.album, ["Rock", "Hard Rock", "Classic Rock"]);
    }

    #[test]
    fn without_answers_only_the_existing_genre_shows() {
        let mut known = identity();
        known.genre = Some("Rock".to_string());
        let genres = song_genres(&known, &Answers::default());
        assert_eq!(genres.album, ["Rock"]);
        assert_eq!(genres.song, Vec::<String>::new());
        assert_eq!(
            song_genres(&identity(), &Answers::default()),
            SongGenres::default()
        );
    }

    #[test]
    fn the_genre_tag_is_comma_separated_with_radio_last() {
        let genres = song_genres(&identity(), &answers());
        assert_eq!(
            genre_tag(&genres, true).as_deref(),
            Some("Classic Rock, Hard Rock, Funk Rock, Radio")
        );
        assert_eq!(
            genre_tag(&genres, false).as_deref(),
            Some("Classic Rock, Hard Rock, Funk Rock")
        );
    }

    #[test]
    fn radio_alone_is_a_genre_and_nothing_at_all_leaves_the_tag_alone() {
        assert_eq!(
            genre_tag(&SongGenres::default(), true).as_deref(),
            Some("Radio")
        );
        assert_eq!(genre_tag(&SongGenres::default(), false), None);
    }

    #[test]
    fn an_existing_radio_genre_is_not_doubled() {
        let genres = SongGenres {
            album: vec!["Rock".to_string(), "radio".to_string()],
            song: Vec::new(),
        };
        assert_eq!(genre_tag(&genres, true).as_deref(), Some("Rock, radio"));
    }

    #[test]
    fn a_song_asks_about_its_album_and_itself_as_far_as_it_is_known() {
        assert_eq!(questions(&identity()).len(), 2);
        let no_album = Identity {
            album: None,
            ..identity()
        };
        assert_eq!(questions(&no_album).len(), 1);
        let nothing = Identity::unknown(IdentitySource::FileName);
        assert_eq!(questions(&nothing), Vec::<Subject>::new());
    }

    #[test]
    fn genre_tags_that_hold_several_are_split() {
        assert_eq!(
            split_genres("Rock, Hard Rock ;Blues"),
            ["Rock", "Hard Rock", "Blues"]
        );
        assert_eq!(split_genres(" , ;"), Vec::<String>::new());
    }

    #[test]
    fn a_songs_album_falls_back_to_its_own_artist_when_no_album_artist_is_known() {
        let identity = Identity {
            album_artist: None,
            ..identity()
        };
        assert_eq!(
            album_subject(&identity).unwrap().key(),
            Subject::Album {
                artist: "Led Zeppelin".to_string(),
                album: "Houses of the Holy".to_string()
            }
            .key()
        );
    }
}
