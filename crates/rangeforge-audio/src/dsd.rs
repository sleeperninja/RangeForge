//! DSD files (`.dsf` and `.dff`).
//!
//! DSD is one-bit audio at millions of samples a second, which nothing here can measure or
//! write as it is. FFmpeg turns it into ordinary audio at a rate we choose, and that audio is
//! then handled like any other file. This module reads the file's header (how many channels,
//! how fast the DSD runs) and streams FFmpeg's output as blocks.
//!
//! FFmpeg's location is lent by whoever finds it, through [`use_ffmpeg`], so this crate does
//! not need to know how FFmpeg is looked for.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::AudioError;
use crate::decode::StreamInfo;

/// The rate DSD is turned into for measuring. It is fixed, so a measurement never depends on
/// what the files are later written as, and the saved measurements stay valid.
pub const ANALYSIS_RATE: u32 = 88_200;

/// Facts from a DSD file's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DsdInfo {
    pub channels: usize,
    /// How fast the one-bit audio runs, such as 2 822 400 (DSD64).
    pub dsd_rate: u32,
}

impl DsdInfo {
    /// The usual name for the speed: `DSD64`, `DSD128`, and so on.
    pub fn name(self) -> String {
        format!("DSD{}", self.dsd_rate / 44_100)
    }
}

/// True for the DSD file types, by extension.
pub fn is_dsd(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("dsf") || e.eq_ignore_ascii_case("dff"))
}

type Launcher = Arc<dyn Fn() -> Command + Send + Sync>;

static FFMPEG: Mutex<Option<Launcher>> = Mutex::new(None);

/// Says how to start FFmpeg, which DSD files need. `launcher` returns a fresh command that
/// runs it, ready for arguments.
pub fn use_ffmpeg(launcher: impl Fn() -> Command + Send + Sync + 'static) {
    if let Ok(mut slot) = FFMPEG.lock() {
        *slot = Some(Arc::new(launcher));
    }
}

fn ffmpeg_command() -> Result<Command, AudioError> {
    let launcher = FFMPEG.lock().ok().and_then(|slot| slot.clone());
    launcher
        .map(|launch| launch())
        .ok_or_else(|| AudioError::Dsd("DSD files need FFmpeg, and it was not found".to_string()))
}

/// Reads the channel count and DSD speed from a `.dsf` or `.dff` header.
pub fn read_info(path: &Path) -> Result<DsdInfo, AudioError> {
    let mut head = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(64 * 1024).read_to_end(&mut head))
        .map_err(|source| AudioError::Open {
            path: path.to_path_buf(),
            source,
        })?;
    parse_header(&head)
        .ok_or_else(|| AudioError::Dsd(format!("{} is not a readable DSD file", path.display())))
}

fn parse_header(head: &[u8]) -> Option<DsdInfo> {
    if head.starts_with(b"DSD ") {
        parse_dsf(head)
    } else if head.starts_with(b"FRM8") {
        parse_dff(head)
    } else {
        None
    }
}

fn le32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// DSF: a 28-byte `DSD ` chunk, then `fmt ` holding the channel count and the rate.
fn parse_dsf(head: &[u8]) -> Option<DsdInfo> {
    if head.get(28..32)? != b"fmt " {
        return None;
    }
    let channels = le32(head, 52)?;
    let dsd_rate = le32(head, 56)?;
    valid(channels, dsd_rate)
}

/// DFF: a `FRM8` container whose `PROP` chunk holds `FS  ` (the rate) and `CHNL` (channels).
fn parse_dff(head: &[u8]) -> Option<DsdInfo> {
    let (mut channels, mut dsd_rate) = (None, None);
    let mut at = 16;
    while at + 12 <= head.len() {
        let id = head.get(at..at + 4)?;
        let size = usize::try_from(u64::from_be_bytes(
            head.get(at + 4..at + 12)?.try_into().ok()?,
        ))
        .ok()?;
        let body = at + 12;
        if id == b"PROP" {
            let end = (body + size).min(head.len());
            let mut inner = body + 4; // skip "SND "
            while inner + 12 <= end {
                let inner_id = head.get(inner..inner + 4)?;
                let inner_size = usize::try_from(be32(head, inner + 8)?).ok()?;
                match inner_id {
                    b"FS  " => dsd_rate = be32(head, inner + 12),
                    b"CHNL" => {
                        let raw = head.get(inner + 12..inner + 14)?;
                        channels = Some(u32::from(u16::from_be_bytes(raw.try_into().ok()?)));
                    }
                    _ => {}
                }
                inner += 12 + inner_size + (inner_size & 1);
            }
            break;
        }
        at = body + size + (size & 1);
    }
    valid(channels?, dsd_rate?)
}

fn valid(channels: u32, dsd_rate: u32) -> Option<DsdInfo> {
    if channels == 0 || dsd_rate < 44_100 {
        return None;
    }
    Some(DsdInfo {
        channels: usize::try_from(channels).ok()?,
        dsd_rate,
    })
}

