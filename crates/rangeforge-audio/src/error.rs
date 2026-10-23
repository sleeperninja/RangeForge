use std::path::PathBuf;

use thiserror::Error;

/// Why an audio file could not be read or measured.
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("cannot open {}: {source}", path.display())]
    Open {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot read the audio: {0}")]
    Decode(#[from] symphonia::core::errors::Error),
    #[error("loudness meter error: {0}")]
    Meter(#[from] ebur128::Error),
    #[error("the file has no audio to measure")]
    NoAudioStream,
    #[error("the audio ended before {missing} track(s) were finished")]
    EndedEarly { missing: usize },
}
