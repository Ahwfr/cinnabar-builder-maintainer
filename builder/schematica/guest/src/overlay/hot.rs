use super::*;

/// Edge of the box that is watched around the camera every frame. The box is snapped to
/// `SNAP`-block steps, so it keeps one terrain query while the player moves a block or two, and
/// it always reaches at least six blocks past the eye on every side: beyond placement reach.
const EXTENT: i32 = 16;
const SNAP: i32 = 2;
const HALF: i32 = 8;
/// Cells in one watched box.
const CELLS: usize = (EXTENT * EXTENT * EXTENT) as usize;
/// Cells the first check of a box verifies per frame.
const SWEEP_CELLS: usize = 600;
/// A runtime id that no block has: the cell was not loaded when it was last seen.
const UNKNOWN: u32 = u32::MAX;

/// Colors of the status boxes, the same as the ones tiles draw.
fn marker_color(status: CellStatus) -> Option<(f32, f32, f32, f32)> {
    match status {
        CellStatus::Wrong => Some((1., 0.22, 0.42, 0.44)),
        CellStatus::WrongState => Some((1., 0.69, 0.08, 0.42)),
        CellStatus::Extra => Some((0.98, 0.14, 0.17, 0.40)),
        _ => None,
    }
}

/// A cell of the watched box whose block changed, with what the guide makes of it now.
pub(crate) struct HotChange {
    pub world: [i32; 3],
    pub status: CellStatus,
    pub skip: bool,
}

/// The block that stood in every cell of the watched box the last time it changed.
struct Seen {
    min: [i32; 3],
    ids: Vec<u32>,
}

impl Seen {
    fn at(&self, world: [i32; 3]) -> Option<u32> {
        let [x, y, z] = std::array::from_fn(|axis| world[axis] - self.min[axis]);
        ((0..EXTENT).contains(&x) && (0..EXTENT).contains(&y) && (0..EXTENT).contains(&z))
            .then(|| self.ids[cell_index([x, y, z])])
            .filter(|&id| id != UNKNOWN)
    }
}

fn cell_index([x, y, z]: [i32; 3]) -> usize {
    ((x * EXTENT + z) * EXTENT + y) as usize
}

/// The world position of box cell `cell` (the terrain order: x, then z, then y).
fn cell_world(origin: [i32; 3], cell: usize) -> [i32; 3] {
    let edge = EXTENT as usize;
    [
        origin[0] + (cell / (edge * edge)) as i32,
        origin[1] + (cell % edge) as i32,
        origin[2] + (cell / edge % edge) as i32,
    ]
}

/// The minimum corner of the box watched around `eye`.
pub(crate) fn watch_origin(eye: [f32; 3]) -> [i32; 3] {
    std::array::from_fn(|axis| (eye[axis].floor() as i32 - HALF).div_euclid(SNAP) * SNAP)
}

/// The cells of the box at `origin` that hold a different known block than `seen` recorded.
/// A cell that was not loaded before is no change: the tiles verify it.
fn changed_cells(seen: &Seen, ids: &[u32], origin: [i32; 3]) -> Vec<(usize, [i32; 3])> {
    ids.iter()
        .enumerate()
        .filter(|&(_, &now)| now != UNKNOWN)
        .filter_map(|(cell, &now)| {
            let world = cell_world(origin, cell);
            seen.at(world)
                .is_some_and(|before| before != now)
                .then_some((cell, world))
        })
        .collect()
}

/// Watches the blocks around the player every frame and updates only the cells that changed:
/// a ghost goes away the frame a correct block lands in it and comes back the frame its
/// block is broken, and a wrong or wrongly rotated block gets its red or yellow box at once.
/// The tiles still own the complete verification; this only gets there sooner and never
/// rebuilds the hologram, only edits single cells of it.
#[derive(Default)]
pub(crate) struct HotWatch {
    stamp: Option<terrain::Stamp>,
    origin: Option<[i32; 3]>,
    seen: Option<Seen>,
    /// Boxes for cells that are wrong now and that no tile draws yet.
    marks: BTreeMap<[i32; 3], CellStatus>,
    marker: Option<(DrawPair, BTreeMap<[i32; 3], CellStatus>)>,
    /// The latest sample of the box, kept so that the cells of the first check can be
    /// verified over several frames.
    region: Option<terrain::Region>,
    /// The next cell of the box the first check verifies; `CELLS` when it is done.
    cursor: usize,
    /// Comparison results by planned block and world block, which only change with the
    /// registry or the guide.
    cache: HashMap<(u32, u32), (CellStatus, bool)>,
}

