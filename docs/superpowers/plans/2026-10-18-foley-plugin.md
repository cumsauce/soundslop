# Foley Plugins Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship two nih-plug CLAP+VST3 plugins — **Foley Kit** (MIDI one-shot instrument) and **Foley Ambience** (6-layer generative soundscape) — that reuse the foley mod's Rust DSP crates directly, made possible by a behavior-preserving sample-rate refactor of those crates in place (spec §7, Option A).

**Architecture:** Phase 0 (Tasks 1–3) makes `foley-dsp`/`foley-synth` sample-rate-generic inside the **foley repo** (`/Volumes/My Shared Files/mc/foley`), mod bit-identical at 48 kHz, and produces the pinned rev. Then a new **plugin repo** (`/Volumes/My Shared Files/mc/foley-plugin`, this repo) consumes those pinned crates through `foley-engine` (no nih-plug dependency: bed wrapper `BedUnion`, knob→scene mapping, pan/LPF/guards/motion, one rebuild worker + one clip worker per instance, headless `AmbienceRig`/`KitRig`). The two plugin crates are thin nih-plug shells (params + process + egui editor) over the rigs. Audio threads never allocate; all construction/one-shot rendering happens on workers using thunder.rs's bounded-channel + `Job::Free` buffer-recycling pattern.

**Tech Stack:** Rust 1.99 / edition 2024 / resolver 3, `#![deny(unsafe_code)]`; `nih_plug` + `nih_plug_egui` (egui 0.31) at git rev `de421011f41a6d10fc8c7a6084e4f4dee0143683`; `foley-math`/`foley-dsp`/`foley-synth` as git deps at `<PIN>` (Task 3 output); `rfd` for file dialogs; `nih_plug_xtask` for bundling.

**Spec:** `docs/superpowers/specs/2026-10-18-foley-plugin-design.md` (all §-references below point there)

## Global Constraints

- Two repos: **foley repo** = `/Volumes/My Shared Files/mc/foley` (Tasks 1–3 run there); **plugin repo** = `/Volumes/My Shared Files/mc/foley-plugin` (Tasks 4–13, this repo). Paths in tasks are relative to the repo named in the task.
- Phase 0 must not clobber in-flight ABI-25/BOOM work (spec §6): before Task 1, `git status --porcelain` in the foley repo; if `foley-ffi/src/lib.rs` or `foley-synth/src/{lib.rs,oneshot.rs}` are still dirty, stop and ask the author. Commit only the rate-refactor hunks (`git add -p`).
- foley dep pin: `rev = "<PIN>"` = merged post-Task-3 master SHA. Never link `foley-ffi`. Never fork the DSP crates.
- nih-plug pin: `https://github.com/robbert-vdh/nih-plug` rev `de421011f41a6d10fc8c7a6084e4f4dee0143683` for both `nih_plug` and `nih_plug_egui`.
- Toolchain: `rust-version = "1.99"`, `edition = "2024"`, `resolver = "3"`, `publish = false`, `[workspace.lints.rust] unsafe_code = "deny"` (opt in per crate via `[lints] workspace = true`). CI runs `cargo clippy --workspace -- -D warnings`.
- Sample rate: host's `BufferConfig.sample_rate` (f32) passed into every bed constructor and one-shot call; supported range 32–192 kHz; no resampler; zero reported latency.
- Mod gate (acceptance for every foley-repo task): `cargo test --workspace --locked`, `cargo xtask header --check`, and `cargo xtask levels` with a peak table **identical to the pre-change baseline** (no scene peak above 0.4).
- Engine constants: `MAX_BLOCK: usize = 512`, `MAX_VOICES: usize = 16`, `LAYER_CEILING: f32 = 0.4`, `MASTER_CEILING: f32 = 0.85`.
- Both plugins: `ProcessStatus::KeepAlive` always, `HARD_REALTIME_ONLY = false`, `SAMPLE_ACCURATE_AUTOMATION = false`, `MidiConfig::Basic` (input). One worker thread per instance (`AmbienceRig`: rebuilder; `KitRig`: clip worker) + at most one Thunder thread per bed; all joined on Drop.
- Param ids exactly as spec §10 (Ambience, 68: `l{i}_kind/gain/seed/motion_amt/motion_rate/dist/az/knob_a/knob_b/knob_c/knob_d` + `output_gain` + `scene_glide`) and §11 (Kit, 83: `s{i}_category/kind/voice_mob/note/knob_a/knob_b/gain/pan/seed/fresh` + `output_gain` + `polyphony` + `humanize`).
- Ids: CLAP `io.github.cumsauce.foley.kit` / `io.github.cumsauce.foley.ambience`; VST3 class ids `*b"F0leyKit.0100001"` and `*b"F0leyAmbience.01"` (arbitrary but frozen); `VST3_SUBCATEGORIES = &[Instrument, Synth]`, `CLAP_FEATURES = &[Instrument, Synthesizer]` for both.
- License: repo is `GPL-3.0-or-later` (VST3 `vst3-sys` requirement, spec §2).
- Ambience layer order (param `kind` variant order == foley.h): `Off, Rain, Wind, Water, Fire, Lava, Thunder, Cave, Underwater, Biome, Surf`.

## Review Focus

