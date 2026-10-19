//! The Soundslop Kit editor: a scrollable rack of eight slot sections, each
//! with a `ParamSlider` per parameter and captions from `kit_knob_labels`.
//! Master Save…/Load… buttons persist the full plugin state to / from JSON.

use std::sync::Arc;

use egui::{self, Vec2};
use nice_plug::context::gui::GuiContext;
use nice_plug::prelude::PluginState;
use nice_plug_egui::{
    EguiEditorState, EguiNiceSettings, Frame, NiceEguiApp, RepaintNotifier, baseview,
    create_egui_editor, resizable_window::ResizableWindow, widgets,
};

use foley_engine::kit_knob_labels;

use crate::params::{EditorSize, KitParams};

/// The egui application state for Kit.
pub struct KitEditor {
    params: Arc<KitParams>,
    gui_ctx: Option<GuiContext>,
}

impl NiceEguiApp for KitEditor {
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

        ResizableWindow::new("soundslop-kit")
            .min_size(Vec2::new(380.0, 520.0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Soundslop Kit");
                    ui.separator();

                    ui.label("Output Gain");
                    ui.add(widgets::ParamSlider::for_param(&p.output_gain, &setter));
                    ui.label("Polyphony");
                    ui.add(widgets::ParamSlider::for_param(&p.polyphony, &setter));
                    ui.label("Humanize");
                    ui.add(widgets::ParamSlider::for_param(&p.humanize, &setter));

                    let slots = [
                        &p.slot1, &p.slot2, &p.slot3, &p.slot4, &p.slot5, &p.slot6, &p.slot7,
                        &p.slot8,
                    ];
                    for (i, sp) in slots.iter().enumerate() {
                        ui.separator();
                        ui.heading(format!("Slot {}", i + 1));

                        ui.label("Category");
                        ui.add(widgets::ParamSlider::for_param(&sp.category, &setter));

                        let labels = kit_knob_labels(sp.category.value().to_category());

                        ui.label("Kind");
                        ui.add(widgets::ParamSlider::for_param(&sp.kind, &setter));
                        ui.label("Voice");
                        ui.add(widgets::ParamSlider::for_param(&sp.voice_mob, &setter));
                        ui.label("Note");
                        ui.add(widgets::ParamSlider::for_param(&sp.note, &setter));

                        ui.label(labels[0]);
                        ui.add(widgets::ParamSlider::for_param(&sp.knob_a, &setter));
                        ui.label(labels[1]);
                        ui.add(widgets::ParamSlider::for_param(&sp.knob_b, &setter));

                        ui.label("Gain");
                        ui.add(widgets::ParamSlider::for_param(&sp.gain, &setter));
                        ui.label("Pan");
                        ui.add(widgets::ParamSlider::for_param(&sp.pan, &setter));
                        ui.label("Seed");
                        ui.add(widgets::ParamSlider::for_param(&sp.seed, &setter));
                        ui.label("Fresh");
                        ui.add(widgets::ParamSlider::for_param(&sp.fresh, &setter));
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

/// Build the egui editor for Kit.
pub(crate) fn make_editor(params: Arc<KitParams>) -> Option<nice_plug_egui::EguiEditor<KitEditor>> {
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
            .with_tile("Soundslop Kit")
            .with_resize_hint(resize_hint),
        KitEditor {
            params,
            gui_ctx: None,
        },
    )
}

/// Serialize the full plugin state to a user-chosen JSON file.
fn save_preset(ctx: &GuiContext) {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name("soundslop-kit-preset.json")
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
