# RangeForge Roadmap

*The friendly version: what works, what's next, and what we've decided not to do. Updated 9 Nov 2026, at version 0.3.1.*

The detailed phase list, with the technical wording, lives in [docs/05-roadmap.md](docs/05-roadmap.md) and the reasons behind every decision are in [docs/04-decisions.md](docs/04-decisions.md). This page is the one to read over coffee.

---

## Where we are

RangeForge does its main job today. Drop in an album or a pile of files, and it measures them once, shows you what *would* happen, and then writes clean files with the volume evened out. It already replaces most of the old toolchain. The desktop window is the real product, and the terminal version is mostly for development.

### Works today

- **Measure once, show a table.** Loudness, true peak and range, cached on disk so dropping the same album again is instant.
- **Change the target volume** and the table updates straight away.
- **Split CUE albums** into songs, and write ALAC, FLAC, WAV or AAC (plus DSD handled through FFmpeg).
- **One gain per song, nothing else.** No compression or limiting, and never past the true-peak ceiling.
- **File Strategy:** next to the original, in a subfolder you name, or all in one folder. Numbering, renaming the old file, or overwriting when a name is taken, and optionally deleting the original once everything is safely written.
- **Names from patterns,** for CUE tracks and for single files (`{artist}-{album}_{track:02}-{title}` and friends).
- **Tags:** read from the file, the CUE sheet, or guessed from the file and folder names (including the long vinyl-download names, like the Doves ones). Edit names and tags by hand: one dialog, **Album & track names…**, for the artist, album, album artist and each song's title; **Add a tag…** for tags. For a CUE album the split points and track numbers stay the rip's, and the names you type replace the sheet's.
- **Name many songs at once** by lining them up with the album's track list, from Last.fm or pasted from a store page. It makes a first guess, shows how each song was paired, and lets you change any row before anything happens.
- **Rows show the name each song will be written as,** with the original file name on hover, and a guess in italics, so you can see the result before pressing Run.
- **Genres:** from the album's Last.fm tags, plus a blue **Radio** tag for songs that charted on the Billboard Hot 100.
- **The window remembers** your File Strategy, Output Format, Volume settings and file name patterns between sessions.

---

## Next up

These are the things on the plate, roughly in the order they make sense.

1. **Give the newest build a real click-through.** A few things are only covered by unit tests, not by someone using the window:
   - right-click → **Open song tags page on Last.fm**, and pasting a list into **Add a tag…**
   - change codec, sample rate, target and the subfolder name, close mid-edit, reopen, and check everything came back
   - drop the Some Cities folder: clean titles and track numbers, and dropping the album again shouldn't add its `_rangeforged` output
   - right-click an album or any song → **Album & track names…**: fix the album's names, get Last.fm's track list and then a pasted one, and change a row by hand; on a CUE album, check the written names and that the track numbers stay
   - the rows' new names: italic for a guess, the original name on hover
2. **Rename from the tags, as a File Strategy choice.** Today the single-file naming pattern lives in Settings and applies to every run. The idea is a visible choice next to the others: **Keep the original name** or **Rename from the tags**.
   *To decide:* should it refuse (or at least warn) when a name is only a guess from the file name, so it never produces worse names than the originals? It also needs a check that two new files can't pick the same name in one run.
3. **Channels control.** Choose how many channels the output has: original, stereo downmix, mono, maybe left-only or right-only for a broken rip.
   *To decide first:* mixing 5.1 down to stereo changes more than a gain, which our "gain only" rule (D-12) doesn't cover. It needs its own decision, the way resampling got one.
4. **Check the downsampling.** The sample rate and bit depth choices exist (44.1 to 192 kHz), but nobody has checked the quality yet: how it resamples, and that dropping 24-bit to 16-bit is dithered cleanly. A listening-free test (nulls and loudness checks) would settle it.
5. **Remember hand edits between sessions.** Names and tags you edit are kept only until you close the program. They could be saved against the file.

---

## Later, when it gets in the way

- **Benchmarks** to back up the speed claims (a first manual comparison is done; proper `criterion` benchmarks aren't).
- **`--json` output and a simple HTML report** from the terminal tool.
- **Multichannel loudness** (4 to 8 channels), FLAC for Plex, and DSD/MLP input, aimed at the 206 GB surround collection.
- **Use each song's length when lining songs up with a track list.** Last.fm gives the length of most tracks, which would be a good check for bonus tracks and odd editions. The measurements need to carry the length first.
- **A CPU limit** in Settings, so a big batch doesn't take over the machine.
- **Signing the installers.** Right now Windows says the publisher is unknown, and the Mac needs the right-click → Open dance. Also the first real run of the installer build on GitHub (the docs say it hasn't run yet).
- **Small window niceties:** right-click to copy table text, and a highlight on an album heading when all its songs are picked.
- **Read ahead while decoding,** to hide the network read time on the NAS (cold reads are noticeably slower than warm ones).
- **Keep the app's own data from growing forever:** cap the on-screen log and prune the cache.

---

## Parked ideas

Not forgotten, just not now.

- **Guess genre from the sound itself.** The only idea that could actually *hear* the reggae in "D'yer Mak'er" without any website. Bigger to build and less certain, so it waits.
- **Peaks above 0 dBFS, and reaching a loudness target without losing peaks.** To be discussed together with any compression, if that ever comes up.
- **Dolby Digital Plus output,** once the encoder support is there.
- **Beginner and intermediate UI modes.** One expert layout for now.
- **The name "RangeForge."** Worth a second thought before anything goes public, in case it's too close to another audio product.

---

## Tried and set aside

So we don't repeat the experiments.

- **Song tags from Last.fm's API.** It answers with nothing for songs, even famous ones.
- **Reading Last.fm's song tags page automatically.** The page does have the good stuff (reggae for D'yer Mak'er, synthpop for Shot at the Night), but it sits behind a bot check. So the app opens the page in your browser and you paste the list into **Add a tag…** instead.
- **MusicBrainz, Wikidata, Discogs, Spotify** for song-level genres. MusicBrainz is as empty as Last.fm for songs, Wikidata is coarse and missed the reggae, Discogs tags the release and not the song, and Spotify only tags artists now.

---

## Rough edges to keep in mind

- **Album tags can be wrong for two bands with the same name.** If a song isn't found, the lookup retries with "The" added or removed, which could pick up a different band called Killers.
- **The pink "song-level" tag colour has no source right now.** Tags you add by hand are green, and that's what the song tags page feeds.
- **Tags you add by hand last for the session only** (see item 6 above).
