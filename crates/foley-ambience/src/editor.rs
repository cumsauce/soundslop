//! The Soundslop Ambience editor: a scrollable rack of six layer sections,
//! each with a `ParamSlider` per parameter. Thunder layers additionally get a
//! momentary Strike button wired to knob B (the plugin's rising-edge detector
//! turns that into a strike). Master Save…/Load… buttons persist the full
//! plugin state to / from a JSON file.

use std::sync::Arc;

use egui::{self, Vec2};
use nice_plug::context::gui::GuiContext;
use nice_plug::prelude::PluginState;
use nice_plug_egui::{
    EguiEditorState, EguiNiceSettings, Frame, NiceEguiApp, RepaintNotifier, baseview,
    create_egui_editor, resizable_window::ResizableWindow, widgets,
};

use foley_engine::{BedKind, knob_labels};

use crate::params::{AmbienceParams, EditorSize};

/// The egui application state for Ambience. Tracks whether each layer's Strike
/// button was held on the previous frame so we emit a 1.0 press and a 0.0
/// release exactly once (also released on pointer cancel/focus loss, since
/// `is_pointer_button_down_on()` goes false).
pub struct AmbienceEditor {
    params: Arc<AmbienceParams>,
    gui_ctx: Option<GuiContext>,
    strike_down: [bool; 6],
}

impl NiceEguiApp for AmbienceEditor {
    fn build(
        &mut self,
        _egui_ctx: egui::Context,
        nice_gui_ctx: GuiContext,
        _frame: &mut Frame,
    ) -> Result<(), baseview::HandlerError> {
        self.gui_ctx = Some(nice_gui_ctx);
        Ok(())
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        let Some(ctx) = &self.gui_ctx else {
            return;
        };
        let setter = ctx.param_setter();
        let p = &self.params;

        ResizableWindow::new("soundslop-ambience")
            .min_size(Vec2::new(380.0, 520.0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Soundslop Ambience");
                    ui.separator();

                    ui.label("Output Gain");
                    ui.add(widgets::ParamSlider::for_param(&p.output_gain, &setter));
                    ui.label("Scene Glide");
                    ui.add(widgets::ParamSlider::for_param(&p.scene_glide, &setter));

                    let layers = [
                        &p.layer1, &p.layer2, &p.layer3, &p.layer4, &p.layer5, &p.layer6,
                    ];
                    for (i, lp) in layers.iter().enumerate() {
                        ui.separator();
                        ui.heading(format!("Layer {}", i + 1));

                        ui.label("Kind");
                        ui.add(widgets::ParamSlider::for_param(&lp.kind, &setter));

                        let bed: Option<BedKind> = lp.kind.value().to_bed_kind();
                        let labels = bed.map(knob_labels).unwrap_or([""; 4]);

                        ui.label("Gain");
                        ui.add(widgets::ParamSlider::for_param(&lp.gain, &setter));
                        ui.label("Seed");
                        ui.add(widgets::ParamSlider::for_param(&lp.seed, &setter));
                        ui.label("Motion Amount");
                        ui.add(widgets::ParamSlider::for_param(&lp.motion_amt, &setter));
                        ui.label("Motion Rate");
                        ui.add(widgets::ParamSlider::for_param(&lp.motion_rate, &setter));
                        ui.label("Distance");
                        ui.add(widgets::ParamSlider::for_param(&lp.dist, &setter));
                        ui.label("Azimuth");
                        ui.add(widgets::ParamSlider::for_param(&lp.az, &setter));

                        ui.label(labels[0]);
                        ui.add(widgets::ParamSlider::for_param(&lp.knob_a, &setter));

                        ui.label(labels[1]);
                        if bed == Some(BedKind::Thunder) {
                            // Knob B is the momentary strike: drive it with a
                            // press (1.0) / release (0.0) button instead of a
                            // continuous slider.
                            let resp = ui.add(egui::Button::new("Strike"));
                            let down = resp.is_pointer_button_down_on();
                            if down && !self.strike_down[i] {
                                setter.begin_set_parameter(&lp.knob_b);
                                setter.set_parameter(&lp.knob_b, 1.0);
                                setter.end_set_parameter(&lp.knob_b);
                            } else if !down && self.strike_down[i] {
                                setter.begin_set_parameter(&lp.knob_b);
                                setter.set_parameter(&lp.knob_b, 0.0);
                                setter.end_set_parameter(&lp.knob_b);
                            }
                            self.strike_down[i] = down;
                        } else {
                            ui.add(widgets::ParamSlider::for_param(&lp.knob_b, &setter));
                        }

                        ui.label(labels[2]);
                        ui.add(widgets::ParamSlider::for_param(&lp.knob_c, &setter));
                        ui.label(labels[3]);
                        ui.add(widgets::ParamSlider::for_param(&lp.knob_d, &setter));
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Save…").clicked() {
                            save_preset(ctx);
                        }
                        if ui.button("Load…").clicked() {
                            load_preset(ctx);
                        }
                    });
                });
            });
    }

    fn resized(&mut self, new_size: baseview::WindowSize) {
        if let Ok(mut sz) = self.params.editor_state.write() {
            sz.width = new_size.logical.width as f32;
            sz.height = new_size.logical.height as f32;
        }
    }

    fn editor_closed(&mut self) {
        self.gui_ctx = None;
    }
}

/// Build the egui editor for Ambience.
pub(crate) fn make_editor(
    params: Arc<AmbienceParams>,
) -> Option<nice_plug_egui::EguiEditor<AmbienceEditor>> {
    let initial_size = params
        .editor_state
        .read()
        .map(|sz| *sz)
        .unwrap_or_else(|_| EditorSize::default());
    let egui_state = EguiEditorState::from_size(
        nice_plug::editor::dpi::LogicalSize::new(
            initial_size.width as f64,
            initial_size.height as f64,
        ),
        1.0,
    );

    let resize_hint = nice_plug::editor::ResizeHint {
        can_resize: true,
        can_resize_horizontally: true,
        can_resize_vertically: true,
        preserve_aspect_ratio: false,
        aspect_ratio_width: 1,
        aspect_ratio_height: 1,
        size_constraints: nice_plug::editor::SizeConstraints::min_logical_size(
            nice_plug::editor::dpi::LogicalSize::new(380.0, 520.0),
        ),
    };

    create_egui_editor(
        egui_state,
        RepaintNotifier::new(),
        EguiNiceSettings::new()
            .with_tile("Soundslop Ambience")
            .with_resize_hint(resize_hint),
        AmbienceEditor {
            params,
            gui_ctx: None,
            strike_down: [false; 6],
        },
    )
}

/// Serialize the full plugin state to a user-chosen JSON file.
fn save_preset(ctx: &GuiContext) {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name("soundslop-ambience-preset.json")
        .add_filter("JSON", &["json"])
        .save_file()
    else {
        return;
    };
    let state = ctx.get_state();
    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(path, json);
    }
}

/// Load a plugin state from a user-chosen JSON file.
fn load_preset(ctx: &GuiContext) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .pick_file()
    else {
        return;
    };
    let Ok(json) = std::fs::read_to_string(&path) else {
        return;
    };
    if let Ok(state) = serde_json::from_str::<PluginState>(&json) {
        ctx.set_state(state);
    }
}
