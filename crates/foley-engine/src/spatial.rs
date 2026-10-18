//! Spatial helpers: equal-power panning and per-layer air absorption.

use foley_dsp::absorption_cutoff;

/// Equal-power pan gains for an azimuth in degrees (0 = centre/front,
/// −90 = full left, +90 = full right, ±180 = behind).
pub fn pan_gains(az_deg: f32) -> (f32, f32) {
    let x = (az_deg + 90.0) / 180.0;
    let theta = x * std::f32::consts::FRAC_PI_2;
    (theta.cos(), theta.sin())
}

/// A one-pole low-pass, coefficient form `y += a·(x − y)` with
/// `a = min(1, cutoff/sample_rate)` (exact passthrough at `cutoff == rate`).
#[derive(Default)]
pub struct OnePoleLpf {
    y: f32,
}

impl OnePoleLpf {
    pub fn new() -> Self {
        Self { y: 0.0 }
    }

    pub fn reset(&mut self) {
        self.y = 0.0;
    }

    pub fn process(&mut self, x: f32, cutoff_hz: f32, sample_rate: f32) -> f32 {
        let a = (cutoff_hz / sample_rate).clamp(0.0, 1.0);
        self.y += a * (x - self.y);
        self.y
    }
}

/// The air-absorption cutoff for a layer `dist_blocks` away, in Hz:
/// nyquist at 0 blocks, otherwise the ISO 9613-1 6 dB point at 15 °C / 65 % RH
/// (spec §10), capped at nyquist. `None` from the absorber means the air
/// takes too little to matter → nyquist.
pub fn layer_cutoff(dist_blocks: f32, sample_rate: f32) -> f32 {
    let nyquist = sample_rate / 2.0;
    if dist_blocks <= 0.0 {
        return nyquist;
    }
    match absorption_cutoff(dist_blocks, 15.0, 65.0) {
        Some(f) => f.min(nyquist).max(20.0),
        None => nyquist,
    }
}
