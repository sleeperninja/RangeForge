# How RangeForge Decides Things

*The plain-words manual: the thinking behind what the program does, and why. Written 9 Nov 2026, for version 0.4.0. The dated record of each decision is in [docs/04-decisions.md](docs/04-decisions.md); the `D-` numbers below point there. Things marked **planned** are decided but not built yet.*

---

## The one promise

RangeForge changes how loud each song is, and nothing else about the music. It never squashes the dynamics, never limits the peaks, and never touches an original until every new file has been written safely.

Everything below follows from that.

---

## 1. How a song's volume is set

**It listens once.** When you drop files in, each one is decoded and measured a single time: its loudness (LUFS), its true peak, and its loudness range. The result is saved on disk against what the file *contains*, not its name or date, so a renamed or moved copy of the same album costs nothing the second time (D-11).

**Then it is just arithmetic.** You choose a target (for example −22 LUFS). Each song's gain is the target minus its measured loudness. Change the target and the table updates instantly, because nothing needs to be measured again.

**One gain per song.** The gain is applied once, to the sound in memory, before it is encoded. A song written at 0 dB is sample for sample identical to its source (D-12).

**The peak ceiling is the hard limit.** Raise a quiet song with a large gain and its loudest moments could go over full scale. So there is a ceiling (−0.6 dBTP to start with), and a song is never boosted past it. If a song runs out of headroom, it stays *below* the target and is marked `*`. That is the setting working as intended, not a fault: it is why someone asking for −10 LUFS with a 0 dB ceiling gets a wide spread of volumes.

**No compression, and an opt-in at most.** The program does not compress or limit by default. People have rightly objected to tools that change the shape of the waveform to hit a number, and this tool's whole point is to leave the music alone. If a limiter is ever added, it will be a switch that is off, that only touches songs the ceiling would otherwise hold back, only as much as needed, and that says exactly which songs it touched (D-36, **planned**).

---

## 2. When the sound is converted

Changing the sample rate, the bit depth, or the number of channels is not "volume work". These conversions change the samples out of necessity, and they are allowed alongside the gain-only rule (D-35). The order is always:

> **convert → measure the converted sound → apply the one gain → encode**

The reason: the gain and the peak ceiling have to be worked out on the sound that is actually written.

### Sample rate (built)

The choices are Original, 44.1, 48, 88.2, 96, 176.4 and 192 kHz (AAC stops at 96). Resampling is done by FFmpeg with a long, careful filter.

I checked this on real music, not only on test tones (D-37): on the Pink Floyd 24/96 files the audible band is within about −100 dB of a very-high-precision reference, which is the 16-bit noise floor, so nobody can hear a difference. The true peak after resampling is unchanged, and loudness lands exactly on target at every rate tried. A SoX-based resampler would give a steeper edge, but no audible gain, so it is an option and not a need.

### Bit depth and dither (built)

Going to 16 bits means rounding, and plain rounding adds a faint distortion that follows the music. Triangular dither replaces it with a steady, very quiet noise instead. So the program dithers whenever it has changed the signal and is writing 16 bits. A file left untouched at 0 dB is never dithered, so it stays bit-exact. 24-bit output needs no dither (D-15).

*One small gap:* a 16-bit file resampled at exactly 0 dB gain is not dithered yet. It almost never happens.

### Channels (**planned**, D-35)

Mixing six channels down to two is a lot like turning a colour photo into black and white. There is more than one honest way, and the results are not the same. So the choice has two parts.

**Channels:** Original, Quad, Stereo or Mono. It only ever reduces: asking for as many channels as the file has, or more, means "original", and nothing is upmixed. Files with more than 6 channels (7.1 and up) are always written as they are. Those are movie-style assets, leveled for volume and not downmixed, and a matrix processor does the downmix better than we would.

**Downmix** (shown only when Channels is not Original):

| Style | What it does | When it fits |
| --- | --- | --- |
| **Standard** (default) | Centre and surrounds go into left and right at −3 dB; the LFE is dropped | Keeps the balance of the original mix |
| **Standard + bass** | The same, with the LFE added at −3 dB | Rarely needed; barely changes anything on most music |
| **Front pair only** | Left and right as mixed; centre and surrounds discarded | When the front pair already *is* the stereo mix |

