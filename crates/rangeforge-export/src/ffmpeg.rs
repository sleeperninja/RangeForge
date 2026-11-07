//! Finding the FFmpeg program.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::ExportError;

#[cfg(windows)]
const PROGRAM: &str = "ffmpeg.exe";
#[cfg(not(windows))]
const PROGRAM: &str = "ffmpeg";

/// Where package managers put FFmpeg when it is not on the path a window gets.
const COMMON_FOLDERS: [&str; 4] = [
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/snap/bin",
];

/// Stops Windows opening a console window for each program that is started.
#[cfg(windows)]
fn quiet(mut command: Command) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(windows))]
fn quiet(command: Command) -> Command {
    command
}

/// A usable FFmpeg.
#[derive(Debug, Clone)]
pub struct Ffmpeg {
    path: PathBuf,
}

impl Ffmpeg {
    /// The environment variable that names FFmpeg's full path.
    pub const ENV: &'static str = "RANGEFORGE_FFMPEG";

    /// Looks for FFmpeg in the `RANGEFORGE_FFMPEG` environment variable first, then next to
    /// this program, then on the system path, then in the places package managers put it.
    ///
    /// The last two matter for a window opened from Finder or the Start menu, which does not
    /// get the path a terminal has (Homebrew's folder is missing from it).
    pub fn locate() -> Result<Self, ExportError> {
        if let Some(named) = std::env::var_os(Self::ENV) {
            return Self::at(PathBuf::from(named));
        }
        let mut candidates = Vec::new();
        if let Some(folder) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
        {
            candidates.push(folder.join(PROGRAM));
        }
        candidates.push(PathBuf::from("ffmpeg"));
        candidates.extend(
            COMMON_FOLDERS
                .iter()
                .map(|folder| Path::new(folder).join(PROGRAM)),
        );
        for candidate in candidates {
            if let Ok(found) = Self::at(candidate) {
                return Ok(found);
            }
        }
        Err(ExportError::FfmpegNotFound {
            tried: PathBuf::from("ffmpeg"),
        })
    }

    /// A command that runs this FFmpeg, without a console window flashing up on Windows.
    #[must_use]
    pub fn command(&self) -> Command {
        quiet(Command::new(&self.path))
    }

    /// Uses the FFmpeg at `path`, after checking that it runs.
    pub fn at(path: impl Into<PathBuf>) -> Result<Self, ExportError> {
        let path = path.into();
        let works = quiet(Command::new(&path))
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if works {
            let found = Self { path };
            found.offer_to_audio();
            Ok(found)
        } else {
            Err(ExportError::FfmpegNotFound { tried: path })
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Lets the audio reader use this FFmpeg to open DSD files. Every FFmpeg that is found is
    /// offered, so measuring DSD works as soon as FFmpeg is known, without passing it along.
    pub fn offer_to_audio(&self) {
        let path = self.path.clone();
        rangeforge_audio::dsd::use_ffmpeg(move || quiet(Command::new(&path)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_program_is_reported_with_where_it_looked() {
        let error = Ffmpeg::at("definitely-not-a-real-ffmpeg-binary").unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("definitely-not-a-real-ffmpeg-binary"),
            "{message}"
        );
        assert!(message.contains("RANGEFORGE_FFMPEG"), "{message}");
    }
}
