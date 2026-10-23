//! CUE sheet parsing.
//!
//! Pure: it takes the text of a CUE sheet and returns a structure. It never touches the
//! file system, so the caller decides how to read the file and which text encoding to use.
//!
//! Positions are kept as [`Frames`] (75 per second, as on a CD) so offsets stay exact.
//! Converting them to audio sample positions is the decoder's job.
//!
//! Only audio tracks are kept. Data tracks are skipped.

use std::fmt;

use thiserror::Error;

/// CD frames per second, the unit CUE sheets use.
pub const FRAMES_PER_SECOND: u32 = 75;

/// A position in a CUE sheet, in CD frames (1/75 s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Frames(pub u32);

impl Frames {
    /// Parses `MM:SS:FF`. Returns `None` for malformed text, `SS` of 60 or more,
    /// `FF` of 75 or more, or a value too large to represent.
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split(':');
        let minutes: u32 = parts.next()?.parse().ok()?;
        let seconds: u32 = parts.next()?.parse().ok()?;
        let frames: u32 = parts.next()?.parse().ok()?;
        if parts.next().is_some() || seconds >= 60 || frames >= FRAMES_PER_SECOND {
            return None;
        }
        minutes
            .checked_mul(60)?
            .checked_add(seconds)?
            .checked_mul(FRAMES_PER_SECOND)?
            .checked_add(frames)
            .map(Self)
    }

    /// The position in seconds.
    pub fn as_seconds(self) -> f64 {
        f64::from(self.0) / f64::from(FRAMES_PER_SECOND)
    }
}

impl fmt::Display for Frames {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total_seconds = self.0 / FRAMES_PER_SECOND;
        let frames = self.0 % FRAMES_PER_SECOND;
        write!(
            f,
            "{:02}:{:02}:{frames:02}",
            total_seconds / 60,
            total_seconds % 60
        )
    }
}

/// Why a CUE sheet could not be parsed.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CueError {
    #[error("line {line}: {keyword} needs an argument")]
    MissingArgument { line: usize, keyword: &'static str },
    #[error("line {line}: '{text}' is not a valid number")]
    InvalidNumber { line: usize, text: String },
    #[error("line {line}: '{text}' is not a valid MM:SS:FF time")]
    InvalidTime { line: usize, text: String },
    #[error("line {line}: TRACK appears before any FILE")]
    TrackBeforeFile { line: usize },
    #[error("line {line}: INDEX appears outside a TRACK")]
    IndexOutsideTrack { line: usize },
    #[error("track {track} (line {line}) has no INDEX 01")]
    MissingIndex { line: usize, track: u32 },
    #[error("track {track} starts before the track that precedes it")]
    OutOfOrder { track: u32 },
    #[error("the CUE sheet contains no audio tracks")]
    NoTracks,
}

/// A parsed CUE sheet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CueSheet {
    pub title: Option<String>,
    pub performer: Option<String>,
    pub genre: Option<String>,
    pub date: Option<String>,
    pub disc_id: Option<String>,
    pub comment: Option<String>,
    pub files: Vec<CueFile>,
}

/// One `FILE` entry and the audio tracks that live in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CueFile {
    /// The name as written in the sheet. It is not checked against the file system.
    pub name: String,
    pub tracks: Vec<CueTrack>,
}

/// One audio track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CueTrack {
    pub number: u32,
    pub title: Option<String>,
    pub performer: Option<String>,
    /// `INDEX 00`, the start of the pregap, if present.
    pub pregap: Option<Frames>,
    /// `INDEX 01`, where the track proper starts.
    pub start: Frames,
}

/// A track together with where it ends inside its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment<'a> {
    pub track: &'a CueTrack,
    pub start: Frames,
    /// The next track's start, or `None` for the last track (it runs to the end of the file).
    pub end: Option<Frames>,
}

impl Segment<'_> {
    /// How long the track is. The last track has no next track, so it needs the
    /// total length of the file to find where it ends.
    pub fn length(&self, file_end: Frames) -> Frames {
        Frames(self.end.unwrap_or(file_end).0.saturating_sub(self.start.0))
    }
}

