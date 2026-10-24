//! Encoding one track by feeding raw audio to an FFmpeg process.

use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread::{self, JoinHandle};

use rangeforge_audio::decode::StreamInfo;

use crate::ExportError;
use crate::ffmpeg::Ffmpeg;
use crate::format::OutputFormat;
use crate::tags::Tags;

/// Everything FFmpeg needs to know to encode one track.
#[derive(Debug)]
pub struct EncodeSpec<'a> {
    pub info: StreamInfo,
    pub format: OutputFormat,
    pub gain_db: f64,
    /// Where to write. The container is chosen by the codec, not by this file's name.
    pub output: &'a Path,
    pub tags: &'a Tags,
    /// Copy this file's tags first (for single files), then apply `tags` over them.
    pub metadata_from: Option<&'a Path>,
}

/// The volume change as a multiplier. 0 dB is exactly 1.0, which leaves samples untouched.
#[allow(clippy::cast_possible_truncation)]
pub fn linear_gain(gain_db: f64) -> f32 {
    10_f64.powf(gain_db / 20.0) as f32
}

/// The FFmpeg command line for one track.
pub fn build_args(spec: &EncodeSpec<'_>) -> Vec<OsString> {
    let bits = spec.format.resolved_bits(spec.info.bits_per_sample);
    let mut args: Vec<OsString> = Vec::new();
    let mut push = |text: &str| args.push(OsString::from(text));

    for fixed in ["-hide_banner", "-loglevel", "error", "-y"] {
        push(fixed);
    }
    // The audio arrives on standard input as raw 32-bit floats.
    for fixed in ["-f", "f32le"] {
        push(fixed);
    }
    push("-ar");
    push(&spec.info.sample_rate.to_string());
    push("-ac");
    push(&spec.info.channels.to_string());
    for fixed in ["-i", "pipe:0"] {
        push(fixed);
    }
    if let Some(source) = spec.metadata_from {
        push("-i");
        args.push(source.as_os_str().to_owned());
    }
    let mut push = |text: &str| args.push(OsString::from(text));
    push("-map");
    push("0:a");
    if spec.metadata_from.is_some() {
        push("-map_metadata");
        push("1");
    }
    for argument in spec.format.encoder_args(bits) {
        push(&argument);
    }
    if spec
        .format
        .needs_dither(spec.info.bits_per_sample, spec.gain_db)
    {
        push("-af");
        push(&format!(
            "aresample=osf={}:dither_method=triangular",
            spec.format.sample_format(bits)
        ));
    }
    for argument in spec.tags.ffmpeg_args(spec.format.codec) {
        push(&argument);
    }
    push("-f");
    push(spec.format.codec.muxer());
    args.push(spec.output.as_os_str().to_owned());
    args
}

/// A running FFmpeg that is encoding one track.
#[derive(Debug)]
pub struct Encoder {
    child: Child,
    stdin: Option<ChildStdin>,
    stderr: Option<JoinHandle<String>>,
    gain: f32,
    scratch: Vec<u8>,
    output: PathBuf,
    track: usize,
    succeeded: bool,
}

impl Encoder {
    /// Starts FFmpeg for track number `track` (used only in error messages).
    pub fn start(
        ffmpeg: &Ffmpeg,
        track: usize,
        spec: &EncodeSpec<'_>,
    ) -> Result<Self, ExportError> {
        let mut child = Command::new(ffmpeg.path())
            .args(build_args(spec))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(ExportError::io("starting", ffmpeg.path()))?;
        let stdin = child.stdin.take();
        // FFmpeg's messages must be read as they come, or a full pipe would stall it.
        let stderr = child.stderr.take().map(|mut pipe| {
            thread::spawn(move || {
                let mut text = String::new();
                let _ = pipe.read_to_string(&mut text);
                text
            })
        });
        Ok(Self {
            child,
            stdin,
            stderr,
            gain: linear_gain(spec.gain_db),
            scratch: Vec::new(),
            output: spec.output.to_path_buf(),
            track,
            succeeded: false,
        })
    }

    /// Applies the gain to interleaved samples and sends them to FFmpeg.
    pub fn write(&mut self, samples: &[f32]) -> Result<(), ExportError> {
        self.scratch.clear();
        self.scratch.reserve(samples.len() * 4);
        for sample in samples {
            self.scratch
                .extend_from_slice(&(sample * self.gain).to_le_bytes());
        }
        let result = match self.stdin.as_mut() {
            Some(stdin) => stdin.write_all(&self.scratch),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "FFmpeg's input is already closed",
            )),
        };
        match result {
            Ok(()) => Ok(()),
            // Usually FFmpeg stopped early, and its own message says why.
            Err(error) => Err(self.failure(&error.to_string())),
        }
    }

    /// Closes FFmpeg's input and waits for it to finish writing the file.
    pub fn finish(mut self) -> Result<(), ExportError> {
        drop(self.stdin.take());
        let status = self.child.wait().map_err(ExportError::io(
            "waiting for FFmpeg to write",
            self.output.clone(),
        ))?;
        let messages = self.take_messages();
        if status.success() {
            self.succeeded = true;
            Ok(())
        } else {
            Err(ExportError::Encoder {
                track: self.track,
                message: describe(&messages, &format!("FFmpeg exited with {status}")),
            })
        }
    }

    fn take_messages(&mut self) -> String {
        self.stderr
            .take()
            .and_then(|handle| handle.join().ok())
            .unwrap_or_default()
    }

    fn failure(&mut self, fallback: &str) -> ExportError {
        drop(self.stdin.take());
        let _ = self.child.wait();
        let messages = self.take_messages();
        ExportError::Encoder {
            track: self.track,
            message: describe(&messages, fallback),
        }
    }
}

