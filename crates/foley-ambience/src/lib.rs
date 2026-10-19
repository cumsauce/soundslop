//! Soundslop Ambience: a six-layer procedural soundscape generator.
//!
//! One CLAP + VST3 plugin (via nice-plug) wrapping [`AmbienceRig`]. All 68
//! parameters are written to the rig once per block; the rig smooths, glides
//! and renders. Thunder strikes are triggered by a rising edge on a Thunder
//! layer's knob B (the momentary "strike" knob), never by MIDI — this plugin
//! is a generative instrument, not a sampler.

use std::num::NonZeroU32;
use std::sync::Arc;

use nice_plug::prelude::*;
use params::AmbienceParams;

pub mod editor;
pub mod params;

use foley_engine::{AmbienceRig, LAYERS, hit_seed};

pub struct Ambience {
    params: Arc<AmbienceParams>,
    rig: Option<AmbienceRig>,
    sample_rate: f32,
    /// Previous raw knob_b value per layer, for Thunder strike edge detection.
    prev_knob_b: [f32; LAYERS],
    /// Per-layer strike counter for hit_seed's `counter` field.
    strike_counter: [u32; LAYERS],
    /// Whether the one-time howl pre-warm has been spawned.
    prewarmed: bool,
}

impl Default for Ambience {
    fn default() -> Self {
        Ambience {
            params: Arc::new(AmbienceParams::default()),
            rig: None,
            sample_rate: 1.0,
            prev_knob_b: [0.0; LAYERS],
            strike_counter: [0; LAYERS],
            prewarmed: false,
        }
    }
}

impl Ambience {
    /// The frozen strike-base derivation (spec §10): `hit_seed(layer seed,
    /// az scaled to u8, dist/64.0, per-layer strike counter)`. Deterministic
    /// given the same knob state and counter.
    fn strike_base(layer_seed: u64, az_deg: f32, dist: f32, counter: u32) -> u64 {
        let az_u8 = ((az_deg + 180.0) / 360.0 * 255.0).round().clamp(0.0, 255.0) as u8;
        hit_seed(layer_seed, az_u8, dist / 64.0, counter)
    }
}

