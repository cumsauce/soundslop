//! `AmbienceRig`: the headless six-layer soundscape machine.
//!
//! The plugin writes [`LayerState`]s once per block and drains stereo here;
//! everything realtime-hostile (bed construction) happens on the rebuilder
//! thread via [`crate::rebuild`]. Per spec §9: layers glide their knobs,
//! swap beds through a fade, and are held under `LAYER_CEILING` by
//! per-sample guards, with a master guard under `MASTER_CEILING` last.

use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};

use crate::bed_union::{BedKind, BedUnion};
use crate::guard::GainGuard;
use crate::motion::Motion;
use crate::rebuild::{RebuildJob, spawn_rebuilder};
use crate::spatial::{OnePoleLpf, layer_cutoff, pan_gains};
use crate::{LAYER_CEILING, MASTER_CEILING, MAX_BLOCK, MAX_SAMPLE_RATE, MIN_SAMPLE_RATE};

/// Layers per scene.
pub const LAYERS: usize = 6;
/// Swap fade-in, seconds.
const SWAP_FADE_SECONDS: f32 = 0.005;
/// Floor on the glide time so the smoother's coefficient stays finite.
const MIN_GLIDE_SECONDS: f32 = 1.0e-4;
/// Thunder flash height scale: knob C (0..=1) → 0..=50 blocks.
const HEIGHT_SCALE: f32 = 50.0;

/// One layer's parameter state. The plugin writes raw values here every
/// block; the rig smooths and applies them in `process`. Defaults mirror
/// the plugin parameter defaults (spec §10): gain 0.5, knobs 0.5,
/// motion_amt 0.15.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LayerState {
    pub kind: Option<BedKind>,
    pub gain: f32,
    pub seed: u64,
    pub motion_amt: f32,
    pub motion_rate: f32,
    pub dist: f32,
    pub az: f32,
    pub knobs: [f32; 4],
}

impl Default for LayerState {
    fn default() -> Self {
        LayerState {
            kind: None,
            gain: 0.5,
            seed: 0,
            motion_amt: 0.15,
            motion_rate: 0.0,
            dist: 0.0,
            az: 0.0,
            knobs: [0.5; 4],
        }
    }
}

/// Runtime state the plugin doesn't see.
struct LayerRt {
    bed: Option<BedUnion>,
    /// Smoothed knobs, before motion offsets.
    smoothed: [f32; 4],
    motion: Motion,
    motion_cfg: (f32, f32, u64),
    lpf: OnePoleLpf,
    guard: GainGuard,
    /// Samples already faded in; `>= fade_len` means fully in.
    fade_pos: usize,
    fade_len: usize,
    /// The (kind, seed) pair last enqueued for rebuild: change detection
    /// and attribution of a rebuilt bed back to this layer.
    requested: Option<(BedKind, u64)>,
    /// A rebuild for this layer is in flight.
    pending: bool,
}

impl Default for LayerRt {
    fn default() -> Self {
        LayerRt {
            bed: None,
            smoothed: [0.5; 4],
            motion: Motion::new(0.0, 0.0, 0),
            motion_cfg: (0.0, 0.0, 0),
            lpf: OnePoleLpf::new(),
            guard: GainGuard::new(LAYER_CEILING),
            fade_pos: 1,
            fade_len: 0,
            requested: None,
            pending: false,
        }
    }
}

/// The six-layer soundscape rig both the plugin and the tests drive.
pub struct AmbienceRig {
    states: [LayerState; LAYERS],
    rt: [LayerRt; LAYERS],
    rate: u32,
    alpha: f32,
    output_gain: f32,
    master: GainGuard,
    scratch: [f32; MAX_BLOCK],
    acc_l: [f32; MAX_BLOCK],
    acc_r: [f32; MAX_BLOCK],
    jobs_tx: Option<SyncSender<RebuildJob>>,
    bed_rx: Option<Receiver<BedUnion>>,
    rebuilder: Option<std::thread::JoinHandle<()>>,
    /// Rebuild requests dropped because the jobs queue was full.
    starved: u64,
}

