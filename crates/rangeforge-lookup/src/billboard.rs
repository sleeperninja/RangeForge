//! The Billboard Hot 100 history, as a small local index.
//!
//! The source is a public GitHub project that keeps every weekly chart in one 42 MB file,
//! `all.json`. Reading that on every launch would be wasteful, so it is turned once into a
//! list of the 30,000-odd songs that ever charted (artist, title, best position, first year)
//! and saved as a file of about 2 MB. A song is a "Radio" song if it is in that list.
//!
//! Keeping it current costs one small request: the server is asked whether the file has changed
//! since the saved copy (an `ETag` check), and the 42 MB is only downloaded when it has. If
//! anything goes wrong, the copy already on disk keeps working.

use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::normalize::{artist_keys, title_keys};

/// Where the whole chart history is published.
pub const SOURCE_URL: &str =
    "https://raw.githubusercontent.com/mhollingshead/billboard-hot-100/main/all.json";

/// Bump this if the saved index changes in a way older versions cannot read.
const VERSION: u32 = 1;

/// One song that reached the Hot 100.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Song {
    #[serde(rename = "a")]
    pub artist: String,
    #[serde(rename = "t")]
    pub title: String,
    /// The best position it reached: 1 is the top.
    #[serde(rename = "p")]
    pub peak: u16,
    /// The year it first appeared on the chart.
    #[serde(rename = "y")]
    pub first_year: u16,
}

/// Every song that charted, ready to be searched.
#[derive(Debug, Default)]
pub struct Hot100 {
    songs: Vec<Song>,
    /// Comparable artist and title text, to the position of the song in `songs`.
    keys: HashMap<String, usize>,
}

fn key(artist: &str, title: &str) -> String {
    format!("{artist}\u{1}{title}")
}

impl Hot100 {
    /// Builds the search table. A song is findable under each way its artist and title can
    /// reasonably be written (see [`crate::normalize`]).
    #[must_use]
    pub fn from_songs(songs: Vec<Song>) -> Self {
        let mut keys: HashMap<String, usize> = HashMap::new();
        for (index, song) in songs.iter().enumerate() {
            for artist in artist_keys(&song.artist) {
                for title in title_keys(&song.title) {
                    keys.entry(key(&artist, &title))
                        .and_modify(|existing| {
                            // Two charted songs can read the same once simplified; the one
                            // that did better wins.
                            if song.peak < songs[*existing].peak {
                                *existing = index;
                            }
                        })
                        .or_insert(index);
                }
            }
        }
        Self { songs, keys }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.songs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    /// The chart entry for a song, if it ever charted.
    #[must_use]
    pub fn find(&self, artist: &str, title: &str) -> Option<&Song> {
        let titles = title_keys(title);
        artist_keys(artist)
            .iter()
            .flat_map(|artist| titles.iter().map(move |title| key(artist, title)))
            .find_map(|wanted| self.keys.get(&wanted))
            .map(|&index| &self.songs[index])
    }
}

#[derive(Deserialize)]
struct Week {
    date: String,
    data: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    song: String,
    artist: String,
    this_week: u16,
}

/// Reads the published chart history (a list of weeks, each with its entries) and keeps one
/// record per song: its best position and the year it first charted.
pub fn songs_from_chart(reader: impl Read) -> Result<Vec<Song>> {
    let weeks: Vec<Week> = serde_json::from_reader(BufReader::new(reader))
        .context("the chart file is not the weekly list that was expected")?;
    let mut by_song: HashMap<(String, String), Song> = HashMap::new();
    for week in weeks {
        let year: u16 = week
            .date
            .get(..4)
            .and_then(|year| year.parse().ok())
            .unwrap_or(0);
        for entry in week.data {
            by_song
                .entry((entry.artist.clone(), entry.song.clone()))
                .and_modify(|song| {
                    song.peak = song.peak.min(entry.this_week);
                    if year != 0 && (song.first_year == 0 || year < song.first_year) {
                        song.first_year = year;
                    }
                })
                .or_insert(Song {
                    artist: entry.artist,
                    title: entry.song,
                    peak: entry.this_week,
                    first_year: year,
                });
        }
    }
    if by_song.is_empty() {
        bail!("the chart file has no songs in it");
    }
    let mut songs: Vec<Song> = by_song.into_values().collect();
    songs.sort_by(|a, b| (&a.artist, &a.title).cmp(&(&b.artist, &b.title)));
    Ok(songs)
}

/// The saved index: the songs, and what is needed to know whether it is still current.
#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    version: u32,
    /// The server's tag for the version of the chart file this was made from.
    etag: Option<String>,
    /// When the server was last asked whether there is something newer, in seconds since 1970.
    checked_at: u64,
    songs: Vec<Song>,
}

