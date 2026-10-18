//! Envelope-follower gain guard (mechanism mirrored from foley-ffi's
//! per-bed ceiling guard: fast attack, smooth release, instant-cap mix).

/// Holds `|out|` at or below `ceiling` by damping hot input, transparent
/// when the signal is quiet.
pub struct GainGuard {
    ceiling: f32,
    envelope: f32,
    guard: f32,
}

impl GainGuard {
    pub fn new(ceiling: f32) -> Self {
        Self {
            ceiling,
            envelope: 0.0,
            guard: 1.0,
        }
    }

    pub fn reset(&mut self) {
        self.envelope = 0.0;
        self.guard = 1.0;
    }

    pub fn process(&mut self, x: f32) -> f32 {
        // One non-finite sample must not poison the state.
        let x = if x.is_finite() { x } else { 0.0 };
        self.envelope = x.abs().max(self.envelope * 0.99996);
        let wanted = (self.ceiling / self.envelope.max(1e-6)).min(1.0);
        let rate = if wanted < self.guard { 0.01 } else { 0.0001 };
        self.guard += (wanted - self.guard) * rate;
        x * self.guard.min(wanted)
    }
}
