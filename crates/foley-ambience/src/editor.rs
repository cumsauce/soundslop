//! The Soundslop Ambience editor, drawn to the UI contract: fixed chrome
//! (title, Save…/Load… and the two master controls) over a scrolling rack of
//! six collapsible layer sections. Thunder layers swap knob B for the
//! momentary magenta Strike control. Widget geometry, tokens and fonts come
//! from `foley-ui`.

use std::sync::Arc;

use egui::{self, Vec2};
use nice_plug::context::gui::GuiContext;
use nice_plug_egui::{
    EguiEditorState, EguiNiceSettings, Frame, NiceEguiApp, RepaintNotifier, baseview,
    create_egui_editor, resizable_window::ResizableWindow,
};

use foley_engine::{BedKind, knob_labels};
use foley_ui::{fonts, preset, theme, widgets};

use crate::params::{AmbienceParams, EditorSize};

const WINDOW_TITLE: &str = "Soundslop Ambience";
const LAYERS: usize = 6;

/// The egui application state for Ambience. Tracks whether each layer's
/// Strike button was held on the previous frame so a 1.0 press and a 0.0
/// release are emitted exactly once (the flag is handed to `widgets::strike`,
/// which also releases on pointer cancel/focus loss).
pub struct AmbienceEditor {
    params: Arc<AmbienceParams>,
    gui_ctx: Option<GuiContext>,
    strike_down: [bool; LAYERS],
    /// Which layer sections are expanded (rack UI state, never persisted).
    expanded: [bool; LAYERS],
}

impl AmbienceEditor {
    fn new(params: Arc<AmbienceParams>) -> Self {
        AmbienceEditor {
            params,
            gui_ctx: None,
            strike_down: [false; LAYERS],
            expanded: [true; LAYERS],
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

impl NiceEguiApp for AmbienceEditor {
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

        ResizableWindow::new("soundslop-ambience")
            .min_size(Vec2::new(theme::WINDOW_MIN.0, theme::WINDOW_MIN.1))
            .show(ui, |ui| {
                // Fixed chrome: masters and host recall stay visible.
                let (save, load) = widgets::chrome(ui, WINDOW_TITLE);
                widgets::slider(ui, &p.output_gain, &setter, 1u8, "Output Gain");
                widgets::slider(ui, &p.scene_glide, &setter, 2u8, "Scene Glide");
                if save {
                    save_preset(ctx);
                }
                if load {
                    load_preset(ctx);
                }

                // The scrolling rack: six layer sections.
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let layers = [
                        &p.layer1 as &crate::params::LayerParams,
                        &p.layer2,
                        &p.layer3,
                        &p.layer4,
                        &p.layer5,
                        &p.layer6,
                    ];
                    for (i, lp) in layers.iter().enumerate() {
                        self.expanded[i] = widgets::section_header(
                            ui,
                            &format!("Layer {}", i + 1),
                            self.expanded[i],
                        );
                        if !self.expanded[i] {
                            continue;
                        }

                        widgets::selector(ui, &lp.kind, &setter, (i, 0u8), "Kind");
                        let bed: Option<BedKind> = lp.kind.value().to_bed_kind();
                        let labels = bed.map(knob_labels).unwrap_or([""; 4]);

                        pair(
                            ui,
                            |ui| widgets::slider(ui, &lp.gain, &setter, (i, 1u8), "Gain"),
                            |ui| widgets::stepper(ui, &lp.seed, &setter, (i, 2u8), "Seed"),
                        );
                        pair(
                            ui,
                            |ui| {
                                widgets::slider(
                                    ui,
                                    &lp.motion_amt,
                                    &setter,
                                    (i, 3u8),
                                    "Motion amount",
                                )
                            },
                            |ui| {
                                widgets::slider(
                                    ui,
                                    &lp.motion_rate,
                                    &setter,
                                    (i, 4u8),
                                    "Motion rate",
                                )
                            },
                        );
                        pair(
                            ui,
                            |ui| widgets::slider_bipolar(ui, &lp.az, &setter, (i, 5u8), "Azimuth"),
                            |ui| widgets::slider(ui, &lp.dist, &setter, (i, 6u8), "Distance"),
                        );
                        pair(
                            ui,
                            |ui| {
                                widgets::slider(
                                    ui,
                                    &lp.knob_a,
                                    &setter,
                                    (i, 7u8),
                                    &format!("A · {}", labels[0]),
                                )
                            },
                            |ui| {
                                if bed == Some(BedKind::Thunder) {
                                    widgets::strike(
                                        ui,
                                        &lp.knob_b,
                                        &setter,
                                        (i, 8u8),
                                        &format!("B · {}", labels[1]),
                                        &mut self.strike_down[i],
                                    );
                                } else {
                                    widgets::slider(
                                        ui,
                                        &lp.knob_b,
                                        &setter,
                                        (i, 8u8),
                                        &format!("B · {}", labels[1]),
                                    );
                                }
                            },
                        );
                        pair(
                            ui,
                            |ui| {
                                widgets::slider(
                                    ui,
                                    &lp.knob_c,
                                    &setter,
                                    (i, 9u8),
                                    &format!("C · {}", labels[2]),
                                )
                            },
                            |ui| {
                                widgets::slider(
                                    ui,
                                    &lp.knob_d,
                                    &setter,
                                    (i, 10u8),
                                    &format!("D · {}", labels[3]),
                                )
                            },
                        );
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
            nice_plug::editor::dpi::LogicalSize::new(theme::WINDOW_MIN.0, theme::WINDOW_MIN.1),
        ),
    };

    create_egui_editor(
        egui_state,
        RepaintNotifier::new(),
        EguiNiceSettings::new()
            .with_tile(WINDOW_TITLE)
            .with_resize_hint(resize_hint),
        AmbienceEditor::new(params),
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
