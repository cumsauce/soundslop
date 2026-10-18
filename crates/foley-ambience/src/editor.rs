//! The Soundslop Ambience editor: a scrollable rack of six layer sections,
//! each with a `ParamSlider` per parameter. Thunder layers additionally get a
//! momentary Strike button wired to knob B (the plugin's rising-edge detector
//! turns that into a strike). Master Save…/Load… buttons persist the full
//! plugin state to / from a JSON file.

use std::sync::Arc;

use nih_plug::prelude::ParamSetter;
use nih_plug_egui::{
    EguiState, create_egui_editor,
    egui::{self, Vec2},
    resizable_window::ResizableWindow,
    widgets,
};

use foley_engine::{BedKind, knob_labels};

use crate::params::AmbienceParams;

/// Per-editor UI state: tracks whether each layer's Strike button was held on
/// the previous frame so we emit a 1.0 press and a 0.0 release exactly once.
#[derive(Default)]
pub struct AmbienceUi {
    strike_down: [bool; 6],
}

/// Build the egui editor for Ambience.
pub(crate) fn make_editor(
    params: Arc<AmbienceParams>,
    egui_state: Arc<EguiState>,
) -> Option<Box<dyn nih_plug::prelude::Editor>> {
    let caption_params = params.clone();
    let egui_state_for_window = egui_state.clone();
    create_egui_editor(
        egui_state,
        AmbienceUi::default(),
        |_, _| {},
        move |egui_ctx, setter, ui_state| {
            ResizableWindow::new("soundslop-ambience")
                .min_size(Vec2::new(380.0, 520.0))
                .show(egui_ctx, egui_state_for_window.as_ref(), |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Soundslop Ambience");
                        ui.separator();

                        ui.label("Output Gain");
                        ui.add(widgets::ParamSlider::for_param(
                            &caption_params.output_gain,
                            setter,
                        ));
                        ui.label("Scene Glide");
                        ui.add(widgets::ParamSlider::for_param(
                            &caption_params.scene_glide,
                            setter,
                        ));

                        let layers = [
                            &caption_params.layer1,
                            &caption_params.layer2,
                            &caption_params.layer3,
                            &caption_params.layer4,
                            &caption_params.layer5,
                            &caption_params.layer6,
                        ];
                        for (i, lp) in layers.iter().enumerate() {
                            ui.separator();
                            ui.heading(format!("Layer {}", i + 1));

                            ui.label("Kind");
                            ui.add(widgets::ParamSlider::for_param(&lp.kind, setter));

                            let bed: Option<BedKind> = lp.kind.value().to_bed_kind();
                            let labels = bed.map(knob_labels).unwrap_or([""; 4]);

                            ui.label("Gain");
                            ui.add(widgets::ParamSlider::for_param(&lp.gain, setter));
                            ui.label("Seed");
                            ui.add(widgets::ParamSlider::for_param(&lp.seed, setter));
                            ui.label("Motion Amount");
                            ui.add(widgets::ParamSlider::for_param(&lp.motion_amt, setter));
                            ui.label("Motion Rate");
                            ui.add(widgets::ParamSlider::for_param(&lp.motion_rate, setter));
                            ui.label("Distance");
                            ui.add(widgets::ParamSlider::for_param(&lp.dist, setter));
                            ui.label("Azimuth");
                            ui.add(widgets::ParamSlider::for_param(&lp.az, setter));

                            ui.label(labels[0]);
                            ui.add(widgets::ParamSlider::for_param(&lp.knob_a, setter));

                            ui.label(labels[1]);
                            if bed == Some(BedKind::Thunder) {
                                // Knob B is the momentary strike: drive it with a
                                // press (1.0) / release (0.0) button instead of a
                                // continuous slider.
                                let resp = ui.add(egui::Button::new("Strike"));
                                let down = resp.is_pointer_button_down_on();
                                if down && !ui_state.strike_down[i] {
                                    setter.begin_set_parameter(&lp.knob_b);
                                    setter.set_parameter(&lp.knob_b, 1.0);
                                    setter.end_set_parameter(&lp.knob_b);
                                } else if !down && ui_state.strike_down[i] {
                                    setter.begin_set_parameter(&lp.knob_b);
                                    setter.set_parameter(&lp.knob_b, 0.0);
                                    setter.end_set_parameter(&lp.knob_b);
                                }
                                ui_state.strike_down[i] = down;
                            } else {
                                ui.add(widgets::ParamSlider::for_param(&lp.knob_b, setter));
                            }

                            ui.label(labels[2]);
                            ui.add(widgets::ParamSlider::for_param(&lp.knob_c, setter));
                            ui.label(labels[3]);
                            ui.add(widgets::ParamSlider::for_param(&lp.knob_d, setter));
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
        .set_file_name("soundslop-ambience-preset.json")
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
