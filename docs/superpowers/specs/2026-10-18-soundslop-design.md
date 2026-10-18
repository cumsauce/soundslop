# Foley → standalone CLAP/VST3 plugins: design

**Date:** 2026-10-18 · **Status:** draft, for author review · **Upstream:** `github.com/cumsauce/foley` (consumed at a pinned rev, never forked)

> Author's purpose, verbatim:
> *"so i can use in a daw ... i want to be able to use the individual sounds as well as create
> soundscapes with the synths we've made procedurally. since theyre rust im wondering if i can
> keep the mod as is but still use the crate as a lib in my clap synth"*

## 1. Intent and why

The foley mod's synthesis is a dependency-clean, pure-Rust DSP stack (`foley-math` →
`foley-dsp` → `foley-synth`): ten continuous ambience "beds", ~30 one-shot synthesis
functions (contacts, note-block instruments, 52 mobs × 43 calls, and 19 named shots), all
seeded and deterministic. None of it depends on Minecraft, Java, FFI, or audio I/O — the
only game-shaped piece is the C-ABI shim `foley-ffi`. A standalone DAW plugin can therefore
link the DSP crates **directly** and expose two musical surfaces:

1. **Soundslop Kit** — a MIDI instrument that plays the one-shots (impacts, drips, bells,
   bird/cricket/frog/bug sounds, mob calls, note-block instruments) with per-hit seed
   variation, like a sampler of procedurally generated field recordings.
2. **Soundslop Ambience** — an always-running generative soundscape: up to 6 layers, each an
   assignable bed (Rain/Wind/Water/Fire/Lava/Cave/Underwater/Thunder/Biome/Surf) with
   spatial placement (azimuth + distance with air absorption), per-layer scene "macro"
   knobs, automatable everything, and a slow random-walk "motion" so scenes evolve.

Both export **CLAP and VST3** from one crate each via NIH-plug, share one engine crate,
and save soundscapes/kits as ordinary plugin state (DAW presets + save/load-to-file
buttons in the GUI).

The mod "stays as is": `mod/`, the Java API, and every exported symbol of `foley-ffi`
keep working exactly as today. Where a shared fix is worth it (sample rate), it happens
as a **behavior-preserving change inside the foley workspace's DSP crates only**, gated by
the foley repo's own test battery.

## 2. What this design verified (and what the original brief got wrong)

Everything below was verified by reading the code at foley's current HEAD (`5020db4`,
tree dirty with in-flight ABI-25/BOOM work in `foley-ffi/src/lib.rs`,
`foley-synth/src/{lib.rs,oneshot.rs}`).

**Confirmed claims:**

- Workspace: `resolver = "3"`, `edition = "2024"`, `rust-version = "1.99"`,
  `MIT OR Apache-2.0`, `publish = false`, `unsafe_code = "deny"` at
  `[workspace.lints.rust]` (opted into per-crate via `[lints] workspace = true`).
  `foley-ffi` is the only `cdylib + rlib`; `foley-sim` is a plain bin.
- **Zero external dependencies** in `foley-math` / `foley-dsp` / `foley-synth`.
  `foley-dsp` is a real `#![no_std]` crate (`lib.rs:6`) with no `extern crate alloc`;
  all 17 of its `Vec` hits live under `#[cfg(test)]`. `foley-synth` is a std crate:
  one-shots allocate, and it has exactly the std hits listed below — no
  fs/net/env/process, no `include_bytes!`/`include_str!`, no `unsafe` anywhere in the
  trio.
- `SAMPLE_RATE` is a single hard const: `crates/foley-dsp/src/lib.rs:40
  pub const SAMPLE_RATE: u32 = 48_000;` — re-exported by `foley-synth/src/lib.rs:23`.
  ≈ **309 use sites** (60 in foley-dsp, 249 in foley-synth). No function or constructor
  takes a sample rate today; each body does `let rate = SAMPLE_RATE as f32;`.
  Only **four** uses are genuine const contexts (compile-time): `wind.rs:47 LEAD`
  (sizes a `[f32; LEAD]` field), `fire.rs:84 TICK_SAMPLES`, `outdoors.rs:11 SURF_LEN`,
  `outdoors.rs:14 KNOCK_LEN`. All other sites are runtime casts
  (`let rate = SAMPLE_RATE as f32;`) inside function bodies; the refactor shape is
  described under §7.
- Beds, confirmed inventory: `Rain, Water, Fire, Lava, Cave, Underwater, Wind` in
  `foley-dsp`; `Thunder, Biome, Surf` in `foley-synth`. Uniform API except: `Thunder::new()`
  takes no seed. `set_scene(...)` + `render(&mut self, out: &mut [f32])`.
