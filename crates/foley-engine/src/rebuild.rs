//! The bed rebuilder: constructs replacement `BedUnion`s off the audio
//! thread.
//!
//! A bed constructor allocates and pre-renders warm-up, so it can never run
//! on the audio thread. When a layer's kind (or seed, or the host sample
//! rate) changes, the rig enqueues a [`RebuildJob`]; this worker builds the
//! new bed and hands it back on a cap-1 channel. Unlike the clip worker
//! this thread **may block** on both ends — it is the one place the design
//! accepts a blocking send, because at most one rebuild is ever in flight
//! and the audio side only `try_recv`s.

use std::sync::mpsc::{Receiver, SyncSender};
use std::thread::JoinHandle;

use crate::bed_union::{BedKind, BedUnion, new_bed};

/// One bed construction request.
#[derive(Clone, Copy, Debug)]
pub struct RebuildJob {
    pub kind: BedKind,
    pub seed: u64,
    pub sample_rate: u32,
}

/// Starts the rebuilder thread. It ends when `jobs` disconnects, and also
/// if the consumer of `out` has gone away.
pub fn spawn_rebuilder(jobs: Receiver<RebuildJob>, out: SyncSender<BedUnion>) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("soundslop-rebuild".into())
        .spawn(move || {
            while let Ok(job) = jobs.recv() {
                let bed = new_bed(job.kind, job.seed, job.sample_rate);
                if out.send(bed).is_err() {
                    return;
                }
            }
        })
        .expect("soundslop: failed to spawn rebuilder thread")
}