impl HotWatch {
    pub(crate) fn clear(&mut self) {
        if let Some((pair, _)) = self.marker.take() {
            pair.remove();
        }
        *self = Self::default();
    }

    /// Reads the box around `eye` and applies whatever changed in it. Returns the changed
    /// cells so the tiles that hold them can be verified next.
    pub(crate) fn step(
        &mut self,
        preview: &Preview,
        schematic: &Schematic,
        whole: &mut WholeGhost,
        tiles: &[Tile],
        dimension: i32,
        eye: [f32; 3],
    ) -> Vec<HotChange> {
        let origin = watch_origin(eye);
        if self.origin != Some(origin) {
            self.origin = Some(origin);
            self.stamp = None;
            self.region = None;
            self.cursor = 0;
        }
        let mut changes = Vec::new();
        if budget::remaining_fuel() >= 650_000 {
            let query = terrain::RegionQuery {
                min: position(origin),
                max: position(origin.map(|axis| axis + EXTENT)),
                geometry: terrain::Geometry::empty(),
            };
            if let Ok(terrain::ReadResult::Changed(region)) =
                terrain::read_region(query, self.stamp)
            {
                self.stamp = Some(region.stamp);
                changes = self.observe(preview, schematic, whole, origin, &region);
                self.region = Some(region);
            }
            self.verify_box(preview, schematic, whole, origin);
        }
        self.maintain_marks(tiles, origin);
        self.publish_marks(dimension, origin);
        changes
    }

    fn observe(
        &mut self,
        preview: &Preview,
        schematic: &Schematic,
        whole: &mut WholeGhost,
        origin: [i32; 3],
        region: &terrain::Region,
    ) -> Vec<HotChange> {
        let runtime: Vec<u32> = region
            .palette
            .iter()
            .map(|block| block.runtime_id)
            .collect();
        let mut ids = vec![UNKNOWN; CELLS];
        for (cell, id) in ids.iter_mut().enumerate() {
            let loaded = region
                .loaded
                .get(cell / 64)
                .is_some_and(|bits| bits & (1 << (cell % 64)) != 0);
            if loaded
                && let Some(&state) = region.states.get(cell)
                && let Some(&runtime_id) = runtime.get(usize::from(state))
            {
                *id = runtime_id;
            }
        }
        let mut changes = Vec::new();
        let mut cut = false;
        if let Some(seen) = &self.seen
            && whole.active()
        {
            for (cell, world) in changed_cells(seen, &ids, origin) {
                if budget::remaining_fuel() < 450_000 {
                    cut = true;
                    break;
                }
                if let Some((status, skip)) =
                    self.verify_cell(preview, schematic, whole, region, cell, world)
                {
                    changes.push(HotChange {
                        world,
                        status,
                        skip,
                    });
                }
            }
        }
        if cut {
            // Out of fuel part-way: keep the old snapshot and read the box again next frame,
            // so the cells that were not reached are still found.
            self.stamp = None;
        } else {
            self.seen = Some(Seen { min: origin, ids });
        }
        changes
    }

