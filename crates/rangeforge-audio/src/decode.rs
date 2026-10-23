//! Decoding an audio file once, as a stream of blocks.

use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::AudioError;

/// Facts about the decoded audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamInfo {
    pub sample_rate: u32,
    pub channels: usize,
    /// How precisely the source stores each sample, such as 16 or 24. `None` for lossy
    /// sources, which have no fixed precision.
    pub bits_per_sample: Option<u32>,
}

/// Decodes `path` in one sequential pass and hands each block of interleaved `f32` samples
/// (from -1.0 to 1.0) to `on_block`, in order.
///
/// The file is read front to back, so it is safe on a network drive. A corrupt packet is an
/// error and not something to skip, because skipping would quietly change the audio.
pub fn for_each_block<E, F>(path: &Path, mut on_block: F) -> Result<StreamInfo, E>
where
    E: From<AudioError>,
    F: FnMut(&StreamInfo, &[f32]) -> Result<(), E>,
{
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
        });
        let samples =
            buffer.get_or_insert_with(|| SampleBuffer::<f32>::new(block.capacity() as u64, spec));
        samples.copy_interleaved_ref(block);
        on_block(&current, samples.samples())?;
    }

    info.ok_or_else(|| AudioError::NoAudioStream.into())
}
