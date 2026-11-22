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
/// Fuel the watch keeps for the rest of the frame while it handles changed cells.
const CUT_FUEL: u64 = 300_000;
/// Fuel a frame needs left before the watch reads the box at all.
const STEP_FUEL: u64 = 400_000;
/// Fuel the first check of a box needs left to go on.
const SWEEP_FUEL: u64 = 450_000;

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

/// What the watch has done, for the debug summary.
#[derive(Default, Clone, Copy)]
pub(crate) struct HotStats {
    /// Frames the watch read the box.
    pub steps: u32,
    /// Frames it could not, for lack of fuel.
    pub starved: u32,
    /// Samples it compared with the last one.
    pub compared: u32,
    /// Of those, compared column by column in place (the cheap case).
    pub in_place: u32,
    /// Changed cells it handled.
    pub changes: u32,
    /// Times it ran out of fuel part-way through a sample.
    pub cut: u32,
    /// Fuel the latest comparison cost, and the dearest one.
    pub last_cost: u64,
    pub worst_cost: u64,
}

/// The box as it was the last time every change in it had been handled. It keeps the raw sample
/// (palette index per cell) rather than a block id per cell, so keeping it costs one copy.
struct Seen {
    min: [i32; 3],
    states: Vec<u16>,
    loaded: Vec<u64>,
    /// Runtime id of each palette index of that sample.
    runtime: Vec<u32>,
}

fn loaded_bit(words: &[u64], cell: usize) -> bool {
    words
        .get(cell / 64)
        .is_some_and(|word| word & (1 << (cell % 64)) != 0)
}

impl Seen {
    fn of(origin: [i32; 3], region: &terrain::Region, runtime: Vec<u32>) -> Option<Self> {
        Some(Self {
            min: origin,
            states: region.states.get(..CELLS)?.to_vec(),
            loaded: region.loaded.clone(),
            runtime,
        })
    }

    /// The block that stood at box cell `cell`, when that cell was loaded.
    fn id(&self, cell: usize) -> Option<u32> {
        loaded_bit(&self.loaded, cell)
            .then(|| self.states.get(cell))
            .flatten()
            .and_then(|&state| self.runtime.get(usize::from(state)))
            .copied()
    }

    /// The block that stood in a world cell, when it was inside the box and loaded.
    #[cfg(test)]
    fn at(&self, world: [i32; 3]) -> Option<u32> {
        let [x, y, z] = std::array::from_fn(|axis| world[axis] - self.min[axis]);
        ((0..EXTENT).contains(&x) && (0..EXTENT).contains(&y) && (0..EXTENT).contains(&z))
            .then(|| self.id(cell_index([x, y, z])))
            .flatten()
    }
}

