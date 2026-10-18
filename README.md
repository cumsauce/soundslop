# soundslop

Two CLAP + VST3 plugins (Soundslop Kit and Soundslop Ambience) that reuse the
Rust DSP crates from the [foley](https://github.com/cumsauce/foley) Minecraft
mod. Procedural foley one-shots and six-layer soundscapes, generated live —
no samples, no Minecraft, no Java.

- **Soundslop Kit** — a MIDI-triggered one-shot instrument. Eight slots, each
  mapping a MIDI note to a procedural foley sound (contacts, fractures, shots,
  primitives, mob voices, note-block instruments). Per-hit seed variation so no
  two drips are the same.
- **Soundslop Ambience** — a six-layer generative soundscape machine. Pick a
  bed per layer (rain, wind, water, fire, lava, thunder, cave, underwater,
  biome, surf), set distance/azimuth, and let motion wander the scene. Thunder
  layers get a momentary strike button.

Both plugins share `foley-engine` (headless, no NIH-plug dependency), which
wraps `foley-dsp` / `foley-synth` from the foley repo at a pinned git rev.

## Build

Requires a recent stable Rust (see `rust-version` in `Cargo.toml`). For a
fresh clone without a sibling `foley` checkout, the committed `[patch]` section
in the root `Cargo.toml` must be dropped (it points cargo at `../foley` for
local development); CI does this automatically. With a sibling checkout at
`../foley`, it is used instead of fetching from GitHub.

```sh
# Debug bundles (CLAP + VST3) into target/bundled/
cargo xtask bundle foley-kit
cargo xtask bundle foley-ambience

# Release universal (arm64 + x86_64) bundles, macOS only
cargo xtask bundle-universal foley-kit --release
cargo xtask bundle-universal foley-ambience --release
```

## Install (macOS)

Copy the bundles from `target/bundled/`:

```sh
cp -r "target/bundled/Soundslop Kit.clap" ~/Library/CLAP/
cp -r "target/bundled/Soundslop Kit.vst3" ~/Library/Audio/Plug-Ins/VST3/
cp -r "target/bundled/Soundslop Ambience.clap" ~/Library/CLAP/
cp -r "target/bundled/Soundslop Ambience.vst3" ~/Library/Audio/Plug-Ins/VST3/
```

## Host compatibility

Tested against **Bitwig** — CLAP is the primary format; VST3 is secondary. AU
is a non-goal for v1. Windows is deferred (spec §14).

## Gatekeeper (unsigned binaries)

These bundles are not code-signed. On macOS, right-click each `.clap`/`.vst3`
in Finder → Open → Open again to bypass Gatekeeper, or run:

```sh
xattr -dr com.apple.quarantine ~/Library/CLAP/*.clap ~/Library/Audio/Plug-Ins/VST3/*.vst3
```

## Development

- `cargo test --workspace` — full test suite (includes an RT-alloc probe that
  proves both rigs allocate zero times on the audio thread after warm-up).
- `cargo test -p foley-engine --test render_bench -- --ignored --nocapture` —
  prints per-category render cost (never asserts on timing).
- The foley repo is pinned by rev in the root `Cargo.toml`; bump the rev to
  pick up DSP changes. Sample-rate-generic constructors accept 32–192 kHz; the
  mod itself always runs 48 kHz mono i16 and is bit-identical at that rate.

## License

GPL-3.0-or-later (see `LICENSE.txt`). The VST3 wrapper's license (GPL) is why
the whole project is GPL.
