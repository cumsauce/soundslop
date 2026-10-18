//! The clip worker: renders Kit one-shots off the audio thread.
//!
//! Mirrors `foley-synth`'s thunder thread: the worker **never blocks** on
//! either channel (poll + 1 ms sleep), because the audio thread only ever
//! uses `try_send`/`try_recv` and std's mpsc locks a waker only while a peer
//! is blocked on the other end. Freed buffers come back through
//! [`EngineJob::Free`] into a local free-list so the audio thread never
//! frees (or allocates) either.

use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};
use std::time::Duration;

use crate::kit_map::render_clip;

/// What a Kit slot plays. The knob and index laws per category live in
/// [`crate::kit_map`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotCategory {
    Contact,
    Fracture,
    Shot,
    Primitive,
    Voice,
    Note,
}

/// One clip render request. Sent by the audio thread, rendered by the
/// worker.
#[derive(Clone, Copy, Debug)]
pub struct RenderJob {
    pub slot: u8,
    pub category: SlotCategory,
    pub kind: u32,
    pub voice_mob: u32,
    pub pitch: u8,
    pub knobs: [f32; 2],
    pub velocity: f32,
    pub seed: u64,
    pub sample_rate: u32,
}

/// What the audio thread asks the clip worker to do.
#[derive(Debug)]
pub enum EngineJob {
    Render(RenderJob),
    /// A clip that's finished playing, freed here rather than on the audio
    /// thread.
    Free(Vec<f32>),
}

/// Starts the clip worker thread. It ends when `jobs` disconnects.
///
/// `jobs` and `results` are the two ends of **bounded** channels (the
/// plan pins jobs at 32 and results at `2 * MAX_VOICES`); the worker polls
/// rather than blocks so the audio thread's `try_send`/`try_recv` stay
/// lock-free in the common case.
pub fn spawn_clip_worker(
    jobs: Receiver<EngineJob>,
    results: SyncSender<Vec<f32>>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("soundslop-clip".into())
        .spawn(move || {
            // Freed buffers, kept warm so the next Render reuses an
            // allocation the audio thread already touched. Never shrunk.
            let mut pool: Vec<Vec<f32>> = Vec::new();
            let mut pending: Option<Vec<f32>> = None;
            loop {
                let mut progress = false;
                // Deliver the finished clip first (same shape as thunder).
                if let Some(buf) = pending.take() {
                    match results.try_send(buf) {
                        Ok(()) => progress = true,
                        Err(TrySendError::Full(buf)) => pending = Some(buf),
                        Err(TrySendError::Disconnected(_)) => return,
                    }
                }
                if pending.is_none() {
                    match jobs.try_recv() {
                        Ok(EngineJob::Free(buf)) => {
                            pool.push(buf);
                            progress = true;
                        }
                        Ok(EngineJob::Render(job)) => {
                            let mut buf = pool.pop().unwrap_or_default();
                            buf.clear();
                            let clip = render_clip(&job);
                            buf.extend_from_slice(&clip);
                            drop(clip);
                            pending = Some(buf);
                            progress = true;
                        }
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => return,
                    }
                }
                if !progress {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        })
        .expect("soundslop: failed to spawn clip worker thread")
}
