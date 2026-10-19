# Soundslop Plugin UI — Design Requirements

## Overview

Soundslop is a Minecraft ambient-sound generator distributed as CLAP and VST3 plugins. Two UIs ship in one binary: **Kit** (event-driven sound triggering) and **Ambience** (continuous environmental beds). This document specifies what each UI must surface. The visual design — layout, colors, typography, proportions — is yours to create.

The UI is implemented in `egui` (via `nih_plug_egui`) as a resizable native window inside the DAW's editor frame. Minimum usable area: **380 × 520 px**. Target aspect ratio: roughly 2.4:3 (taller than wide, like most mixer-channel strips).

These requirements reference similar plugins for convention and clarity, not for aesthetic mimicry.

---

## Functional Requirements

### Core Constraints (apply to both UIs)

1. **Every parameter in `params.rs` must be editable from the UI.** There are no hidden or headless parameters.
2. **Every parameter has a live readback label** showing its current normalized value.
3. **State persistence**: the host must be able to save and recall the full UI state (parameter values + window size/position) via the DAW's preset system. The "Save…" and "Load…" buttons export/import a JSON snapshot of all parameters (not a DAW preset file).
4. **Responsive to the parameter system**: parameter IDs must match `params.rs` exactly (e.g., `output_gain`, `s1_category`, `s8_seed`). Parameter names shown to the user should be human-readable but must map to the IDs.

### Soundslop Kit UI

**Parameter inventory** (83 total):

| Group | Parameters | Type | Range |
|---|---|---|---|
| Master | Output Gain | float, dB | −24 to +6 dB |
| Master | Polyphony | enum | 4, 8, 16 |
| Master | Humanize | float, 0–1 | 0.0 to 1.0 |
| Per slot (×8) | Category | enum | Contact, Fracture, Shot, Primitive, Voice, Note |
| Per slot | Kind | int | 0–62 |
| Per slot | Voice/Mob | int | 0–51 |
| Per slot | Note | int | 0–127 (MIDI note) |
| Per slot | Knob A | float, 0–1 | continuous |
| Per slot | Knob B | float, 0–1 | continuous |
| Per slot | Gain | float | 0.0–1.5 |
| Per slot | Pan | float | −100 to +100 |
| Per slot | Seed | int | 0–16,777,215 |
| Per slot | Fresh | bool | on/off |

**Must-surface behavior:**
- An 8-slot rack where each slot is a collapsible/expandable panel.
- The two context-dependent knobs (Knob A / Knob B) must show **dynamic labels** — the label changes based on which Category + Kind is selected (e.g., selecting a "Bell" sound shows "Damp" and "Decay" for its knobs; selecting "Birds" shows "Wobble" and "Rate"). The `params.rs` doc comment confirms: "two context-dependent knobs, labels come from `kit_knob_labels`".
- "Fresh" is a boolean toggle — present a checkbox or on/off button.
- "Kind" and "Voice/Mob" are index selectors, not simple sliders. A numeric spinner (0–62 / 0–51) should suffice.

### Soundslop Ambience UI

**Parameter inventory**:

| Group | Parameters | Type | Range |
|---|---|---|---|
| Master | Output Gain | float, dB | −24 to +6 dB |
| Master | Scene Glide | float | 0–1 (unspecified unit, smoothing time) |
| Per layer (×6) | Kind | int | bed type selector |
| Per layer | Gain | float | 0.0–1.5 |
| Per layer | Seed | int | 0–16,777,215 |
| Per layer | Motion Amount | float | 0–1 |
| Per layer | Motion Rate | float | 0–1 |
| Per layer | Distance | float | 0–1 |
| Per layer | Azimuth | float | 0–1 |
| Per layer | Knob A | float | context-dependent label |
| Per layer | Knob B | float or momentary button | see below |
| Per layer | Knob C | float | context-dependent label |
| Per layer | Knob D | float | context-dependent label |