impl CueFile {
    /// Each track with its end position. A track ends where the next one begins.
    pub fn segments(&self) -> Vec<Segment<'_>> {
        self.tracks
            .iter()
            .enumerate()
            .map(|(index, track)| Segment {
                track,
                start: track.start,
                end: self.tracks.get(index + 1).map(|next| next.start),
            })
            .collect()
    }
}

/// Parses the text of a CUE sheet.
pub fn parse(text: &str) -> Result<CueSheet, CueError> {
    let mut parser = Parser::default();
    for (index, raw) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        parser.line(index + 1, raw)?;
    }
    parser.finish()
}

/// A track that is still being read. It becomes a [`CueTrack`] once its `INDEX 01` is known.
#[derive(Debug)]
struct Pending {
    line: usize,
    number: u32,
    title: Option<String>,
    performer: Option<String>,
    pregap: Option<Frames>,
    start: Option<Frames>,
}

impl Pending {
    fn finish(self) -> Result<CueTrack, CueError> {
        let start = self.start.ok_or(CueError::MissingIndex {
            line: self.line,
            track: self.number,
        })?;
        Ok(CueTrack {
            number: self.number,
            title: self.title,
            performer: self.performer,
            pregap: self.pregap,
            start,
        })
    }
}

#[derive(Debug, Default)]
struct Parser {
    sheet: CueSheet,
    /// True between a `TRACK` line and the next `TRACK` or `FILE`, audio or not.
    in_track: bool,
    /// The audio track being read. `None` while inside a data track.
    pending: Option<Pending>,
}

impl Parser {
    fn line(&mut self, line: usize, raw: &str) -> Result<(), CueError> {
        let tokens = tokenize(raw);
        let Some((keyword, args)) = tokens.split_first() else {
            return Ok(());
        };
        match keyword.to_ascii_uppercase().as_str() {
            "REM" => self.rem(args),
            "TITLE" => {
                let value = joined(args, line, "TITLE")?;
                match (self.in_track, self.pending.as_mut()) {
                    (false, _) => self.sheet.title = Some(value),
                    (true, Some(track)) => track.title = Some(value),
                    (true, None) => {}
                }
            }
            "PERFORMER" => {
                let value = joined(args, line, "PERFORMER")?;
                match (self.in_track, self.pending.as_mut()) {
                    (false, _) => self.sheet.performer = Some(value),
                    (true, Some(track)) => track.performer = Some(value),
                    (true, None) => {}
                }
            }
            "FILE" => self.file(args, line)?,
            "TRACK" => self.track(args, line)?,
            "INDEX" => self.index(args, line)?,
            _ => {}
        }
        Ok(())
    }

    fn rem(&mut self, args: &[String]) {
        if self.in_track {
            return;
        }
        let Some((key, rest)) = args.split_first() else {
            return;
        };
        if rest.is_empty() {
            return;
        }
        let value = rest.join(" ");
        match key.to_ascii_uppercase().as_str() {
            "GENRE" => self.sheet.genre = Some(value),
            "DATE" => self.sheet.date = Some(value),
            "DISCID" => self.sheet.disc_id = Some(value),
            "COMMENT" => self.sheet.comment = Some(value),
            _ => {}
        }
    }

    fn file(&mut self, args: &[String], line: usize) -> Result<(), CueError> {
        self.finish_pending()?;
        self.in_track = false;
        // `FILE "name with spaces" WAVE` arrives as two tokens. An unquoted name with spaces
        // arrives as several, so everything before the final type token is the name.
        let name = match args {
            [] => {
                return Err(CueError::MissingArgument {
                    line,
                    keyword: "FILE",
                });
            }
            [only] => only.clone(),
            [name @ .., _type] => name.join(" "),
        };
        self.sheet.files.push(CueFile {
            name,
            tracks: Vec::new(),
        });
        Ok(())
    }

    fn track(&mut self, args: &[String], line: usize) -> Result<(), CueError> {
        self.finish_pending()?;
        if self.sheet.files.is_empty() {
            return Err(CueError::TrackBeforeFile { line });
        }
        let number_text = args.first().ok_or(CueError::MissingArgument {
            line,
            keyword: "TRACK",
        })?;
        let number = number_text
            .parse::<u32>()
            .map_err(|_| CueError::InvalidNumber {
                line,
                text: number_text.clone(),
            })?;
        let is_audio = args
            .get(1)
            .is_some_and(|kind| kind.eq_ignore_ascii_case("AUDIO"));
        self.in_track = true;
        self.pending = is_audio.then_some(Pending {
            line,
            number,
            title: None,
            performer: None,
            pregap: None,
            start: None,
        });
        Ok(())
    }

