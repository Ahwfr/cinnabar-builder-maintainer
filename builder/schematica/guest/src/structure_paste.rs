use crate::gameplay;
use schematica_core::{Placement, Schematic, StateValue};
use serde_json::{Value, json};
use std::collections::HashMap;

const EDGE: i32 = 8;
// Stock BDS accepts 440-byte script commands and disconnects on larger payloads.
const FRAGMENT: usize = 399;
const PREFIX: &str = "/scriptevent cinnamatica:structure";

pub(super) struct StructurePaste {
    min: [i32; 3],
    max: [i32; 3],
    next: [i32; 3],
    replace: bool,
    tile: Tile,
    sent: usize,
    started: bool,
}

#[derive(Default)]
struct Tile {
    cursor: usize,
    palette: Vec<Value>,
    states: HashMap<u32, Option<usize>>,
    runs: Vec<(usize, usize, usize)>,
    payload: String,
    offset: usize,
    part: usize,
}

impl StructurePaste {
    pub(super) fn new(schematic: &Schematic, placement: Placement, replace: bool) -> Self {
        let (min, max) = placement.world_bounds(schematic.size());
        Self {
            min,
            max,
            next: min,
            replace,
            tile: Tile::default(),
            sent: 0,
            started: false,
        }
    }

    pub(super) fn step(
        &mut self,
        schematic: &Schematic,
        placement: Placement,
    ) -> Result<Option<String>, String> {
        if !self.started {
            match gameplay::request_command(&format!("{PREFIX} begin")) {
                Ok(()) => self.started = true,
                Err(error) if error == "command rate limit reached" => {}
                Err(error) => return Err(error),
            }
            return Ok(Some("Preparing Structure Paste".into()));
        }
        if self.next[1] >= self.max[1] {
            return match gameplay::request_command(&format!("{PREFIX} done")) {
                Ok(()) => Ok(None),
                Err(error) if error == "command rate limit reached" => {
                    Ok(Some("Finishing Structure Paste".into()))
                }
                Err(error) => Err(error),
            };
        }
        let size: [i32; 3] = std::array::from_fn(|i| (self.max[i] - self.next[i]).min(EDGE));
        let [sx, sy, sz] = size.map(|axis| axis as usize);
        let volume = sx * sy * sz;
        let tile = &mut self.tile;
        if tile.payload.is_empty() {
            let end = (tile.cursor + 512).min(volume);
            while tile.cursor < end && crate::budget::remaining_fuel() > 900_000 {
                let cell = tile.cursor;
                tile.cursor += 1;
                let offset = [
                    (cell % sx) as i32,
                    (cell / (sx * sz)) as i32,
                    ((cell / sx) % sz) as i32,
                ];
                let world = std::array::from_fn(|i| self.next[i] + offset[i]);
                let local = placement.world_to_local(schematic.size(), world).unwrap();
                let index = schematic.palette_index(local).unwrap();
                let state = tile.states.entry(index).or_insert_with(|| {
                    let block = &schematic.palette()[index as usize];
                    if block.is_air()
                        || block.is_placement_artifact()
                        || !block.name.starts_with("minecraft:")
                    {
                        return None;
                    }
                    let block = placement.transform_state(block);
                    let properties: serde_json::Map<String, Value> = block
                        .states
                        .into_iter()
                        .filter(|(name, _)| super::paste::placement_property(name))
                        .map(|(name, value)| {
                            (
                                name,
                                match value {
                                    StateValue::Bool(value) => json!(value),
                                    StateValue::Byte(0) => json!(false),
                                    StateValue::Byte(1) => json!(true),
                                    StateValue::Byte(value) => json!(value),
                                    StateValue::Int(value) => json!(value),
                                    StateValue::Str(value) => json!(value),
                                },
                            )
                        })
                        .collect();
                    let value = json!([block.name, properties]);
                    if !value
                        .to_string()
                        .bytes()
                        .all(|byte| (0x21..0x7f).contains(&byte))
                    {
                        return None;
                    }
                    let id = tile.palette.len();
                    tile.palette.push(value);
                    Some(id)
                });
                if let Some(id) = *state {
                    if let Some((start, count, previous)) = tile.runs.last_mut()
                        && *previous == id
                        && *start + *count == cell
                    {
                        *count += 1;
                    } else {
                        tile.runs.push((cell, 1, id));
                    }
                }
            }
            if tile.cursor < volume {
                return Ok(Some(format!(
                    "Preparing structure | {} sections sent",
                    self.sent
                )));
            }
            if tile.runs.is_empty() {
                self.advance();
                return Ok(Some(format!(
                    "Preparing structure | {} sections sent",
                    self.sent
                )));
            }
            tile.payload =
                serde_json::to_string(&(self.next, size, self.replace, &tile.palette, &tile.runs))
                    .map_err(|error| error.to_string())?;
            if tile.payload.len() > 384 * 1024 {
                return Err("Section exceeds Bedrock's paste transfer limit".into());
            }
        }
        for _ in 0..4 {
            let end = (tile.offset + FRAGMENT).min(tile.payload.len());
            let final_part = u8::from(end == tile.payload.len());
            let command = format!(
                "{PREFIX} {} {final_part} {}",
                tile.part,
                &tile.payload[tile.offset..end]
            );
            match gameplay::request_command(&command) {
                Ok(()) => {
                    tile.offset = end;
                    tile.part += 1;
                    if final_part == 1 {
                        self.sent += 1;
                        self.advance();
                        break;
                    }
                }
                Err(error) if error == "command rate limit reached" => break,
                Err(error) => return Err(error),
            }
        }
        Ok(Some(format!(
            "Structure paste | {} sections sent",
            self.sent
        )))
    }

    fn advance(&mut self) {
        self.tile = Tile::default();
        self.next[0] += EDGE;
        if self.next[0] >= self.max[0] {
            self.next[0] = self.min[0];
            self.next[2] += EDGE;
            if self.next[2] >= self.max[2] {
                self.next[2] = self.min[2];
                self.next[1] += EDGE;
            }
        }
    }
}
