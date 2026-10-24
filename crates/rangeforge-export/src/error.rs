use std::path::PathBuf;

use rangeforge_audio::AudioError;
use thiserror::Error;

/// Why an export failed. Whatever the reason, the original files are never changed until
/// every track has been encoded.
#[derive(Debug, Error)]
pub enum ExportError {
    #[error(transparent)]
    Audio(#[from] AudioError),
    #[error(
        "FFmpeg was not found (tried {}). Install it, or set RANGEFORGE_FFMPEG to its full path",
        tried.display()
    )]
    FfmpegNotFound { tried: PathBuf },
    #[error("FFmpeg failed on track {track}: {message}")]
    Encoder { track: usize, message: String },
    #[error("{action} {}: {source}", path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the suffix {0:?} must end in digits, such as _00, so it can count up")]
    InvalidSuffix(String),
    #[error(
        "{0:?} cannot be used as a folder name: use one plain name, with no slashes or : * ? \" < > |"
    )]
    InvalidSubfolder(String),
    #[error("could not find a free name for {} after many tries", .0.display())]
    NoFreeName(PathBuf),
    #[error("{} already exists, so it was not overwritten", .0.display())]
    DestinationExists(PathBuf),
    #[error("{expected} track(s) were planned but the file layout has {found}")]
    TrackCountMismatch { expected: usize, found: usize },
}

impl ExportError {
    pub(crate) fn io(
        action: &'static str,
        path: impl Into<PathBuf>,
    ) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io {
            action,
            path,
            source,
        }
    }
}
