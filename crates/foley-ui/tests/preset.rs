//! Preset snapshot contract: parameters only, never editor geometry.

use std::collections::BTreeMap;

use foley_ui::preset;
use nice_plug::prelude::PluginState;

fn state_with_editor_size() -> PluginState {
    let mut fields = BTreeMap::new();
    fields.insert(
        "editor-state".to_owned(),
        r#"{"width":576.0,"height":720.0}"#.to_owned(),
    );
    PluginState {
        version: "0.1.0".to_owned(),
        params: BTreeMap::new(),
        fields,
    }
}

#[test]
fn snapshot_keeps_parameters_and_drops_editor_geometry() {
    let json = preset::to_json(state_with_editor_size()).expect("state serializes");
    assert!(json.contains("\"params\""), "parameters are exported");
    assert!(!json.contains("editor-state"), "no persisted field name");
    assert!(!json.contains("width"), "no editor geometry");

    let round_trip = preset::from_json(&json).expect("snapshot parses");
    assert!(round_trip.fields.is_empty());
    assert_eq!(round_trip.version, "0.1.0");
}

#[test]
fn snapshot_without_geometry_loads_unchanged() {
    // Loading a snapshot must leave persisted state (the window size) alone.
    assert!(preset::from_json(r#"{"version":"0.1.0","params":{},"fields":{}}"#).is_some());
    assert!(preset::from_json("not json").is_none());
}

#[test]
fn legacy_snapshot_editor_state_is_stripped_on_load() {
    // Snapshots written before this contract carried editor geometry; loading
    // one must not resize the window.
    let legacy = r#"{"version":"0.1.0","params":{},"fields":{"editor-state":"{\"width\":576.0,\"height\":720.0}"}}"#;

    let loaded = preset::from_json(legacy).expect("snapshot parses");
    assert!(
        !loaded.fields.contains_key("editor-state"),
        "editor geometry is dropped on load too"
    );
}