1. **44.1 kHz host** — every bed and one-shot renders finite, panic-free audio at 44_100 Hz with lengths scaling with rate; nothing assumes 48 k. → Tests: Task 1 (`rate_generic` bed render at 44_100), Task 2 (one-shots at 44_100), Task 13 (`rt_alloc` rig loop at 44_100).
2. **96 kHz (and 192 kHz) host** — same correctness at high rates, and `Wind`'s fixed `MAX_LEAD` cap is sized so 192 kHz never clips the gust lead. → Tests: Task 1 (wind at 192_000), Task 13 (rig loop at 96_000).
3. **Host unloads the plugin while a Thunder layer is active** — every worker thread (rebuilder, clip worker, Thunder's own) is joined in bounded time on Drop, no leak, no hang. → Tests: Task 2 (`thunder_shutdown` join < 1 s, idempotent), Task 8 (rig Drop joined within 2 s watchdog).
4. **Two plugin instances in one process** — foley-synth's lazy `TABLES` OnceLock is built once, immutable after `warm_voice_tables()`, no cross-talk. → Tests: Task 2 (`warm_voice_tables` idempotent), Task 13 (two rigs rendering interleaved).
5. **Thunder strike triggered from the UI thread** (momentary param rising edge) — strike only enqueues: never blocks or allocates on the audio thread; a strike still lands. → Test: Task 8 (`strike` while the process loop runs).
6. **Empty scene / unmatched note** — all 6 layers `Off` → exact 0.0 output; `NoteOn` matching no slot → exact 0.0; both no-panic, no-alloc. → Tests: Task 8 (all-off), Task 9 (no-slot silence).

## File Structure

```
foley repo (Tasks 1–3 only; existing crates, behavior-preserving edits):
  crates/foley-dsp/src/**          rate-generic refactor (Task 1)
  crates/foley-synth/src/**        rate-generic + warm_voice_tables + Thunder::shutdown (Task 2)
  crates/foley-ffi/src/lib.rs      call sites only, DEFAULT_SAMPLE_RATE (Task 3)
  crates/foley-sim/**              call sites only (Task 3)

plugin repo (this repo):
  Cargo.toml                       workspace (resolver 3, shared lints/pins)
  bundler.toml                     display names
  xtask/src/main.rs                2-line nih_plug_xtask wrapper
  .github/workflows/ci.yml         lint + tests + rt-probe + bundle matrix (Task 13)
  crates/foley-engine/src/
    lib.rs                         constants, re-exports, module wiring
    bed_union.rs                   BedKind, BedUnion, new_bed            (Task 5)
    kind_map.rs                    scene_from_knobs, knob_labels        (Task 5)
    spatial.rs                     pan_gains, OnePoleLpf, layer_cutoff  (Task 6)
    guard.rs                       GainGuard                            (Task 6)
    motion.rs                      Motion                               (Task 6)
    seed_mix.rs                    hit_seed                             (Task 7)
    kit_map.rs                     SlotCategory, RenderJob, render_clip, kind_range (Task 7)
    clip.rs                        EngineJob, spawn_clip_worker         (Task 7)
    rebuild.rs                     RebuildJob, spawn_rebuilder          (Task 7)
    rig_ambience.rs                AmbienceRig, LayerState              (Task 8)
    rig_kit.rs                     KitRig, SlotState                    (Task 9)
  crates/foley-engine/tests/       pins, bed_union, dsp_utils, workers, seed_mix,
                                   ambience_rig, kit_rig, rt_alloc, render_bench
  crates/foley-ambience/src/       lib.rs (Plugin), params.rs, editor.rs (Tasks 10, 12)
  crates/foley-kit/src/            lib.rs (Plugin), params.rs, editor.rs (Tasks 11, 12)
```

---

### Task 1: foley-dsp becomes sample-rate-generic

**Repo:** foley (`/Volumes/My Shared Files/mc/foley`). Branch off `master` first (`git checkout -b rate-generic`).

**Files:**
- Modify: `crates/foley-dsp/src/lib.rs` (keep `SAMPLE_RATE` at line 40; add `DEFAULT_SAMPLE_RATE`)
- Modify: every `crates/foley-dsp/src/**` module touching `SAMPLE_RATE` (~60 sites; beds `Rain, Water, Fire, Lava, Cave, Underwater, Wind` + any dsp-local one-shots)
- Test: `crates/foley-dsp/tests/rate_generic.rs`

**Interfaces:**
- Consumes: existing bed API `new(...)` / `set_scene(...)` / `render(&mut self, out: &mut [f32])` (unchanged except arity, below).
- Produces: `pub const DEFAULT_SAMPLE_RATE: u32 = SAMPLE_RATE;`; every foley-dsp bed constructor gains a trailing `sample_rate: u32` parameter; per-instance rate stored as a field and used where bodies did `let rate = SAMPLE_RATE as f32;`. The four genuine const sites in foley-dsp: `wind.rs:47 LEAD` → `pub const MAX_LEAD: usize` (same formula evaluated at 192 kHz, with the per-instance value a `usize` field capped at `MAX_LEAD`); `fire.rs:84 TICK_SAMPLES` → private field computed in `new()`.

- [ ] **Step 1: Write the failing test**

```rust
// crates/foley-dsp/tests/rate_generic.rs
fn finite(buf: &[f32]) -> bool { buf.iter().all(|s| s.is_finite()) }

macro_rules! render_at { // one case per bed, so each bed is independently auditable
    ($name:ident, $ctor:expr) => {
        #[test]
        fn $name() {
            for rate in [44_100u32, 96_000, 192_000] {
                let mut bed = $ctor(rate);
                let mut out = vec![0.0f32; 512];
                bed.render(&mut out);
                assert!(finite(&out), "non-finite at {rate} Hz");
            }
        }
    };
}
// One macro invocation per bed with its existing new() args plus the new trailing
// sample_rate, e.g. render_at!(rain_44100_96k_192k, |rate| Rain::new(1234, rate));
// (fill in each bed's actual seed/scene args from the crate as of this branch).
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p foley-dsp --test rate_generic`
Expected: FAIL — compile error, constructors take no `sample_rate` argument.

- [ ] **Step 3: Implement the rate threading**

Add `DEFAULT_SAMPLE_RATE` next to `SAMPLE_RATE`. Give every bed constructor a trailing `sample_rate: u32`, store it, and replace each body's `let rate = SAMPLE_RATE as f32;` with the field. `Wind`: `MAX_LEAD` const sized from the current `LEAD` formula at 192 kHz, instance field clamped to it. `Fire`: `TICK_SAMPLES` becomes a field computed in `new()`. No other behavior changes; 48_000 bodies must produce identical arithmetic (same expressions, rate now from the field).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p foley-dsp`
Expected: PASS — new test and the entire existing foley-dsp suite.

- [ ] **Step 5: Run the mod gate (partial)**

Run: `cargo test --workspace --locked`
Expected: PASS except foley-ffi/foley-sim compile errors at call sites — that is Task 3's scope; foley-dsp and foley-synth suites must be green.

- [ ] **Step 6: Commit**

```bash
git add crates/foley-dsp
git commit -m "feat(dsp): thread sample_rate through beds; DEFAULT_SAMPLE_RATE keeps 48k default"
```

### Task 2: foley-synth rate-generic + `warm_voice_tables()` + `Thunder::shutdown`

**Repo:** foley. Same branch as Task 1.

**Files:**
- Modify: `crates/foley-synth/src/**` — all ~249 `SAMPLE_RATE` sites: `thunder.rs`, `outdoors.rs` (`SURF_LEN` line 11, `KNOCK_LEN` line 14), `note.rs`, `voice/**`, `lib.rs` (re-exports), `oneshot.rs`
- Modify: `crates/foley-synth/src/lf.rs` (the `TABLES` OnceLock at line 104)
- Test: `crates/foley-synth/tests/rate_generic.rs`, `crates/foley-synth/tests/thunder_shutdown.rs`, `crates/foley-synth/tests/voice_tables.rs`

**Interfaces:**
- Consumes: Task 1's `DEFAULT_SAMPLE_RATE` (re-exported from `foley_dsp`).
- Produces:
  - Every foley-synth one-shot fn gains a trailing `sample_rate: u32`: e.g. `pub fn contact(contact: Contact, material: Material, speed: f32, seed: u64, sample_rate: u32) -> Vec<f32>`, `pub fn note(instrument: u32, note: f32, character: f32, seed: u64, sample_rate: u32) -> Option<Vec<f32>>`, `pub fn voice(mob: Mob, call: Call, seed: u64, sample_rate: u32) -> Vec<f32>`; same trailing-arg rule for every named shot (`bell`, `crickets`, `drip_into_water`, … 19 total) and `contact_with`/`fracture`/`fracture_with`/`creature_voice`.
  - Synth beds gain trailing `sample_rate: u32` on `new`; `Thunder::new(seed: u64, sample_rate: u32)` also **gains a seed** (today it takes none).
  - `pub fn surf_len(rate: u32) -> usize` in `outdoors.rs`; the `SURF_LEN` const is kept, defined as `surf_len(DEFAULT_SAMPLE_RATE)` (foley-ffi compat).
  - `woodpecker`'s `KNOCK_LEN` const becomes a local computed from the rate.
  - `pub fn warm_voice_tables()` in `lf.rs`, re-exported at `lib.rs` root: forces the `TABLES` OnceLock; idempotent.
  - `pub fn shutdown(&mut self)` on `Thunder`: drops the job sender and joins the worker thread; idempotent (second call no-op). `Drop` keeps delegating to `shutdown()`.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/foley-synth/tests/rate_generic.rs  (shape)
#[test] fn oneshots_finite_at_nondefault_rates() { /* call one fn per family
  (contact, fracture, note, voice, and each of the 19 named shots) at
  44_100 and 96_000 with representative args + trailing rate; assert all
  finite and non-empty where the 48 kHz path is non-empty */ }
