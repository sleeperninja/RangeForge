//! Turning Last.fm's tags into genres.
//!
//! Last.fm's tags are whatever listeners typed, so alongside real genres (`classic rock`) there
//! are opinions (`awesome`), places (`british`), habits (`seen live`, `albums I own`), decades
//! (`70s`) and the artist's own name. Only the real genres are kept, the most popular first, and
//! tidied into the capitals genres are normally written with.

use crate::lastfm::RawTag;
use crate::normalize::normalize;

/// Tags that are never genres.
const JUNK: &[&str] = &[
    "seen live",
    "favorites",
    "favourites",
    "favorite",
    "favourite",
    "favorite songs",
    "favourite songs",
    "favorite albums",
    "favourite albums",
    "albums i own",
    "owned",
    "my music",
    "to listen",
    "spotify",
    "check out",
    "male vocalists",
    "female vocalists",
    "male vocalist",
    "female vocalist",
    "legend",
    "legends",
    "awesome",
    "amazing",
    "love",
    "beautiful",
    "good",
    "great",
    "best",
    "cool",
    "epic",
    "all",
    "albums",
    "album",
    "single",
    "mp3",
    "cd",
    "vinyl",
    "lp",
    "ep",
    "british",
    "american",
    "english",
    "uk",
    "usa",
    "us",
    "australian",
    "canadian",
    "swedish",
    "german",
    "french",
    "irish",
    "scottish",
    "welsh",
    "canada",
    "under 2000 listeners",
    "overrated",
];

/// A tag that is a decade or a year: `70s`, `1970s`, `1973`.
fn is_period(name: &str) -> bool {
    let digits = name.trim_end_matches('s');
    !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
        && matches!(digits.len(), 2 | 4)
}

fn is_junk(name: &str) -> bool {
    JUNK.contains(&name)
        || is_period(name)
        || name.contains("favorite")
        || name.contains("favourite")
        || name.contains("seen live")
}

/// Words left small in the middle of a name: `Drum and Bass`.
const SMALL_WORDS: [&str; 5] = ["and", "of", "the", "in", "n"];

