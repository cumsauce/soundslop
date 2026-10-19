//! The Soundslop widget kit: parameter controls drawn to the UI contract.
//!
//! Every control is a 36-tall box on the raised "control" surface: a label
//! row (key + editable value chip) over a track row (segmented slider +
//! `n 0.000` readback) for sliders, or the row-2 editor shape the inventory
//! requests (steppers with note names, selectors, toggles, the momentary
//! Strike).

use egui::{
    Align2, Color32, ComboBox, CornerRadius, DragValue, FontFamily, FontId, Id, Pos2, Rect, Sense,
    Ui, UiBuilder, Vec2,
};

use nice_plug::prelude::*;

use crate::gesture;
use crate::metrics::{SEGMENTS, note_name, readback, segment_fills};
use crate::theme;

const CONTROL_H: f32 = 36.0;
const INSET: f32 = 6.0;
const ROW_H: f32 = 15.0;
const READBACK_W: f32 = 38.0;
const TRACK_H: f32 = 7.0;
const EXTENT_H: f32 = 3.0;
/// Contract: the track sits at y=22.5 (2.5 into the y=20 readback row).
const TRACK_TOP: f32 = 22.5;
/// Contract: the second editor row starts at y=20.
const ROW2_TOP: f32 = 20.0;

fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Values, chips and buttons use the mono voice at readback size and read
/// as recessed wells with a hairline border, per the contract.
fn small_text(ui: &mut Ui) {
    for style in [egui::TextStyle::Button, egui::TextStyle::Body] {
        ui.style_mut().text_styles.insert(style, mono(9.0));
    }
    let widgets = &mut ui.style_mut().visuals.widgets;
    widgets.inactive.weak_bg_fill = theme::SURFACE_RECESSED;
    widgets.inactive.bg_fill = theme::SURFACE_RECESSED;
    widgets.inactive.bg_stroke = egui::Stroke::new(1.0, theme::BORDER_DEFAULT);
    widgets.hovered.bg_stroke = egui::Stroke::new(1.0, theme::ACCENT_CYAN);
    widgets.active.bg_stroke = egui::Stroke::new(1.0, theme::ACCENT_CYAN);
}

fn id(salt: impl std::hash::Hash + std::fmt::Debug, name: &str) -> Id {
    Id::unique(salt).with(name)
}

/// Gesture state for one control (see `crate::gesture`): whether a begin
/// was already sent and the gesture is still open this frame.
fn gesture_open(ui: &Ui, key: Id) -> bool {
    ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(false)
}

/// Apply one gesture step around a normalized write.
fn apply_step<P: Param>(setter: &ParamSetter, param: &P, step: gesture::Step, n: f32) {
    match step {
        gesture::Step::None => {}
        gesture::Step::Begin => setter.begin_set_parameter(param),
        gesture::Step::BeginSet => {
            setter.begin_set_parameter(param);
            setter.set_parameter_normalized(param, n);
        }
        gesture::Step::Set => setter.set_parameter_normalized(param, n),
        gesture::Step::End => setter.end_set_parameter(param),
        gesture::Step::BeginSetEnd => {
            setter.begin_set_parameter(param);
            setter.set_parameter_normalized(param, n);
            setter.end_set_parameter(param);
        }
    }
}

/// A single discrete change (button click, menu pick, keyboard step): one
/// complete begin/set/end gesture. Public so editors can force-release a
/// parameter (e.g. Strike on focus loss).
pub fn set_once<P: Param>(setter: &ParamSetter, param: &P, value: P::Plain) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

/// Drive a control's gesture from this frame's press/change facts and run
/// the resulting step against `param`.
fn drive_gesture<P: Param>(
    ui: &mut Ui,
    key: Id,
    setter: &ParamSetter,
    param: &P,
    pressed: bool,
    changed: bool,
    n: f32,
) {
    let open = gesture_open(ui, key);
    let (step, open_after) = gesture::next(open, pressed, changed);
    apply_step(setter, param, step, n);
    ui.data_mut(|d| d.insert_temp(key, open_after));
}

