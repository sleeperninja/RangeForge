//! Audio input for `RangeForge`: decode a file once and measure loudness per track.
//!
//! The file is read in a single sequential pass, so it is safe on a network drive. A separate
//! loudness meter runs for each CUE track and is fed the exact sample range of that track.
//! The [`cache`] module remembers results so a file is only decoded once, ever.

pub mod cache;
mod error;
pub mod measure;

pub use error::AudioError;