/// `classic rock` becomes `Classic Rock`, `hip-hop` `Hip-Hop`, `r&b` `R&B`, `drum and bass`
/// `Drum and Bass`.
#[must_use]
pub fn title_case(name: &str) -> String {
    fn capitalise(word: &str) -> String {
        let mut chars = word.chars();
        chars.next().map_or_else(String::new, |first| {
            first.to_uppercase().chain(chars).collect::<String>()
        })
    }
    name.split_whitespace()
        .enumerate()
        .map(|(index, word)| {
            if word.contains('&') && word.chars().count() <= 4 {
                // R&B, D&B.
                word.to_uppercase()
            } else if index > 0 && SMALL_WORDS.contains(&word) {
                word.to_string()
            } else {
                word.split('-')
                    .map(capitalise)
                    .collect::<Vec<_>>()
                    .join("-")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// How many genres to keep and how popular a tag must be to count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    /// At most this many.
    pub limit: usize,
    /// Tags below this popularity (0 to 100) are dropped, except that the first usable tag is
    /// always kept, so a song has a genre if there is any.
    pub min_count: u32,
}

impl Rules {
    /// For a whole album: the top three that are clearly used.
    pub const ALBUM: Self = Self {
        limit: 3,
        min_count: 25,
    };
    /// For one song: only what stands out, and in addition to the album's.
    pub const SONG: Self = Self {
        limit: 4,
        min_count: 15,
    };
}

/// The genres among `tags`, best first. `artist` is dropped if used as a tag, and so is
/// anything in `already` (the album's genres, when picking a song's extras).
#[must_use]
pub fn pick(tags: &[RawTag], artist: &str, already: &[String], rules: Rules) -> Vec<String> {
    let artist = normalize(artist);
    let mut sorted: Vec<&RawTag> = tags.iter().collect();
    sorted.sort_by_key(|tag| std::cmp::Reverse(tag.count));

    let mut picked: Vec<String> = Vec::new();
    let mut seen_any = false;
    for tag in sorted {
        if picked.len() >= rules.limit {
            break;
        }
        let name = tag.name.trim().to_lowercase();
        let bare = normalize(&name);
        // The artist's name as whole words, so `Yes` does not remove `eyes`.
        let names_the_artist =
            !artist.is_empty() && format!(" {bare} ").contains(&format!(" {artist} "));
        if name.is_empty() || is_junk(&name) || names_the_artist {
            continue;
        }
        let shown = title_case(&name);
        let known = |list: &[String]| list.iter().any(|g| g.eq_ignore_ascii_case(&shown));
        if known(already) || known(&picked) {
            continue;
        }
        // The first usable tag is kept whatever its popularity; the rest must be popular.
        if seen_any && tag.count < rules.min_count {
            continue;
        }
        seen_any = true;
        picked.push(shown);
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(name: &str, count: u32) -> RawTag {
        RawTag {
            name: name.to_string(),
            count,
        }
    }

    fn houses() -> Vec<RawTag> {
        vec![
            tag("seen live", 90),
            tag("classic rock", 100),
            tag("Led Zeppelin", 95),
            tag("hard rock", 88),
            tag("70s", 70),
            tag("rock", 60),
            tag("british", 55),
            tag("awesome", 50),
            tag("blues rock", 30),
            tag("progressive rock", 12),
        ]
    }

    #[test]
    fn real_genres_survive_and_the_rest_is_dropped() {
        let genres = pick(&houses(), "Led Zeppelin", &[], Rules::ALBUM);
        assert_eq!(genres, ["Classic Rock", "Hard Rock", "Rock"]);
    }

    #[test]
    fn the_limit_and_the_popularity_cutoff_both_apply() {
        let more = pick(
            &houses(),
            "Led Zeppelin",
            &[],
            Rules {
                limit: 10,
                min_count: 25,
            },
        );
        // Blues Rock (30) is popular enough, Progressive Rock (12) is not.
        assert_eq!(more, ["Classic Rock", "Hard Rock", "Rock", "Blues Rock"]);
    }

    #[test]
    fn the_first_usable_tag_is_kept_even_when_unpopular() {
        let faint = [
            tag("seen live", 100),
            tag("shoegaze", 6),
            tag("noise pop", 5),
        ];
        assert_eq!(
            pick(&faint, "Nobody", &[], Rules::ALBUM),
            ["Shoegaze"],
            "one genre beats none, but the second is below the cutoff"
        );
    }

    #[test]
    fn a_songs_extras_leave_out_what_the_album_already_has() {
        let song = [
            tag("hard rock", 100),
            tag("classic rock", 90),
            tag("funk rock", 70),
            tag("funk", 45),
            tag("reggae", 41),
        ];
        let album = vec!["Classic Rock".to_string(), "Hard Rock".to_string()];
        assert_eq!(
            pick(&song, "Led Zeppelin", &album, Rules::SONG),
            ["Funk Rock", "Funk", "Reggae"]
        );
    }

    #[test]
    fn a_rarer_but_distinct_song_tag_still_shows() {
        // D'yer Mak'er: mostly rock, but listeners also call it reggae.
        let song = [
            tag("classic rock", 100),
            tag("rock", 80),
            tag("reggae", 22),
            tag("hard rock", 60),
            tag("led zeppelin", 50),
            tag("70s", 40),
            tag("seen live", 12),
        ];
        let album = vec![
            "Classic Rock".to_string(),
            "Rock".to_string(),
            "Hard Rock".to_string(),
        ];
        assert_eq!(pick(&song, "Led Zeppelin", &album, Rules::SONG), ["Reggae"]);
    }

    #[test]
    fn the_artists_own_name_is_never_a_genre() {
        let tags = [
            tag("the killers", 100),
            tag("killers", 90),
            tag("indie rock", 80),
        ];
        assert_eq!(
            pick(&tags, "The Killers", &[], Rules::ALBUM),
            ["Indie Rock"]
        );
    }

    #[test]
    fn a_short_artist_name_only_removes_tags_that_say_it_as_a_word() {
        let tags = [
            tag("yes", 100),
            tag("yes band", 90),
            tag("eyes", 80),
            tag("prog rock", 70),
        ];
        assert_eq!(pick(&tags, "Yes", &[], Rules::ALBUM), ["Eyes", "Prog Rock"]);
    }

    #[test]
    fn decades_and_years_are_not_genres() {
        for name in ["70s", "1970s", "80s", "1973", "00s"] {
            assert!(is_period(name), "{name}");
        }
        for name in ["rock", "blues rock", "4 non blondes", "drum and bass"] {
            assert!(!is_period(name), "{name}");
        }
    }

    #[test]
    fn names_get_the_capitals_genres_are_written_with() {
        assert_eq!(title_case("classic rock"), "Classic Rock");
        assert_eq!(title_case("hip-hop"), "Hip-Hop");
        assert_eq!(title_case("r&b"), "R&B");
        assert_eq!(title_case("drum and bass"), "Drum and Bass");
        assert_eq!(title_case("rock 'n' roll"), "Rock 'n' Roll");
        assert_eq!(title_case(""), "");
    }

    #[test]
    fn nothing_usable_gives_nothing() {
        assert_eq!(pick(&[], "A", &[], Rules::ALBUM), Vec::<String>::new());
        let junk = [tag("seen live", 100), tag("favorites", 90)];
        assert_eq!(pick(&junk, "A", &[], Rules::ALBUM), Vec::<String>::new());
    }

    #[test]
    fn the_same_genre_twice_is_kept_once() {
        let tags = [tag("rock", 100), tag("Rock", 90), tag("ROCK", 80)];
        assert_eq!(pick(&tags, "A", &[], Rules::ALBUM), ["Rock"]);
    }
}
