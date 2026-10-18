//! Parameters for Soundslop Kit: 83 params total.
//!
//! Eight slots × 10 params (`category`, `kind`, `voice_mob`, `note`, `knob_a`,
//! `knob_b`, `gain`, `pan`, `seed`, `fresh`) + master `output_gain`,
//! `polyphony`, `humanize`.
//!
//! Like Ambience (Task 10), every slot param id is the exact literal
//! `s{N}_{name}` produced by nesting a shared [`SlotParams`] under
//! `#[nested(id_prefix = "s1", ...)]` — the framework builds `format!("{prefix}_{key}")`.
//! This is *not* `nested(array)` (which yields `{key}_{index}`).

use nih_plug::prelude::{
    BoolParam, Enum, EnumParam, FloatParam, FloatRange, IntParam, IntRange, Params,
};

use foley_engine::SlotCategory;

/// The six slot categories, mirroring [`SlotCategory`] (which doesn't impl nih Enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum SlotKind {
    Contact,
    Fracture,
    Shot,
    Primitive,
    Voice,
    Note,
}

impl SlotKind {
    /// Map to the engine's [`SlotCategory`].
    pub fn to_category(self) -> SlotCategory {
        match self {
            SlotKind::Contact => SlotCategory::Contact,
            SlotKind::Fracture => SlotCategory::Fracture,
            SlotKind::Shot => SlotCategory::Shot,
            SlotKind::Primitive => SlotCategory::Primitive,
            SlotKind::Voice => SlotCategory::Voice,
            SlotKind::Note => SlotCategory::Note,
        }
    }
}

/// Master polyphony: 4, 8 or 16 voices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum Polyphony {
    V4,
    V8,
    V16,
}

impl Polyphony {
    /// The voice count this setting maps to.
    pub fn count(self) -> usize {
        match self {
            Polyphony::V4 => 4,
            Polyphony::V8 => 8,
            Polyphony::V16 => 16,
        }
    }
}

/// The ten per-slot params, nested under a `s{N}_` id prefix.
#[derive(Params)]
pub struct SlotParams {
    #[id = "category"]
    pub category: EnumParam<SlotKind>,
    #[id = "kind"]
    pub kind: IntParam,
    #[id = "voice_mob"]
    pub voice_mob: IntParam,
    #[id = "note"]
    pub note: IntParam,
    #[id = "knob_a"]
    pub knob_a: FloatParam,
    #[id = "knob_b"]
    pub knob_b: FloatParam,
    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "pan"]
    pub pan: FloatParam,
    #[id = "seed"]
    pub seed: IntParam,
    #[id = "fresh"]
    pub fresh: BoolParam,
}

