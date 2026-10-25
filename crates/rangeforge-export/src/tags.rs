//! Tags written into each output file.

use rangeforge_core::cue::{CueSheet, CueTrack};

use crate::format::OutputFormat;

/// The tags for one output track. Anything left empty is simply not written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track: Option<u32>,
    pub track_total: Option<u32>,
    pub genre: Option<String>,
    pub date: Option<String>,
}

impl Tags {
    /// Tags for one track of a CUE album. A track's own performer wins over the album's.
    pub fn for_cue_track(sheet: &CueSheet, track: &CueTrack, total_tracks: usize) -> Self {
        Self {
            title: track.title.clone(),
            artist: track.performer.clone().or_else(|| sheet.performer.clone()),
            album: sheet.title.clone(),
            album_artist: sheet.performer.clone(),
            track: Some(track.number),
            track_total: u32::try_from(total_tracks).ok(),
            genre: sheet.genre.clone(),
            date: sheet.date.clone(),
        }
    }

    /// FFmpeg `-metadata key=value` arguments. Container formats name some tags differently:
    /// MP4 and WAV keep the track as `n/total`, while FLAC wants separate number and total
    /// fields and spells the album artist without an underscore.
    pub(crate) fn ffmpeg_args(&self, format: OutputFormat) -> Vec<String> {
        let mut pairs: Vec<(&str, String)> = Vec::new();
        let mut add = |key: &'static str, value: Option<String>| {
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                pairs.push((key, value));
            }
        };

        add("title", self.title.clone());
        add("artist", self.artist.clone());
        add("album", self.album.clone());
        if format.uses_vorbis_style_tags() {
            add("albumartist", self.album_artist.clone());
            add("tracknumber", self.track.map(|n| n.to_string()));
            add("tracktotal", self.track_total.map(|n| n.to_string()));
        } else {
            add("album_artist", self.album_artist.clone());
            add(
                "track",
                self.track.map(|number| match self.track_total {
                    Some(total) => format!("{number}/{total}"),
                    None => number.to_string(),
                }),
            );
        }
        add("genre", self.genre.clone());
        add("date", self.date.clone());

        pairs
            .into_iter()
            .flat_map(|(key, value)| ["-metadata".to_string(), format!("{key}={value}")])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{BitDepth, Codec};
    use rangeforge_core::cue;

    const SHEET: &str = "REM GENRE Rock\nREM DATE 1973\nPERFORMER \"Led Zeppelin\"\nTITLE \"Houses of the Holy\"\nFILE \"a.flac\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"The Song Remains the Same\"\n    PERFORMER \"Led Zeppelin\"\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    TITLE \"The Rain Song\"\n    INDEX 01 05:30:30\n";

    fn tags_for(index: usize) -> Tags {
        let sheet = cue::parse(SHEET).unwrap();
        let tracks = &sheet.files[0].tracks;
        Tags::for_cue_track(&sheet, &tracks[index], tracks.len())
    }

    #[test]
    fn a_cue_track_gets_album_and_track_tags() {
        let tags = tags_for(0);
        assert_eq!(tags.title.as_deref(), Some("The Song Remains the Same"));
        assert_eq!(tags.artist.as_deref(), Some("Led Zeppelin"));
        assert_eq!(tags.album.as_deref(), Some("Houses of the Holy"));
        assert_eq!(tags.album_artist.as_deref(), Some("Led Zeppelin"));
        assert_eq!(tags.track, Some(1));
        assert_eq!(tags.track_total, Some(2));
        assert_eq!(tags.genre.as_deref(), Some("Rock"));
        assert_eq!(tags.date.as_deref(), Some("1973"));
    }

    #[test]
    fn a_track_without_its_own_performer_uses_the_albums() {
        assert_eq!(tags_for(1).artist.as_deref(), Some("Led Zeppelin"));
    }

    #[test]
    fn mp4_style_arguments() {
        let args = tags_for(1).ffmpeg_args(OutputFormat::new(Codec::Alac, BitDepth::Source));
        let expected = [
            "-metadata",
            "title=The Rain Song",
            "-metadata",
            "artist=Led Zeppelin",
            "-metadata",
            "album=Houses of the Holy",
            "-metadata",
            "album_artist=Led Zeppelin",
            "-metadata",
            "track=2/2",
            "-metadata",
            "genre=Rock",
            "-metadata",
            "date=1973",
        ];
        assert_eq!(args, expected);
    }

    #[test]
    fn flac_style_arguments_split_the_track_number() {
        let args = tags_for(0).ffmpeg_args(OutputFormat::new(Codec::Flac, BitDepth::Source));
        assert!(args.contains(&"tracknumber=1".to_string()), "{args:?}");
        assert!(args.contains(&"tracktotal=2".to_string()), "{args:?}");
        assert!(
            args.contains(&"albumartist=Led Zeppelin".to_string()),
            "{args:?}"
        );
        assert!(!args.iter().any(|a| a.starts_with("track=")), "{args:?}");
    }

    #[test]
    fn empty_tags_are_left_out() {
        assert_eq!(
            Tags::default().ffmpeg_args(OutputFormat::new(Codec::Alac, BitDepth::Source)),
            Vec::<String>::new()
        );
        let tags = Tags {
            title: Some(String::new()),
            artist: Some("Someone".to_string()),
            ..Tags::default()
        };
        assert_eq!(
            tags.ffmpeg_args(OutputFormat::new(Codec::Wav, BitDepth::Source)),
            ["-metadata", "artist=Someone"]
        );
    }

    #[test]
    fn a_track_number_without_a_total() {
        let tags = Tags {
            track: Some(7),
            ..Tags::default()
        };
        assert_eq!(
            tags.ffmpeg_args(OutputFormat::new(Codec::Alac, BitDepth::Source)),
            ["-metadata", "track=7"]
        );
    }
}
