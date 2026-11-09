//! Lining songs up with an album's track list, to name many songs at once.
//!
//! The list comes from Last.fm (`album.getInfo`) or from text the user pasted (a track list
//! copied from a store page). Each song is matched to at most one entry, and the user can
//! change any pairing before it is applied, so the rules here only have to be a good first
//! guess. A song's length is not used yet: the measurements do not carry it.

use rangeforge_lookup::lastfm::ListedTrack;
use rangeforge_lookup::normalize::normalize;
use serde::Serialize;

/// What is known about a song before it is matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Song {
    /// What the row says: the file name, or a CUE track's title.
    pub label: String,
    /// Its track number, from the file's tags or a guess from its name.
    pub number: Option<u32>,
    /// Its title as far as it is known (often junk when the name was a guess).
    pub title: Option<String>,
}

/// How a song came to be paired with a list entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum How {
    /// The entry's title is the song's title, or is found in its name.
    Title,
    /// Only the track numbers agree.
    Number,
    /// Neither: what was left over was lined up in order, because the counts agree.
    Order,
}

/// One song paired with one list entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Pairing {
    /// The place in the list.
    pub item: usize,
    pub how: How,
}

/// How strongly the text of `song` points at `title`: 6 for the same words, 4 when the title's
/// words appear whole inside the song's name, 0 for neither.
fn title_score(song: &Song, title: &str) -> u32 {
    let wanted = normalize(title);
    if wanted.is_empty() {
        return 0;
    }
    let own = song.title.as_deref().map(normalize).unwrap_or_default();
    let label = normalize(&song.label);
    if own == wanted || label == wanted {
        return 6;
    }
    let padded = format!(" {wanted} ");
    let found_in = |text: &str| format!(" {text} ").contains(&padded);
    if found_in(&own) || found_in(&label) {
        4
    } else {
        0
    }
}

/// A song as the window shows it in the matching dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanSong {
    pub group: usize,
    pub track: usize,
    /// What the row says, such as the file name.
    pub label: String,
    pub number: Option<u32>,
    pub title: Option<String>,
}

/// Everything the matching dialog needs: the songs, the list, and the first guess at who is who.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchPlan {
    /// The album the list is for, as the songs say it (and as Last.fm was asked).
    pub artist: String,
    pub album: String,
    pub songs: Vec<PlanSong>,
    pub list: Vec<ListedTrack>,
    /// For each song, the entry it was paired with, if any.
    pub pairs: Vec<Option<Pairing>>,
}

/// Pairs each song with the list entry it most likely is, or `None`.
///
/// 1. Pairs whose title matches (best), or whose track numbers agree, are taken strongest first.
///    A longer title wins over a shorter one it contains (`Someday Soon` over `Someday`), and
///    the nearer place in the list breaks a tie.
/// 2. If songs and entries are left over in equal numbers, they are lined up in order.
///
/// A number alone counts, but is the weakest evidence: vinyl rips often number by side.
#[must_use]
pub fn assign(songs: &[Song], list: &[ListedTrack]) -> Vec<Option<Pairing>> {
    struct Candidate {
        score: u32,
        length: usize,
        distance: usize,
        song: usize,
        item: usize,
    }
    let mut candidates = Vec::new();
    for (s, song) in songs.iter().enumerate() {
        for (i, entry) in list.iter().enumerate() {
            let mut score = title_score(song, &entry.title);
            if song.number.is_some_and(|n| n == entry.number) {
                score += 3;
            }
            if score >= 3 {
                candidates.push(Candidate {
                    score,
                    length: normalize(&entry.title).len(),
                    distance: s.abs_diff(i),
                    song: s,
                    item: i,
                });
            }
        }
    }
    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(b.length.cmp(&a.length))
            .then(a.distance.cmp(&b.distance))
            .then(a.song.cmp(&b.song))
    });

    let mut result: Vec<Option<Pairing>> = vec![None; songs.len()];
    let mut taken = vec![false; list.len()];
    for candidate in candidates {
        if result[candidate.song].is_none() && !taken[candidate.item] {
            taken[candidate.item] = true;
            // Six or four is the title; with a title only partly there it is the number.
            let by_title = title_score(&songs[candidate.song], &list[candidate.item].title) > 0;
            result[candidate.song] = Some(Pairing {
                item: candidate.item,
                how: if by_title { How::Title } else { How::Number },
            });
        }
    }

    let loose_songs: Vec<usize> = (0..songs.len()).filter(|s| result[*s].is_none()).collect();
    let loose_items: Vec<usize> = (0..list.len()).filter(|i| !taken[*i]).collect();
    if !loose_songs.is_empty() && loose_songs.len() == loose_items.len() {
        for (song, item) in loose_songs.into_iter().zip(loose_items) {
            result[song] = Some(Pairing {
                item,
                how: How::Order,
            });
        }
    }
    result
}