impl Plugin for Ambience {
    const NAME: &'static str = "Soundslop Ambience";
    const VENDOR: &'static str = "cumsauce";
    const URL: &'static str = "https://github.com/cumsauce/soundslop";
    const EMAIL: &'static str = "cumsauce@users.noreply.github.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const HARD_REALTIME_ONLY: bool = false;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();
    type Editor = nice_plug_egui::EguiEditor<editor::AmbienceEditor>;

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        editor::make_editor(self.params.clone())
    }

    fn activate(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        let rate = buffer_config.sample_rate as u32;

        // Pre-warm the shared LUTs (idempotent; safe off the audio thread).
        foley_synth::warm_voice_tables();

        self.rig = Some(AmbienceRig::new(rate));

        // Spawn the howl render on a detached thread exactly once per instance,
        // so plugin activation doesn't block for ~1.4 s.
        if !self.prewarmed {
            self.prewarmed = true;
            std::thread::spawn(move || {
                let _ = foley_synth::howl(0x5eed, rate);
            });
        }

        true
    }

    fn reset(&mut self) {
        // Recreate the rig: clears all bed/filter/guard state deterministically.
        if self.sample_rate >= 1.0 {
            self.rig = Some(AmbienceRig::new(self.sample_rate as u32));
        }
        self.prev_knob_b = [0.0; LAYERS];
        self.strike_counter = [0; LAYERS];
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Drain and ignore events: this is a generative instrument.
        while context.next_event().is_some() {}

        // 1. Snapshot all layer param values (owned) so we can borrow `self.rig`
        //    mutably below without conflicting with `&self` param reads.
        struct Snap {
            kind: Option<foley_engine::BedKind>,
            gain: f32,
            seed: u64,
            motion_amt: f32,
            motion_rate: f32,
            dist: f32,
            az: f32,
            knobs: [f32; 4],
        }
        let ps = &self.params;
        let lps = [
            &ps.layer1, &ps.layer2, &ps.layer3, &ps.layer4, &ps.layer5, &ps.layer6,
        ];
        let snaps: [Snap; LAYERS] = std::array::from_fn(|i| Snap {
            kind: lps[i].kind.value().to_bed_kind(),
            gain: lps[i].gain.value(),
            seed: lps[i].seed.value().max(0) as u64,
            motion_amt: lps[i].motion_amt.value(),
            motion_rate: lps[i].motion_rate.value(),
            dist: lps[i].dist.value(),
            az: lps[i].az.value(),
            knobs: [
                lps[i].knob_a.value(),
                lps[i].knob_b.value(),
                lps[i].knob_c.value(),
                lps[i].knob_d.value(),
            ],
        });
        let glide = self.params.scene_glide.value();
        let out_db = self.params.output_gain.value();

        let Some(rig) = self.rig.as_mut() else {
            buffer.as_slice().iter_mut().for_each(|ch| ch.fill(0.0));
            return ProcessStatus::KeepAlive;
        };

        // 2. Write layer states + detect Thunder strikes.
        for (i, s) in snaps.iter().enumerate() {
            // Thunder strike on rising edge of knob B (the momentary knob).
            let is_thunder = s.kind == Some(foley_engine::BedKind::Thunder);
            if is_thunder && Self::rising_edge(self.prev_knob_b[i], s.knobs[1]) {
                let base = Self::strike_base(s.seed, s.az, s.dist, self.strike_counter[i]);
                self.strike_counter[i] = self.strike_counter[i].wrapping_add(1);
                rig.strike(i, base);
            }
            self.prev_knob_b[i] = s.knobs[1];

            let st = &mut rig.layers()[i];
            st.kind = s.kind;
            st.gain = s.gain;
            st.seed = s.seed;
            st.motion_amt = s.motion_amt;
            st.motion_rate = s.motion_rate;
            st.dist = s.dist;
            st.az = s.az;
            st.knobs = s.knobs;
        }

        // 3. Master params.
        rig.set_glide(glide);
        rig.set_output_gain(util::db_to_gain(out_db));

        // 4. Render stereo. The rig chunks internally at MAX_BLOCK.
        let n = buffer.samples();
        let channels = buffer.as_slice();
        if channels.len() >= 2 {
            let (left, rest) = channels.split_at_mut(1);
            let (right, _) = rest.split_at_mut(1);
            let len = left[0].len().min(right[0].len()).min(n);
            rig.process(&mut left[0][..len], &mut right[0][..len]);
            for ch in channels.iter_mut().skip(2) {
                ch[..len].fill(0.0);
            }
        } else {
            // Mono or zero-channel: nothing to write into a stereo rig.
            channels.iter_mut().for_each(|ch| ch.fill(0.0));
        }

        ProcessStatus::KeepAlive
    }
}

impl Ambience {
    /// Rising-edge detect on a 0..=1 momentary knob: crossed 0.5 upward.
    fn rising_edge(prev: f32, cur: f32) -> bool {
        prev < 0.5 && cur >= 0.5
    }
}

impl ClapPlugin for Ambience {
    const CLAP_ID: &'static str = "io.github.cumsauce.soundslop.ambience";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Procedural six-layer soundscape generator");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] =
        &[ClapFeature::Instrument, ClapFeature::Synthesizer];
}

impl Vst3Plugin for Ambience {
    const VST3_CLASS_ID: [u8; 16] = *b"S0undslopAmb.001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nice_export_clap!(Ambience);
nice_export_vst3!(Ambience);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rising_edge_detects_only_upward_crossing() {
        assert!(!Ambience::rising_edge(0.0, 0.4));
        assert!(Ambience::rising_edge(0.4, 0.6));
        assert!(!Ambience::rising_edge(0.6, 0.6));
        assert!(!Ambience::rising_edge(0.6, 0.4));
        assert!(!Ambience::rising_edge(0.5, 0.5));
    }

    #[test]
    fn strike_base_is_deterministic_and_counter_sensitive() {
        let a = Ambience::strike_base(42, 0.0, 0.0, 0);
        let b = Ambience::strike_base(42, 0.0, 0.0, 0);
        assert_eq!(a, b);
        let c = Ambience::strike_base(42, 0.0, 0.0, 1);
        assert_ne!(a, c);
    }
}
