//! Asking Last.fm what listeners call an album or a song.
//!
//! Last.fm's "top tags" are what people have tagged something with, each with a popularity from
//! 0 to 100 (the most used tag is 100). They are the raw material for genres: see
//! [`crate::genres`] for how they are filtered.
//!
//! The network is only touched through [`Get`], so every rule here can be tested with canned
//! answers. The API key is only ever sent to Last.fm. It is never put in an error message, since
//! the HTTP library's errors would otherwise print the whole address, key included.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Where Last.fm's API lives.
pub const API_URL: &str = "https://ws.audioscrobbler.com/2.0/";

/// A tag as Last.fm gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawTag {
    pub name: String,
    /// How popular it is for this album or song, 0 to 100.
    pub count: u32,
}

/// Why asking Last.fm failed.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LastFmError {
    #[error("Last.fm does not accept the API key")]
    InvalidKey,
    #[error("Last.fm says to slow down")]
    RateLimited,
    #[error("Last.fm could not be reached: {0}")]
    Network(String),
    #[error("Last.fm answered with something unexpected: {0}")]
    Unexpected(String),
}

/// Makes one request to Last.fm and returns the answer's text, whatever the HTTP status: Last.fm
/// explains its errors in the body.
pub trait Get: Send + Sync {
    fn get(&self, params: &[(&str, &str)]) -> Result<String, LastFmError>;
}

/// The real thing: an HTTPS request.
#[derive(Debug, Default)]
pub struct HttpGet;

impl Get for HttpGet {
    fn get(&self, params: &[(&str, &str)]) -> Result<String, LastFmError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("RangeForge/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| LastFmError::Network(error.without_url().to_string()))?;
        client
            .get(API_URL)
            .query(params)
            .send()
            .and_then(reqwest::blocking::Response::text)
            // `without_url`: the address holds the key.
            .map_err(|error| LastFmError::Network(error.without_url().to_string()))
    }
}

/// A Last.fm client.
pub struct LastFm<G> {
    get: G,
    api_key: String,
}

// Not derived: that would print the key into any log or test failure that shows the client.
impl<G> std::fmt::Debug for LastFm<G> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LastFm")
            .field("api_key", &"<hidden>")
            .finish_non_exhaustive()
    }
}

impl<G: Get> LastFm<G> {
    pub fn new(get: G, api_key: impl Into<String>) -> Self {
        Self {
            get,
            api_key: api_key.into(),
        }
    }

    /// The tags listeners gave an album.
    pub fn album_tags(&self, artist: &str, album: &str) -> Result<Vec<RawTag>, LastFmError> {
        self.ask("album.gettoptags", &[("artist", artist), ("album", album)])
    }

    /// The tags listeners gave a song.
    pub fn track_tags(&self, artist: &str, title: &str) -> Result<Vec<RawTag>, LastFmError> {
        self.ask("track.gettoptags", &[("artist", artist), ("track", title)])
    }

    fn ask(&self, method: &str, what: &[(&str, &str)]) -> Result<Vec<RawTag>, LastFmError> {
        let mut params = vec![
            ("method", method),
            ("api_key", self.api_key.as_str()),
            // Let Last.fm correct small spelling differences.
            ("autocorrect", "1"),
            ("format", "json"),
        ];
        params.extend_from_slice(what);
        parse_top_tags(&self.get.get(&params)?)
    }
}