/// Reads a pasted track list, one title per line. A leading track number (`1.`, `01 -`, `(1)`,
/// `01 `) and a trailing length (`3:45`, `(3:45)`) are taken off; blank lines are skipped. Where
/// a line has no number it gets the next one in order.
#[must_use]
pub fn parse_pasted(text: &str) -> Vec<ListedTrack> {
    let mut tracks: Vec<ListedTrack> = Vec::new();
    for line in text.lines() {
        let (number, rest) = strip_number(line.trim());
        let (title, seconds) = strip_length(rest);
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        if title.is_empty() {
            continue;
        }
        let fallback = u32::try_from(tracks.len() + 1).unwrap_or(u32::MAX);
        tracks.push(ListedTrack {
            number: number.unwrap_or(fallback),
            title,
            seconds,
        });
    }
    tracks
}

/// A track number at the front of a line. Only a number that is clearly one is taken: it needs
/// a `.`, `)`, `:` or `-` after it, brackets round it, or a leading zero. A bare `99 Luftballons`
/// stays whole.
fn strip_number(line: &str) -> (Option<u32>, &str) {
    let bracketed = line.strip_prefix('(');
    let body = bracketed.unwrap_or(line);
    let digits: String = body.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 3 {
        return (None, line);
    }
    let after = &body[digits.len()..];
    let number = digits.parse().ok();
    if bracketed.is_some() {
        return match after.strip_prefix(')') {
            Some(rest) => (number, rest.trim_start_matches([' ', '.', '-', ':']).trim()),
            None => (None, line),
        };
    }
    let spaced = after.trim_start();
    let marked = spaced.starts_with(['.', ')', ':', '-']);
    let zero_padded = digits.len() > 1 && digits.starts_with('0') && after.starts_with(' ');
    if marked || zero_padded {
        let rest = spaced.trim_start_matches(['.', ')', ':', '-']).trim();
        if !rest.is_empty() {
            return (number, rest);
        }
    }
    (None, line)
}

