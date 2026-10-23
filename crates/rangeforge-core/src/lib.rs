//! RangeForge core.
//!
//! Everything in here is deterministic and free of UI. Front ends (the CLI, and later
//! the desktop app) call into this crate and render the results.
//!
//! - [`cue`]: parse CUE sheets into tracks with sample-accurate offsets.
//! - [`planner`]: decide how much gain each track gets, with true-peak protection.

pub mod cue;
pub mod planner;
