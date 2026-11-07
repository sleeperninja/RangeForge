//! The brain of the `RangeForge` screens.
//!
//! Everything a screen needs that is not drawing lives here, so the terminal view and the
//! desktop app share one set of rules and cannot disagree:
//!
//! - [`load`]: turning dropped paths into albums and songs.
//! - [`engine`]: measuring files on background threads.
//! - [`model`] and [`controls`]: the state of the screen and the rules for changing it.
//! - [`plan`]: each song's gain and output file name.
//! - [`runner`]: writing the finished files on a background thread.
//! - [`presets`]: saved sets of settings.
//! - [`naming`] and [`appsettings`]: the file name patterns, and the file that keeps them.
//! - [`dsd`]: what DSD files are written as.
//!
//! Nothing here draws, so all of it can be tested without a window or a terminal.

pub mod appsettings;
pub mod controls;
pub mod dsd;
pub mod engine;
pub mod genres;
pub mod identity;
pub mod load;
pub mod measuring;
pub mod model;
pub mod naming;
pub mod paths;
pub mod plan;
pub mod presets;
pub mod runner;
pub mod secrets;
pub mod snapshot;