/// Reads an answer to a `gettoptags` request. A song or album Last.fm does not know is not an
/// error: it has no tags.
pub fn parse_top_tags(body: &str) -> Result<Vec<RawTag>, LastFmError> {
    let json: Value =
        serde_json::from_str(body).map_err(|_| LastFmError::Unexpected("not JSON".to_string()))?;

    if let Some(code) = json.get("error").and_then(Value::as_i64) {
        return match code {
            6 => Ok(Vec::new()),
            10 | 26 => Err(LastFmError::InvalidKey),
            29 => Err(LastFmError::RateLimited),
            _ => Err(LastFmError::Unexpected(
                json.get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("an error")
                    .to_string(),
            )),
        };
    }

    // One tag arrives as an object and several as a list.
    let tags = match json.get("toptags").and_then(|top| top.get("tag")) {
        Some(Value::Array(list)) => list.clone(),
        Some(single @ Value::Object(_)) => vec![single.clone()],
        _ => Vec::new(),
    };
    Ok(tags
        .iter()
        .filter_map(|tag| {
            let name = tag.get("name")?.as_str()?.trim().to_string();
            // A count is a number, but has been seen as text.
            let count = match tag.get("count")? {
                Value::Number(number) => number.as_u64()?,
                Value::String(text) => text.parse().ok()?,
                _ => return None,
            };
            (!name.is_empty()).then(|| RawTag {
                name,
                count: u32::try_from(count).unwrap_or(u32::MAX),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    const HOUSES: &str = r#"{"toptags":{"tag":[
        {"count":100,"name":"classic rock","url":"x"},
        {"count":88,"name":"hard rock","url":"x"},
        {"count":"41","name":"seen live","url":"x"}
    ],"@attr":{"artist":"Led Zeppelin","album":"Houses of the Holy"}}}"#;

    #[test]
    fn tags_are_read_in_order_with_their_popularity() {
        let tags = parse_top_tags(HOUSES).unwrap();
        assert_eq!(
            tags,
            [
                RawTag {
                    name: "classic rock".to_string(),
                    count: 100
                },
                RawTag {
                    name: "hard rock".to_string(),
                    count: 88
                },
                RawTag {
                    name: "seen live".to_string(),
                    count: 41
                },
            ]
        );
    }

    #[test]
    fn a_single_tag_and_an_empty_answer_are_both_fine() {
        let one = r#"{"toptags":{"tag":{"count":100,"name":"rock","url":"x"}}}"#;
        assert_eq!(parse_top_tags(one).unwrap().len(), 1);
        assert_eq!(parse_top_tags(r#"{"toptags":{"tag":[]}}"#).unwrap(), []);
        assert_eq!(parse_top_tags(r#"{"toptags":""}"#).unwrap(), []);
    }

    #[test]
    fn something_last_fm_does_not_know_has_no_tags_and_is_not_an_error() {
        let unknown = r#"{"error":6,"message":"Album not found"}"#;
        assert_eq!(parse_top_tags(unknown).unwrap(), []);
    }

    #[test]
    fn a_bad_key_and_a_busy_server_are_told_apart() {
        let bad = r#"{"error":10,"message":"Invalid API key - You must be granted a valid key by last.fm"}"#;
        assert_eq!(parse_top_tags(bad), Err(LastFmError::InvalidKey));
        let suspended = r#"{"error":26,"message":"Suspended API key"}"#;
        assert_eq!(parse_top_tags(suspended), Err(LastFmError::InvalidKey));
        let busy = r#"{"error":29,"message":"Rate limit exceeded"}"#;
        assert_eq!(parse_top_tags(busy), Err(LastFmError::RateLimited));
        let other = r#"{"error":8,"message":"Operation failed"}"#;
        assert_eq!(
            parse_top_tags(other),
            Err(LastFmError::Unexpected("Operation failed".to_string()))
        );
    }

    #[test]
    fn nonsense_is_reported_not_trusted() {
        assert!(matches!(
            parse_top_tags("<html>maintenance</html>"),
            Err(LastFmError::Unexpected(_))
        ));
        // Tags without a name or a number are skipped, not guessed at.
        let odd =
            r#"{"toptags":{"tag":[{"name":"","count":5},{"name":"x"},{"name":"ok","count":7}]}}"#;
        let tags = parse_top_tags(odd).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "ok");
    }

    /// Remembers what it was asked and answers with a canned text.
    struct Canned {
        answer: String,
        asked: Mutex<Vec<Vec<(String, String)>>>,
    }

    impl Get for Canned {
        fn get(&self, params: &[(&str, &str)]) -> Result<String, LastFmError> {
            self.asked.lock().unwrap().push(
                params
                    .iter()
                    .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                    .collect(),
            );
            Ok(self.answer.clone())
        }
    }

    #[test]
    fn requests_name_the_method_and_carry_the_key_only_as_a_parameter() {
        let client = LastFm::new(
            Canned {
                answer: HOUSES.to_string(),
                asked: Mutex::new(Vec::new()),
            },
            "the-key",
        );
        let tags = client
            .album_tags("Led Zeppelin", "Houses of the Holy")
            .unwrap();
        assert_eq!(tags.len(), 3);
        client.track_tags("Led Zeppelin", "No Quarter").unwrap();

        let asked = client.get.asked.lock().unwrap();
        let value = |request: usize, name: &str| {
            asked[request]
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        };
        assert_eq!(value(0, "method").as_deref(), Some("album.gettoptags"));
        assert_eq!(value(0, "album").as_deref(), Some("Houses of the Holy"));
        assert_eq!(value(0, "api_key").as_deref(), Some("the-key"));
        assert_eq!(value(0, "format").as_deref(), Some("json"));
        assert_eq!(value(1, "method").as_deref(), Some("track.gettoptags"));
        assert_eq!(value(1, "track").as_deref(), Some("No Quarter"));
    }

    #[test]
    fn the_key_never_shows_in_debug_output_or_in_an_error() {
        let client = LastFm::new(HttpGet, "secret-key-123");
        let shown = format!("{client:?}");
        assert!(!shown.contains("secret-key-123"), "{shown}");
        assert!(shown.contains("hidden"));

        // Errors hold only text that was put in them, never a request address.
        let failing = LastFmError::Network("connection refused".to_string());
        assert!(!failing.to_string().contains("api_key"));
    }
}
