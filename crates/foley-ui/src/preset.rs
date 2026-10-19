//! JSON preset snapshots: parameters only, never editor geometry.

use nice_plug::prelude::PluginState;

/// Serialize a plugin state for Save…, dropping persisted editor geometry.
/// The contract treats JSON snapshots as parameter recall; window size is
/// host-mediated editor state, not part of a preset.
pub fn to_json(mut state: PluginState) -> Option<String> {
    state.fields.remove("editor-state");
    serde_json::to_string_pretty(&state).ok()
}

/// Parse a snapshot for Load…; absent persisted fields keep their current
/// values (nice-plug only applies the fields present in the snapshot).
/// Editor geometry is stripped here too: snapshots written before this
/// contract carried it, and loading one must not resize the window.
pub fn from_json(json: &str) -> Option<PluginState> {
    let mut state: PluginState = serde_json::from_str(json).ok()?;
    state.fields.remove("editor-state");
    Some(state)
}
