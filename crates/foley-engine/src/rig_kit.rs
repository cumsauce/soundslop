//! `KitRig`: the headless MIDI one-shot sampler behind Soundslop Kit.
//!
//! The plugin writes [`SlotState`]s once per block and forwards NoteOns
//! here; every matching slot enqueues a [`RenderJob`] to the clip worker
//! ([`crate::clip`]) — rendering never happens on the audio thread. The
//! audio thread only ever `try_recv`s finished clips, plays them from
//! memory with fixed fades, and returns exhausted buffers to the worker's
//! free-list via [`EngineJob::Free`].

use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};

use crate::clip::{EngineJob, RenderJob, SlotCategory, spawn_clip_worker};
use crate::guard::GainGuard;
use crate::kit_map::kind_range;
use crate::seed_mix::hit_seed;
use crate::spatial::pan_gains;
use crate::{MASTER_CEILING, MAX_SAMPLE_RATE, MAX_VOICES, MIN_SAMPLE_RATE};

/// Slots per Kit instance.
pub const SLOTS: usize = 8;
/// Attack fade, seconds.
const ATTACK_SECONDS: f32 = 0.002;
/// Release fade, seconds.
const RELEASE_SECONDS: f32 = 0.010;
/// Pan −100..=100 maps to azimuth via this scale.
const PAN_SCALE: f32 = 0.9;

/// One slot's parameter state. The plugin writes raw values here every
/// block; `kind`/`voice_mob` are clamped via `kind_range` when a hit is
/// built. Defaults mirror the plugin parameter defaults (spec §11): note
/// 60, gain 0.8, knobs 0.5, fresh true.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SlotState {
    pub category: SlotCategory,
    pub kind: u32,
    pub voice_mob: u32,
    pub note: u8,
    pub knobs: [f32; 2],
    pub gain: f32,
    pub pan: f32,
    pub seed: u64,
    pub fresh: bool,
}

impl Default for SlotState {
    fn default() -> Self {
        SlotState {
            category: SlotCategory::Contact,
            kind: 0,
            voice_mob: 0,
            note: 60,
            knobs: [0.5; 2],
            gain: 0.8,
            pan: 0.0,
            seed: 0,
            fresh: true,
        }
    }
}

/// What a voice is doing. `seq` preserves arrival order (pool indices get
/// reused, so index order is not hit order).
enum VoiceState {
    Free,
    /// A hit was enqueued; its clip hasn't arrived yet.
    Pending {
        seq: u64,
        gain: f32,
        pan: f32,
    },
    Playing {
        seq: u64,
        clip: Vec<f32>,
        pos: usize,
        gain: f32,
        pan: f32,
    },
}

/// One voice-pool entry.
struct Voice {
    state: VoiceState,
}

impl Voice {
    fn free() -> Voice {
        Voice {
            state: VoiceState::Free,
        }
    }
}

/// The eight-slot sampler rig both the plugin and the tests drive.
pub struct KitRig {
    slots: [SlotState; SLOTS],
    /// Per-slot hit counters, fed to `hit_seed` on every fresh hit.
    counters: [u32; SLOTS],
    voices: Vec<Voice>,
    /// Monotonic hit counter for FIFO clip→pending pairing and steal-oldest.
    next_seq: u64,
    polyphony: usize,
    humanize: f32,
    output_gain: f32,
    master: GainGuard,
    rate: u32,
    jobs_tx: Option<SyncSender<EngineJob>>,
    clips_rx: Option<Receiver<Vec<f32>>>,
    worker: Option<std::thread::JoinHandle<()>>,
    /// Hits dropped because the jobs queue was full.
    starved: u64,
    attack_len: usize,
    release_len: usize,
}

impl KitRig {
    /// A rig for the host's sample rate (32..=192 kHz). All slots start at
    /// their spec defaults; the clip worker thread starts with it and the
    /// shared voice LUTs are grown off the first-note path.
    pub fn new(sample_rate: u32) -> KitRig {
        let rate = sample_rate.clamp(MIN_SAMPLE_RATE, MAX_SAMPLE_RATE);
        foley_synth::warm_voice_tables();
        let (jobs_tx, jobs_rx) = sync_channel::<EngineJob>(32);
        let (clips_tx, clips_rx) = sync_channel::<Vec<f32>>(MAX_VOICES * 2);
        let worker = spawn_clip_worker(jobs_rx, clips_tx);
        KitRig {
            slots: std::array::from_fn(|_| SlotState::default()),
            counters: [0; SLOTS],
            voices: (0..MAX_VOICES).map(|_| Voice::free()).collect(),
            next_seq: 0,
            polyphony: 4,
            humanize: 0.0,
            output_gain: 1.0,
            master: GainGuard::new(MASTER_CEILING),
            rate,
            jobs_tx: Some(jobs_tx),
            clips_rx: Some(clips_rx),
            worker: Some(worker),
            starved: 0,
            attack_len: fade_len(ATTACK_SECONDS, rate),
            release_len: fade_len(RELEASE_SECONDS, rate),
        }
    }

