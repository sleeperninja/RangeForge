//! Audio input for `RangeForge`: decode a file once and measure loudness per track.
//!
//! The file is read in a single sequential pass, so it is safe on a network drive. A separate
//! loudness meter runs for each CUE track and is fed the exact sample range of that track.
//! The [`cache`] module remembers results so a file is only decoded once, ever.
//!
//! The pieces: [`decode`] streams a file as blocks of audio, [`span`] cuts that stream into
//! tracks, [`measure`] measures each track, [`cache`] remembers the results, [`tags`] reads
//! the tags stored in a file, and [`dsd`] turns DSD files into ordinary audio through FFmpeg.

pub mod cache;
pub mod decode;
pub mod dsd;
mod error;
pub mod measure;
pub mod span;
pub mod tags;

pub use error::AudioError;