/// An index read from disk.
#[derive(Debug)]
pub struct Saved {
    pub hot100: Hot100,
    pub etag: Option<String>,
    pub checked_at: u64,
}

/// Reads the saved index, or `None` if there is none or it cannot be read (it is rebuilt by the
/// next refresh, so a damaged file costs one download and nothing else).
#[must_use]
pub fn load(path: &Path) -> Option<Saved> {
    let bytes = fs::read(path).ok()?;
    let stored: Stored = serde_json::from_slice(&bytes).ok()?;
    (stored.version == VERSION).then(|| Saved {
        hot100: Hot100::from_songs(stored.songs),
        etag: stored.etag,
        checked_at: stored.checked_at,
    })
}

/// Writes the index through a temporary file, so a crash cannot leave half a file behind.
fn save(path: &Path, stored: &Stored) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let json = serde_json::to_vec(stored).context("writing the chart index")?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, json).with_context(|| format!("writing {}", temporary.display()))?;
    fs::rename(&temporary, path).with_context(|| format!("saving {}", path.display()))?;
    Ok(())
}

/// What a request for the chart file returned.
pub enum Fetched {
    /// The copy we have is still the current one.
    NotModified,
    /// A newer copy, to be read from `body`.
    Body {
        body: Box<dyn Read>,
        etag: Option<String>,
    },
}

impl std::fmt::Debug for Fetched {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotModified => f.write_str("NotModified"),
            Self::Body { etag, .. } => f.debug_struct("Body").field("etag", etag).finish(),
        }
    }
}

/// Gets the chart file. A trait so that tests never touch the network.
pub trait Fetcher {
    /// Asks for `url`, unless the server's tag for it is still `etag`.
    fn get(&self, url: &str, etag: Option<&str>) -> Result<Fetched>;
}

/// The real thing: a plain HTTPS request.
#[derive(Debug, Default)]
pub struct HttpFetcher;

impl Fetcher for HttpFetcher {
    fn get(&self, url: &str, etag: Option<&str>) -> Result<Fetched> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(300))
            .user_agent(concat!("RangeForge/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("preparing the request")?;
        let mut request = client.get(url);
        if let Some(etag) = etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        let response = request
            .send()
            .with_context(|| format!("asking {url} for the chart file"))?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(Fetched::NotModified);
        }
        if !response.status().is_success() {
            bail!("{url} answered {}", response.status());
        }
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        Ok(Fetched::Body {
            body: Box::new(response),
            etag,
        })
    }
}

/// What a refresh did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refresh {
    /// The saved index was checked recently, so nobody was asked.
    Recent,
    /// The server says the saved index is still current.
    Unchanged,
    /// A newer chart was downloaded and the index rebuilt from it.
    Updated { songs: usize },
}

