//! Looking songs up in outside data for `RangeForge`.
//!
//! - [`normalize`]: making artist and title text comparable.
//! - [`billboard`]: the Billboard Hot 100 history as a small local index, the source of the
//!   "Radio" tag.
//!
//! Nothing here runs unless the user turns the Metadata option on, and the network is only
//! touched through [`billboard::Fetcher`], so every rule can be tested without it.

pub mod billboard;
pub mod normalize;
