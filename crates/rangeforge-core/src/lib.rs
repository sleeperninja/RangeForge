//! `RangeForge` core.
//!
//! Everything in here is deterministic and free of UI. Front ends (the CLI, and later
//! the desktop app) call into this crate and render the results.
//!
//! - [`cue`]: parse CUE sheets into tracks with sample-accurate offsets.
//! - [`planner`]: decide how much gain each track gets, with true-peak protection.
//! - [`identify`]: who a song is (artist, title, album), from a CUE sheet or a file name.
//! - [`mix`]: mixing channels down (5.1 to quad, stereo or mono), by the rules of decision D-35.

pub mod cue;
pub mod identify;
pub mod mix;
pub mod planner;