impl AmbienceRig {
    /// A rig for the host's sample rate (32..=192 kHz). All layers start
    /// Off; the rebuilder thread starts with it.
    pub fn new(sample_rate: u32) -> AmbienceRig {
        let rate = sample_rate.clamp(MIN_SAMPLE_RATE, MAX_SAMPLE_RATE);
        let (jobs_tx, jobs_rx) = sync_channel::<RebuildJob>(32);
        let (bed_tx, bed_rx) = sync_channel::<BedUnion>(1);
        let rebuilder = spawn_rebuilder(jobs_rx, bed_tx);
        AmbienceRig {
            states: [LayerState::default(); LAYERS],
            rt: std::array::from_fn(|_| LayerRt::default()),
            rate,
            alpha: knob_alpha(0.30, rate),
            output_gain: 1.0,
            master: GainGuard::new(MASTER_CEILING),
            scratch: [0.0; MAX_BLOCK],
            acc_l: [0.0; MAX_BLOCK],
            acc_r: [0.0; MAX_BLOCK],
            jobs_tx: Some(jobs_tx),
            bed_rx: Some(bed_rx),
            rebuilder: Some(rebuilder),
            starved: 0,
        }
    }

    /// Knob smoothing time in seconds (spec default 0.30 s).
    pub fn set_glide(&mut self, seconds: f32) {
        self.alpha = knob_alpha(seconds.max(MIN_GLIDE_SECONDS), self.rate);
    }

    /// The plugin writes raw per-layer values here every block.
    pub fn layers(&mut self) -> &mut [LayerState; LAYERS] {
        &mut self.states
    }

    /// The bed currently sounding on a layer, if any (GUI + tests).
    pub fn layer_kind(&self, layer: usize) -> Option<BedKind> {
        self.rt.get(layer)?.bed.as_ref().map(BedUnion::kind)
    }

    /// Linear output gain, applied before the master guard.
    pub fn set_output_gain(&mut self, linear: f32) {
        self.output_gain = linear;
    }

    /// Rebuild requests dropped to a full queue (diagnostics).
    pub fn starved(&self) -> u64 {
        self.starved
    }

    /// Strike a layer's Thunder bed from outside `process`. Enqueue-only
    /// (Thunder's own worker does the rendering), so RT-safe. No-op when
    /// the layer isn't currently Thunder.
    pub fn strike(&mut self, layer: usize, base: u64) {
        let Some((state, rt)) = self.states.get_mut(layer).zip(self.rt.get_mut(layer)) else {
            return;
        };
        let Some(bed) = rt.bed.as_mut() else {
            return;
        };
        if bed.kind() != BedKind::Thunder {
            return;
        }
        let az = state.az.to_radians();
        let pos = [
            state.dist * az.sin(),
            rt.smoothed[2] * HEIGHT_SCALE,
            state.dist * az.cos(),
        ];
        bed.strike(base, pos);
    }

    /// Drain `left`/`right` (uses `min(len)` of the two), gliding layers,
    /// swapping beds, guarding and panning. Chunks internally at
    /// [`MAX_BLOCK`].
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        if n == 0 {
            return;
        }
        let dt = n as f32 / self.rate as f32;

