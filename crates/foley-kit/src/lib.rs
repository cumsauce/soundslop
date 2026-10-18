//! Soundslop Kit: a MIDI-triggered procedural foley one-shot instrument.
//!
//! One CLAP + VST3 plugin (via NIH-plug) wrapping [`KitRig`]. MIDI NoteOn
//! triggers any slot whose `note` matches; each hit renders a fresh one-shot
//! on the clip worker. NoteOff is ignored (foley one-shots run to completion,
//! spec §11).

use std::num::NonZeroU32;
use std::sync::Arc;

use nih_plug::prelude::*;
use params::KitParams;

pub mod params;

use foley_engine::{KitRig, SlotState};

pub struct Kit {
    params: Arc<KitParams>,
    rig: Option<KitRig>,
    sample_rate: f32,
}

impl Default for Kit {
    fn default() -> Self {
        Kit {
            params: Arc::new(KitParams::default()),
            rig: None,
            sample_rate: 1.0,
        }
    }
}

impl Plugin for Kit {
    const NAME: &'static str = "Soundslop Kit";
    const VENDOR: &'static str = "cumsauce";
    const URL: &'static str = "https://github.com/cumsauce/soundslop";
    const EMAIL: &'static str = "";
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

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        let rate = buffer_config.sample_rate as u32;

        // Pre-warm the shared LUTs and grow the clip worker's pool with one
        // render (spec §9 pre-warm). KitRig::new already calls
        // warm_voice_tables, but this is idempotent and cheap off-thread.
        foley_synth::warm_voice_tables();

        self.rig = Some(KitRig::new(rate));
        true
    }

    fn reset(&mut self) {
        if self.sample_rate >= 1.0 {
            self.rig = Some(KitRig::new(self.sample_rate as u32));
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let Some(rig) = self.rig.as_mut() else {
            buffer.as_slice().iter_mut().for_each(|ch| ch.fill(0.0));
            return ProcessStatus::KeepAlive;
        };

        // 1. Copy slot params into the rig (owned reads, disjoint from rig).
        {
            let ps = &self.params;
            let sps = [
                &ps.slot1, &ps.slot2, &ps.slot3, &ps.slot4, &ps.slot5, &ps.slot6, &ps.slot7,
                &ps.slot8,
            ];
            let slots = rig.slots();
            for (i, sp) in sps.iter().enumerate() {
                slots[i] = SlotState {
                    category: sp.category.value().to_category(),
                    kind: sp.kind.value().max(0) as u32,
                    voice_mob: sp.voice_mob.value().max(0) as u32,
                    note: sp.note.value().clamp(0, 127) as u8,
                    knobs: [sp.knob_a.value(), sp.knob_b.value()],
                    gain: sp.gain.value(),
                    pan: sp.pan.value() / 100.0,
                    seed: sp.seed.value().max(0) as u64,
                    fresh: sp.fresh.value(),
                };
            }
        }

        // 2. Master params.
        rig.set_polyphony(self.params.polyphony.value().count());
        rig.set_humanize(self.params.humanize.value());
        rig.set_output_gain(util::db_to_gain(self.params.output_gain.value()));

        // 3. Drain MIDI: NoteOn triggers matching slots; NoteOff ignored.
        while let Some(event) = context.next_event() {
            if let NoteEvent::NoteOn { note, velocity, .. } = event {
                rig.note_on(note, velocity);
            }
        }

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
            channels.iter_mut().for_each(|ch| ch.fill(0.0));
        }

        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for Kit {
    const CLAP_ID: &'static str = "io.github.cumsauce.soundslop.kit";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Procedural foley one-shot instrument");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] =
        &[ClapFeature::Instrument, ClapFeature::Synthesizer];
}

impl Vst3Plugin for Kit {
    const VST3_CLASS_ID: [u8; 16] = *b"S0undslopKit.001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(Kit);
nih_export_vst3!(Kit);
