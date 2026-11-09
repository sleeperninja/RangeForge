# Frequently asked questions

Short answers in plain words. For the full picture, see the [manual](MANUAL.md).

## The basics

**What is RangeForge?**
A program for people with big music collections. You drop in albums and songs, it measures how loud each one is, evens the volume out, splits CUE albums, fixes the tags and names, and writes clean files. You see the plan before anything is written.

**Is it free?**
Yes. If it saves you time, you can say thanks on Ko-fi ([ko-fi.com/sleeperninja42](https://ko-fi.com/sleeperninja42)). Nothing is locked behind it.

**Which computers does it run on?**
Windows is the main target and the one it is tested on. macOS (Apple Silicon and Intel) and Linux builds are set up but have not been tried by hand yet.

**Does it change my original files?**
No, not unless you tell it to. New files are written first, to hidden temporary names. Only when every song from a source was written without an error are they moved into place. If anything fails, they are deleted and your originals are exactly as they were. Deleting an original is a separate option, off by default, and it asks first.

## Volume

**What is "volume balancing"?**
Making songs play at the same *perceived* loudness, without distorting them. It is based on loudness standards (LUFS), not on peak level.

**Why not just "normalize"?**
Peak normalization makes each song's loudest instant hit the same level, so two songs can still sound very different in volume. Loudness balancing targets how loud a song *sounds*.

**Does it change the dynamics of my music?**
No. It applies one gain to each song (a single volume change) and stops short of your peak ceiling. It never compresses or limits the sound.

**What are LUFS, LRA, DR and true peak?**
- **LUFS:** how loud it sounds to a person, as used by broadcasters and streaming services.
- **LRA (loudness range):** how much the loudness varies across a song. This is the Range column.
- **DR:** a related but different measure from the TT DR Meter. RangeForge does not show it.
- **True peak (dBTP):** the real highest level, including between samples. It matters for avoiding distortion.

**Why is a song quieter than my target, with a `*`?**
Its loudest moment would have gone over your peak ceiling if it were raised any more. The ceiling wins, so the song stays a little quieter. That is the setting doing its job. If you do not mind the risk, turn Peak Protection off.

**What happens to peaks above 0 dB when Peak Protection is off?**
It depends on the format. 16-bit and 24-bit WAV, FLAC and ALAC clip them flat. **32-bit float WAV keeps them exactly**, so you can lower the volume later and find them intact. MP3, AAC and Opus hold them in the file, but many players clip them on playback. When 32-bit float WAV is chosen, Peak Protection switches itself off for that output, since there is nothing to protect. The manual has the details.

## Formats

**Which format should I choose?**
- **ALAC:** the main choice for an iTunes or Apple library.
- **FLAC:** for surround, for Plex, and for archives. Compression 0 to 8 only changes size and time, never the sound.
- **WAV:** for editing. 32-bit float WAV is the one that can keep peaks above 0 dB.
- **AAC, MP3, Opus:** lossy, for small files. They throw away a lot of data permanently. Prefer ALAC or FLAC when you can.

**What do V0 to V4 mean for MP3?**
They are LAME's variable-rate settings: V0 is the best quality and V4 the smallest offered. They sit between the steady bitrates where their average rate falls (V2 is about 190 kbps, V0 about 245). Nothing below 128 kbps is offered.

**Why can't I pick another sample rate for Opus?**
Opus is always 48 kHz. MP3 stops at 48 kHz, and AAC at 96 kHz.

**Why is Quad missing for ALAC and MP3?**
ALAC cannot hold four channels in the quad layout (it turns them into five with a silent centre), and MP3 holds only two channels. FLAC, WAV, AAC and Opus can.

**Why won't it write my 5.1 file as MP3?**
MP3 holds two channels. Set Channels to Stereo or Mono and it works.

**Which surround downmix should I use?**
**Standard** for most music. **Standard + bass** adds the LFE channel, which on some albums gives a slightly denser low end. **Front pair only** is for discs where the front left and right already are the stereo mix. There is no centre boost on purpose.

**What about DSD?**
It is converted, because DSD cannot be written back out as it is. It is written in your Output Format. Only the sample rate (88.2 kHz to start with) and bit depth (24-bit) have a fallback in Settings, and each applies only where your Output Format says "source".

**Is the sample-rate conversion good?**
Yes. It was checked on real hi-res music against a very-high-precision reference, and the difference is below the 16-bit noise floor. Loudness and true peak are unchanged.

## Names, tags and the internet

**Does it go online by itself?**
No. Everything online starts off. Turn on **Metadata API** to let it look things up.

**What is the blue "Radio" tag?**
It marks songs that charted on the Billboard Hot 100. It needs no key. RangeForge keeps a local copy of the chart history and downloads it again only when a newer one exists.

**Do I need a Last.fm key?**
Only for album genres from Last.fm. RangeForge does not come with a key. Getting your own is free (Settings has a link: create one for an app on Last.fm). The key stays on your computer, in its credential store.

**Why can't it fetch tags for single songs?**
Last.fm does not give song tags to programs, and the page that has them is behind a bot check. So RangeForge opens that page in your browser, and you add the tags you want with **Add a tag…**. You can paste a list separated by commas, semicolons or line breaks.

**The names are wrong. What do I do?**
Right-click the song or album and choose **Album & track names…**. You can type names, fetch a track list from Last.fm, or paste one from a store page. It also copes with long vinyl-download names (barcode, track number, artist, title, ISRC). Names that are only a guess from the file name show in italics.

**Does it change where a CUE album is cut?**
No. The split points and track numbers come from the disc rip and are never changed. Only the words (titles, artist, album) can be replaced.

## Settings and trouble

**Where are my settings?**
In the program's own folder under your user profile (on Windows, `%APPDATA%\RangeForge`): `settings.json`, `presets.json` and the measurement cache. Deleting `settings.json` puts everything back to the defaults.

**Is there an Auto Mode that runs by itself?**
No, on purpose. With conversions and compression in the mix, a run that starts by itself does too much at once. You look at the table, then press Run.

**It said something I do not understand.**
Open the log (**Show Logs**). It says what the program is doing, step by step, including exactly what was sent to any online service.

**How do I report a bug?**
Open an issue on the project page: [github.com/sleeperninja/RangeForge/issues](https://github.com/sleeperninja/RangeForge/issues). Say what you did, what you expected, and include the log.
