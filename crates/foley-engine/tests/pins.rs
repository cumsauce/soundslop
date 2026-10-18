#[test]
fn pinned_foley_crates_link() {
    assert_eq!(foley_engine::foley_dsp::SAMPLE_RATE, 48_000);
    foley_engine::foley_synth::warm_voice_tables(); // Task 2 API exists at <PIN>
}
