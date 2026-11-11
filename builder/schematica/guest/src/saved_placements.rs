//! Remembers the active schematic in each local world and dimension.

use crate::{Mirror, Placement, Viewer};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const MAX_SAVED: usize = 100;

/// Accepts only bounded records from the component's settings file.
pub(super) fn load(value: &Value) -> BTreeMap<String, Value> {
    value
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(key, value)| key.len() <= 29 && decode(value).is_some())
        .take(MAX_SAVED)
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// Decodes an entry index and a bounded placement transform.
fn decode(value: &Value) -> Option<(usize, Placement)> {
    let [entry, x, y, z, turns, mirror] = value.as_array()?.as_slice() else {
        return None;
    };
    let entry = usize::try_from(entry.as_u64()?).ok()?;
    let coordinate = |value: &Value| {
        let n = value.as_i64()?;
        (-30_000_000..=30_000_000).contains(&n).then_some(n as i32)
    };
    Some((
        entry,
        Placement {
            origin: [coordinate(x)?, coordinate(y)?, coordinate(z)?],
            quarter_turns: u8::try_from(turns.as_u64()?)
                .ok()
                .filter(|turns| *turns < 4)?,
            mirror: match mirror.as_u64()? {
                0 => Mirror::None,
                1 => Mirror::X,
                2 => Mirror::Z,
                _ => return None,
            },
        },
    ))
}

impl Viewer {
    /// Restores the transform after decoding the saved library entry.
    pub(super) fn restore_placement(&mut self, entering_from_world: bool) {
        let Some((entry, placement)) = self
            .placement_key
            .as_ref()
            .filter(|_| self.options.remember_placement)
            .and_then(|key| self.options.saved_placements.get(key))
            .and_then(decode)
            .filter(|(entry, _)| {
                *entry < self.library.len() && !self.options.deleted_entries.contains(entry)
            })
        else {
            return;
        };
        if self.active_entry == Some(entry) && self.loaded {
            self.placement = Some(placement);
            self.auto_place = false;
            self.dirty = true;
        } else if entering_from_world || !self.loaded {
            self.request = Some(entry);
        }
    }

    /// Writes changes only, including removing the saved position on explicit unload.
    pub(super) fn remember_placement(&mut self) {
        let Some(key) = self
            .placement_key
            .as_ref()
            .filter(|_| self.options.remember_placement)
        else {
            return;
        };
        let records = &mut self.options.saved_placements;
        let changed = if let Some((entry, placement)) = self.active_entry.zip(self.placement) {
            let [x, y, z] = placement.origin;
            let mirror = match placement.mirror {
                Mirror::None => 0,
                Mirror::X => 1,
                Mirror::Z => 2,
            };
            let value = json!([entry, x, y, z, placement.quarter_turns, mirror]);
            if records.get(key) == Some(&value)
                || (records.len() >= MAX_SAVED && !records.contains_key(key))
            {
                false
            } else {
                records.insert(key.clone(), value);
                true
            }
        } else if !self.loaded {
            records.remove(key).is_some()
        } else {
            false
        };
        if changed {
            self.options.save();
        }
    }
}

#[cfg(test)]
#[test]
fn restores_only_the_matching_world_and_dimension() {
    let mut viewer = crate::tests::reference_viewer();
    viewer.options.remember_placement = true;
    viewer.placement_key = Some("world-a/0".into());
    viewer.active_entry = Some(0);
    let mut placement = Placement::new([50, 82, -40]);
    placement.rotate_cw();
    placement.set_mirror(Mirror::Z);
    viewer.placement = Some(placement);
    viewer.remember_placement();
    viewer.placement = None;
    viewer.placement_key = Some("world-a/1".into());
    viewer.restore_placement(false);
    assert_eq!(viewer.placement, None);
    viewer.placement_key = Some("world-a/0".into());
    viewer.restore_placement(false);
    assert_eq!(viewer.placement, Some(placement));
}
