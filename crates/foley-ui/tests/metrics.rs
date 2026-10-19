//! Pure-logic tests for the Soundslop rack widgets (Figma contract geometry).

use foley_ui::metrics::{
    SEGMENTS, bipolar_fills, bipolar_lit, handle_index, note_name, readback, segment_fills,
};

// ---- segmented track (20 segments, handle occupies one) ----

#[test]
fn track_has_twenty_segments() {
    assert_eq!(SEGMENTS, 20);
}

#[test]
fn handle_index_sits_on_study_positions() {
    // Study n=0.35: handle extent at x=59.5 -> segment 7 (7 * 8.5 = 59.5).
    assert_eq!(handle_index(0.35), 7);
    // Study n=0.60: 11 filled extents + handle + 8 empty -> handle at 11.
    assert_eq!(handle_index(0.60), 11);
}

#[test]
fn handle_index_clamps_to_track_ends() {
    assert_eq!(handle_index(0.0), 0);
    assert_eq!(handle_index(1.0), SEGMENTS - 1);
}

#[test]
fn segment_fills_match_study_counts() {
    // n=0.35: 7 filled extents before the handle, 12 after.
    let (before, handle, after) = segment_fills(0.35);
    assert_eq!((before, handle, after), (7, 7, 12));
    // n=0.60: 11 filled extents + 1 handle + 8 empty.
    let (before, handle, after) = segment_fills(0.60);
    assert_eq!((before, handle, after), (11, 11, 8));
}

#[test]
fn bipolar_fills_from_center() {
    // Pan 0 -> center handle, no filled extents either side.
    assert_eq!(bipolar_fills(0.5), (0, 10, 0));
    // Pan max -> handle at 19, the 9 extents strictly between the centre
    // boundary (between segments 9 and 10) and the handle are lit.
    let (_, handle, right) = bipolar_fills(1.0);
    assert_eq!(handle, SEGMENTS - 1);
    assert_eq!(right, 9);
    // n=0.0: handle 0, the 9 extents strictly right of the handle up to the
    // centre boundary are lit.
    let (left, handle, right) = bipolar_fills(0.0);
    assert_eq!(handle, 0);
    assert_eq!(left, 9);
    assert_eq!(right, 0);
}

#[test]
fn bipolar_lit_spans_handle_to_centre() {
    let lit = |h: usize| {
        (0..SEGMENTS)
            .filter(|i| bipolar_lit(h, *i))
            .collect::<Vec<_>>()
    };
    // Pan hard left: handle at 0, extents 1..=9 (towards the centre) lit.
    assert_eq!(lit(0), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    // Pan hard right: handle at 19, extents 10..=18 lit.
    assert_eq!(lit(19), (10..=18).collect::<Vec<_>>());
    // Centre seating lights nothing.
    assert!(lit(9).is_empty());
    assert!(lit(10).is_empty());
    // Partway left (handle 5) and right (handle 14): four extents each.
    assert_eq!(lit(5), vec![6, 7, 8, 9]);
    assert_eq!(lit(14), vec![10, 11, 12, 13]);
}

#[test]
fn bipolar_lit_agrees_with_bipolar_fill_counts() {
    for h in 0..SEGMENTS {
        let (before, _, after) = bipolar_fills(h as f32 / (SEGMENTS - 1) as f32);
        let lit = (0..SEGMENTS).filter(|i| bipolar_lit(h, *i)).count();
        assert_eq!(lit, before + after, "handle {h}");
    }
}

// ---- MIDI note name ----

#[test]
fn note_name_uses_scientific_pitch() {
    assert_eq!(note_name(60), "C4");
    assert_eq!(note_name(69), "A4");
    assert_eq!(note_name(0), "C-1");
    assert_eq!(note_name(127), "G9");
}

#[test]
fn note_name_sharps() {
    assert_eq!(note_name(61), "C#4");
    assert_eq!(note_name(63), "D#4");
}

// ---- normalized readback ----

#[test]
fn readback_shows_three_decimals_with_n_prefix() {
    assert_eq!(readback(0.35), "n 0.350");
    assert_eq!(readback(0.0), "n 0.000");
    assert_eq!(readback(1.0), "n 1.000");
}
