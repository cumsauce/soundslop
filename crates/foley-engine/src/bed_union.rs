//! One variant per ambience bed; enum dispatch only, no new audio logic.

use crate::kind_map::{SceneArgs, scene_from_knobs};

/// The ten ambience beds, in the order the plugin's kind enum uses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BedKind {
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

impl BedKind {
    /// Every bed kind, in enum order.
    pub fn all() -> [BedKind; 10] {
        [
            BedKind::Rain,
            BedKind::Wind,
            BedKind::Water,
            BedKind::Fire,
            BedKind::Lava,
            BedKind::Thunder,
            BedKind::Cave,
            BedKind::Underwater,
            BedKind::Biome,
            BedKind::Surf,
        ]
    }
}

/// A constructed bed of one kind.
///
/// The large variants are boxed so the enum stays pointer-sized; beds are
/// built on the rebuilder thread (not the audio thread) and only ever
/// accessed through `&mut`, so the indirection costs nothing realtime.
pub enum BedUnion {
    Rain(Box<foley_dsp::Rain>),
    Wind(Box<foley_dsp::Wind>),
    Water(Box<foley_dsp::Water>),
    Fire(Box<foley_dsp::Fire>),
    Lava(Box<foley_dsp::Lava>),
    Thunder(Box<foley_synth::Thunder>),
    Cave(Box<foley_dsp::Cave>),
    Underwater(Box<foley_dsp::Underwater>),
    Biome(Box<foley_synth::Biome>),
    Surf(Box<foley_synth::Surf>),
}

/// Constructs the bed for `kind` at the given seed and sample rate.
pub fn new_bed(kind: BedKind, seed: u64, sample_rate: u32) -> BedUnion {
    match kind {
        BedKind::Rain => BedUnion::Rain(Box::new(foley_dsp::Rain::new(seed, sample_rate))),
        BedKind::Wind => BedUnion::Wind(Box::new(foley_dsp::Wind::new(seed, sample_rate))),
        BedKind::Water => BedUnion::Water(Box::new(foley_dsp::Water::new(seed, sample_rate))),
        BedKind::Fire => BedUnion::Fire(Box::new(foley_dsp::Fire::new(seed, sample_rate))),
        BedKind::Lava => BedUnion::Lava(Box::new(foley_dsp::Lava::new(seed, sample_rate))),
        BedKind::Thunder => {
            BedUnion::Thunder(Box::new(foley_synth::Thunder::new(seed, sample_rate)))
        }
        BedKind::Cave => BedUnion::Cave(Box::new(foley_dsp::Cave::new(seed, sample_rate))),
        BedKind::Underwater => {
            BedUnion::Underwater(Box::new(foley_dsp::Underwater::new(seed, sample_rate)))
        }
        BedKind::Biome => BedUnion::Biome(Box::new(foley_synth::Biome::new(seed, sample_rate))),
        BedKind::Surf => BedUnion::Surf(Box::new(foley_synth::Surf::new(seed, sample_rate))),
    }
}

impl BedUnion {
    /// The kind this bed was constructed as.
    pub fn kind(&self) -> BedKind {
        match self {
            BedUnion::Rain(_) => BedKind::Rain,
            BedUnion::Wind(_) => BedKind::Wind,
            BedUnion::Water(_) => BedKind::Water,
            BedUnion::Fire(_) => BedKind::Fire,
            BedUnion::Lava(_) => BedKind::Lava,
            BedUnion::Thunder(_) => BedKind::Thunder,
            BedUnion::Cave(_) => BedKind::Cave,
            BedUnion::Underwater(_) => BedKind::Underwater,
            BedUnion::Biome(_) => BedKind::Biome,
            BedUnion::Surf(_) => BedKind::Surf,
        }
    }

    /// Rebuilds the bed's scene from four 0..=1 knobs.
    pub fn set_scene_knobs(&mut self, knobs: [f32; 4]) {
        let kind = self.kind();
        match (self, scene_from_knobs(kind, knobs)) {
            (BedUnion::Rain(bed), SceneArgs::Rain(s)) => bed.set_scene(s),
            (BedUnion::Wind(bed), SceneArgs::Wind(s)) => bed.set_scene(s),
            (BedUnion::Water(bed), SceneArgs::Water(s)) => bed.set_scene(s),
            (BedUnion::Fire(bed), SceneArgs::Fire(s)) => bed.set_scene(s),
            (BedUnion::Lava(bed), SceneArgs::Lava(s)) => bed.set_scene(s),
            (BedUnion::Thunder(bed), SceneArgs::Thunder(s)) => bed.set_scene(s),
            (BedUnion::Cave(bed), SceneArgs::Cave(s)) => bed.set_scene(s),
            (BedUnion::Underwater(bed), SceneArgs::Underwater(s)) => bed.set_scene(s),
            (BedUnion::Biome(bed), SceneArgs::Biome(s)) => bed.set_scene(s),
            (BedUnion::Surf(bed), SceneArgs::Surf(s)) => bed.set_scene(s),
            _ => unreachable!("scene_from_knobs matches the bed's own kind"),
        }
    }

    /// Renders one block of mono audio into `out`.
    pub fn render(&mut self, out: &mut [f32]) {
        match self {
            BedUnion::Rain(bed) => bed.render(out),
            BedUnion::Wind(bed) => bed.render(out),
            BedUnion::Water(bed) => bed.render(out),
            BedUnion::Fire(bed) => bed.render(out),
            BedUnion::Lava(bed) => bed.render(out),
            BedUnion::Thunder(bed) => bed.render(out),
            BedUnion::Cave(bed) => bed.render(out),
            BedUnion::Underwater(bed) => bed.render(out),
            BedUnion::Biome(bed) => bed.render(out),
            BedUnion::Surf(bed) => bed.render(out),
        }
    }

    /// Enqueues a lightning strike at `pos` (metres, y-up) seeding from `base`.
    /// No-op at every kind but Thunder.
    pub fn strike(&mut self, base: u64, pos: [f32; 3]) {
        if let BedUnion::Thunder(bed) = self {
            bed.strike(pos, base);
        }
    }
}