        // 1. Rebuild bookkeeping: enqueue kind/seed changes, pick up any
        //    finished bed.
        self.service_rebuilds();
        // 2. Smooth knobs once per call, add motion, arm the beds.
        let knob_eff = self.advance_knobs(dt);
        for (rt, knobs) in self.rt.iter_mut().zip(knob_eff.iter()) {
            if let Some(bed) = rt.bed.as_mut() {
                bed.set_scene_knobs(*knobs);
            }
        }
        // 3. Render in MAX_BLOCK chunks, guard, write out.
        let mut offset = 0;
        while offset < n {
            let chunk = (n - offset).min(MAX_BLOCK);
            self.acc_l[..chunk].fill(0.0);
            self.acc_r[..chunk].fill(0.0);
            for i in 0..LAYERS {
                self.render_layer(i, chunk);
            }
            for k in 0..chunk {
                left[offset + k] = self.master.process(self.acc_l[k] * self.output_gain);
                right[offset + k] = self.master.process(self.acc_r[k] * self.output_gain);
            }
            offset += chunk;
        }
    }

    fn service_rebuilds(&mut self) {
        // Enqueue kind/seed changes.
        for i in 0..LAYERS {
            let want = self.states[i].kind.map(|kind| (kind, self.states[i].seed));
            if want == self.rt[i].requested {
                continue;
            }
            match want {
                Some(job_kind) => {
                    let job = RebuildJob {
                        kind: job_kind.0,
                        seed: job_kind.1,
                        sample_rate: self.rate,
                    };
                    match self.jobs_tx.as_ref().map(|tx| tx.try_send(job)) {
                        Some(Ok(())) => {
                            self.rt[i].requested = want;
                            self.rt[i].pending = true;
                        }
                        Some(Err(TrySendError::Full(_))) => {
                            self.starved += 1;
                            // Retry next block; requested stays stale.
                        }
                        Some(Err(TrySendError::Disconnected(_))) => {
                            // Rebuilder gone: go silent, stop retrying.
                            self.rt[i].bed = None;
                            self.rt[i].requested = None;
                            self.rt[i].pending = false;
                        }
                        None => {}
                    }
                }
                None => {
                    // Layer turned Off: silence immediately.
                    self.rt[i].bed = None;
                    self.rt[i].requested = None;
                    self.rt[i].pending = false;
                }
            }
        }
        // Pick up at most one finished bed (out channel cap 1).
        let Some(rx) = self.bed_rx.as_ref() else {
            return;
        };
        let Ok(bed) = rx.try_recv() else {
            return;
        };
        let kind = bed.kind();
        // Attribute to the oldest pending layer that asked for this
        // (kind, seed); FIFO ordering keeps duplicates honest.
        let target = (0..LAYERS).find(|&i| {
            self.rt[i].pending && self.rt[i].requested == Some((kind, self.states[i].seed))
        });
        if let Some(i) = target {
            self.rt[i].bed = Some(bed);
            self.rt[i].pending = false;
            self.rt[i].fade_pos = 0;
            self.rt[i].fade_len = (SWAP_FADE_SECONDS * self.rate as f32).max(1.0) as usize;
            self.rt[i].lpf.reset();
            self.rt[i].guard.reset();
            // Snap knobs on the first scene after a swap.
            self.rt[i].smoothed = self.states[i].knobs;
        }
        // Orphaned bed (its layer went Off while it was building): drop it.
    }

    /// Smooth each layer's knobs, add clamped motion offsets, and return
    /// the effective 0..=1 quadruples.
    fn advance_knobs(&mut self, dt: f32) -> [[f32; 4]; LAYERS] {
        let mut out = [[0.0f32; 4]; LAYERS];
        let alpha = self.alpha;
        for (i, rt) in self.rt.iter_mut().enumerate() {
            let st = self.states[i];
            // Recreate the walk only when its config actually changed.
            let cfg = (st.motion_amt, st.motion_rate, st.seed);
            if cfg != rt.motion_cfg {
                rt.motion = Motion::new(cfg.0, cfg.1, cfg.2);
                rt.motion_cfg = cfg;
            }
            let offs = rt.motion.next(dt);
            for k in 0..4 {
                let sm = &mut rt.smoothed[k];
                *sm += (st.knobs[k] - *sm) * alpha;
                out[i][k] = (*sm + offs[k]).clamp(0.0, 1.0);
            }
        }
        out
    }

    fn render_layer(&mut self, i: usize, chunk: usize) {
        let st = self.states[i];
        let Some(bed) = self.rt[i].bed.as_mut() else {
            return;
        };
        let cutoff = layer_cutoff(st.dist, self.rate as f32);
        let (lg, rg) = pan_gains(st.az);
        let gain = st.gain;
        bed.render(&mut self.scratch[..chunk]);
        let fade_len = self.rt[i].fade_len;
        let mut fade_pos = self.rt[i].fade_pos;
        let lpf = &mut self.rt[i].lpf;
        let guard = &mut self.rt[i].guard;
        for k in 0..chunk {
            let mut x = lpf.process(self.scratch[k], cutoff, self.rate as f32);
            let fade = if fade_pos < fade_len {
                let m = (fade_pos + 1) as f32 / fade_len as f32;
                fade_pos += 1;
                m
            } else {
                1.0
            };
            x = guard.process(x * fade * gain);
            self.acc_l[k] += x * lg;
            self.acc_r[k] += x * rg;
        }
        self.rt[i].fade_pos = fade_pos;
    }
}

fn knob_alpha(glide_seconds: f32, rate: u32) -> f32 {
    1.0 - (-1.0 / (glide_seconds.max(MIN_GLIDE_SECONDS) * rate as f32)).exp()
}

impl Drop for AmbienceRig {
    fn drop(&mut self) {
        // Stop the rebuilder: drop the jobs sender so recv() returns,
        // then drop the out receiver so a blocked send() errors out, then
        // join. (Joining while it could block on send would hang.)
        self.jobs_tx.take();
        self.bed_rx.take();
        if let Some(handle) = self.rebuilder.take() {
            let _ = handle.join();
        }
    }
}
