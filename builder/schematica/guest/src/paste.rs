//! Explicit operator paste through Bedrock's advertised editing commands.

use crate::{Viewer, gameplay, overlay, structure_paste::StructurePaste, terrain};
use schematica_core::{BlockState, Placement, Schematic, StateValue};
use std::collections::HashMap;

const COMMAND_BYTES: usize = 128;

pub(super) struct Paste {
    scope: (u64, i32),
    placement: Placement,
    destroy: bool,
    cursor: usize,
    sent: usize,
    skipped: usize,
    palette: HashMap<u32, Option<String>>,
    structure: Option<StructurePaste>,
}

impl Viewer {
    pub(super) fn paste_button(&mut self) {
        if self.paste.take().is_some() {
            self.paste_confirm = false;
            let _ = crate::panel::close();
            self.show_label("Paste canceled | Completed blocks were kept");
            return;
        }
        let Some((scope, placement)) = self.world.zip(self.placement) else {
            return;
        };
        if !std::mem::take(&mut self.paste_confirm) {
            self.paste_confirm = true;
            return;
        }
        self.paste = Some(Paste {
            scope,
            placement,
            destroy: self.options.paste_destroy,
            cursor: 0,
            sent: 0,
            skipped: 0,
            palette: HashMap::new(),
            structure: self.options.structure_paste.then(|| {
                StructurePaste::new(&self.schematic, placement, self.options.paste_destroy)
            }),
        });
        let _ = crate::panel::close();
    }

    pub(super) fn paste_step(&mut self) -> bool {
        let Some(mut run) = self.paste.take() else {
            return false;
        };
        if self.world != Some(run.scope) || self.placement != Some(run.placement) {
            self.show_label("Paste canceled because the world or placement changed");
            return true;
        }
        if !gameplay::can_paste_blocks() {
            self.show_label(
                "Paste needs Creative, operator cheats, and setblock and fill mod permissions",
            );
            return true;
        }
        if let Some(structure) = &mut run.structure {
            match structure.step(&self.schematic, run.placement) {
                Ok(Some(progress)) => {
                    self.show_label(&progress);
                    self.paste = Some(run);
                }
                Ok(None) => self.show_label("Structure sent | The host reports placement in chat"),
                Err(error) => self.show_label(&format!("Structure paste stopped | {error}")),
            }
            return true;
        }
        let [width, height, depth] = self.schematic.size().map(|size| size as usize);
        let volume = width * height * depth;
        for _ in 0..4 {
            while run.cursor < volume {
                if crate::budget::remaining_fuel() < 500_000 {
                    self.paste = Some(run);
                    return true;
                }
                let local = coordinates(run.cursor, width, depth);
                let palette_index = self.schematic.palette_index(local).unwrap();
                if self.schematic.is_air_index(palette_index)
                    || self.schematic.palette()[palette_index as usize].is_placement_artifact()
                {
                    run.cursor += 1;
                    continue;
                }
                let mut line_end = width as i32 - 1;
                if run.destroy {
                    let last = (local[0] + 31).min(line_end);
                    let [a, b] = [local, [last, local[1], local[2]]]
                        .map(|cell| run.placement.local_to_world(self.schematic.size(), cell));
                    let min: [i32; 3] = std::array::from_fn(|i| a[i].min(b[i]));
                    let max: [i32; 3] = std::array::from_fn(|i| a[i].max(b[i]) + 1);
                    let point = |[x, y, z]: [i32; 3]| terrain::Position { x, y, z };
                    let sample = terrain::read_region(
                        terrain::RegionQuery {
                            min: point(min),
                            max: point(max),
                            geometry: terrain::Geometry::empty(),
                        },
                        None,
                    );
                    let region = match sample {
                        Ok(terrain::ReadResult::Changed(region)) => region,
                        Ok(_) | Err(terrain::ReadError::BudgetExhausted) => {
                            self.paste = Some(run);
                            return true;
                        }
                        Err(_) => {
                            self.show_label("Paste stopped | Move closer to the destination");
                            return true;
                        }
                    };
                    let block = run
                        .placement
                        .transform_state(&self.schematic.palette()[palette_index as usize]);
                    let expected =
                        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                            &overlay::canonical(&block),
                        )
                        .unwrap();
                    let eligible = |x| {
                        let world = run
                            .placement
                            .local_to_world(self.schematic.size(), [x, local[1], local[2]]);
                        let cell =
                            ((world[0] - min[0]) * (max[2] - min[2]) + world[2] - min[2]) as usize;
                        needs_replace(&region, cell, &block.name, &expected)
                    };
                    if !eligible(local[0]) {
                        run.cursor += 1;
                        continue;
                    }
                    line_end = local[0];
                    while line_end < last
                        && self
                            .schematic
                            .palette_index([line_end + 1, local[1], local[2]])
                            == Some(palette_index)
                        && eligible(line_end + 1)
                    {
                        line_end += 1;
                    }
                }
                let formatted = run.palette.entry(palette_index).or_insert_with(|| {
                    format_state(
                        &run.placement
                            .transform_state(&self.schematic.palette()[palette_index as usize]),
                    )
                });
                let Some(formatted) = formatted else {
                    run.cursor += 1;
                    run.skipped += 1;
                    continue;
                };
                let (command, cells) = command(
                    &self.schematic,
                    run.placement,
                    local,
                    palette_index,
                    formatted,
                    run.destroy,
                    line_end,
                );
                let Some(command) = command else {
                    run.cursor += 1;
                    run.skipped += 1;
                    continue;
                };
                match gameplay::request_command(&command) {
                    Ok(()) => {
                        run.cursor += cells;
                        run.sent += cells;
                        break;
                    }
                    Err(error) if error == "command rate limit reached" => {
                        self.paste = Some(run);
                        return true;
                    }
                    Err(error) => {
                        self.show_label(&format!("Paste stopped | {error}"));
                        return true;
                    }
                }
            }
            if run.cursor == volume {
                self.show_label(&format!(
                    "Paste sent {} cells | {} skipped",
                    run.sent, run.skipped
                ));
                return true;
            }
        }
        self.show_label(&format!("Paste | {} of {volume} cells checked", run.cursor));
        self.paste = Some(run);
        true
    }
}

