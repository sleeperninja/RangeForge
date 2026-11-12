# RangeForge Roadmap

*What works, what's next, and what we've set aside. Updated 11 Nov 2026, at version 1.0.0.*

How to use the program, and how it decides things, is in the [manual](MANUAL.md). Common questions are in the [FAQ](FAQ.md).

---

## Where we are

RangeForge does its main job today. Drop in an album or a pile of files, and it measures them once, shows you what *would* happen, then writes clean files with the volume evened out. The desktop window is the real product; the terminal version is mostly for development. It is free, with an optional Ko-fi link for people who want to say thanks.

### Works today

- **Measure once, show a table.** Loudness, true peak and range, cached on disk so dropping the same album again is instant. Changing the target updates the table straight away.
- **Split CUE albums** into songs, and write **ALAC, FLAC, WAV, AAC, MP3 or Opus** (plus DSD, converted through FFmpeg). Bitrate pickers for the lossy ones (MP3 includes V0 to V4), FLAC compression 0 to 8, and 32-bit float WAV.
- **One gain per song, nothing else.** No compression or limiting, and never past the true-peak ceiling. With 32-bit float WAV and MP3, peaks above 0 dB are kept.
- **Convert the sound** when you ask: sample rate, bit depth (with dither), and channels (Quad, Stereo or Mono, with three downmix styles), always measured on the sound that is actually written.
- **File Strategy:** next to the original, in a subfolder you name (`_forged` to start), or all in one folder. Numbering, renaming the old file, or overwriting when a name is taken, and optionally deleting the original once everything is safely written.
- **Names from patterns,** for CUE tracks and single files. **Album & track names…** fixes the artist, album and each title in one dialog, from the album's track list on Last.fm or pasted from a store page. It copes with long vinyl-download names, and shows its guesses in italics.
- **Genres:** from the album's Last.fm tags (with your own free key), plus a blue **Radio** tag for songs that charted on the Billboard Hot 100. Song-level tags are added by hand from Last.fm's tags page.
- **Presets, and the window remembers** your File Strategy, Output Format, Volume settings and file name patterns between sessions.
- **About window** with the version, the manual, downloads and the Ko-fi link.

---

## Still to check after 1.0

1. **Try it on a Mac** (Apple Silicon and Intel) **and on a Linux machine.** Both build, and neither has been run by hand yet.
2. **Uninstall leaves nothing surprising.** Settings, presets and the measurement cache are kept on purpose; the saved Last.fm key stays until you press Forget.

Everything else on the first-release list (every codec, 5.1 downmix, File Strategy, names and tags, a run on real music, the Windows installer, the licence and the Manual PDF) is done.

---

## Next, probably 1.0.1

- **A CPU limit** in Settings, with the default being everything the machine has. Choose less when you want to keep working while a long run goes.

---

## On demand: built if people ask

Things the author would not use, but other people might. They change the sound or the workflow beyond RangeForge's one-gain promise, so each would be a switch that is **off by default**, kept out of the normal path, and would say exactly which songs it touched. We think ahead about how, and build when there is demand.

- **An opt-in limiter** for songs that Peak Protection holds below the target. The way to do it is the gentlest one: change the sound as little as possible, for as long as possible, easing into the peak instead of cutting at it. A choice of strategies is possible, but only a few, and each described in plain words.
- **Loudness that reshapes the waveform** (the compress-to-a-target style of "normalizing"). RangeForge does not do this today, on purpose.
- **A different preset per album in one run,** for processing a whole library for several targets in a single pass. Not before the single-preset run has had its bugs shaken out.

---

## Later, when it gets in the way

- **A disc number in file names.** For albums on several discs, a `{disc}` variable that is empty on a one-disc album. Because a missing value already takes its neighbouring separator with it, `{disc:02}-{track:02} - {title}` would give `02-07 - Title` on a three-disc set and `07 - Title` on a single disc, with no IF syntax. It needs the disc number read from the file's tags (and from a CUE sheet or a folder called "Disc 2"), and the disc count from the tags or from the albums loaded together.
- **Say more on an album's row** about where it came from, such as the source codec and channel count, or its folder. Today a stereo and a surround copy of one album look alike until you open them.
- **Rename from the tags,** as a File Strategy choice next to *keep the original name*. To decide: it should warn rather than produce names worse than the originals when a name is only a guess.
- **A "will clip" mark** on songs that a whole-number format (16/24-bit, FLAC, ALAC) would clip when Peak Protection is off.
- **Mix 7.1 and bigger files down** to quad, stereo or mono, where there is no technical reason not to. Today anything above six channels is written as it is, so a big movie-style rip (say 9.1 LPCM) comes out unchanged. Until then, say on a file's row when it cannot be mixed.
- **A SoX-quality resampler** option, using the libsoxr library (LGPL, the same terms as the rest of FFmpeg, and already inside the Windows FFmpeg we ship; the Mac and Linux builds would need it added), not the SoX program, which is GPL. A steeper filter, at a difference no one can hear. DSD conversion would take more time, which is fine. Worth looking at how audio is held in memory for savings at the same time.
- **Use each song's length** when lining songs up with a track list.
- **Benchmarks,** and a `--json` output and HTML report from the terminal tool.
- **Multichannel loudness** (4 to 8 channels), and MLP input.
- **Signing the installers.** Windows showed no publisher warning in testing. The Mac will probably still need the right-click → Open step, which is untried.
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
- **Remembering hand edits between sessions.** Names and tags you edit are kept until you close the program. That only matters when the same files are processed again and again, which is mostly a side effect of testing, so it is not worth the work.
- **An Auto Mode** that runs by itself. With conversions and compression in the mix, a run that starts by itself does too much at once.
- **A centre boost** in surround downmixes, and Dolby-style matrix encodings.

---

## Rough edges to keep in mind

- **Album tags can be wrong for two bands with the same name.** If a song isn't found, the lookup retries with "The" added or removed, which could pick up a different band.
- **Lossy files with peaks above 0 dB** may clip on playback, depending on the player.
- **The Radio tag's chart data** comes from a public community-maintained file. It is a convenience, not an official source.