- One-shots confirmed: `contact(Contact, Material, speed: f32, seed: u64) -> Vec<f32>`
  (`modal.rs:356`), `contact_with(..., &MaterialProps, ...)` (custom material — the
  mod's "F1" path), `fracture`/`fracture_with` (`modal.rs:548`),
  `note(instrument: u32, note: f32, character: f32, seed: u64) -> Option<Vec<f32>>`
  (`note.rs:180`, 16 instruments, semantic `note` = MIDI note number, length capped 3.0 s),
  `voice(Mob, Call, seed: u64) -> Vec<f32>` (`voice/mod.rs:344`),
  `creature_voice(&mut Rng, &CreatureSpec, CreatureCall)` (`voice/creature.rs:45`), plus
  the named shots: `bell`, `crickets`, `birdsong`, `bird_call`, `birdsong2`, `frog`,
  `drip_into_water`, `drip_on_stone`, `lava_drip`, `bang`, `sculk_ring`, `brush`,
  `ice_singing`, `settling_stone`, `flush`, `buzz`, `woodpecker`, `owl`, `cicadas`,
  `surf`, `bubble`, `noise_burst`, `tone`, `gust`, `aeolian`, `howl`. Enum counts
  verified: `Mob` 52, `Call` 43, `Material` 10, `Contact` 4, `Surface` 7,
  `BiomeKind` 7, `Bird` 9, `Birds` 57, note-block instruments 16. All `from_index`
  constructors return `Option` (no panic); birds `of()` **wraps** mod len.
- foley-ffi policy, confirmed: `GLIDE_BLOCK = 128` / `GLIDE_TIME = 0.3 s`
  (`lib.rs:269-270`), exponential per-parameter glide applied in 128-frame chunks, with
  snap on the first scene; `BED_CEILING = 0.4` applied as an envelope-follower gain guard
  per bed (`lib.rs:742`); `SUBMERGED` is a **process-global `AtomicBool`** mixing a 1 kHz
  LPF onto every non-underwater bed (`lib.rs:284, 731-754`); output is mono i16 via
  `to_pcm` = `(sample.tanh() * i16::MAX as f32) as i16` (`foley-dsp/src/lib.rs:43-45`).
  The render path is allocation-free for any frames and proven by a counting
  `#[global_allocator]` test (`no_alloc.rs`) plus RealtimeSanitizer tests in CI.
- `Thunder` internals: `thunder.rs:268` spawns a named OS thread `foley-thunder`
  (claimed line 262 was near it, actual 268); two **bounded** `sync_channel`s (job cap
  `2*MAX_STRIKES`, result cap `MAX_STRIKES`), worker polls `try_recv()` + `sleep(1ms)`,
  never blocks; `strike(base, seed, celsius, humidity)` is a **pure free function**;
  `Thunder::strike()` only enqueues; there is **no global/static state** in Thunder, so
  two instances are safe; the thread's lifetime is tied to the bed via the job-sender
  (worker exits on channel disconnect — see §9 for what that means).
- `lf.rs`: the global is at `lf.rs:104` (accessor at 103): `static TABLES:
  OnceLock<Vec<Pulse>>`, built on first `lookup`, ≈ 205 KB, private, no warm-up fn today;
  only the voiced path (`voice/score.rs:341,347`) touches it.
- Internal scene smoothing per bed (relevant to the plugin's glide design): **only**
  `Water` (0.4 s), `Underwater` (`dt/0.3`), and `Cave`'s `openness` (0.1 s) glide
  internally; `Rain`, `Fire`, `Lava`, `Wind`, `Biome`, `Thunder` jump abruptly.
- foley-sim (non-game host): renders scenes to 48 kHz mono WAV at a 20 Hz tick with
  `foley_bed_set_scene`/`foley_bed_render` on a stream-thread-like loop; `bed_bench.rs`
  drives `Rain/Wind/Water/Fire/Lava` with plain `f32` buffers and prints a per-render
  timing table.
- Mod side: `NativeBridge.SAMPLE_RATE = 48_000` (`natives/NativeBridge.java:29`),
  `BedSound` plays 48 kHz mono 16-bit; **no 44100 anywhere in `mod/`**; loader policy:
  "It is never unloaded, because it owns live threads."

**Corrections to the brief:**

| Brief said | Code says |
|---|---|
| 18 shot kinds | **19** (`oneshot BOOM = 18` exists in `foley-ffi`'s `n_*` registry; the checked-in `foley.h` is stale at version 24 and missing BOOM — the working tree moves it to 25) |
| "18 surfaces" | `Surface` is the 7-variant rain enum (Water, Stone, Leaves, Wood, Soft, Glass, Metal); 18/19 was likely a conflation with shot kinds |
| `lf.rs:103` is the OnceLock | the static is at `lf.rs:104`; 103 is the private `fn tables()` |
| "Remove Thunder's long-lived thread" is the fix | see §7: the thread is per-instance, exits on drop, and **removal is not needed**; what *is* needed is a deterministic-exit guarantee (test) — the "never unloads" policy is the mod loader's own choice |
| (implied) mod tests would catch a sample-rate regression | the Java-side tests never pin the rate; the real gates are `cargo test --workspace`, `xtask levels` (det.), header check, natives/dist builds |

**NIH-plug facts** (pinned rev `de421011f41a6d10fc8c7a6084e4f4dee0143683`, current master,
2026-09-03; framework in maintenance mode → the published alternative is the BillyDM fork
on Codeberg; not on crates.io, git dep required):

- `Plugin` trait today: `const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout]` (replaces the
  old buses config), `initialize(&mut self, &AudioIOLayout, &BufferConfig, &mut impl
  InitContext<Self>) -> bool`, `process(&mut self, &mut Buffer, &mut AuxiliaryBuffers,
  &mut impl ProcessContext<Self>) -> ProcessStatus`, `params() -> Arc<dyn Params>`,
  `fn editor(&mut self, async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>>`.
- `BufferConfig.sample_rate` is **`f32`**; hosts may hand variable block sizes
  (`min_buffer_size: Option<u32>`, `max_buffer_size: u32`).
- Events: drained per sample via `context.next_event() -> Option<NoteEvent<...>>`;
  `NoteEvent::NoteOn { timing, voice_id, channel, note, velocity, .. }`.
- Params: `#[derive(Params)]`, fields registered only with `#[id = "stable"]`; nested
  structs via `#[nested]`/`#[nested(array, group = "...")]`; extra persistent state via
  `#[persist = "key"]` implementing `PersistentField` (serde JSON behind it).
- State: host save/load auto-serializes params (unnormalized) + persist fields; UI
  presets via `GuiContext::get_state() -> PluginState` / `set_state(state)`.
- Off-thread work: `InitContext::execute(task)` (blocking, init only),
  `ProcessContext::execute_background(task)`, and `task_executor()` backed by a real
  background thread in the **CLAP** wrapper (the VST3 wrapper's background-thread story
  is unverified — do not rely on it for audio-timing).
- Categories: `impl Vst3Plugin { const VST3_SUBCATEGORIES: &[Vst3SubCategory] }` and
  `impl ClapPlugin { const CLAP_FEATURES: &[ClapFeature] }` — declared explicitly, never
  inferred from MIDI config. `MidiConfig::Basic` = notes only (no `MidiCCs` VST3 param
  bloat).
- Statuses: `ProcessStatus::KeepAlive` (~infinite tail; correct for always-running
  generative plugins); `HARD_REALTIME_ONLY` stays `false` so offline bounce works.
- GUI: `nih_plug_egui` (egui 0.31, last touched 2025-02 — the freshest of the three
  integrations; `create_egui_editor(egui_state, (), |_,_|{}, move |ctx, setter, _| {...})`,
  `widgets::ParamSlider::for_param(&param, setter)`, `ResizableWindow`, editor
  geometry persisted via `#[persist = "editor-state"] Arc<EguiState>`). `nih_plug_vizia`
  is pinned to a 2024 fork tag; `nih_plug_iced` is older still.
- Build: NOT on crates.io; the repo's own `xtask/` is reusable as the `nih_plug_xtask`
  library: new repo gets `xtask/src/main.rs = nih_plug_xtask::main();`
  `cargo xtask bundle <pkg> [--release]` and macOS universal via `cargo xtask
  bundle-universal`; output under `target/bundled/` with VST3/CLAP bundle plumbing
  (macOS Info.plist/PkgInfo generated; CLAP_ID/VST3_CLASS_ID/VST3_SUBCATEGORIES from the
  `impl`).
- License: framework is ISC, but `nih_export_vst3!` pulls GPLv3-licensed
  `vst3-sys` bindings — VST3 builds of this plugin must be GPL-compliant; the plugin
  repo should carry `GPL-3.0-or-later` (default) with the CLAP path's code remaining
  permissively licensed if dual-stacked later.

## 3. Goals

1. **Kit**: play every foley one-shot from MIDI — contacts (Tap/Place/Land/Step × 10
   materials), fractures, the 19 shots, primitives, note-block instruments (16, pitched),
   and mob calls (52 mobs × 43 calls) — with per-hit seed variation.
2. **Ambience**: build soundscapes from up to 6 layers of beds with mixing, spatial
   placement, scene-automation, and time evolution — meaning the per-layer motion
   random-walk (§10) plus Kit humanize (§11). No separate "memory" behavior exists.
3. **Presets**: full state recall via DAW plugin state and in-GUI save/load buttons.
4. Both formats from one crate each; one repo, one engine; macOS universal release.
5. Host-rate-native rendering (any host sample rate, **including 44.1 and 96 kHz**) with
   no internal resampler and zero reported latency.
6. Preserve the crates' realtime discipline: nothing allocates on the audio thread after
   init; CI proves it.

## 4. Non-goals

- **Any change to `mod/`, `foley-ffi`'s exported symbols/ABI, or the Java API.** The
  plugin never links `foley-ffi`.
- Publishing DSP crates to crates.io (see §6 for the chosen sharing mechanism and this
  omission's justification).
- Custom-material parameter editing ("F1" 9-float `MaterialProps`) — happy path is
  `Material::ALL` presets; `contact_with` is reserved for a later version.
- MIDI CC automation, MPE, poly-modulation (items are `MidiConfig::Basic`).
- Runtime sample import/export, wavetables, drag-drop samples — the plugin is purely
  procedural.
- Surround/HRTF; stereo-out only with equal-power panning (the crates are mono).
- Re-implementing or "improving" synthesis; the engine crates are the sound.
- AAX, AU, iOS/Android, web — and no shipping a Windows binary in v1 (build story listed
  as a follow-up; see §14).

## 5. Anti-goals (failure even if it technically works)

- **Breaking the mod.** Any behavioral change to the sound mod users hear at 48 kHz is a
  failure — hence 48 kHz-default bit-compatible refactor + the foley repo's own gates as
  hard task acceptances.
- **Forking the DSP.** Silent divergence between mod and plugin builds costs more than
  any adapter code.
- **Chipping every hole into the parameter list.** No bed gets every scene field (e.g.
  Rain gets 1 macro for its 7 surface weights); macro couplings are defined once in
  `foley-engine` and stay stable.
- **Inheriting game-shaped policy.** Do not transplant: the global `SUBMERGED` LPF
  (per-layer `Underwater` + per-layer distance LPF already cover underwater; user can
  filter after us), the i16 conversion (host takes f32), and the hardcoded bed ceiling
  as an invisible constant. The *sound-design* numbers (0.3 s glide feel, 0.4-layer
  ceiling) reappear as plugin-defined constants/params, not as imports from foley-ffi.
- **Shipping dead switches.** Every param must drive audible sound or is cut from v1.

## 6. Constraints

1. **Consumption:** git dependency pinned by rev on `https://github.com/cumsauce/foley`
   for `foley-math`/`foley-dsp`/`foley-synth`; local `[patch."..."]` blocks redirect to a
   sibling checkout for development; **no crates.io publishing** in scope. Concretely:
   - All workspace deps: `rev = "<PIN>"` where `<PIN>` is the merged post-Option-A HEAD
     on foley's `master` (the plan's Phase 0 produces that commit; the pin task uses its
     exact SHA).
   - CI: the foley repo is public — git deps clone without credentials, so CI needs no
     PAT; local patching keeps dev friction at zero.
   - Phase 0 (the Option-A refactor) lands as a branch off foley `master`. The tree at
     `5020db4` is dirty with in-flight ABI-25/BOOM work in `foley-ffi/src/lib.rs` and
     `foley-synth/src/{lib.rs,oneshot.rs}` — Phase 0 must not clobber it: if that work
     is still uncommitted, rebase onto it (or coordinate with the author) before
     pinning, and stage only the rate-refactor hunks when committing.
2. **Toolchain floors:** Rust `rust-version = "1.99"`, `edition = "2024"`,
   `resolver = "3"` — mirroring foley. `unsafe_code = "deny"` everywhere. Recommend
   CI runs `cargo clippy -D warnings`.
3. **NIH-plug pin:** `nih_plug` and `nih_plug_egui` as git deps at rev
   `de421011f41a6d10fc8c7a6084e4f4dee0143683` (pinned to prevent the API from sliding
   out from under the builder; the BillyDM fork is the escape hatch if a fix can't land
   upstream).
4. **The mod gate:** any task touching foley's crates must keep, on the author's
   machine AND in CI, `cargo test --workspace --locked` + `cargo xtask header --check`
   passing and `cargo xtask levels` printing a peak table with **no scene peak above
   0.4**, all unchanged from before the change (behavioral parity at 48 kHz; numerically
   identical since the default-rate math is unchanged).
5. **Sample rate:** the plugin pass the host's `BufferConfig.sample_rate` into every bed
   constructor and one-shot call; the *supported range* is 32–192 kHz (the rate-generic
   refactor must tolerate 32k..192k; hosts run 44.1/48/88.2/96/176.4/192).
   *The plan's Review Focus carries the 44.1 kHz / 96 kHz render-correctness tests.*
6. **Formats:** CLAP + VST3 per plugin crate; state serialized across formats by
   NIH-plug.
7. **Platform:** author works on macOS Apple Silicon; macOS universal
   (aarch64 + x86_64, lipo'd) is the release target for v1. Linux x86_64 assumed to work
   via `cargo xtask bundle` on Linux CI; Windows deferred (listed in §14).

## 7. The central decision: how the plugin gets host-agnostic synth code

**Decision: Option (A) — fix the shared crates in place.** Specifically, in the foley
repo (DSP crates only; mod/ and the ABI untouched):

- `SAMPLE_RATE` stays defined (`pub const SAMPLE_RATE: u32 = 48_000;`) but stops being
  *the* rate: every bed constructor and one-shot gains an explicit `sample_rate: u32`
  parameter, threaded through as a field/argument everywhere; a new
  `pub const DEFAULT_SAMPLE_RATE: u32 = SAMPLE_RATE;` documents that the default path is
  still 48 kHz. `foley-ffi`, `foley-sim`, and `xtask` pass the default at every call
  site, so the mod's audio is bit-identical by construction.
- The four genuinely-const sites get named solutions (see Task 2/3 in the plan):
  `Wind`'s gust-lead array becomes a fixed `MAX_LEAD` cap (sized for 192 kHz — foley-dsp
  has no alloc, large constants like `Fixed<Voice,384>` and `Cap` are idiomatic already),
  `Fire`'s `TICK_SAMPLES` becomes a field computed in `new()`, `SURF_LEN` becomes
  `pub fn surf_len(rate: u32) -> usize` while the const is retained for foley-ffi's
  compat, and woodpecker's `KNOCK_LEN` becomes a local computed from rate.
- Add `pub fn warm_voice_tables()` (`lf.rs`'s OnceLock) so the plugin never pays the
  first-render allocation on the audio thread.
- **Thunder: keep its thread** (deviation from the brief's Option A sketch — see below),
  but add a determinism/exit guarantee and unit tests proving the worker exits when the
  bed is dropped (see the plan's Task 4).

**Why A, not B (adapter) or C (fork/vendor):**

- B hides the rate problem behind a browser-facing resampler. That is a permanent
  quality/latency tax (async resampling latency must be reported, and 96 kHz hosts would
  get an oversampled-content mismatch), and its Thunder piece ("rewrite Thunder locally")
  means duplicating worker/queue logic into the plugin. A kills the problem for all
  future consumers (`foley-sim`, future exporters) without mod risk, because the exact
  48 kHz path is preserved via the default const.
- C forks silently drift from the mod's sound discipline; that's the worst-cost option
  over months.
- The task explicitly gates A with "mod-side verification" — which exists in the foley
  repo today (`cargo test --workspace`, `xtask levels`, `xtask header --check`,
  `cargo xtask dist` running `mod/gradlew test` + gametest). These are the Phase-0
  task gates in the plan.

**Why deviate from the brief's "remove Thunder's long-lived thread":** the thread is
per-instance, bounded, and — verified by reading the worker loop — expected to exit when
the job-sender drops (so a plugin instance can reclaim it on Drop). A plugin is well
within its rights to spawn one thread per instance (spectral-compressor, a shipped
nih-plug plugin, uses a Rayon pool). Keeping the crate's thread avoids a behavioral
change to `foley_bed_strike` (which today enqueues, never renders, per FFI contract) —
any inline-rendering "fix" would hand the mod's stream thread a 1–20 ms CPU blip per
strike. The plan pins the thread's exit determinism with a test instead, so the mod's
library-unload policy stays exactly as it is.

## 8. Repo / crate layout (also see the plan's File Structure)

```
soundslop/                        (repo: cumsauce/soundslop)
  Cargo.toml                      [workspace] resolver 3, edition 2024, rust-version 1.99, publish = false, [lints] unsafe deny
  bundler.toml                    human-readable display names for bundling
  xtask/                          2-line wrapper: `nih_plug_xtask::main()`
  crates/
    foley-engine/                 no nih_plug dep: bed rigs, spatialization, guards, clip worker, motion, warmup
    foley-kit/                    "Soundslop Kit" plugin (instrument; MIDI one-shot voice bank)
    foley-ambience/               "Soundslop Ambience" plugin (soundscape layer machine)
  docs/superpowers/{specs,plans}/
  .github/workflows/ci.yml        build + tests + rt-probe + bundle matrix
```

## 9. Threading and realtime-safety model

- **Audio thread (`process`):** zero allocations, always. Statistics:
  - Per ambience layer: read the 4 smoothed scene knobs (`params...smoothed.next()`),
    hand them to `scene_from_knobs(...)` (no-alloc transform), call `bed.set_scene(...)`
    (only Water/Underwater/Cave-openness glide internally — the param smoother covers the
    rest), then `bed.render(&mut scratch[..])` — beds render sample-by-sample into the
    layer's preallocated scratch (`[f32; MAX_BLOCK]`, `MAX_BLOCK = 512`, stack
    buffers), then the layer is panned, guarded, and accumulated into the output.
  - Kit voices: play pre-rendered `Vec<f32>` clips (allocated by the **worker**, not
    here) with a small fade envelope (2 ms in, 10 ms out), pan, level.
  - Kind changes and per-hit seeds do NOT construct new beds on the audio thread.
- **Worker thread(s): exactly one per plugin instance**, spawned in `initialize()`,
  joined on `Drop` (drop the job `Sender`, then `join()` the handle). Because the crate-level Thunder (kept per §7) also owns ONE MORE
  thread when a Thunder layer is on, a full ambience instance has ≤ 2 threads.
  - Jobs income: `sync_channel::<EngineJob>(32)` bounded; `try_send` from the audio
    thread (allocation-free; full queue = drop + bump a starvation counter).
  - Results: `sync_channel::<Vec<f32>>(MAX_VOICES * 2)`; audio thread `try_recv()`s new
    clips each process (start-`RenderJob`s to start play head), then reuses them via
    `Job::Free` back to the worker (carrying the finished `Vec`) — the EXACT pattern
    thunder.rs uses, so a clip never has to be free'd on the audio thread.
  - Worker body renders one-shots (`contact`, `note`, `voice`, …) with the current
    params/seed; worker may allocate freely (rates of always ≤ 32 outstanding jobs).
- **Rebuilds:** bed `kind` changes (and bed `seed` changes big enough to require
  reconstruction) are executed by the worker — it constructs the new bed (alloc), pushes
  it to `sync_channel::<BedUnion>(1)`, and the audio thread does `try_recv()` once per
  block to take it (full/pending = keep current, do not block). This matches
  foley-ffi's design of "never construct on the audio thread".
- **Swap fade:** when the audio thread takes a new bed from the rebuild channel, that
  layer applies a short (~5 ms) linear fade-in and its guard resets, so a kind swap
  never clicks.
- **Kit hit latency (accepted):** a NoteOn renders on the worker and starts on a later
  block — variable latency ≈ one block + the one-shot render time (the plan's bench pins
  typical figures). Accepted for v1; an RT-safe render-into-sink one-shot API in
  foley-synth is the follow-up if it ever bothers the author. A starved job (queue full)
  drops the hit and bumps the starvation counter.
- **Pre-warm:** `initialize()` calls `warm_voice_tables()` (new foley API) once, plus
  one `howl(seed)` render on the worker for table warm-up (redundancy for older revs).
- **Guards:** per-layer envelope-follower ceiling exactly like foley-ffi's bed guard
  (`wanted = LAYER_CEILING / env`, `LAYER_CEILING = 0.4`), plus a post-mix master
  ceiling guard `MASTER_CEILING = 0.85` with the same mechanism (smoother gain travel —
  it protects the whole mix). The mechanism reference is foley-ffi's own bed guard
  (`crates/foley-ffi/src/lib.rs:742-750`).
- **Statuses**: both plugins return `ProcessStatus::KeepAlive` always (generative,
  always-on); `HARD_REALTIME_ONLY = false` so hosts may bounce offline;
  `SAMPLE_ACCURATE_AUTOMATION = false` — scene glide is smoothed by the param smoother
  at block resolution, not per-sample-split, so block boundaries stay where the DAW puts
  them.
- **No plugin-owned globals.** The only process-global will be foley-synth's lazily
  built `TABLES` (init'd by warm-up), which is immutable-after-init, which is exactly
  safe for multi-instance use.

## 10. Parameter model: Soundslop Ambience

For layer i ∈ 1..=6 (param ids `l{i}_kind`, `l{i}_gain`, `l{i}_seed`, `l{i}_motion_amt`,
`l{i}_motion_rate`, `l{i}_dist`, `l{i}_az`, `l{i}_knob_a`, `l{i}_knob_b`, `l{i}_knob_c`,
`l{i}_knob_d`):

| Param | Type | Meaning |
|---|---|---|
| `l{i}_kind` | EnumParam, 11 variants | `Off` + the 10 beds (order == foley.h: Rain, Wind, Water, Fire, Lava, Thunder, Cave, Underwater, Biome, Surf) |
| `l{i}_gain` | FloatParam 0..=1.5, default 0.5 | layer's output level (before pan) |
| `l{i}_seed` | IntParam 0..=16_777_215, default seeded by index | bed/hit randomness source |
| `l{i}_motion_amt` | FloatParam 0..=1, default 0.15 | how far the scene might wander over time |
| `l{i}_motion_rate` | FloatParam 0..=1 | how fast the wander state updates |
| `l{i}_dist` | FloatParam 0..=64 (blocks), default 0.0 | distance; drives geometry attenuation + air-absorption LPF (via `foley_dsp::absorption_cutoff`, standard 15 °C / 65 % RH) |
| `l{i}_az` | FloatParam −180..=180°, default 0 | azimuth; constant-power panner |
| knob a/b/c/d | FloatParam 0..=1, default 0.5 | meaning per `l{i}_kind` per the table below |

Per-kind scene-knob mapping (this table is the "instrument" — implemented in
`foley-engine/src/kind_map.rs` as `scene_from_knobs(...)` per kind; every unused knob is
documented as "no effect at this kind" and left in the rack):

| kind | knob A | knob B | knob C | knob D |
|---|---|---|---|---|
| `Rain` | `rate` mm/h, 0..=50 | roof height in blocks, 0..=20 (0.0 = `roof: None`, else `roof = (dominant surface, height)`) | surface preset, 0..=4 step 0.25 (Field, City, Leaves, Ocean, Glass — vec's defined in `foley-engine`) | (unused) |
| `Wind` | `speed` m/s, 0..=30 | `exposure` 0..=1 | `leaves` 0..=1 | (unused) |
| `Water` | `flow` blocks, 0..=20 | `falls` blocks, 0..=20 | (unused — `gain` stays at scene default) | (unused) |
| `Fire` | `size` blocks, 0..=10 | (unused) | (unused) | (unused) |
| `Lava` | `area` blocks, 0..=15 | (unused) | (unused) | (unused) |
| `Cave` | `depth` 0..=1 | `drips` 0..=1 | `openness` 0..=1 | (unused — `mass` fixed 0.3) |
| `Underwater` | `level` 0..=1 | (unused) | (unused) | (unused) |
| `Thunder` | `exposure` 0..=1 | `strike` momentary, 0→1 edge enqueues a strike with `base` derived from az/dist | `height` blocks 0..=50 (the strike base's Y) | (unused — celsius 20, humidity 60 fixed) |
| `Biome` | `kind` 0..=6 step 1/6 (`BiomeKind::from_index`) | `intensity` 0..=1 | `sub` 0..=10 | (unused) |
| `Surf` | `height` m 0..=3 | `storm` 0..=1 | `shore` 0..=1 | (unused) |

Motion semantics: the walk yields an **additive offset** on top of the automated knob
values, clamped to each knob's range — it never overrides automation. The Thunder
`strike` knob is a momentary param: a rising edge on the raw value (0→1) enqueues a
strike; the GUI exposes it as a press-and-release button that toggles the param.

Master params: `output_gain` (FloatParam −24..=+6 dB, default 0 dB) and `scene_glide`
(FloatParam 0.02..=1.0 s, default 0.30 — smoothing style `Exponential` applied to all
scene knobs; snap-on-first-scene stays the bed-construction layer's job, not the
smoother's). The global `SUBMERGED` LPF from foley-ffi is deliberately dropped (§5):
underwater comes from the `Underwater` layer and per-layer distance LPF. v1 total =
6 layers × 11 + 2 = **68 params**.

## 11. Parameter model: Soundslop Kit

For slot i ∈ 1..=8 (ids `s{i}_category`, `s{i}_kind`, `s{i}_voice_mob`, `s{i}_note`,
`s{i}_knob_a`, `s{i}_knob_b`, `s{i}_gain`, `s{i}_pan`, `s{i}_seed`, `s{i}_fresh`):

| Unit | Type | Range/meaning |
|---|---|---|
| category | EnumParam, 6 variants | `Contact`, `Fracture`, `Shot`, `Primitive`, `Voice`, `Note` |
| Kit slot kinds | IntParam 0..=62 | semantic per category (ranges per category are clamped in the engine: Contact `material*4+contact_idx` 0..=39, Fracture 0..=9, Shot 0..=18, Primitive 0..=5, Note 0..=15; Voice uses `kind = call` 0..=42) |
| voice_mob | IntParam 0..=51 | only meaningful for `Voice` (the `Mob`); other categories ignore |
| note | IntParam 0..=127, default 60 | the MIDI note this slot triggers on |
| knob a/b | FloatParam 0..=1 | semantic per category (see table) |
| gain | FloatParam 0..=1.5, default 0.8 | per-hit level multiplier |
| pan | FloatParam −100..=100 | per-hit stereo pan |
| seed | IntParam 0..=16_777_215 | per-slot base seed |
| `s{i}_fresh` | BoolParam, default true | per-hit `seed = f(hit counter)`, so "no two drips are the same" (foley's own design, `docs/PLAN.md:62`) |

Knob semantics per category (defaults in parens; full per-kind table lives in
`foley-engine/src/kit_map.rs`):

- **Contact** — knob A = speed multiplier 0.25×..4× (0.5 = 1×); velocity maps impact
  speed via `speed = 0.5 + 5.0·velocity` m/s.
- **Fracture** — no knobs.
- **Shot** — A/B per shot kind (bell: A = q; crickets: A = celsius 0..=1 → −5..=35 °C;
  birdsong: A = species 0..=1 → 0..=56 wraps; drip_*: A = radius 0..=1 → 0.5..=4 mm;
  bang: A = size 0..=5 m).
- **Primitive** — A/B = the primitive's first two floats (bubble: radius, —;
  noise_burst: centre/q; tone: start/target; gust: speed, —).
- **Voice** — no knobs (mob = `s{i}_voice_mob`, call = kind).
- **Note** — knob A = character (0..=1, default 0.5, passed straight to `note()`);
  pitch is the triggering MIDI note. Velocity does not bend character — it only scales
  gain (the earlier "0.25 + 0.75·velocity" idea is dropped: a param cannot tell whether
  the user has touched it).

Master: `output_gain` (−24..=+6 dB, default 0 dB), `polyphony` EnumParam {4, 8, 16}
(at the cap the **oldest playing voice is stolen**), `humanize` FloatParam 0..=1
(per-hit gain multiplier in [1 − 0.5·humanize, 1.0]) — **83 params** total (8×10 + 3).

**MIDI → slot:** `NoteOn` triggers the unique slot(s) whose `s{i}_note` == note
(multiple slots on one note is allowed, plays all). `NoteOff` is ignored (one-shots are
fixed-length). Velocity → the per-category velocity law (Contact speed, Note character)
plus gain. Per-hit seed = `mix(slot.seed, note, hit_counter, token)` — splitmix64
recipe in `foley-engine::seed_mix` (`hit_seed(base: u64, note: u8, velocity: f32,
counter: u32) -> u64`).

**Out-of-range / missing sounds:** a `NoteOn` whose note matches no slot → **silence,
no panic** (the GUI shows which notes are bound). `note()` returning `None`
(out-of-range instrument) → that voice is dropped silently. `Mob::from_index` etc.
returning `None` is guarded the same way (matches the crates' no-panic contract).

## 12. Plugin metadata (both crates)

Both plugin crates export both formats:

```rust
nih_export_clap!(Kit); nih_export_vst3!(Kit);
```

CLAP IDs are reverse-DNS: `"io.github.cumsauce.soundslop.kit"` and
`"io.github.cumsauce.soundslop.ambience"`.

VST3 category: `const VST3_SUBCATEGORIES: &[Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];`
CLAP features: `&[ClapFeature::Instrument, ClapFeature::Synthesizer]` for both plugins
(Ambience ignores MIDI at runtime but declares Basic MIDI input for host compatibility).

## 13. Preset / state model

All "soundscape" data is either a parameter (layer kind/gain/knobs/motion/dist/az/seed)
or derivable from them — so **soundscape presets = plugin state**, nothing exotic:

- DAW save/recall: automatic (NIH-plug serializes params + persist fields).
- In-GUI: `Save…` / `Load…` buttons → `GuiContext::get_state()` → JSON → `<path>` (UI
  thread; no RT interaction), `Load…` → `set_state(...)`. Both plugins carry this (the
  kit's state = its slot mapping).
- State format version pin: `PluginState.version` field remains informational under the
  pinned rev; no migrations are needed for v1 (param layout is stable).

## 14. Platform / release / CI story

- **Confirmed: macOS Apple Silicon is the author's DAW machine; primary host is
  Bitwig (CLAP-first, VST3 secondary).** v1 release = macOS
  universal (aarch64 + x86_64) `.clap` + `.vst3` bundles via `cargo xtask
  bundle-universal <pkg> --release`, unsigned ("may need to disable Gatekeeper" —
  documented). Linux x86_64: CI-only "it builds and tests" (no real host tested).
  Windows: deferred task (xtask cross-compile story documented but not built).
- CI: `clippy -D warnings` + tests + RT-alloc probe + `cargo xtask bundle -p <each>`
  on macOS; bundle artifacts saved per rev. No sibling foley checkout needed — git deps
  pull from the pinned rev, and the foley repo is public, so CI clones without
  credentials.
- VST3 license note as a release constraint: distributable only under GPLv3-compliant
  terms because of nih-plug's bundled vst3-sys.

## 15. What's left to the builder

- egui layout/tuning is free (sizes, grouping, scroll), but the v1 **widget inventory
  is fixed**: one `ParamSlider` per parameter grouped per layer/slot, the Thunder
  `strike` as a momentary button, and `Save…`/`Load…` buttons with a native file dialog
  (`rfd`). No meters, activity LEDs, or note-binding keyboard in v1 — RT→GUI telemetry
  is a follow-up.
- The seed-mix details: candidate formula chosen so context + hit counter + velocity are
  mixed with splitmix64 (`foley-engine::seed_mix`), keep it stable forever (presets
  PIN it).
- Note-block `note()` pitch ↔ MIDI mapping is fixed (MIDI note = `note: f32`); no
  transposition knobs in v1.
- Name/identity is decided: repo `soundslop` (GitHub `cumsauce/soundslop`), display
  names "Soundslop Kit"/"Soundslop Ambience", CLAP/VST3
  IDs as in §12 (tags as 0.1.0).
- Measured CPU budget assertions in tests are soft targets, not hard guarantees
  (hardware varies); the tests pin correctness (allocs/panics/NaN), the bench pins
  nothing that would fail a CI run.

---

## Appendix — prior art pointers (all inside the foley repo)

- `crates/foley-ffi/src/no_alloc.rs` — counting-global-allocator allocation proof, to be
  mirrored in the plugin repo's test harness (`alloc_probe`).
- `crates/foley-ffi/src/lib.rs:696-711, 741-750` — the 0.3 s/128-frame glide and the
  envelope ceiling guard; the plugin reuses the *mechanisms*, not the constants (except
  confirming the 0.3 s default from the author's own prior art).
- `crates/foley-synth/src/thunder.rs:262-322` — the bounded-channel worker + `Job::Free`
  dequeue pattern to be mirrored exactly in `foley-engine/src/clip.rs`.
- `crates/foley-sim/src/main.rs` — a non-game host's control loop.
- `crates/foley-synth/examples/bed_bench.rs` — f32 bed usage without FFI (timing table).
- `docs/INTEGRATING.md` / `docs/PLAN.md` — seed/variation/48kHz-mono-I16 statements.