fn coordinates(index: usize, width: usize, depth: usize) -> [i32; 3] {
    [
        (index % width) as i32,
        (index / (width * depth)) as i32,
        ((index / width) % depth) as i32,
    ]
}

fn command(
    schematic: &Schematic,
    placement: Placement,
    local: [i32; 3],
    index: u32,
    state: &str,
    destroy: bool,
    line_end: i32,
) -> (Option<String>, usize) {
    let mode = if destroy { "replace" } else { "keep" };
    let [x, y, z] = local;
    let size = schematic.size();
    let start = placement.local_to_world(size, local);
    let mut end = x;
    while end < line_end && schematic.palette_index([end + 1, y, z]) == Some(index) {
        end += 1;
    }
    if end > x {
        let tail = placement.local_to_world(size, [end, y, z]);
        let [ax, ay, az] = std::array::from_fn(|axis| start[axis].min(tail[axis]));
        let [bx, by, bz] = std::array::from_fn(|axis| start[axis].max(tail[axis]));
        let fill = format!("/fill {ax} {ay} {az} {bx} {by} {bz} {state} {mode}");
        if fill.len() <= COMMAND_BYTES {
            return (Some(fill), (end - x + 1) as usize);
        }
    }
    let [x, y, z] = start;
    let setblock = format!("/setblock {x} {y} {z} {state} {mode}");
    ((setblock.len() <= COMMAND_BYTES).then_some(setblock), 1)
}

/// Never replace an unreadable cell or a matching state. Omitted or
/// neighbor-derived properties do not make an otherwise correct block wrong.
fn needs_replace(
    region: &terrain::Region,
    cell: usize,
    name: &str,
    expected: &serde_json::Map<String, serde_json::Value>,
) -> bool {
    if !region
        .loaded
        .get(cell / 64)
        .is_some_and(|mask| mask & (1 << (cell % 64)) != 0)
    {
        return false;
    }
    let Some(block) = region
        .states
        .get(cell)
        .and_then(|&index| region.palette.get(index as usize))
    else {
        return false;
    };
    let Some(actual_name) = block.identifier.as_deref() else {
        return false;
    };
    if actual_name != name {
        return true;
    }
    let Some(actual) = block.canonical_state.as_deref().and_then(|data| {
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(data).ok()
    }) else {
        return false;
    };
    expected.iter().any(|(property, value)| {
        placement_property(property) && actual.get(property) != Some(value)
    })
}

pub(super) fn placement_property(name: &str) -> bool {
    !name.starts_with("minecraft:connection_")
        && name != "minecraft:corner"
        && !name.starts_with("wall_connection_type_")
        && name != "wall_post_bit"
}

fn format_state(block: &BlockState) -> Option<String> {
    let safe = |text: &str| {
        !text.is_empty()
            && text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_:+.-".contains(&b))
    };
    if !block.name.starts_with("minecraft:") || !safe(&block.name) {
        return None;
    }
    let mut properties = Vec::new();
    for (name, value) in &block.states {
        if !placement_property(name) {
            continue;
        }
        if !safe(name) {
            return None;
        }
        let value = match value {
            StateValue::Str(value) if safe(value) => format!("\"{value}\""),
            StateValue::Str(_) => return None,
            StateValue::Byte(0) => "false".into(),
            StateValue::Byte(1) => "true".into(),
            value => value.to_string(),
        };
        properties.push(format!("\"{name}\"={value}"));
    }
    Some(if properties.is_empty() {
        block.name.clone()
    } else {
        format!("{} [{}]", block.name, properties.join(","))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_preserve_halves_and_keep_existing_blocks() {
        let block = BlockState::new("minecraft:stone_stairs")
            .with("upside_down_bit", StateValue::Byte(1))
            .with("weirdo_direction", StateValue::Int(2));
        assert_eq!(
            format_state(&block).unwrap(),
            "minecraft:stone_stairs [\"upside_down_bit\"=true,\"weirdo_direction\"=2]"
        );
        assert!(format_state(&BlockState::new("minecraft:stone\n/fill")).is_none());
    }
}