/// Hands the file to `on_block` as ordinary audio at `rate`, every channel kept, in order.
///
/// FFmpeg does the conversion. If the caller stops early (an error), FFmpeg is stopped too.
pub fn for_each_block<E, F>(path: &Path, rate: u32, mut on_block: F) -> Result<StreamInfo, E>
where
    E: From<AudioError>,
    F: FnMut(&StreamInfo, &[f32]) -> Result<(), E>,
{
    let header = read_info(path)?;
    let info = StreamInfo {
        sample_rate: rate,
        channels: header.channels,
        bits_per_sample: None,
    };

    let mut command = ffmpeg_command()?;
    command
        .args(["-nostdin", "-v", "error", "-i"])
        .arg(path)
        .args([
            "-vn",
            "-map",
            "0:a:0",
            "-f",
            "f32le",
            "-acodec",
            "pcm_f32le",
        ])
        .args(["-ar", &rate.to_string(), "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| AudioError::Dsd(format!("could not start FFmpeg: {error}")))?;
    let (Some(mut stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
        return Err(AudioError::Dsd("could not read FFmpeg's output".to_string()).into());
    };
    // FFmpeg's messages are read on the side, so a full pipe can never stall it.
    let messages = thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });

    let frame_bytes = header.channels * 4;
    let mut buffer = vec![0_u8; frame_bytes * 16_384];
    let mut carried = 0_usize;
    let mut samples: Vec<f32> = Vec::new();
    let mut produced = false;
    let outcome: Result<(), E> = loop {
        let read = match stdout.read(&mut buffer[carried..]) {
            Ok(0) => break Ok(()),
            Ok(read) => read,
            Err(error) => {
                break Err(AudioError::Dsd(format!("reading FFmpeg's output: {error}")).into());
            }
        };
        let whole = (carried + read) / frame_bytes * frame_bytes;
        if whole > 0 {
            samples.clear();
            samples.extend(
                buffer[..whole]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|bytes| f32::from_le_bytes(*bytes)),
            );
            produced = true;
            if let Err(error) = on_block(&info, &samples) {
                break Err(error);
            }
        }
        buffer.copy_within(whole..carried + read, 0);
        carried = carried + read - whole;
    };

    if outcome.is_err() {
        let _ = child.kill();
    }
    let status = child.wait();
    let text = messages.join().unwrap_or_default();
    outcome?;
    match status {
        Ok(status) if status.success() && produced => Ok(info),
        Ok(status) if status.success() => Err(AudioError::NoAudioStream.into()),
        _ => Err(AudioError::Dsd(format!(
            "FFmpeg could not read {}: {}",
            path.display(),
            text.trim()
                .lines()
                .last()
                .unwrap_or("it stopped without a reason")
        ))
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dsf_header(channels: u32, rate: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(b"DSD ");
        bytes.extend(28_u64.to_le_bytes());
        bytes.extend(0_u64.to_le_bytes()); // total file size
        bytes.extend(0_u64.to_le_bytes()); // metadata pointer
        bytes.extend(b"fmt ");
        bytes.extend(52_u64.to_le_bytes());
        bytes.extend(1_u32.to_le_bytes()); // version
        bytes.extend(0_u32.to_le_bytes()); // format id
        bytes.extend(7_u32.to_le_bytes()); // channel type
        bytes.extend(channels.to_le_bytes());
        bytes.extend(rate.to_le_bytes());
        bytes.extend(1_u32.to_le_bytes()); // bits per sample
        bytes.extend(1000_u64.to_le_bytes()); // sample count
        bytes.extend(4096_u32.to_le_bytes()); // block size
        bytes.extend(0_u32.to_le_bytes());
        bytes
    }

    fn dff_header(channels: u16, rate: u32) -> Vec<u8> {
        let mut prop = Vec::new();
        prop.extend(b"SND ");
        prop.extend(b"FS  ");
        prop.extend(4_u64.to_be_bytes());
        prop.extend(rate.to_be_bytes());
        prop.extend(b"CHNL");
        prop.extend((2 + 4 * u64::from(channels)).to_be_bytes());
        prop.extend(channels.to_be_bytes());
        for _ in 0..channels {
            prop.extend(b"SLFT");
        }
        let mut bytes = Vec::new();
        bytes.extend(b"FRM8");
        bytes.extend(0_u64.to_be_bytes());
        bytes.extend(b"DSD ");
        bytes.extend(b"FVER");
        bytes.extend(4_u64.to_be_bytes());
        bytes.extend(0x0105_0000_u32.to_be_bytes());
        bytes.extend(b"PROP");
        bytes.extend((prop.len() as u64).to_be_bytes());
        bytes.extend(prop);
        bytes
    }

    #[test]
    fn a_dsf_header_gives_channels_and_speed() {
        let info = parse_header(&dsf_header(6, 2_822_400)).unwrap();
        assert_eq!(info.channels, 6);
        assert_eq!(info.dsd_rate, 2_822_400);
        assert_eq!(info.name(), "DSD64");
    }

    #[test]
    fn a_dff_header_gives_channels_and_speed() {
        let info = parse_header(&dff_header(2, 5_644_800)).unwrap();
        assert_eq!(info.channels, 2);
        assert_eq!(info.name(), "DSD128");
    }

    #[test]
    fn things_that_are_not_dsd_are_refused() {
        assert_eq!(parse_header(b"RIFF....WAVE"), None);
        assert_eq!(parse_header(&dsf_header(0, 2_822_400)), None);
        assert_eq!(parse_header(&dsf_header(2, 0)), None);
        assert_eq!(parse_header(b"DSD "), None);
        assert_eq!(parse_header(b"FRM8"), None);
    }

    #[test]
    fn the_extension_decides_what_is_dsd() {
        assert!(is_dsd(Path::new("a/Song.DSF")));
        assert!(is_dsd(Path::new("song.dff")));
        assert!(!is_dsd(Path::new("song.flac")));
        assert!(!is_dsd(Path::new("song")));
    }
}
