//! Parameters for Soundslop Ambience: 68 params total.
//!
//! Six layers × 11 params (`kind`, `gain`, `seed`, `motion_amt`, `motion_rate`,
//! `dist`, `az`, `knob_a`..`knob_d`) + master `output_gain` and `scene_glide`.
//!
//! Every layer param id is the exact literal `l{N}_{name}` (e.g. `l1_kind`,
//! `l3_knob_a`). This is produced by nesting a shared [`LayerParams`] struct
//! under `#[nested(id_prefix = "l1", ...)]`: nice-plug's derive builds each id
//! as `format!("{prefix}_{key}")`, so the inner bare id `"kind"` under prefix
//! `"l1"` becomes exactly `l1_kind`. This is *not* `nested(array)` (which
//! would synthesise `{key}_{index}`) — the brief forbids that form.

use nice_plug::prelude::{Enum, EnumParam, FloatParam, FloatRange, IntParam, IntRange, Params};
use serde::{Deserialize, Serialize};

/// Editor window geometry, persisted with the plugin state. `EguiEditorState` itself is not
/// serializable, so we persist the logical size and rebuild the state in `Plugin::editor()`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EditorSize {
    pub width: f32,
    pub height: f32,
}

impl Default for EditorSize {
    fn default() -> Self {
        // Preferred 576 × 720 per the UI contract; minimum 380 × 520.
        Self {
            width: 576.0,
            height: 720.0,
        }
    }
}

/// The eleven layer kinds: `Off` first, then the ten beds in [`BedKind`] order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum LayerKind {
    Off,
    Rain,
    Wind,
    Water,
    Fire,
    Lava,
    Thunder,
    Cave,
    Underwater,
    Biome,
    Surf,
}

use foley_engine::BedKind;

impl LayerKind {
    /// Map a layer kind to the engine bed it selects. `Off` → `None`.
    pub fn to_bed_kind(self) -> Option<BedKind> {
        match self {
            LayerKind::Off => None,
            LayerKind::Rain => Some(BedKind::Rain),
            LayerKind::Wind => Some(BedKind::Wind),
            LayerKind::Water => Some(BedKind::Water),
            LayerKind::Fire => Some(BedKind::Fire),
            LayerKind::Lava => Some(BedKind::Lava),
            LayerKind::Thunder => Some(BedKind::Thunder),
            LayerKind::Cave => Some(BedKind::Cave),
            LayerKind::Underwater => Some(BedKind::Underwater),
            LayerKind::Biome => Some(BedKind::Biome),
            LayerKind::Surf => Some(BedKind::Surf),
        }
    }

    /// Inverse of [`LayerKind::to_bed_kind`]. `None` → `Off`.
    pub fn from_bed_kind(bed: Option<BedKind>) -> Self {
        match bed {
            None => LayerKind::Off,
            Some(BedKind::Rain) => LayerKind::Rain,
            Some(BedKind::Wind) => LayerKind::Wind,
            Some(BedKind::Water) => LayerKind::Water,
            Some(BedKind::Fire) => LayerKind::Fire,
            Some(BedKind::Lava) => LayerKind::Lava,
            Some(BedKind::Thunder) => LayerKind::Thunder,
            Some(BedKind::Cave) => LayerKind::Cave,
            Some(BedKind::Underwater) => LayerKind::Underwater,
            Some(BedKind::Biome) => LayerKind::Biome,
            Some(BedKind::Surf) => LayerKind::Surf,
        }
    }
}

/// The eleven per-layer params, nested under a `l{N}_` id prefix.
///
/// The seed default is supplied by the caller so each layer can get a distinct
/// per-index default (spec §10: "seed ... default seeded by index").
#[derive(Params)]
pub struct LayerParams {
    #[id = "kind"]
    pub kind: EnumParam<LayerKind>,
    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "seed"]
    pub seed: IntParam,
    #[id = "motion_amt"]
    pub motion_amt: FloatParam,
    #[id = "motion_rate"]
    pub motion_rate: FloatParam,
    #[id = "dist"]
    pub dist: FloatParam,
    #[id = "az"]
    pub az: FloatParam,
    #[id = "knob_a"]
    pub knob_a: FloatParam,
    #[id = "knob_b"]
    pub knob_b: FloatParam,
    #[id = "knob_c"]
    pub knob_c: FloatParam,
    #[id = "knob_d"]
    pub knob_d: FloatParam,
}

