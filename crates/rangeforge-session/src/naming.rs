//! The file name patterns: what the user typed, the checked templates made from them, and an
//! example of each so the screen can show what a pattern will produce.

use rangeforge_export::template::{NameValues, Scope, Template, Variable};

/// The usual name of a song cut from a CUE album.
pub const DEFAULT_ALBUM: &str = "{track:02} - {title}";
/// The usual name of a single file: its own name.
pub const DEFAULT_SINGLE: &str = "{name}";

/// The two patterns as the user wrote them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameSettings {
    /// For songs cut from a CUE album.
    pub album: String,
    /// For single files.
    pub single: String,
}

impl Default for NameSettings {
    fn default() -> Self {
        Self {
            album: DEFAULT_ALBUM.to_string(),
            single: DEFAULT_SINGLE.to_string(),
        }
    }
}

/// The two patterns, checked and ready to use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameTemplates {
    pub album: Template,
    pub single: Template,
}

impl Default for NameTemplates {
    fn default() -> Self {
        Self {
            album: Template::standard_album(),
            single: Template::standard_single(),
        }
    }
}

impl NameSettings {
    /// Checks both patterns, naming the one that is wrong.
    pub fn templates(&self) -> Result<NameTemplates, String> {
        Ok(NameTemplates {
            album: Template::parse(&self.album, Scope::Album)
                .map_err(|problem| format!("CUE album names: {problem}"))?,
            single: Template::parse(&self.single, Scope::Single)
                .map_err(|problem| format!("single file names: {problem}"))?,
        })
    }
}

/// A made-up song, used to show what a pattern produces.
fn sample_album_song() -> NameValues {
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

fn sample_single() -> NameValues {
    NameValues {
        name: Some("14 - Shot at the Night".to_string()),
        ..NameValues::default()
    }
}

/// What the Settings window shows while the user types: an example of each pattern, or what is
/// wrong with it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NamingPreview {
    pub album_example: Option<String>,
    pub album_problem: Option<String>,
    pub single_example: Option<String>,
    pub single_problem: Option<String>,
}

/// Works out the examples for patterns that may not be valid yet, using `extension`.
#[must_use]
pub fn preview(album: &str, single: &str, extension: &str) -> NamingPreview {
    let album = Template::parse(album, Scope::Album);
    let single = Template::parse(single, Scope::Single);
    NamingPreview {
        album_example: album
            .as_ref()
            .ok()
            .map(|t| t.file_name(&sample_album_song(), extension)),
        album_problem: album.err(),
        single_example: single
            .as_ref()
            .ok()
            .map(|t| t.file_name(&sample_single(), extension)),
        single_problem: single.err(),
    }
}

/// The variables a pattern can use, with what each means, for the Settings window to list.
#[must_use]
pub fn variables() -> Vec<(String, String, bool)> {
    Variable::ALL
        .into_iter()
        .map(|variable| {
            (
                format!("{{{}}}", variable.key()),
                variable.describe().to_string(),
                Scope::Single.allows(variable),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_standard_templates() {
        let templates = NameSettings::default().templates().unwrap();
        assert_eq!(templates, NameTemplates::default());
    }

    #[test]
    fn a_bad_pattern_names_which_one_it_is() {
        let settings = NameSettings {
            album: "{nope}".to_string(),
            ..NameSettings::default()
        };
        assert!(
            settings
                .templates()
                .unwrap_err()
                .starts_with("CUE album names:")
        );

        let settings = NameSettings {
            single: "{artist}".to_string(),
            ..NameSettings::default()
        };
        assert!(
            settings
                .templates()
                .unwrap_err()
                .starts_with("single file names:")
        );
    }

    #[test]
    fn the_preview_shows_an_example_or_a_problem() {
        let shown = preview("{artist} - {title}", "{name}", "m4a");
        assert_eq!(
            shown.album_example.as_deref(),
            Some("Led Zeppelin - No Quarter.m4a")
        );
        assert_eq!(
            shown.single_example.as_deref(),
            Some("14 - Shot at the Night.m4a")
        );
        assert_eq!(shown.album_problem, None);

        let broken = preview("{oops}", "{title}", "flac");
        assert_eq!(broken.album_example, None);
        assert!(broken.album_problem.is_some());
        assert!(broken.single_problem.unwrap().contains("single files"));
    }

    #[test]
    fn the_variable_list_says_which_apply_to_single_files() {
        let list = variables();
        assert_eq!(list.len(), 8);
        let name = list.iter().find(|(key, ..)| key == "{name}").unwrap();
        assert!(name.2);
        let title = list.iter().find(|(key, ..)| key == "{title}").unwrap();
        assert!(!title.2);
    }
}