impl Drop for Encoder {
    /// An encoder that did not finish cleanly takes its half-written file with it.
    fn drop(&mut self) {
        if !self.succeeded {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = fs::remove_file(&self.output);
        }
    }
}

fn describe(messages: &str, fallback: &str) -> String {
    let messages = messages.trim();
    if messages.is_empty() {
        fallback.to_string()
    } else {
        messages.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{BitDepth, Codec};

    fn spec<'a>(
        format: OutputFormat,
        gain_db: f64,
        bits: Option<u32>,
        tags: &'a Tags,
        output: &'a Path,
        metadata_from: Option<&'a Path>,
    ) -> EncodeSpec<'a> {
        EncodeSpec {
            info: StreamInfo {
                sample_rate: 44_100,
                channels: 2,
                bits_per_sample: bits,
            },
            format,
            gain_db,
            output,
            tags,
            metadata_from,
        }
    }

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn zero_gain_is_exactly_one() {
        // Compared as bits: "exactly one" is the whole point, so no tolerance.
        assert_eq!(linear_gain(0.0).to_bits(), 1.0_f32.to_bits());
    }

    #[test]
    fn gain_converts_decibels_to_a_multiplier() {
        assert!((linear_gain(-6.0206) - 0.5).abs() < 1e-4);
        assert!((linear_gain(20.0) - 10.0).abs() < 1e-4);
        assert!((linear_gain(-20.0) - 0.1).abs() < 1e-5);
    }

    #[test]
    fn lossless_untouched_audio_has_no_dither() {
        let tags = Tags::default();
        let output = Path::new("out.part");
        let format = OutputFormat::new(Codec::Alac, BitDepth::Source);
        let args = strings(&build_args(&spec(
            format,
            0.0,
            Some(16),
            &tags,
            output,
            None,
        )));
        assert_eq!(
            args,
            [
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "f32le",
                "-ar",
                "44100",
                "-ac",
                "2",
                "-i",
                "pipe:0",
                "-map",
                "0:a",
                "-c:a",
                "alac",
                "-sample_fmt",
                "s16p",
                "-f",
                "ipod",
                "out.part",
            ]
        );
    }

    #[test]
    fn a_gain_on_16_bit_audio_adds_triangular_dither() {
        let tags = Tags::default();
        let output = Path::new("out.part");
        let format = OutputFormat::new(Codec::Alac, BitDepth::Source);
        let args = strings(&build_args(&spec(
            format,
            -3.2,
            Some(16),
            &tags,
            output,
            None,
        )));
        let at = args.iter().position(|a| a == "-af").expect("-af present");
        assert_eq!(args[at + 1], "aresample=osf=s16p:dither_method=triangular");
    }

    #[test]
    fn twenty_four_bit_output_is_not_dithered() {
        let tags = Tags::default();
        let output = Path::new("out.part");
        let format = OutputFormat::new(Codec::Flac, BitDepth::Source);
        let args = strings(&build_args(&spec(
            format,
            -3.2,
            Some(24),
            &tags,
            output,
            None,
        )));
        assert!(!args.contains(&"-af".to_string()), "{args:?}");
        assert!(args.contains(&"s32".to_string()), "{args:?}");
    }

    #[test]
    fn single_files_copy_their_tags_before_the_new_ones() {
        let tags = Tags {
            title: Some("New Title".to_string()),
            ..Tags::default()
        };
        let output = Path::new("out.part");
        let source = Path::new("song.flac");
        let format = OutputFormat::new(Codec::Alac, BitDepth::Source);
        let args = strings(&build_args(&spec(
            format,
            0.0,
            Some(16),
            &tags,
            output,
            Some(source),
        )));

        let second_input = args.iter().position(|a| a == "song.flac").unwrap();
        assert_eq!(args[second_input - 1], "-i");
        let map_metadata = args.iter().position(|a| a == "-map_metadata").unwrap();
        assert_eq!(args[map_metadata + 1], "1");
        // Explicit tags come after, so they win.
        let title = args.iter().position(|a| a == "title=New Title").unwrap();
        assert!(title > map_metadata);
        // Only audio from the pipe is used; the source's cover art is not pulled in.
        let map = args.iter().position(|a| a == "-map").unwrap();
        assert_eq!(args[map + 1], "0:a");
    }

    #[test]
    fn tags_and_muxer_come_last() {
        let tags = Tags {
            title: Some("A".to_string()),
            ..Tags::default()
        };
        let output = Path::new("out.part");
        let format = OutputFormat::new(Codec::Flac, BitDepth::Source);
        let args = strings(&build_args(&spec(
            format,
            0.0,
            Some(16),
            &tags,
            output,
            None,
        )));
        let tail: Vec<&str> = args
            .iter()
            .rev()
            .take(4)
            .rev()
            .map(String::as_str)
            .collect();
        assert_eq!(tail, ["title=A", "-f", "flac", "out.part"]);
    }
}