#[test] fn surf_len_scales_with_rate() {
    assert!(foley_synth::surf_len(96_000) > foley_synth::surf_len(44_100));
    assert_eq!(foley_synth::SURF_LEN, foley_synth::surf_len(foley_synth::DEFAULT_SAMPLE_RATE));
}

// crates/foley-synth/tests/thunder_shutdown.rs
#[test] fn thunder_worker_joins_on_shutdown() {
    let mut t = foley_synth::Thunder::new(7, 48_000);
    t.strike(/* base/args per current signature */);
    let started = std::time::Instant::now();
    t.shutdown();
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
    t.shutdown(); // idempotent no-op
}
#[test] fn thunder_renders_strikes_at_44100() { /* new(9, 44_100), strike, pump
  render() for ~2 s of frames, assert finite */ }

// crates/foley-synth/tests/voice_tables.rs
#[test] fn warm_voice_tables_is_idempotent() {
    foley_synth::warm_voice_tables();
    foley_synth::warm_voice_tables(); // second call must not panic or rebuild
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p foley-synth --tests`
Expected: FAIL — compile errors (missing `sample_rate` args, no `warm_voice_tables`/`shutdown`/`surf_len`).

- [ ] **Step 3: Implement**

Thread `sample_rate: u32` as the trailing argument everywhere per Interfaces; convert `SURF_LEN`/`KNOCK_LEN` per spec §7; add `warm_voice_tables()` (move the table-build body of `tables()` behind the OnceLock so it can be forced eagerly); add `Thunder::shutdown()` + seed on `new`. Keep the worker loop and channel bounds untouched.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p foley-synth`
Expected: PASS — new tests + full existing foley-synth suite.

- [ ] **Step 5: Commit**

```bash
git add crates/foley-synth
git commit -m "feat(synth): rate-generic one-shots/beds; warm_voice_tables; Thunder seed+shutdown"
```

### Task 3: foley-ffi / foley-sim call sites + full mod gate battery → `<PIN>`

**Repo:** foley. Same branch. This task produces the `<PIN>` every later task depends on.

**Files:**
- Modify: `crates/foley-ffi/src/lib.rs` — **call sites only**; no exported symbol, ABI, or policy change
- Modify: `crates/foley-sim/src/main.rs` (+ any other sim/xtask call sites)

**Interfaces:**
- Consumes: Tasks 1–2 signatures + `DEFAULT_SAMPLE_RATE`.
- Produces: `<PIN>` — the merged master SHA after this task; recorded in the plan log and used by Task 4. `foley-ffi`'s exported symbols are byte-identical.

- [ ] **Step 1: Verify the tree is safe**

Run: `git status --porcelain`
Expected: the ABI-25/BOOM work is committed (or absent). If those files are still dirty, STOP and ask the author (Global Constraints).

- [ ] **Step 2: Capture the baseline peak table**

Run: `cargo xtask levels`
Expected: peak table printed; copy it verbatim into the commit message below (this is the parity baseline).

- [ ] **Step 3: Fix call sites to compile**

Run: `cargo test --workspace --locked`
Expected: FAIL (compile errors at foley-ffi/foley-sim call sites). Fix every call site by appending `DEFAULT_SAMPLE_RATE` (or `Thunder::new(seed, DEFAULT_SAMPLE_RATE)`). Nothing else changes.

- [ ] **Step 4: Run the full mod gate battery**

