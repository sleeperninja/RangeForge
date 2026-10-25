//! File name templates: a pattern such as `{track:02} - {title}` that becomes a file name.
//!
//! A template is plain text with variables in braces. Each variable's value is made safe to
//! use in a file name before it is put in, so a title like `AC/DC: Live?` cannot break a path.
//! When a value is missing, the separator text next to it goes too, so `{track:02} - {title}`
//! gives `Title` for a song with no track number rather than ` - Title`.

use rangeforge_core::identify::Identity;

use crate::naming::{replace_forbidden, sanitize};
use crate::tags::Tags;

/// What a variable in a template stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variable {
    Track,
    Title,
    Artist,
    Album,
    AlbumArtist,
    Year,
    Genre,
    Name,
}

impl Variable {
    pub const ALL: [Self; 8] = [
        Self::Track,
        Self::Title,
        Self::Artist,
        Self::Album,
        Self::AlbumArtist,
        Self::Year,
        Self::Genre,
        Self::Name,
    ];

    /// The word used between the braces.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Title => "title",
            Self::Artist => "artist",
            Self::Album => "album",
            Self::AlbumArtist => "albumartist",
            Self::Year => "year",
            Self::Genre => "genre",
            Self::Name => "name",
        }
    }

    /// What it means, in plain words.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::Track => "track number; {track:02} pads it to two digits",
            Self::Title => "song title",
            Self::Artist => "song artist",
            Self::Album => "album title",
            Self::AlbumArtist => "album artist",
            Self::Year => "year, from the CUE sheet's date",
            Self::Genre => "genre",
            Self::Name => "the original file's name, without its extension",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|variable| variable.key() == key)
    }
}

/// The values a template can use. A value that is `None` counts as missing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NameValues {
    pub track: Option<u32>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub name: Option<String>,
}

impl NameValues {
    /// The values for a song with these tags, from the file called `name` (no extension).
    #[must_use]
    pub fn from_tags(tags: &Tags, name: &str) -> Self {
        Self {
            track: tags.track,
            title: Some(tags.title.clone().unwrap_or_else(|| "untitled".to_string())),
            artist: tags.artist.clone(),
            album: tags.album.clone(),
            album_artist: tags.album_artist.clone(),
            year: tags.date.as_deref().and_then(year_of),
            genre: tags.genre.clone(),
            name: Some(name.to_string()),
        }
    }

    /// The values for a song known by this identity (from its tags, or guessed from its path),
    /// from the file called `name` (no extension). What is not known stays missing.
    #[must_use]
    pub fn from_identity(identity: &Identity, name: &str) -> Self {
        Self {
            track: identity.track,
            title: identity.title.clone(),
            artist: identity.artist.clone(),
            album: identity.album.clone(),
            album_artist: identity.album_artist.clone(),
            year: identity.date.as_deref().and_then(year_of),
            genre: identity.genre.clone(),
            name: Some(name.to_string()),
        }
    }

    fn get(&self, variable: Variable, width: usize) -> Option<String> {
        let text = |value: &Option<String>| value.clone().filter(|v| !v.trim().is_empty());
        match variable {
            Variable::Track => self.track.map(|n| format!("{n:0width$}")),
            Variable::Title => text(&self.title),
            Variable::Artist => text(&self.artist),
            Variable::Album => text(&self.album),
            Variable::AlbumArtist => text(&self.album_artist),
            Variable::Year => text(&self.year),
            Variable::Genre => text(&self.genre),
            Variable::Name => text(&self.name),
        }
    }
}

/// A date can be a whole date; the year is its first four digits.
fn year_of(date: &str) -> Option<String> {
    let digits: String = date.chars().take_while(char::is_ascii_digit).collect();
    (digits.len() >= 4).then(|| digits[..4].to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Text(String),
    Variable { variable: Variable, width: usize },
}

/// A checked file name pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pieces: Vec<Piece>,
}

impl Template {
    /// The usual name of a song cut from a CUE album: `07 - No Quarter`.
    #[must_use]
    pub fn standard_album() -> Self {
        Self {
            pieces: vec![
                Piece::Variable {
                    variable: Variable::Track,
                    width: 2,
                },
                Piece::Text(" - ".to_string()),
                Piece::Variable {
                    variable: Variable::Title,
                    width: 0,
                },
            ],
        }
    }

    /// The usual name of a single file: its own name.
    #[must_use]
    pub fn standard_single() -> Self {
        Self {
            pieces: vec![Piece::Variable {
                variable: Variable::Name,
                width: 0,
            }],
        }
    }

