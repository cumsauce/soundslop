//! Task 9: `KitRig` — MIDI-driven one-shot sampler rig.

use foley_engine::{KitRig, SlotCategory};

const RATE: u32 = 48_000;

/// Pump `blocks` process calls of `n` samples, returning the concatenated
/// mono-summed output for assertions. A short sleep per block gives the
/// clip worker wall-clock time to render and deliver (the rig's sample
/// clock advances per process call, not per wall second).
fn pump(rig: &mut KitRig, blocks: usize, n: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(blocks * n);
    let mut left = vec![0.0f32; n];
    let mut right = vec![0.0f32; n];
    for _ in 0..blocks {
        std::thread::sleep(std::time::Duration::from_micros(100));
        rig.process(&mut left, &mut right);
        for k in 0..n {
            out.push(0.5 * (left[k] + right[k]));
        }
    }
    out
}

/// Pump until `cond` holds or `blocks` elapse; returns whether it held.
fn pump_until(rig: &mut KitRig, blocks: usize, n: usize, cond: impl Fn(&KitRig) -> bool) -> bool {
    let mut left = vec![0.0f32; n];
    let mut right = vec![0.0f32; n];
    for _ in 0..blocks {
        if cond(rig) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_micros(100));
        rig.process(&mut left, &mut right);
    }
    cond(rig)
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |m, &x| m.max(x.abs()))
}

fn all_finite(samples: &[f32]) -> bool {
    samples.iter().all(|x| x.is_finite())
}

#[test]
fn unmatched_note_is_exact_silence() {
    let mut rig = KitRig::new(RATE);
    // Slot 0 keeps its default note binding (60).
    rig.note_on(61, 1.0);
    let out = pump(&mut rig, 10, 256);
    assert!(all_finite(&out));
    assert_eq!(peak(&out), 0.0, "note 61 must match no slot → silence");
}

#[test]
fn polyphony_cap_steals() {
    let mut rig = KitRig::new(RATE);
    rig.set_polyphony(4);
    for slot in rig.slots().iter_mut().take(6) {
        slot.category = SlotCategory::Contact;
        slot.kind = 0;
        slot.note = 72;
    }
    rig.note_on(72, 1.0);
    // Pump with wall-clock time for the worker; track the high-water mark
    // so a run where no clip ever arrived can't pass vacuously.
    let mut max_active = 0usize;
    let mut left = vec![0.0f32; 128];
    let mut right = vec![0.0f32; 128];
    for _ in 0..600 {
        std::thread::sleep(std::time::Duration::from_micros(100));
        rig.process(&mut left, &mut right);
        max_active = max_active.max(rig.active_voices());
        assert!(
            rig.active_voices() <= 4,
            "playing voices must never exceed the polyphony cap (got {})",
            rig.active_voices()
        );
    }
    assert!(
        max_active > 0,
        "clips must actually arrive (no vacuous pass)"
    );
}

#[test]
fn fresh_slots_vary_per_hit() {
    let mut rig = KitRig::new(RATE);
    {
        let slot = &mut rig.slots()[0];
        slot.category = SlotCategory::Contact;
        slot.kind = 0;
        slot.note = 60;
        slot.fresh = true;
    }
    // Hit 1, pump until its clip has certainly played out, hit 2, pump again.
    rig.note_on(60, 1.0);
    let win1 = pump(&mut rig, 500, 128);
    rig.note_on(60, 1.0);
    let win2 = pump(&mut rig, 500, 128);
    assert!(all_finite(&win1) && all_finite(&win2));
    assert!(
        peak(&win1) > 0.0,
        "first fresh hit must produce audible output"
    );
    assert!(
        peak(&win2) > 0.0,
        "second fresh hit must produce audible output"
    );
}

#[test]
fn voice_tail_ends_in_silence() {
    let mut rig = KitRig::new(RATE);
    {
        let slot = &mut rig.slots()[0];
        slot.category = SlotCategory::Contact;
        slot.kind = 0;
        slot.note = 60;
    }
    rig.note_on(60, 1.0);
    // Wait for the clip to arrive and start, then for it to play out —
    // breaking on active_voices()==0 up front would exit while the hit is
    // still pending on the worker (a vacuous pass).
    let started = pump_until(&mut rig, 5000, 128, |r| r.active_voices() > 0);
    assert!(started, "clip must arrive and start playing");
    let finished = pump_until(&mut rig, 10_000, 128, |r| r.active_voices() == 0);
    assert!(finished, "the voice must finish");
    let mut left = vec![0.5f32; 128]; // poison the buffers; process must overwrite
    let mut right = vec![0.5f32; 128];
    rig.process(&mut left, &mut right);
    assert!(
        left.iter().all(|&x| x == 0.0) && right.iter().all(|&x| x == 0.0),
        "post-tail block must be exact zeros"
    );
}

#[test]
fn drop_joins_worker_promptly() {
    let (tx, rx) = std::sync::mpsc::channel();
    let h = std::thread::spawn(move || {
        let mut rig = KitRig::new(RATE);
        rig.note_on(60, 1.0);
        // Pump once so a job is in flight.
        let mut left = [0.0f32; 64];
        let mut right = [0.0f32; 64];
        rig.process(&mut left, &mut right);
        drop(rig);
        tx.send(()).unwrap();
    });
    assert!(rx.recv_timeout(std::time::Duration::from_secs(2)).is_ok());
    h.join().unwrap();
}