Run (each must pass):
- `cargo test --workspace --locked` → PASS
- `cargo xtask header --check` → PASS
- `cargo xtask levels` → table **identical** to Step 2's baseline
- `cargo xtask dist` → PASS (runs the mod's gradle test + gametest)

- [ ] **Step 5: Commit, push, merge, record `<PIN>`**

```bash
git add -p   # stage ONLY the rate-refactor hunks; leave any unrelated work unstaged
git commit -m "feat: pass DEFAULT_SAMPLE_RATE at ffi/sim call sites (mod audio bit-identical)"
git push -u origin rate-generic
```
Open/merge the PR to `master`. Record the merged master SHA as `<PIN>`; substitute it literally in Task 4. If you cannot merge, report the branch SHA to the author and wait.

### Task 4: Plugin repo workspace scaffold + dependency pins

**Repo:** plugin (`/Volumes/My Shared Files/mc/foley-plugin`).

**Files:**
- Create: `Cargo.toml` (workspace: `members = ["xtask", "crates/*"]`, `resolver = "3"`, `[workspace.package] edition = "2024", rust-version = "1.99", publish = false`, `[workspace.lints.rust] unsafe_code = "deny"`; `[workspace.dependencies]` with the pins below)
- Create: `xtask/Cargo.toml`, `xtask/src/main.rs` (`fn main() { nih_plug_xtask::main() }`)
- Create: `bundler.toml` (name `Foley Kit` → `foley-kit`, `Foley Ambience` → `foley-ambience`)
- Create: `LICENSE.txt` (GPL-3.0-or-later), `.gitignore` (`/target`, `/target/bundled`)
- Create: `crates/foley-engine/Cargo.toml`, `crates/foley-engine/src/lib.rs`
- Test: `crates/foley-engine/tests/pins.rs`

**Interfaces:**
- Consumes: `<PIN>` from Task 3.
- Produces (workspace dependency declarations, verbatim):
  - `foley-math`, `foley-dsp`, `foley-synth`: `git = "https://github.com/cumsauce/foley"`, `rev = "<PIN>"`
  - `nih_plug`, `nih_plug_egui`: `git = "https://github.com/robbert-vdh/nih-plug"`, `rev = "de421011f41a6d10fc8c7a6084e4f4dee0143683"`
  - `[patch."https://github.com/cumsauce/foley"]` → `../foley/crates/<crate>` path entries (dev convenience; keep committed, commented as dev-only)
- Produces (foley-engine lib, used by all later tasks):
  - `pub const MAX_BLOCK: usize = 512;` `pub const MAX_VOICES: usize = 16;` `pub const LAYER_CEILING: f32 = 0.4;` `pub const MASTER_CEILING: f32 = 0.85;`
  - `pub use foley_dsp; pub use foley_synth;`

- [ ] **Step 1: Write the failing test**

```rust
// crates/foley-engine/tests/pins.rs
#[test]
fn pinned_foley_crates_link() {
    assert_eq!(foley_engine::foley_dsp::SAMPLE_RATE, 48_000);
    foley_engine::foley_synth::warm_voice_tables(); // Task 2 API exists at <PIN>
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p foley-engine --test pins`
Expected: FAIL (crate/paths don't exist yet).

- [ ] **Step 3: Implement the scaffold**

Workspace + engine crate skeleton + xtask wrapper + pins exactly per Interfaces. Engine `lib.rs` holds the four constants and the re-exports; nothing else yet.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p foley-engine --test pins`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: workspace scaffold; pin foley@<PIN> and nih-plug@de421011"
```

### Task 5: engine — `BedKind` / `BedUnion` / `kind_map`

**Repo:** plugin.

**Files:**
- Create: `crates/foley-engine/src/bed_union.rs`, `crates/foley-engine/src/kind_map.rs`
- Modify: `crates/foley-engine/src/lib.rs` (module decls + re-exports)
- Test: `crates/foley-engine/tests/bed_union.rs`

**Interfaces:**
- Consumes: foley bed types at `<PIN>` (constructors now `(seed, sample_rate)`-shaped, Task 1/2).
- Produces:
  - `#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum BedKind { Rain, Wind, Water, Fire, Lava, Thunder, Cave, Underwater, Biome, Surf }`
  - `pub enum BedUnion { Rain(foley_dsp::Rain), …, Surf(foley_synth::Surf) }` (one variant per bed)
  - `pub fn new_bed(kind: BedKind, seed: u64, sample_rate: u32) -> BedUnion`
  - `impl BedUnion`: `pub fn set_scene_knobs(&mut self, knobs: [f32; 4])`, `pub fn render(&mut self, out: &mut [f32])`, `pub fn strike(&mut self, base: u64)` (forwards to `Thunder::strike`; other variants no-op), `pub fn kind(&self) -> BedKind`
  - `pub fn knob_labels(kind: BedKind) -> [&'static str; 4]` — per spec §10 table, `""` for unused knobs (GUI labels)
  - `pub(crate) fn scene_from_knobs(kind: BedKind, knobs: [f32; 4])` in `kind_map.rs` — computes each bed's `set_scene` args per the spec §10 table (Rain: `rate = a·50` mm/h, `roof = None` if `b == 0.0` else `(preset_surface, (b·20).round())`, preset = 5-way quantize of `c`; Biome: index `= (a·6).round().min(6.0) as usize` via `BiomeKind::from_index`; `d` unused everywhere; Thunder's `celsius 20`/`humidity 60` fixed).

- [ ] **Step 1: Write the failing tests**

```rust
// crates/foley-engine/tests/bed_union.rs
#[test]
fn every_kind_renders_finite_at_48000() { /* for kind in BedKind::all():
    let mut b = new_bed(kind, 7, 48_000); b.set_scene_knobs([0.5;4]);
    let mut out = [0f32; 512]; b.render(&mut out);
    assert!(out.iter().all(|s| s.is_finite())); */ }
#[test]
fn strike_is_noop_except_thunder() { /* new_bed(Wind, ..).strike(1) must not
    panic; new_bed(Thunder, ..).strike(1) must not panic */ }
#[test]
fn knob_labels_match_spec_table() {
    assert_eq!(knob_labels(BedKind::Rain), ["rate mm/h", "roof height (blocks)", "surface preset", ""]);
    assert_eq!(knob_labels(BedKind::Cave)[2], "openness");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p foley-engine --test bed_union`
Expected: FAIL (types not defined).

- [ ] **Step 3: Implement**

`bed_union.rs` wraps the 10 beds; `kind_map.rs` holds `scene_from_knobs` + `knob_labels`. Enum dispatch only — no new audio logic.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p foley-engine`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/foley-engine
git commit -m "feat(engine): BedUnion wrapper + knob-to-scene mapping"
```

### Task 6: engine — spatial / guard / motion utilities

**Repo:** plugin.

**Files:**
- Create: `crates/foley-engine/src/spatial.rs`, `crates/foley-engine/src/guard.rs`, `crates/foley-engine/src/motion.rs`
- Modify: `crates/foley-engine/src/lib.rs`
- Test: `crates/foley-engine/tests/dsp_utils.rs`

**Interfaces:**
- Consumes: `foley_dsp::absorption_cutoff` (spec §10; exact name verified against the pinned rev — adapt the call to the real signature).
- Produces:
  - `pub fn pan_gains(az_deg: f32) -> (f32, f32)` — equal power: `x = (az_deg + 90.0) / 180.0`, `θ = x·π/2`, returns `(θ.cos(), θ.sin())`.
  - `pub struct OnePoleLpf` — `pub fn new() -> Self`, `pub fn reset(&mut self)`, `pub fn process(&mut self, x: f32, cutoff_hz: f32, sample_rate: f32) -> f32`.
  - `pub fn layer_cutoff(dist_blocks: f32, sample_rate: f32) -> f32` — `dist == 0` → `sample_rate / 2.0`; else min(nyquist, absorption cutoff for `dist_blocks`).
  - `pub struct GainGuard` — `pub fn new(ceiling: f32) -> Self`, `pub fn process(&mut self, x: f32) -> f32`, `pub fn reset(&mut self)` — envelope-follower gain guard; mechanism copied from foley-ffi `lib.rs:742-750` (fast attack, smooth release, `wanted = ceiling / env`).
  - `pub struct Motion` — `pub fn new(amt: f32, rate: f32, seed: u64) -> Self`, `pub fn next(&mut self, dt: f32) -> [f32; 4]` — seeded random-walk offsets, each component in `[-amt, amt]`, update cadence from `rate`.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/foley-engine/tests/dsp_utils.rs (assertion values pinned)
#[test] fn pan_gains_pins() {
    assert_eq!(pan_gains(-90.0), (1.0, 0.0));
    assert!((pan_gains(0.0).0 - 0.7071).abs() < 1e-3);
    assert_eq!(pan_gains(90.0), (0.0, 1.0));
    assert!((pan_gains(180.0).0 + 0.7071).abs() < 1e-3);
}
#[test] fn one_pole_passthrough_at_nyquist() { /* sine in → out ≈ in (< 1e-3 err)
    when cutoff == sample_rate */ }
#[test] fn one_pole_attenuates_low_cutoff() { /* 1 kHz sine, cutoff 100 Hz,
    48 kHz: settled amplitude < 0.2 */ }
#[test] fn layer_cutoff_pins() {
    assert_eq!(layer_cutoff(0.0, 48_000.0), 24_000.0);
    assert!(layer_cutoff(64.0, 48_000.0) < layer_cutoff(8.0, 48_000.0));
}
#[test] fn guard_caps_hot_signal() { /* ±2.0 constant into GainGuard::new(0.4):
    settled |out| <= 0.41 */ }
#[test] fn guard_is_transparent_when_quiet() { /* ±0.1 in → |out - in| < 0.01
    after settle */ }