/// Allocate the 36-tall control box, paint its surface and its label, then
/// host `body` in an id-scoped child UI over the inner rect (box shrunk by
/// INSET).
fn control_box(ui: &mut Ui, uid: Id, label: &str, body: impl FnOnce(&mut Ui, Rect)) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), CONTROL_H), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(2), theme::SURFACE_CONTROL);
    ui.painter().text(
        Pos2::new(rect.left() + INSET, rect.top() + 4.0),
        Align2::LEFT_TOP,
        label,
        mono(12.0),
        theme::TEXT_PRIMARY,
    );
    let inner = Rect::from_min_size(
        Pos2::new(rect.left() + INSET, rect.top()),
        Vec2::new(rect.width() - 2.0 * INSET, rect.height()),
    );
    ui.push_id(uid, |ui| {
        ui.scope_builder(UiBuilder::new().max_rect(inner), |child| body(child, inner));
    });
}

/// Editable normalized value field: drag to scrub, type to enter. `display`
/// renders the current normalization in human units. Edits round-trip
/// through `string_to_normalized_value` so units survive.
fn value_field<P: Param>(
    ui: &mut Ui,
    param: &P,
    setter: &ParamSetter,
    size: Vec2,
    display: impl Fn(f32) -> String,
) {
    let n = param.unmodulated_normalized_value();
    let mut v = n as f64;
    let response = ui.add_sized(
        size,
        DragValue::new(&mut v)
            .speed(0.004)
            .range(0.0..=1.0)
            .custom_formatter(move |_, _| display(n))
            .custom_parser(move |s: &str| param.string_to_normalized_value(s).map(f64::from)),
    );
    // Typed entry, keyboard steps and drag scrubs all land here; the gesture
    // state machine decides how the write is wrapped for the host.
    drive_gesture(
        ui,
        response.id,
        setter,
        param,
        response.is_pointer_button_down_on(),
        response.changed(),
        v as f32,
    );
}

/// Editable value chip, right-aligned on the label row.
fn chip<P: Param>(ui: &mut Ui, inner: Rect, width: f32, param: &P, setter: &ParamSetter) {
    ui.scope_builder(
        UiBuilder::new().max_rect(Rect::from_min_size(
            Pos2::new(inner.right() - width, inner.top() + 3.0),
            Vec2::new(width, ROW_H),
        )),
        |ui| {
            small_text(ui);
            value_field(ui, param, setter, Vec2::new(width, ROW_H), |n| {
                param.normalized_value_to_string(n, true)
            });
        },
    );
}

/// `n 0.000` readback at the right edge of the track row.
fn readback_at(painter: &egui::Painter, inner: Rect, text: &str, color: Color32) {
    painter.text(
        readback_pos(inner),
        Align2::RIGHT_TOP,
        text,
        mono(9.0),
        color,
    );
}

/// One segmented track: 20 equal extents, the handle drawn as a full-height
/// cell at its index, cyan fills unipolar (from 0) or bipolar (from the
/// centre towards the handle).
fn draw_track(ui: &mut Ui, track: Rect, fills: (usize, usize, usize), bipolar: bool) {
    let (before, h, _after) = fills;
    let painter = ui.painter();
    let seg_w = track.width() / SEGMENTS as f32;
    let mid_y = track.top() + (TRACK_H - EXTENT_H) / 2.0;

    for i in 0..SEGMENTS {
        let is_handle = i == h;
        let lit = if bipolar {
            crate::metrics::bipolar_lit(h, i)
        } else {
            i < before
        };
        let color = if is_handle {
            theme::TEXT_PRIMARY
        } else if lit {
            theme::ACCENT_CYAN
        } else {
            theme::SURFACE_RECESSED
        };
        let (y, height) = if is_handle {
            (track.top(), TRACK_H)
        } else {
            (mid_y, EXTENT_H)
        };
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(track.left() + i as f32 * seg_w, y),
                Vec2::new(seg_w, height),
            ),
            CornerRadius::ZERO,
            color,
        );
    }
}

