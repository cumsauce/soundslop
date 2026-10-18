//! Knob-to-scene mapping and per-kind knob captions.
//!
//! Knobs arrive as `[a, b, c, d]`, each 0..=1. `d` is unused at every kind.

use foley_dsp::{
    CaveScene, FireScene, LavaScene, RainScene, Surface, UnderwaterScene, WaterScene, WindScene,
};
use foley_synth::{BiomeKind, BiomeScene, SurfScene, ThunderScene};

use crate::BedKind;

/// Rain surface-share presets, indexed 0..=4.
/// Surface order: Water, Stone, Leaves, Wood, Soft, Glass, Metal.
const RAIN_PRESETS: [[f32; 7]; 5] = [
    // Field
    [0.0, 0.05, 0.05, 0.05, 0.85, 0.0, 0.0],
    // City
    [0.0, 0.75, 0.0, 0.05, 0.2, 0.0, 0.0],
    // Leaves
    [0.0, 0.05, 0.85, 0.05, 0.05, 0.0, 0.0],
    // Ocean
    [0.9, 0.05, 0.0, 0.0, 0.05, 0.0, 0.0],
    // Glass
    [0.0, 0.1, 0.0, 0.0, 0.0, 0.8, 0.1],
];

/// The dominant (loudest-share) surface of a rain preset.
fn dominant_surface(preset: usize) -> Surface {
    let row = &RAIN_PRESETS[preset];
    let mut best = 0;
    for (i, &share) in row.iter().enumerate() {
        if share > row[best] {
            best = i;
        }
    }
    Surface::from_index(best).unwrap_or(Surface::Soft)
}

/// A fully constructed scene for one bed kind.
pub(crate) enum SceneArgs {
    Rain(RainScene),
    Wind(WindScene),
    Water(WaterScene),
    Fire(FireScene),
    Lava(LavaScene),
    Thunder(ThunderScene),
    Cave(CaveScene),
    Underwater(UnderwaterScene),
    Biome(BiomeScene),
    Surf(SurfScene),
}

/// Computes each bed's `set_scene` arguments from four 0..=1 knobs.
pub(crate) fn scene_from_knobs(kind: BedKind, knobs: [f32; 4]) -> SceneArgs {
    let [a, b, c, _d] = knobs;
    match kind {
        BedKind::Rain => {
            let p = (c * 4.0).round().clamp(0.0, 4.0) as usize;
            SceneArgs::Rain(RainScene {
                rate: a * 50.0,
                surfaces: RAIN_PRESETS[p],
                roof: if b == 0.0 {
                    None
                } else {
                    Some((dominant_surface(p), (b * 20.0).round()))
                },
            })
        }
        BedKind::Wind => SceneArgs::Wind(WindScene {
            speed: a * 30.0,
            exposure: b,
            leaves: c,
        }),
        BedKind::Water => SceneArgs::Water(WaterScene {
            flow: a * 20.0,
            falls: b * 20.0,
            gain: 1.0,
        }),
        BedKind::Fire => SceneArgs::Fire(FireScene {
            size: a * 10.0,
            gain: 1.0,
        }),
        BedKind::Lava => SceneArgs::Lava(LavaScene {
            area: a * 15.0,
            gain: 1.0,
        }),
        BedKind::Thunder => SceneArgs::Thunder(ThunderScene {
            celsius: 20.0,
            humidity: 60.0,
            exposure: a,
        }),
        BedKind::Cave => SceneArgs::Cave(CaveScene {
            depth: a,
            mass: 0.3,
            openness: c,
            drips: b,
            gain: 1.0,
        }),
        BedKind::Underwater => SceneArgs::Underwater(UnderwaterScene { level: a }),
        BedKind::Biome => SceneArgs::Biome(BiomeScene {
            kind: BiomeKind::from_index((a * 6.0).round() as u32).unwrap_or(BiomeKind::Desert),
            intensity: b,
            sub: c * 10.0,
            gain: 1.0,
        }),
        BedKind::Surf => SceneArgs::Surf(SurfScene {
            height: (a * 3.0).clamp(0.2, 3.0),
            shore: c,
            storm: b,
            exposure: 0.5,
        }),
    }
}

/// Per-kind captions for the four scene knobs (GUI labels).
pub fn knob_labels(kind: BedKind) -> [&'static str; 4] {
    match kind {
        BedKind::Rain => ["rate mm/h", "roof height (blocks)", "surface preset", ""],
        BedKind::Wind => ["speed m/s", "exposure", "leaves", ""],
        BedKind::Water => ["flow (blocks)", "falls (blocks)", "", ""],
        BedKind::Fire => ["size (blocks)", "", "", ""],
        BedKind::Lava => ["area (blocks)", "", "", ""],
        BedKind::Thunder => ["exposure", "strike (momentary)", "height (blocks)", ""],
        BedKind::Cave => ["depth", "drips", "openness", ""],
        BedKind::Underwater => ["level", "", "", ""],
        BedKind::Biome => ["kind", "intensity", "sub", ""],
        BedKind::Surf => ["height (m)", "storm", "shore", ""],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rain_preset_quantises_and_roof_gates_on_b() {
        // a=0 b=0 c=0 → preset 0 (Field), roof None.
        let SceneArgs::Rain(s) = scene_from_knobs(BedKind::Rain, [0.0, 0.0, 0.0, 0.0]) else {
            panic!("expected Rain scene");
        };
        assert_eq!(s.rate, 0.0);
        assert!(s.roof.is_none());
        assert_eq!(s.surfaces, RAIN_PRESETS[0]);

        // c=1 → preset 4 (Glass); b=0.5 → roof Some, height 10.
        let SceneArgs::Rain(s) = scene_from_knobs(BedKind::Rain, [0.0, 0.5, 1.0, 0.0]) else {
            panic!("expected Rain scene");
        };
        let Some((surface, height)) = s.roof else {
            panic!("expected a roof at b=0.5");
        };
        assert_eq!(surface, Surface::Glass); // Glass preset's argmax
        assert_eq!(height, 10.0);

        // c=0.5 → preset 2 (Leaves).
        let SceneArgs::Rain(s) = scene_from_knobs(BedKind::Rain, [0.0, 0.0, 0.5, 0.0]) else {
            panic!("expected Rain scene");
        };
        assert_eq!(s.surfaces, RAIN_PRESETS[2]);
    }

    #[test]
    fn biome_kind_quantises_sixths() {
        for (knob, expected) in [
            (0.0, BiomeKind::Desert),
            (1.0 / 6.0, BiomeKind::Mushroom),
            (2.0 / 6.0, BiomeKind::Lush),
            (0.5, BiomeKind::Dripstone),
            (1.0, BiomeKind::Mangrove),
        ] {
            let SceneArgs::Biome(s) = scene_from_knobs(BedKind::Biome, [knob, 0.5, 0.5, 0.0])
            else {
                panic!("expected Biome scene");
            };
            assert_eq!(s.kind, expected);
        }
    }
}
