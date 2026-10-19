//! Shared engine constants and re-exports for the Soundslop plugins.

/// Supported host sample-rate range (spec §14).
pub const MIN_SAMPLE_RATE: u32 = 32_000;
/// Supported host sample-rate range (spec §14).
pub const MAX_SAMPLE_RATE: u32 = 192_000;

mod bed_union;
mod clip;
mod guard;
mod kind_map;
mod kit_map;
mod motion;
mod rebuild;
mod rig_ambience;
mod rig_kit;
mod seed_mix;
mod spatial;

pub use bed_union::{BedKind, BedUnion, new_bed};
pub use clip::{EngineJob, RenderJob, SlotCategory, spawn_clip_worker};
pub use guard::GainGuard;
pub use kind_map::knob_labels;
pub use kit_map::{kind_range, kit_knob_labels, render_clip};
pub use motion::Motion;
pub use rebuild::{RebuildJob, spawn_rebuilder};
pub use rig_ambience::{AmbienceRig, LAYERS, LayerState};
pub use rig_kit::{KitRig, SLOTS, SlotState};
pub use seed_mix::hit_seed;
pub use spatial::{OnePoleLpf, layer_cutoff, pan_gains};

pub const MAX_BLOCK: usize = 512;
pub const MAX_VOICES: usize = 16;
pub const LAYER_CEILING: f32 = 0.4;
pub const MASTER_CEILING: f32 = 0.85;

pub use foley_dsp;
pub use foley_synth;
