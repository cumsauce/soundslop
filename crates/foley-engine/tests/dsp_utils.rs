use foley_engine::{GainGuard, Motion, OnePoleLpf, layer_cutoff, pan_gains};

#[test]
fn pan_gains_pins() {
    let near = |a: f32, b: f32| (a - b).abs() < 1e-6;
    assert!(near(pan_gains(-90.0).0, 1.0) && near(pan_gains(-90.0).1, 0.0));
    assert!((pan_gains(0.0).0 - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3);
    assert!(near(pan_gains(90.0).0, 0.0) && near(pan_gains(90.0).1, 1.0));
    assert!((pan_gains(180.0).0 + std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3);
}

#[test]
fn one_pole_passthrough_at_nyquist() {
    let sample_rate = 48_000.0f32;
    let mut lpf = OnePoleLpf::new();
    let mut max_err = 0.0f32;
    for n in 0..4800 {
        let x = (2.0 * std::f32::consts::PI * 1000.0 * n as f32 / sample_rate).sin();
        let y = lpf.process(x, sample_rate, sample_rate);
        if n >= 2400 {
            max_err = max_err.max((y - x).abs());
        }
    }
    assert!(max_err < 1e-3, "max settled error {max_err}");
}

#[test]
fn one_pole_attenuates_low_cutoff() {
    let sample_rate = 48_000.0f32;
    let mut lpf = OnePoleLpf::new();
    let mut peak = 0.0f32;
    for n in 0..48_000 {
        let x = (2.0 * std::f32::consts::PI * 1000.0 * n as f32 / sample_rate).sin();
        let y = lpf.process(x, 100.0, sample_rate);
        if n >= 24_000 {
            peak = peak.max(y.abs());
        }
    }
    assert!(peak < 0.2, "settled peak {peak}");
}

#[test]
fn layer_cutoff_pins() {
    assert_eq!(layer_cutoff(0.0, 48_000.0), 24_000.0);
    assert!(layer_cutoff(64.0, 48_000.0) < layer_cutoff(8.0, 48_000.0));
}

#[test]
fn guard_caps_hot_signal() {
    let mut guard = GainGuard::new(0.4);
    let mut settled = 0.0f32;
    for n in 0..100_000 {
        let out = guard.process(2.0);
        if n >= 99_000 {
            settled = settled.max(out.abs());
        }
    }
    assert!(settled <= 0.41, "settled peak {settled}");
}

#[test]
fn guard_is_transparent_when_quiet() {
    let mut guard = GainGuard::new(0.4);
    let mut max_err = 0.0f32;
    for n in 0..10_000 {
        let x = 0.1f32;
        let out = guard.process(x);
        if n >= 5000 {
            max_err = max_err.max((out - x).abs());
        }
    }
    assert!(max_err < 0.01, "max settled error {max_err}");
}

#[test]
fn motion_bounds_and_determinism() {
    // amt = 0 → always zeros.
    let mut still = Motion::new(0.0, 1.0, 5);
    for _ in 0..1000 {
        assert_eq!(still.next(0.01), [0.0; 4]);
    }
    // Bounds hold over a long walk.
    let mut m = Motion::new(0.3, 1.0, 7);
    for _ in 0..10_000 {
        let o = m.next(0.01);
        for c in o {
            assert!(c.abs() <= 0.3 + f32::EPSILON, "offset {c} out of bounds");
        }
    }
    // Same seed → identical sequences.
    let mut a = Motion::new(0.3, 0.7, 42);
    let mut b = Motion::new(0.3, 0.7, 42);
    for _ in 0..1000 {
        assert_eq!(a.next(0.05), b.next(0.05));
    }
    // Different seeds eventually diverge.
    let mut c = Motion::new(0.3, 0.7, 42);
    let mut d = Motion::new(0.3, 0.7, 43);
    let diverges = (0..1000).any(|_| c.next(0.05) != d.next(0.05));
    assert!(diverges);
}
