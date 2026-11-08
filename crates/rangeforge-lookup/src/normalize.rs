//! Making artist and title text comparable.
//!
//! The same song is written many ways: `D'yer Mak'er`, `Dyer Maker`, `Simon & Garfunkel`,
//! `Simon and Garfunkel`, `Song (Remastered 2009)`, `Beyoncé`. Both sides of a comparison go
//! through the same steps, so they only have to agree after the steps, not before.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Lower case, no accents, no apostrophes or quotes, `&` as `and`, other punctuation as a
/// space, single spaces, and without a leading "the".
#[must_use]
pub fn normalize(text: &str) -> String {
    let mut spaced = String::with_capacity(text.len());
    for c in text.nfd().filter(|c| !is_combining_mark(*c)) {
        match c {
            '\'' | '\u{2019}' | '\u{2018}' | '`' | '"' | '\u{201c}' | '\u{201d}' => {}
            '&' => spaced.push_str(" and "),
            c if c.is_alphanumeric() => spaced.extend(c.to_lowercase()),
            _ => spaced.push(' '),
        }
    }
    let words: Vec<&str> = spaced.split_whitespace().collect();
    let words = match words.as_slice() {
        ["the", rest @ ..] if !rest.is_empty() => rest,
        all => all,
    };
    words.join(" ")
}

/// `text` with every `(...)` and `[...]` group removed.
fn without_brackets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0_u32;
    for c in text.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Words that mark the part after " - " as a version note rather than part of the title.
const VERSION_WORDS: [&str; 12] = [
    "remaster", "version", "mix", "edit", "live", "mono", "stereo", "single", "album", "demo",
    "acoustic", "radio",
];

/// `Title - Remastered 2009` becomes `Title`. A hyphen that is part of a title is kept: only
/// a suffix that talks about versions is cut.
fn without_version_note(text: &str) -> String {
    match text.rsplit_once(" - ") {
        Some((title, note))
            if VERSION_WORDS
                .iter()
                .any(|word| note.to_lowercase().contains(word)) =>
        {
            title.to_string()
        }
        _ => text.to_string(),
    }
}

/// Every way a title can reasonably be compared: as written, without bracketed notes such as
/// `(Remastered)` or `(feat. X)`, and without a trailing version note.
#[must_use]
pub fn title_keys(title: &str) -> Vec<String> {
    let bare = without_brackets(title);
    let mut keys = Vec::new();
    for variant in [
        title.to_string(),
        bare.clone(),
        without_version_note(title),
        without_version_note(&bare),
    ] {
        let key = normalize(&variant);
        if !key.is_empty() && !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

/// Typographic punctuation as plain keyboard characters: `D’yer` becomes `D'yer`.
fn straightened(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{02bc}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{2013}' | '\u{2014}' => '-',
            c => c,
        })
        .collect()
}

/// The spellings of a title or album worth asking an online service about, as written first:
/// then with plain punctuation, without bracketed notes such as `(Remastered)`, and without a
/// trailing `- 2007 Remaster`. Case is kept, because services show names as people write them.
#[must_use]
pub fn spellings(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let plain = straightened(text);
    let bare = without_brackets(&plain);
    for variant in [
        text.to_string(),
        plain.clone(),
        bare.clone(),
        without_version_note(&plain),
        without_version_note(&bare),
    ] {
        let variant = variant.split_whitespace().collect::<Vec<_>>().join(" ");
        if !variant.is_empty() && !found.contains(&variant) {
            found.push(variant);
        }
    }
    found
}

/// The spellings of an artist worth asking about: as written, with plain punctuation, and with
/// a leading "The" added or taken away (`Alan Parsons Project` and `The Alan Parsons Project`).
#[must_use]
pub fn artist_spellings(artist: &str) -> Vec<String> {
    let mut found = spellings(artist);
    let plain = straightened(artist).trim().to_string();
    let toggled = match plain
        .strip_prefix("The ")
        .or_else(|| plain.strip_prefix("the "))
    {
        Some(rest) if !rest.is_empty() => rest.to_string(),
        _ if !plain.is_empty() => format!("The {plain}"),
        _ => String::new(),
    };
    if !toggled.is_empty() && !found.contains(&toggled) {
        found.push(toggled);
    }
    found
}

#[cfg(test)]
mod spelling_tests {
    use super::*;

