//! Worker threads that measure files in the background, so the screen stays responsive.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use rangeforge_audio::cache::{Cache, Source};
use rangeforge_core::planner::Measurement;

use crate::identity::identify_file;
use crate::load::Job;
use crate::measuring::{measure_split, measure_whole};
use crate::model::{MeasureOutcome, Origin};

/// More workers than this just fight over a network drive.
const MAX_WORKERS: usize = 4;

/// Starts measuring `jobs` in the background and returns where results arrive, one
/// [`MeasureOutcome`] per job, in the order they finish.
pub fn start(jobs: Vec<Job>, cache: Option<&Cache>) -> Receiver<MeasureOutcome> {
    let (job_sender, job_receiver) = mpsc::channel();
    for job in jobs {
        // The receiver is alive, so sending cannot fail.
        let _ = job_sender.send(job);
    }
    // With the sender gone, each worker stops once the queue is empty.
    drop(job_sender);

    let (outcome_sender, outcome_receiver) = mpsc::channel();
    let queue = Arc::new(Mutex::new(job_receiver));
    let workers = thread::available_parallelism()
        .map_or(2, usize::from)
        .clamp(1, MAX_WORKERS);
    for _ in 0..workers {
        let queue = Arc::clone(&queue);
        let outcomes = outcome_sender.clone();
        let cache = cache.cloned();
        thread::spawn(move || work(&queue, &outcomes, cache.as_ref()));
    }
    outcome_receiver
}

fn work(queue: &Mutex<Receiver<Job>>, outcomes: &Sender<MeasureOutcome>, cache: Option<&Cache>) {
    loop {
        let next = match queue.lock() {
            Ok(guard) => guard.recv(),
            Err(_) => return,
        };
        let Ok(job) = next else {
            return;
        };
        let started = Instant::now();
        let result = measure(&job, cache);
        let outcome = MeasureOutcome {
            group: job.group,
            result,
            elapsed: started.elapsed(),
            // A single file brings its tags along; a CUE album is identified by its sheet.
            identity: job.cue_file.is_none().then(|| identify_file(&job.audio)),
        };
        if outcomes.send(outcome).is_err() {
            return;
        }
    }
}

fn measure(job: &Job, cache: Option<&Cache>) -> Result<(Vec<Measurement>, Origin), String> {
    let measured = match &job.cue_file {
        Some(file) => measure_split(&job.audio, &file.segments(), job.mix, cache),
        None => measure_whole(&job.audio, job.mix, cache),
    };
    let (measured, source) = measured.map_err(|error| format!("{error:#}"))?;
    let origin = match source {
        Source::Cache => Origin::Cache,
        Source::Decoded { .. } => Origin::Decoded,
    };
    Ok((measured.tracks, origin))
}