impl LayerParams {
    /// Build a layer's params with the given per-index seed default.
    pub fn new(seed_default: i32) -> Self {
        LayerParams {
            kind: EnumParam::new("Kind", LayerKind::Off),
            gain: FloatParam::new("Gain", 0.5, FloatRange::Linear { min: 0.0, max: 1.5 }),
            seed: IntParam::new(
                "Seed",
                seed_default,
                IntRange::Linear {
                    min: 0,
                    max: 16_777_215,
                },
            ),
            motion_amt: FloatParam::new(
                "Motion Amount",
                0.15,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            motion_rate: FloatParam::new(
                "Motion Rate",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            dist: FloatParam::new(
                "Distance",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 64.0,
                },
            ),
            az: FloatParam::new(
                "Azimuth",
                0.0,
                FloatRange::Linear {
                    min: -180.0,
                    max: 180.0,
                },
            ),
            knob_a: FloatParam::new("Knob A", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            knob_b: FloatParam::new("Knob B", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            knob_c: FloatParam::new("Knob C", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            knob_d: FloatParam::new("Knob D", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
        }
    }
}

/// The full 68-param set for Soundslop Ambience.
#[derive(Params)]
pub struct AmbienceParams {
    /// Editor window size, saved with the plugin state.
    #[persist = "editor-state"]
    pub editor_state: std::sync::RwLock<EditorSize>,
    #[id = "output_gain"]
    pub output_gain: FloatParam,
    #[id = "scene_glide"]
    pub scene_glide: FloatParam,
    #[nested(id_prefix = "l1", group = "Layer 1")]
    pub layer1: LayerParams,
    #[nested(id_prefix = "l2", group = "Layer 2")]
    pub layer2: LayerParams,
    #[nested(id_prefix = "l3", group = "Layer 3")]
    pub layer3: LayerParams,
    #[nested(id_prefix = "l4", group = "Layer 4")]
    pub layer4: LayerParams,
    #[nested(id_prefix = "l5", group = "Layer 5")]
    pub layer5: LayerParams,
    #[nested(id_prefix = "l6", group = "Layer 6")]
    pub layer6: LayerParams,
}

impl AmbienceParams {
    /// Per-index seed defaults for the six layers (deterministic, distinct).
    const SEED_DEFAULTS: [i32; 6] = [1, 2, 3, 4, 5, 6];
}

impl Default for AmbienceParams {
    fn default() -> Self {
        let [s1, s2, s3, s4, s5, s6] = Self::SEED_DEFAULTS;
        AmbienceParams {
            editor_state: std::sync::RwLock::new(EditorSize::default()),
            output_gain: FloatParam::new(
                "Output Gain",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 6.0,
                },
            )
            .with_unit(" dB"),
            scene_glide: FloatParam::new(
                "Scene Glide",
                0.30,
                FloatRange::Linear {
                    min: 0.02,
                    max: 1.0,
                },
            )
            .with_unit(" s"),
            layer1: LayerParams::new(s1),
            layer2: LayerParams::new(s2),
            layer3: LayerParams::new(s3),
            layer4: LayerParams::new(s4),
            layer5: LayerParams::new(s5),
            layer6: LayerParams::new(s6),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_kind_conversion_is_total() {
        // Every LayerKind variant maps to/from Option<BedKind>; Off → None.
        assert_eq!(LayerKind::Off.to_bed_kind(), None);
        assert_eq!(LayerKind::from_bed_kind(None), LayerKind::Off);

        let all = [
            LayerKind::Rain,
            LayerKind::Wind,
            LayerKind::Water,
            LayerKind::Fire,
            LayerKind::Lava,
            LayerKind::Thunder,
            LayerKind::Cave,
            LayerKind::Underwater,
            LayerKind::Biome,
            LayerKind::Surf,
        ];
        for kind in all {
            let bed = kind.to_bed_kind();
            assert!(bed.is_some(), "{kind:?} should map to a bed");
            assert_eq!(LayerKind::from_bed_kind(bed), kind);
        }
    }

    #[test]
    fn defaults_match_spec() {
        let p = AmbienceParams::default();
        assert_eq!(p.output_gain.value(), 0.0);
        assert_eq!(p.scene_glide.value(), 0.30);

        let l1 = &p.layer1;
        assert_eq!(l1.kind.value(), LayerKind::Off);
        assert_eq!(l1.gain.value(), 0.5);
        assert_eq!(l1.motion_amt.value(), 0.15);
        assert_eq!(l1.dist.value(), 0.0);
        assert_eq!(l1.az.value(), 0.0);
        assert_eq!(l1.knob_a.value(), 0.5);
        assert_eq!(l1.knob_b.value(), 0.5);
        assert_eq!(l1.knob_c.value(), 0.5);
        assert_eq!(l1.knob_d.value(), 0.5);

        // Seed defaults are distinct and seeded by index.
        assert_eq!(p.layer1.seed.value(), 1);
        assert_eq!(p.layer2.seed.value(), 2);
        assert_eq!(p.layer6.seed.value(), 6);
    }

    #[test]
    fn param_ids_are_exact_l_n_form() {
        // The nested id_prefix must produce the exact literal ids the spec
        // pins (l1_kind, l3_knob_a, ...), not nested(array)'s {key}_{index}.
        let p = AmbienceParams::default();
        let ids: Vec<String> = p.param_map().into_iter().map(|(id, _, _)| id).collect();
        assert!(ids.contains(&"l1_kind".to_string()), "got {ids:?}");
        assert!(ids.contains(&"l3_knob_a".to_string()), "got {ids:?}");
        assert!(ids.contains(&"l6_knob_d".to_string()), "got {ids:?}");
        assert!(ids.contains(&"output_gain".to_string()), "got {ids:?}");
        assert!(ids.contains(&"scene_glide".to_string()), "got {ids:?}");
        // 68 total: 6 layers × 11 + 2 master.
        assert_eq!(ids.len(), 68, "got {ids:?}");
    }
}
