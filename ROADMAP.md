# RangeForge Roadmap

*What works, what's next, and what we've set aside. Updated 9 Nov 2026, at version 0.6.8.*

How to use the program, and how it decides things, is in the [manual](MANUAL.md). Common questions are in the [FAQ](FAQ.md).

---

## Where we are

RangeForge does its main job today. Drop in an album or a pile of files, and it measures them once, shows you what *would* happen, then writes clean files with the volume evened out. The desktop window is the real product; the terminal version is mostly for development. It is free, with an optional Ko-fi link for people who want to say thanks.

### Works today

- **Measure once, show a table.** Loudness, true peak and range, cached on disk so dropping the same album again is instant. Changing the target updates the table straight away.
- **Split CUE albums** into songs, and write **ALAC, FLAC, WAV, AAC, MP3 or Opus** (plus DSD, converted through FFmpeg). Bitrate pickers for the lossy ones (MP3 includes V0 to V4), FLAC compression 0 to 8, and 32-bit float WAV.
- **One gain per song, nothing else.** No compression or limiting, and never past the true-peak ceiling. With 32-bit float WAV, peaks above 0 dB are kept.
- **Convert the sound** when you ask: sample rate, bit depth (with dither), and channels (Quad, Stereo or Mono, with three downmix styles), always measured on the sound that is actually written.
- **File Strategy:** next to the original, in a subfolder you name (`_forged` to start), or all in one folder. Numbering, renaming the old file, or overwriting when a name is taken, and optionally deleting the original once everything is safely written.
- **Names from patterns,** for CUE tracks and single files. **Album & track names…** fixes the artist, album and each title in one dialog, from the album's track list on Last.fm or pasted from a store page. It copes with long vinyl-download names, and shows its guesses in italics.
- **Genres:** from the album's Last.fm tags (with your own free key), plus a blue **Radio** tag for songs that charted on the Billboard Hot 100. Song-level tags are added by hand from Last.fm's tags page.
- **Presets, and the window remembers** your File Strategy, Output Format, Volume settings and file name patterns between sessions.
- **About window** with the version, the manual, downloads and the Ko-fi link.

---

## Before the first release

These need a person with the window open:

1. **Click through the newest build.** Everything below is covered by tests, but nobody has used it by hand yet:
   - every codec in the Output Format panel: Bitrate, Compression, Bit Depth and Sample Rate go empty and dimmed when they do nothing; Opus shows 48 kHz; 32-bit float appears for WAV only and Peak Protection dims with it
   - Channels and Downmix on a real 5.1 album; Quad refused for ALAC and MP3; MP3 with 5.1 refuses and says why
   - File Strategy labels fit, the `_forged` default, and settings survive closing mid-edit
   - Album & track names, song tags page and Add a tag
   - Clear completed / Clear all under the Run button, the About window and its links, the Last.fm key link in Settings
2. **Run it on real music:** a CUE album, vinyl-download singles, a 5.1 FLAC, a DSD file. Play the results in two players, and check tags and track numbers.
3. **Build the installers on GitHub** (set up, never run), and install on a clean Windows machine. macOS and Linux are untested.
4. ~~Add a licence file~~ Done: MIT, in [LICENSE](LICENSE).
5. **Attach the manual as a PDF** to each release (the About window already links to the downloads page).

---

## Later, when it gets in the way

- **Rename from the tags,** as a File Strategy choice next to *keep the original name*. To decide: it should warn rather than produce names worse than the originals when a name is only a guess.
- **Remember hand edits between sessions.** Names and tags you edit are kept only until you close the program.
- **A "will clip" mark** on songs that a whole-number format (16/24-bit, FLAC, ALAC) would clip when Peak Protection is off.
- **An opt-in limiter** for songs peak protection holds below the target. Designed, not built.
- **Say on a file's row when it cannot be mixed** (more than six channels, or already fewer).
- **A SoX-based resampler** option: a steeper filter and a faster DSD stage, at a gain no one can hear. Whether it is worth the bundling work is undecided.
- **Use each song's length** when lining songs up with a track list.
- **Benchmarks,** and a `--json` output and HTML report from the terminal tool.
- **Multichannel loudness** (4 to 8 channels), and MLP input.
- **A CPU limit** in Settings, so a big batch doesn't take over the machine.
- **Signing the installers.** Windows says the publisher is unknown, and the Mac needs the right-click → Open step.
- **Small window niceties:** right-click to copy table text, a highlight on an album heading when all its songs are picked.
- **Read ahead while decoding,** to hide the network read time on a NAS.
- **Keep the app's own data from growing forever:** cap the on-screen log and prune the cache.

---

## Parked ideas

Not forgotten, just not now.

- **Guess genre from the sound itself.** The only idea that could *hear* the reggae in "D'yer Mak'er" without any website. Bigger to build and less certain.
- **Dolby Digital Plus output,** once the encoder support is there.
- **Beginner and intermediate modes.** One expert layout for now.
- **The name "RangeForge."** Worth a second thought before anything goes public, in case it's too close to another audio product.

---

## Tried and set aside

So we don't repeat the experiments.

- **Song tags from Last.fm's API.** It answers with nothing for songs, even famous ones.
- **Reading Last.fm's song tags page automatically.** It has the good stuff, but sits behind a bot check. So the app opens the page in your browser and you paste the list into **Add a tag…**.
- **MusicBrainz, Wikidata, Discogs, Spotify** for song-level genres. MusicBrainz is as empty as Last.fm for songs, Wikidata is coarse, Discogs tags the release and not the song, and Spotify only tags artists now.
- **An Auto Mode** that runs by itself. With conversions and compression in the mix, a run that starts by itself does too much at once.
- **A centre boost** in surround downmixes, and Dolby-style matrix encodings.

---

## Rough edges to keep in mind

- **Album tags can be wrong for two bands with the same name.** If a song isn't found, the lookup retries with "The" added or removed, which could pick up a different band.
- **Lossy files with peaks above 0 dB** may clip on playback, depending on the player.
- **The Radio tag's chart data** comes from a public community-maintained file. It is a convenience, not an official source.
