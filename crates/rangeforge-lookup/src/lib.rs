//! Looking songs up in outside data for `RangeForge`.
//!
//! - [`normalize`]: making artist and title text comparable.
//! - [`billboard`]: the Billboard Hot 100 history as a small local index, the source of the
//!   "Radio" tag.
//! - [`lastfm`]: asking Last.fm what listeners tag an album or song with.
//! - [`genres`]: keeping the real genres among those tags.
//! - [`tagcache`]: remembering Last.fm's answers.
//! - [`songtags`]: song tags the user pasted from Last.fm's tags page, kept apart from the cache.
//!
//! Nothing here runs unless the user turns the Metadata option on, and the network is only
//! touched through [`billboard::Fetcher`] and [`lastfm::Get`], so every rule can be tested
//! without it.

pub mod billboard;
pub mod genres;
pub mod lastfm;
pub mod normalize;
pub mod songtags;
pub mod tagcache;
