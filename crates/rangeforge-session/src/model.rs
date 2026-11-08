//! The state behind the interactive view.
//!
//! Nothing here draws, reads files or spawns threads, so all of it can be tested directly.
//! When the user asks to run, the model builds a [`RunSpec`] and leaves it for the caller to
//! start; progress comes back as [`RunEvent`]s.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rangeforge_core::identify::Identity;
use rangeforge_core::planner::{self, GainPlan, Measurement, PlanError, Target};
use rangeforge_export::format::{BitDepth, Codec, Container, OutputFormat, SampleRate};
use rangeforge_export::pipeline::TrackPlan;
use rangeforge_export::strategy::Strategy;
use rangeforge_lookup::billboard::Hot100;
use rangeforge_lookup::lastfm::{self, RawTag};
use rangeforge_lookup::tagcache::Subject;

use crate::appsettings::{MetadataSettings, SavedSettings};
use crate::controls::{
    ConflictKind, Focus, Mode, OutputSettings, PresetList, Saved, StrategySettings, WhereKind,
    apply_preset,
};
use crate::dsd::DsdFormat;
use crate::genres::{self, Answers, SongGenres, TagEdits};
use crate::load::{Job, load_from};
use crate::naming::NameSettings;
use crate::plan::{plan_tracks, track_plans};
use crate::secrets::KeySource;

/// A key press, in terms the screens share. The terminal view translates real key codes into
/// these. The desktop app does not use keys at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Tab,
    BackTab,
    Enter,
    Esc,
    Backspace,
}

const TARGET_STEP: f64 = 0.5;
const CEILING_STEP: f64 = 0.1;
const TARGET_LIMITS: (f64, f64) = (-60.0, 0.0);
const CEILING_LIMITS: (f64, f64) = (-6.0, 0.0);
const PAGE: usize = 10;

fn tenths(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// The volume settings the user can change.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub target_lufs: f64,
    pub peak_protection: bool,
    pub peak_ceiling_dbtp: f64,
}

impl Default for Settings {
    /// The defaults shown in the mockup.
    fn default() -> Self {
        Self {
            target_lufs: -22.0,
            peak_protection: true,
            peak_ceiling_dbtp: -0.6,
        }
    }
}

impl Settings {
    /// Builds settings, keeping values inside their allowed ranges.
    pub fn new(target_lufs: f64, peak_protection: bool, peak_ceiling_dbtp: f64) -> Self {
        Self {
            target_lufs: tenths(target_lufs).clamp(TARGET_LIMITS.0, TARGET_LIMITS.1),
            peak_protection,
            peak_ceiling_dbtp: tenths(peak_ceiling_dbtp).clamp(CEILING_LIMITS.0, CEILING_LIMITS.1),
        }
    }

    /// What the planner needs.
    pub fn target(&self) -> Target {
        Target {
            loudness_lufs: self.target_lufs,
            peak_ceiling_dbtp: self.peak_protection.then_some(self.peak_ceiling_dbtp),
        }
    }

    /// Moves the target loudness by `steps` of half a LU.
    pub fn nudge_target(&mut self, steps: i32) {
        self.target_lufs = tenths(self.target_lufs + f64::from(steps) * TARGET_STEP)
            .clamp(TARGET_LIMITS.0, TARGET_LIMITS.1);
    }

    /// Moves the peak ceiling by `steps` of a tenth of a dB.
    pub fn nudge_ceiling(&mut self, steps: i32) {
        self.peak_ceiling_dbtp = tenths(self.peak_ceiling_dbtp + f64::from(steps) * CEILING_STEP)
            .clamp(CEILING_LIMITS.0, CEILING_LIMITS.1);
    }
}

/// Where a finished measurement came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Cache,
    Decoded,
}

/// How far along measuring a file is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Pending,
    Done(Origin),
    Failed(String),
}

/// How far along writing a file's output is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RunState {
    #[default]
    Idle,
    /// Part of a run that has started but not reached this file yet.
    Queued,
    /// Song `index` (counting from 0) of `total` is being encoded.
    Encoding {
        index: usize,
        total: usize,
    },
    Saved,
    Failed(String),
}

/// What kind of row group this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    CueAlbum,
    SingleFile,
}

/// One song.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackRow {
    pub label: String,
    pub measurement: Option<Measurement>,
    /// Its output file has been written.
    pub saved: bool,
    /// Part of the next run. Songs the user switches off are left out.
    pub included: bool,
    /// The song reached the Billboard Hot 100 (the blue "Radio" tag), once looked up.
    pub radio: Option<RadioHit>,
    /// Who the song is: from the CUE sheet, the file's tags, or a guess from its name. What
    /// every lookup works from.
    pub identity: Option<Identity>,
    /// Tags the user added or took away by hand. Kept for this session.
    pub edits: TagEdits,
}

/// What a change from the window applies to: one song, or with no song, every song of a group.
/// A window sends a list of these, so one action can work on a selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub struct Pick {
    pub group: usize,
    pub track: Option<usize>,
}

/// Names typed in by hand for a song or an album. A blank field means "not known".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Names {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub album_artist: String,
}

/// How a song did on the Billboard Hot 100.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadioHit {
    /// The best position it reached: 1 is the top.
    pub peak: u16,
    /// The year it first charted.
    pub year: u16,
}

impl TrackRow {
    /// A song that is waiting to be measured and will be part of the run.
    pub fn new(label: String) -> Self {
        Self {
            label,
            measurement: None,
            saved: false,
            included: true,
            radio: None,
            identity: None,
            edits: TagEdits::default(),
        }
    }
}

/// An album, or a single file, with its songs.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub name: String,
    pub kind: Kind,
    /// The audio file's extension, such as `flac`.
    pub source_extension: String,
    pub tracks: Vec<TrackRow>,
    pub status: Status,
    pub run: RunState,
}

impl Group {
    /// The Codec column text for the parent row.
    pub fn codec_label(&self) -> String {
        match self.kind {
            Kind::CueAlbum => "Split(cue)".to_string(),
            Kind::SingleFile => format!("({})", self.source_extension),
        }
    }

    /// Rows this group takes in the table: itself, its songs, and a line for each failure.
    fn row_count(&self) -> usize {
        let measuring_message = usize::from(matches!(self.status, Status::Failed(_)));
        let run_message = usize::from(matches!(self.run, RunState::Failed(_)));
        1 + measuring_message + run_message + self.tracks.len()
    }
}

/// A finished measurement job, sent back from a worker thread.
#[derive(Debug)]
pub struct MeasureOutcome {
    pub group: usize,
    pub result: Result<(Vec<Measurement>, Origin), String>,
    pub elapsed: Duration,
    /// For a single file, who its song is, read from the file's tags. `None` for CUE albums,
    /// which are identified from their sheet.
    pub identity: Option<Identity>,
}

/// Progress of a run, sent back from the worker that is writing the files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEvent {
    Encoding {
        group: usize,
        index: usize,
        total: usize,
    },
    Saved {
        group: usize,
        index: usize,
        path: PathBuf,
    },
    /// The original was deleted, as the File Strategy asked.
    RemovedOriginal {
        group: usize,
        path: PathBuf,
    },
    Finished {
        group: usize,
    },
    Failed {
        group: usize,
        message: String,
    },
    /// Every file has been dealt with.
    AllDone,
}

/// One file to write: what to make of it, ready to hand to the exporter.
#[derive(Debug)]
pub struct RunItem {
    pub group: usize,
    pub job: Job,
    pub plans: Vec<TrackPlan>,
    /// What to write this file as: the main Output Format, with the DSD Format filled in for a
    /// DSD source.
    pub format: OutputFormat,
    /// The folder the source lives in.
    pub directory: PathBuf,
}

/// Everything a run needs, fixed at the moment the user asked for it.
#[derive(Debug)]
pub struct RunSpec {
    pub items: Vec<RunItem>,
    pub format: OutputFormat,
    pub strategy: Strategy,
}

/// What happened when a window asked to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunRequest {
    /// The run is prepared and waiting for the caller to start it.
    Started,
    /// The File Strategy overwrites files, so the window must ask the user and call again.
    NeedsConfirmation,
    /// It cannot run, for this plain-words reason.
    Refused(String),
}

// The screen's state is a flat set of independent switches, so the count of booleans is fine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
pub struct App {
    pub groups: Vec<Group>,
    pub jobs: Vec<Job>,
    pub settings: Settings,
    pub output: OutputSettings,
    pub strategy: StrategySettings,
    /// The file name patterns, kept between sessions.
    pub naming: NameSettings,
    pub presets: PresetList,
    pub focus: Focus,
    pub mode: Mode,
    pub show_log: bool,
    pub log: Vec<String>,
    /// The first table row shown.
    pub scroll: usize,
    pub should_quit: bool,
    /// A run is in progress.
    pub running: bool,
    /// A run the user has asked for, waiting for the caller to start it.
    pending_run: Option<RunSpec>,
    /// The presets changed and should be saved to disk.
    presets_changed: bool,
    /// The saved settings changed and should be written to disk.
    settings_changed: bool,
    /// Which outside lookups are switched on.
    pub metadata: MetadataSettings,
    /// What DSD files are written as.
    pub dsd: DsdFormat,
    /// Ask before a run that overwrites files or deletes originals.
    pub confirm_destructive: bool,
    /// The settings as last saved or loaded, to notice any change however it was made.
    saved_baseline: SavedSettings,
    /// The Hot 100 index, once it has been loaded.
    radio: Option<Arc<Hot100>>,
    /// Whether a Last.fm key is available, and from where. The key itself is never held here.
    pub lastfm_key: KeySource,
    /// What Last.fm has said about albums and songs so far.
    answers: Answers,
    /// Questions already handed out to be asked, so none is asked twice at once.
    asked: HashSet<String>,
    /// Questions the user asked for by hand (right-click). Asked even while Last.fm lookups are
    /// switched off, and asked afresh rather than answered from the saved copy.
    requested: HashMap<String, Subject>,
}

impl App {
    pub fn new(groups: Vec<Group>, jobs: Vec<Job>, log: Vec<String>, settings: Settings) -> Self {
        Self {
            groups,
            jobs,
            settings,
            output: OutputSettings::default(),
            strategy: StrategySettings::default(),
            naming: NameSettings::default(),
            presets: PresetList::new(Vec::new()),
            focus: Focus::Target,
            mode: Mode::Normal,
            show_log: false,
            log,
            scroll: 0,
            should_quit: false,
            running: false,
            pending_run: None,
            presets_changed: false,
            settings_changed: false,
            metadata: MetadataSettings::default(),
            dsd: DsdFormat::default(),
            confirm_destructive: true,
            saved_baseline: SavedSettings::default(),
            radio: None,
            lastfm_key: KeySource::None,
            answers: Answers::default(),
            asked: HashSet::new(),
            requested: HashMap::new(),
        }
    }

    /// The settings that are kept between sessions.
    pub fn saved_settings(&self) -> SavedSettings {
        SavedSettings {
            naming: self.naming.clone(),
            metadata: self.metadata,
            strategy: self.strategy.clone(),
            confirm_destructive: self.confirm_destructive,
            dsd: self.dsd,
        }
    }

    /// Applies settings read from disk at start-up.
    pub fn apply_saved_settings(&mut self, saved: SavedSettings) {
        self.naming = saved.naming;
        self.strategy = saved.strategy.clone();
        self.confirm_destructive = saved.confirm_destructive;
        self.dsd = saved.dsd;
        self.metadata = saved.metadata;
        self.saved_baseline = self.saved_settings();
    }

    /// Switches outside lookups on or off. Off hides the Radio tag too, and nothing is
    /// fetched until it is on again.
    pub fn set_metadata_enabled(&mut self, on: bool) {
        if self.metadata.enabled != on {
            self.metadata.enabled = on;
            self.settings_changed = true;
            self.refresh_radio();
            self.log(if on {
                "Metadata lookups are on"
            } else {
                "Metadata lookups are off"
            });
        }
    }

    /// Switches Last.fm on or off. It also needs a key, and the Metadata switch.
    pub fn set_lastfm_enabled(&mut self, on: bool) {
        if self.metadata.lastfm_enabled != on {
            self.metadata.lastfm_enabled = on;
            self.settings_changed = true;
        }
    }

