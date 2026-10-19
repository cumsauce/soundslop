#[test]
fn pinned_foley_crates_link() {
    assert_eq!(foley_engine::foley_dsp::SAMPLE_RATE, 48_000);
    foley_engine::foley_synth::warm_voice_tables(); // Task 2 API exists at <PIN>
}

#[test]
fn rigs_clamp_sample_rate_and_render_finite() {
    // Zero rate → clamped to MIN_SAMPLE_RATE, constructs and renders silence.
    let mut amb = foley_engine::AmbienceRig::new(0);
    let mut kit = foley_engine::KitRig::new(0);
    let mut l = [0.0f32; 64];
    let mut r = [0.0f32; 64];
    amb.process(&mut l, &mut r);
    kit.process(&mut l, &mut r);
    assert!(l.iter().all(|x| x.is_finite()));
    assert!(r.iter().all(|x| x.is_finite()));

    // Excessive rate → clamped to MAX_SAMPLE_RATE, constructs and renders silence.
    let mut amb2 = foley_engine::AmbienceRig::new(1_000_000);
    let mut kit2 = foley_engine::KitRig::new(1_000_000);
    amb2.process(&mut l, &mut r);
    kit2.process(&mut l, &mut r);
    assert!(l.iter().all(|x| x.is_finite()));
    assert!(r.iter().all(|x| x.is_finite()));
}