/// A length at the end of a line: `Title 3:45`, `Title (3:45)` or `Title<tab>3:45`.
fn strip_length(line: &str) -> (&str, Option<u32>) {
    let trimmed = line.trim_end();
    let Some((head, last)) = trimmed.rsplit_once(char::is_whitespace) else {
        return (trimmed, None);
    };
    let time = last.trim_matches(['(', ')', '[', ']']);
    let parts: Vec<&str> = time.split(':').collect();
    let numeric = (2..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 2 && p.bytes().all(|b| b.is_ascii_digit()));
    if !numeric {
        return (trimmed, None);
    }
    let seconds = parts.iter().try_fold(0_u32, |total, p| {
        p.parse::<u32>().ok().map(|n| total * 60 + n)
    });
    // A trailing dash left by "Title - 3:45" is not part of the title.
    (
        head.trim_end().trim_end_matches(['-', '–', '—']).trim_end(),
        seconds,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(label: &str, number: Option<u32>, title: Option<&str>) -> Song {
        Song {
            label: label.to_string(),
            number,
            title: title.map(str::to_string),
        }
    }

    fn entry(number: u32, title: &str) -> ListedTrack {
        ListedTrack {
            number,
            title: title.to_string(),
            seconds: None,
        }
    }

    fn some_cities() -> Vec<ListedTrack> {
        vec![
            entry(1, "Some Cities"),
            entry(2, "Black and White Town"),
            entry(3, "Almost Forgot Myself"),
            entry(4, "Snowden"),
            entry(5, "The Storm"),
            entry(6, "Walk in Fire"),
            entry(7, "One Of These Days"),
            entry(8, "Someday Soon"),
            entry(9, "Shadows Of Salford"),
            entry(10, "Sky Starts Falling"),
            entry(11, "Ambition"),
        ]
    }

    #[test]
    fn songs_named_like_their_entries_are_paired_whatever_their_order_and_capitals() {
        // The store's capitals differ from Last.fm's, the files are out of order, and one
        // song's name still holds the whole barcode name.
        let songs = [
            song(
                "00724387460928_006_Doves_Walk In Fire_GBDGG0400081",
                Some(6),
                Some("Walk In Fire"),
            ),
            song("Snowden", Some(4), Some("Snowden")),
            song("x", Some(2), Some("Black And White Town")),
            song(
                "00724387460928_001_Doves_Some Cities_GBDGG0400080",
                None,
                None,
            ),
        ];
        let paired = assign(&songs, &some_cities());
        let items: Vec<Option<usize>> = paired.iter().map(|p| p.map(|p| p.item)).collect();
        assert_eq!(items, [Some(5), Some(3), Some(1), Some(0)]);
        assert!(paired.iter().flatten().all(|p| p.how == How::Title));
    }

    #[test]
    fn a_longer_title_wins_over_a_shorter_one_inside_it() {
        let list = vec![entry(1, "Someday"), entry(2, "Someday Soon")];
        let songs = [song("Doves Someday Soon", None, None)];
        assert_eq!(assign(&songs, &list)[0].map(|p| p.item), Some(1));
    }

    #[test]
    fn a_number_alone_pairs_but_is_marked_as_the_weak_evidence_it_is() {
        let songs = [
            song("Track 04", Some(4), None),
            song("Track 05", Some(5), None),
        ];
        let paired = assign(&songs, &some_cities());
        assert_eq!(
            paired[0],
            Some(Pairing {
                item: 3,
                how: How::Number
            })
        );
        assert_eq!(
            paired[1],
            Some(Pairing {
                item: 4,
                how: How::Number
            })
        );
    }

    #[test]
    fn what_is_left_over_in_equal_numbers_is_lined_up_in_order() {
        // Nothing to go on but the order: three songs, three entries.
        let songs = [
            song("a", None, None),
            song("b", None, None),
            song("c", None, None),
        ];
        let list = vec![entry(1, "One"), entry(2, "Two"), entry(3, "Three")];
        let paired = assign(&songs, &list);
        let items: Vec<usize> = paired.iter().map(|p| p.unwrap().item).collect();
        assert_eq!(items, [0, 1, 2]);
        assert!(paired.iter().flatten().all(|p| p.how == How::Order));

        // Unequal numbers: no guess at all.
        let two = [song("a", None, None), song("b", None, None)];
        assert_eq!(assign(&two, &list), [None, None]);
    }

    #[test]
    fn no_entry_is_given_to_two_songs_and_an_empty_list_pairs_nothing() {
        let songs = [
            song("Snowden", None, None),
            song("Snowden (live)", None, None),
        ];
        let paired = assign(&songs, &[entry(1, "Snowden")]);
        assert_eq!(paired.iter().flatten().count(), 1);
        assert_eq!(assign(&songs, &[]), [None, None]);
        assert_eq!(assign(&[], &some_cities()), Vec::<Option<Pairing>>::new());
    }

    #[test]
    fn a_pasted_list_loses_its_numbers_and_times_but_keeps_its_titles() {
        let text = "1. Some Cities\n02 - Black And White Town  4:17\n(3) Almost Forgot Myself (4:27)\n\n\
                    4) Snowden\t4:10\nThe Storm\n99 Luftballons\n  \n7: One Of These Days - 4:50\n";
        let tracks = parse_pasted(text);
        let shown: Vec<(u32, &str, Option<u32>)> = tracks
            .iter()
            .map(|t| (t.number, t.title.as_str(), t.seconds))
            .collect();
        assert_eq!(
            shown,
            [
                (1, "Some Cities", None),
                (2, "Black And White Town", Some(257)),
                (3, "Almost Forgot Myself", Some(267)),
                (4, "Snowden", Some(250)),
                // No number: its place among the lines.
                (5, "The Storm", None),
                // A bare number with only a space after it is part of the title.
                (6, "99 Luftballons", None),
                (7, "One Of These Days", Some(290)),
            ]
        );
        assert_eq!(parse_pasted(" \n\n"), Vec::<ListedTrack>::new());
    }

    #[test]
    fn a_pasted_whole_block_with_hours_and_odd_spacing_still_reads() {
        let tracks = parse_pasted("1.\tFiresite\t1:02:03\n2.   Here It Comes   \n");
        assert_eq!(tracks[0].title, "Firesite");
        assert_eq!(tracks[0].seconds, Some(3723));
        assert_eq!(tracks[1].title, "Here It Comes");
        assert_eq!(tracks[1].number, 2);
    }
}
