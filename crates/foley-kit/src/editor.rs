//! The Soundslop Kit editor: a scrollable rack of eight slot sections, each
//! with a `ParamSlider` per parameter and captions from `kit_knob_labels`.
//! Master Save…/Load… buttons persist the full plugin state to / from JSON.

use std::sync::Arc;

use nih_plug::prelude::ParamSetter;
use nih_plug_egui::{
    EguiState, create_egui_editor,
    egui::{self, Vec2},
    resizable_window::ResizableWindow,
    widgets,
};

use foley_engine::kit_knob_labels;

use crate::params::KitParams;

/// Build the egui editor for Kit.
pub(crate) fn make_editor(
    params: Arc<KitParams>,
    egui_state: Arc<EguiState>,
) -> Option<Box<dyn nih_plug::prelude::Editor>> {
    let p = params.clone();
    let egui_state_for_window = egui_state.clone();
    create_egui_editor(
        egui_state,
        (),
        |_, _| {},
        move |egui_ctx, setter, _state| {
            ResizableWindow::new("soundslop-kit")
                .min_size(Vec2::new(380.0, 520.0))
                .show(egui_ctx, egui_state_for_window.as_ref(), |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Soundslop Kit");
                        ui.separator();

                        ui.label("Output Gain");
                        ui.add(widgets::ParamSlider::for_param(&p.output_gain, setter));
                        ui.label("Polyphony");
                        ui.add(widgets::ParamSlider::for_param(&p.polyphony, setter));
                        ui.label("Humanize");
                        ui.add(widgets::ParamSlider::for_param(&p.humanize, setter));

                        let slots = [
                            &p.slot1, &p.slot2, &p.slot3, &p.slot4, &p.slot5, &p.slot6, &p.slot7,
                            &p.slot8,
                        ];
                        for (i, sp) in slots.iter().enumerate() {
                            ui.separator();
                            ui.heading(format!("Slot {}", i + 1));

                            ui.label("Category");
                            ui.add(widgets::ParamSlider::for_param(&sp.category, setter));

                            let labels = kit_knob_labels(sp.category.value().to_category());

                            ui.label("Kind");
                            ui.add(widgets::ParamSlider::for_param(&sp.kind, setter));
                            ui.label("Voice");
                            ui.add(widgets::ParamSlider::for_param(&sp.voice_mob, setter));
                            ui.label("Note");
                            ui.add(widgets::ParamSlider::for_param(&sp.note, setter));

                            ui.label(labels[0]);
                            ui.add(widgets::ParamSlider::for_param(&sp.knob_a, setter));
                            ui.label(labels[1]);
                            ui.add(widgets::ParamSlider::for_param(&sp.knob_b, setter));

                            ui.label("Gain");
                            ui.add(widgets::ParamSlider::for_param(&sp.gain, setter));
                            ui.label("Pan");
                            ui.add(widgets::ParamSlider::for_param(&sp.pan, setter));
                            ui.label("Seed");
                            ui.add(widgets::ParamSlider::for_param(&sp.seed, setter));
                            ui.label("Fresh");
                            ui.add(widgets::ParamSlider::for_param(&sp.fresh, setter));
                        }

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Save…").clicked() {
                                save_preset(setter);
                            }
                            if ui.button("Load…").clicked() {
                                load_preset(setter);
                            }
                        });
                    });
                });
        },
    )
}

/// Serialize the full plugin state to a user-chosen JSON file.
fn save_preset(setter: &ParamSetter) {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name("soundslop-kit-preset.json")
        .add_filter("JSON", &["json"])
        .save_file()
    else {
        return;
    };
    let state = setter.raw_context.get_state();
    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(path, json);
    }
}

/// Load a plugin state from a user-chosen JSON file.
fn load_preset(setter: &ParamSetter) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .pick_file()
    else {
        return;
    };
    let Ok(json) = std::fs::read_to_string(&path) else {
        return;
    };
    if let Ok(state) = serde_json::from_str::<nih_plug::prelude::PluginState>(&json) {
        setter.raw_context.set_state(state);
    }
}
