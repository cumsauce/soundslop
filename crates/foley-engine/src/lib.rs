//! Shared engine constants and re-exports for the Soundslop plugins.

pub const MAX_BLOCK: usize = 512;
pub const MAX_VOICES: usize = 16;
pub const LAYER_CEILING: f32 = 0.4;
pub const MASTER_CEILING: f32 = 0.85;

pub use foley_dsp;
pub use foley_synth;
