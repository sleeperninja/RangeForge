//! The File Strategy: where finished files go, what happens when a name is already taken, and
//! whether the original is removed afterwards.
//!
//! These are three separate choices. A name is only ever changed when it would clash with a
//! file that is already there. The worst a strategy can do is overwrite a file at exactly the
//! output path, or, if asked, delete the original after every song from it has been written.

use std::path::{Path, PathBuf};

use crate::ExportError;

/// How many serial numbers to try before giving up.
const MAX_TRIES: u64 = 10_000;

/// Where the finished files go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// In the same folder as the original.
    NextToOriginal,
    /// In a folder of this name inside the original's folder.
    Subfolder(String),
    /// Every file in this one folder, whatever it was made from.
    Folder(PathBuf),
}

/// What to do when a file is already at the output path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conflict {
    /// The new file gets a serial suffix such as `_00`, counting up until the name is free.
    Number { suffix: String },
    /// The existing file is renamed with the suffix (such as `_old`) and the new file takes its
    /// name. A file already named with the suffix is never overwritten.
    RenameExisting { suffix: String },
    /// The existing file is overwritten.
    Overwrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Strategy {
    pub destination: Destination,
    pub conflict: Conflict,
    /// Delete the original once every song from it has been written. Never done when a song
    /// was left out, or when the original is one of the files just written.
    pub delete_original: bool,
}

impl Strategy {
    /// The default serial suffix.
    pub const DEFAULT_NUMBER: &'static str = "_00";
    /// The default suffix for renamed files.
    pub const DEFAULT_OLD: &'static str = "_old";
    /// The default name of the subfolder.
    pub const DEFAULT_SUBFOLDER: &'static str = "_forged";

    /// Next to the original, numbering the new file if its name is taken. This is the safe
    /// starting choice: nothing existing is touched.
    #[must_use]
    pub fn safe() -> Self {
        Self {
            destination: Destination::NextToOriginal,
            conflict: Conflict::Number {
                suffix: Self::DEFAULT_NUMBER.to_string(),
            },
            delete_original: false,
        }
    }

    /// The folder files go in, given the folder the original is in.
    #[must_use]
    pub fn directory(&self, original_folder: &Path) -> PathBuf {
        match &self.destination {
            Destination::NextToOriginal => original_folder.to_path_buf(),
            Destination::Subfolder(name) => original_folder.join(name.trim()),
            Destination::Folder(folder) => folder.clone(),
        }
    }

    /// True if running could overwrite or delete something, so the user should be asked first.
    #[must_use]
    pub fn needs_confirmation(&self) -> bool {
        self.delete_original || self.conflict == Conflict::Overwrite
    }

    /// Checks that the strategy can be carried out, before any work is done.
    pub fn validate(&self) -> Result<(), ExportError> {
        if let Destination::Subfolder(name) = &self.destination {
            check_subfolder(name)?;
        }
        if let Conflict::Number { suffix } = &self.conflict {
            serial_parts(suffix)?;
        }
        Ok(())
    }
}

/// A subfolder name must be a single, ordinary folder name.
fn check_subfolder(name: &str) -> Result<(), ExportError> {
    let name = name.trim();
    let bad = name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']);
    if bad {
        Err(ExportError::InvalidSubfolder(name.to_string()))
    } else {
        Ok(())
    }
}

/// Where one output file will end up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub final_path: PathBuf,
    /// An existing file to move out of the way first, and where to.
    pub move_existing_to: Option<PathBuf>,
    /// True if an existing file at `final_path` will be overwritten.
    pub overwrites: bool,
}

impl Resolved {
    fn plain(final_path: PathBuf) -> Self {
        Self {
            final_path,
            move_existing_to: None,
            overwrites: false,
        }
    }
}

/// Works out where `file_name` goes under `strategy`. It looks at the folder but changes
/// nothing, so it can be used to preview a run.
pub fn resolve(
    strategy: &Strategy,
    original_folder: &Path,
    file_name: &str,
) -> Result<Resolved, ExportError> {
    strategy.validate()?;
    let wanted = strategy.directory(original_folder).join(file_name);
    if !wanted.exists() {
        return Ok(Resolved::plain(wanted));
    }
    match &strategy.conflict {
        Conflict::Number { suffix } => Ok(Resolved::plain(serialized(&wanted, suffix)?)),
        Conflict::RenameExisting { suffix } => {
            let old = with_suffix(&wanted, suffix);
            if old.exists() {
                return Err(ExportError::DestinationExists(old));
            }
            Ok(Resolved {
                final_path: wanted,
                move_existing_to: Some(old),
                overwrites: false,
            })
        }
        Conflict::Overwrite => Ok(Resolved {
            final_path: wanted,
            move_existing_to: None,
            overwrites: true,
        }),
    }
}

