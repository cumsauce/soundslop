//! Embedded typefaces: IBM Plex Mono (keys / labels / values) and Inter
//! (headings / descriptions). Applied with `egui_ctx.set_fonts`.

use egui::{Context, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

use crate::theme::{FONT_HEADING, FONT_INTER, FONT_MONO};

/// IBM Plex Mono Regular (latin), SIL OFL.
pub static PLEX_MONO: &[u8] = include_bytes!("../assets/IBMPlexMono-Regular.ttf");
/// Inter Regular (latin), SIL OFL.
pub static INTER_REGULAR: &[u8] = include_bytes!("../assets/Inter-Regular.ttf");
/// Inter SemiBold (latin), SIL OFL.
pub static INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/Inter-SemiBold.ttf");

/// Register the embedded faces and route the default families to them.
pub fn install(ctx: &Context) {
    // Keep egui's bundled faces as fallbacks so symbols outside the latin
    // subsets (arrows, bullets, ellipses) still render.
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        FONT_MONO.to_owned(),
        Arc::new(FontData::from_static(PLEX_MONO)),
    );
    fonts.font_data.insert(
        FONT_INTER.to_owned(),
        Arc::new(FontData::from_static(INTER_REGULAR)),
    );
    fonts.font_data.insert(
        format!("{FONT_INTER}-SemiBold"),
        Arc::new(FontData::from_static(INTER_SEMIBOLD)),
    );

    // Everything defaults to the mono voice; headings opt into Inter.
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, FONT_MONO.to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, FONT_MONO.to_owned());
    fonts
        .families
        .entry(FontFamily::Name(FONT_INTER.into()))
        .or_default()
        .insert(0, FONT_INTER.to_owned());
    fonts
        .families
        .entry(FontFamily::Name(FONT_HEADING.into()))
        .or_default()
        .insert(0, format!("{FONT_INTER}-SemiBold"));

    ctx.set_fonts(fonts);
}
