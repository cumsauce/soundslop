//! Seeded random-walk offsets for scene knobs ("motion"): an additive
//! wander on top of automation, never an override (spec §10).

use foley_dsp::Rng;

/// Four wandering offsets in `[−amt, +amt]`, re-targeted on a cadence
/// driven by `rate` (0 = frozen, 1 = ten updates per second).
pub struct Motion {
    amt: f32,
    interval: f32,
    rng: Rng,
    state: [f32; 4],
    target: [f32; 4],
    acc: f32,
}

impl Motion {
    pub fn new(amt: f32, rate: f32, seed: u64) -> Self {
        Self {
            amt,
            interval: 1.0 / (rate * 10.0).max(f32::MIN_POSITIVE),
            rng: Rng::new(seed),
            state: [0.0; 4],
            target: [0.0; 4],
            acc: 0.0,
        }
    }

    /// Seconds `dt` since the last call; returns the current offsets.
    pub fn next(&mut self, dt: f32) -> [f32; 4] {
        if self.amt == 0.0 {
            return [0.0; 4];
        }
        self.acc += dt;
        while self.acc >= self.interval {
            self.acc -= self.interval;
            for c in 0..4 {
                self.target[c] = (self.rng.unit() * 2.0 - 1.0) * self.amt;
            }
        }
        // Convex step toward the target: state stays within [−amt, amt].
        for c in 0..4 {
            self.state[c] += (self.target[c] - self.state[c]) * 0.25;
        }
        self.state
    }
}