    /// Checks `text` and turns it into a template, or says what is wrong in plain words.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.trim().is_empty() {
            return Err("the name pattern is empty".to_string());
        }
        if text.contains(['/', '\\']) {
            return Err("a file name cannot contain / or \\".to_string());
        }
        let mut pieces = Vec::new();
        let mut literal = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    let mut inner = String::new();
                    loop {
                        match chars.next() {
                            Some('}') => break,
                            Some(c) => inner.push(c),
                            None => return Err("a { is missing its closing }".to_string()),
                        }
                    }
                    if !literal.is_empty() {
                        pieces.push(Piece::Text(std::mem::take(&mut literal)));
                    }
                    pieces.push(parse_variable(&inner)?);
                }
                '}' => return Err("a } has no opening {".to_string()),
                other => literal.push(other),
            }
        }
        if !literal.is_empty() {
            pieces.push(Piece::Text(literal));
        }
        if !pieces
            .iter()
            .any(|piece| matches!(piece, Piece::Variable { .. }))
        {
            return Err(
                "the pattern needs at least one variable, such as {title}, or every file would get the same name"
                    .to_string(),
            );
        }
        Ok(Self { pieces })
    }

    /// The file name without its extension.
    #[must_use]
    pub fn render(&self, values: &NameValues) -> String {
        let resolved: Vec<Option<String>> = self
            .pieces
            .iter()
            .map(|piece| match piece {
                Piece::Text(text) => Some(text.clone()),
                Piece::Variable { variable, width } => values
                    .get(*variable, *width)
                    .map(|value| replace_forbidden(&value)),
            })
            .collect();

        // A missing value takes the separator beside it along, so no stray " - " is left.
        let mut dropped = vec![false; self.pieces.len()];
        for (index, value) in resolved.iter().enumerate() {
            let is_missing_variable =
                matches!(self.pieces[index], Piece::Variable { .. }) && value.is_none();
            if !is_missing_variable {
                continue;
            }
            let before = index
                .checked_sub(1)
                .filter(|&at| matches!(self.pieces[at], Piece::Text(_)) && !dropped[at]);
            let after = (index + 1 < self.pieces.len()
                && matches!(self.pieces[index + 1], Piece::Text(_)))
            .then_some(index + 1);
            if let Some(at) = before.or(after) {
                dropped[at] = true;
            }
        }

        let joined: String = resolved
            .iter()
            .enumerate()
            .filter(|(index, _)| !dropped[*index])
            .filter_map(|(_, value)| value.as_deref())
            .collect();
        let tidy = joined.split_whitespace().collect::<Vec<_>>().join(" ");
        sanitize(&tidy)
    }

    /// The full file name, with `extension` (without a dot).
    #[must_use]
    pub fn file_name(&self, values: &NameValues, extension: &str) -> String {
        format!("{}.{extension}", self.render(values))
    }
}