/// Position of the `n 0.000` readback text (contract: the 12-tall box at
/// the right edge of the y=20 readback row).
fn readback_pos(inner: Rect) -> Pos2 {
    Pos2::new(inner.right(), inner.top() + ROW2_TOP)
}

/// Track-row geometry shared by all controls: the usable horizontal band
/// after reserving the readback gutter.
fn track_row_rect(inner: Rect) -> Rect {
    Rect::from_min_size(
        Pos2::new(inner.left(), inner.top() + TRACK_TOP),
        Vec2::new(inner.width() - READBACK_W - 8.0, TRACK_H),
    )
}

/// Shared slider body: chip, draggable segmented track, readback.
fn slider_body(
    ui: &mut Ui,
    inner: Rect,
    param: &FloatParam,
    setter: &ParamSetter,
    wid: Id,
    bipolar: bool,
) {
    chip(ui, inner, 46.0, param, setter);
    let n = param.unmodulated_normalized_value();
    let track = track_row_rect(inner);
    draw_track(ui, track, segment_fills(n), bipolar);
    readback_at(ui.painter(), inner, &readback(n), theme::ACCENT_CYAN);

    let response = ui.interact(track, wid, Sense::drag());
    let target = response
        .interact_pointer_pos()
        .map(|pos| ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0));
    // A plain click (press frame) and a threshold-crossing drag are both
    // gestures; the state machine wraps each write accordingly.
    let changed = target.is_some_and(|m| (m - n).abs() > f32::EPSILON);
    drive_gesture(
        ui,
        wid,
        setter,
        param,
        response.is_pointer_button_down_on(),
        changed,
        target.unwrap_or(n),
    );
}

/// Float slider: label + editable chip, segmented track, `n 0.000`.
pub fn slider(
    ui: &mut Ui,
    param: &FloatParam,
    setter: &ParamSetter,
    salt: impl std::hash::Hash + Copy + std::fmt::Debug,
    label: &str,
) {
    // Ids key off the control slot, never the painted caption: dynamic knob
    // labels change without re-identifying the widget mid-interaction.
    let wid = id(salt, "slider");
    control_box(ui, wid, label, |ui, inner| {
        slider_body(ui, inner, param, setter, wid, false)
    });
}

/// Bipolar slider (fills from the centre) for Pan / Azimuth.
pub fn slider_bipolar(
    ui: &mut Ui,
    param: &FloatParam,
    setter: &ParamSetter,
    salt: impl std::hash::Hash + Copy + std::fmt::Debug,
    label: &str,
) {
    let wid = id(salt, "bipolar");
    control_box(ui, wid, label, |ui, inner| {
        slider_body(ui, inner, param, setter, wid, true)
    });
}

/// The momentary Strike control: the chip is swapped for a magenta
/// `Strike ↓ / •` button and the readback line reads `HOLD n x.xxx` in
/// magenta. Held at 1.0 only while pressed; released to 0.0 on pointer
/// release or cancel (`is_pointer_button_down_on` covers those). Focus
/// loss and a mid-hold kind change are the editor's job: it must call
/// `set_once(..., 0.0)` and clear the held flag (see `foley-ambience`).
/// The held flag is owned by the editor.
pub fn strike(
    ui: &mut Ui,
    param: &FloatParam,
    setter: &ParamSetter,
    salt: impl std::hash::Hash + Copy + std::fmt::Debug,
    label: &str,
    down: &mut bool,
) {
    let wid = id(salt, "strike");
    let n = param.unmodulated_normalized_value();
    control_box(ui, wid, label, |ui, inner| {
        let chip_rect = Rect::from_min_size(
            Pos2::new(inner.right() - 58.0, inner.top() + 3.0),
            Vec2::new(58.0, ROW_H),
        );
        ui.scope_builder(UiBuilder::new().max_rect(chip_rect), |ui| {
            small_text(ui);
            let text = if *down { "Strike •" } else { "Strike ↓" };
            let r = ui.add_sized(
                Vec2::new(58.0, ROW_H),
                egui::Button::new(
                    egui::RichText::new(text)
                        .color(theme::ACCENT_MAGENTA)
                        .size(9.0),
                )
                .fill(theme::SURFACE_WINDOW)
                .stroke(egui::Stroke::new(1.0, theme::ACCENT_MAGENTA)),
            );
            let is_down = r.is_pointer_button_down_on();
            if is_down && !*down {
                *down = true;
                set_once(setter, param, 1.0);
            } else if *down && !is_down {
                *down = false;
                set_once(setter, param, 0.0);
            }
        });
        draw_track(ui, track_row_rect(inner), segment_fills(n), false);
        readback_at(
            ui.painter(),
            inner,
            &format!("HOLD n {:.3}", n),
            theme::ACCENT_MAGENTA,
        );
    });
}

