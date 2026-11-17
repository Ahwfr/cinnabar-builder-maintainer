//! The Cinnamatica window's panel: its precomputed layout and the values each refresh sends.

use crate::{Viewer, label, panel};
use serde_json::{Map, Value, json};
use std::cell::RefCell;

thread_local! {
    /// The binding values the panel last accepted, so a refresh sends only what changed.
    static SENT: RefCell<Map<String, Value>> = RefCell::new(Map::new());
}

/// Sets the window's layout and its current values; false when the host refused them.
pub fn prepare(viewer: &Viewer) -> bool {
    SENT.with(|sent| sent.borrow_mut().clear());
    let result = panel::set_content(crate::browser::LAYOUT).and_then(|()| update(viewer));
    if let Err(e) = result {
        label(&format!("Cinnamatica panel: {e}"));
        return false;
    }
    true
}

pub fn request_open() {
    if let Err(e) = panel::open() {
        label(&format!("Cinnamatica panel: {e}"));
    }
}

/// Sends the window's values that changed since the last refresh.
pub fn refresh(viewer: &Viewer) {
    if let Err(e) = update(viewer) {
        label(&format!("Cinnamatica panel: {e}"));
    }
}

/// Builds every binding but sends only the changed ones, written straight to text: copying and
/// re-sending the whole set on every scroll exhausted the frame's fuel for large schematics.
fn update(viewer: &Viewer) -> Result<(), String> {
    let mut out = Map::new();
    let mut controls = Vec::new();
    crate::browser::bindings(viewer, &mut out, &mut controls);
    SENT.with(|sent| {
        let mut sent = sent.borrow_mut();
        let changed: Map<String, Value> = out
            .into_iter()
            .filter(|(name, value)| sent.get(name) != Some(value))
            .collect();
        if changed.is_empty() && controls.is_empty() {
            return Ok(());
        }
        let text = format!(
            r#"{{"binding_changes":{},"controls":{}}}"#,
            serde_json::to_string(&changed).map_err(|error| error.to_string())?,
            serde_json::to_string(&controls).map_err(|error| error.to_string())?,
        );
        match panel::update_content(&text) {
            Ok(()) => {
                sent.extend(changed);
                Ok(())
            }
            Err(error) => {
                // Resend everything next time rather than trusting a partial state.
                sent.clear();
                Err(error)
            }
        }
    })
}

pub(super) fn material_icon(line: Option<&schematica_core::MaterialLine>) -> Value {
    // The native item renderer supplies isometric block artwork; item-shaped
    // materials retain their normal sprites. Unknown identifiers draw nothing.
    json!({"item":{"identifier": line.map_or("minecraft:air", |line| line.name.as_str()), "metadata":0}})
}