    /// Turns the question before an overwriting or deleting run on or off. Off is what "don't
    /// ask again" does, and Settings turns it back on.
    pub fn set_confirm_destructive(&mut self, on: bool) {
        if self.confirm_destructive != on {
            self.confirm_destructive = on;
            self.log(if on {
                "Will ask before overwriting files or deleting originals"
            } else {
                "Will not ask before overwriting files or deleting originals"
            });
        }
    }

    /// Chooses the codec DSD files are written in, or `None` to follow the Output Format.
    pub fn set_dsd_codec(&mut self, codec: Option<Codec>) {
        self.dsd.codec = codec;
    }

    /// Likewise for the container.
    pub fn set_dsd_container(&mut self, container: Option<Container>) {
        self.dsd.container = container;
    }

    /// Chooses the sample rate DSD files get when the Output Format says original.
    pub fn set_dsd_sample_rate(&mut self, rate: SampleRate) -> Result<(), String> {
        self.dsd.set_sample_rate(rate)
    }

    /// Chooses the bit depth DSD files get when the Output Format says source.
    pub fn set_dsd_bit_depth(&mut self, depth: BitDepth) -> Result<(), String> {
        self.dsd.set_bit_depth(depth)
    }

    /// Whether Last.fm should be asked for genres: switched on, with a key to use.
    pub fn wants_lastfm(&self) -> bool {
        self.metadata.enabled && self.metadata.lastfm_enabled && self.lastfm_key != KeySource::None
    }

    /// The genres for a song, as shown and as written. Empty while lookups are off, so nothing
    /// shows and nothing in a file is changed.
    pub fn song_genres(&self, track: &TrackRow) -> SongGenres {
        if !self.metadata.enabled {
            return SongGenres::default();
        }
        let found = match &track.identity {
            Some(identity) => genres::song_genres(identity, &self.answers),
            None => SongGenres::default(),
        };
        found.with_edits(&track.edits)
    }

    /// Whether the song carries the Radio tag: it charted, and nobody took the tag away.
    pub fn shows_radio(&self, track: &TrackRow) -> bool {
        track.radio.is_some() && !track.edits.hides("Radio")
    }

    /// The text to write in a song's genre tag, or `None` to leave the tag as it is. If every
    /// tag was taken away by hand, the tag is written empty rather than left as it was.
    pub fn genre_tag(&self, track: &TrackRow) -> Option<String> {
        if !self.metadata.enabled {
            return None;
        }
        genres::genre_tag(&self.song_genres(track), self.shows_radio(track))
            .or_else(|| (!track.edits.removed.is_empty()).then(String::new))
    }

    /// Adds tags by hand to every song the targets name. `text` can hold several tags, separated
    /// by commas, semicolons or line breaks (see [`genres::split_genres`]). Returns how many
    /// different tags were added.
    pub fn add_tag(&mut self, targets: &[Pick], text: &str) -> Result<usize, String> {
        let names = genres::split_genres(text);
        if names.is_empty() {
            return Err("Type the tag first.".to_string());
        }
        for row in self.rows_for(targets)? {
            for name in &names {
                row.edits.add(name);
            }
        }
        Ok(names.len())
    }

    /// Takes a tag away from one song, by hand. It stays away when Last.fm is asked again.
    pub fn remove_tag(&mut self, group: usize, track: usize, text: &str) -> Result<(), String> {
        let target = Pick {
            group,
            track: Some(track),
        };
        for row in self.rows_for(&[target])? {
            row.edits.remove(text);
        }
        Ok(())
    }

    /// The songs a change applies to, each once: the ones named, or all of a group's songs for
    /// a target with no song.
    fn rows_for(&mut self, targets: &[Pick]) -> Result<Vec<&mut TrackRow>, String> {
        let mut wanted: HashSet<(usize, usize)> = HashSet::new();
        for target in targets {
            let group = self
                .groups
                .get(target.group)
                .ok_or("That album is not in the list.")?;
            match target.track {
                Some(index) if index < group.tracks.len() => {
                    wanted.insert((target.group, index));
                }
                Some(_) => return Err("That song is not in the list.".to_string()),
                None => wanted.extend((0..group.tracks.len()).map(|index| (target.group, index))),
            }
        }
        let wanted = &wanted;
        Ok(self
            .groups
            .iter_mut()
            .enumerate()
            .flat_map(|(group, rows)| {
                rows.tracks
                    .iter_mut()
                    .enumerate()
                    .filter(move |(index, _)| wanted.contains(&(group, *index)))
                    .map(|(_, row)| row)
            })
            .collect())
    }

    /// Types in who songs are. For songs, sets artist, title, album and album artist; for a
    /// whole album (targets with no song), only the album and album artist of every song in it.
    /// A blank field clears it, unless `only_filled`, which leaves fields alone that were left
    /// blank: for changing the album of many songs without touching each one's title. What was
    /// typed replaces whatever was known, and nothing replaces it later.
    pub fn set_names(
        &mut self,
        targets: &[Pick],
        names: &Names,
        only_filled: bool,
    ) -> Result<(), String> {
        let clean = |text: &str| Some(text.trim().to_string()).filter(|t| !t.is_empty());
        let (artist, title) = (clean(&names.artist), clean(&names.title));
        let (album, album_artist) = (clean(&names.album), clean(&names.album_artist));
        let whole_album = targets.iter().all(|target| target.track.is_none());
        let set = |slot: &mut Option<String>, value: &Option<String>| {
            if !(only_filled && value.is_none()) {
                slot.clone_from(value);
            }
        };
        for row in self.rows_for(targets)? {
            let identity = row.identity.get_or_insert_with(|| {
                Identity::unknown(rangeforge_core::identify::IdentitySource::Hand)
            });
            let before = identity.clone();
            if !whole_album {
                set(&mut identity.artist, &artist);
                set(&mut identity.title, &title);
            }
            set(&mut identity.album, &album);
            set(&mut identity.album_artist, &album_artist);
            if *identity != before {
                identity.source = rangeforge_core::identify::IdentitySource::Hand;
            }
        }
        self.refresh_radio();
        Ok(())
    }

    /// The questions for Last.fm that nobody has been sent to ask yet, once. Empty unless
    /// Last.fm is switched on and has a key. Each comes back through [`App::apply_answer`].
    pub fn take_questions(&mut self) -> Vec<Subject> {
        let mut subjects: Vec<Subject> = self.requested.values().cloned().collect();
        if self.wants_lastfm() {
            subjects.extend(self.askable().flat_map(genres::questions));
        }
        let mut wanted = Vec::new();
        for subject in subjects {
            if self.answers.get(&subject).is_none() && self.asked.insert(subject.key()) {
                wanted.push(subject);
            }
        }
        wanted
    }

    /// True if there is something to ask Last.fm that has not been handed out.
    pub fn has_questions(&self) -> bool {
        let waiting = |subject: &Subject| {
            self.answers.get(subject).is_none() && !self.asked.contains(&subject.key())
        };
        self.requested.values().any(waiting)
            || (self.wants_lastfm()
                && self
                    .askable()
                    .flat_map(genres::questions)
                    .any(|s| waiting(&s)))
    }

    /// Whether this question was asked for by hand, so the saved answer should not be used.
    pub fn was_requested(&self, subject: &Subject) -> bool {
        self.requested.contains_key(&subject.key())
    }

    /// Asks Last.fm, by hand, about the albums of the songs the targets name. Works even while
    /// automatic lookups are off, but needs a key and the main metadata switch. Albums asked
    /// about again are asked afresh. (Songs are not asked: Last.fm's API has no song tags, see
    /// [`App::set_song_tags`].)
    pub fn request_lastfm(&mut self, targets: &[Pick]) -> Result<(), String> {
        if !self.metadata.enabled {
            return Err("Metadata lookups are switched off. Turn them on in Settings.".to_string());
        }
        if self.lastfm_key == KeySource::None {
            return Err("There is no Last.fm key yet. Add one in Settings.".to_string());
        }
        let mut subjects = Vec::new();
        for row in self.rows_for(targets)? {
            if let Some(identity) = row.identity.as_ref()
                && let Some(subject) = genres::album_subject(identity)
                && !subjects.contains(&subject)
            {
                subjects.push(subject);
            }
        }
        if subjects.is_empty() {
            return Err(
                "Last.fm needs an album's artist and name, and they are not known.".to_string(),
            );
        }
        for subject in subjects {
            let key = subject.key();
            self.answers.remove(&subject);
            self.asked.remove(&key);
            self.requested.insert(key, subject);
        }
        Ok(())
    }

    /// The address of a song's Last.fm tags page, for the person to open in a browser.
    pub fn song_page_url(&self, group: usize, track: usize) -> Result<String, String> {
        let identity = self
            .groups
            .get(group)
            .and_then(|g| g.tracks.get(track))
            .ok_or("That song is not in the list.")?
            .identity
            .as_ref();
        match identity.and_then(genres::track_subject) {
            Some(Subject::Track { artist, title }) => Ok(lastfm::song_page_url(&artist, &title)),
            _ => Err(
                "Opening the page needs the song's artist and title, and they are not known."
                    .to_string(),
            ),
        }
    }

    /// The identities worth asking Last.fm about. A single file waits until its tags have been
    /// read, which happens when it has been measured: asking about a guess from its file name
    /// would often be a wasted question.
    fn askable(&self) -> impl Iterator<Item = &Identity> {
        self.groups
            .iter()
            .filter(|group| {
                group.kind == Kind::CueAlbum || !matches!(group.status, Status::Pending)
            })
            .flat_map(|group| &group.tracks)
            .filter_map(|track| track.identity.as_ref())
    }

    /// Records what Last.fm said about an album or song.
    pub fn apply_answer(&mut self, subject: &Subject, tags: Vec<RawTag>) {
        self.answers.insert(subject, tags);
        self.requested.remove(&subject.key());
    }

    /// Lets every question not yet answered be asked again, such as after the lookups stopped
    /// early because the network went away.
    pub fn forget_asked(&mut self) {
        self.asked.clear();
    }

    /// Throws away what Last.fm has said so far, so every question can be asked again. Tags
    /// already in files, and edits made by hand, are not touched.
    pub fn forget_lastfm_answers(&mut self) {
        self.answers.clear();
        self.asked.clear();
    }

    /// How many albums and songs Last.fm has answered for.
    pub fn answered(&self) -> usize {
        self.answers.len()
    }

    /// Gives the app the Hot 100 index (or takes it away) and marks the songs that charted.
    pub fn set_radio(&mut self, index: Option<Arc<Hot100>>) {
        self.radio = index;
        self.refresh_radio();
    }

    /// How many songs the loaded Hot 100 index holds, or `None` if it is not loaded.
    pub fn radio_songs(&self) -> Option<usize> {
        self.radio.as_ref().map(|index| index.len())
    }

    /// Marks each song that reached the Hot 100, or clears the marks when lookups are off or
    /// there is no index yet. A song is looked up by its identity, so it needs an artist and a
    /// title from somewhere: its CUE sheet, its file's tags, or its file name.
    fn refresh_radio(&mut self) {
        let index = self.radio.clone().filter(|_| self.metadata.enabled);
        for track in self.groups.iter_mut().flat_map(|group| &mut group.tracks) {
            track.radio = index.as_ref().and_then(|index| {
                let identity = track.identity.as_ref()?;
                let song = index.find(identity.artist.as_deref()?, identity.title.as_deref()?)?;
                Some(RadioHit {
                    peak: song.peak,
                    year: song.first_year,
                })
            });
        }
    }

    /// Files still being measured.
    pub fn pending(&self) -> usize {
        self.groups
            .iter()
            .filter(|group| group.status == Status::Pending)
            .count()
    }

    /// Songs that a run would process: everything switched on, except files that failed.
    pub fn queued(&self) -> usize {
        self.groups
            .iter()
            .filter(|group| !matches!(group.status, Status::Failed(_)))
            .flat_map(|group| &group.tracks)
            .filter(|track| track.included)
            .count()
    }

    /// Total table rows.
    pub fn row_count(&self) -> usize {
        self.groups.iter().map(Group::row_count).sum()
    }