impl SlotParams {
    /// Build a slot's params with the given per-index seed default.
    pub fn new(seed_default: i32) -> Self {
        SlotParams {
            category: EnumParam::new("Category", SlotKind::Contact),
            kind: IntParam::new("Kind", 0, IntRange::Linear { min: 0, max: 62 }),
            voice_mob: IntParam::new("Voice", 0, IntRange::Linear { min: 0, max: 51 }),
            note: IntParam::new("Note", 60, IntRange::Linear { min: 0, max: 127 }),
            knob_a: FloatParam::new("Knob A", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            knob_b: FloatParam::new("Knob B", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            gain: FloatParam::new("Gain", 0.8, FloatRange::Linear { min: 0.0, max: 1.5 }),
            pan: FloatParam::new(
                "Pan",
                0.0,
                FloatRange::Linear {
                    min: -100.0,
                    max: 100.0,
                },
            ),
            seed: IntParam::new(
                "Seed",
                seed_default,
                IntRange::Linear {
                    min: 0,
                    max: 16_777_215,
                },
            ),
            fresh: BoolParam::new("Fresh", true),
        }
    }
}

/// The full 83-param set for Soundslop Kit.
#[derive(Params)]
pub struct KitParams {
    #[id = "output_gain"]
    pub output_gain: FloatParam,
    #[id = "polyphony"]
    pub polyphony: EnumParam<Polyphony>,
    #[id = "humanize"]
    pub humanize: FloatParam,
    #[nested(id_prefix = "s1", group = "Slot 1")]
    pub slot1: SlotParams,
    #[nested(id_prefix = "s2", group = "Slot 2")]
    pub slot2: SlotParams,
    #[nested(id_prefix = "s3", group = "Slot 3")]
    pub slot3: SlotParams,
    #[nested(id_prefix = "s4", group = "Slot 4")]
    pub slot4: SlotParams,
    #[nested(id_prefix = "s5", group = "Slot 5")]
    pub slot5: SlotParams,
    #[nested(id_prefix = "s6", group = "Slot 6")]
    pub slot6: SlotParams,
    #[nested(id_prefix = "s7", group = "Slot 7")]
    pub slot7: SlotParams,
    #[nested(id_prefix = "s8", group = "Slot 8")]
    pub slot8: SlotParams,
}

impl KitParams {
    /// Per-index seed defaults for the eight slots (deterministic, distinct).
    const SEED_DEFAULTS: [i32; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
}

impl Default for KitParams {
    fn default() -> Self {
        let [s1, s2, s3, s4, s5, s6, s7, s8] = Self::SEED_DEFAULTS;
        KitParams {
            output_gain: FloatParam::new(
                "Output Gain",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 6.0,
                },
            )
            .with_unit(" dB"),
            polyphony: EnumParam::new("Polyphony", Polyphony::V8),
            humanize: FloatParam::new("Humanize", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            slot1: SlotParams::new(s1),
            slot2: SlotParams::new(s2),
            slot3: SlotParams::new(s3),
            slot4: SlotParams::new(s4),
            slot5: SlotParams::new(s5),
            slot6: SlotParams::new(s6),
            slot7: SlotParams::new(s7),
            slot8: SlotParams::new(s8),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let p = KitParams::default();
        assert_eq!(p.output_gain.value(), 0.0);
        assert_eq!(p.polyphony.value(), Polyphony::V8);
        assert_eq!(p.humanize.value(), 0.0);

        let s1 = &p.slot1;
        assert_eq!(s1.category.value(), SlotKind::Contact);
        assert_eq!(s1.note.value(), 60);
        assert_eq!(s1.gain.value(), 0.8);
        assert_eq!(s1.knob_a.value(), 0.5);
        assert_eq!(s1.knob_b.value(), 0.5);
        assert_eq!(s1.pan.value(), 0.0);
        assert!(s1.fresh.value());

        // Seed defaults are distinct and seeded by index.
        assert_eq!(p.slot1.seed.value(), 1);
        assert_eq!(p.slot8.seed.value(), 8);
    }

    #[test]
    fn polyphony_enum_maps_to_counts() {
        assert_eq!(Polyphony::V4.count(), 4);
        assert_eq!(Polyphony::V8.count(), 8);
        assert_eq!(Polyphony::V16.count(), 16);
    }

    #[test]
    fn slot_kind_maps_to_category() {
        assert_eq!(SlotKind::Contact.to_category(), SlotCategory::Contact);
        assert_eq!(SlotKind::Note.to_category(), SlotCategory::Note);
    }

    #[test]
    fn param_ids_are_exact_s_n_form() {
        let p = KitParams::default();
        let ids: Vec<String> = p.param_map().into_iter().map(|(id, _, _)| id).collect();
        assert!(ids.contains(&"s1_category".to_string()), "got {ids:?}");
        assert!(ids.contains(&"s5_note".to_string()), "got {ids:?}");
        assert!(ids.contains(&"s8_fresh".to_string()), "got {ids:?}");
        assert!(ids.contains(&"output_gain".to_string()), "got {ids:?}");
        assert!(ids.contains(&"polyphony".to_string()), "got {ids:?}");
        assert!(ids.contains(&"humanize".to_string()), "got {ids:?}");
        // 83 total: 8 slots × 10 + 3 master.
        assert_eq!(ids.len(), 83, "got {ids:?}");
    }
}
