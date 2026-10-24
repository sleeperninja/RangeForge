//! Finding the FFmpeg program.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::ExportError;

/// A usable FFmpeg.
#[derive(Debug, Clone)]
pub struct Ffmpeg {
    path: PathBuf,
}

impl Ffmpeg {
    /// The environment variable that names FFmpeg's full path.
    pub const ENV: &'static str = "RANGEFORGE_FFMPEG";

    /// Looks for FFmpeg in the `RANGEFORGE_FFMPEG` environment variable first, then on the
    /// system path, and checks that it runs.
    pub fn locate() -> Result<Self, ExportError> {
        let candidate =
            std::env::var_os(Self::ENV).map_or_else(|| PathBuf::from("ffmpeg"), PathBuf::from);
        Self::at(candidate)
    }

    /// Uses the FFmpeg at `path`, after checking that it runs.
    pub fn at(path: impl Into<PathBuf>) -> Result<Self, ExportError> {
        let path = path.into();
        let works = Command::new(&path)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if works {
            Ok(Self { path })
        } else {
            Err(ExportError::FfmpegNotFound { tried: path })
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
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
