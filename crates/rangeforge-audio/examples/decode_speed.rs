//! Times how long a file takes to decode, with nothing else running.
//!
//! ```text
//! cargo run --release -p rangeforge-audio --example decode_speed -- <file> [<file> ...]
//! ```
//!
//! This separates the decoder's own speed from the loudness meter and the encoder, so the
//! choice of decoder can be judged on its own.

use std::path::PathBuf;
use std::time::Instant;

use rangeforge_audio::AudioError;
use rangeforge_audio::decode::for_each_block;

fn main() -> Result<(), AudioError> {
    for path in std::env::args_os().skip(1).map(PathBuf::from) {
        let started = Instant::now();
        let mut sample_frames = 0_u64;
        let info = for_each_block::<AudioError, _>(&path, |info, samples| {
            sample_frames += (samples.len() / info.channels) as u64;
            Ok(())
        })?;
        let seconds = started.elapsed().as_secs_f64();
        #[allow(clippy::cast_precision_loss)]
        let audio_seconds = sample_frames as f64 / f64::from(info.sample_rate);
        println!(
            "{:>7.2} s  {:>5.0}x real time  {:>7.1} s of audio  {}",
            seconds,
            audio_seconds / seconds,
            audio_seconds,
            path.display()
        );
    }
    Ok(())
}
