//! Copies a box of the world into a new in-memory schematic, one terrain region at a time.

use crate::{budget, terrain};
use schematica_core::{BlockState, Schematic, StateValue};

/// Longest box edge and largest volume a capture accepts.
pub(crate) const MAX_EDGE: i32 = 64;
pub(crate) const MAX_VOLUME: i64 = 64 * 64 * 64;
/// Terrain reads cover at most this many blocks along one axis.
const CHUNK: i32 = 16;
/// Frames a region may stay pending before the capture gives up.
const MAX_PENDING_FRAMES: u32 = 240;
/// Fuel a frame keeps in reserve while applying a region.
const RESERVE_FUEL: u64 = 700_000;

pub(crate) enum Progress {
    Working(String),
    Done {
        schematic: Schematic,
        /// Cells the client had not loaded; they stay air in the copy.
        unloaded: u64,
        unknown: u64,
    },
    Failed(String),
}

struct Applying {
    lo: [i32; 3],
    size: [i32; 3],
    region: terrain::Region,
    cursor: usize,
    /// Per region palette entry: not yet converted, air or unusable, or the schematic index.
    palette: Vec<Option<Option<u32>>>,
}

pub(crate) struct Capture {
    min: [i32; 3],
    schematic: Schematic,
    chunks: Vec<([i32; 3], [i32; 3])>,
    next: usize,
    applying: Option<Applying>,
    pending_frames: u32,
    unloaded: u64,
    unknown: u64,
    pub(crate) scope: (u64, i32),
}

/// The box spanned by two corners, inclusive: its minimum corner and size.
pub(crate) fn bounds(a: [i32; 3], b: [i32; 3]) -> ([i32; 3], [i32; 3]) {
    let min = [0, 1, 2].map(|i| a[i].min(b[i]));
    let size = [0, 1, 2].map(|i| a[i].max(b[i]) - min[i] + 1);
    (min, size)
}

/// Whether a box is small enough to copy.
pub(crate) fn check_size(size: [i32; 3]) -> Result<(), String> {
    let volume: i64 = size.iter().map(|axis| i64::from(*axis)).product();
    if size.iter().any(|axis| *axis > MAX_EDGE) || volume > MAX_VOLUME {
        return Err(format!(
            "Selection is too large | At most {MAX_EDGE} blocks on a side"
        ));
    }
    Ok(())
}

/// Splits a box of `size` into terrain-sized pieces as `(low, high exclusive)` offsets.
pub(crate) fn chunks(size: [i32; 3]) -> Vec<([i32; 3], [i32; 3])> {
    let mut out = Vec::new();
    for x in (0..size[0]).step_by(CHUNK as usize) {
        for z in (0..size[2]).step_by(CHUNK as usize) {
            for y in (0..size[1]).step_by(CHUNK as usize) {
                let lo = [x, y, z];
                out.push((lo, [0, 1, 2].map(|i| (lo[i] + CHUNK).min(size[i]))));
            }
        }
    }
    out
}

/// Terrain cell order: `((x * size_z) + z) * size_y + y`, relative to the region minimum.
#[cfg(test)]
pub(crate) fn cell_index(size: [i32; 3], [x, y, z]: [i32; 3]) -> usize {
    (((x * size[2]) + z) * size[1] + y) as usize
}

/// Rebuilds a block state from the host's identifier and typed-JSON canonical state.
pub(crate) fn block_from_terrain(identifier: &str, canonical: Option<&str>) -> Option<BlockState> {
    let mut block = BlockState::new(identifier);
    let Some(canonical) = canonical else {
        return Some(block);
    };
    let value: serde_json::Value = serde_json::from_str(canonical).ok()?;
    for (name, typed) in value.as_object()? {
        let state = match typed["type"].as_str()? {
            "byte" => StateValue::Byte(i8::try_from(typed["value"].as_i64()?).ok()?),
            "int" => StateValue::Int(i32::try_from(typed["value"].as_i64()?).ok()?),
            "string" => StateValue::Str(typed["value"].as_str()?.to_owned()),
            _ => return None,
        };
        block = block.with(name, state);
    }
    Some(block)
}

impl Capture {
    pub(crate) fn new(
        a: [i32; 3],
        b: [i32; 3],
        scope: (u64, i32),
    ) -> Result<(Self, [i32; 3]), String> {
        let (min, size) = bounds(a, b);
        check_size(size)?;
        let name = format!("Selection {}x{}x{}", size[0], size[1], size[2]);
        Ok((
            Self {
                min,
                schematic: Schematic::new(name, size),
                chunks: chunks(size),
                next: 0,
                applying: None,
                pending_frames: 0,
                unloaded: 0,
                unknown: 0,
                scope,
            },
            min,
        ))
    }

    fn query(&self, lo: [i32; 3], hi: [i32; 3]) -> terrain::RegionQuery {
        let at = |offset: [i32; 3]| terrain::Position {
            x: self.min[0] + offset[0],
            y: self.min[1] + offset[1],
            z: self.min[2] + offset[2],
        };
        terrain::RegionQuery {
            min: at(lo),
            max: at(hi),
            geometry: terrain::Geometry::empty(),
        }
    }

