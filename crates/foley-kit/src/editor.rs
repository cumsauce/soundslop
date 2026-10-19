//! The Soundslop Kit editor, drawn to the UI contract: fixed chrome (title,
//! Save…/Load… and the three master controls) over a scrolling rack of eight
//! collapsible slot sections. Widget geometry, tokens and fonts come from
//! `foley-ui`.

use std::sync::Arc;

use egui::{self, Vec2};
use nice_plug::context::gui::GuiContext;
use nice_plug_egui::{
    EguiEditorState, EguiNiceSettings, Frame, NiceEguiApp, RepaintNotifier, baseview,
    create_egui_editor, resizable_window::ResizableWindow,
};

use foley_engine::kit_knob_labels;
use foley_ui::{fonts, preset, theme, widgets};

use crate::params::{EditorSize, KitParams};

const WINDOW_TITLE: &str = "Soundslop Kit";
const SLOTS: usize = 8;

/// The egui application state for Kit.
pub struct KitEditor {
    params: Arc<KitParams>,
    gui_ctx: Option<GuiContext>,
    /// Which slot sections are expanded (rack UI state, never persisted).
    expanded: [bool; SLOTS],
}

impl KitEditor {
    fn new(params: Arc<KitParams>) -> Self {
        KitEditor {
            params,
            gui_ctx: None,
            expanded: [true; SLOTS],
        }
    }
}

/// Two equal-width parameter controls on one row.
fn pair<C0, C1>(ui: &mut egui::Ui, left: C0, right: C1)
where
    C0: FnOnce(&mut egui::Ui),
    C1: FnOnce(&mut egui::Ui),
{
    ui.columns(2, |columns| {
        left(&mut columns[0]);
        right(&mut columns[1]);
    });
}

impl NiceEguiApp for KitEditor {
    fn build(
        &mut self,
        egui_ctx: egui::Context,
        nice_gui_ctx: GuiContext,
        _frame: &mut Frame,
    ) -> Result<(), baseview::HandlerError> {
        fonts::install(&egui_ctx);
        egui_ctx.set_theme(egui::Theme::Dark);
        egui_ctx.style_mut_of(egui::Theme::Dark, theme::apply);
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
            .min_size(Vec2::new(theme::WINDOW_MIN.0, theme::WINDOW_MIN.1))
            .show(ui, |ui| {
                // Fixed chrome: masters and host recall stay visible.
                let (save, load) = widgets::chrome(ui, WINDOW_TITLE);
                widgets::slider(ui, &p.output_gain, &setter, 1u8, "Output Gain");
                pair(
                    ui,
                    |ui| widgets::selector(ui, &p.polyphony, &setter, 2u8, "Polyphony"),
                    |ui| widgets::slider(ui, &p.humanize, &setter, 3u8, "Humanize"),
                );
                if save {
                    save_preset(ctx);
                }
                if load {
                    load_preset(ctx);
                }

                // The scrolling rack: eight slot sections.
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let slots = [
                        &p.slot1 as &crate::params::SlotParams,
                        &p.slot2,
                        &p.slot3,
                        &p.slot4,
                        &p.slot5,
                        &p.slot6,
                        &p.slot7,
                        &p.slot8,
                    ];
                    for (i, sp) in slots.iter().enumerate() {
                        self.expanded[i] = widgets::section_header(
                            ui,
                            &format!("Slot {}", i + 1),
                            self.expanded[i],
                        );
                        if !self.expanded[i] {
                            continue;
                        }

                        let labels = kit_knob_labels(sp.category.value().to_category());
                        widgets::selector(ui, &sp.category, &setter, (i, 0u8), "Category");
                        pair(
                            ui,
                            |ui| widgets::stepper(ui, &sp.kind, &setter, (i, 1u8), "Kind"),
                            |ui| widgets::stepper(ui, &sp.voice_mob, &setter, (i, 2u8), "Voice"),
                        );
                        pair(
                            ui,
                            |ui| widgets::note_stepper(ui, &sp.note, &setter, (i, 3u8), "Note"),
                            |ui| widgets::slider(ui, &sp.gain, &setter, (i, 4u8), "Gain"),
                        );
                        pair(
                            ui,
                            |ui| {
                                widgets::slider(
                                    ui,
                                    &sp.knob_a,
                                    &setter,
                                    (i, 5u8),
                                    &format!("A · {}", labels[0]),
                                )
                            },
                            |ui| {
                                widgets::slider(
                                    ui,
                                    &sp.knob_b,
                                    &setter,
                                    (i, 6u8),
                                    &format!("B · {}", labels[1]),
                                )
                            },
                        );
                        pair(
                            ui,
                            |ui| widgets::slider_bipolar(ui, &sp.pan, &setter, (i, 7u8), "Pan"),
                            |ui| widgets::stepper(ui, &sp.seed, &setter, (i, 8u8), "Seed"),
                        );
                        widgets::toggle(ui, &sp.fresh, &setter, (i, 9u8), "Fresh");
                    }
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
            nice_plug::editor::dpi::LogicalSize::new(theme::WINDOW_MIN.0, theme::WINDOW_MIN.1),
        ),
    };

    create_egui_editor(
        egui_state,
        RepaintNotifier::new(),
        EguiNiceSettings::new()
            .with_tile(WINDOW_TITLE)
            .with_resize_hint(resize_hint),
        KitEditor::new(params),
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
    if let Some(json) = preset::to_json(state) {
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
    if let Some(state) = preset::from_json(&json) {
        ctx.set_state(state);
    }
}
