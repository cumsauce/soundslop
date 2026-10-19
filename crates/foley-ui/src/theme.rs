//! Design tokens from the Soundslop UI contract.

use egui::{Color32, Margin, Style, Visuals};

/// Window / rack background.
pub const SURFACE_WINDOW: Color32 = Color32::from_rgb(0x23, 0x27, 0x22);
/// Raised chrome (headers).
pub const SURFACE_RAISED: Color32 = Color32::from_rgb(0x35, 0x3b, 0x33);
/// Empty slider extents / recessed wells.
pub const SURFACE_RECESSED: Color32 = Color32::from_rgb(0x1d, 0x21, 0x1d);
/// Control chips (value editors).
pub const SURFACE_CONTROL: Color32 = Color32::from_rgb(0x2c, 0x31, 0x2b);
/// Accent: filled segments, values, readback.
pub const ACCENT_CYAN: Color32 = Color32::from_rgb(0x8d, 0xce, 0xd0);
/// Accent: notices.
pub const ACCENT_NOTICE: Color32 = Color32::from_rgb(0xc6, 0xb9, 0x99);
/// Accent: the momentary Strike control.
pub const ACCENT_MAGENTA: Color32 = Color32::from_rgb(0xd2, 0xa0, 0xbe);
/// Primary text.
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(0xe5, 0xe8, 0xdc);
/// Secondary text.
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0xa8, 0xb1, 0x9f);
/// Borders.
pub const BORDER_DEFAULT: Color32 = Color32::from_rgb(0x49, 0x50, 0x44);

/// Named font families installed by [`crate::fonts::install`].
pub const FONT_MONO: &str = "SoundslopMono";
pub const FONT_INTER: &str = "SoundslopInter";
/// Inter SemiBold, for headings.
pub const FONT_HEADING: &str = "SoundslopInter-Heading";

/// Window geometry: preferred 576 × 720, minimum 380 × 520.
pub const WINDOW_MIN: (f32, f32) = (380.0, 520.0);

/// Apply the token palette to a style (fonts are installed separately).
pub fn apply(style: &mut Style) {
    let v = Visuals {
        dark_mode: true,
        override_text_color: Some(TEXT_PRIMARY),
        panel_fill: SURFACE_WINDOW,
        extreme_bg_color: SURFACE_RECESSED,
        window_fill: SURFACE_WINDOW,
        ..Visuals::dark()
    };
    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.menu_margin = Margin::same(4);
    style.visuals.widgets.inactive.bg_fill = SURFACE_CONTROL;
    style.visuals.widgets.inactive.weak_bg_fill = SURFACE_CONTROL;
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    style.visuals.widgets.hovered.bg_fill = SURFACE_RAISED;
    style.visuals.widgets.hovered.weak_bg_fill = SURFACE_RAISED;
    style.visuals.widgets.active.bg_fill = SURFACE_RAISED;
    style.visuals.widgets.active.weak_bg_fill = SURFACE_RAISED;
    style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT_CYAN);
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_SECONDARY);
}