    /// Checks one cell of the box against the guide and updates its ghost and status box.
    /// Returns what the guide makes of the cell when it covers it and it was loaded.
    fn verify_cell(
        &mut self,
        preview: &Preview,
        schematic: &Schematic,
        whole: &mut WholeGhost,
        region: &terrain::Region,
        cell: usize,
        world: [i32; 3],
    ) -> Option<(CellStatus, bool)> {
        let size = schematic.size();
        let (lo, hi) = preview.bounds(size);
        if !(0..3).all(|axis| (lo[axis]..hi[axis]).contains(&world[axis]))
            || !region
                .loaded
                .get(cell / 64)
                .is_some_and(|bits| bits & (1 << (cell % 64)) != 0)
        {
            return None;
        }
        let Some(local) = preview
            .placement
            .and_then(|placement| placement.world_to_local(size, world))
        else {
            return None;
        };
        let Some(id) = schematic.palette_index(local) else {
            return None;
        };
        if schematic.palette()[id as usize].is_placement_artifact() {
            return None;
        }
        let Some(runtime_id) = region
            .states
            .get(cell)
            .and_then(|&state| region.palette.get(usize::from(state)))
            .map(|block| block.runtime_id)
        else {
            return None;
        };
        let air = schematic.is_air_index(id);
        let (status, skip) = *self
            .cache
            .entry((id, runtime_id))
            .or_insert_with(|| compare_cell(region, cell, &preview.palette[id as usize], air));
        if status == CellStatus::Unloaded {
            return None;
        }
        if !air {
            let pos = std::array::from_fn(|axis| (world[axis] - lo[axis]) as u32);
            let [x, y, z] = pos.map(|axis| axis as usize);
            let (gy, gz) = ((hi[1] - lo[1]) as usize, (hi[2] - lo[2]) as usize);
            whole.apply_observation((x * gz + z) * gy + y, pos, id, status);
        }
        if marker_color(status).is_some() && !skip {
            self.marks.insert(world, status);
        } else {
            self.marks.remove(&world);
        }
        Some((status, skip))
    }

    /// The first check of a box: the diff above only sees blocks that change, so this goes
    /// over the blocks that were already there, a few hundred cells a frame, instead of
    /// leaving their ghosts to the tile verifier's slower lap. Air cells are skipped (the
    /// tiles report extra blocks).
    fn verify_box(
        &mut self,
        preview: &Preview,
        schematic: &Schematic,
        whole: &mut WholeGhost,
        origin: [i32; 3],
    ) {
        if self.cursor >= CELLS || !whole.active() {
            return;
        }
        let Some(region) = self.region.take() else {
            return;
        };
        if self.cache.len() > 4096 {
            self.cache.clear();
        }
        let mut done = 0;
        while self.cursor < CELLS && done < SWEEP_CELLS && budget::remaining_fuel() >= 600_000 {
            let cell = self.cursor;
            self.cursor += 1;
            done += 1;
            self.verify_cell(preview, schematic, whole, &region, cell, cell_world(origin, cell));
        }
        self.region = Some(region);
    }

    /// Drops the boxes that a tile draws by now, and those that left the watched box.
    fn maintain_marks(&mut self, tiles: &[Tile], origin: [i32; 3]) {
        self.marks.retain(|&cell, &mut status| {
            (0..3).all(|axis| (origin[axis]..origin[axis] + EXTENT).contains(&cell[axis]))
                && match tiles.iter().find(|tile| tile.contains(cell)) {
                    // A tile whose boxes are out of date does not draw this one yet.
                    Some(tile) => tile.markers_stale || tile.status_at(cell) != Some(status),
                    None => true,
                }
        });
    }

    fn publish_marks(&mut self, dimension: i32, origin: [i32; 3]) {
        let drawn = self.marker.as_ref().map(|(_, cells)| cells);
        if (drawn.is_none() && self.marks.is_empty()) || drawn == Some(&self.marks) {
            return;
        }
        if self.marks.is_empty() {
            if let Some((pair, _)) = self.marker.take() {
                pair.remove();
            }
            return;
        }
        if budget::remaining_fuel() < 450_000 {
            return;
        }
        let mut mesh = draw::Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
        for (&cell, &status) in &self.marks {
            if let Some(color) = marker_color(status) {
                let local = std::array::from_fn(|axis| cell[axis] - origin[axis]);
                add_box(&mut mesh, local, color, 0b11_1111);
            }
        }
        let anchor = draw::BlockAnchor {
            dimension,
            position: origin.into(),
        };
        // The new boxes replace the old ones in the same frame, so there is never a gap.
        if let Ok(pair) = create(draw::GeometrySpec::Triangles(mesh), anchor, 0., true)
            && let Some((previous, _)) = self.marker.replace((pair, self.marks.clone()))
        {
            previous.remove();
        }
    }
}

