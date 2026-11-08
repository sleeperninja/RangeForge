//! Asking Last.fm what listeners call an album or a song.
//!
//! Last.fm's "top tags" are what people have tagged something with, each with a popularity from
//! 0 to 100 (the most used tag is 100). They are the raw material for genres: see
//! [`crate::genres`] for how they are filtered.
//!
//! The network is only touched through [`Get`], so every rule here can be tested with canned
//! answers. The API key is only ever sent to Last.fm. It is never put in an error message, since
//! the HTTP library's errors would otherwise print the whole address, key included.

use std::fmt::Write as _;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::normalize::{artist_spellings, spellings};

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

    /// Fetches a public web page and returns its HTML, whatever the HTTP status.
    fn page(&self, _url: &str) -> Result<String, LastFmError> {
        Err(LastFmError::Network(
            "web pages are not available".to_string(),
        ))
    }
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

    fn page(&self, url: &str) -> Result<String, LastFmError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("RangeForge/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| LastFmError::Network(error.without_url().to_string()))?;
        client
            .get(url)
            .send()
            .and_then(reqwest::blocking::Response::text)
            .map_err(|error| LastFmError::Network(error.without_url().to_string()))
    }
}

/// A Last.fm client.
pub struct LastFm<G> {
    get: G,
    api_key: String,
    /// Waited between the tries at other spellings, to stay within Last.fm's limits.
    pause: Duration,
    /// What each try of the last question got back, for the log when nothing was found.
    trail: std::sync::Mutex<Vec<String>>,
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
            pause: Duration::from_millis(250),
            trail: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// What each try of the last question got back, such as `“Led Zeppelin” – “Song”: Last.fm
    /// does not know it`. Taking it empties it.
    pub fn take_trail(&self) -> Vec<String> {
        self.trail
            .lock()
            .map(|mut trail| std::mem::take(&mut *trail))
            .unwrap_or_default()
    }

    /// How long to wait between tries at other spellings. Tests use none.
    #[must_use]
    pub fn with_pause(mut self, pause: Duration) -> Self {
        self.pause = pause;
        self
    }

    /// The tags listeners gave an album.
    pub fn album_tags(&self, artist: &str, album: &str) -> Result<Vec<RawTag>, LastFmError> {
        self.ask_spellings(artist, album, |artist, album| {
            self.ask("album.gettoptags", &[("artist", artist), ("album", album)])
        })
    }

    /// The tags listeners gave a song. The API's `track.getTopTags` answers with nothing for
    /// songs that do have tags on the site, so the song's public tags page is read instead:
    /// `last.fm/music/<artist>/_/<song>/+tags`. The page lists tags most-used first but gives no
    /// counts, so each gets a made-up popularity that falls with its place in the list.
    pub fn track_tags(&self, artist: &str, title: &str) -> Result<Vec<RawTag>, LastFmError> {
        self.ask_spellings(artist, title, |artist, title| {
            let url = tags_page_url(artist, title);
            let html = self.get.page(&url)?;
            let tags = parse_tags_page(&html);
            if tags.is_empty()
                && let Ok(mut trail) = self.trail.lock()
            {
                trail.push(format!(
                    "“{artist}” – “{title}”: the Last.fm page has no tags ({url})"
                ));
            }
            Ok(tags)
        })
    }

    /// Asks as written first. Last.fm answers "no such song" (not an error, just no tags) when
    /// the spelling differs from its own, such as a curly apostrophe in `D’yer Mak’er` or a
    /// `(Remastered)` note, so on an empty answer other spellings are tried: the name with plain
    /// punctuation and without notes, then the artist with "The" added or removed. A real
    /// error stops at once.
    fn ask_spellings(
        &self,
        artist: &str,
        name: &str,
        ask: impl Fn(&str, &str) -> Result<Vec<RawTag>, LastFmError>,
    ) -> Result<Vec<RawTag>, LastFmError> {
        self.take_trail();
        let mut names = spellings(name);
        let mut artists = artist_spellings(artist);
        if names.is_empty() {
            names.push(name.to_string());
        }
        if artists.is_empty() {
            artists.push(artist.to_string());
        }
        let mut attempts: Vec<(&str, &str)> = Vec::new();
        for name in &names {
            attempts.push((&artists[0], name));
        }
        for other in artists.iter().skip(1) {
            for name in names.iter().take(2) {
                attempts.push((other, name));
            }
        }
        attempts.truncate(8);
        for (index, (artist, name)) in attempts.into_iter().enumerate() {
            if index > 0 {
                std::thread::sleep(self.pause);
            }
            let tags = ask(artist, name)?;
            if !tags.is_empty() {
                return Ok(tags);
            }
        }
        Ok(Vec::new())
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
        let body = self.get.get(&params)?;
        let tags = parse_top_tags(&body)?;
        if tags.is_empty()
            && let Ok(mut trail) = self.trail.lock()
        {
            let asked: Vec<String> = what.iter().map(|(_, value)| format!("“{value}”")).collect();
            let result = if is_not_found(&body) {
                "Last.fm does not know it"
            } else {
                "Last.fm knows it, but nobody has tagged it"
            };
            trail.push(format!("{}: {result}", asked.join(" – ")));
        }
        Ok(tags)
    }
}