#[test] fn motion_bounds_and_determinism() { /* amt=0 → zeros; amt=0.3 over
    10_000 next() calls → |offset| <= 0.3 + f32::EPSILON; two Motions with the
    same seed produce identical sequences */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p foley-engine --test dsp_utils`
Expected: FAIL (items not defined).

- [ ] **Step 3: Implement**

Straightforward DSP; `GainGuard` mirrors foley-ffi's mechanism (read it, don't import it — anti-goal §5 forbids depending on foley-ffi).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p foley-engine`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/foley-engine
git commit -m "feat(engine): equal-power pan, one-pole LPF, gain guards, motion walk"
```

### Task 7: engine — `seed_mix` / clip worker / rebuilder / `kit_map`

**Repo:** plugin.

**Files:**
- Create: `crates/foley-engine/src/seed_mix.rs`, `crates/foley-engine/src/clip.rs`, `crates/foley-engine/src/rebuild.rs`, `crates/foley-engine/src/kit_map.rs`
- Modify: `crates/foley-engine/src/lib.rs`
- Test: `crates/foley-engine/tests/seed_mix.rs`, `crates/foley-engine/tests/workers.rs`

**Interfaces:**
- Consumes: `BedUnion`/`new_bed` (Task 5); foley-synth one-shots with trailing `sample_rate` (Task 2).
- Produces:
  - `pub fn hit_seed(base: u64, note: u8, velocity: f32, counter: u32) -> u64` — splitmix64 mix of `base`, `note`, `velocity.to_bits()`, `counter` with distinct domain tags; **stable forever** (presets pin it).
  - `#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum SlotCategory { Contact, Fracture, Shot, Primitive, Voice, Note }`
  - `pub struct RenderJob { pub slot: u8, pub category: SlotCategory, pub kind: u32, pub voice_mob: u32, pub pitch: u8, pub knobs: [f32; 2], pub velocity: f32, pub seed: u64, pub sample_rate: u32 }`
  - `pub enum EngineJob { Render(RenderJob), Free(Vec<f32>) }`
  - `pub fn spawn_clip_worker(jobs: std::sync::mpsc::Receiver<EngineJob>, results: std::sync::mpsc::Sender<Vec<f32>>) -> std::thread::JoinHandle<()>` — thunder.rs pattern: worker keeps a **local free-list**; `Render` → pops a pooled `Vec` (or allocates) → `kit_map::render_clip` → `results.send`; `Free(vec)` → returns the buffer to the pool (never shrunk); exits when `jobs` disconnects.
  - `pub struct RebuildJob { pub kind: BedKind, pub seed: u64, pub sample_rate: u32 }`
  - `pub fn spawn_rebuilder(jobs: std::sync::mpsc::Receiver<RebuildJob>, out: std::sync::mpsc::Sender<BedUnion>) -> JoinHandle<()>` — recv → `new_bed` → `out.send` (cap-1 channel; worker may block on send — by design).
  - `kit_map.rs`: `pub fn render_clip(job: &RenderJob) -> Vec<f32>` — the single dispatch: Contact → `contact(Contact::from_index(kind % 4)?, Material::from_index(kind / 4)?, speed_law, seed, rate)`; Fracture → `fracture(Material::from_index(kind)?, …)`; Shot → match on the 19 named shots, kind 0..=18; Primitive → match `[bubble, noise_burst, tone, gust, aeolian, howl]` (pinned order), knobs A/B = first two float args; Voice → `voice(Mob::from_index(voice_mob)?, Call::from_index(kind)?, seed, rate)`; Note → `note(kind as instrument, job.pitch as f32, job.knobs[0] (character), seed, rate)`.
  - `pub fn kind_range(category: SlotCategory) -> u32` — max inclusive index: Contact 39, Fracture 9, Shot 18, Primitive 5, Voice 42 (call; mob separate), Note 15.
  - `pub fn kit_knob_labels(category: SlotCategory) -> [&'static str; 2]` — per spec §11.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/foley-engine/tests/seed_mix.rs
#[test] fn hit_seed_is_deterministic_and_field_sensitive() {
    assert_eq!(hit_seed(1, 60, 0.5, 0), hit_seed(1, 60, 0.5, 0));
    assert_ne!(hit_seed(1, 60, 0.5, 0), hit_seed(1, 60, 0.5, 1));
    assert_ne!(hit_seed(1, 60, 0.5, 0), hit_seed(1, 61, 0.5, 0));
    assert_ne!(hit_seed(1, 60, 0.5, 0), hit_seed(1, 60, 0.75, 0));
}

// crates/foley-engine/tests/workers.rs
#[test] fn clip_worker_roundtrip_reuses_buffers() {
    let (jtx, jrx) = std::sync::mpsc::sync_channel(32);
    let (rtx, rrx) = std::sync::mpsc::sync_channel(32);
    spawn_clip_worker(jrx, rtx);
    jtx.send(EngineJob::Render(job(Shot, /*bell*/ 0))).unwrap();
    let clip1 = rrx.recv().unwrap();
    assert!(!clip1.is_empty() && clip1.iter().all(|s| s.is_finite()));
    let cap = clip1.capacity();
    jtx.send(EngineJob::Free(clip1)).unwrap();
    jtx.send(EngineJob::Render(job(Shot, 0))).unwrap();
    let clip2 = rrx.recv().unwrap();
    assert!(clip2.capacity() >= cap.min(1) && cap > 0 || clip2.capacity() >= cap);
}
#[test] fn rebuilder_produces_requested_kind() {
    let (jtx, jrx) = std::sync::mpsc::sync_channel(1);
    let (otx, orx) = std::sync::mpsc::sync_channel(1);
    spawn_rebuilder(jrx, otx);
    jtx.send(RebuildJob { kind: BedKind::Rain, seed: 7, sample_rate: 48_000 }).unwrap();
    assert_eq!(orx.recv().unwrap().kind(), BedKind::Rain);
}
#[test] fn render_clip_smoke_every_category() { /* one representative per
    SlotCategory → non-empty, finite */ }
#[test] fn kind_range_pins() {
    assert_eq!(kind_range(SlotCategory::Shot), 18);
    assert_eq!(kind_range(SlotCategory::Note), 15);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p foley-engine --test seed_mix --test workers`
Expected: FAIL (items not defined).

- [ ] **Step 3: Implement**

Copy the worker skeleton from `crates/foley-synth/src/thunder.rs:262-322` (bounded channels, `try_recv`-free worker, `Job::Free` pool) adapted to these job types. `render_clip` uses `?`/`from_index` Option-guards everywhere — an out-of-range kind returns an empty `Vec`, never panics (spec §11 no-panic contract).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p foley-engine`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/foley-engine
git commit -m "feat(engine): hit_seed, clip worker + rebuilder, kit_map dispatch"
```

### Task 8: engine — `AmbienceRig`

**Repo:** plugin.

**Files:**
- Create: `crates/foley-engine/src/rig_ambience.rs`
- Modify: `crates/foley-engine/src/lib.rs`
- Test: `crates/foley-engine/tests/ambience_rig.rs`

**Interfaces:**
- Consumes: Tasks 5–7 (`BedUnion`, `RebuildJob`/`spawn_rebuilder`, `pan_gains`, `OnePoleLpf`, `layer_cutoff`, `GainGuard`, `Motion`).
- Produces:
  - `#[derive(Default)] pub struct LayerState { pub kind: Option<BedKind>, pub gain: f32, pub seed: u64, pub motion_amt: f32, pub motion_rate: f32, pub dist: f32, pub az: f32, pub knobs: [f32; 4] }`
  - `pub struct AmbienceRig`:
    - `pub fn new(sample_rate: u32) -> Self` — 6 `LayerState::default()` layers; owns the rebuilder channels (jobs cap **32**, out cap **1**, spec §9) + `JoinHandle`, per-layer scratch `[f32; MAX_BLOCK]`, `OnePoleLpf`, `GainGuard(LAYER_CEILING)`, `Motion`, knob smoothers; master `GainGuard(MASTER_CEILING)`.
    - `pub fn set_glide(&mut self, seconds: f32)` — knob smoothing time (spec default 0.30 s).
    - `pub fn layers(&mut self) -> &mut [LayerState; 6]` — the plugin writes post-smoothing-free raw values here every block.
    - `pub fn layer_kind(&self, layer: usize) -> Option<BedKind>` — currently active bed (for GUI + tests).
    - `pub fn set_output_gain(&mut self, linear: f32)` — applied before the master guard.
    - `pub fn strike(&mut self, layer: usize, base: u64)` — enqueue-only; RT-safe.
    - `pub fn process(&mut self, left: &mut [f32], right: &mut [f32])` — accepts any length ≤ any n; internally chunks at `MAX_BLOCK`. Per block per layer: detect `kind` change → `try_send(RebuildJob)` (full queue → starvation counter, retry next block); `try_recv` pending `BedUnion` → swap in + ~5 ms linear fade-in + guard reset; smooth the 4 knobs toward `LayerState.knobs` over `set_glide` (snap on first scene after a swap); add `Motion` offsets (clamped 0..=1); `bed.set_scene_knobs` → `bed.render(scratch)` → `OnePole` at `layer_cutoff(dist, rate)` → `pan_gains(az) × gain × GainGuard` → accumulate; master guard last.
  - `impl Drop for AmbienceRig` — drops the job sender, then joins the rebuilder handle. (A Thunder layer's own thread is joined by that bed's `Drop`/`shutdown`.)