    /// The volume change for a song under the current settings, once it is measured.
    pub fn plan(&self, track: &TrackRow) -> Option<Result<GainPlan, PlanError>> {
        track
            .measurement
            .map(|measurement| planner::plan_gain(measurement, self.settings.target()))
    }

    /// True if peak protection reduced the gain of any song.
    pub fn any_limited(&self) -> bool {
        self.groups
            .iter()
            .flat_map(|group| &group.tracks)
            .filter_map(|track| self.plan(track))
            .any(|plan| plan.is_ok_and(|plan| plan.limited_by_peak))
    }

    /// True if the settings have been changed from the selected preset.
    pub fn preset_modified(&self) -> bool {
        self.presets.is_modified(&self.settings, self.output)
    }

    /// True when there is something worth saving as a preset: the settings match no preset
    /// yet, or have been changed from the selected one. The mockup greys Save out otherwise.
    pub fn can_save_preset(&self) -> bool {
        self.presets.selected.is_none() || self.preset_modified()
    }

    /// Adds a line to the log.
    pub fn log(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
    }

    /// Gives up on a run that was asked for but could not start, such as when FFmpeg is
    /// missing, and puts the rows back as they were.
    pub fn abort_run(&mut self, reason: &str) {
        self.running = false;
        for group in &mut self.groups {
            if group.run == RunState::Queued {
                group.run = RunState::Idle;
            }
        }
        self.log(format!("Cannot run: {reason}"));
    }

    /// Records a finished measurement.
    pub fn apply(&mut self, outcome: MeasureOutcome) {
        let Some(group) = self.groups.get_mut(outcome.group) else {
            return;
        };
        let name = group.name.clone();
        // A single file's tags arrive with its measurement.
        if group.kind == Kind::SingleFile
            && let (Some(identity), Some(track)) = (outcome.identity, group.tracks.first_mut())
            && track.identity.as_ref().map(|known| known.source)
                != Some(rangeforge_core::identify::IdentitySource::Hand)
        {
            track.identity = Some(identity);
        }
        let line = match outcome.result {
            Ok((measurements, origin)) if measurements.len() == group.tracks.len() => {
                for (track, measurement) in group.tracks.iter_mut().zip(measurements) {
                    track.measurement = Some(measurement);
                }
                group.status = Status::Done(origin);
                match origin {
                    Origin::Cache => format!("Loudness scan: {name} (cached)"),
                    Origin::Decoded => format!(
                        "Loudness scan: {name} ({:.1} s)",
                        outcome.elapsed.as_secs_f64()
                    ),
                }
            }
            Ok((measurements, _)) => {
                let message = format!(
                    "measured {} track(s) but expected {}",
                    measurements.len(),
                    group.tracks.len()
                );
                group.status = Status::Failed(message.clone());
                format!("Failed: {name}: {message}")
            }
            Err(message) => {
                group.status = Status::Failed(message.clone());
                format!("Failed: {name}: {message}")
            }
        };
        self.log(line);
        self.refresh_radio();
    }

    /// Records progress of a run.
    pub fn apply_run_event(&mut self, event: RunEvent) {
        match event {
            RunEvent::Encoding {
                group,
                index,
                total,
            } => {
                if let Some(group) = self.groups.get_mut(group) {
                    group.run = RunState::Encoding { index, total };
                }
            }
            RunEvent::Saved { group, index, path } => {
                if let Some(track) = self
                    .groups
                    .get_mut(group)
                    .and_then(|group| group.tracks.get_mut(index))
                {
                    track.saved = true;
                }
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                self.log(format!("Saved {name}"));
            }
            RunEvent::RemovedOriginal { path, .. } => {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                self.log(format!("Deleted original {name}"));
            }
            RunEvent::Finished { group } => {
                if let Some(group) = self.groups.get_mut(group) {
                    group.run = RunState::Saved;
                    let line = format!("Finished {}", group.name);
                    self.log(line);
                }
            }
            RunEvent::Failed { group, message } => {
                if let Some(group) = self.groups.get_mut(group) {
                    group.run = RunState::Failed(message.clone());
                    let line = format!("Failed to write {}: {message}", group.name);
                    self.log(line);
                }
            }
            RunEvent::AllDone => {
                self.running = false;
                let saved = self
                    .groups
                    .iter()
                    .filter(|group| group.run == RunState::Saved)
                    .count();
                let failed = self
                    .groups
                    .iter()
                    .filter(|group| matches!(group.run, RunState::Failed(_)))
                    .count();
                self.log(format!(
                    "Run complete: {saved} file(s) written, {failed} failed"
                ));
            }
        }
    }

    /// Takes the run the user asked for, if any. The caller starts it.
    pub fn take_run_spec(&mut self) -> Option<RunSpec> {
        self.pending_run.take()
    }

    /// True once after the presets change, so the caller can save them.
    pub fn take_presets_changed(&mut self) -> bool {
        std::mem::take(&mut self.presets_changed)
    }

    /// True once after the saved settings change, so the caller can save them.
    pub fn take_settings_changed(&mut self) -> bool {
        let flagged = std::mem::take(&mut self.settings_changed);
        // Comparing with what was last saved catches every change, whichever control made it.
        let current = self.saved_settings();
        let changed = flagged || current != self.saved_baseline;
        if changed {
            self.saved_baseline = current;
        }
        changed
    }

    /// Sets the file name patterns, or says what is wrong with them and changes nothing.
    pub fn set_naming(&mut self, album: &str, single: &str) -> Result<(), String> {
        let naming = NameSettings {
            album: album.trim().to_string(),
            single: single.trim().to_string(),
        };
        naming.templates()?;
        if naming != self.naming {
            self.naming = naming;
            self.settings_changed = true;
            self.log("Changed the file name patterns");
        }
        Ok(())
    }

    /// Builds the run from what is on screen now, or says why it cannot be done.
    fn build_run(&self) -> Result<RunSpec, String> {
        let strategy = self.strategy.to_strategy()?;
        let names = self.naming.templates()?;
        let format = self.output.format();
        let mut items = Vec::new();
        for job in &self.jobs {
            let Some(group) = self.groups.get(job.group) else {
                continue;
            };
            if matches!(group.status, Status::Failed(_)) {
                continue;
            }
            let measurements: Option<Vec<Measurement>> =
                group.tracks.iter().map(|track| track.measurement).collect();
            let Some(measurements) = measurements else {
                continue;
            };
            if !group.tracks.iter().any(|track| track.included) {
                continue;
            }
            let planned = plan_tracks(job, &measurements, self.settings.target());
            let identities: Vec<Option<Identity>> = group
                .tracks
                .iter()
                .map(|track| track.identity.clone())
                .collect();
            let job_format = self.dsd.format_for(format, &job.audio);
            let (mut plans, directory) =
                track_plans(job, &planned, job_format, &names, &identities);
            for (plan, track) in plans.iter_mut().zip(&group.tracks) {
                plan.skip = !track.included;
                // Genres found by lookups are written, comma separated, with Radio last.
                if let Some(genre) = self.genre_tag(track) {
                    plan.tags.genre = Some(genre);
                }
            }
            items.push(RunItem {
                group: job.group,
                job: job.clone(),
                plans,
                format: job_format,
                directory,
            });
        }
        if items.is_empty() {
            return Err("there is nothing to write".to_string());
        }
        Ok(RunSpec {
            items,
            format,
            strategy,
        })
    }

    /// Why a run cannot start right now, in plain words, or `None` if it can.
    pub fn run_blocker(&self) -> Option<String> {
        if self.running {
            Some("A run is already in progress".to_string())
        } else if self.pending() > 0 {
            Some("Measuring is not finished yet".to_string())
        } else if self.queued() == 0 {
            Some("Nothing to run".to_string())
        } else {
            self.build_run()
                .err()
                .map(|reason| format!("Cannot run: {reason}"))
        }
    }

    /// Whether Run should ask first: the File Strategy overwrites or deletes, and the user has
    /// not turned the question off.
    pub fn asks_before_run(&self) -> bool {
        self.strategy.needs_confirmation() && self.confirm_destructive
    }

    /// The user pressed Run. Checks that it makes sense, asks for confirmation if the
    /// strategy overwrites files, and otherwise prepares the run.
    fn request_run(&mut self) {
        if let Some(reason) = self.run_blocker() {
            self.log(reason);
        } else if self.asks_before_run() {
            self.mode = Mode::ConfirmRun;
        } else {
            self.start_run();
        }
    }

    /// A window asked to run. Unlike the keyboard, a window asks for the confirmation itself
    /// and calls again with `confirmed` set.
    pub fn begin_run(&mut self, confirmed: bool) -> RunRequest {
        if let Some(reason) = self.run_blocker() {
            self.log(reason.clone());
            return RunRequest::Refused(reason);
        }
        if self.asks_before_run() && !confirmed {
            return RunRequest::NeedsConfirmation;
        }
        self.start_run();
        if self.pending_run.is_some() {
            RunRequest::Started
        } else {
            RunRequest::Refused(self.log.last().cloned().unwrap_or_default())
        }
    }

    /// Sets the target loudness, kept inside its allowed range.
    pub fn set_target(&mut self, lufs: f64) {
        self.settings = Settings::new(
            lufs,
            self.settings.peak_protection,
            self.settings.peak_ceiling_dbtp,
        );
    }

    /// Sets the peak ceiling, kept inside its allowed range.
    pub fn set_ceiling(&mut self, dbtp: f64) {
        self.settings = Settings::new(
            self.settings.target_lufs,
            self.settings.peak_protection,
            dbtp,
        );
    }

    pub fn set_peak_protection(&mut self, on: bool) {
        self.settings.peak_protection = on;
    }

    /// Sets the codec. A sample rate the new codec cannot write (192 kHz AAC) goes back to the
    /// source's own, with a line in the log saying so.
    pub fn set_codec(&mut self, codec: Codec) {
        self.output.codec = codec;
        if self.output.fit_to_codec() {
            self.log(format!(
                "{} cannot be written at that sample rate, so it is back to original",
                codec.name()
            ));
        }
    }

    pub fn set_bit_depth(&mut self, depth: BitDepth) {
        self.output.bit_depth = depth;
    }

    pub fn set_container(&mut self, container: Container) {
        self.output.container = container;
    }

    /// Sets the sample rate, or says why the codec cannot write it.
    pub fn set_sample_rate(&mut self, rate: SampleRate) -> Result<(), String> {
        if let Some(hz) = rate.hz()
            && !self.output.codec.supports_rate(hz)
        {
            return Err(format!(
                "{} cannot be written at {}",
                self.output.codec.name(),
                rate.label()
            ));
        }
        self.output.sample_rate = rate;
        Ok(())
    }

    /// Selects the preset at `index` and applies its settings. An index past the end is
    /// ignored.
    pub fn select_preset(&mut self, index: usize) {
        if let Some(preset) = self.presets.items.get(index).cloned() {
            self.presets.selected = Some(index);
            apply_preset(&preset, &mut self.settings, &mut self.output);
        }
    }

