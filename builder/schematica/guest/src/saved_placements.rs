//! Remembers each schematic transform and the active entry of each local world.

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
        .filter(|(key, value)| key.len() <= 33 && decode(value).is_some())
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
    /// Restores the selected entry, with the older single-record format as a fallback.
    pub(super) fn restore_placement(&mut self, entering_from_world: bool) {
        let Some(key) = self
            .placement_key
            .as_ref()
            .filter(|_| self.options.remember_placement)
        else {
            return;
        };
        let records = &self.options.saved_placements;
        let last = records.get(key).and_then(decode);
        let valid =
            |entry| entry < self.library.len() && !self.options.deleted_entries.contains(&entry);
        if let Some(entry) = self
            .active_entry
            .filter(|entry| self.loaded && valid(*entry))
        {
            let saved = records
                .get(&format!("{key}/{entry}"))
                .and_then(decode)
                .or_else(|| last.filter(|(index, _)| *index == entry));
            if let Some((_, placement)) = saved {
                self.placement = Some(placement);
                self.auto_place = false;
                self.dirty = true;
                return;
            }
        }
        if (entering_from_world || !self.loaded)
            && let Some((entry, _)) = last.filter(|(entry, _)| valid(*entry))
        {
            self.request = Some(entry);
        }
    }

    /// Saves the entry transform and keeps track of the last active entry for world rejoin.
    pub(super) fn remember_placement(&mut self) {
        let Some(key) = self
            .placement_key
            .as_ref()
            .filter(|_| self.options.remember_placement)
        else {
            return;
        };
        let Some((entry, placement)) = self.active_entry.zip(self.placement) else {
            return;
        };
        let [x, y, z] = placement.origin;
        let mirror = match placement.mirror {
            Mirror::None => 0,
            Mirror::X => 1,
            Mirror::Z => 2,
        };
        let value = json!([entry, x, y, z, placement.quarter_turns, mirror]);
        let mut changed = false;
        for key in [format!("{key}/{entry}"), key.clone()] {
            let records = &mut self.options.saved_placements;
            if records.get(&key) == Some(&value)
                || (records.len() >= MAX_SAVED && !records.contains_key(&key))
            {
                continue;
            }
            records.insert(key, value.clone());
            changed = true;
        }
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

#[cfg(test)]
#[test]
fn reloads_each_entry_after_switching() {
    let mut viewer = crate::tests::reference_viewer();
    viewer.options.remember_placement = true;
    viewer.library = vec!["First".into(), "Second".into()];
    viewer.placement_key = Some("world-a/0".into());
    let first = Placement::new([10, 64, 10]);
    let second = Placement::new([32, 70, 12]);
    viewer.active_entry = Some(0);
    viewer.placement = Some(first);
    viewer.remember_placement();
    viewer.active_entry = Some(1);
    viewer.placement = Some(second);
    viewer.remember_placement();
    viewer.active_entry = Some(0);
    viewer.placement = None;
    viewer.restore_placement(false);
    assert_eq!(viewer.placement, Some(first));
    viewer.active_entry = Some(1);
    viewer.restore_placement(false);
    assert_eq!(viewer.placement, Some(second));
}