- [ ] **Step 1: Write the failing tests**

```rust
// crates/foley-engine/tests/ambience_rig.rs
#[test] fn all_off_is_exact_silence() { /* rig, all layers Off; process 4096
    samples → every output sample == 0.0 */ }
#[test] fn single_layer_is_bounded_and_nonzero() { /* layer 0 Rain, gain 0.5,
    seed 7; process 2 s → finite, some |sample| > 0.01, peak <= 0.85*1.01 */ }
#[test] fn guard_caps_overdriven_mix() { /* all knobs 1.0, gain 1.5 → block
    peak <= MASTER_CEILING + 0.01 */ }
#[test] fn kind_swap_rebuilds_without_panic() { /* layer 0 Rain → process
    0.5 s → write kind = Wind → pump process until layer_kind(0) == Wind
    (bounded loop with progress yields) → output finite throughout */ }
#[test] fn strike_from_outside_process_is_safe() { /* layer 0 Thunder; call
    rig.strike(0, 42) then pump process 2 s → finite, no panic */ }
#[test] fn drop_joins_workers_promptly() {
    let (tx, rx) = std::sync::mpsc::channel();
    let h = std::thread::spawn(move || { let mut rig = AmbienceRig::new(48_000);
        rig.layers()[0].kind = Some(BedKind::Thunder); /* pump once */ drop(rig); tx.send(()).unwrap(); });
    assert!(rx.recv_timeout(std::time::Duration::from_secs(2)).is_ok());
    h.join().unwrap();
}
#[test] fn irregular_block_sizes_never_panic() { /* seeded PRNG lengths 1..=512,
    200 blocks at 44_100 and 96_000 → finite, no panic */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p foley-engine --test ambience_rig`
Expected: FAIL (`AmbienceRig` not defined).

- [ ] **Step 3: Implement**

Per Interfaces; the smoothing, swap-fade, and guard orchestration is the one nontrivial body — follow the spec §9 bullet list in order.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p foley-engine`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/foley-engine
git commit -m "feat(engine): AmbienceRig — layers, glide, guards, swap-fade, strike"
```

### Task 9: engine — `KitRig`

**Repo:** plugin.

**Files:**
- Create: `crates/foley-engine/src/rig_kit.rs`
- Modify: `crates/foley-engine/src/lib.rs`
- Test: `crates/foley-engine/tests/kit_rig.rs`

**Interfaces:**
- Consumes: Tasks 6–7 (`pan_gains`, `GainGuard`, `hit_seed`, `EngineJob`/`spawn_clip_worker`, `kind_range`).
- Produces:
  - `#[derive(Clone, Copy)] pub struct SlotState { pub category: SlotCategory, pub kind: u32, pub voice_mob: u32, pub note: u8, pub knobs: [f32; 2], pub gain: f32, pub pan: f32, pub seed: u64, pub fresh: bool }` (kind/voice_mob clamped via `kind_range` at use; `pan` −100..=100 mapped to azimuth `pan·0.9` for `pan_gains`)
  - `pub struct KitRig`:
    - `pub fn new(sample_rate: u32) -> Self` — 8 default slots; owns clip channels (jobs cap 32, results cap `MAX_VOICES * 2`) + `spawn_clip_worker` handle; voice pool; master `GainGuard(MASTER_CEILING)`.
    - `pub fn slots(&mut self) -> &mut [SlotState; 8]`
    - `pub fn note_on(&mut self, note: u8, velocity: f32)` — for **every** slot whose `note` matches: per-slot hit counter += 1; `seed = if fresh { hit_seed(slot.seed, note, velocity, counter) } else { slot.seed }`; build `RenderJob` → `try_send` (full → starvation counter, hit dropped).
    - `pub fn set_polyphony(&mut self, n: usize)` — clamp to {4, 8, 16}.
    - `pub fn set_humanize(&mut self, h: f32)` — per-hit gain multiplier `1.0 − 0.5·h·frac(hit_seed >> 11)` (deterministic per hit).
    - `pub fn set_output_gain(&mut self, linear: f32)`
    - `pub fn active_voices(&self) -> usize`
    - `pub fn process(&mut self, left: &mut [f32], right: &mut [f32])` — `try_recv` finished clips (FIFO-activate pending voices, oldest first; at the polyphony cap the **oldest** playing voice is stolen); per active voice: read the clip region with a 2 ms linear fade-in and 10 ms linear fade-out, pan, gain, humanize jitter, accumulate; master guard; drop the exhausted clip back to the worker via `EngineJob::Free`.
  - `impl Drop` — drop job sender, join worker.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/foley-engine/tests/kit_rig.rs
#[test] fn unmatched_note_is_exact_silence() { /* slot 0 bound to note 60;
    note_on(61, 1.0) + pump → all zeros */ }
#[test] fn polyphony_cap_steals() { /* set_polyphony(4); note_on ×6 across
    6 slots bound to the same note; pump process until clips arrive;
    active_voices() <= 4, no panic */ }
#[test] fn fresh_slots_vary_per_hit() { /* fresh = true; two note_ons on one
    slot produce two RenderJobs — observable via two distinct clips arriving
    on process (both finite) */ }
#[test] fn voice_tail_ends_in_silence() { /* one hit, pump until
    active_voices() == 0, then one more block → exact 0.0 tail */ }
#[test] fn drop_joins_worker_promptly() { /* same watchdog pattern as Task 8 */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p foley-engine --test kit_rig`
Expected: FAIL (`KitRig` not defined).

- [ ] **Step 3: Implement**

Voice pool as `Vec<Voice>` with states `Pending(slot) / Playing { clip, pos, … } / Free`; FIFO clip→voice pairing; `Free` the finished `Vec` on the audio thread via `try_send(EngineJob::Free(vec))` (drop the buffer if the queue is full — worker pool refills).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p foley-engine`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/foley-engine
git commit -m "feat(engine): KitRig — voice pool, FIFO clip pickup, steal-oldest"
```

