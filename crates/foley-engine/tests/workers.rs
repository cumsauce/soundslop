use foley_engine::{
    BedKind, EngineJob, RebuildJob, RenderJob, SlotCategory, kind_range, render_clip,
    spawn_clip_worker, spawn_rebuilder,
};

fn job(category: SlotCategory, kind: u32) -> RenderJob {
    RenderJob {
        slot: 0,
        category,
        kind,
        voice_mob: 0,
        pitch: 60,
        knobs: [0.5, 0.5],
        velocity: 0.8,
        seed: 42,
        sample_rate: 48_000,
    }
}

#[test]
fn clip_worker_roundtrip_reuses_buffers() {
    let (jtx, jrx) = std::sync::mpsc::sync_channel(32);
    let (rtx, rrx) = std::sync::mpsc::sync_channel(32);
    spawn_clip_worker(jrx, rtx);
    // Bell (shot kind 17): a cheap modal ring.
    jtx.send(EngineJob::Render(job(SlotCategory::Shot, 17)))
        .unwrap();
    let clip1 = rrx.recv().unwrap();
    assert!(!clip1.is_empty() && clip1.iter().all(|s| s.is_finite()));
    let cap = clip1.capacity();
    assert!(cap > 0);
    jtx.send(EngineJob::Free(clip1)).unwrap();
    jtx.send(EngineJob::Render(job(SlotCategory::Shot, 17)))
        .unwrap();
    let clip2 = rrx.recv().unwrap();
    assert!(
        clip2.capacity() >= cap,
        "clip2 capacity {} < clip1 capacity {}",
        clip2.capacity(),
        cap
    );
}

#[test]
fn rebuilder_produces_requested_kind() {
    let (jtx, jrx) = std::sync::mpsc::sync_channel(1);
    let (otx, orx) = std::sync::mpsc::sync_channel(1);
    spawn_rebuilder(jrx, otx);
    jtx.send(RebuildJob {
        kind: BedKind::Rain,
        seed: 7,
        sample_rate: 48_000,
    })
    .unwrap();
    assert_eq!(orx.recv().unwrap().kind(), BedKind::Rain);
}

#[test]
fn render_clip_smoke_every_category() {
    let cases = [
        (SlotCategory::Contact, 0),   // Stone + Tap
        (SlotCategory::Fracture, 0),  // Stone
        (SlotCategory::Shot, 17),     // bell
        (SlotCategory::Primitive, 1), // noise_burst, 0.5 s
        (SlotCategory::Voice, 0),     // Mob 0 / Call 0
        (SlotCategory::Note, 0),      // instrument 0, MIDI 60
    ];
    for (cat, kind) in cases {
        let clip = render_clip(&job(cat, kind));
        assert!(!clip.is_empty(), "{cat:?} kind {kind} rendered empty");
        assert!(
            clip.iter().all(|s| s.is_finite()),
            "{cat:?} kind {kind} rendered non-finite"
        );
    }
}

#[test]
fn render_clip_out_of_range_is_empty_not_a_panic() {
    let cases = [
        (SlotCategory::Contact, 40),
        (SlotCategory::Fracture, 10),
        (SlotCategory::Shot, 19),
        (SlotCategory::Primitive, 6),
        (SlotCategory::Voice, 43),
        (SlotCategory::Note, 16),
    ];
    for (cat, kind) in cases {
        assert!(
            render_clip(&job(cat, kind)).is_empty(),
            "{cat:?} kind {kind} should render empty"
        );
    }
}

#[test]
fn kind_range_pins() {
    assert_eq!(kind_range(SlotCategory::Contact), 39);
    assert_eq!(kind_range(SlotCategory::Fracture), 9);
    assert_eq!(kind_range(SlotCategory::Shot), 18);
    assert_eq!(kind_range(SlotCategory::Primitive), 5);
    assert_eq!(kind_range(SlotCategory::Voice), 42);
    assert_eq!(kind_range(SlotCategory::Note), 15);
}