/// Row-2 cluster rect (width minus the readback gutter).
fn editor_row_rect(inner: Rect) -> Rect {
    Rect::from_min_size(
        Pos2::new(inner.left(), inner.top() + ROW2_TOP),
        Vec2::new(inner.width() - READBACK_W - 8.0, ROW_H),
    )
}

/// Integer control as `− / value / +` with an `n` readback. `note`
/// suffixes the chip with the pitch name.
fn stepper_inner(
    ui: &mut Ui,
    uid: Id,
    param: &IntParam,
    setter: &ParamSetter,
    label: &str,
    note: bool,
) {
    let cur = param.value();
    let n = param.unmodulated_normalized_value();
    control_box(ui, uid, label, |ui, inner| {
        let row = editor_row_rect(inner);
        ui.scope_builder(UiBuilder::new().max_rect(row), |ui| {
            small_text(ui);
            ui.horizontal(|ui| {
                if ui
                    .add_sized(Vec2::new(18.0, ROW_H), egui::Button::new("−"))
                    .clicked()
                {
                    set_once(setter, param, param.previous_step(cur, false));
                }
                let field_w = (row.width() - 36.0 - 12.0).max(24.0);
                value_field(ui, param, setter, Vec2::new(field_w, ROW_H), |n| {
                    if note {
                        format!(
                            "{} · {}",
                            param.normalized_value_to_string(n, false),
                            note_name(param.preview_plain(n).clamp(0, 127) as u8)
                        )
                    } else {
                        param.normalized_value_to_string(n, false)
                    }
                });
                if ui
                    .add_sized(Vec2::new(18.0, ROW_H), egui::Button::new("+"))
                    .clicked()
                {
                    set_once(setter, param, param.next_step(cur, false));
                }
            });
        });
        readback_at(ui.painter(), inner, &readback(n), theme::ACCENT_CYAN);
    });
}

/// Integer stepper `− / value / +`.
pub fn stepper(
    ui: &mut Ui,
    param: &IntParam,
    setter: &ParamSetter,
    salt: impl std::hash::Hash + Copy + std::fmt::Debug,
    label: &str,
) {
    let uid = id(salt, "stepper");
    stepper_inner(ui, uid, param, setter, label, false);
}

/// MIDI note spinner: chip shows `61 · C#4`.
pub fn note_stepper(
    ui: &mut Ui,
    param: &IntParam,
    setter: &ParamSetter,
    salt: impl std::hash::Hash + Copy + std::fmt::Debug,
    label: &str,
) {
    let uid = id(salt, "note");
    stepper_inner(ui, uid, param, setter, label, true);
}

