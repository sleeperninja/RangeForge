//! Encoding one track by feeding raw audio to an FFmpeg process.

use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
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
    /// The name of the channel layout to give FFmpeg for the raw input, such as `quad`, when the
    /// count alone would be taken for another layout (four channels are taken for 4.0, which has
    /// a centre and not two backs). `None` leaves it to the count.
    pub channel_layout: Option<&'static str>,
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
    if let Some(layout) = spec.channel_layout {
        push("-channel_layout");
        push(layout);
    }
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
    // One resampler does both jobs, so the audio is only converted once: a new sample rate,
    // and, when going down to 16 bits, dither on the way.
    let mut resampler: Vec<String> = Vec::new();
    resampler.extend(spec.format.resample_options(spec.info.sample_rate));
    if spec.format.needs_dither(
        spec.info.bits_per_sample,
        spec.gain_db,
        spec.info.mixed || resampler.iter().any(|option| option.starts_with("osr=")),
    ) {
        resampler.push(format!(
            "osf={}:dither_method=triangular",
            spec.format.sample_format(bits)
        ));
    }
    if !resampler.is_empty() {
        push("-af");
        push(&format!("aresample={}", resampler.join(":")));
    }
    for argument in spec.tags.ffmpeg_args(spec.format) {
        push(&argument);
    }
    // ID3 version 2.3 is the one every player, tagger and car stereo reads.
    if spec.format.codec == crate::format::Codec::Mp3
        && spec.format.container == crate::format::Container::Native
    {
        push("-id3v2_version");
        push("3");
    }
    push("-f");
    push(spec.format.muxer());
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
        let mut child = ffmpeg
            .command()
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
    use crate::format::{BitDepth, Codec, Container, SampleRate};

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
                mixed: false,
            },
            format,
            gain_db,
            output,
            tags,
            metadata_from,
            channel_layout: None,
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
    fn a_new_sample_rate_adds_one_resampler_and_shares_it_with_the_dither() {
        let tags = Tags::default();
        let output = Path::new("out.part");
        let filter = |format: OutputFormat, gain: f64, bits: Option<u32>| {
            let args = strings(&build_args(&spec(format, gain, bits, &tags, output, None)));
            args.iter()
                .position(|a| a == "-af")
                .map(|at| args[at + 1].clone())
        };
        let alac_96 =
            OutputFormat::new(Codec::Alac, BitDepth::Source).with_sample_rate(SampleRate::Hz96000);

        // 24-bit source at 0 dB: only the new rate.
        assert_eq!(
            filter(alac_96, 0.0, Some(24)).as_deref(),
            Some("aresample=osr=96000:filter_size=64:phase_shift=14:cutoff=0.97")
        );
        // 16-bit source with a gain: the new rate and the dither in the same filter.
        assert_eq!(
            filter(alac_96, -3.2, Some(16)).as_deref(),
            Some(
                "aresample=osr=96000:filter_size=64:phase_shift=14:cutoff=0.97:osf=s16p:dither_method=triangular"
            )
        );
        // The source's own rate asked for: nothing to do.
        let alac_44 =
            OutputFormat::new(Codec::Alac, BitDepth::Source).with_sample_rate(SampleRate::Hz44100);
        assert_eq!(filter(alac_44, 0.0, Some(24)), None);
    }

    #[test]
    fn a_16_bit_file_at_0_db_is_dithered_when_the_signal_was_converted() {
        let tags = Tags::default();
        let output = Path::new("out.part");
        let alac = OutputFormat::new(Codec::Alac, BitDepth::Source);
        let filter = |format: OutputFormat, mixed: bool| {
            let mut spec = spec(format, 0.0, Some(16), &tags, output, None);
            spec.info.mixed = mixed;
            let args = strings(&build_args(&spec));
            args.iter()
                .position(|a| a == "-af")
                .map(|at| args[at + 1].clone())
        };
        // Untouched: nothing to dither, so the file stays bit for bit what it was.
        assert_eq!(filter(alac, false), None);
        // The channels were mixed: every sample is a new value, so it is dithered.
        assert_eq!(
            filter(alac, true).as_deref(),
            Some("aresample=osf=s16p:dither_method=triangular")
        );
        // A new sample rate on a 16-bit file at 0 dB: the same.
        let at_48 = alac.with_sample_rate(SampleRate::Hz48000);
        let resampled = filter(at_48, false).unwrap();
        assert!(resampled.starts_with("aresample=osr=48000:"), "{resampled}");
        assert!(
            resampled.ends_with("osf=s16p:dither_method=triangular"),
            "{resampled}"
        );
        // 24-bit output is never dithered, mixed or not.
        let wide = OutputFormat::new(Codec::Flac, BitDepth::TwentyFour);
        assert_eq!(filter(wide, true), None);
    }

    #[test]
    fn quad_is_named_to_ffmpeg_and_other_layouts_are_left_to_the_count() {
        let tags = Tags::default();
        let output = Path::new("out.part");
        let format = OutputFormat::new(Codec::Flac, BitDepth::Source);
        let input = |channels: usize, layout: Option<&'static str>| {
            let mut spec = spec(format, 0.0, Some(24), &tags, output, None);
            spec.info.channels = channels;
            spec.channel_layout = layout;
            let args = strings(&build_args(&spec));
            let at = args.iter().position(|a| a == "-ac").unwrap();
            // The input options as far as the input file.
            args[at..args.iter().position(|a| a == "-i").unwrap()].to_vec()
        };
        // The layout goes right after the channel count, before the input.
        assert_eq!(
            input(4, Some("quad")),
            ["-ac", "4", "-channel_layout", "quad"]
        );
        // Without one, FFmpeg takes it from the count: four would be 4.0 and six 5.1.
        assert_eq!(input(4, None), ["-ac", "4"]);
        assert_eq!(input(6, None), ["-ac", "6"]);
        assert_eq!(input(2, None), ["-ac", "2"]);
    }

    #[test]
    fn matroska_output_names_the_container_and_uses_vorbis_style_tags() {
        let tags = Tags {
            title: Some("A".to_string()),
            album_artist: Some("B".to_string()),
            track: Some(3),
            track_total: Some(9),
            ..Tags::default()
        };
        let output = Path::new("out.part");
        let format =
            OutputFormat::new(Codec::Alac, BitDepth::Source).with_container(Container::Mka);
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
            .take(3)
            .rev()
            .map(String::as_str)
            .collect();
        assert_eq!(tail, ["-f", "matroska", "out.part"]);
        assert!(args.contains(&"albumartist=B".to_string()), "{args:?}");
        assert!(args.contains(&"tracknumber=3".to_string()), "{args:?}");
        assert!(args.contains(&"tracktotal=9".to_string()), "{args:?}");
        // The codec is still ALAC, just in a different wrapper.
        assert!(args.contains(&"alac".to_string()));
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