/// Where a song's tags page lives. The album is not needed, so `_` stands in for it.
pub fn tags_page_url(artist: &str, title: &str) -> String {
    format!(
        "https://www.last.fm/music/{}/_/{}/+tags",
        url_part(artist),
        url_part(title)
    )
}

/// One piece of a Last.fm address: spaces are `+`, anything unusual is percent-encoded.
fn url_part(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(byte));
            }
            b' ' => out.push('+'),
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

/// Reads the tag names off a song's tags page: the text of each link in a heading inside a
/// list (`ol > li > div > h3 > a`). Counts are not on the page, so they are made up from the
/// order: 100 for the first, 5 fewer for each after, never below 10. The page's order is
/// already most-used first, and the genre rules still choose which ones count.
pub fn parse_tags_page(html: &str) -> Vec<RawTag> {
    let lower = html.to_ascii_lowercase();
    let mut names: Vec<String> = Vec::new();
    let mut from = 0;
    while let Some(start) = lower[from..].find("<ol") {
        let start = from + start;
        let end = lower[start..]
            .find("</ol>")
            .map_or(lower.len(), |end| start + end);
        let mut at = start;
        while let Some(h3) = lower[at..end].find("<h3") {
            let h3 = at + h3;
            let h3_end = lower[h3..end].find("</h3>").map_or(end, |e| h3 + e);
            if let Some(a) = lower[h3..h3_end].find("<a")
                && let Some(open) = lower[h3 + a..h3_end].find('>')
            {
                let text_start = h3 + a + open + 1;
                let text_end = lower[text_start..h3_end]
                    .find("</a>")
                    .map_or(h3_end, |e| text_start + e);
                let name = clean_text(&html[text_start..text_end]);
                if !name.is_empty() && !names.contains(&name) {
                    names.push(name);
                }
            }
            at = h3_end;
        }
        from = end;
    }
    names
        .into_iter()
        .enumerate()
        .map(|(rank, name)| {
            let fall = u32::try_from(rank).unwrap_or(u32::MAX).saturating_mul(5);
            RawTag {
                name,
                count: 100u32.saturating_sub(fall).max(10),
            }
        })
        .collect()
}