fn parse_variable(inner: &str) -> Result<Piece, String> {
    let inner = inner.trim();
    let (key, width) = match inner.split_once(':') {
        Some((key, width)) => (key.trim(), Some(width.trim())),
        None => (inner, None),
    };
    let variable = Variable::from_key(key).ok_or_else(|| {
        let known: Vec<String> = Variable::ALL
            .iter()
            .map(|v| format!("{{{}}}", v.key()))
            .collect();
        format!(
            "{{{key}}} is not a known variable. Use {}",
            known.join(", ")
        )
    })?;
    let width = match width {
        None => 0,
        Some(digits) if variable == Variable::Track => digits
            .parse::<usize>()
            .ok()
            .filter(|w| (1..=4).contains(w))
            .ok_or_else(|| {
                "the width after : must be a number from 1 to 4, like {track:02}".to_string()
            })?,
        Some(_) => {
            return Err(format!(
                "only {{track}} takes a width, so {{{}:...}} is not allowed",
                variable.key()
            ));
        }
    };
    Ok(Piece::Variable { variable, width })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song() -> NameValues {
        NameValues {
            track: Some(7),
            title: Some("No Quarter".to_string()),
            artist: Some("Led Zeppelin".to_string()),
            album: Some("Houses of the Holy".to_string()),
            album_artist: Some("Led Zeppelin".to_string()),
            year: Some("1973".to_string()),
            genre: Some("Rock".to_string()),
            name: Some("Houses of the Holy".to_string()),
        }
    }

    fn album(text: &str) -> Template {
        Template::parse(text).unwrap()
    }

    #[test]
    fn the_standard_templates_are_what_they_say() {
        assert_eq!(Template::standard_album(), album("{track:02} - {title}"));
        assert_eq!(Template::standard_single(), album("{name}"));
        assert_eq!(
            Template::standard_album().file_name(&song(), "m4a"),
            "07 - No Quarter.m4a"
        );
    }

    #[test]
    fn variables_and_text_come_through() {
        let template = album("{albumartist} - {album} ({year}) - {track:02} {title}");
        assert_eq!(
            template.render(&song()),
            "Led Zeppelin - Houses of the Holy (1973) - 07 No Quarter"
        );
        assert_eq!(album("{track}").render(&song()), "7");
        assert_eq!(album("{track:03}").render(&song()), "007");
        assert_eq!(
            album("{genre}-{artist}").render(&song()),
            "Rock-Led Zeppelin"
        );
    }

    #[test]
    fn values_are_made_safe_but_the_pattern_is_not_touched() {
        let values = NameValues {
            title: Some("AC/DC: Live?".to_string()),
            ..song()
        };
        assert_eq!(album("{title}").render(&values), "AC_DC_ Live_");
        // Spaces in the pattern are kept; runs of spaces in a value collapse.
        let values = NameValues {
            title: Some("A   B".to_string()),
            ..song()
        };
        assert_eq!(album("{title}").render(&values), "A B");
    }

    #[test]
    fn a_missing_value_takes_its_separator_with_it() {
        let template = album("{track:02} - {title}");
        let no_track = NameValues {
            track: None,
            ..song()
        };
        assert_eq!(template.render(&no_track), "No Quarter");
        let no_title = NameValues {
            title: None,
            ..song()
        };
        assert_eq!(template.render(&no_title), "07");

        let template = album("{artist} - {album} - {title}");
        let no_album = NameValues {
            album: None,
            ..song()
        };
        assert_eq!(template.render(&no_album), "Led Zeppelin - No Quarter");

        // Two missing values in a row still leave something tidy.
        let template = album("{year} - {genre} - {title}");
        let neither = NameValues {
            year: None,
            genre: None,
            ..song()
        };
        assert_eq!(template.render(&neither), "No Quarter");

        // A title that begins with dots is a title, not a separator.
        let values = NameValues {
            title: Some("...And Justice".to_string()),
            ..song()
        };
        assert!(album("{title}").render(&values).contains("And Justice"));
    }

    #[test]
    fn nothing_at_all_still_gives_a_name() {
        assert_eq!(album("{title}").render(&NameValues::default()), "untitled");
    }

    #[test]
    fn a_year_is_the_first_four_digits_of_the_date() {
        let tags = |date: &str| Tags {
            date: Some(date.to_string()),
            ..Tags::default()
        };
        assert_eq!(
            NameValues::from_tags(&tags("1973-03-28"), "x")
                .year
                .as_deref(),
            Some("1973")
        );
        assert_eq!(NameValues::from_tags(&tags("73"), "x").year, None);
        assert_eq!(NameValues::from_tags(&Tags::default(), "x").year, None);
        // A song with no title is called untitled, as before.
        assert_eq!(
            NameValues::from_tags(&Tags::default(), "x")
                .title
                .as_deref(),
            Some("untitled")
        );
    }

    #[test]
    fn mistakes_are_explained() {
        let error = |text: &str| Template::parse(text).unwrap_err();
        assert!(error("").contains("empty"));
        assert!(error("  ").contains("empty"));
        assert!(error("{nope}").contains("not a known variable"));
        assert!(error("{nope}").contains("{title}"));
        assert!(error("{title").contains("closing"));
        assert!(error("title}").contains("opening"));
        assert!(error("just text").contains("at least one variable"));
        assert!(error("{artist}/{title}").contains('/'));
        assert!(error("{track:x}").contains("1 to 4"));
        assert!(error("{track:9}").contains("1 to 4"));
        assert!(error("{title:02}").contains("only {track}"));
    }

    #[test]
    fn a_song_known_by_its_identity_is_named_from_it() {
        use rangeforge_core::identify::{Identity, IdentitySource};
        let identity = Identity {
            artist: Some("Led Zeppelin".to_string()),
            title: Some("No Quarter".to_string()),
            album: Some("Houses of the Holy".to_string()),
            track: Some(7),
            date: Some("1973-03-28".to_string()),
            ..Identity::unknown(IdentitySource::FileTags)
        };
        let values = NameValues::from_identity(&identity, "file name");
        assert_eq!(
            album("{track:02} {artist} - {title} ({year})").render(&values),
            "07 Led Zeppelin - No Quarter (1973)"
        );
        assert_eq!(album("{name}").render(&values), "file name");

        // What the identity does not know is simply left out.
        let guess = Identity {
            title: Some("song".to_string()),
            ..Identity::unknown(IdentitySource::FileName)
        };
        let values = NameValues::from_identity(&guess, "song");
        assert_eq!(album("{artist} - {title}").render(&values), "song");
    }

    #[test]
    fn whitespace_inside_braces_is_forgiven() {
        assert_eq!(
            album("{ track : 02 } - { title }").render(&song()),
            "07 - No Quarter"
        );
    }

    #[test]
    fn every_variable_is_described() {
        for variable in Variable::ALL {
            assert_ne!(variable.describe(), "");
            assert_eq!(Variable::from_key(variable.key()), Some(variable));
        }
    }
}