/// Enum selector: full-width dropdown on row 2, `n` readback
/// (`index / count - 1`) at the right.
pub fn selector<T: Enum + PartialEq + Copy + 'static>(
    ui: &mut Ui,
    param: &EnumParam<T>,
    setter: &ParamSetter,
    salt: impl std::hash::Hash + Copy + std::fmt::Debug,
    label: &str,
) {
    let wid = id(salt, "selector");
    let variants = T::variants();
    let n = param.unmodulated_normalized_value();
    control_box(ui, wid, label, |ui, inner| {
        ui.scope_builder(UiBuilder::new().max_rect(editor_row_rect(inner)), |ui| {
            small_text(ui);
            let idx = param.value().to_index();
            let mut sel = idx;
            ComboBox::from_id_salt(wid)
                .selected_text(variants[idx % variants.len()])
                .show_ui(ui, |ui| {
                    for (v, text) in variants.iter().enumerate() {
                        ui.selectable_value(&mut sel, v, *text);
                    }
                });
            if sel != idx {
                set_once(setter, param, T::from_index(sel));
            }
        });
        readback_at(ui.painter(), inner, &readback(n), theme::ACCENT_CYAN);
    });
}

/// Latched boolean toggle (`Fresh`): a full-width On/Off button, cyan when
/// latched, with `n` reflecting the discrete state (booleans read 0 / 1).
pub fn toggle(
    ui: &mut Ui,
    param: &BoolParam,
    setter: &ParamSetter,
    salt: impl std::hash::Hash + Copy + std::fmt::Debug,
    label: &str,
) {
    let wid = id(salt, "toggle");
    let on = param.value();
    control_box(ui, wid, label, |ui, inner| {
        ui.scope_builder(UiBuilder::new().max_rect(editor_row_rect(inner)), |ui| {
            small_text(ui);
            let text = egui::RichText::new(if on { "On" } else { "Off" }).color(if on {
                theme::ACCENT_CYAN
            } else {
                theme::TEXT_SECONDARY
            });
            if ui
                .add_sized(
                    Vec2::new(ui.available_width(), ROW_H),
                    egui::Button::new(text),
                )
                .clicked()
            {
                set_once(setter, param, !on);
            }
        });
        readback_at(
            ui.painter(),
            inner,
            if on { "n 1" } else { "n 0" },
            theme::ACCENT_CYAN,
        );
    });
}

/// Rail chrome: title, Save…/Load… host recall.
pub fn chrome(ui: &mut Ui, title: &str) -> (bool, bool) {
    let mut save = false;
    let mut load = false;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(title)
                .font(FontId::new(
                    19.0,
                    FontFamily::Name(theme::FONT_HEADING.into()),
                ))
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Load…").clicked() {
                load = true;
            }
            if ui.button("Save…").clicked() {
                save = true;
            }
        });
    });
    (save, load)
}

/// Collapsible section header for a slot / layer. Returns the new expanded
/// value.
pub fn section_header(ui: &mut Ui, title: &str, expanded: bool) -> bool {
    let arrow = if expanded { "−" } else { "+" };
    let text = egui::RichText::new(format!("{arrow} {title}")).font(FontId::new(
        14.0,
        FontFamily::Name(theme::FONT_HEADING.into()),
    ));
    ui.add_sized(
        Vec2::new(ui.available_width(), 22.0),
        egui::Button::new(text),
    )
    .clicked()
        != expanded
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The contract's "Parameter control" is 228 × 36 with a 216-wide inner
    /// band: label row 3..18, readback row 20..32, track 22.5..29.5.
    fn study_inner() -> Rect {
        Rect::from_min_size(Pos2::new(6.0, 0.0), Vec2::new(216.0, 36.0))
    }

    #[test]
    fn track_row_matches_the_contract() {
        let track = track_row_rect(study_inner());
        assert_eq!(track.left(), 6.0);
        assert_eq!(track.top(), 22.5);
        assert_eq!(track.height(), 7.0);
        assert_eq!(track.width(), 170.0); // 216 - 38 readback - 8 gap
    }

    #[test]
    fn readback_sits_on_the_readback_row() {
        let pos = readback_pos(study_inner());
        assert_eq!(pos.x, 222.0); // inner right edge
        assert_eq!(pos.y, 20.0);
    }

    #[test]
    fn editor_row_matches_the_contract() {
        let row = editor_row_rect(study_inner());
        assert_eq!(row.top(), 20.0);
        assert_eq!(row.height(), 15.0);
    }
}
