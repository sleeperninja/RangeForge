//! Decoding an audio file once, as a stream of blocks.

use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;

use rangeforge_core::mix::{self, MixPlan};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::{AudioError, dsd};

/// Facts about the decoded audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamInfo {
    pub sample_rate: u32,
    pub channels: usize,
    /// How precisely the source stores each sample, such as 16 or 24. `None` for lossy
    /// sources, which have no fixed precision.
    pub bits_per_sample: Option<u32>,
    /// The channels were mixed down from the file's own (see [`for_each_block_mixed`]), so every
    /// sample is a new value and not a copy of one in the file.
    pub mixed: bool,
}

/// Decodes `path` in one sequential pass and hands each block of interleaved `f32` samples
/// (from -1.0 to 1.0) to `on_block`, in order.
///
/// The file is read front to back, so it is safe on a network drive. A corrupt packet is an
/// error and not something to skip, because skipping would quietly change the audio.
pub fn for_each_block<E, F>(path: &Path, on_block: F) -> Result<StreamInfo, E>
where
    E: From<AudioError>,
    F: FnMut(&StreamInfo, &[f32]) -> Result<(), E>,
{
    for_each_block_at(path, dsd::ANALYSIS_RATE, on_block)
}

/// Like [`for_each_block`], but a DSD file is turned into audio at `dsd_rate`. Other files are
/// decoded at their own rate, and `dsd_rate` does not apply to them.
pub fn for_each_block_at<E, F>(path: &Path, dsd_rate: u32, on_block: F) -> Result<StreamInfo, E>
where
    E: From<AudioError>,
    F: FnMut(&StreamInfo, &[f32]) -> Result<(), E>,
{
    decode_blocks(path, dsd_rate, on_block)
}

/// Like [`for_each_block_at`], and the channels are mixed as `mix` says before each block is
/// handed over, so a meter or an encoder sees the mixed sound. The [`StreamInfo`] given with
/// every block, and the one returned, name the channel count after mixing. Audio that the plan
/// leaves alone (no mix asked for, or a layout that is not mixed, see
/// [`rangeforge_core::mix::matrix_for`]) is handed over untouched and unchanged.
///
/// The channel order is taken to be the usual one for the count (5.1 as front left, front
/// right, centre, LFE, back left, back right): the decoders here do not report a layout.
pub fn for_each_block_mixed<E, F>(
    path: &Path,
    dsd_rate: u32,
    mix: MixPlan,
    mut on_block: F,
) -> Result<StreamInfo, E>
where
    E: From<AudioError>,
    F: FnMut(&StreamInfo, &[f32]) -> Result<(), E>,
{
    // Decided from the first block, which is where the source's channel count is known.
    let mut matrix: Option<Option<mix::Matrix>> = None;
    let mut mixed: Vec<f32> = Vec::new();
    let info = decode_blocks(path, dsd_rate, |info: &StreamInfo, samples: &[f32]| {
        if matrix.is_none() {
            matrix = Some(mix::matrix_for(info.channels, mix));
        }
        match matrix.as_ref().and_then(Option::as_ref) {
            None => on_block(info, samples),
            Some(matrix) => {
                matrix.apply(samples, &mut mixed);
                let after = StreamInfo {
                    channels: matrix.outputs(),
                    mixed: true,
                    ..*info
                };
                on_block(&after, &mixed)
            }
        }
    })?;
    Ok(StreamInfo {
        channels: mix::channels_after(info.channels, mix),
        mixed: mix::matrix_for(info.channels, mix).is_some(),
        ..info
    })
}

/// The decoding itself: blocks of the file's own channels.
fn decode_blocks<E, F>(path: &Path, dsd_rate: u32, mut on_block: F) -> Result<StreamInfo, E>
where
    E: From<AudioError>,
    F: FnMut(&StreamInfo, &[f32]) -> Result<(), E>,
{
    if dsd::is_dsd(path) {
        return dsd::for_each_block(path, dsd_rate, on_block);
    }
    let file = File::open(path).map_err(|source| AudioError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    let stream = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(AudioError::from)?;
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(AudioError::NoAudioStream)?;
    let track_id = track.id;
    let params = track.codec_params.clone();
    let declared_bits = params.bits_per_sample.or(params.bits_per_coded_sample);

    let mut decoder = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions::default())
        .map_err(AudioError::from)?;
    let mut buffer: Option<SampleBuffer<f32>> = None;
    // Not every format states its sample rate and channel layout up front (ALAC inside MP4,
    // for one), so the info is taken from the first decoded block.
    let mut info: Option<StreamInfo> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            // Symphonia reports the end of the stream as an unexpected end of file.
            Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                break;
            }
            Err(error) => return Err(AudioError::from(error).into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let block = decoder.decode(&packet).map_err(AudioError::from)?;
        let spec = *block.spec();
        let current = *info.get_or_insert(StreamInfo {
            sample_rate: spec.rate,
            channels: spec.channels.count(),
            bits_per_sample: declared_bits,
            mixed: false,
        });
        let samples =
            buffer.get_or_insert_with(|| SampleBuffer::<f32>::new(block.capacity() as u64, spec));
        samples.copy_interleaved_ref(block);
        on_block(&current, samples.samples())?;
    }

    info.ok_or_else(|| AudioError::NoAudioStream.into())
}