    #[test]
    fn titles_are_offered_as_written_then_plainer() {
        assert_eq!(
            spellings("D\u{2019}yer Mak\u{2019}er (Remastered 2014)"),
            [
                "D\u{2019}yer Mak\u{2019}er (Remastered 2014)",
                "D'yer Mak'er (Remastered 2014)",
                "D'yer Mak'er",
            ]
        );
        assert_eq!(
            spellings("Black Dog - 2007 Remaster"),
            ["Black Dog - 2007 Remaster", "Black Dog"]
        );
        // A hyphen that is part of the title stays.
        assert_eq!(spellings("Mother-in-Law"), ["Mother-in-Law"]);
        assert_eq!(spellings("  ").len(), 0);
    }

    #[test]
    fn artists_also_get_the_added_or_taken_away() {
        assert_eq!(
            artist_spellings("Alan Parsons Project"),
            ["Alan Parsons Project", "The Alan Parsons Project"]
        );
        assert_eq!(artist_spellings("The Killers"), ["The Killers", "Killers"]);
    }
}

/// Words that join a lead artist to a guest: `Beyoncé Featuring Jay-Z`.
const GUEST_JOINERS: [&str; 6] = [
    " featuring ",
    " feat. ",
    " feat ",
    " ft. ",
    " ft ",
    " with ",
];

/// Every way an artist can reasonably be compared: the whole credit, and the lead artist
/// alone when guests are named. `Simon & Garfunkel` stays whole, since `&` can be part of a
/// name.
#[must_use]
pub fn artist_keys(artist: &str) -> Vec<String> {
    let lower = artist.to_lowercase();
    let cut = GUEST_JOINERS
        .iter()
        .filter_map(|joiner| lower.find(joiner))
        .min();
    let mut keys = vec![normalize(artist)];
    if let Some(at) = cut {
        // `at` is a position in the lower-case copy; accents and case do not change byte
        // positions for the ASCII joiners it was found by, but guard against odd text anyway.
        if let Some(lead) = artist.get(..at) {
            keys.push(normalize(lead));
        }
    }
    keys.retain(|key| !key.is_empty());
    keys.dedup();
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spelling_variants_meet_in_the_middle() {
        assert_eq!(normalize("D'yer Mak'er"), "dyer maker");
        assert_eq!(normalize("D\u{2019}yer Mak\u{2019}er"), "dyer maker");
        assert_eq!(normalize("Simon & Garfunkel"), "simon and garfunkel");
        assert_eq!(normalize("Simon and Garfunkel"), "simon and garfunkel");
        assert_eq!(normalize("Beyonc\u{e9}"), "beyonce");
        assert_eq!(normalize("Mr. Brightside"), "mr brightside");
        assert_eq!(normalize("  Rock-And  Roll! "), "rock and roll");
    }

    #[test]
    fn a_leading_the_is_ignored_but_a_lone_the_is_kept() {
        assert_eq!(normalize("The Killers"), "killers");
        assert_eq!(normalize("The"), "the");
        assert_eq!(normalize("Theory of a Deadman"), "theory of a deadman");
    }

    #[test]
    fn titles_match_with_or_without_their_version_notes() {
        let keys = title_keys("Immigrant Song (Remastered 2014)");
        assert!(keys.contains(&"immigrant song remastered 2014".to_string()));
        assert!(keys.contains(&"immigrant song".to_string()));

        let keys = title_keys("Black Dog - 2007 Remaster");
        assert!(keys.contains(&"black dog".to_string()));

        let keys = title_keys("Whole Lotta Love - Live");
        assert!(keys.contains(&"whole lotta love".to_string()));
    }

    #[test]
    fn a_hyphen_that_belongs_to_the_title_is_kept() {
        let keys = title_keys("Rock-n-Roll - Part 2");
        assert!(keys.contains(&"rock n roll part 2".to_string()));
        assert!(
            !keys.contains(&"rock n roll".to_string()),
            "\"Part 2\" is not a version note: {keys:?}"
        );
    }

    #[test]
    fn a_guest_credit_also_matches_on_the_lead_artist() {
        let keys = artist_keys("The Killers Featuring Toni Halliday");
        assert_eq!(keys, ["killers featuring toni halliday", "killers"]);
        let keys = artist_keys("Beyonc\u{e9} feat. Jay-Z");
        assert!(keys.contains(&"beyonce".to_string()));
    }

    #[test]
    fn an_ampersand_in_a_name_is_not_a_guest() {
        assert_eq!(artist_keys("Simon & Garfunkel"), ["simon and garfunkel"]);
        assert_eq!(
            artist_keys("Crosby, Stills, Nash & Young"),
            ["crosby stills nash and young"]
        );
    }

    #[test]
    fn nothing_makes_an_empty_key() {
        assert_eq!(title_keys("()"), Vec::<String>::new());
        assert_eq!(artist_keys(""), Vec::<String>::new());
    }
}
