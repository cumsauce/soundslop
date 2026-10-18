//! The one seed law both plugins share: every Kit hit and every Ambience
//! layer derives its RNG seed from this mix.
//!
//! **Stable forever.** Presets pin their sound to these exact bytes; any
//! change here silently retunes every saved preset.

/// splitmix64 finalizer: scrambles one u64 into well-mixed bits.
#[inline]
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Mix a slot/layer `base` seed with a note, a velocity and a counter into
/// the seed one hit or render consumes. Distinct domain tags keep the four
/// fields from cancelling each other out.
#[inline]
pub fn hit_seed(base: u64, note: u8, velocity: f32, counter: u32) -> u64 {
    let mixed = base
        ^ splitmix64(note as u64).wrapping_mul(0xA24B_AED4_96E5_C7B1)
        ^ splitmix64(velocity.to_bits() as u64).wrapping_mul(0x9FB2_1C65_1E98_DF25)
        ^ splitmix64(counter as u64).wrapping_mul(0xC1E1_4F37_D8A7_CE43);
    splitmix64(mixed)
}