    /// Advances the copy within this frame's fuel.
    pub(crate) fn step(&mut self) -> Progress {
        loop {
            if let Some(work) = &mut self.applying {
                let [sx, sy, sz] = work.size;
                let cells = (sx * sy * sz) as usize;
                while work.cursor < cells {
                    if work.cursor % 128 == 0 && budget::remaining_fuel() < RESERVE_FUEL {
                        return Progress::Working(self.progress());
                    }
                    let cell = work.cursor;
                    work.cursor += 1;
                    if !work
                        .region
                        .loaded
                        .get(cell / 64)
                        .is_some_and(|bits| bits & (1 << (cell % 64)) != 0)
                    {
                        self.unloaded += 1;
                        continue;
                    }
                    let Some(&entry) = work.region.states.get(cell) else {
                        self.unloaded += 1;
                        continue;
                    };
                    let Some(slot) = work.palette.get_mut(entry as usize) else {
                        continue;
                    };
                    if slot.is_none() {
                        let block = &work.region.palette[entry as usize];
                        *slot = Some(match block.identifier.as_deref() {
                            Some("minecraft:air" | "minecraft:cave_air" | "minecraft:void_air") => {
                                None
                            }
                            Some(identifier) => {
                                block_from_terrain(identifier, block.canonical_state.as_deref())
                                    .map(|state| self.schematic.intern_state(state))
                            }
                            None => None,
                        });
                        if block.identifier.is_none() {
                            self.unknown += 1;
                        }
                    }
                    if let Some(Some(index)) = slot {
                        let y = cell % sy as usize;
                        let z = (cell / sy as usize) % sz as usize;
                        let x = cell / (sy * sz) as usize;
                        let local = [
                            work.lo[0] + x as i32,
                            work.lo[1] + y as i32,
                            work.lo[2] + z as i32,
                        ];
                        self.schematic.set_index(local, *index);
                    }
                }
                self.applying = None;
                self.next += 1;
                self.pending_frames = 0;
                continue;
            }
            let Some(&(lo, hi)) = self.chunks.get(self.next) else {
                return Progress::Done {
                    schematic: std::mem::replace(&mut self.schematic, Schematic::new("", [1; 3])),
                    unloaded: self.unloaded,
                    unknown: self.unknown,
                };
            };
            if budget::remaining_fuel() < RESERVE_FUEL {
                return Progress::Working(self.progress());
            }
            match terrain::read_region(self.query(lo, hi), None) {
                Ok(terrain::ReadResult::Changed(region)) => {
                    let palette = vec![None; region.palette.len()];
                    self.applying = Some(Applying {
                        lo,
                        size: [0, 1, 2].map(|i| hi[i] - lo[i]),
                        region,
                        cursor: 0,
                        palette,
                    });
                }
                Ok(terrain::ReadResult::Pending | terrain::ReadResult::Unchanged(_)) => {
                    self.pending_frames += 1;
                    if self.pending_frames > MAX_PENDING_FRAMES {
                        return Progress::Failed(
                            "Selection copy stopped | The terrain did not load. Move closer."
                                .into(),
                        );
                    }
                    return Progress::Working(self.progress());
                }
                Err(error) => {
                    return Progress::Failed(format!(
                        "Selection copy stopped | Terrain {error:?}. Stay within 128 blocks."
                    ));
                }
            }
        }
    }

    fn progress(&self) -> String {
        format!(
            "Copying selection | {} of {} regions",
            self.next,
            self.chunks.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_in_any_order_span_an_inclusive_box() {
        assert_eq!(bounds([5, 70, -2], [2, 64, 3]), ([2, 64, -2], [4, 7, 6]));
        assert_eq!(bounds([1, 1, 1], [1, 1, 1]), ([1, 1, 1], [1, 1, 1]));
    }

    #[test]
    fn oversized_boxes_are_refused() {
        assert!(check_size([64, 64, 64]).is_ok());
        assert!(check_size([65, 1, 1]).is_err());
        assert!(check_size([64, 64, 65]).is_err());
    }

    #[test]
    fn chunks_tile_the_box_exactly_once_within_the_terrain_limit() {
        let size = [40, 20, 17];
        let mut covered = vec![0u8; (size[0] * size[1] * size[2]) as usize];
        for (lo, hi) in chunks(size) {
            assert!((0..3).all(|i| hi[i] - lo[i] <= CHUNK && hi[i] > lo[i]));
            for x in lo[0]..hi[0] {
                for y in lo[1]..hi[1] {
                    for z in lo[2]..hi[2] {
                        covered[((y * size[2] + z) * size[0] + x) as usize] += 1;
                    }
                }
            }
        }
        assert!(covered.iter().all(|count| *count == 1));
    }

    #[test]
    fn cells_follow_the_terrain_order() {
        let size = [2, 3, 4];
        let mut seen = vec![false; 24];
        for x in 0..2 {
            for z in 0..4 {
                for y in 0..3 {
                    let cell = cell_index(size, [x, y, z]);
                    assert!(!seen[cell]);
                    seen[cell] = true;
                    assert_eq!(cell % 3, y as usize);
                    assert_eq!((cell / 3) % 4, z as usize);
                    assert_eq!(cell / 12, x as usize);
                }
            }
        }
    }

    #[test]
    fn terrain_states_convert_to_blueprint_states() {
        let canonical = r#"{"upside_down_bit":{"type":"byte","value":1},"weirdo_direction":{"type":"int","value":2},"wood_type":{"type":"string","value":"oak"}}"#;
        let block = block_from_terrain("minecraft:oak_stairs", Some(canonical)).unwrap();
        assert_eq!(block.name, "minecraft:oak_stairs");
        assert_eq!(block.states["upside_down_bit"], StateValue::Byte(1));
        assert_eq!(block.states["weirdo_direction"], StateValue::Int(2));
        assert_eq!(block.states["wood_type"], StateValue::Str("oak".into()));
        assert_eq!(
            block_from_terrain("minecraft:stone", None).unwrap(),
            BlockState::new("minecraft:stone")
        );
        assert!(block_from_terrain("minecraft:stone", Some("{")).is_none());
        assert!(
            block_from_terrain(
                "minecraft:stone",
                Some(r#"{"a":{"type":"float","value":1}}"#)
            )
            .is_none()
        );
    }
}