/// Text from HTML: tags dropped, the usual entities decoded, spaces tidied.
fn clean_text(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let decoded = text
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// True when Last.fm's answer is its "not found" error rather than an empty list of tags.
fn is_not_found(body: &str) -> bool {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|json| json.get("error").and_then(Value::as_i64))
        == Some(6)
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

    /// A song's tags page as the site serves it (trimmed): the tag list is `ol > li > div > h3 >
    /// a`, and other headings and links around it are not tags.
    const TAGS_PAGE: &str = r#"<html><body><h3><a href="/x">Not a tag</a></h3>
<ol class="big-tags">
<li class="big-tags-item"><div class="big-tags-item-inner"><h3 class="big-tags-item-name"><a href="/tag/hard+rock">hard rock</a></h3></div></li>
<li class="big-tags-item"><div><h3><a href="/tag/rock"> rock </a></h3></div></li>
<li class="big-tags-item"><div><h3><a href="/tag/reggae">reggae</a></h3></div></li>
<li class="big-tags-item"><div><h3><a href="/tag/r">R&amp;B &#39;70s</a></h3></div></li>
</ol><h3><a href="/y">Also not a tag</a></h3></body></html>"#;

    /// Serves the tags page for one address only; any other is a page with no tags.
    struct Picky {
        knows: &'static str,
        asked: Mutex<Vec<String>>,
        down: bool,
    }

    impl Get for Picky {
        fn get(&self, _: &[(&str, &str)]) -> Result<String, LastFmError> {
            Ok(HOUSES.to_string())
        }

        fn page(&self, url: &str) -> Result<String, LastFmError> {
            self.asked.lock().unwrap().push(url.to_string());
            if self.down {
                return Err(LastFmError::Network("refused".to_string()));
            }
            Ok(if url == self.knows {
                TAGS_PAGE.to_string()
            } else {
                "<html><h3><a>Page not found</a></h3></html>".to_string()
            })
        }
    }

    fn picky(knows: &'static str, down: bool) -> LastFm<Picky> {
        LastFm::new(
            Picky {
                knows,
                asked: Mutex::new(Vec::new()),
                down,
            },
            "key",
        )
        .with_pause(Duration::ZERO)
    }

    #[test]
    fn a_tags_page_gives_its_tags_in_order() {
        let tags = parse_tags_page(TAGS_PAGE);
        let names: Vec<&str> = tags.iter().map(|tag| tag.name.as_str()).collect();
        assert_eq!(names, ["hard rock", "rock", "reggae", "R&B '70s"]);
        assert_eq!(tags[0].count, 100);
        assert_eq!(tags[1].count, 95);
        assert_eq!(parse_tags_page("<html>nothing here</html>"), []);
        // Far down the list the made-up count stays above the cut.
        let mut long = String::new();
        for n in 0..40 {
            let _ = write!(long, "<ol><li><div><h3><a>tag {n}</a></h3></div></li></ol>");
        }
        assert_eq!(parse_tags_page(&long).last().unwrap().count, 10);
    }

    #[test]
    fn a_songs_address_uses_plus_for_spaces_and_an_underscore_for_the_album() {
        assert_eq!(
            tags_page_url("Led Zeppelin", "D'yer Mak'er"),
            "https://www.last.fm/music/Led+Zeppelin/_/D%27yer+Mak%27er/+tags"
        );
        assert_eq!(
            tags_page_url("AC/DC", "Rock & Roll"),
            "https://www.last.fm/music/AC%2FDC/_/Rock+%26+Roll/+tags"
        );
    }

    #[test]
    fn a_curly_apostrophe_or_a_missing_the_does_not_hide_a_songs_tags() {
        // The file says D’yer Mak’er (typographic apostrophes); Last.fm spells it plainly.
        let client = picky(
            "https://www.last.fm/music/Led+Zeppelin/_/D%27yer+Mak%27er/+tags",
            false,
        );
        let tags = client
            .track_tags("Led Zeppelin", "D\u{2019}yer Mak\u{2019}er")
            .unwrap();
        assert_eq!(tags.len(), 4);
        let asked = client.get.asked.lock().unwrap();
        assert!(
            asked[0].contains("%E2%80%99"),
            "as written first: {asked:?}"
        );
        assert!(asked[1].contains("D%27yer+Mak%27er"));
        drop(asked);

        // A version note and the wrong "The" are handled too.
        let client = picky(
            "https://www.last.fm/music/The+Alan+Parsons+Project/_/I+Robot/+tags",
            false,
        );
        let tags = client
            .track_tags("Alan Parsons Project", "I Robot (Remastered 2007)")
            .unwrap();
        assert_eq!(tags.len(), 4);
    }

    #[test]
    fn a_song_without_a_tags_page_stops_trying_and_a_real_error_stops_at_once() {
        let client = picky("https://nowhere/", false);
        assert_eq!(client.track_tags("Nobody", "Nothing").unwrap(), []);
        // What was tried is kept, so the log can say it.
        let trail = client.take_trail();
        assert!(
            trail[0].starts_with("“Nobody” – “Nothing”: the Last.fm page has no tags"),
            "{trail:?}"
        );
        assert!(trail.len() > 1, "other spellings were tried: {trail:?}");
        assert_eq!(client.take_trail().len(), 0, "taking it empties it");
        assert!(
            client.get.asked.lock().unwrap().len() <= 8,
            "a limited number of tries"
        );

        let client = picky("https://nowhere/", true);
        assert!(matches!(
            client.track_tags("Led Zeppelin", "D\u{2019}yer Mak\u{2019}er"),
            Err(LastFmError::Network(_))
        ));
        assert_eq!(
            client.get.asked.lock().unwrap().len(),
            1,
            "no retry on a real error"
        );
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