### Task 10: foley-ambience plugin crate

**Repo:** plugin.

**Files:**
- Create: `crates/foley-ambience/Cargo.toml` (deps: `nih_plug`, `foley-engine`)
- Create: `crates/foley-ambience/src/params.rs`, `crates/foley-ambience/src/lib.rs`
- Test: unit tests in `crates/foley-ambience/src/params.rs`

**Interfaces:**
- Consumes: `AmbienceRig`/`LayerState` (Task 8), `BedKind` (Task 5).
- Produces:
  - `pub struct Ambience;` implementing `nih_plug::plugin::Plugin`: `NAME = "Foley Ambience"`; `AUDIO_IO_LAYOUTS = [AudioIOLayout { main_input_channels: None, main_output_channels: NonZeroU32::new(2), .. } ]`; `MIDI_INPUT = MidiConfig::Basic`, `MIDI_OUTPUT = None`; `HARD_REALTIME_ONLY = false`; `SAMPLE_ACCURATE_AUTOMATION = false`; `process` always returns `ProcessStatus::KeepAlive`.
  - `params.rs`: `pub struct AmbienceParams` with `#[derive(Params)]` — `#[id = "output_gain"] output_gain: FloatParam` (−24..=+6 dB, default 0.0), `#[id = "scene_glide"] scene_glide: FloatParam` (0.02..=1.0, default 0.30), and six layer structs generated by a `macro_rules!` (`Layer1Params`…`Layer6Params`, `#[nested(group = "Layer N")]`) so every id is the **exact literal** `l{n}_kind`, `l{n}_gain`, `l{n}_seed`, `l{n}_motion_amt`, `l{n}_motion_rate`, `l{n}_dist`, `l{n}_az`, `l{n}_knob_a`…`l{n}_knob_d`. Per spec §10: `kind` = `EnumParam<LayerKind>` (11 variants in the pinned order), `gain` 0..=1.5 default 0.5, `seed` IntParam 0..=16_777_215 default per index, `motion_amt` 0..=1 default 0.15, `motion_rate` 0..=1, `dist` 0..=64 default 0.0, `az` −180..=180 default 0, knobs 0..=1 default 0.5.
  - `LayerKind` (plugin-local enum with `Off` first) ↔ `Option<BedKind>` conversion.
  - `process`: drain and ignore events (`while context.next_event().is_some() {}`); track per-layer previous raw `knob_b` value; rising edge on a Thunder layer → `rig.strike(layer, base)` with the frozen derivation `base = hit_seed(layer seed, az scaled to u8, dist/64.0, per-layer strike counter)`; write `rig.layers()[i]` from params (no smoothing here — the rig smooths); `rig.set_glide(scene_glide)`, `rig.set_output_gain(db_to_linear(output_gain))`; chunk the output buffer at `MAX_BLOCK` into `rig.process`.
  - `initialize`: `foley_synth::warm_voice_tables()` then one `howl(seed, sample_rate)` render off-thread (spec §9 pre-warm; the howl call is allowed to run inside `initialize` since it is not the audio thread).
  - Exports: `nih_export_clap!(Ambience); nih_export_vst3!(Ambience);` + `impl ClapPlugin { const CLAP_ID: &str = "io.github.cumsauce.foley.ambience"; const CLAP_FEATURES: &[ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Synthesizer]; }` + `impl Vst3Plugin { const VST3_CLASS_ID: [u8; 16] = *b"F0leyAmbience.01"; const VST3_SUBCATEGORIES: &[Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth]; }` (editor comes in Task 12; `editor()` may return `None` for now).

- [ ] **Step 1: Write the failing tests** (in `params.rs`)

```rust
#[test] fn layer_kind_conversion_is_total() { /* every LayerKind variant maps
    to/from Option<BedKind>; Off → None */ }
#[test] fn defaults_match_spec() { /* AmbienceParams::default(): output_gain 0.0,
    scene_glide 0.30, layer gain 0.5, motion_amt 0.15, dist 0.0, az 0.0,
    knobs 0.5; layer 1 kind Off */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p foley-ambience`