    /// The plugin writes raw per-slot values here every block.
    pub fn slots(&mut self) -> &mut [SlotState; SLOTS] {
        &mut self.slots
    }

    /// Playing-voice count (pending hits are not voices yet).
    pub fn active_voices(&self) -> usize {
        self.voices
            .iter()
            .filter(|v| matches!(v.state, VoiceState::Playing { .. }))
            .count()
    }

    /// Enqueue a hit on **every** slot bound to `note`. Per-hit seed
    /// follows the spec law: `fresh` → `hit_seed(slot.seed, note,
    /// velocity, counter)`; otherwise the slot's base seed. Full jobs
    /// queue drops the hit and bumps the starvation counter.
    pub fn note_on(&mut self, note: u8, velocity: f32) {
        for i in 0..SLOTS {
            if self.slots[i].note != note {
                continue;
            }
            let slot = self.slots[i];
            self.counters[i] = self.counters[i].wrapping_add(1);
            let mixed = if slot.fresh {
                hit_seed(slot.seed, note, velocity, self.counters[i])
            } else {
                slot.seed
            };
            let gain = slot.gain
                * velocity.clamp(0.0, 1.0)
                * (1.0 - 0.5 * self.humanize * unit_from_seed(mixed));
            let pan = slot.pan;
            let job = RenderJob {
                slot: i as u8,
                category: slot.category,
                kind: slot.kind.min(kind_range(slot.category)),
                voice_mob: slot.voice_mob,
                pitch: note,
                knobs: slot.knobs,
                velocity,
                seed: mixed,
                sample_rate: self.rate,
            };
            match self
                .jobs_tx
                .as_ref()
                .map(|tx| tx.try_send(EngineJob::Render(job)))
            {
                Some(Ok(())) => {
                    let seq = self.next_seq;
                    self.next_seq = self.next_seq.wrapping_add(1);
                    self.push_pending(seq, gain, pan);
                }
                Some(Err(TrySendError::Full(_))) => {
                    self.starved += 1;
                }
                Some(Err(TrySendError::Disconnected(_))) => {
                    // Worker gone: this hit is lost; keep counting.
                    self.starved += 1;
                }
                None => {}
            }
        }
    }

    /// Polyphony cap, clamped to {4, 8, 16}. At the cap the oldest playing
    /// voice is stolen on the next activation.
    pub fn set_polyphony(&mut self, n: usize) {
        self.polyphony = if n <= 4 {
            4
        } else if n <= 8 {
            8
        } else {
            16
        };
    }

    /// Humanize depth 0..=1: per-hit gain multiplier in
    /// `[1 − 0.5·humanize, 1.0]`, deterministic per hit seed.
    pub fn set_humanize(&mut self, h: f32) {
        self.humanize = h.clamp(0.0, 1.0);
    }

    /// Linear output gain, applied before the master guard.
    pub fn set_output_gain(&mut self, linear: f32) {
        self.output_gain = linear;
    }

    /// Hits dropped to a full queue (diagnostics).
    pub fn starved(&self) -> u64 {
        self.starved
    }