/// Seconds since 1970.
#[must_use]
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Brings the saved index up to date. If the saved copy was checked less than `min_interval`
/// ago, nothing is asked. Otherwise the server is asked whether the chart changed, and the
/// whole file is only downloaded if it did. Any failure leaves the saved copy untouched.
pub fn refresh(
    path: &Path,
    fetcher: &dyn Fetcher,
    now: u64,
    min_interval: Duration,
) -> Result<Refresh> {
    let saved = load(path);
    if let Some(saved) = &saved
        && now.saturating_sub(saved.checked_at) < min_interval.as_secs()
    {
        return Ok(Refresh::Recent);
    }
    let etag = saved.as_ref().and_then(|saved| saved.etag.as_deref());
    match fetcher.get(SOURCE_URL, etag)? {
        Fetched::NotModified => {
            if let Some(saved) = saved {
                // Remember that we asked, so the next check waits its turn.
                let songs = saved.hot100.songs;
                save(
                    path,
                    &Stored {
                        version: VERSION,
                        etag: saved.etag,
                        checked_at: now,
                        songs,
                    },
                )?;
            }
            Ok(Refresh::Unchanged)
        }
        Fetched::Body { body, etag } => {
            let songs = songs_from_chart(body)?;
            let count = songs.len();
            save(
                path,
                &Stored {
                    version: VERSION,
                    etag,
                    checked_at: now,
                    songs,
                },
            )?;
            Ok(Refresh::Updated { songs: count })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::io::Cursor;
    use tempfile::tempdir;

    const CHART: &str = r#"[
      {"date": "1973-10-20", "data": [
        {"song": "D'yer Mak'er", "artist": "Led Zeppelin", "this_week": 71, "last_week": null, "peak_position": 71, "weeks_on_chart": 1},
        {"song": "Mr. Brightside", "artist": "The Killers", "this_week": 99, "last_week": null, "peak_position": 99, "weeks_on_chart": 1}
      ]},
      {"date": "1973-11-03", "data": [
        {"song": "D'yer Mak'er", "artist": "Led Zeppelin", "this_week": 20, "last_week": 40, "peak_position": 20, "weeks_on_chart": 3},
        {"song": "The Sound Of Silence", "artist": "Simon & Garfunkel", "this_week": 1, "last_week": 2, "peak_position": 1, "weeks_on_chart": 9},
        {"song": "A Great Big Sled", "artist": "The Killers Featuring Toni Halliday", "this_week": 54, "last_week": null, "peak_position": 54, "weeks_on_chart": 1}
      ]}
    ]"#;

    fn hot100() -> Hot100 {
        Hot100::from_songs(songs_from_chart(Cursor::new(CHART)).unwrap())
    }

    #[test]
    fn a_song_keeps_its_best_position_and_first_year() {
        let songs = songs_from_chart(Cursor::new(CHART)).unwrap();
        assert_eq!(songs.len(), 4);
        let dyer = songs.iter().find(|s| s.title == "D'yer Mak'er").unwrap();
        assert_eq!((dyer.peak, dyer.first_year), (20, 1973));
    }

    #[test]
    fn songs_are_found_however_they_are_written() {
        let chart = hot100();
        for (artist, title) in [
            ("Led Zeppelin", "D'yer Mak'er"),
            ("led zeppelin", "Dyer Maker"),
            (
                "Led Zeppelin",
                "D\u{2019}yer Mak\u{2019}er (Remastered 2014)",
            ),
            ("Killers", "Mr Brightside"),
            ("The Killers", "Mr. Brightside - 2005 Remaster"),
            ("Simon and Garfunkel", "The Sound of Silence"),
            ("Simon & Garfunkel", "Sound Of Silence"),
        ] {
            assert!(chart.find(artist, title).is_some(), "{artist} - {title}");
        }
    }

    #[test]
    fn a_guest_credit_matches_the_lead_artist_either_way() {
        let chart = hot100();
        // The chart credits a guest; the file names only the band.
        assert!(chart.find("The Killers", "A Great Big Sled").is_some());
        // The file names the guest too.
        assert!(
            chart
                .find("The Killers feat. Toni Halliday", "A Great Big Sled")
                .is_some()
        );
    }

    #[test]
    fn songs_that_never_charted_are_not_found() {
        let chart = hot100();
        assert!(chart.find("Led Zeppelin", "The Rain Song").is_none());
        assert!(chart.find("Someone Else", "D'yer Mak'er").is_none());
        assert!(chart.find("", "").is_none());
    }

    #[test]
    fn two_songs_that_read_alike_give_the_better_position() {
        let songs = vec![
            Song {
                artist: "Band".to_string(),
                title: "Song (Live)".to_string(),
                peak: 80,
                first_year: 1990,
            },
            Song {
                artist: "Band".to_string(),
                title: "Song".to_string(),
                peak: 12,
                first_year: 1989,
            },
        ];
        let chart = Hot100::from_songs(songs);
        assert_eq!(chart.find("Band", "Song").unwrap().peak, 12);
    }

    #[test]
    fn a_file_that_is_not_a_chart_is_refused() {
        assert!(songs_from_chart(Cursor::new("{}")).is_err());
        assert!(songs_from_chart(Cursor::new("[]")).is_err());
    }

    /// One scripted answer: an error, "not modified", or a body and its tag.
    type Answer = Result<Option<(&'static str, &'static str)>>;

    /// Answers from a script and remembers what it was asked.
    struct Script {
        answers: RefCell<Vec<Answer>>,
        asked: RefCell<Vec<Option<String>>>,
    }

    impl Script {
        fn new(answers: Vec<Answer>) -> Self {
            Self {
                answers: RefCell::new(answers),
                asked: RefCell::new(Vec::new()),
            }
        }
    }

    impl Fetcher for Script {
        fn get(&self, _url: &str, etag: Option<&str>) -> Result<Fetched> {
            self.asked.borrow_mut().push(etag.map(str::to_string));
            match self.answers.borrow_mut().remove(0)? {
                None => Ok(Fetched::NotModified),
                Some((body, etag)) => Ok(Fetched::Body {
                    body: Box::new(Cursor::new(body.as_bytes().to_vec())),
                    etag: Some(etag.to_string()),
                }),
            }
        }
    }

    const HOUR: Duration = Duration::from_secs(3600);

    #[test]
    fn the_first_refresh_downloads_and_saves_the_index() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("index.json");
        let script = Script::new(vec![Ok(Some((CHART, "\"v1\"")))]);

        let done = refresh(&path, &script, 1_000, HOUR).unwrap();

        assert_eq!(done, Refresh::Updated { songs: 4 });
        assert_eq!(script.asked.borrow()[0], None);
        let saved = load(&path).unwrap();
        assert_eq!(saved.hot100.len(), 4);
        assert_eq!(saved.etag.as_deref(), Some("\"v1\""));
        assert_eq!(saved.checked_at, 1_000);
        assert!(saved.hot100.find("Led Zeppelin", "D'yer Mak'er").is_some());
    }

    #[test]
    fn a_recent_check_asks_nobody() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("index.json");
        refresh(
            &path,
            &Script::new(vec![Ok(Some((CHART, "\"v1\"")))]),
            1_000,
            HOUR,
        )
        .unwrap();

        let script = Script::new(vec![]);
        let done = refresh(&path, &script, 1_000 + 60, HOUR).unwrap();

        assert_eq!(done, Refresh::Recent);
        assert!(script.asked.borrow().is_empty());
    }

    #[test]
    fn an_old_check_asks_with_the_saved_tag_and_keeps_the_copy_if_unchanged() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("index.json");
        refresh(
            &path,
            &Script::new(vec![Ok(Some((CHART, "\"v1\"")))]),
            1_000,
            HOUR,
        )
        .unwrap();

        let script = Script::new(vec![Ok(None)]);
        let done = refresh(&path, &script, 1_000 + 7_200, HOUR).unwrap();

        assert_eq!(done, Refresh::Unchanged);
        assert_eq!(script.asked.borrow()[0].as_deref(), Some("\"v1\""));
        let saved = load(&path).unwrap();
        assert_eq!(saved.checked_at, 1_000 + 7_200, "the check is remembered");
        assert_eq!(saved.hot100.len(), 4, "the songs are kept");
        assert_eq!(saved.etag.as_deref(), Some("\"v1\""));
    }

    #[test]
    fn a_changed_chart_replaces_the_index() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("index.json");
        refresh(
            &path,
            &Script::new(vec![Ok(Some((CHART, "\"v1\"")))]),
            1_000,
            HOUR,
        )
        .unwrap();

        let newer = r#"[{"date": "2026-10-24", "data": [
            {"song": "Brand New", "artist": "Someone", "this_week": 3, "last_week": null, "peak_position": 3, "weeks_on_chart": 1}
        ]}]"#;
        let done = refresh(
            &path,
            &Script::new(vec![Ok(Some((newer, "\"v2\"")))]),
            1_000 + 7_200,
            HOUR,
        )
        .unwrap();

        assert_eq!(done, Refresh::Updated { songs: 1 });
        let saved = load(&path).unwrap();
        assert_eq!(saved.etag.as_deref(), Some("\"v2\""));
        assert!(saved.hot100.find("Someone", "Brand New").is_some());
    }

    #[test]
    fn a_failed_request_leaves_the_saved_index_alone() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("index.json");
        refresh(
            &path,
            &Script::new(vec![Ok(Some((CHART, "\"v1\"")))]),
            1_000,
            HOUR,
        )
        .unwrap();
        let before = fs::read(&path).unwrap();

        let offline = Script::new(vec![Err(anyhow::anyhow!("no network"))]);
        assert!(refresh(&path, &offline, 1_000 + 7_200, HOUR).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);

        // A download that arrives as nonsense is refused the same way.
        let nonsense = Script::new(vec![Ok(Some(("not json", "\"v3\"")))]);
        assert!(refresh(&path, &nonsense, 1_000 + 7_200, HOUR).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn a_damaged_saved_index_is_just_downloaded_again() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("index.json");
        fs::write(&path, b"{ broken").unwrap();
        assert!(load(&path).is_none());

        let script = Script::new(vec![Ok(Some((CHART, "\"v1\"")))]);
        let done = refresh(&path, &script, 5_000, HOUR).unwrap();

        assert_eq!(done, Refresh::Updated { songs: 4 });
        assert_eq!(script.asked.borrow()[0], None, "no stale tag is sent");
        assert!(load(&path).is_some());
    }

    #[test]
    fn saving_leaves_no_temporary_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("index.json");
        refresh(
            &path,
            &Script::new(vec![Ok(Some((CHART, "\"v1\"")))]),
            1,
            HOUR,
        )
        .unwrap();
        let names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["index.json"]);
    }

    /// Checks the real chart file, which is not kept in git. Set `RANGEFORGE_HOT100_JSON` to a
    /// copy of `all.json` and run with `--ignored`.
    #[test]
    #[ignore = "needs a copy of the real all.json"]
    fn the_real_chart_knows_houses_of_the_holy_and_friends() {
        let path = std::env::var("RANGEFORGE_HOT100_JSON").expect("set RANGEFORGE_HOT100_JSON");
        let songs = songs_from_chart(fs::File::open(path).unwrap()).unwrap();
        assert!(songs.len() > 30_000, "{} songs", songs.len());
        let chart = Hot100::from_songs(songs);

        // Houses of the Holy: three of its songs charted.
        for (title, peak) in [
            ("D'yer Mak'er", 20),
            ("Over the Hills and Far Away", 51),
            ("Living Loving Maid (She's Just a Woman)", 65),
        ] {
            let found = chart.find("Led Zeppelin", title).expect(title);
            assert_eq!(found.peak, peak, "{title}");
        }
        for title in ["The Rain Song", "No Quarter", "The Ocean", "The Crunge"] {
            assert!(chart.find("Led Zeppelin", title).is_none(), "{title}");
        }
        // Spellings that differ from the chart's.
        assert!(chart.find("Simon and Garfunkel", "Mrs. Robinson").is_some());
        assert!(chart.find("Killers", "Mr Brightside").is_some());
        assert!(chart.find("The Killers", "A Great Big Sled").is_some());
    }
}