    fn index(&mut self, args: &[String], line: usize) -> Result<(), CueError> {
        if !self.in_track {
            return Err(CueError::IndexOutsideTrack { line });
        }
        let [number_text, time_text, ..] = args else {
            return Err(CueError::MissingArgument {
                line,
                keyword: "INDEX",
            });
        };
        let number = number_text
            .parse::<u32>()
            .map_err(|_| CueError::InvalidNumber {
                line,
                text: number_text.clone(),
            })?;
        let time = Frames::parse(time_text).ok_or_else(|| CueError::InvalidTime {
            line,
            text: time_text.clone(),
        })?;
        if let Some(track) = self.pending.as_mut() {
            match number {
                0 => track.pregap = Some(time),
                1 => track.start = Some(time),
                _ => {}
            }
        }
        Ok(())
    }

    fn finish_pending(&mut self) -> Result<(), CueError> {
        if let Some(pending) = self.pending.take() {
            let track = pending.finish()?;
            if let Some(file) = self.sheet.files.last_mut() {
                file.tracks.push(track);
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<CueSheet, CueError> {
        self.finish_pending()?;
        if self.sheet.files.iter().all(|file| file.tracks.is_empty()) {
            return Err(CueError::NoTracks);
        }
        for file in &self.sheet.files {
            for pair in file.tracks.windows(2) {
                if let [previous, next] = pair
                    && next.start < previous.start
                {
                    return Err(CueError::OutOfOrder { track: next.number });
                }
            }
        }
        Ok(self.sheet)
    }
}

/// Joins all arguments with spaces, so an unquoted `PERFORMER Led Zeppelin` still works.
fn joined(args: &[String], line: usize, keyword: &'static str) -> Result<String, CueError> {
    if args.is_empty() {
        Err(CueError::MissingArgument { line, keyword })
    } else {
        Ok(args.join(" "))
    }
}

/// Splits a line on whitespace, keeping quoted text together and dropping the quotes.
fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_token = false;
    for ch in line.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    tokens.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                current.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const SAMPLE: &str = r#"REM GENRE Rock
REM DATE 1973
REM DISCID 1170216
PERFORMER "Led Zeppelin"
TITLE "Houses of the Holy"
FILE "LZ-HoTH.flac" WAVE
  TRACK 01 AUDIO
    TITLE "The Song Remains the Same"
    PERFORMER "Led Zeppelin"
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    TITLE "The Rain Song"
    INDEX 00 05:30:10
    INDEX 01 05:32:00
"#;

    #[test]
    fn frames_parse_and_display() {
        assert_eq!(Frames::parse("05:32:00"), Some(Frames(24_900)));
        assert_eq!(Frames::parse("00:00:74"), Some(Frames(74)));
        assert_eq!(Frames(24_900).to_string(), "05:32:00");
        assert!((Frames(150).as_seconds() - 2.0).abs() < 1e-12);
    }

    #[test]
    fn frames_reject_bad_input() {
        for bad in [
            "",
            "1:2",
            "00:60:00",
            "00:00:75",
            "a:b:c",
            "00:00:00:00",
            "-1:00:00",
        ] {
            assert_eq!(Frames::parse(bad), None, "{bad:?} should be rejected");
        }
    }

    #[test]
    fn parses_sheet_metadata() {
        let sheet = parse(SAMPLE).unwrap();
        assert_eq!(sheet.title.as_deref(), Some("Houses of the Holy"));
        assert_eq!(sheet.performer.as_deref(), Some("Led Zeppelin"));
        assert_eq!(sheet.genre.as_deref(), Some("Rock"));
        assert_eq!(sheet.date.as_deref(), Some("1973"));
        assert_eq!(sheet.disc_id.as_deref(), Some("1170216"));
        assert_eq!(sheet.files.len(), 1);
        assert_eq!(sheet.files[0].name, "LZ-HoTH.flac");
    }

    #[test]
    fn segments_end_where_the_next_track_starts() {
        let sheet = parse(SAMPLE).unwrap();
        let segments = sheet.files[0].segments();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start, Frames(0));
        assert_eq!(segments[0].end, Some(Frames(24_900)));
        assert_eq!(segments[1].end, None);
        assert_eq!(segments[1].track.pregap, Some(Frames(24_760)));
        assert_eq!(segments[1].track.title.as_deref(), Some("The Rain Song"));
    }

    #[test]
    fn handles_byte_order_mark_and_windows_line_endings() {
        let text = format!("\u{feff}{}", SAMPLE.replace('\n', "\r\n"));
        let sheet = parse(&text).unwrap();
        assert_eq!(sheet.files[0].tracks.len(), 2);
        assert_eq!(sheet.title.as_deref(), Some("Houses of the Holy"));
    }

    #[test]
    fn accepts_unquoted_names_with_spaces() {
        let text = "PERFORMER Led Zeppelin\nFILE Houses of the Holy.flac WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\n";
        let sheet = parse(text).unwrap();
        assert_eq!(sheet.performer.as_deref(), Some("Led Zeppelin"));
        assert_eq!(sheet.files[0].name, "Houses of the Holy.flac");
    }

    #[test]
    fn supports_one_file_per_track() {
        let text = "FILE \"01.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\nFILE \"02.wav\" WAVE\nTRACK 02 AUDIO\nINDEX 01 00:00:00\n";
        let sheet = parse(text).unwrap();
        assert_eq!(sheet.files.len(), 2);
        assert_eq!(sheet.files[1].tracks[0].number, 2);
        assert_eq!(sheet.files[0].segments()[0].end, None);
    }

    #[test]
    fn skips_data_tracks() {
        let text = "FILE \"disc.bin\" BINARY\nTRACK 01 MODE1/2352\nTITLE \"data\"\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nTITLE \"song\"\nINDEX 01 00:02:00\n";
        let sheet = parse(text).unwrap();
        assert_eq!(sheet.files[0].tracks.len(), 1);
        assert_eq!(sheet.files[0].tracks[0].title.as_deref(), Some("song"));
        assert_eq!(sheet.title, None);
    }

    #[test]
    fn reports_structural_errors() {
        assert_eq!(
            parse("TRACK 01 AUDIO\n"),
            Err(CueError::TrackBeforeFile { line: 1 })
        );
        assert_eq!(
            parse("FILE \"a.wav\" WAVE\nINDEX 01 00:00:00\n"),
            Err(CueError::IndexOutsideTrack { line: 2 })
        );
        assert_eq!(
            parse("FILE \"a.wav\" WAVE\nTRACK 01 AUDIO\nTITLE \"x\"\n"),
            Err(CueError::MissingIndex { line: 2, track: 1 })
        );
        assert_eq!(parse("FILE \"a.wav\" WAVE\n"), Err(CueError::NoTracks));
        assert_eq!(parse(""), Err(CueError::NoTracks));
    }

    #[test]
    fn reports_bad_values() {
        assert_eq!(
            parse("FILE \"a.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:61:00\n"),
            Err(CueError::InvalidTime {
                line: 3,
                text: "00:61:00".to_string()
            })
        );
        assert_eq!(
            parse("FILE \"a.wav\" WAVE\nTRACK xx AUDIO\n"),
            Err(CueError::InvalidNumber {
                line: 2,
                text: "xx".to_string()
            })
        );
        assert_eq!(
            parse("FILE\n"),
            Err(CueError::MissingArgument {
                line: 1,
                keyword: "FILE"
            })
        );
    }

    #[test]
    fn rejects_tracks_that_go_backwards() {
        let text = "FILE \"a.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 05:00:00\nTRACK 02 AUDIO\nINDEX 01 01:00:00\n";
        assert_eq!(parse(text), Err(CueError::OutOfOrder { track: 2 }));
    }

    proptest! {
        #[test]
        fn parsing_arbitrary_text_never_panics(text in ".*") {
            let _ = parse(&text);
        }

        #[test]
        fn valid_times_round_trip(minutes in 0u32..1000, seconds in 0u32..60, frames in 0u32..75) {
            let text = format!("{minutes:02}:{seconds:02}:{frames:02}");
            let parsed = Frames::parse(&text).unwrap();
            prop_assert_eq!(parsed.to_string(), text);
        }
    }
}
