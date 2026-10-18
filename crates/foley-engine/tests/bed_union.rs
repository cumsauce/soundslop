use foley_engine::{BedKind, BedUnion, knob_labels, new_bed};

#[test]
fn every_kind_renders_finite_at_48000() {
    for kind in BedKind::all() {
        let mut b = new_bed(kind, 7, 48_000);
        b.set_scene_knobs([0.5; 4]);
        let mut out = [0f32; 512];
        b.render(&mut out);
        assert!(
            out.iter().all(|s| s.is_finite()),
            "{kind:?} rendered non-finite samples"
        );
    }
}

#[test]
fn strike_is_noop_except_thunder() {
    let mut wind: BedUnion = new_bed(BedKind::Wind, 1, 48_000);
    wind.set_scene_knobs([0.5; 4]);
    wind.strike(1, [0.0, 32.0, 0.0]); // no panic on non-Thunder
    let mut thunder: BedUnion = new_bed(BedKind::Thunder, 1, 48_000);
    thunder.set_scene_knobs([0.5; 4]);
    thunder.strike(1, [0.0, 32.0, 0.0]); // no panic on Thunder
}

#[test]
fn knob_labels_match_spec_table() {
    assert_eq!(
        knob_labels(BedKind::Rain),
        ["rate mm/h", "roof height (blocks)", "surface preset", ""]
    );
    assert_eq!(knob_labels(BedKind::Cave)[2], "openness");
    assert_eq!(
        knob_labels(BedKind::Surf),
        ["height (m)", "storm", "shore", ""]
    );
    assert_eq!(knob_labels(BedKind::Thunder)[1], "strike (momentary)");
    assert_eq!(
        knob_labels(BedKind::Wind),
        ["speed m/s", "exposure", "leaves", ""]
    );
    assert_eq!(
        knob_labels(BedKind::Water),
        ["flow (blocks)", "falls (blocks)", "", ""]
    );
    assert_eq!(knob_labels(BedKind::Fire), ["size (blocks)", "", "", ""]);
    assert_eq!(knob_labels(BedKind::Lava), ["area (blocks)", "", "", ""]);
    assert_eq!(knob_labels(BedKind::Underwater), ["level", "", "", ""]);
    assert_eq!(
        knob_labels(BedKind::Biome),
        ["kind", "intensity", "sub", ""]
    );
}

#[test]
fn kind_roundtrips_and_bed_reports_its_own_kind() {
    for kind in BedKind::all() {
        let b: BedUnion = new_bed(kind, 3, 48_000);
        assert_eq!(b.kind(), kind);
    }
}