Quad keeps the four corners and folds the centre into the fronts. Mono is the average of the standard stereo mix.

**Why the downmix is measured on its own.** On real 5.1 music (Pink Floyd's two albums at 24/96, and I Robot from DSD), the standard stereo downmix came out between −0.6 and +1.6 LU of the six-channel loudness, and its true peak was 2.6 to 4.9 dB *higher*. The six-channel measurement simply cannot be trusted for a stereo file. So a downmix is measured after the mix, the gain and ceiling are worked out on that, and the table shows the result.

**What is deliberately not offered:**

- **A centre boost.** It makes the result sound very different, and "lossy" in a way that is not subtle compared with the accepted strategies. It is also inconsistent from one file to the next, because of how channels relate in a mix.
- **Dolby-style matrix encodings.** Not until someone asks for them.

---

## 3. Who a song is

Almost everything else (lookups, file names, tags) depends on knowing a song's artist, title and album. They come from the best source available, in this order, and **anything you type always wins**:

1. A **CUE sheet**, for songs split from an album image.
2. The **tags** in the file.
3. A **guess** from the file's name and folders. These are marked in italics in the list, so you know the name is a guess. The guesser understands the long vinyl-download names too (barcode, track number, artist, title, ISRC).
4. **Typed in by you** (D-25, D-29).

### The CUE sheet is the rip's, except for the words

A CUE sheet's **split points and track numbers** come from reading the disc, so they stay exactly as they are. The **words** in it (title, artist, album) are only starting values: the database entry that filled them in might be wrong, or have another disc's timings. What you type replaces the sheet's words in the written tags, the file names and the rows, and never changes the splits or the numbers (D-34).

### Naming many songs at once

**Album & track names…** is one dialog for an album and all its songs. You can fix the artist, album and album artist, and give each song a title, by typing or from the album's track list, from Last.fm or pasted from a store page. The list is lined up with the songs by title first, then by track number, then (if what is left is equal in number) by order. Each row says how it was paired, and the weak ones are marked. The program makes the first guess; you correct it, and nothing is applied until you press Save (D-33).

### Tags for individual songs

Last.fm's data about a *song's* tags is not available to a program: the API returns nothing for songs, and the page that has them sits behind a bot check. MusicBrainz and Wikidata were tried, and are too thin or too coarse. So the program opens the song's tags page in your own browser, and you add the ones you want with **Add a tag…**, which takes a list separated by commas, semicolons or line breaks (D-31, D-32).

### What the program checks and what it leaves to you

It splits, measures, plans, tags and names as well as it can from what it has, and makes correcting quick. It does not pretend the data is perfect. A disc can carry the wrong title, a database can carry the wrong album, and a file name can be anything. Those are human mistakes, and the right answer is to make them cheap to fix, not to guess harder.

---

## 4. What happens when you press Run

1. Each song is encoded to a hidden temporary file next to where it will end up.
2. Only after every song from a source has been written without a single error are the temporary files moved into place, following your File Strategy.
3. If anything fails, the temporary files are deleted and your originals are exactly as they were.

The File Strategy is three separate choices: where the new files go, what to do when a file is already there, and whether to delete the original afterwards (off by default; it only happens last, and only when everything was written) (D-13, D-14, D-21).

Your settings are remembered between sessions: the File Strategy, the Output Format, the volume settings and the file name patterns.

---

## 5. Where to look

| You want to know | Read |
| --- | --- |
| What is built and what is next | [ROADMAP.md](ROADMAP.md) |
| What a control does on screen | [docs/02-ui.md](docs/02-ui.md) |
| How it works inside | [docs/03-architecture.md](docs/03-architecture.md) |
| Why something was decided, and when to reopen it | [docs/04-decisions.md](docs/04-decisions.md) |
| How to build and test it | [docs/06-development.md](docs/06-development.md) |

*This manual explains; the decisions file records. When a decision changes how something works, both are updated, and this page links to decision numbers instead of repeating them.*