/// `dir/name.ext` becomes `dir/name<suffix>.ext`.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    named(path, |stem| format!("{stem}{suffix}"))
}

fn named(path: &Path, rename_stem: impl FnOnce(&str) -> String) -> PathBuf {
    let stem = path
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let mut name = rename_stem(&stem);
    if let Some(extension) = path.extension() {
        name.push('.');
        name.push_str(&extension.to_string_lossy());
    }
    path.with_file_name(name)
}

/// Splits a serial suffix into the text before its number, where counting starts, and how many
/// places to pad to. The suffix must end in digits.
fn serial_parts(suffix: &str) -> Result<(&str, u64, usize), ExportError> {
    let digits = suffix
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .count();
    if digits == 0 {
        return Err(ExportError::InvalidSuffix(suffix.to_string()));
    }
    let (prefix, number) = suffix.split_at(suffix.len() - digits);
    let start: u64 = number
        .parse()
        .map_err(|_| ExportError::InvalidSuffix(suffix.to_string()))?;
    Ok((prefix, start, digits))
}

/// The first free name of the form `name<prefix><number>.ext`. The digits of the suffix set
/// where counting starts and how many places to pad to, so `_00` gives `_00`, `_01`, `_02`.
fn serialized(path: &Path, suffix: &str) -> Result<PathBuf, ExportError> {
    let (prefix, start, digits) = serial_parts(suffix)?;
    for n in start..start.saturating_add(MAX_TRIES) {
        let candidate = named(path, |stem| format!("{stem}{prefix}{n:0digits$}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(ExportError::NoFreeName(path.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn touch(dir: &Path, name: &str) {
        fs::write(dir.join(name), b"x").unwrap();
    }

    fn number(suffix: &str) -> Strategy {
        Strategy {
            conflict: Conflict::Number {
                suffix: suffix.to_string(),
            },
            ..Strategy::safe()
        }
    }

    fn rename_existing() -> Strategy {
        Strategy {
            conflict: Conflict::RenameExisting {
                suffix: "_old".to_string(),
            },
            ..Strategy::safe()
        }
    }

    fn overwrite() -> Strategy {
        Strategy {
            conflict: Conflict::Overwrite,
            ..Strategy::safe()
        }
    }

    #[test]
    fn a_free_name_is_used_as_it_is() {
        let dir = tempdir().unwrap();
        for strategy in [number("_00"), rename_existing(), overwrite()] {
            let resolved = resolve(&strategy, dir.path(), "song.m4a").unwrap();
            assert_eq!(resolved, Resolved::plain(dir.path().join("song.m4a")));
        }
    }

    #[test]
    fn numbering_starts_only_when_the_name_is_taken() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "song.m4a");
        let resolved = resolve(&number("_00"), dir.path(), "song.m4a").unwrap();
        assert_eq!(resolved.final_path, dir.path().join("song_00.m4a"));
        assert_eq!(resolved.move_existing_to, None);
        assert!(!resolved.overwrites);
    }

    #[test]
    fn numbering_counts_up_past_taken_names() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "song.m4a");
        touch(dir.path(), "song_00.m4a");
        touch(dir.path(), "song_01.m4a");
        let resolved = resolve(&number("_00"), dir.path(), "song.m4a").unwrap();
        assert_eq!(resolved.final_path, dir.path().join("song_02.m4a"));
    }

    #[test]
    fn serial_suffixes_set_the_start_and_the_padding() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "a.flac");
        let resolved = resolve(&number(" v001"), dir.path(), "a.flac").unwrap();
        assert_eq!(resolved.final_path, dir.path().join("a v001.flac"));

        let resolved = resolve(&number("_7"), dir.path(), "a.flac").unwrap();
        assert_eq!(resolved.final_path, dir.path().join("a_7.flac"));
    }

    #[test]
    fn a_suffix_without_digits_is_refused() {
        let dir = tempdir().unwrap();
        let error = resolve(&number("_new"), dir.path(), "a.flac").unwrap_err();
        assert!(matches!(error, ExportError::InvalidSuffix(_)), "{error}");
    }

    #[test]
    fn names_with_dots_keep_their_extension() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "Vol.2 Live.m4a");
        let resolved = resolve(&number("_00"), dir.path(), "Vol.2 Live.m4a").unwrap();
        assert_eq!(resolved.final_path, dir.path().join("Vol.2 Live_00.m4a"));
    }

    #[test]
    fn rename_existing_moves_the_file_in_the_way_aside() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "song.m4a");
        let resolved = resolve(&rename_existing(), dir.path(), "song.m4a").unwrap();
        assert_eq!(resolved.final_path, dir.path().join("song.m4a"));
        assert_eq!(
            resolved.move_existing_to,
            Some(dir.path().join("song_old.m4a"))
        );
    }

    #[test]
    fn rename_existing_never_overwrites_an_earlier_backup() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "song.m4a");
        touch(dir.path(), "song_old.m4a");
        let error = resolve(&rename_existing(), dir.path(), "song.m4a").unwrap_err();
        assert!(
            matches!(error, ExportError::DestinationExists(_)),
            "{error}"
        );
    }

    #[test]
    fn overwrite_replaces_only_the_exact_path() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "song.flac");
        // A differently named source is not a clash, so nothing is overwritten.
        let resolved = resolve(&overwrite(), dir.path(), "song.m4a").unwrap();
        assert!(!resolved.overwrites);

        touch(dir.path(), "song.m4a");
        let resolved = resolve(&overwrite(), dir.path(), "song.m4a").unwrap();
        assert!(resolved.overwrites);
        assert_eq!(resolved.final_path, dir.path().join("song.m4a"));
    }

    #[test]
    fn the_destination_decides_the_folder() {
        let original = Path::new("music/album");
        let strategy = Strategy::safe();
        assert_eq!(strategy.directory(original), original);

        let sub = Strategy {
            destination: Destination::Subfolder(" Balanced ".to_string()),
            ..Strategy::safe()
        };
        assert_eq!(sub.directory(original), original.join("Balanced"));

        let folder = Strategy {
            destination: Destination::Folder(PathBuf::from("D:/out")),
            ..Strategy::safe()
        };
        assert_eq!(folder.directory(original), PathBuf::from("D:/out"));
    }

    #[test]
    fn a_folder_and_a_subfolder_number_clashes_there() {
        let source = tempdir().unwrap();
        let target = tempdir().unwrap();
        let strategy = Strategy {
            destination: Destination::Folder(target.path().to_path_buf()),
            ..Strategy::safe()
        };

        let resolved = resolve(&strategy, source.path(), "song.m4a").unwrap();
        assert_eq!(resolved.final_path, target.path().join("song.m4a"));

        touch(target.path(), "song.m4a");
        let resolved = resolve(&strategy, source.path(), "song.m4a").unwrap();
        assert_eq!(resolved.final_path, target.path().join("song_00.m4a"));
    }

    #[test]
    fn a_subfolder_must_be_one_ordinary_folder_name() {
        let dir = tempdir().unwrap();
        for bad in ["", "  ", "..", ".", "a/b", "a\\b", "C:", "what?"] {
            let strategy = Strategy {
                destination: Destination::Subfolder(bad.to_string()),
                ..Strategy::safe()
            };
            let error = resolve(&strategy, dir.path(), "a.m4a").unwrap_err();
            assert!(matches!(error, ExportError::InvalidSubfolder(_)), "{bad:?}");
        }
    }

    #[test]
    fn only_overwriting_or_deleting_asks_for_confirmation() {
        assert!(!Strategy::safe().needs_confirmation());
        assert!(!rename_existing().needs_confirmation());
        assert!(overwrite().needs_confirmation());
        let delete = Strategy {
            delete_original: true,
            ..Strategy::safe()
        };
        assert!(delete.needs_confirmation());
    }

    #[test]
    fn resolving_changes_nothing_on_disk() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "song.m4a");
        for strategy in [number("_00"), rename_existing(), overwrite()] {
            resolve(&strategy, dir.path(), "song.m4a").unwrap();
        }
        let mut names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["song.m4a"]);
    }
}
