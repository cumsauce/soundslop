//! The Kit's single dispatch: a [`RenderJob`] → the right foley one-shot.
//!
//! Every knob law here is pinned by the spec (§11). The contract is total:
//! an out-of-range `kind` (or a `voice_mob`/`kind` that doesn't index its
//! enum) renders an **empty** clip — it never panics.

use foley_synth::{
    Birds, Call, Contact, Material, Mob, Plant, aeolian, bang, bell, birdsong2, brush, bubble,
    buzz, cicadas, contact, crickets, drip_into_water, drip_on_stone, flush, fracture, frog, gust,
    howl, ice_singing, lava_drip, noise_burst, note, owl, sculk_ring, settling_stone, surf, tone,
    voice, woodpecker,
};

use crate::clip::{RenderJob, SlotCategory};

/// Kit-chosen fixed durations/gains for the primitives that take them
/// (the spec pins the knob laws, not these; they match the mod's example
/// calls).
const NOISE_BURST_SECONDS: f32 = 0.5;
const NOISE_BURST_GAIN: f32 = 0.9;
const TONE_SECONDS: f32 = 0.5;
const TONE_GAIN: f32 = 0.9;
const GUST_SECONDS: f32 = 2.0;
const GUST_GAIN: f32 = 0.9;
const AEOLIAN_SECONDS: f32 = 2.0;
const AEOLIAN_GAIN: f32 = 0.9;
/// Boom's default radius (m) when knob A sits at or below 0 — the mod's
/// fallback.
const BOOM_DEFAULT_RADIUS_M: f32 = 2.0;

/// Max inclusive `kind` index per category. `voice_mob` is separate.
pub fn kind_range(category: SlotCategory) -> u32 {
    match category {
        SlotCategory::Contact => 39,
        SlotCategory::Fracture => 9,
        SlotCategory::Shot => 18,
        SlotCategory::Primitive => 5,
        SlotCategory::Voice => 42,
        SlotCategory::Note => 15,
    }
}

/// Per-category knob captions for the GUI (spec §11; per-shot semantics
/// live in the spec's table, not here).
pub fn kit_knob_labels(category: SlotCategory) -> [&'static str; 2] {
    match category {
        SlotCategory::Contact => ["speed mult", ""],
        SlotCategory::Fracture => ["", ""],
        SlotCategory::Shot => ["size", ""],
        SlotCategory::Primitive => ["a", "b"],
        SlotCategory::Voice => ["", ""],
        SlotCategory::Note => ["character", ""],
    }
}

/// Render one clip for a Kit hit. Total: bad indices render empty.
pub fn render_clip(job: &RenderJob) -> Vec<f32> {
    let a = job.knobs[0];
    let b = job.knobs[1];
    let seed = job.seed;
    let rate = job.sample_rate;
    match job.category {
        SlotCategory::Contact => {
            let (Some(ct), Some(mat)) = (
                Contact::from_index((job.kind % 4) as usize),
                Material::from_index((job.kind / 4) as usize),
            ) else {
                return Vec::new();
            };
            // Spec §11: velocity maps impact speed = 0.5 + 5.0·velocity m/s;
            // knob A is a speed multiplier 0.25×..4× with 0.5 = 1×.
            let mult = 0.25 * 16.0_f32.powf(a);
            let speed = (0.5 + 5.0 * job.velocity) * mult;
            contact(ct, mat, speed, seed, rate)
        }
        SlotCategory::Fracture => {
            let Some(material) = Material::from_index(job.kind as usize) else {
                return Vec::new();
            };
            fracture(material, seed, rate)
        }
        SlotCategory::Shot => shot(job.kind, a, seed, rate),
        SlotCategory::Primitive => primitive(job.kind, a, b, seed, rate),
        SlotCategory::Voice => {
            let (Some(mob), Some(call)) = (
                Mob::from_index(job.voice_mob as usize),
                Call::from_index(job.kind as usize),
            ) else {
                return Vec::new();
            };
            voice(mob, call, seed, rate)
        }
        SlotCategory::Note => note(job.kind, job.pitch as f32, a, seed, rate).unwrap_or_default(),
    }
}

/// The 19 named shots, kinds 0..=18 (mod foley-ffi's canonical order).
fn shot(kind: u32, a: f32, seed: u64, rate: u32) -> Vec<f32> {
    match kind {
        // Drips take radius in metres; knob A → 0.5..=4 mm.
        0 => {
            let mm = 0.5 + 3.5 * a;
            drip_into_water(mm / 1000.0, seed, rate)
        }
        1 => {
            let mm = 0.5 + 3.5 * a;
            drip_on_stone(mm / 1000.0, seed, rate)
        }
        2 => lava_drip(seed, rate),
        3 => crickets(-5.0 + 40.0 * a, seed, rate),
        // Birds::of wraps modulo, so any A stays in range.
        4 => birdsong2(Birds::of((a * 56.0) as u32), seed, rate),
        5 => frog(2.0 + 18.0 * a, seed, rate),
        6 => surf(0.2 + 2.8 * a, seed, rate),
        7 => woodpecker(seed, rate),
        8 => owl(seed, rate),
        9 => cicadas(-30.0 + 80.0 * a, seed, rate),
        10 => howl(seed, rate),
        // Plant::of clamps, so any A stays in range.
        11 => brush(Plant::of((a * 2.0).round() as u32), seed, rate),
        12 => ice_singing(seed, rate),
        13 => settling_stone(seed, rate),
        14 => flush(seed, rate),
        15 => buzz(if a < 0.5 { 0 } else { 1 }, seed, rate),
        16 => sculk_ring(0.5 + 19.5 * a, seed, rate),
        17 => bell(0.5 + 19.5 * a, seed, rate),
        // Boom: knob A → 0..=5 m; at/below 0 the mod's default 2.0 m.
        18 => bang(
            if a <= 0.0 {
                BOOM_DEFAULT_RADIUS_M
            } else {
                a * 5.0
            },
            seed,
            rate,
        ),
        _ => Vec::new(),
    }
}

/// Pinned primitive order: kinds 0..=5 = bubble, noise_burst, tone, gust,
/// aeolian, howl. Knobs A/B are the first two float args each takes.
fn primitive(kind: u32, a: f32, b: f32, seed: u64, rate: u32) -> Vec<f32> {
    match kind {
        // A → radius, log scale 0.05..=20 mm.
        0 => bubble(0.05 * 400.0_f32.powf(a), seed, rate),
        // A → centre 20..=20000 Hz (log), B → q 0.5..=20.
        1 => noise_burst(
            20.0 * 1000.0_f32.powf(a),
            0.5 + 19.5 * b,
            NOISE_BURST_SECONDS,
            NOISE_BURST_GAIN,
            seed,
            rate,
        ),
        // A → start 20..=10000 Hz (log), B → target 20..=20000 Hz (log).
        2 => tone(
            20.0 * 500.0_f32.powf(a),
            20.0 * 1000.0_f32.powf(b),
            TONE_SECONDS,
            TONE_GAIN,
            seed,
            rate,
        ),
        // A → speed 0..=60 m/s.
        3 => gust(60.0 * a, GUST_SECONDS, GUST_GAIN, seed, rate),
        // A → diameter 0.005..=0.5 m, B → wind 0.5..=30 m/s.
        4 => aeolian(
            0.005 + 0.495 * a,
            0.5 + 29.5 * b,
            AEOLIAN_SECONDS,
            AEOLIAN_GAIN,
            seed,
            rate,
        ),
        5 => howl(seed, rate),
        _ => Vec::new(),
    }
}
