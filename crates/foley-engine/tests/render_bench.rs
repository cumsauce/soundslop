//! Representative render-cost bench. Prints duration and equivalent-latency
//! milliseconds for one clip per `SlotCategory` at 48 kHz. Never asserts on
//! timing (spec §15); run with `cargo test -p foley-engine --test render_bench
//! -- --ignored --nocapture`.

use std::time::Instant;

use foley_engine::{RenderJob, SlotCategory, render_clip};

fn job(category: SlotCategory, kind: u32, note: u8) -> RenderJob {
    RenderJob {
        slot: 0,
        category,
        kind,
        voice_mob: 0,
        pitch: note,
        knobs: [0.5, 0.5],
        velocity: 0.8,
        seed: 12345,
        sample_rate: 48_000,
    }
}

fn bench(label: &str, j: RenderJob) {
    let t = Instant::now();
    let clip = render_clip(&j);
    let dt = t.elapsed();
    let ms = dt.as_secs_f64() * 1000.0;
    let audio_ms = clip.len() as f64 / 48.0; // 48k → samples/ms = 48
    println!(
        "{label:>12}: {} samples, render {ms:.2} ms, audio {audio_ms:.1} ms, ratio {:.1}x",
        clip.len(),
        audio_ms / ms.max(f64::MIN_POSITIVE)
    );
}

#[test]
#[ignore]
fn bench_one_clip_per_category() {
    bench("contact", job(SlotCategory::Contact, 0, 60));
    bench("fracture", job(SlotCategory::Fracture, 0, 60));
    bench("shot", job(SlotCategory::Shot, 0, 60));
    bench("primitive", job(SlotCategory::Primitive, 0, 60));
    bench("voice", job(SlotCategory::Voice, 0, 60));
    bench("note", job(SlotCategory::Note, 0, 60));
}