    /// Saves the current settings as a preset, or says why not. As in the mockup, there must be
    /// something to save.
    pub fn save_preset(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("A preset needs a name".to_string());
        }
        if !self.can_save_preset() {
            return Err(
                "Nothing has changed from this preset, so there is nothing to save".to_string(),
            );
        }
        let saved = self.presets.save_as(name, &self.settings, self.output);
        self.presets_changed = true;
        self.log(match saved {
            Saved::Added => format!("Saved preset {name}"),
            Saved::Updated => format!("Updated preset {name}"),
        });
        Ok(())
    }

    /// Deletes the selected preset, or says why not.
    pub fn delete_preset(&mut self) -> Result<(), String> {
        match self.presets.delete_selected() {
            Some(name) => {
                self.presets_changed = true;
                self.log(format!("Deleted preset {name}"));
                Ok(())
            }
            None => Err("No preset is selected to delete".to_string()),
        }
    }

    pub fn set_where(&mut self, kind: WhereKind) {
        self.strategy.where_kind = kind;
    }

    /// Sets the subfolder name or the folder, whichever the chosen place uses.
    pub fn set_where_text(&mut self, text: String) {
        self.strategy.set_where_text(text);
    }

    pub fn set_conflict(&mut self, kind: ConflictKind) {
        self.strategy.conflict_kind = kind;
    }

    /// Sets the suffix of the chosen way of handling a clash.
    pub fn set_conflict_text(&mut self, text: String) {
        self.strategy.set_conflict_text(text);
    }

    pub fn set_delete_original(&mut self, on: bool) {
        self.strategy.delete_original = on;
    }

    /// Switches one song on or off for the next run.
    pub fn set_track_included(&mut self, group: usize, track: usize, included: bool) {
        if let Some(track) = self
            .groups
            .get_mut(group)
            .and_then(|group| group.tracks.get_mut(track))
        {
            track.included = included;
        }
    }

    /// Switches every song of a group on or off.
    pub fn set_group_included(&mut self, group: usize, included: bool) {
        if let Some(group) = self.groups.get_mut(group) {
            for track in &mut group.tracks {
                track.included = included;
            }
        }
    }

    /// Removes one group from the list. Not while a run is writing or files are being
    /// measured, because both refer to groups by position.
    pub fn remove_group(&mut self, index: usize) -> Result<(), String> {
        self.check_list_is_still()?;
        if index >= self.groups.len() {
            return Ok(());
        }
        let name = self.groups.remove(index).name;
        self.jobs.retain(|job| job.group != index);
        for job in &mut self.jobs {
            if job.group > index {
                job.group -= 1;
            }
        }
        self.scroll = 0;
        self.log(format!("Removed {name} from the list"));
        Ok(())
    }

    /// Removes every group whose files have been written. Returns how many went.
    pub fn clear_completed(&mut self) -> Result<usize, String> {
        self.check_list_is_still()?;
        let done: Vec<usize> = self
            .groups
            .iter()
            .enumerate()
            .filter(|(_, group)| group.run == RunState::Saved)
            .map(|(index, _)| index)
            .collect();
        for index in done.iter().rev() {
            self.groups.remove(*index);
            self.jobs.retain(|job| job.group != *index);
            for job in &mut self.jobs {
                if job.group > *index {
                    job.group -= 1;
                }
            }
        }
        self.scroll = 0;
        if !done.is_empty() {
            self.log(format!("Cleared {} finished item(s)", done.len()));
        }
        Ok(done.len())
    }

    fn check_list_is_still(&self) -> Result<(), String> {
        if self.running {
            Err("Wait for the run to finish before changing the list".to_string())
        } else if self.pending() > 0 {
            Err("Wait for measuring to finish before changing the list".to_string())
        } else {
            Ok(())
        }
    }

    /// Adds files, CUE sheets or folders to the list and returns the measuring jobs for them.
    /// The caller starts those jobs.
    pub fn add_paths(&mut self, paths: &[PathBuf]) -> Vec<Job> {
        let loaded = load_from(paths, self.groups.len());
        self.groups.extend(loaded.groups);
        self.jobs.extend(loaded.jobs.iter().cloned());
        self.log.extend(loaded.log);
        self.refresh_radio();
        loaded.jobs
    }

    /// Empties the list, unless a run is writing files.
    pub fn clear(&mut self) -> Result<(), String> {
        if self.running {
            return Err("Wait for the run to finish before clearing the list".to_string());
        }
        self.groups.clear();
        self.jobs.clear();
        self.scroll = 0;
        Ok(())
    }

    fn start_run(&mut self) {
        match self.build_run() {
            Ok(spec) => {
                for item in &spec.items {
                    if let Some(group) = self.groups.get_mut(item.group) {
                        group.run = RunState::Queued;
                        for track in &mut group.tracks {
                            track.saved = false;
                        }
                    }
                }
                self.log(format!("Run started: {} file(s)", spec.items.len()));
                self.running = true;
                self.pending_run = Some(spec);
            }
            Err(reason) => self.log(format!("Cannot run: {reason}")),
        }
    }

    fn scroll_down(&mut self, rows: usize) {
        let last = self.row_count().saturating_sub(1);
        self.scroll = (self.scroll + rows).min(last);
    }

    fn scroll_up(&mut self, rows: usize) {
        self.scroll = self.scroll.saturating_sub(rows);
    }

    /// Changes the value of the focused control by one step.
    fn change(&mut self, steps: i32) {
        match self.focus {
            Focus::Target => self.settings.nudge_target(steps),
            Focus::PeakProtection => self.settings.peak_protection = !self.settings.peak_protection,
            Focus::Ceiling => self.settings.nudge_ceiling(steps),
            Focus::Codec => self.output.cycle_codec(steps),
            Focus::BitDepth => self.output.cycle_bit_depth(steps),
            Focus::Preset => {
                if let Some(preset) = self.presets.cycle(steps) {
                    apply_preset(&preset, &mut self.settings, &mut self.output);
                }
            }
            Focus::Where => self.strategy.cycle_where(steps),
            Focus::Conflict => self.strategy.cycle_conflict(steps),
            Focus::DeleteOriginal => {
                self.strategy.delete_original = !self.strategy.delete_original;
            }
            // Typed fields are changed by editing them, not by stepping.
            Focus::WhereText | Focus::ConflictText => {}
        }
    }

    /// Reacts to a key press.
    pub fn handle_key(&mut self, key: Key) {
        match self.mode.clone() {
            Mode::Normal => self.key_normal(key),
            Mode::Editing(buffer) => self.key_editing(key, buffer),
            Mode::NamingPreset(buffer) => self.key_naming(key, buffer),
            Mode::ConfirmRun => self.key_confirm(key),
        }
    }

    fn key_normal(&mut self, key: Key) {
        match key {
            Key::Char('q') | Key::Esc => self.should_quit = true,
            Key::Tab => self.focus = self.focus.step(1, &self.strategy),
            Key::BackTab => self.focus = self.focus.step(-1, &self.strategy),
            Key::Left => self.change(-1),
            Key::Right => self.change(1),
            Key::Char('[') => self.settings.nudge_ceiling(-1),
            Key::Char(']') => self.settings.nudge_ceiling(1),
            Key::Char('p') => self.settings.peak_protection = !self.settings.peak_protection,
            Key::Char('l') => self.show_log = !self.show_log,
            Key::Up => self.scroll_up(1),
            Key::Down => self.scroll_down(1),
            Key::PageUp => self.scroll_up(PAGE),
            Key::PageDown => self.scroll_down(PAGE),
            Key::Home => self.scroll = 0,
            Key::End => self.scroll = self.row_count().saturating_sub(1),
            Key::Char('e') => self.begin_edit(),
            Key::Char('s') => {
                if self.can_save_preset() {
                    let name = self
                        .presets
                        .selected_preset()
                        .map_or_else(String::new, |preset| preset.name.clone());
                    self.mode = Mode::NamingPreset(name);
                } else {
                    self.log("Nothing has changed from this preset, so there is nothing to save");
                }
            }
            Key::Char('d') => match self.presets.delete_selected() {
                Some(name) => {
                    self.presets_changed = true;
                    self.log(format!("Deleted preset {name}"));
                }
                None => self.log("No preset is selected to delete"),
            },
            Key::Enter | Key::Char('r') => self.request_run(),
            _ => {}
        }
    }

    fn begin_edit(&mut self) {
        let text = match self.focus {
            Focus::WhereText => self.strategy.where_text(),
            Focus::ConflictText => self.strategy.conflict_text(),
            _ => {
                self.log("Tab to the folder or suffix first, then press e to edit it");
                return;
            }
        };
        match text {
            Some(text) => self.mode = Mode::Editing(text.to_string()),
            None => self.log("The selected File Strategy choice has nothing to edit"),
        }
    }

    fn key_editing(&mut self, key: Key, mut buffer: String) {
        match key {
            Key::Esc => self.mode = Mode::Normal,
            Key::Enter => {
                match self.focus {
                    Focus::WhereText => self.strategy.set_where_text(buffer),
                    Focus::ConflictText => self.strategy.set_conflict_text(buffer),
                    _ => {}
                }
                self.mode = Mode::Normal;
            }
            Key::Backspace => {
                buffer.pop();
                self.mode = Mode::Editing(buffer);
            }
            Key::Char(c) if !c.is_control() => {
                buffer.push(c);
                self.mode = Mode::Editing(buffer);
            }
            _ => {}
        }
    }

    fn key_naming(&mut self, key: Key, mut buffer: String) {
        match key {
            Key::Esc => self.mode = Mode::Normal,
            Key::Enter => {
                self.mode = Mode::Normal;
                let name = buffer.trim().to_string();
                if name.is_empty() {
                    self.log("A preset needs a name");
                    return;
                }
                let saved = self.presets.save_as(&name, &self.settings, self.output);
                self.presets_changed = true;
                self.log(match saved {
                    Saved::Added => format!("Saved preset {name}"),
                    Saved::Updated => format!("Updated preset {name}"),
                });
            }
            Key::Backspace => {
                buffer.pop();
                self.mode = Mode::NamingPreset(buffer);
            }
            Key::Char(c) if !c.is_control() => {
                buffer.push(c);
                self.mode = Mode::NamingPreset(buffer);
            }
            _ => {}
        }
    }

    fn key_confirm(&mut self, key: Key) {
        self.mode = Mode::Normal;
        if matches!(key, Key::Char('y' | 'Y')) {
            self.start_run();
        } else {
            self.log("Run cancelled");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::built_in;
    use rangeforge_core::cue;
    use rangeforge_export::format::{BitDepth, Codec, SampleRate};

    fn measurement(lufs: f64, peak: f64) -> Measurement {
        Measurement {
            integrated_lufs: lufs,
            true_peak_dbtp: peak,
            loudness_range_lu: 10.0,
        }
    }

    fn row(label: &str, measurement: Option<Measurement>) -> TrackRow {
        TrackRow {
            measurement,
            ..TrackRow::new(label.to_string())
        }
    }

    fn album(rows: Vec<TrackRow>) -> Group {
        Group {
            name: "Led Zeppelin - Houses of the Holy".to_string(),
            kind: Kind::CueAlbum,
            source_extension: "flac".to_string(),
            tracks: rows,
            status: Status::Pending,
            run: RunState::Idle,
        }
    }

    fn app_with(groups: Vec<Group>) -> App {
        App::new(groups, Vec::new(), Vec::new(), Settings::default())
    }

    const SHEET: &str = "PERFORMER \"Led Zeppelin\"\nTITLE \"Houses of the Holy\"\nFILE \"album.flac\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"The Rain Song\"\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    TITLE \"No Quarter\"\n    INDEX 01 05:00:00\n";

    /// An app with one measured two-song CUE album, ready to run.
    fn ready_app() -> App {
        let sheet = cue::parse(SHEET).unwrap();
        let mut group = album(vec![
            row("1 - The Rain Song", Some(measurement(-20.66, -1.91))),
            row("2 - No Quarter", Some(measurement(-19.05, -2.37))),
        ]);
        // As when loaded from the sheet, each song knows who it is.
        for (row, track) in group.tracks.iter_mut().zip(&sheet.files[0].tracks) {
            row.identity = Some(Identity::from_cue(&sheet, track));
        }
        let job = Job {
            group: 0,
            audio: PathBuf::from("music/album/album.flac"),
            cue_file: Some(sheet.files[0].clone()),
            sheet: Some(sheet),
        };
        group.status = Status::Done(Origin::Decoded);
        let mut app = App::new(vec![group], vec![job], Vec::new(), Settings::default());
        app.presets = PresetList::new(built_in());
        app.presets.select_matching(&app.settings, app.output);
        app
    }

    fn last_log(app: &App) -> &str {
        app.log.last().map_or("", String::as_str)
    }

    #[test]
    fn defaults_match_the_mockup() {
        let settings = Settings::default();
        assert!((settings.target_lufs - -22.0).abs() < 1e-9);
        assert!((settings.peak_ceiling_dbtp - -0.6).abs() < 1e-9);
        assert!(settings.peak_protection);
    }

    #[test]
    fn nudging_stays_tidy_and_inside_its_limits() {
        let mut settings = Settings::default();
        for _ in 0..7 {
            settings.nudge_ceiling(1);
        }
        // Seven steps of 0.1 from -0.6 would be 0.1 without a limit; 0.0 is the maximum.
        assert!(settings.peak_ceiling_dbtp.abs() < 1e-9);

        settings.nudge_target(1);
        assert!((settings.target_lufs - -21.5).abs() < 1e-9);
        for _ in 0..200 {
            settings.nudge_target(1);
        }
        assert!(settings.target_lufs.abs() < 1e-9);
        for _ in 0..300 {
            settings.nudge_target(-1);
        }
        assert!((settings.target_lufs - -60.0).abs() < 1e-9);
    }

    #[test]
    fn repeated_nudges_do_not_drift() {
        let mut settings = Settings::default();
        for _ in 0..30 {
            settings.nudge_ceiling(-1);
            settings.nudge_ceiling(1);
        }
        assert!((settings.peak_ceiling_dbtp - -0.6).abs() < 1e-12);
    }

    #[test]
    fn new_settings_are_clamped() {
        let settings = Settings::new(-99.0, true, 5.0);
        assert!((settings.target_lufs - -60.0).abs() < 1e-9);
        assert!(settings.peak_ceiling_dbtp.abs() < 1e-9);
    }

    #[test]
    fn plans_follow_the_settings_and_flag_peak_limits() {
        let mut app = app_with(vec![album(vec![
            row("1 - quiet", Some(measurement(-20.0, -1.0))),
            row("2 - loud", Some(measurement(-14.2, -1.65))),
            row("3 - pending", None),
        ])]);

        let plan = app.plan(&app.groups[0].tracks[1]).unwrap().unwrap();
        assert!((plan.gain_db - -7.8).abs() < 1e-9);
        assert!(app.plan(&app.groups[0].tracks[2]).is_none());
        assert!(!app.any_limited());

        // Raise the target enough that the quiet song would need more than its headroom.
        app.settings.target_lufs = -14.0;
        assert!(app.any_limited());
        // Turning protection off removes the limit.
        app.handle_key(Key::Char('p'));
        assert!(!app.any_limited());
    }

    #[test]
    fn applying_a_result_fills_the_rows_and_logs_it() {
        let mut app = app_with(vec![album(vec![row("1 - a", None), row("2 - b", None)])]);
        assert_eq!(app.pending(), 1);

        app.apply(MeasureOutcome {
            group: 0,
            result: Ok((
                vec![measurement(-15.0, -3.0), measurement(-20.7, -1.9)],
                Origin::Cache,
            )),
            elapsed: Duration::ZERO,
            identity: None,
        });

        assert_eq!(app.pending(), 0);
        assert_eq!(app.groups[0].status, Status::Done(Origin::Cache));
        assert!(app.groups[0].tracks.iter().all(|t| t.measurement.is_some()));
        assert!(last_log(&app).contains("(cached)"));
    }

    #[test]
    fn a_failed_or_mismatched_result_marks_the_group_failed() {
        let mut app = app_with(vec![
            album(vec![row("1 - a", None)]),
            album(vec![row("1 - a", None), row("2 - b", None)]),
        ]);
        app.apply(MeasureOutcome {
            group: 0,
            result: Err("file not found".to_string()),
            elapsed: Duration::ZERO,
            identity: None,
        });
        app.apply(MeasureOutcome {
            group: 1,
            result: Ok((vec![measurement(-15.0, -3.0)], Origin::Decoded)),
            elapsed: Duration::ZERO,
            identity: None,
        });

        assert_eq!(
            app.groups[0].status,
            Status::Failed("file not found".into())
        );
        assert!(matches!(app.groups[1].status, Status::Failed(_)));
        // Failed files are not part of a run.
        assert_eq!(app.queued(), 0);
    }

    #[test]
    fn counts_rows_and_queued_songs() {
        let mut app = app_with(vec![
            album(vec![row("1", None), row("2", None), row("3", None)]),
            album(vec![row("1", None)]),
        ]);
        assert_eq!(app.row_count(), 6);
        assert_eq!(app.queued(), 4);

        // A failed album still lists its songs, plus one line for the message, but a run
        // skips it.
        app.groups[1].status = Status::Failed("gone".to_string());
        assert_eq!(app.row_count(), 7);
        assert_eq!(app.queued(), 3);

        // A failed write adds a message line too.
        app.groups[0].run = RunState::Failed("disk full".to_string());
        assert_eq!(app.row_count(), 8);
    }

    #[test]
    fn keys_drive_the_settings_and_the_view() {
        let mut app = app_with(vec![album(
            (0..30).map(|n| row(&n.to_string(), None)).collect(),
        )]);

        // Target is focused to begin with, so left and right change it.
        app.handle_key(Key::Right);
        assert!((app.settings.target_lufs - -21.5).abs() < 1e-9);
        app.handle_key(Key::Left);
        app.handle_key(Key::Left);
        assert!((app.settings.target_lufs - -22.5).abs() < 1e-9);

        app.handle_key(Key::Char('l'));
        assert!(app.show_log);

        app.handle_key(Key::PageDown);
        assert_eq!(app.scroll, 10);
        app.handle_key(Key::End);
        assert_eq!(app.scroll, 30);
        app.handle_key(Key::Down);
        assert_eq!(app.scroll, 30, "cannot scroll past the last row");
        app.handle_key(Key::Home);
        assert_eq!(app.scroll, 0);
        app.handle_key(Key::Up);
        assert_eq!(app.scroll, 0);

        assert!(!app.should_quit);
        app.handle_key(Key::Char('q'));
        assert!(app.should_quit);
    }

    #[test]
    fn tab_moves_between_controls_and_arrows_change_the_focused_one() {
        let mut app = ready_app();
        for _ in 0..3 {
            app.handle_key(Key::Tab);
        }
        assert_eq!(app.focus, Focus::Codec);
        app.handle_key(Key::Right);
        assert_eq!(app.output.codec, Codec::Flac);
        app.handle_key(Key::Tab);
        assert_eq!(app.focus, Focus::BitDepth);
        app.handle_key(Key::Left);
        assert_eq!(app.output.bit_depth, BitDepth::TwentyFour);
        app.handle_key(Key::BackTab);
        assert_eq!(app.focus, Focus::Codec);

        // The volume did not move while another control had the keyboard.
        assert!((app.settings.target_lufs - -22.0).abs() < 1e-9);
    }

    #[test]
    fn the_peak_protection_control_toggles_with_either_arrow() {
        let mut app = ready_app();
        app.handle_key(Key::Tab);
        assert_eq!(app.focus, Focus::PeakProtection);
        app.handle_key(Key::Right);
        assert!(!app.settings.peak_protection);
        app.handle_key(Key::Left);
        assert!(app.settings.peak_protection);
    }

    #[test]
    fn choosing_a_preset_fills_the_output_and_volume() {
        let mut app = ready_app();
        app.focus = Focus::Preset;
        app.handle_key(Key::Right);
        assert_eq!(app.presets.selected_preset().unwrap().name, "CD Quality");
        assert_eq!(app.output.codec, Codec::Flac);
        assert_eq!(app.output.bit_depth, BitDepth::Sixteen);
        assert!(!app.preset_modified());

        app.focus = Focus::Codec;
        app.handle_key(Key::Right);
        assert!(
            app.preset_modified(),
            "changing a setting marks the preset modified"
        );
    }

    #[test]
    fn saving_a_preset_asks_for_a_name_and_flags_the_change() {
        let mut app = ready_app();
        app.settings.nudge_target(2);

        app.handle_key(Key::Char('s'));
        assert!(matches!(&app.mode, Mode::NamingPreset(name) if name == "iTunes Lossless"));
        // Replace the suggested name.
        for _ in 0..20 {
            app.handle_key(Key::Backspace);
        }
        for c in "Quiet car".chars() {
            app.handle_key(Key::Char(c));
        }
        app.handle_key(Key::Enter);

        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.presets.selected_preset().unwrap().name, "Quiet car");
        assert!((app.presets.selected_preset().unwrap().target_lufs - -21.0).abs() < 1e-9);
        assert!(app.take_presets_changed());
        assert!(
            !app.take_presets_changed(),
            "the flag is cleared once taken"
        );
        assert!(last_log(&app).contains("Saved preset Quiet car"));
    }

    #[test]
    fn saving_is_only_offered_when_something_has_changed() {
        let mut app = ready_app();
        assert!(!app.can_save_preset());
        app.handle_key(Key::Char('s'));
        assert_eq!(app.mode, Mode::Normal);
        assert!(last_log(&app).contains("nothing to save"));

        app.settings.nudge_target(1);
        assert!(app.can_save_preset());

        // Settings that match no preset can always be saved as a new one.
        app.presets.selected = None;
        app.settings = Settings::default();
        assert!(app.can_save_preset());
    }

    #[test]
    fn a_run_that_cannot_start_puts_the_rows_back() {
        let mut app = ready_app();
        app.handle_key(Key::Enter);
        let _ = app.take_run_spec();
        assert_eq!(app.groups[0].run, RunState::Queued);

        app.abort_run("FFmpeg was not found");

        assert!(!app.running);
        assert_eq!(app.groups[0].run, RunState::Idle);
        assert!(last_log(&app).contains("Cannot run: FFmpeg was not found"));
    }

    #[test]
    fn naming_can_be_cancelled_and_an_empty_name_is_refused() {
        let mut app = ready_app();
        app.settings.nudge_target(1);
        app.handle_key(Key::Char('s'));
        app.handle_key(Key::Esc);
        assert_eq!(app.mode, Mode::Normal);
        assert!(!app.take_presets_changed());

        app.handle_key(Key::Char('s'));
        for _ in 0..20 {
            app.handle_key(Key::Backspace);
        }
        app.handle_key(Key::Enter);
        assert!(last_log(&app).contains("needs a name"));
        assert!(!app.take_presets_changed());
        assert_eq!(app.presets.items.len(), 5);
    }

    #[test]
    fn deleting_a_preset_needs_one_selected() {
        let mut app = ready_app();
        app.handle_key(Key::Char('d'));
        assert_eq!(app.presets.items.len(), 4);
        assert!(app.take_presets_changed());
        assert!(last_log(&app).contains("Deleted preset iTunes Lossless"));

        app.handle_key(Key::Char('d'));
        assert_eq!(app.presets.items.len(), 4);
        assert!(last_log(&app).contains("No preset is selected"));
        assert!(!app.take_presets_changed());
    }

    #[test]
    fn text_fields_are_edited_in_place() {
        let mut app = ready_app();
        app.focus = Focus::ConflictText;
        app.handle_key(Key::Char('e'));
        assert_eq!(app.mode, Mode::Editing("_00".to_string()));

        app.handle_key(Key::Backspace);
        app.handle_key(Key::Backspace);
        app.handle_key(Key::Char('1'));
        app.handle_key(Key::Char('2'));
        // Keys that would normally act, like q, are just letters while typing.
        app.handle_key(Key::Char('q'));
        assert!(!app.should_quit);
        app.handle_key(Key::Enter);

        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.strategy.number_suffix, "_12q");
    }

    #[test]
    fn editing_can_be_cancelled() {
        let mut app = ready_app();
        app.focus = Focus::ConflictText;
        app.handle_key(Key::Char('e'));
        app.handle_key(Key::Char('9'));
        app.handle_key(Key::Esc);
        assert_eq!(app.strategy.number_suffix, "_00");
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn editing_only_starts_on_a_text_control() {
        let mut app = ready_app();
        app.handle_key(Key::Char('e'));
        assert_eq!(app.mode, Mode::Normal);
        assert!(last_log(&app).contains("Tab to the folder or suffix"));
    }

    #[test]
    fn run_waits_for_measuring_and_for_something_to_run() {
        let mut app = app_with(vec![album(vec![row("1", None)])]);
        app.handle_key(Key::Enter);
        assert!(last_log(&app).contains("not finished"));
        assert!(app.take_run_spec().is_none());

        let mut empty = app_with(Vec::new());
        empty.handle_key(Key::Enter);
        assert!(last_log(&empty).contains("Nothing to run"));
    }

    #[test]
    fn run_builds_a_plan_for_every_song() {
        let mut app = ready_app();
        app.handle_key(Key::Enter);

        assert!(app.running);
        assert_eq!(app.groups[0].run, RunState::Queued);
        let spec = app.take_run_spec().unwrap();
        assert_eq!(spec.items.len(), 1);
        let plans = &spec.items[0].plans;
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].file_name, "01 - The Rain Song.m4a");
        assert_eq!(plans[1].file_name, "02 - No Quarter.m4a");
        // The same gains the table shows: -22 minus the measured loudness.
        assert!((plans[0].gain_db - -1.34).abs() < 1e-9);
        assert!((plans[1].gain_db - -2.95).abs() < 1e-9);
        assert_eq!(spec.strategy, Strategy::safe());
        assert_eq!(spec.items[0].directory, PathBuf::from("music/album"));
        assert!(
            app.take_run_spec().is_none(),
            "a spec is handed over only once"
        );
    }

    #[test]
    fn run_uses_the_output_format_on_screen() {
        let mut app = ready_app();
        app.output.codec = Codec::Flac;
        app.handle_key(Key::Enter);
        let spec = app.take_run_spec().unwrap();
        assert_eq!(spec.format.codec, Codec::Flac);
        assert_eq!(spec.items[0].plans[0].file_name, "01 - The Rain Song.flac");
    }

    #[test]
    fn a_second_run_is_refused_while_one_is_going() {
        let mut app = ready_app();
        app.handle_key(Key::Enter);
        let _ = app.take_run_spec();
        app.handle_key(Key::Enter);
        assert!(last_log(&app).contains("already in progress"));
        assert!(app.take_run_spec().is_none());
    }

    #[test]
    fn overwriting_asks_first_and_only_a_yes_starts_the_run() {
        let mut app = ready_app();
        app.strategy.conflict_kind = ConflictKind::Overwrite;

        app.handle_key(Key::Enter);
        assert_eq!(app.mode, Mode::ConfirmRun);
        assert!(!app.running);
        assert!(app.take_run_spec().is_none());

        app.handle_key(Key::Char('n'));
        assert_eq!(app.mode, Mode::Normal);
        assert!(last_log(&app).contains("cancelled"));
        assert!(app.take_run_spec().is_none());

        app.handle_key(Key::Enter);
        app.handle_key(Key::Char('y'));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.running);
        assert!(app.take_run_spec().is_some());
    }

    #[test]
    fn a_strategy_that_cannot_work_stops_the_run_with_a_reason() {
        let mut app = ready_app();
        app.strategy.where_kind = WhereKind::Folder;
        app.handle_key(Key::Enter);
        assert!(!app.running);
        assert!(last_log(&app).contains("choose a folder"));

        app.strategy.where_kind = WhereKind::NextToOriginal;
        app.strategy.number_suffix = "_new".to_string();
        app.handle_key(Key::Enter);
        assert!(last_log(&app).contains("must end in digits"));
        assert!(app.take_run_spec().is_none());
    }

    #[test]
    fn failed_files_are_left_out_of_a_run() {
        let mut app = ready_app();
        app.groups[0].status = Status::Failed("gone".to_string());
        app.handle_key(Key::Enter);
        assert!(last_log(&app).contains("Nothing to run"));
    }

    #[test]
    fn run_events_update_the_rows_and_the_log() {
        let mut app = ready_app();
        app.handle_key(Key::Enter);
        let _ = app.take_run_spec();

        app.apply_run_event(RunEvent::Encoding {
            group: 0,
            index: 0,
            total: 2,
        });
        assert_eq!(app.groups[0].run, RunState::Encoding { index: 0, total: 2 });

        app.apply_run_event(RunEvent::Saved {
            group: 0,
            index: 0,
            path: PathBuf::from("out/01 - The Rain Song.m4a"),
        });
        assert!(app.groups[0].tracks[0].saved);
        assert!(!app.groups[0].tracks[1].saved);
        assert_eq!(last_log(&app), "Saved 01 - The Rain Song.m4a");

        app.apply_run_event(RunEvent::Finished { group: 0 });
        assert_eq!(app.groups[0].run, RunState::Saved);

        app.apply_run_event(RunEvent::AllDone);
        assert!(!app.running);
        assert_eq!(last_log(&app), "Run complete: 1 file(s) written, 0 failed");
    }

    #[test]
    fn a_failed_write_is_shown_and_counted() {
        let mut app = ready_app();
        app.handle_key(Key::Enter);
        let _ = app.take_run_spec();
        app.apply_run_event(RunEvent::Failed {
            group: 0,
            message: "FFmpeg failed".to_string(),
        });
        assert_eq!(
            app.groups[0].run,
            RunState::Failed("FFmpeg failed".to_string())
        );
        app.apply_run_event(RunEvent::AllDone);
        assert!(last_log(&app).contains("0 file(s) written, 1 failed"));
    }

    #[test]
    fn a_new_run_clears_the_saved_marks() {
        let mut app = ready_app();
        app.groups[0].tracks[0].saved = true;
        app.groups[0].run = RunState::Saved;
        app.handle_key(Key::Enter);
        assert!(!app.groups[0].tracks[0].saved);
        assert_eq!(app.groups[0].run, RunState::Queued);
    }

    #[test]
    fn codec_labels_follow_the_mockup() {
        let mut group = album(Vec::new());
        assert_eq!(group.codec_label(), "Split(cue)");
        group.kind = Kind::SingleFile;
        assert_eq!(group.codec_label(), "(flac)");
    }

    #[test]
    fn the_blocker_explains_why_a_run_cannot_start() {
        let mut app = app_with(vec![album(vec![row("1", None)])]);
        assert_eq!(
            app.run_blocker().as_deref(),
            Some("Measuring is not finished yet")
        );

        let empty = app_with(Vec::new());
        assert_eq!(empty.run_blocker().as_deref(), Some("Nothing to run"));

        let mut ready = ready_app();
        assert_eq!(ready.run_blocker(), None);
        ready.strategy.where_kind = WhereKind::Folder;
        assert!(
            ready
                .run_blocker()
                .unwrap()
                .starts_with("Cannot run: choose a folder")
        );
        ready.strategy.where_kind = WhereKind::NextToOriginal;
        ready.running = true;
        assert_eq!(
            ready.run_blocker().as_deref(),
            Some("A run is already in progress")
        );
        app.running = false;
    }

    #[test]
    fn a_window_asks_for_confirmation_itself() {
        let mut app = ready_app();
        app.set_delete_original(true);

        assert_eq!(app.begin_run(false), RunRequest::NeedsConfirmation);
        assert!(!app.running);
        assert!(app.take_run_spec().is_none());

        assert_eq!(app.begin_run(true), RunRequest::Started);
        assert!(app.running);
        assert!(app.take_run_spec().is_some());
    }

    #[test]
    fn a_window_run_without_confirmation_starts_straight_away() {
        let mut app = ready_app();
        assert_eq!(app.begin_run(false), RunRequest::Started);
        assert!(app.take_run_spec().is_some());
        // And a second request while it is going is refused with the reason, and logged.
        assert_eq!(
            app.begin_run(false),
            RunRequest::Refused("A run is already in progress".to_string())
        );
        assert!(last_log(&app).contains("already in progress"));
    }

    #[test]
    fn a_window_run_is_refused_while_measuring() {
        let mut app = app_with(vec![album(vec![row("1", None)])]);
        assert_eq!(
            app.begin_run(true),
            RunRequest::Refused("Measuring is not finished yet".to_string())
        );
    }

    #[test]
    fn direct_setters_keep_values_in_range() {
        let mut app = ready_app();
        app.set_target(-99.0);
        assert!((app.settings.target_lufs - -60.0).abs() < 1e-9);
        app.set_target(-17.34);
        assert!(
            (app.settings.target_lufs - -17.3).abs() < 1e-9,
            "rounded to a tenth"
        );
        app.set_ceiling(3.0);
        assert!(app.settings.peak_ceiling_dbtp.abs() < 1e-9);
        app.set_peak_protection(false);
        assert!(!app.settings.peak_protection);
        assert!(
            (app.settings.target_lufs - -17.3).abs() < 1e-9,
            "other values are kept"
        );

        app.set_codec(Codec::Wav);
        app.set_bit_depth(BitDepth::TwentyFour);
        assert_eq!(
            (app.output.codec, app.output.bit_depth),
            (Codec::Wav, BitDepth::TwentyFour)
        );
    }

    #[test]
    fn selecting_a_preset_by_index_applies_it() {
        let mut app = ready_app();
        app.select_preset(2);
        assert_eq!(app.presets.selected_preset().unwrap().name, "CD Quality");
        assert_eq!(app.output.codec, Codec::Flac);
        assert_eq!(app.output.bit_depth, BitDepth::Sixteen);

        app.select_preset(99);
        assert_eq!(
            app.presets.selected,
            Some(2),
            "an index past the end changes nothing"
        );
    }

    #[test]
    fn saving_and_deleting_presets_from_a_window() {
        let mut app = ready_app();
        assert!(
            app.save_preset("Car")
                .unwrap_err()
                .contains("nothing to save")
        );

        app.set_target(-18.0);
        assert!(app.save_preset("   ").unwrap_err().contains("needs a name"));
        app.save_preset("Car").unwrap();
        assert!(app.take_presets_changed());
        assert_eq!(app.presets.selected_preset().unwrap().name, "Car");

        app.delete_preset().unwrap();
        assert!(app.take_presets_changed());
        assert!(
            app.delete_preset()
                .unwrap_err()
                .contains("No preset is selected")
        );
        assert_eq!(app.presets.items.len(), 5);
    }

    #[test]
    fn strategy_choices_can_be_set_directly() {
        let mut app = ready_app();
        app.set_conflict(ConflictKind::RenameExisting);
        app.set_conflict_text("_backup".to_string());
        assert_eq!(app.strategy.old_suffix, "_backup");
        app.set_where(WhereKind::Folder);
        app.set_where_text("D:\\music\\out".to_string());
        assert_eq!(app.strategy.folder, "D:\\music\\out");
        app.set_where(WhereKind::Subfolder);
        app.set_where_text("Balanced".to_string());
        assert_eq!(app.strategy.subfolder, "Balanced");
        app.set_delete_original(true);
        assert!(app.strategy.delete_original);
    }

    #[test]
    fn the_name_patterns_are_checked_before_they_are_kept() {
        let mut app = ready_app();
        assert!(!app.take_settings_changed());

        let error = app.set_naming("{nope}", "{name}").unwrap_err();
        assert!(error.contains("CUE album names"), "{error}");
        assert_eq!(app.naming, NameSettings::default(), "nothing changed");
        assert!(!app.take_settings_changed());

        app.set_naming(" {artist} - {title} ", "{name}").unwrap();
        assert_eq!(app.naming.album, "{artist} - {title}");
        assert!(app.take_settings_changed());
        assert!(
            !app.take_settings_changed(),
            "the flag is cleared once taken"
        );

        // Setting the same thing again is not a change.
        app.set_naming("{artist} - {title}", "{name}").unwrap();
        assert!(!app.take_settings_changed());
    }

    #[test]
    fn a_run_names_files_by_the_patterns() {
        let mut app = ready_app();
        app.set_naming("{artist} - {title}", "{name}").unwrap();
        app.handle_key(Key::Enter);
        let spec = app.take_run_spec().unwrap();
        assert_eq!(
            spec.items[0].plans[0].file_name,
            "Led Zeppelin - The Rain Song.m4a"
        );
    }

    #[test]
    fn a_dsd_source_is_written_as_the_dsd_format_and_others_are_not() {
        let mut app = ready_app();
        app.jobs[0].audio.set_extension("dsf");
        app.handle_key(Key::Enter);
        let spec = app.take_run_spec().unwrap();
        let format = spec.items[0].format;
        assert_eq!(format.sample_rate, SampleRate::Hz88200);
        assert_eq!(format.bit_depth, BitDepth::TwentyFour);
        assert_eq!(
            spec.format.sample_rate,
            SampleRate::Original,
            "the main format is untouched"
        );

        let mut other = ready_app();
        other.handle_key(Key::Enter);
        let spec = other.take_run_spec().unwrap();
        assert_eq!(spec.items[0].format, spec.format);
    }

    #[test]
    fn the_file_strategy_and_the_question_are_noticed_as_changes_to_save() {
        let mut app = ready_app();
        app.take_settings_changed();
        app.set_delete_original(true);
        assert!(app.take_settings_changed());
        assert!(!app.take_settings_changed(), "once is enough");
        app.set_conflict(ConflictKind::Overwrite);
        assert!(app.take_settings_changed());
        app.set_confirm_destructive(false);
        assert!(app.take_settings_changed());
        assert!(!app.saved_settings().confirm_destructive);
    }

    #[test]
    fn turning_the_question_off_lets_a_deleting_run_start_without_asking() {
        let mut app = ready_app();
        app.set_delete_original(true);
        assert_eq!(app.begin_run(false), RunRequest::NeedsConfirmation);
        app.set_confirm_destructive(false);
        assert!(!app.asks_before_run());
        assert_eq!(app.begin_run(false), RunRequest::Started);
    }

    #[test]
    fn settings_loaded_at_start_are_not_saved_straight_back() {
        let mut app = ready_app();
        let mut saved = SavedSettings::default();
        saved.strategy.delete_original = true;
        saved.confirm_destructive = false;
        app.apply_saved_settings(saved);
        assert!(app.strategy.delete_original);
        assert!(!app.take_settings_changed());
    }

    fn charted(
        artist: &str,
        title: &str,
        peak: u16,
        year: u16,
    ) -> rangeforge_lookup::billboard::Song {
        rangeforge_lookup::billboard::Song {
            artist: artist.to_string(),
            title: title.to_string(),
            peak,
            first_year: year,
        }
    }

    #[test]
    fn songs_that_charted_are_marked_only_while_lookups_are_on() {
        let mut app = ready_app();
        let index = Arc::new(Hot100::from_songs(vec![charted(
            "Led Zeppelin",
            "The Rain Song",
            14,
            1973,
        )]));

        // Lookups start off, so nothing is marked even with an index.
        app.set_radio(Some(Arc::clone(&index)));
        assert_eq!(app.groups[0].tracks[0].radio, None);
        assert_eq!(app.radio_songs(), Some(1));

        app.set_metadata_enabled(true);
        assert_eq!(
            app.groups[0].tracks[0].radio,
            Some(RadioHit {
                peak: 14,
                year: 1973
            })
        );
        assert_eq!(
            app.groups[0].tracks[1].radio, None,
            "No Quarter never charted"
        );

        app.set_metadata_enabled(false);
        assert_eq!(app.groups[0].tracks[0].radio, None);

        // Taking the index away clears the marks too.
        app.set_metadata_enabled(true);
        app.set_radio(None);
        assert_eq!(app.groups[0].tracks[0].radio, None);
        assert_eq!(app.radio_songs(), None);
    }

    #[test]
    fn files_added_later_are_marked_too() {
        let dir = tempfile::tempdir().unwrap();
        let cue = dir.path().join("album.cue");
        std::fs::write(&cue, SHEET.replace("album.flac", "other.flac")).unwrap();
        std::fs::write(dir.path().join("other.flac"), b"").unwrap();

        let mut app = app_with(Vec::new());
        app.set_metadata_enabled(true);
        app.set_radio(Some(Arc::new(Hot100::from_songs(vec![charted(
            "Led Zeppelin",
            "No Quarter",
            1,
            1973,
        )]))));
        app.add_paths(&[cue]);

        assert_eq!(app.groups[0].tracks[1].radio.map(|hit| hit.peak), Some(1));
    }

    #[test]
    fn a_single_file_is_looked_up_by_its_tags_once_they_are_read_and_by_its_name_before() {
        use rangeforge_core::identify::IdentitySource;

        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Led Zeppelin").join("Houses of the Holy");
        std::fs::create_dir_all(&folder).unwrap();
        let song = folder.join("04 - The Song Remains the Same.flac");
        std::fs::write(&song, b"").unwrap();

        let mut app = app_with(Vec::new());
        app.set_metadata_enabled(true);
        app.set_radio(Some(Arc::new(Hot100::from_songs(vec![
            charted("Led Zeppelin", "Dyer Maker", 20, 1973),
            charted("Led Zeppelin", "The Song Remains The Same (Live)", 99, 1976),
        ]))));
        app.add_paths(&[song]);

        // Added: only its name is known, and it names a song that did not chart.
        let track = &app.groups[0].tracks[0];
        let guess = track.identity.as_ref().unwrap();
        assert_eq!(guess.source, IdentitySource::FileName);
        assert_eq!(guess.artist.as_deref(), Some("Led Zeppelin"));
        assert_eq!(guess.title.as_deref(), Some("The Song Remains the Same"));
        assert_eq!(
            track.radio.map(|hit| hit.peak),
            Some(99),
            "found by its guessed name"
        );

        // Measured: the file's own tags say something else, and they win.
        let tagged = Identity {
            artist: Some("Led Zeppelin".to_string()),
            title: Some("D'yer Mak'er".to_string()),
            ..Identity::unknown(IdentitySource::FileTags)
        };
        app.apply(MeasureOutcome {
            group: 0,
            result: Ok((vec![measurement(-15.0, -3.0)], Origin::Decoded)),
            elapsed: Duration::ZERO,
            identity: Some(tagged),
        });
        let track = &app.groups[0].tracks[0];
        assert_eq!(
            track.identity.as_ref().unwrap().source,
            IdentitySource::FileTags
        );
        assert_eq!(track.radio.map(|hit| hit.peak), Some(20));
    }

    #[test]
    fn a_cue_albums_identity_is_not_replaced_by_a_measurement() {
        let mut app = ready_app();
        let before = app.groups[0].tracks[0].identity.clone();
        app.groups[0].status = Status::Pending;
        app.apply(MeasureOutcome {
            group: 0,
            result: Ok((
                vec![measurement(-20.0, -2.0), measurement(-19.0, -2.0)],
                Origin::Decoded,
            )),
            elapsed: Duration::ZERO,
            identity: Some(Identity::unknown(
                rangeforge_core::identify::IdentitySource::FileTags,
            )),
        });
        assert_eq!(app.groups[0].tracks[0].identity, before);
    }

    #[test]
    fn single_files_are_named_from_what_is_known_about_them() {
        use rangeforge_core::identify::IdentitySource;

        let dir = tempfile::tempdir().unwrap();
        let song = dir.path().join("whatever.flac");
        std::fs::write(&song, b"").unwrap();
        let mut app = app_with(Vec::new());
        app.add_paths(&[song]);
        app.apply(MeasureOutcome {
            group: 0,
            result: Ok((vec![measurement(-15.0, -3.0)], Origin::Decoded)),
            elapsed: Duration::ZERO,
            identity: Some(Identity {
                artist: Some("The Killers".to_string()),
                title: Some("Shot at the Night".to_string()),
                track: Some(14),
                ..Identity::unknown(IdentitySource::FileTags)
            }),
        });
        app.set_naming("{track:02} - {title}", "{artist} - {title}")
            .unwrap();

        app.handle_key(Key::Enter);
        let spec = app.take_run_spec().unwrap();
        assert_eq!(
            spec.items[0].plans[0].file_name,
            "The Killers - Shot at the Night.m4a"
        );
    }

    fn raw(name: &str, count: u32) -> RawTag {
        RawTag {
            name: name.to_string(),
            count,
        }
    }

    /// The test album with Last.fm switched on and a key available.
    fn lastfm_app() -> App {
        let mut app = ready_app();
        app.set_metadata_enabled(true);
        app.set_lastfm_enabled(true);
        app.lastfm_key = KeySource::Saved;
        app
    }

    fn pick_song(group: usize, track: usize) -> Pick {
        Pick {
            group,
            track: Some(track),
        }
    }

    fn pick_album(group: usize) -> Pick {
        Pick { group, track: None }
    }

    #[test]
    fn one_change_can_reach_a_selection_of_songs() {
        let mut app = ready_app();
        app.set_metadata_enabled(true);
        app.lastfm_key = KeySource::Saved;
        // The same song twice, and the other, are each changed once.
        let picked = [pick_song(0, 1), pick_song(0, 0), pick_song(0, 1)];
        app.add_tag(&picked, "Mixtape").unwrap();
        assert!(
            app.groups[0]
                .tracks
                .iter()
                .all(|t| t.edits.added == ["Mixtape"])
        );
        assert!(
            app.add_tag(&[pick_song(0, 0), pick_song(5, 0)], "x")
                .is_err(),
            "nothing half-done"
        );

        // Only the fields filled in are changed, so each song keeps its own title.
        let titles: Vec<_> = app.groups[0]
            .tracks
            .iter()
            .map(|t| t.identity.as_ref().unwrap().title.clone())
            .collect();
        let names = Names {
            album: "Shared".to_string(),
            album_artist: "Band".to_string(),
            ..Names::default()
        };
        app.set_names(&picked, &names, true).unwrap();
        for (row, title) in app.groups[0].tracks.iter().zip(&titles) {
            let identity = row.identity.as_ref().unwrap();
            assert_eq!(identity.album.as_deref(), Some("Shared"));
            assert_eq!(identity.album_artist.as_deref(), Some("Band"));
            assert_eq!(&identity.title, title);
        }

        // Both songs share one album, which is asked about once.
        app.request_lastfm(&picked).unwrap();
        assert_eq!(app.take_questions().len(), 1, "they share one album");
    }

    #[test]
    fn tags_and_names_can_be_edited_by_hand() {
        let mut app = ready_app();
        app.set_metadata_enabled(true);
        app.groups[0].tracks[0].identity.as_mut().unwrap().genre = Some("Rock, Prog".to_string());
        let shown = |app: &App| app.song_genres(&app.groups[0].tracks[0]);

        assert!(app.add_tag(&[pick_song(0, 0)], "  ").is_err());
        assert!(
            app.add_tag(
                &[pick_song(0, 0)],
                " , ;
"
            )
            .is_err(),
            "only separators is no tag"
        );
        // Several tags at once, whatever separates them; a repeat is added once.
        assert_eq!(
            app.add_tag(
                &[pick_song(0, 0)],
                "Mixed; Live,
mixed ,	Rare"
            )
            .unwrap(),
            3
        );
        assert_eq!(
            app.groups[0].tracks[0].edits.added,
            ["Mixed", "Live", "Rare"]
        );
        assert_eq!(app.groups[0].tracks[1].edits.added, Vec::<String>::new());
        for tag in ["Mixed", "Live", "Rare"] {
            app.remove_tag(0, 0, tag).unwrap();
        }
        app.add_tag(&[pick_song(0, 0)], "Favourite").unwrap();
        app.remove_tag(0, 0, "prog").unwrap();
        assert_eq!(shown(&app).album, ["Rock"]);
        assert_eq!(shown(&app).mine, ["Favourite"]);
        assert_eq!(
            app.genre_tag(&app.groups[0].tracks[0]).as_deref(),
            Some("Rock, Favourite")
        );
        assert!(app.add_tag(&[pick_song(0, 99)], "x").is_err());

        // Taking away everything writes an empty tag, rather than leaving the old one.
        app.remove_tag(0, 0, "Rock").unwrap();
        app.remove_tag(0, 0, "Favourite").unwrap();
        assert_eq!(app.genre_tag(&app.groups[0].tracks[0]).as_deref(), Some(""));

        // An album-wide tag reaches every song.
        app.add_tag(&[pick_album(0)], "Mine").unwrap();
        assert!(
            app.groups[0]
                .tracks
                .iter()
                .all(|t| t.edits.added == ["Mine"])
        );

        // Names typed in replace what was known, and a measurement does not undo them.
        let names = Names {
            artist: "The Alan Parsons Project".to_string(),
            title: "I Robot".to_string(),
            album: " I Robot ".to_string(),
            album_artist: String::new(),
        };
        app.set_names(&[pick_song(0, 0)], &names, false).unwrap();
        let identity = app.groups[0].tracks[0].identity.clone().unwrap();
        assert_eq!(identity.artist.as_deref(), Some("The Alan Parsons Project"));
        assert_eq!(identity.album.as_deref(), Some("I Robot"));
        assert_eq!(identity.album_artist, None);
        assert_eq!(
            identity.source,
            rangeforge_core::identify::IdentitySource::Hand
        );
        // Last.fm is now asked about the typed names.
        assert!(genres::questions(&identity).iter().any(|s| matches!(
            s,
            Subject::Album { artist, .. } if artist == "The Alan Parsons Project"
        )));

        // The album's names go to every song but leave artist and title alone.
        let album = Names {
            album: "Other".to_string(),
            ..Names::default()
        };
        app.set_names(&[pick_album(0)], &album, false).unwrap();
        assert!(
            app.groups[0]
                .tracks
                .iter()
                .all(|t| t.identity.as_ref().unwrap().album.as_deref() == Some("Other"))
        );
        assert_eq!(
            app.groups[0].tracks[0]
                .identity
                .as_ref()
                .unwrap()
                .title
                .as_deref(),
            Some("I Robot")
        );
        assert!(app.set_names(&[pick_album(9)], &album, false).is_err());
    }

    #[test]
    fn asking_last_fm_by_hand_works_with_lookups_off_and_asks_afresh() {
        let mut app = ready_app();
        app.set_metadata_enabled(true);
        assert!(
            app.request_lastfm(&[pick_song(0, 0)]).is_err(),
            "no key yet"
        );

        app.lastfm_key = KeySource::Saved;
        assert!(!app.wants_lastfm(), "automatic lookups stay off");
        app.request_lastfm(&[pick_song(0, 0)]).unwrap();
        assert!(app.has_questions());
        let album = app.take_questions();
        assert_eq!(album.len(), 1, "only that album, nothing automatic");
        assert!(matches!(album[0], Subject::Album { .. }));
        assert!(app.was_requested(&album[0]));
        app.apply_answer(&album[0], vec![raw("rock", 100)]);
        assert!(!app.was_requested(&album[0]), "done once answered");
        assert!(!app.has_questions());

        // Asking again forgets the answer and asks again.
        app.request_lastfm(&[pick_song(0, 0)]).unwrap();
        assert_eq!(app.take_questions().len(), 1);

        // The album row asks about the album too.
        app.request_lastfm(&[pick_album(0)]).unwrap();
        assert_eq!(app.take_questions().len(), 1);
        assert!(app.request_lastfm(&[pick_album(99)]).is_err());
    }

    #[test]
    fn last_fm_is_asked_about_each_album_once_and_only_when_wanted() {
        let mut app = ready_app();
        assert!(app.take_questions().is_empty(), "everything starts off");
        assert!(!app.has_questions());

        let mut app = lastfm_app();
        assert!(app.has_questions());
        let questions = app.take_questions();
        // One album; its two songs are not asked about.
        assert_eq!(questions.len(), 1);
        assert!(matches!(questions[0], Subject::Album { .. }));
        assert!(app.take_questions().is_empty(), "handed out once");
        assert!(!app.has_questions());

        // Failed or abandoned questions can be handed out again.
        app.forget_asked();
        assert!(app.has_questions());
        assert_eq!(app.take_questions().len(), 1);

        // Answered ones are never asked again.
        app.forget_asked();
        for subject in &questions {
            app.apply_answer(subject, vec![raw("rock", 100)]);
        }
        assert!(!app.has_questions());
        assert_eq!(app.answered(), 1);

        // Switching Last.fm off, or losing the key, stops the questions.
        let mut off = ready_app();
        off.set_metadata_enabled(true);
        off.lastfm_key = KeySource::Saved;
        assert!(off.take_questions().is_empty(), "Last.fm itself is off");
    }

    #[test]
    fn answers_become_chips_and_a_comma_separated_genre_tag_with_radio_last() {
        let mut app = lastfm_app();
        app.set_radio(Some(Arc::new(Hot100::from_songs(vec![charted(
            "Led Zeppelin",
            "The Rain Song",
            14,
            1973,
        )]))));
        for subject in app.take_questions() {
            app.apply_answer(
                &subject,
                vec![raw("classic rock", 100), raw("hard rock", 80)],
            );
        }
        // A song-level answer (nothing produces one now): the album's own tags are not repeated.
        let rain = Subject::Track {
            artist: "Led Zeppelin".to_string(),
            title: "The Rain Song".to_string(),
        };
        app.apply_answer(&rain, vec![raw("ballad", 90), raw("classic rock", 80)]);

        let rain = &app.groups[0].tracks[0];
        let genres = app.song_genres(rain);
        assert_eq!(genres.album, ["Classic Rock", "Hard Rock"]);
        assert_eq!(genres.song, ["Ballad"]);
        assert_eq!(
            app.genre_tag(rain).as_deref(),
            Some("Classic Rock, Hard Rock, Ballad, Radio")
        );
        // No Quarter did not chart and has no song-level extras.
        let quarter = &app.groups[0].tracks[1];
        assert_eq!(
            app.genre_tag(quarter).as_deref(),
            Some("Classic Rock, Hard Rock")
        );

        // The genre tag is what a run writes.
        app.handle_key(Key::Enter);
        let spec = app.take_run_spec().unwrap();
        assert_eq!(
            spec.items[0].plans[0].tags.genre.as_deref(),
            Some("Classic Rock, Hard Rock, Ballad, Radio")
        );
        assert_eq!(
            spec.items[0].plans[1].tags.genre.as_deref(),
            Some("Classic Rock, Hard Rock")
        );
    }

    #[test]
    fn a_song_page_address_is_built_from_the_songs_names() {
        let mut app = lastfm_app();
        assert_eq!(
            app.song_page_url(0, 0).unwrap(),
            "https://www.last.fm/music/Led+Zeppelin/_/The+Rain+Song/+tags"
        );
        assert!(app.song_page_url(0, 99).is_err());
        app.groups[0].tracks[0].identity = None;
        assert!(app.song_page_url(0, 0).is_err(), "nobody to look up");
    }

    #[test]
    fn with_lookups_off_nothing_shows_and_no_genre_is_changed() {
        let mut app = ready_app();
        app.groups[0].tracks[0].identity.as_mut().unwrap().genre = Some("Rock".to_string());
        // Even an existing genre is neither shown nor rewritten while lookups are off.
        assert!(app.song_genres(&app.groups[0].tracks[0]).is_empty());
        assert_eq!(app.genre_tag(&app.groups[0].tracks[0]), None);

        app.handle_key(Key::Enter);
        let spec = app.take_run_spec().unwrap();
        assert_eq!(spec.items[0].plans[0].tags.genre, None);
    }

    #[test]
    fn a_song_whose_file_already_has_a_genre_keeps_it_and_gains_the_others() {
        let mut app = lastfm_app();
        app.groups[0].tracks[0].identity.as_mut().unwrap().genre = Some("Rock".to_string());
        for subject in app.take_questions() {
            if matches!(subject, Subject::Album { .. }) {
                app.apply_answer(&subject, vec![raw("classic rock", 100)]);
            }
        }
        assert_eq!(
            app.genre_tag(&app.groups[0].tracks[0]).as_deref(),
            Some("Rock, Classic Rock")
        );
    }

    #[test]
    fn the_lookup_switches_are_saved_and_last_fm_needs_all_three() {
        let mut app = ready_app();
        assert!(!app.take_settings_changed());
        assert!(!app.wants_lastfm());

        app.set_lastfm_enabled(true);
        assert!(app.take_settings_changed());
        assert!(!app.wants_lastfm(), "the main switch is off");

        app.set_metadata_enabled(true);
        assert!(!app.wants_lastfm(), "there is no key");

        app.lastfm_key = KeySource::Saved;
        assert!(app.wants_lastfm());
        app.set_lastfm_enabled(false);
        assert!(
            !app.wants_lastfm(),
            "a saved key stays unused while switched off"
        );

        let saved = app.saved_settings();
        assert!(saved.metadata.enabled);
        assert!(!saved.metadata.lastfm_enabled);

        // Setting the same value again is not a change.
        app.take_settings_changed();
        app.set_metadata_enabled(true);
        assert!(!app.take_settings_changed());
    }

    #[test]
    fn songs_can_be_left_out_of_a_run() {
        let mut app = ready_app();
        assert_eq!(app.queued(), 2);

        app.set_track_included(0, 1, false);
        assert_eq!(app.queued(), 1);
        app.handle_key(Key::Enter);
        let spec = app.take_run_spec().unwrap();
        let skips: Vec<bool> = spec.items[0].plans.iter().map(|p| p.skip).collect();
        assert_eq!(skips, [false, true]);

        // Out-of-range indexes are ignored.
        app.set_track_included(5, 0, false);
        app.set_track_included(0, 9, false);
    }

    #[test]
    fn a_group_with_every_song_left_out_is_not_run() {
        let mut app = ready_app();
        app.set_group_included(0, false);
        assert_eq!(app.queued(), 0);
        assert_eq!(app.run_blocker().as_deref(), Some("Nothing to run"));

        app.set_group_included(0, true);
        assert_eq!(app.queued(), 2);
    }

    #[test]
    fn a_removed_group_takes_its_job_and_renumbers_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = ready_app();
        for name in ["b.flac", "c.flac"] {
            let path = dir.path().join(name);
            std::fs::write(&path, b"").unwrap();
            app.add_paths(&[path]);
        }
        for group in &mut app.groups {
            group.status = Status::Done(Origin::Decoded);
        }
        assert_eq!(app.groups.len(), 3);

        app.remove_group(1).unwrap();

        assert_eq!(app.groups.len(), 2);
        assert_eq!(app.groups[1].name, "c.flac");
        let indexes: Vec<usize> = app.jobs.iter().map(|job| job.group).collect();
        assert_eq!(indexes, [0, 1], "c.flac's job now points at group 1");
        assert!(app.jobs[1].audio.ends_with("c.flac"));
        assert!(last_log(&app).contains("Removed b.flac"));

        app.remove_group(9).unwrap();
        assert_eq!(app.groups.len(), 2, "an unknown index changes nothing");
    }

    #[test]
    fn finished_groups_can_be_cleared_and_the_rest_stay() {
        let mut app = ready_app();
        let mut second = app.groups[0].clone();
        second.name = "Second".to_string();
        second.run = RunState::Idle;
        app.groups.push(second);
        let mut job = app.jobs[0].clone();
        job.group = 1;
        app.jobs.push(job);
        app.groups[0].run = RunState::Saved;

        assert_eq!(app.clear_completed().unwrap(), 1);

        assert_eq!(app.groups.len(), 1);
        assert_eq!(app.groups[0].name, "Second");
        assert_eq!(app.jobs.len(), 1);
        assert_eq!(app.jobs[0].group, 0);
        assert_eq!(app.clear_completed().unwrap(), 0);
    }

    #[test]
    fn the_list_is_left_alone_while_busy() {
        let mut app = ready_app();
        app.groups[0].run = RunState::Saved;

        app.running = true;
        assert!(app.remove_group(0).unwrap_err().contains("run"));
        assert!(app.clear_completed().is_err());

        app.running = false;
        app.groups[0].status = Status::Pending;
        assert!(app.remove_group(0).unwrap_err().contains("measuring"));
        assert_eq!(app.groups.len(), 1);
    }

    #[test]
    fn deleting_the_original_is_announced_in_the_log() {
        let mut app = ready_app();
        app.apply_run_event(RunEvent::RemovedOriginal {
            group: 0,
            path: PathBuf::from("music/album/album.flac"),
        });
        assert_eq!(last_log(&app), "Deleted original album.flac");
    }

    #[test]
    fn files_can_be_added_while_the_app_is_open() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("a.flac");
        let second = dir.path().join("b.flac");
        std::fs::write(&first, b"").unwrap();
        std::fs::write(&second, b"").unwrap();

        let mut app = ready_app();
        assert_eq!(app.groups.len(), 1);

        let jobs = app.add_paths(&[first]);
        assert_eq!(app.groups.len(), 2);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].group, 1, "the new job points at the new group");

        let jobs = app.add_paths(&[second]);
        assert_eq!(jobs[0].group, 2);
        assert_eq!(app.groups.len(), 3);
        assert_eq!(app.jobs.len(), 3);
        assert!(app.log.iter().any(|line| line == "Added b.flac"));
    }

    #[test]
    fn the_list_can_be_cleared_unless_a_run_is_writing() {
        let mut app = ready_app();
        app.running = true;
        assert!(app.clear().unwrap_err().contains("Wait for the run"));
        assert_eq!(app.groups.len(), 1);

        app.running = false;
        app.scroll = 4;
        app.clear().unwrap();
        assert_eq!(app.groups.len(), 0);
        assert_eq!(app.jobs.len(), 0);
        assert_eq!(app.scroll, 0);
    }
}
