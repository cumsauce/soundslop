use foley_engine::{AmbienceRig, BedKind, LAYER_CEILING, MASTER_CEILING};

const RATE: u32 = 48_000;

/// Pump `total` samples through the rig in 256-sample blocks.
fn pump(rig: &mut AmbienceRig, total: usize) -> (f32, bool) {
    let mut peak: f32 = 0.0;
    let mut finite = true;
    let mut left = vec![0.0f32; 256];
    let mut right = vec![0.0f32; 256];
    let mut done = 0usize;
    while done < total {
        let n = (total - done).min(256);
        rig.process(&mut left[..n], &mut right[..n]);
        for i in 0..n {
            finite &= left[i].is_finite() && right[i].is_finite();
            peak = peak.max(left[i].abs()).max(right[i].abs());
        }
        done += n;
    }
    (peak, finite)
}

#[test]
fn all_off_is_exact_silence() {
    let mut rig = AmbienceRig::new(RATE);
    let mut left = vec![0.25f32; 4096];
    let mut right = vec![-0.25f32; 4096];
    rig.process(&mut left, &mut right);
    assert!(left.iter().all(|&s| s == 0.0));
    assert!(right.iter().all(|&s| s == 0.0));
}

#[test]
fn single_layer_is_bounded_and_nonzero() {
    let mut rig = AmbienceRig::new(RATE);
    {
        let layers = rig.layers();
        layers[0].kind = Some(BedKind::Rain);
        layers[0].gain = 0.5;
        layers[0].seed = 7;
    }
    // Wait for the rebuilder worker to construct + deliver the Rain bed. The
    // pump loop runs in microseconds of wall time, so without an explicit
    // yield the worker may never be scheduled (fails under parallel test
    // load). Same pattern as kind_swap_rebuilds_without_panic below.
    let mut installed = false;
    for _ in 0..10_000 {
        let (_, finite) = pump(&mut rig, 256);
        assert!(finite, "output went non-finite while waiting for Rain bed");
        if rig.layer_kind(0) == Some(BedKind::Rain) {
            installed = true;
            break;
        }
        std::thread::yield_now();
    }
    assert!(installed, "Rain bed never installed");

    let (peak, finite) = pump(&mut rig, 2 * RATE as usize);
    assert!(finite);
    assert!(peak > 0.01, "expected audible output, peak {peak}");
    assert!(
        peak <= MASTER_CEILING * 1.01,
        "peak {peak} above master ceiling {MASTER_CEILING}"
    );
}

#[test]
fn guard_caps_overdriven_mix() {
    let mut rig = AmbienceRig::new(RATE);
    {
        let layers = rig.layers();
        for layer in layers.iter_mut() {
            layer.kind = Some(BedKind::Rain);
            layer.gain = 1.5;
            layer.knobs = [1.0; 4];
            layer.seed = 99;
        }
    }
    // Punt the per-layer ceiling for the test's purposes: the assertion is
    // about the MASTER guard holding even a hot mix.
    let _ = LAYER_CEILING;
    let (peak, finite) = pump(&mut rig, RATE as usize);
    assert!(finite);
    assert!(
        peak <= MASTER_CEILING + 0.01,
        "peak {peak} escaped the master ceiling"
    );
}

#[test]
fn kind_swap_rebuilds_without_panic() {
    let mut rig = AmbienceRig::new(RATE);
    {
        let layers = rig.layers();
        layers[0].kind = Some(BedKind::Rain);
        layers[0].seed = 3;
    }
    let (_, finite) = pump(&mut rig, RATE as usize / 2);
    assert!(finite);
    rig.layers()[0].kind = Some(BedKind::Wind);
    let mut swapped = false;
    for _ in 0..10_000 {
        let (_, finite) = pump(&mut rig, 256);
        assert!(finite, "output went non-finite during swap");
        if rig.layer_kind(0) == Some(BedKind::Wind) {
            swapped = true;
            break;
        }
        std::thread::yield_now();
    }
    assert!(swapped, "layer 0 never swapped to Wind");
}

#[test]
fn strike_from_outside_process_is_safe() {
    let mut rig = AmbienceRig::new(RATE);
    {
        let layers = rig.layers();
        layers[0].kind = Some(BedKind::Thunder);
        layers[0].seed = 11;
    }
    // Enqueue-only: safe before or after the bed exists.
    rig.strike(0, 42);
    let (peak, finite) = pump(&mut rig, 2 * RATE as usize);
    assert!(finite);
    let _ = peak;
}

#[test]
fn drop_joins_workers_promptly() {
    let (tx, rx) = std::sync::mpsc::channel();
    let h = std::thread::spawn(move || {
        let mut rig = AmbienceRig::new(RATE);
        rig.layers()[0].kind = Some(BedKind::Thunder);
        // Pump once so the rebuild request is in flight.
        let mut left = [0.0f32; 64];
        let mut right = [0.0f32; 64];
        rig.process(&mut left, &mut right);
        drop(rig);
        tx.send(()).unwrap();
    });
    assert!(rx.recv_timeout(std::time::Duration::from_secs(2)).is_ok());
    h.join().unwrap();
}

#[test]
fn irregular_block_sizes_never_panic() {
    for rate in [44_100u32, 96_000u32] {
        let mut rig = AmbienceRig::new(rate);
        {
            let layers = rig.layers();
            layers[0].kind = Some(BedKind::Water);
            layers[0].seed = 5;
            layers[1].kind = Some(BedKind::Fire);
            layers[1].seed = 6;
        }
        let mut rng = foley_engine::foley_dsp::Rng::new(0xC0FFEE);
        for _ in 0..200 {
            let n = 1 + (rng.unit() * 512.0) as usize;
            let n = n.min(512);
            let mut left = vec![0.0f32; n];
            let mut right = vec![0.0f32; n];
            rig.process(&mut left, &mut right);
            assert!(
                left.iter().all(|s| s.is_finite()) && right.iter().all(|s| s.is_finite()),
                "non-finite at {rate} Hz, block {n}"
            );
        }
    }
}
