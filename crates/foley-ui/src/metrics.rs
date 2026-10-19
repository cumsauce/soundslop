//! Pure geometry / formatting helpers shared by the Soundslop widgets.
//!
//! These encode the Figma contract's math (segment fill counts, note names,
//! readback strings) so the painter code stays thin and testable.

/// Segments in the study track (20 extents, the 20th slot is the handle's
/// resting width — the handle itself sits ON an extent index).
pub const SEGMENTS: usize = 20;

/// Handle extent index for a normalized value.
pub fn handle_index(n: f32) -> usize {
    (n.clamp(0.0, 1.0) * (SEGMENTS - 1) as f32).round() as usize
}

/// Unipolar fill: `(cyan extents before the handle, handle index, extents
/// after)`. Extents strictly below the handle index light up.
pub fn segment_fills(n: f32) -> (usize, usize, usize) {
    let h = handle_index(n);
    (h, h, SEGMENTS - 1 - h)
}

/// Bipolar (pan-style) fill from the centre boundary between segments 9 and
/// 10: `(cyan extents on the left of centre, handle index, cyan extents on
/// the right of centre)`.
pub fn bipolar_fills(n: f32) -> (usize, usize, usize) {
    let h = handle_index(n);
    let center_boundary = SEGMENTS / 2 - 1; // between segment 9 and 10
    if h <= center_boundary {
        (center_boundary - h, h, 0)
    } else {
        (0, h, h - center_boundary - 1)
    }
}

/// Whether extent `i` is lit on a bipolar track whose handle sits at `h`.
/// The span between the handle and the centre boundary (between extents 9
/// and 10) lights up; the handle's own extent is drawn as the handle.
pub fn bipolar_lit(h: usize, i: usize) -> bool {
    let center = SEGMENTS / 2; // 10: the first extent right of the boundary
    if h < center {
        i > h && i < center
    } else {
        i >= center && i < h
    }
}

/// Scientific pitch name with MIDI note 60 as C4.
pub fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = note as i32 / 12 - 1;
    format!("{}{}", NAMES[(note % 12) as usize], octave)
}

/// The cyan readback string shown right of the track.
pub fn readback(n: f32) -> String {
    format!("n {:.3}", n)
}