**Must-surface behavior:**
- A 6-layer rack layout.
- The four labeled knobs (A–D) must have **dynamic labels** that change when "Kind" is changed. The specific labels depend on whether the layer is configured as Thunder vs. any other bed type.
- **Thunder special case**: When a layer is a Thunder bed, Knob B becomes a momentary push-button labeled "Strike". When the button is pressed, Knob B receives 1.0; on release, 0.0. For all non-Thunder layers, Knob B is a standard continuous slider.
- "Scene Glide" controls cross-fade time between scene presets — a standard slider is appropriate.

---

## What the Designer Controls

You have freedom in all of the following. These are **not** prescriptions — they are decision points where similar plugins have made different choices:

| Element | Options seen in comparable plugins |
|---|---|
| **Layout of per-slot/layer panels** | Horizontal strip (Valhalla), vertical stack (FabFilter style), accordion (iZotope Neutron), or tabbed (some Soundtoys) |
| **Parameter controls** | Sliders, knobs/dials (Valhalla, Soundtoys), numeric fields (FabFilter), or hybrid sliders-with-fields (FabFilter Pro-Q 3) |
| **Collapsibility** | Fully expanded (Valhalla Room), collapsible by category (FabFilter), or fixed-height with scroll (Serum) |
| **Label placement** | Above/below controls (most), inline (some Waves UIs), or tooltip-on-hover (modern flat designs) |
| **Boolean switches** | Checkbox, toggle switch (iOS style — FabFilter), or button (momentary vs. latched) |
| **Enum selectors** | Dropdown (FabFilter), segmented buttons (macOS-style), or numbered tabs (Valhalla style) |

### Plugin references for layout patterns

- **Valhalla plugins** (Room, VintageVerb, Delay): Compact panels, horizontal parameter strips, clear labels inside knobs. Known for dense but readable layouts.
- **FabFilter Pro-Q 3 / Pro-C 2**: Horizontal top bar with a vertically-scrolling parameter strip, each parameter as its own labeled row (slider + numeric field), collapsible sections.
- **iZotope Ozone / Neutron**: Modal-style panels with tabs, collapsible modules, and a consistent "module header → controls" pattern.
- **Soundtoys Decapitator / Little AlterBoy**: Characterful but compact single-panel layouts, knobs as the primary control metaphor.

### Minecraft-specific context

Soundslop is a Minecraft mod — the UI should feel at home in that world without being kitsch. Consider:
- Using Minecraft's color palette (earth tones, cyan/magenta highlights from the game's UI) as an accent system, not a theme park.
- Avoiding literal Minecraft fonts (which are pixelated and unsuitable for fine control).
- Referencing the existing `egui` styling as the baseline — the current UI already has a minimalist, no-chrome aesthetic.

---

## Technical Constraints

These are imposed by the codebase and cannot be changed:
- UI framework is `egui` (immediate mode), rendered as a native window inside the host DAW.
- The egui state is persisted as `EguiState` tied to the plugin's `#[persist = "editor-state"]` parameter group.
- Font rendering and sizing is handled by egui's built-in font system.
- Window minimum size is 380 × 520 px.

These **do not** constrain your visual design — the framework supports custom theming, colors, and layout within the egui model.

---

## Success Criteria

The design is complete when:
1. Every parameter (83 for Kit, ~56 for Ambience) has a mapped control.
2. Dynamic label contexts are clearly indicated for the four context-dependent knobs.
3. The Thunder "Strike" button is visually distinct from Knob B elsewhere but clearly occupies the same conceptual position.
4. Save/Load (JSON) and preset recall are reachable without hunting.

---

## Next Steps

1. Create or open a Figma file for this project.
2. Build the Kit UI first (it's listed first and has a fixed slot count).
3. Reuse components and variables across both UIs (Kit and Ambience share parameter semantics).
4. When ready, export assets or Code Connect mappings for implementation handoff.