/// The block a sample holds in box cell `cell`, when that cell was loaded.
fn sample_id(region: &terrain::Region, runtime: &[u32], cell: usize) -> Option<u32> {
    loaded_bit(&region.loaded, cell)
        .then(|| region.states.get(cell))
        .flatten()
        .and_then(|&state| runtime.get(usize::from(state)))
        .copied()
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

/// The cells of an unmoved box whose block changed since `seen`, when its palette and loaded
/// cells are the same: whole runs of 64 cells that are equal are passed with one comparison.
fn in_place_candidates(seen: &Seen, now: &[u16]) -> Vec<usize> {
    let mut cells = Vec::new();
    for word in 0..CELLS / 64 {
        let span = word * 64..word * 64 + 64;
        if now[span.clone()] == seen.states[span.clone()] {
            continue;
        }
        for cell in span {
            if now[cell] == seen.states[cell] || !loaded_bit(&seen.loaded, cell) {
                continue;
            }
            if let (Some(before), Some(after)) = (
                seen.runtime.get(usize::from(seen.states[cell])),
                seen.runtime.get(usize::from(now[cell])),
            ) && before != after
            {
                cells.push(cell);
            }
        }
    }
    cells
}

/// The cells, as indexes into the new box, that both boxes hold and whose block is different
/// and known in both. A column of cells that is identical is passed with one comparison when
/// the palette is the same.
fn overlap_candidates(
    seen: &Seen,
    region: &terrain::Region,
    runtime: &[u32],
    origin: [i32; 3],
) -> Vec<usize> {
    let mut cells = Vec::new();
    let lo: [i32; 3] = std::array::from_fn(|axis| origin[axis].max(seen.min[axis]));
    let hi: [i32; 3] = std::array::from_fn(|axis| (origin[axis] + EXTENT).min(seen.min[axis] + EXTENT));
    if (0..3).any(|axis| lo[axis] >= hi[axis]) {
        return cells;
    }
    let Some(now) = region.states.get(..CELLS) else {
        return cells;
    };
    let same_palette = seen.runtime == runtime;
    let length = (hi[1] - lo[1]) as usize;
    for x in lo[0]..hi[0] {
        for z in lo[2]..hi[2] {
            let fresh = cell_index([x - origin[0], lo[1] - origin[1], z - origin[2]]);
            let old = cell_index([x - seen.min[0], lo[1] - seen.min[1], z - seen.min[2]]);
            if same_palette && now[fresh..fresh + length] == seen.states[old..old + length] {
                continue;
            }
            for step in 0..length {
                if let (Some(after), Some(before)) = (
                    sample_id(region, runtime, fresh + step),
                    seen.id(old + step),
                ) && before != after
                {
                    cells.push(fresh + step);
                }
            }
        }
    }
    cells
}

/// A cheap hash for the comparison cache: its keys are two small integers, and the default
/// hash costs several times more fuel than the lookup is worth.
#[derive(Default, Clone, Copy)]
struct Mix(u64);

impl std::hash::Hasher for Mix {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u32(u32::from(byte));
        }
    }
    fn write_u32(&mut self, value: u32) {
        self.0 = (self.0.rotate_left(5) ^ u64::from(value)).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

type Comparisons = HashMap<(u32, u32), (CellStatus, bool), std::hash::BuildHasherDefault<Mix>>;

/// The minimum corner of the box watched around `eye`.
pub(crate) fn watch_origin(eye: [f32; 3]) -> [i32; 3] {
    std::array::from_fn(|axis| (eye[axis].floor() as i32 - HALF).div_euclid(SNAP) * SNAP)
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
    cache: Comparisons,
    pub(crate) stats: HotStats,
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
        if budget::remaining_fuel() >= STEP_FUEL {
            self.stats.steps += 1;
            let query = terrain::RegionQuery {
                min: position(origin),
                max: position(origin.map(|axis| axis + EXTENT)),
                geometry: terrain::Geometry::empty(),
            };
            if let Ok(terrain::ReadResult::Changed(region)) =
                terrain::read_region(query, self.stamp)
            {
                self.stamp = Some(region.stamp);
                let before = budget::remaining_fuel();
                changes = self.observe(preview, schematic, whole, origin, &region);
                let cost = before.saturating_sub(budget::remaining_fuel());
                self.stats.last_cost = cost;
                self.stats.worst_cost = self.stats.worst_cost.max(cost);
                self.stats.changes += changes.len() as u32;
                self.region = Some(region);
            }
            self.verify_box(preview, schematic, whole, origin);
        } else {
            self.stats.starved += 1;
        }
        self.maintain_marks(tiles, origin);
        self.publish_marks(dimension, origin);
        changes
    }

    /// Compares a new sample of the box with the last one whose changes were all handled, and
    /// applies each changed cell. When the box has not moved and the palette is the same, whole
    /// runs of 64 cells are skipped by one comparison each; otherwise the overlap of the old and
    /// new box is compared.
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
        let mut changes = Vec::new();
        let Some(now) = region.states.get(..CELLS) else {
            self.seen = None;
            return changes;
        };
        let mut cut = false;
        if whole.active()
            && let Some(seen) = &self.seen
        {
            self.stats.compared += 1;
            let cells = if seen.min == origin && seen.runtime == runtime && seen.loaded == region.loaded
            {
                self.stats.in_place += 1;
                in_place_candidates(seen, now)
            } else {
                overlap_candidates(seen, region, &runtime, origin)
            };
            for cell in cells {
                if budget::remaining_fuel() < CUT_FUEL {
                    cut = true;
                    break;
                }
                let world = cell_world(origin, cell);
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
            self.stats.cut += 1;
            self.stamp = None;
        } else {
            self.seen = Seen::of(origin, region, runtime);
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
        while self.cursor < CELLS && done < SWEEP_CELLS && budget::remaining_fuel() >= SWEEP_FUEL {
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

/// A snapshot whose cells hold the given palette indexes (`None` is a cell that was not loaded).
#[cfg(test)]
fn seen_of(min: [i32; 3], runtime: &[u32], cells: &[([i32; 3], Option<u16>)]) -> Seen {
    let mut states = vec![0; CELLS];
    let mut loaded = vec![u64::MAX; CELLS / 64];
    for &(local, state) in cells {
        let cell = cell_index(local);
        match state {
            Some(state) => states[cell] = state,
            None => loaded[cell / 64] &= !(1 << (cell % 64)),
        }
    }
    Seen {
        min,
        states,
        loaded,
        runtime: runtime.to_vec(),
    }
}

#[test]
fn a_cell_that_was_not_loaded_before_has_no_block() {
    let seen = seen_of(
        [10, 20, 30],
        &[0, 7],
        &[([1, 2, 3], Some(1)), ([0, 0, 0], None)],
    );
    assert_eq!(seen.at([11, 22, 33]), Some(7));
    assert_eq!(seen.at([10, 20, 30]), None);
    assert_eq!(seen.at([26, 20, 30]), None);
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

/// The cells (as world positions) whose block differs between the snapshot and a new sample of
/// a box at `origin`, found one cell at a time: the answer the fast comparisons must give.
#[cfg(test)]
fn reference_changes(seen: &Seen, after: &terrain::Region, origin: [i32; 3]) -> Vec<[i32; 3]> {
    let runtime: Vec<u32> = after.palette.iter().map(|block| block.runtime_id).collect();
    let mut changed: Vec<[i32; 3]> = (0..CELLS)
        .filter_map(|cell| {
            let world = cell_world(origin, cell);
            let (now, was) = (sample_id(after, &runtime, cell)?, seen.at(world)?);
            (now != was).then_some(world)
        })
        .collect();
    changed.sort_unstable();
    changed
}

/// A sample of a whole box that holds `base` everywhere but in the `edits`, with `unloaded` cells
/// missing and a palette of `names` blocks.
#[cfg(test)]
fn box_sample(base: u16, names: usize, edits: &[(usize, u16)], unloaded: &[usize]) -> terrain::Region {
    let list: Vec<String> = (0..names).map(|index| format!("minecraft:block_{index}")).collect();
    let names: Vec<&str> = list.iter().map(String::as_str).collect();
    let mut region = region_of(&names);
    region.states = vec![base; CELLS];
    region.loaded = vec![u64::MAX; CELLS / 64];
    for &(cell, state) in edits {
        region.states[cell] = state;
    }
    for &cell in unloaded {
        region.loaded[cell / 64] &= !(1 << (cell % 64));
    }
    region
}

#[cfg(test)]
fn snapshot(origin: [i32; 3], region: &terrain::Region) -> Seen {
    let runtime = region.palette.iter().map(|block| block.runtime_id).collect();
    Seen::of(origin, region, runtime).unwrap()
}

#[cfg(test)]
fn world_of(origin: [i32; 3], cells: Vec<usize>) -> Vec<[i32; 3]> {
    let mut worlds: Vec<_> = cells.into_iter().map(|cell| cell_world(origin, cell)).collect();
    worlds.sort_unstable();
    worlds
}

#[test]
fn an_unmoved_box_reports_only_the_cells_that_changed() {
    let origin = [10, 20, 30];
    let seen = snapshot(origin, &box_sample(0, 4, &[], &[]));
    // One cell placed, one broken back to the base block, one unchanged.
    let second = box_sample(0, 4, &[(5, 2), (900, 3), (4000, 0)], &[]);
    let runtime: Vec<u32> = second.palette.iter().map(|block| block.runtime_id).collect();
    assert!(seen.min == origin && seen.runtime == runtime && seen.loaded == second.loaded);
    let found = in_place_candidates(&seen, &second.states[..CELLS]);
    assert_eq!(
        world_of(origin, found),
        vec![cell_world(origin, 5), cell_world(origin, 900)]
    );
    assert_eq!(
        world_of(origin, in_place_candidates(&seen, &second.states[..CELLS])),
        reference_changes(&seen, &second, origin)
    );
}

#[test]
fn a_cell_that_was_not_loaded_is_not_a_change_either_way() {
    let origin = [0, 0, 0];
    let before = box_sample(1, 3, &[(7, 2)], &[7, 8]);
    let seen = snapshot(origin, &before);
    // Cell 7 was unloaded and loads with a block; cell 8 was unloaded and stays so; cell 9 was
    // loaded and breaks. Only the last is a change.
    let after = box_sample(1, 3, &[(7, 0), (9, 0)], &[8]);
    let found = overlap_candidates(&seen, &after, &[0, 1, 2], origin);
    assert_eq!(world_of(origin, found), vec![cell_world(origin, 9)]);
    assert_eq!(
        world_of(origin, overlap_candidates(&seen, &after, &[0, 1, 2], origin)),
        reference_changes(&seen, &after, origin)
    );
}

#[test]
fn a_moved_box_compares_only_the_overlap() {
    let first = box_sample(1, 3, &[], &[]);
    let seen = snapshot([0, 0, 0], &first);
    // Shifted two blocks along x: the first two columns of the new box are old cells.
    let second = box_sample(
        1,
        3,
        &[(cell_index([EXTENT - 1, 0, 0]), 2), (cell_index([0, 0, 0]), 2)],
        &[],
    );
    let found = overlap_candidates(&seen, &second, &[0, 1, 2], [2, 0, 0]);
    // The last column of the new box is outside the old one, so only the first cell changed.
    assert_eq!(world_of([2, 0, 0], found), vec![[2, 0, 0]]);
    // A box that shares nothing with the old one compares nothing.
    assert!(overlap_candidates(&seen, &second, &[0, 1, 2], [40, 0, 0]).is_empty());
}

#[test]
fn a_changed_palette_still_finds_the_right_cells() {
    // The same blocks, numbered differently: nothing changed. One cell really changed.
    let first = box_sample(0, 3, &[(11, 1)], &[]);
    let seen = snapshot([0, 0, 0], &first);
    let mut second = box_sample(0, 3, &[(11, 1)], &[]);
    // Swap the palette order of blocks 1 and 2 (runtime ids follow the blocks) and renumber.
    second.palette.swap(1, 2);
    for state in &mut second.states {
        *state = match *state {
            1 => 2,
            2 => 1,
            other => other,
        };
    }
    second.states[40] = 1; // runtime id 2 now stands here
    let runtime: Vec<u32> = second.palette.iter().map(|block| block.runtime_id).collect();
    assert!(seen.runtime != runtime);
    let found = overlap_candidates(&seen, &second, &runtime, [0, 0, 0]);
    assert_eq!(world_of([0, 0, 0], found), vec![cell_world([0, 0, 0], 40)]);
}

#[test]
fn the_fast_comparisons_agree_with_the_slow_one() {
    // A few hundred pseudo-random edits, unloaded cells and box shifts.
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move |modulus: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % modulus
    };
    for round in 0..40 {
        let edits_a: Vec<(usize, u16)> = (0..60).map(|_| (next(CELLS as u64) as usize, next(5) as u16)).collect();
        let unloaded_a: Vec<usize> = (0..20).map(|_| next(CELLS as u64) as usize).collect();
        let first = box_sample(0, 5, &edits_a, &unloaded_a);
        let origin = [3, -7, 11];
        let seen = snapshot(origin, &first);
        let edits_b: Vec<(usize, u16)> = edits_a
            .iter()
            .copied()
            .chain((0..30).map(|_| (next(CELLS as u64) as usize, next(5) as u16)))
            .collect();
        let unloaded_b: Vec<usize> = (0..20).map(|_| next(CELLS as u64) as usize).collect();
        let second = box_sample(0, 5, &edits_b, &unloaded_b);
        let runtime: Vec<u32> = second.palette.iter().map(|block| block.runtime_id).collect();
        // Same box: the overlap comparison and the reference agree, and so does the in-place one
        // whenever the loaded cells did not change.
        let slow = reference_changes(&seen, &second, origin);
        assert_eq!(
            world_of(origin, overlap_candidates(&seen, &second, &runtime, origin)),
            slow,
            "round {round}: overlap"
        );
        if seen.loaded == second.loaded {
            assert_eq!(
                world_of(origin, in_place_candidates(&seen, &second.states[..CELLS])),
                slow,
                "round {round}: in place"
            );
        }
        // A shifted box: the reference only counts cells both boxes hold.
        let shifted = [origin[0] + 2, origin[1], origin[2] - 2];
        assert_eq!(
            world_of(shifted, overlap_candidates(&seen, &second, &runtime, shifted)),
            reference_changes(&seen, &second, shifted),
            "round {round}: shifted"
        );
    }
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
