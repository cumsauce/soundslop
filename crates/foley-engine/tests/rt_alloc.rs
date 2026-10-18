//! Realtime-safety probe: a thread-local counting global allocator that
//! mirrors `crates/foley-ffi/src/no_alloc.rs`. It counts only allocations
//! made on the *watching* thread, so worker threads may allocate freely while
//! the audio-thread path must allocate zero times after warm-up.
#![allow(unsafe_code)] // The counting allocator is necessarily unsafe.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use foley_engine::{AmbienceRig, BedKind, KitRig, LAYERS, SlotCategory};

thread_local! {
    static WATCHING: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

struct Counting;

fn note() {
    let _ = WATCHING.try_with(|watching| {
        if watching.get() {
            COUNT.with(|count| count.set(count.get() + 1));
        }
    });
}

// SAFETY: every call passes straight through to the system allocator.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note();
        // SAFETY: as the caller guarantees for `alloc`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note();
        // SAFETY: as the caller guarantees for `alloc_zeroed`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        note();
        // SAFETY: as the caller guarantees for `dealloc`.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note();
        // SAFETY: as the caller guarantees for `realloc`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// How many times `f` allocated or freed on this thread.
fn allocations(f: impl FnOnce()) -> usize {
    COUNT.with(|count| count.set(0));
    WATCHING.with(|watching| watching.set(true));
    f();
    WATCHING.with(|watching| watching.set(false));
    COUNT.with(Cell::get)
}

#[test]
fn ambience_rig_allocates_zero_after_warmup() {
    for rate in [44_100u32, 48_000, 96_000] {
        let mut rig = AmbienceRig::new(rate);
        // One layer on Rain (so a bed gets built + installed) and one on
        // Thunder (so a mid-run strike is meaningful).
        rig.layers()[0].kind = Some(BedKind::Rain);
        rig.layers()[1].kind = Some(BedKind::Thunder);

        // Warm-up: one block so the rebuild is enqueued / possibly installed.
        let mut wl = vec![0.0f32; 128];
        let mut wr = vec![0.0f32; 128];
        rig.process(&mut wl, &mut wr);

        let mut left = [0.0f32; 512];
        let mut right = [0.0f32; 512];
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let count = allocations(|| {
            for b in 0..200usize {
                // Seeded irregular block size in 1..=512.
                seed = seed
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let n = ((seed >> 33) as usize % 512) + 1;
                if b == 100 {
                    // Mid-run: swap a third layer's kind and strike Thunder.
                    rig.layers()[2].kind = Some(BedKind::Fire);
                    rig.strike(1, 42);
                }
                rig.process(&mut left[..n], &mut right[..n]);
            }
        });
        assert_eq!(count, 0, "audio-thread allocations at {rate} Hz: {count}");
    }
}

#[test]
fn kit_rig_allocates_zero_on_note_storm() {
    let mut rig = KitRig::new(48_000);
    // Eight slots across categories, each on a distinct note.
    let cats = [
        SlotCategory::Contact,
        SlotCategory::Fracture,
        SlotCategory::Shot,
        SlotCategory::Primitive,
        SlotCategory::Voice,
        SlotCategory::Note,
        SlotCategory::Contact,
        SlotCategory::Shot,
    ];
    for (i, slot) in rig.slots().iter_mut().enumerate() {
        slot.category = cats[i];
        slot.note = 60 + i as u8;
        slot.gain = 0.8;
        slot.seed = (i as u64 + 1) * 1000;
        slot.fresh = true;
    }

    // Warm-up so the clip worker spins up.
    let mut wl = vec![0.0f32; 128];
    let mut wr = vec![0.0f32; 128];
    rig.process(&mut wl, &mut wr);

    let mut left = [0.0f32; 512];
    let mut right = [0.0f32; 512];
    let count = allocations(|| {
        for n in 0..32u8 {
            rig.note_on(60 + (n % 8), 0.8);
            for _ in 0..4 {
                rig.process(&mut left, &mut right);
            }
        }
    });
    assert_eq!(count, 0, "kit audio-thread allocations: {count}");
}

#[test]
fn two_instances_share_tables_safely() {
    // Review Focus: two instances sharing the OnceLock voice tables.
    let mut a = AmbienceRig::new(48_000);
    let mut b = AmbienceRig::new(48_000);
    let mut k = KitRig::new(48_000);
    a.layers()[0].kind = Some(BedKind::Water);
    b.layers()[0].kind = Some(BedKind::Wind);
    k.slots()[0].category = SlotCategory::Primitive;
    k.slots()[0].note = 60;

    let mut la = [0.0f32; 256];
    let mut ra = [0.0f32; 256];
    let mut lb = [0.0f32; 256];
    let mut rb = [0.0f32; 256];
    let mut lk = [0.0f32; 256];
    let mut rk = [0.0f32; 256];
    for _ in 0..50 {
        k.note_on(60, 0.9);
        a.process(&mut la, &mut ra);
        b.process(&mut lb, &mut rb);
        k.process(&mut lk, &mut rk);
    }
    for pair in [
        la.iter().zip(ra.iter()),
        lb.iter().zip(rb.iter()),
        lk.iter().zip(rk.iter()),
    ] {
        for (x, y) in pair {
            assert!(x.is_finite() && y.is_finite());
        }
    }
    // Silence-by-default layers render finite (possibly zero) output; the
    // point is no panic / no NaN when two instances share the tables.
    assert_eq!(LAYERS, 6);
}
