//! The File Strategy: what to do when an output file would land on a name that is taken.
//!
//! The four choices match the mockup. None of them ever deletes a file. The worst case,
//! [`Strategy::Replace`], overwrites a file at exactly the output path and nothing else.

use std::path::{Path, PathBuf};

use crate::ExportError;

/// How many serial numbers to try before giving up.
const MAX_TRIES: u64 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Strategy {
    /// The new file always gets a serial suffix such as `_00`, counting up if that is taken.
    /// Nothing existing is touched.
    Serialize { suffix: String },
    /// If a file is already at the output path, it is renamed with the suffix (such as
    /// `_old`) and the new file takes its name. A file already named with the suffix is never
    /// overwritten.
    RenameOld { suffix: String },
    /// A file at exactly the output path is overwritten. A source with a different name, such
    /// as a FLAC when making ALAC, is left alone.
    Replace,
    /// Write into this folder. If a name is taken there, the new file gets a serial suffix.
    SaveTo { directory: PathBuf },
}

impl Strategy {
    /// The mockup's default serial suffix.
    pub const DEFAULT_SERIAL: &'static str = "_00";
    /// The mockup's default suffix for renamed originals.
    pub const DEFAULT_OLD: &'static str = "_old";

    /// The folder files go in, given the folder they would go in by default.
    pub fn directory(&self, default: &Path) -> PathBuf {
        match self {
            Self::SaveTo { directory } => directory.clone(),
            _ => default.to_path_buf(),
        }
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
    default_directory: &Path,
    file_name: &str,
) -> Result<Resolved, ExportError> {
    let wanted = strategy.directory(default_directory).join(file_name);
    match strategy {
        Strategy::Serialize { suffix } => Ok(Resolved::plain(serialized(&wanted, suffix)?)),
        Strategy::RenameOld { suffix } => {
            if !wanted.exists() {
                return Ok(Resolved::plain(wanted));
            }
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
        Strategy::Replace => Ok(Resolved {
            overwrites: wanted.exists(),
            final_path: wanted,
            move_existing_to: None,
        }),
        Strategy::SaveTo { .. } => {
            if wanted.exists() {
                Ok(Resolved::plain(serialized(
                    &wanted,
                    Strategy::DEFAULT_SERIAL,
                )?))
            } else {
                Ok(Resolved::plain(wanted))
            }
        }
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

/// The first free name of the form `name<prefix><number>.ext`. The suffix must end in digits;
/// those digits set where counting starts and how many places to pad to, so `_00` gives
/// `_00`, `_01`, `_02`, and so on.
fn serialized(path: &Path, suffix: &str) -> Result<PathBuf, ExportError> {
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

    fn serialize() -> Strategy {
        Strategy::Serialize {
            suffix: "_00".to_string(),
        }
    }

    fn rename_old() -> Strategy {
        Strategy::RenameOld {
            suffix: "_old".to_string(),
        }
    }

    #[test]
    fn serialize_always_adds_the_suffix() {
        let dir = tempdir().unwrap();
        let resolved = resolve(&serialize(), dir.path(), "song.m4a").unwrap();
        assert_eq!(resolved.final_path, dir.path().join("song_00.m4a"));
        assert_eq!(resolved.move_existing_to, None);
        assert!(!resolved.overwrites);
    }

    #[test]
    fn serialize_counts_up_past_taken_names() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "song_00.m4a");
        touch(dir.path(), "song_01.m4a");
        // The original is not in the way either; only the serial names matter.
        touch(dir.path(), "song.m4a");
        let resolved = resolve(&serialize(), dir.path(), "song.m4a").unwrap();
        assert_eq!(resolved.final_path, dir.path().join("song_02.m4a"));
    }

    #[test]
    fn serial_suffixes_set_the_start_and_the_padding() {
        let dir = tempdir().unwrap();
        let strategy = Strategy::Serialize {
            suffix: " v001".to_string(),
        };
        let resolved = resolve(&strategy, dir.path(), "a.flac").unwrap();
        assert_eq!(resolved.final_path, dir.path().join("a v001.flac"));

        let strategy = Strategy::Serialize {
            suffix: "_7".to_string(),
        };
        let resolved = resolve(&strategy, dir.path(), "a.flac").unwrap();
        assert_eq!(resolved.final_path, dir.path().join("a_7.flac"));
    }

    #[test]
    fn a_suffix_without_digits_is_refused() {
        let dir = tempdir().unwrap();
        let strategy = Strategy::Serialize {
            suffix: "_new".to_string(),
        };
        let error = resolve(&strategy, dir.path(), "a.flac").unwrap_err();
        assert!(matches!(error, ExportError::InvalidSuffix(_)), "{error}");
    }

    #[test]
    fn names_with_dots_keep_their_extension() {
        let dir = tempdir().unwrap();
        let resolved = resolve(&serialize(), dir.path(), "Vol.2 Live.m4a").unwrap();
        assert_eq!(resolved.final_path, dir.path().join("Vol.2 Live_00.m4a"));
    }

    #[test]
    fn rename_old_moves_an_existing_file_aside() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "song.m4a");
        let resolved = resolve(&rename_old(), dir.path(), "song.m4a").unwrap();
        assert_eq!(resolved.final_path, dir.path().join("song.m4a"));
        assert_eq!(
            resolved.move_existing_to,
            Some(dir.path().join("song_old.m4a"))
        );
    }

    #[test]
    fn rename_old_without_a_clash_just_writes() {
        let dir = tempdir().unwrap();
        let resolved = resolve(&rename_old(), dir.path(), "song.m4a").unwrap();
        assert_eq!(resolved, Resolved::plain(dir.path().join("song.m4a")));
    }

    #[test]
    fn rename_old_never_overwrites_an_earlier_backup() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "song.m4a");
        touch(dir.path(), "song_old.m4a");
        let error = resolve(&rename_old(), dir.path(), "song.m4a").unwrap_err();
        assert!(
            matches!(error, ExportError::DestinationExists(_)),
            "{error}"
        );
    }

    #[test]
    fn replace_overwrites_only_the_exact_path() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "song.flac");
        // A differently named source is not a clash, so nothing is overwritten.
        let resolved = resolve(&Strategy::Replace, dir.path(), "song.m4a").unwrap();
        assert!(!resolved.overwrites);

        touch(dir.path(), "song.m4a");
        let resolved = resolve(&Strategy::Replace, dir.path(), "song.m4a").unwrap();
        assert!(resolved.overwrites);
        assert_eq!(resolved.final_path, dir.path().join("song.m4a"));
    }

    #[test]
    fn save_to_uses_the_other_folder_and_serializes_clashes() {
        let source = tempdir().unwrap();
        let target = tempdir().unwrap();
        touch(source.path(), "song.m4a");
        let strategy = Strategy::SaveTo {
            directory: target.path().to_path_buf(),
        };

        let resolved = resolve(&strategy, source.path(), "song.m4a").unwrap();
        assert_eq!(resolved.final_path, target.path().join("song.m4a"));

        touch(target.path(), "song.m4a");
        let resolved = resolve(&strategy, source.path(), "song.m4a").unwrap();
        assert_eq!(resolved.final_path, target.path().join("song_00.m4a"));
    }

    #[test]
    fn resolving_changes_nothing_on_disk() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "song.m4a");
        for strategy in [serialize(), rename_old(), Strategy::Replace] {
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
