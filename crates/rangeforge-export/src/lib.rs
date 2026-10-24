//! Writing finished files for `RangeForge`.
//!
//! [`pipeline::export_file`] decodes a source file once, cuts out each track, applies that
//! track's gain, and encodes it through FFmpeg. Files are encoded to temporary names first and
//! only moved into place, by the chosen [`strategy::Strategy`], once everything has succeeded,
//! so a failure never leaves an original changed.

pub mod encode;
mod error;
pub mod ffmpeg;
pub mod format;
pub mod naming;
pub mod pipeline;
pub mod strategy;
pub mod tags;

pub use error::ExportError;