    /// Play finished clips in arrival order and render the voice pool into
    /// `left`/`right` (uses `min(len)` of the two).
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        if n == 0 {
            return;
        }
        self.pickup_clips();
        left[..n].fill(0.0);
        right[..n].fill(0.0);
        // Fixed-size buffer: `MAX_VOICES` voices can each finish at most once
        // per block, so this never overflows and never allocates (RT-safe).
        let mut done = [0usize; MAX_VOICES];
        let mut done_len = 0usize;
        for (vi, voice) in self.voices.iter_mut().enumerate() {
            let VoiceState::Playing {
                clip,
                pos,
                gain,
                pan,
                ..
            } = &mut voice.state
            else {
                continue;
            };
            let (lg, rg) = pan_gains(*pan * PAN_SCALE);
            let fade_in = self.attack_len;
            let fade_out = self.release_len;
            let total = clip.len();
            let mut k = 0;
            while k < n && *pos < total {
                let mut x = clip[*pos];
                // Linear attack for the first samples…
                if *pos < fade_in {
                    x *= (*pos + 1) as f32 / fade_in as f32;
                }
                // …linear release over the final samples.
                let remaining = total - *pos;
                if remaining < fade_out {
                    x *= remaining as f32 / fade_out as f32;
                }
                x *= *gain;
                left[k] += x * lg;
                right[k] += x * rg;
                *pos += 1;
                k += 1;
            }
            if *pos >= total {
                done[done_len] = vi;
                done_len += 1;
            }
        }
        for &vi in &done[..done_len] {
            if let VoiceState::Playing { clip, .. } =
                std::mem::replace(&mut self.voices[vi].state, VoiceState::Free)
            {
                self.return_buffer(clip);
            }
        }
        for k in 0..n {
            left[k] = self.master.process(left[k] * self.output_gain);
            right[k] = self.master.process(right[k] * self.output_gain);
        }
    }

    fn push_pending(&mut self, seq: u64, gain: f32, pan: f32) {
        if let Some(v) = self
            .voices
            .iter_mut()
            .find(|v| matches!(v.state, VoiceState::Free))
        {
            v.state = VoiceState::Pending { seq, gain, pan };
        }
        // No free voice (more in-flight hits than pool slots): the clip
        // will arrive orphaned and be returned to the worker unused.
    }

    /// Pair arriving clips with the oldest pending hit; steal the oldest
    /// playing voice when the polyphony cap is reached.
    fn pickup_clips(&mut self) {
        loop {
            let Some(rx) = self.clips_rx.as_ref() else {
                return;
            };
            let Ok(clip) = rx.try_recv() else {
                return;
            };
            let pending = self
                .voices
                .iter()
                .enumerate()
                .filter_map(|(vi, v)| match v.state {
                    VoiceState::Pending { seq, gain, pan } => Some((seq, vi, gain, pan)),
                    _ => None,
                })
                .min_by_key(|&(seq, _, _, _)| seq);
            let Some((seq, pending_idx, gain, pan)) = pending else {
                // Orphan clip (its pending voice lost): hand it straight back.
                self.return_buffer(clip);
                continue;
            };
            if clip.is_empty() {
                // Empty render (e.g. out-of-range kind): nothing to play.
                self.voices[pending_idx].state = VoiceState::Free;
                self.return_buffer(clip);
                continue;
            }
            if self.active_voices() >= self.polyphony {
                self.steal_oldest();
            }
            self.voices[pending_idx].state = VoiceState::Playing {
                seq,
                clip,
                pos: 0,
                gain,
                pan,
            };
        }
    }

    fn steal_oldest(&mut self) {
        // The oldest playing voice is the lowest-seq Playing entry (pool
        // indices get reused, so index order is not hit order).
        let victim = self
            .voices
            .iter_mut()
            .filter_map(|v| match v.state {
                VoiceState::Playing { seq, .. } => Some((seq, v)),
                _ => None,
            })
            .min_by_key(|&(seq, _)| seq)
            .map(|(_, v)| v);
        if let Some(v) = victim
            && let VoiceState::Playing { clip, .. } =
                std::mem::replace(&mut v.state, VoiceState::Free)
        {
            self.return_buffer(clip);
        }
    }

    /// Return a clip buffer to the worker's free-list without allocating
    /// on the audio thread. Full queue → the buffer is dropped here (the
    /// worker's pool refills from its own reserves).
    fn return_buffer(&mut self, clip: Vec<f32>) {
        if let Some(tx) = self.jobs_tx.as_ref() {
            let _ = tx.try_send(EngineJob::Free(clip));
        }
    }
}

/// Uniform [0, 1) from the top 53 bits of a seed (the standard f64 trick).
#[inline]
fn unit_from_seed(seed: u64) -> f32 {
    ((seed >> 11) as f64 / ((1u64 << 53) as f64)) as f32
}

fn fade_len(seconds: f32, rate: u32) -> usize {
    (seconds * rate as f32).max(1.0) as usize
}

impl Drop for KitRig {
    fn drop(&mut self) {
        // Stop the worker: drop the jobs sender so try_recv() returns
        // Disconnected (the worker never blocks, so joining is safe), then
        // drop the results receiver so a pending try_send() errors out.
        self.jobs_tx.take();
        self.clips_rx.take();
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}