#[test]
fn the_watched_box_always_reaches_past_placement_reach() {
    for eye in [-33.4_f32, -1., -0.1, 0., 0.9, 1., 17.3, 100.99] {
        let origin = watch_origin([eye; 3])[0];
        let cell = eye.floor() as i32;
        assert!(cell - origin >= 8, "{eye}: {} blocks below", cell - origin);
        assert!(origin + EXTENT - 1 - cell >= 6, "{eye}: too little above");
    }
    assert_eq!(watch_origin([0.5; 3]), watch_origin([1.5; 3]));
    // The most a terrain query may span on one axis.
    assert!(EXTENT <= 16);
}

#[test]
fn a_cell_that_was_not_loaded_before_is_not_a_change() {
    let mut ids = vec![UNKNOWN; CELLS];
    ids[cell_index([1, 2, 3])] = 7;
    let seen = Seen {
        min: [10, 20, 30],
        ids,
    };
    assert_eq!(seen.at([11, 22, 33]), Some(7));
    assert_eq!(seen.at([10, 20, 30]), None);
    assert_eq!(seen.at([24, 20, 30]), None);
    assert_eq!(seen.at([9, 20, 30]), None);
}

#[test]
fn only_wrong_blocks_get_boxes() {
    assert!(marker_color(CellStatus::WrongState).is_some());
    assert!(marker_color(CellStatus::Wrong).is_some());
    assert!(marker_color(CellStatus::Extra).is_some());
    assert!(marker_color(CellStatus::Correct).is_none());
    assert!(marker_color(CellStatus::Missing).is_none());
    assert!(marker_color(CellStatus::Unloaded).is_none());
}

#[test]
fn only_cells_with_a_different_known_block_are_changes() {
    let origin = [10, 20, 30];
    let mut before = vec![UNKNOWN; CELLS];
    before[cell_index([1, 2, 3])] = 5;
    before[cell_index([4, 5, 6])] = 5;
    before[cell_index([7, 7, 7])] = 5;
    let seen = Seen {
        min: origin,
        ids: before,
    };
    let mut now = vec![UNKNOWN; CELLS];
    now[cell_index([1, 2, 3])] = 0; // broken
    now[cell_index([4, 5, 6])] = 5; // untouched
    now[cell_index([7, 7, 7])] = UNKNOWN; // unloaded since
    now[cell_index([9, 9, 9])] = 6; // not known before
    let changed = changed_cells(&seen, &now, origin);
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].1, [11, 22, 33]);
    assert_eq!(changed[0].0, cell_index([1, 2, 3]));
}

#[test]
fn a_moved_box_compares_only_the_overlap() {
    let mut before = vec![UNKNOWN; CELLS];
    before.iter_mut().for_each(|id| *id = 1);
    let seen = Seen {
        min: [0, 0, 0],
        ids: before,
    };
    // Shifted two blocks along x: the last two columns of the new box are outside the old one.
    let mut now = vec![1; CELLS];
    now[cell_index([EXTENT - 1, 0, 0])] = 2;
    now[cell_index([0, 0, 0])] = 2;
    let changed = changed_cells(&seen, &now, [2, 0, 0]);
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].1, [2, 0, 0]);
}

#[test]
fn box_cells_map_to_world_positions_in_terrain_order() {
    let origin = [10, 20, 30];
    for local in [[0, 0, 0], [1, 2, 3], [15, 15, 15], [4, 0, 9]] {
        let [x, y, z] = local;
        assert_eq!(
            cell_world(origin, cell_index(local)),
            [10 + x, 20 + y, 30 + z]
        );
    }
}