Expected: FAIL (crate doesn't exist).

- [ ] **Step 3: Implement**

Params per the verbatim id list; process per Interfaces. The `macro_rules!` generation is what guarantees the exact `l{n}_*` ids — do not rely on nih nested-array id synthesis.

- [ ] **Step 4: Run tests to verify they pass + bundle builds**

Run: `cargo test -p foley-ambience && cargo xtask bundle foley-ambience`
Expected: tests PASS; bundle step succeeds and writes `target/bundled/Foley Ambience.clap` and `.vst3`.

- [ ] **Step 5: Commit**

```bash
git add crates/foley-ambience
git commit -m "feat(ambience): Foley Ambience plugin — 68 params, KeepAlive process"
```

### Task 11: foley-kit plugin crate

**Repo:** plugin.

**Files:**
- Create: `crates/foley-kit/Cargo.toml` (deps: `nih_plug`, `foley-engine`)
- Create: `crates/foley-kit/src/params.rs`, `crates/foley-kit/src/lib.rs`
- Test: unit tests in `crates/foley-kit/src/params.rs`

**Interfaces:**
- Consumes: `KitRig`/`SlotState` (Task 9), `SlotCategory`/`kind_range` (Task 7).
- Produces:
  - `pub struct Kit;` implementing `Plugin`: `NAME = "Foley Kit"`; `AUDIO_IO_LAYOUTS` stereo-out/no-in; `MIDI_INPUT = MidiConfig::Basic`; `HARD_REALTIME_ONLY = false`; `SAMPLE_ACCURATE_AUTOMATION = false`; `ProcessStatus::KeepAlive`.
  - `params.rs`: `KitParams` — master `#[id = "output_gain"]` (−24..=+6 dB, 0.0), `#[id = "polyphony"]` `EnumParam<Polyphony>` with `#[derive(Enum, Clone, Copy, PartialEq)] enum Polyphony { V4, V8, V16 }` (default V8), `#[id = "humanize"]` FloatParam 0..=1 default 0.0; eight slot structs via `macro_rules!` (`Slot1Params`…`Slot8Params`) with literal ids `s{n}_category` (`EnumParam<SlotKind>` over the 6 `SlotCategory` variants), `s{n}_kind` IntParam 0..=62 default 0, `s{n}_voice_mob` IntParam 0..=51 default 0, `s{n}_note` IntParam 0..=127 default 60, `s{n}_knob_a`/`s{n}_knob_b` 0..=1 default 0.5, `s{n}_gain` 0..=1.5 default 0.8, `s{n}_pan` −100..=100 default 0, `s{n}_seed` IntParam 0..=16_777_215 default per index, `s{n}_fresh` BoolParam default true.
  - `process`: `while let Some(event) = context.next_event()` — `NoteOn { note, velocity, .. }` → `rig.note_on(note, velocity)`; `NoteOff` ignored (spec §11); copy slots into the rig; `set_polyphony`/`set_humanize`/`set_output_gain`; chunk at `MAX_BLOCK` → `rig.process`.
  - `initialize`: `warm_voice_tables()` + one worker warm-up render (spec §9).
  - Exports: `nih_export_clap!(Kit); nih_export_vst3!(Kit);` + `CLAP_ID = "io.github.cumsauce.foley.kit"`, `CLAP_FEATURES = &[Instrument, Synthesizer]`, `VST3_CLASS_ID = *b"F0leyKit.0100001"`, `VST3_SUBCATEGORIES = &[Instrument, Synth]`.

- [ ] **Step 1: Write the failing tests** (in `params.rs`)

```rust
#[test] fn defaults_match_spec() { /* KitParams::default(): note 60, gain 0.8,
    fresh true, polyphony V8, humanize 0.0, output_gain 0.0 */ }
#[test] fn polyphony_enum_maps_to_counts() { /* V4→4, V8→8, V16→16 */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p foley-kit`
Expected: FAIL (crate doesn't exist).

- [ ] **Step 3: Implement**

Same shape as Task 10; MIDI is the only event source.

- [ ] **Step 4: Run tests to verify they pass + bundle builds**

Run: `cargo test -p foley-kit && cargo xtask bundle foley-kit`
Expected: PASS + bundles under `target/bundled/`.

- [ ] **Step 5: Commit**

```bash
git add crates/foley-kit
git commit -m "feat(kit): Foley Kit plugin — 83 params, MIDI one-shot trigger"
```

### Task 12: egui editors + preset save/load (both plugins)

**Repo:** plugin.

**Files:**
- Create: `crates/foley-ambience/src/editor.rs`, `crates/foley-kit/src/editor.rs`
- Modify: both `Cargo.toml` (+ `nih_plug_egui`, `rfd`), both `src/lib.rs` (implement `Plugin::editor`, `#[persist = "editor-state"] Arc<EguiState>`)

**Interfaces:**
- Consumes: `knob_labels` (Task 5), `kit_knob_labels` (Task 7), param structs (Tasks 10–11).
- Produces: `fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>>` via `nih_plug_egui::create_egui_editor(egui_state, (), |_, _| {}, move |ctx, setter, state| { … })`. Widget inventory per spec §15 (fixed):
  - Ambience: `ScrollArea`, one section per layer — `ParamSlider` per param with `knob_labels(kind)`-derived captions; Thunder layers get a momentary `strike` button that does `setter.begin_set_parameter(&knob_b) / setter.set_parameter(&knob_b, 1.0)` on press and `… 0.0` on release (the plugin's rising-edge detector does the rest).
  - Kit: one section per slot — `ParamSlider` per param with `kit_knob_labels(category)` captions.
  - Both: `Save…`/`Load…` buttons — `rfd::FileDialog` for the path; Save serializes the full plugin state (see the framework's gain example for the exact `GuiContext::get_state()` plumbing at the pinned rev) to JSON; Load reads it and `set_state`. No meters/LEDs/keyboard widgets (spec §15).

- [ ] **Step 1: Write the failing test**

There is no headless test for pixels; the failing gate is the build with the editor wired:

Run: `cargo clippy -p foley-kit -p foley-ambience -- -D warnings`
Expected: FAIL (editor not implemented / unused persist field).

- [ ] **Step 2: Implement both editors**

Per Interfaces; follow `nih_plug_egui`'s example for `create_egui_editor` argument order and `widgets::ParamSlider::for_param(&param, setter)`.

- [ ] **Step 3: Verify**

Run: `cargo clippy -p foley-kit -p foley-ambience -- -D warnings && cargo xtask bundle foley-kit foley-ambience`
Expected: clean; bundles rebuilt.

- [ ] **Step 4: Commit**

```bash
git add crates/foley-kit crates/foley-ambience Cargo.lock
git commit -m "feat(gui): egui editors with param racks, strike button, preset save/load"
```

### Task 13: RT-alloc probe, render bench, CI, release docs

**Repo:** plugin.

**Files:**
- Create: `crates/foley-engine/tests/rt_alloc.rs`, `crates/foley-engine/tests/render_bench.rs`
- Create: `.github/workflows/ci.yml`, `README.md`

**Interfaces:**
- Consumes: `AmbienceRig`, `KitRig` (Tasks 8–9).
- Produces: CI badge-worthy workflow; allocation-proof and bench test binaries; README with build/bundle/Gatekeeper notes.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/foley-engine/tests/rt_alloc.rs — thread-local counting global allocator
// mirroring crates/foley-ffi/src/no_alloc.rs (read it first); counts only the
// calling thread so worker threads may allocate freely.
#[test] fn ambience_rig_allocates_zero_after_warmup() { /* for rate in
    [44_100, 48_000, 96_000]: build rig, one warm-up block, reset counter,
    200 blocks of seeded irregular sizes 1..=512 with one kind swap and one
    strike mid-run → assert count == 0 */ }
#[test] fn kit_rig_allocates_zero_on_note_storm() { /* note_on ×32 across
    slots while pumping process at 48_000 → audio-thread allocations == 0 */ }
#[test] fn two_instances_share_tables_safely() { /* two AmbienceRigs (and one
    KitRig) in one test, rendered interleaved → all finite, no panic (Review
    Focus: two instances) */ }

// crates/foley-engine/tests/render_bench.rs — #[ignore]d, prints (never asserts
// timing, spec §15): renders one representative clip per SlotCategory at 48 kHz
// and prints duration + equivalent-latency ms.
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p foley-engine --test rt_alloc`
Expected: FAIL (harness not present) — or PASS-by-luck of the counting allocator; the test is the permanent gate either way.

- [ ] **Step 3: Implement the probe + bench**

Mirror foley-ffi's `no_alloc.rs` thread-local accounting. Bench prints only.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p foley-engine --test rt_alloc`
Expected: PASS (zero audio-thread allocations).

- [ ] **Step 5: Add CI + README**

`.github/workflows/ci.yml`: (a) `ubuntu-latest` — `cargo clippy --workspace -- -D warnings` + `cargo test --workspace`; (b) `macos-latest` — `cargo xtask bundle-universal foley-kit foley-ambience --release`, upload `target/bundled/` artifacts. Note in the workflow comments: if the foley repo is private, cargo needs a read-scoped PAT (e.g. `NETRC`/`insteadOf` setup) — left as an author decision. `README.md`: build (`cargo xtask bundle …`), install paths (`~/Library/CLAP`, `~/Library/Audio/Plug-Ins/VST3`), the unsigned-binary Gatekeeper note, Windows deferred (spec §14).

- [ ] **Step 6: Full gate + commit**

Run: `cargo clippy --workspace -- -D warnings && cargo test --workspace && cargo xtask bundle-universal foley-kit foley-ambience --release`
Expected: all green; universal bundles in `target/bundled/`.

```bash
git add -A
git commit -m "ci: rt-alloc probe, render bench, bundle CI, README"
```

---

## Execution notes

- Tasks 1–3 are sequential and gated by the mod battery; `<PIN>` from Task 3 unblocks Task 4. Tasks 5–7 can proceed in parallel after Task 4; Tasks 8–9 depend on 5–7; Tasks 10–11 on 8–9; Task 12 on 10–11; Task 13 last.
- Every commit in the foley repo must keep the working tree's unrelated in-flight work untouched (`git add -p` discipline).
- `<PIN>` is substituted as a literal SHA everywhere it appears (workspace deps + commit messages).
