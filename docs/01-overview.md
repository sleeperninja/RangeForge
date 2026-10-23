# 01 - Overview

## In one paragraph

RangeForge is a desktop and command-line tool for people with big music collections. You drop in albums (including one-big-file-plus-CUE albums) and single tracks. It measures how loud each track is, shows you what it would change, lets you adjust the target volume and output format, and then writes finished files with correct tags. You see the plan before anything is written.

## Who it is for

Right now: **one person**. A power user with a video-editing background, a large library (stereo albums plus a 206 GB multichannel collection from SACD, DVD-A and Blu-ray), and a fast machine. If it later helps others, good, but no decision is made for them yet.

## The problem

- An album takes 45-75 minutes across four or more apps.
- Volume differs track to track, which is tiring to listen to.
- The same metadata steps repeat for every file.
- The existing PowerShell volume script is slow enough to run overnight.

## What it does

| Step | What happens |
| --- | --- |
| Drop | Files and CUE sheets are recognised. CUE albums become a parent row with tracks under it. |
| Analyse | Each source file is decoded once. Loudness (LUFS), true peak (dBTP) and loudness range are measured per track. |
| Preview | The table shows the gain each track would get toward your target, with a peak cap so nothing clips. |
| Tag | Optional lookups fill genres and the "Radio" tag (see the FAQ for the sources). |
| Run | Tracks are split, gained and encoded to your chosen format and written using your file strategy. |

## Output formats you actually use

- **ALAC** is the main output, for the iTunes library.
- **FLAC** for multichannel and for Plex.
- **WAV/PCM** for video-editing proxies.
- **AAC** for very long recordings.
- Dolby Digital Plus is parked until encoders mature.

## Inputs

Anything FFmpeg or our decoders can read, including DSD and MLP. DSD converts using the fallback format set in Settings.

## Words you will see

- **CUE album:** one big audio file plus a small `.cue` text file that says where each song starts.
- **Loudness (LUFS):** how loud something sounds to a person. Lower numbers are quieter, and every song is moved toward the same target.
- **True peak (dBTP):** the highest point the sound reaches. Pushing it too high causes distortion.
- **Range:** how much the loudness rises and falls within a song.
- **Codec / container:** the way audio is compressed (codec, like ALAC) and the file type that holds it (container, like `.m4a`).
- **Sample rate, bit depth, channels:** the sound's detail level, its precision, and how many speakers it is for (2 is stereo, 6 is 5.1).
- **DSD:** a rare high-end format that has to be converted to something else to be edited.
- **Preset:** a saved set of output settings you can pick again.
- **Run:** do the real work, writing the new files.

## What it deliberately does not do

- No plugin system.
- No workflow scripting (it would recreate the PowerShell problem).
- No cloud processing. Network lookups are limited to Last.fm, Discogs and the Billboard chart file.
- No beginner/intermediate UI modes. There is one expert layout.
- No accounts or multi-user features.

Each of these has its reasoning recorded in [04-decisions.md](04-decisions.md), and the list is open to review.

## How we know it works

The measuring stick is the **Led Zeppelin test**: drop a discography, get a list of songs with tags, review and edit, run, and end up with correct ALAC files ready for iTunes. Beyond that, the PowerShell script is gone from the workflow.

## Platforms

First class: **Windows x64** and **macOS Apple Silicon**. Built and released as well: macOS Intel, Linux (GNU), Windows ARM. See [06-development.md](06-development.md).
