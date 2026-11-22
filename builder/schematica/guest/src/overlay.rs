use crate::{budget, drawing as draw, shapes, terrain};
use schematica_core::{BlockState, CellStatus, LayerMode, Placement, Schematic, StateValue, Tally};
use serde_json::{Map, json};
use std::collections::{BTreeMap, HashMap};

mod coverage;
mod hot;
pub(crate) use coverage::on_distant_scan_frame;
pub(crate) use hot::HotWatch;

const SMALL_TILE_EDGE: i32 = 4;
const LARGE_TILE_EDGE: i32 = 5;
const MAX_VISIBLE_TILES: i32 = 128;
const WINDOW_TILES: i32 = 5;
const DEFAULT_LIGHT: u32 = 0xffff0000;
/// How many frames a pasted cell's ghost stays hidden while the world still shows air: long
/// enough for the server to apply the paste, short enough that a refused one comes back.
const HOLD_FRAMES: u32 = 240;
// Lowering triangle vertices to WASM host calls dominates dense mismatch tiles.
// The verifier and material counts still track every cell.
/// Host block-grid parameter slots (`mod_api::drawing::BLOCK_GRID_*_PARAM`).
const OUTLINE_PARAM: usize = 10;
const WASH_PARAM: usize = 11;
const WASH_COLOR_PARAM: usize = 12;
/// The light blue that ghost blocks blend toward: #33B3E6, the hue Litematica uses for
/// missing blocks.
const GHOST_WASH: [f32; 3] = [0.2, 0.702, 0.902];
/// The colour of the frame drawn around a missing rail or chest.
const GHOST_FRAME: (f32, f32, f32, f32) = (0.2, 0.702, 0.902, 0.95);
/// How far status marker boxes extend past the block on each side.
const MARKER_INSET: f32 = 0.0075;
/// Most indices one marker mesh may hold. The host refuses a geometry past 196,608, and a
/// section that went over lost every box and outline it had.
const MARKER_INDEX_BUDGET: usize = 150_000;
/// Indices in the twelve-edge frame, and in the flat frame of a rail (four strips, each
/// written for both sides so it shows whichever way the host culls).
const FRAME_INDICES: usize = 12 * 36;
const FLAT_FRAME_INDICES: usize = 4 * 12;
/// Boxes at or below this height get the flat frame.
const FLAT_HEIGHT: f32 = 0.2;
/// Width of a flat frame's strips and of a frame's edges, in blocks. Thin lines vanish at a
/// distance, so these are wider than they look up close.
const FLAT_STRIP: f32 = 0.07;
const FRAME_EDGE: f32 = 0.04;

thread_local! {
    /// The dearest section check seen lately, in fuel.
    static RENDER_COST: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Remembers what a section check cost. Rises at once and falls slowly, so one dear check is
/// not forgotten by the next cheap one.
pub(crate) fn note_render_cost(cost: u64) {
    RENDER_COST.with(|seen| seen.set(cost.max(seen.get() / 16 * 15)));
}

/// Fuel a frame needs before a section is checked: what the check has cost lately, with
/// room to spare, between the old fixed floor and nearly a whole frame. A check that started
/// with too little left ran the mod out of fuel, which restarts it.
pub(crate) fn render_gate() -> u64 {
    (RENDER_COST.with(std::cell::Cell::get) / 2 * 3 + 150_000).clamp(575_000, 1_800_000)
}

/// The colour of the box that marks a wrong, wrongly turned or extra block.
fn status_color(status: CellStatus) -> Option<(f32, f32, f32, f32)> {
    match status {
        CellStatus::Wrong => Some((1., 0.22, 0.42, 0.44)),
        CellStatus::WrongState => Some((1., 0.69, 0.08, 0.42)),
        CellStatus::Extra => Some((0.98, 0.14, 0.17, 0.40)),
        _ => None,
    }
}

/// Which of a material's three counts (missing, correct, unloaded) a cell adds to.
fn material_slot(status: CellStatus, air: bool, ignored: bool) -> Option<usize> {
    match status {
        CellStatus::Missing if !air && !ignored => Some(0),
        CellStatus::Unloaded if !air && !ignored => Some(2),
        CellStatus::Wrong | CellStatus::WrongState => Some(0),
        CellStatus::Correct if !air && !ignored => Some(1),
        _ => None,
    }
}

#[derive(Default)]
pub struct Preview {
    placement: Option<Placement>,
    palette: Vec<draw::NamedBlock>,
    layers: Option<LayerMode>,
}

impl Preview {
    /// The block planned for schematic palette entry `index`.
    pub(crate) fn expected(&self, index: u32) -> Option<&draw::NamedBlock> {
        self.palette.get(index as usize)
    }

    pub fn reposition(&mut self, placement: Placement, layers: LayerMode) {
        if self.placement.is_none_or(|previous| {
            previous.quarter_turns != placement.quarter_turns || previous.mirror != placement.mirror
        }) {
            self.palette.clear();
        }
        self.placement = Some(placement);
        self.layers = Some(layers);
    }

    pub fn prepare(&mut self, schematic: &Schematic) -> bool {
        let placement = self.placement.unwrap();
        for state in schematic.palette().iter().skip(self.palette.len()).take(8) {
            if budget::remaining_fuel() < 500_000 {
                break;
            }
            let state = placement.transform_state(state);
            self.palette.push(draw::NamedBlock {
                states: canonical(&state),
                name: state.name,
            });
        }
        self.palette.len() == schematic.palette().len()
    }

    pub fn palette_ready(&self, schematic: &Schematic) -> bool {
        self.palette.len() == schematic.palette().len()
    }

    pub(super) fn palette_names(&self, start: usize, end: usize) -> impl Iterator<Item = &str> {
        self.palette[start..end]
            .iter()
            .map(|block| block.name.as_str())
    }

    /// The palette's storage and length, which change whenever it is rebuilt.
    pub fn palette_identity(&self) -> (usize, usize) {
        (self.palette.as_ptr() as usize, self.palette.len())
    }

    fn bounds(&self, size: [i32; 3]) -> ([i32; 3], [i32; 3]) {
        let p = self.placement.unwrap();
        let (mut min, mut max) = p.world_bounds(size);
        match self.layers.unwrap_or(LayerMode::All) {
            LayerMode::All => {}
            LayerMode::Single(layer) => {
                min[1] += layer;
                max[1] = min[1] + 1;
            }
            LayerMode::Range {
                min: bottom,
                max: top,
            } => {
                max[1] = min[1] + top + 1;
                min[1] += bottom;
            }
        }
        (min, max)
    }

    pub fn focus(&self, size: [i32; 3], eye: [f32; 3]) -> Option<[i32; 3]> {
        let (min, max) = self.bounds(size);
        let edge = tile_edge(min, max);
        let counts = tile_counts(min, max, edge);
        (counts.iter().product::<i32>() > MAX_VISIBLE_TILES).then(|| {
            [0, 1, 2].map(|i| {
                let cell = (eye[i].floor() as i32).clamp(min[i], max[i] - 1);
                let center = (cell - min[i]) / edge;
                (center - WINDOW_TILES / 2 + 1).clamp(0, (counts[i] - WINDOW_TILES).max(0))
            })
        })
    }

    pub fn tiles(&self, size: [i32; 3], focus: Option<[i32; 3]>) -> Vec<Tile> {
        let (min, max) = self.bounds(size);
        let edge = tile_edge(min, max);
        let (start, end) = self.scan_window(size, focus);
        let mut tiles = Vec::new();
        for x in (start[0]..end[0]).step_by(edge as usize) {
            for z in (start[2]..end[2]).step_by(edge as usize) {
                for y in (start[1]..end[1]).step_by(edge as usize) {
                    let lo = [x, y, z];
                    let hi = [0, 1, 2].map(|i| (lo[i] + edge).min(end[i]));
                    tiles.push(Tile {
                        lo,
                        hi,
                        ..Tile::default()
                    });
                }
            }
        }
        tiles
    }

    fn scan_window(&self, size: [i32; 3], focus: Option<[i32; 3]>) -> ([i32; 3], [i32; 3]) {
        let (min, bounds_max) = self.bounds(size);
        let edge = tile_edge(min, bounds_max);
        if let Some(focus) = focus {
            let start = [0, 1, 2].map(|i| min[i] + focus[i] * edge);
            let end = [0, 1, 2].map(|i| (start[i] + WINDOW_TILES * edge).min(bounds_max[i]));
            (start, end)
        } else {
            (min, bounds_max)
        }
    }
}

fn tile_counts(min: [i32; 3], max: [i32; 3], edge: i32) -> [i32; 3] {
    std::array::from_fn(|i| (max[i] - min[i] + edge - 1) / edge)
}

fn tile_edge(min: [i32; 3], max: [i32; 3]) -> i32 {
    if tile_counts(min, max, SMALL_TILE_EDGE)
        .iter()
        .product::<i32>()
        > MAX_VISIBLE_TILES
    {
        LARGE_TILE_EDGE
    } else {
        SMALL_TILE_EDGE
    }
}

/// Partial focus windows and tiles without a terrain result cannot finish a layer.
pub(super) fn layer_complete(tiles: &[Tile], width: i32, depth: i32) -> bool {
    let mut cells = 0;
    for tile in tiles {
        if tile.stamp.is_none()
            || tile.tally.missing != 0
            || tile.tally.wrong != 0
            || tile.tally.unloaded != 0
        {
            return false;
        }
        cells += (tile.hi[0] - tile.lo[0]) * (tile.hi[2] - tile.lo[2]);
    }
    cells == width * depth
}

#[cfg(test)]
#[test]
fn large_schematic_keeps_a_bounded_window_around_the_camera() {
    let size = [47, 45, 36];
    let mut preview = Preview::default();
    preview.reposition(Placement::new([0; 3]), LayerMode::All);
    let eye = [40., 35., 30.];
    let tiles = preview.tiles(size, preview.focus(size, eye));
    assert!(tiles.len() <= MAX_VISIBLE_TILES as usize);
    assert!(tiles.iter().any(|tile| tile.contains([40, 35, 30])));
    assert!(
        tiles
            .iter()
            .all(|tile| (0..3).all(|i| tile.hi[i] - tile.lo[i] <= LARGE_TILE_EDGE))
    );
}

#[cfg(test)]
#[test]
fn layer_completion_requires_all_tiles_and_correct_required_blocks() {
    let mut tile = Tile {
        hi: [4, 1, 2],
        stamp: Some(terrain::Stamp {
            session: 1,
            dimension: 0,
            registry: 1,
            revision: 1,
        }),
        ..Tile::default()
    };
    tile.tally.correct = 5;
    tile.tally.extra = 1;
    assert!(layer_complete(std::slice::from_ref(&tile), 4, 2));
    assert!(!layer_complete(std::slice::from_ref(&tile), 8, 2));
    tile.tally.wrong = 1;
    assert!(!layer_complete(std::slice::from_ref(&tile), 4, 2));
    tile.tally.wrong = 0;
    tile.stamp = None;
    assert!(!layer_complete(&[tile], 4, 2));
}

#[derive(Default)]
pub struct Tile {
    lo: [i32; 3],
    hi: [i32; 3],
    pub stamp: Option<terrain::Stamp>,
    pub tally: Tally,
    /// Blocks in this tile that are in the world exactly as planned. The material list counts
    /// what is left as everything else, so cells that are unloaded or not yet checked are left.
    pub built: u64,
    pub fallback: bool,
    pub error: Option<String>,
    materials: Vec<[u16; 3]>,
    statuses: Vec<CellStatus>,
    /// Cells of water, lava or powder snow that the guide reports nothing about.
    ignored: Vec<bool>,
    pub mismatches: Vec<([i32; 3], CellStatus)>,
    markers: Option<DrawPair>,
    blocks: Option<DrawPair>,
    blocks_ready: bool,
    pending_frames: u8,
    pub(super) ghost_dirty: bool,
    /// A change seen around the camera has not reached the boxes and outlines yet.
    pub(super) markers_stale: bool,
    grid_palette: Vec<u32>,
    grid_cells: Vec<u32>,
}

/// One retained grid for the entire requested view. Preparation is bounded, and the
/// renderer publishes the mesh only when all native sections are ready.
#[derive(Default)]
pub struct WholeGhost {
    packed: Vec<u32>,
    distant: coverage::DistantScan,
    hidden: Vec<bool>,
    changes: BTreeMap<[u32; 3], u32>,
    /// Cells a paste has just asked the server to fill, with the frame their hold ends.
    holds: HashMap<usize, u32>,
    /// Counts `flush` calls: the clock the holds run on.
    frame: u32,
    pair: Option<DrawPair>,
    retiring: Option<DrawPair>,
    /// Fuel one cell edit cost the last time `flush` measured it; zero until measured.
    edit_cost: u64,
    pub failed: bool,
    pub error: Option<String>,
}

/// Fuel `flush` leaves unspent for the rest of the frame.
const FLUSH_RESERVE: u64 = 350_000;

/// How many cell edits one flush may send, given the frame's remaining fuel and the measured
/// cost of an edit (zero when not yet measured, which assumes a pessimistic 700). Costs are
/// padded by half again so one dear batch cannot take the frame past its budget.
pub(crate) fn flush_batch(remaining: u64, edit_cost: u64) -> usize {
    let cost = if edit_cost == 0 {
        700
    } else {
        edit_cost.clamp(100, 5_000)
    };
    let room = remaining.saturating_sub(FLUSH_RESERVE);
    ((room.saturating_mul(2) / (cost * 3)) as usize).clamp(64, 1024)
}

impl WholeGhost {
    pub fn clear(&mut self) {
        if let Some(pair) = self.pair.take() {
            pair.remove();
        }
        if let Some(pair) = self.retiring.take() {
            pair.remove();
        }
        *self = Self::default();
    }

    pub(super) fn restart(&mut self, retain: bool) {
        if !retain {
            self.clear();
            return;
        }
        let previous = self.retiring.take().or_else(|| self.pair.take());
        self.clear();
        self.retiring = previous;
    }

    pub fn active(&self) -> bool {
        self.pair.is_some()
    }

    /// Ghost edits that are waiting for the host to take them.
    pub fn pending_edits(&self) -> usize {
        self.changes.len()
    }

    pub fn prepare(
        &mut self,
        preview: &Preview,
        schematic: &Schematic,
        dimension: i32,
        lightness: f32,
    ) -> bool {
        if let Some(pair) = &mut self.pair {
            let ready = self.retiring.is_none()
                || matches!(
                    draw::geometry_status(pair.geometry),
                    Ok(draw::GeometryState::Ready)
                );
            if ready
                && pair.set_shading(lightness, 1.)
                && let Some(previous) = self.retiring.take()
            {
                previous.remove();
            }
            return true;
        }
        if self.failed {
            return true;
        }
        let placement = preview.placement.unwrap();
        let (lo, hi) = preview.bounds(schematic.size());
        let [sx, sy, sz] = [0, 1, 2].map(|axis| (hi[axis] - lo[axis]) as usize);
        let volume = sx * sy * sz;
        if self.packed.len() < volume {
            self.packed.reserve_exact(volume - self.packed.len());
            let end = (self.packed.len() + 4096).min(volume);
            for index in self.packed.len()..end {
                if index % 128 == 0 && budget::remaining_fuel() < 450_000 {
                    break;
                }
                let [x, y, z] = [index / (sy * sz), index % sy, index / sy % sz];
                let world = [lo[0] + x as i32, lo[1] + y as i32, lo[2] + z as i32];
                let local = placement.world_to_local(schematic.size(), world).unwrap();
                let id = schematic.palette_index(local).unwrap();
                let block = &schematic.palette()[id as usize];
                let palette = if block.is_air() || block.is_placement_artifact() {
                    0
                } else {
                    id + 1
                };
                self.packed.push(DEFAULT_LIGHT | palette);
            }
            return false;
        }
        if budget::remaining_fuel() < 750_000 {
            return false;
        }
        let palette = std::iter::once(draw::BlockState::Named(draw::NamedBlock {
            name: "minecraft:air".into(),
            states: "{}".into(),
        }))
        .chain(preview.palette.iter().cloned().map(draw::BlockState::Named))
        .collect();
        let cells = std::mem::take(&mut self.packed);
        let grid = draw::BlockGrid {
            size: (sx as u32, sy as u32, sz as u32),
            palette,
            cells,
        };
        match create(
            draw::GeometrySpec::Blocks(grid),
            draw::BlockAnchor {
                dimension,
                position: lo.into(),
            },
            lightness,
            self.retiring.is_none(),
        ) {
            Ok(pair) => {
                self.hidden = vec![false; volume];
                self.pair = Some(pair);
            }
            Err(error) => {
                if let Some(previous) = self.retiring.take() {
                    previous.remove();
                }
                self.failed = true;
                self.error = Some(error.detail);
            }
        }
        true
    }

    pub fn compare_tile(&mut self, preview: &Preview, schematic: &Schematic, tile: &Tile) {
        if !self.active() {
            return;
        }
        let (lo, hi) = preview.bounds(schematic.size());
        let sy = (hi[1] - lo[1]) as usize;
        let sz = (hi[2] - lo[2]) as usize;
        let placement = preview.placement.unwrap();
        for x in tile.lo[0]..tile.hi[0] {
            for z in tile.lo[2]..tile.hi[2] {
                for y in tile.lo[1]..tile.hi[1] {
                    let world = [x, y, z];
                    let Some(status) = tile.status_at(world) else {
                        continue;
                    };
                    let local = placement.world_to_local(schematic.size(), world).unwrap();
                    let id = schematic.palette_index(local).unwrap();
                    if schematic.is_air_index(id)
                        || schematic.palette()[id as usize].is_placement_artifact()
                    {
                        continue;
                    }
                    let pos = [0, 1, 2].map(|axis| (world[axis] - lo[axis]) as u32);
                    let index = (pos[0] as usize * sz + pos[2] as usize) * sy + pos[1] as usize;
                    self.apply_observation(index, pos, id, status);
                }
            }
        }
    }

    fn apply_observation(&mut self, index: usize, pos: [u32; 3], id: u32, status: CellStatus) {
        if let Some(&until) = self.holds.get(&index) {
            // The world may still show the air that was there before the paste arrived.
            if matches!(status, CellStatus::Missing | CellStatus::Unloaded) && self.frame < until {
                return;
            }
            self.holds.remove(&index);
        }
        let hidden = ghost_hidden(status, self.hidden[index]);
        if hidden != self.hidden[index] {
            self.hidden[index] = hidden;
            self.changes.insert(pos, if hidden { 0 } else { id + 1 });
        }
    }

    /// Hides the ghost of a cell a paste has just asked the server to fill, without waiting for
    /// the world to show the block. Observations of air keep it hidden for `HOLD_FRAMES`;
    /// anything else, or the end of the hold, goes back to what the world says.
    pub(crate) fn expect_placed(
        &mut self,
        preview: &Preview,
        schematic: &Schematic,
        world: [i32; 3],
    ) {
        if !self.active() {
            return;
        }
        let size = schematic.size();
        let (lo, hi) = preview.bounds(size);
        if !(0..3).all(|axis| (lo[axis]..hi[axis]).contains(&world[axis])) {
            return;
        }
        let Some(local) = preview
            .placement
            .and_then(|placement| placement.world_to_local(size, world))
        else {
            return;
        };
        let Some(id) = schematic.palette_index(local) else {
            return;
        };
        if schematic.is_air_index(id) || schematic.palette()[id as usize].is_placement_artifact() {
            return;
        }
        let pos = std::array::from_fn(|axis| (world[axis] - lo[axis]) as u32);
        let [x, y, z] = pos.map(|axis| axis as usize);
        let (gy, gz) = ((hi[1] - lo[1]) as usize, (hi[2] - lo[2]) as usize);
        self.hold_hidden((x * gz + z) * gy + y, pos);
    }

    fn hold_hidden(&mut self, index: usize, pos: [u32; 3]) {
        self.holds
            .insert(index, self.frame.saturating_add(HOLD_FRAMES));
        if !self.hidden[index] {
            self.hidden[index] = true;
            self.changes.insert(pos, 0);
        }
    }

    pub fn flush(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        let Some(pair) = &self.pair else {
            return;
        };
        match draw::geometry_status(pair.geometry) {
            Ok(draw::GeometryState::Ready) if !self.changes.is_empty() => {
                // A generation remeshes whole columns whatever its size, so a large paste goes
                // in big batches while fuel allows. The batch follows the measured cost of an
                // edit, so a frame that is already busy sends fewer.
                let batch = flush_batch(u64::from(budget::remaining_fuel()), self.edit_cost);
                let edits: Vec<_> = self
                    .changes
                    .iter()
                    .take(batch)
                    .map(|(&[x, y, z], &palette)| draw::BlockEdit {
                        position: (x, y, z),
                        cell: draw::BlockCell {
                            palette,
                            sky_light: None,
                            block_light: None,
                        },
                    })
                    .collect();
                let before = u64::from(budget::remaining_fuel());
                if draw::update_blocks(pair.geometry, &edits).is_ok() {
                    for edit in edits.iter() {
                        let (x, y, z) = edit.position;
                        self.changes.remove(&[x, y, z]);
                    }
                }
                let spent = before.saturating_sub(u64::from(budget::remaining_fuel()));
                if !edits.is_empty() {
                    // Rises at once and falls slowly, so one dear call is remembered.
                    let each = spent / edits.len() as u64;
                    self.edit_cost = each.max((self.edit_cost * 7 + each) / 8);
                }
            }
            Err(error) if error.code != draw::ErrorCode::BudgetExceeded => {
                self.clear();
                self.failed = true;
                self.error = Some(error.detail);
            }
            _ => {}
        }
    }
}

/// A temporarily unavailable terrain sample leaves the last observed ghost visibility intact.
fn ghost_hidden(status: CellStatus, was_hidden: bool) -> bool {
    match status {
        CellStatus::Unloaded => was_hidden,
        CellStatus::Missing => false,
        _ => true,
    }
}

#[test]
fn a_terrain_gap_does_not_restore_a_previously_hidden_ghost() {
    let placed = ghost_hidden(CellStatus::Correct, false);
    assert!(ghost_hidden(CellStatus::Unloaded, placed));
    assert!(!ghost_hidden(CellStatus::Unloaded, false));
    assert!(!ghost_hidden(CellStatus::Missing, placed));
}

#[cfg(test)]
fn whole_with_hidden(hidden: bool) -> WholeGhost {
    WholeGhost {
        hidden: vec![hidden; 4],
        ..WholeGhost::default()
    }
}

#[test]
fn a_pasted_cell_stays_hidden_while_the_world_still_shows_air() {
    let mut whole = whole_with_hidden(false);
    whole.hold_hidden(1, [0, 1, 0]);
    assert_eq!(whole.changes.get(&[0, 1, 0]), Some(&0));
    whole.changes.clear();
    // A stale read from before the server applied the paste does not bring the ghost back.
    whole.apply_observation(1, [0, 1, 0], 6, CellStatus::Missing);
    assert!(whole.hidden[1] && whole.changes.is_empty());
    whole.apply_observation(1, [0, 1, 0], 6, CellStatus::Unloaded);
    assert!(whole.hidden[1] && whole.changes.is_empty());
    // The placed block ends the hold, and it stays hidden.
    whole.apply_observation(1, [0, 1, 0], 6, CellStatus::Correct);
    assert!(whole.hidden[1] && whole.holds.is_empty() && whole.changes.is_empty());
    // After that, a broken block restores the ghost at once.
    whole.apply_observation(1, [0, 1, 0], 6, CellStatus::Missing);
    assert!(!whole.hidden[1]);
    assert_eq!(whole.changes.get(&[0, 1, 0]), Some(&7));
}

#[test]
fn a_refused_paste_gets_its_ghost_back_when_the_hold_ends() {
    let mut whole = whole_with_hidden(false);
    whole.hold_hidden(2, [0, 2, 0]);
    whole.changes.clear();
    whole.frame += HOLD_FRAMES;
    whole.apply_observation(2, [0, 2, 0], 3, CellStatus::Missing);
    assert!(!whole.hidden[2]);
    assert_eq!(whole.changes.get(&[0, 2, 0]), Some(&4));
    assert!(whole.holds.is_empty());
}

#[test]
fn a_wrong_block_ends_the_hold_too() {
    let mut whole = whole_with_hidden(false);
    whole.hold_hidden(0, [0, 0, 0]);
    whole.apply_observation(0, [0, 0, 0], 3, CellStatus::WrongState);
    assert!(whole.hidden[0] && whole.holds.is_empty());
}

pub(super) struct DrawPair {
    geometry: draw::Handle,
    instance: draw::Handle,
    placement: draw::Instance,
}

impl DrawPair {
    fn set_shading(&mut self, lightness: f32, alpha: f32) -> bool {
        if self.placement.params[WASH_PARAM] == lightness && self.placement.tint.3 == alpha {
            return true;
        }
        let mut changed = self.placement.clone();
        changed.params[WASH_PARAM] = lightness;
        changed.tint.3 = alpha;
        if draw::update(self.instance, &changed).is_err() {
            return false;
        }
        self.placement = changed;
        true
    }

    pub(super) fn remove(self) {
        let _ = draw::remove(self.instance);
        let _ = draw::remove(self.geometry);
    }
}

impl Tile {
    pub fn distance_squared(&self, eye: [f32; 3]) -> f32 {
        (0..3)
            .map(|axis| {
                let nearest = eye[axis].clamp(self.lo[axis] as f32, self.hi[axis] as f32);
                (eye[axis] - nearest).powi(2)
            })
            .sum()
    }

    pub fn contains(&self, world: [i32; 3]) -> bool {
        (0..3).all(|axis| (self.lo[axis]..self.hi[axis]).contains(&world[axis]))
    }

    pub fn origin(&self) -> [i32; 3] {
        self.lo
    }
    pub(crate) fn is_missing(&self, world: [i32; 3]) -> bool {
        self.status_at(world) == Some(CellStatus::Missing)
    }

    pub(crate) fn status_at(&self, world: [i32; 3]) -> Option<CellStatus> {
        if !self.contains(world) {
            return None;
        }
        let [x, y, z] = [0, 1, 2].map(|axis| world[axis] - self.lo[axis]);
        let height = self.hi[1] - self.lo[1];
        let depth = self.hi[2] - self.lo[2];
        self.statuses
            .get(((x * depth + z) * height + y) as usize)
            .copied()
    }

    pub fn has_draws(&self) -> bool {
        self.markers.is_some() || self.blocks.is_some()
    }

    pub fn clear(&mut self) {
        if let Some(pair) = self.markers.take() {
            pair.remove();
        }
        if let Some(pair) = self.blocks.take() {
            pair.remove();
        }
        self.stamp = None;
        self.grid_cells.clear();
        self.blocks_ready = false;
        self.statuses.clear();
        self.ignored.clear();
        self.materials.clear();
        self.mismatches.clear();
        self.tally = Tally::default();
        self.built = 0;
        self.markers_stale = false;
    }

    pub(super) fn subtract_placed_materials(&self, remaining: &mut [u64]) {
        for (count, material) in remaining.iter_mut().zip(&self.materials) {
            *count = count.saturating_sub(u64::from(material[1]));
        }
    }

    pub fn material_counts(&self, indices: &[usize]) -> [u32; 3] {
        indices
            .iter()
            .filter_map(|&index| self.materials.get(index))
            .fold([0; 3], |sum, counts| {
                std::array::from_fn(|i| sum[i] + u32::from(counts[i]))
            })
    }

    fn query(&self) -> terrain::RegionQuery {
        terrain::RegionQuery {
            min: position(self.lo),
            max: position(self.hi),
            geometry: terrain::Geometry::empty(),
        }
    }

    pub fn prefetch(&self) {
        let _ = terrain::read_region(self.query(), self.stamp);
    }

    pub fn step(
        &mut self,
        preview: &Preview,
        schematic: &Schematic,
        dimension: i32,
        lightness: Option<f32>,
        whole: bool,
    ) -> (bool, bool) {
        if let (Some(pair), Some(lightness)) = (&mut self.blocks, lightness) {
            pair.set_shading(lightness, 1.);
        }
        if !self.blocks_ready
            && let Some(pair) = &self.blocks
        {
            match draw::geometry_status(pair.geometry) {
                Ok(draw::GeometryState::Ready) => self.blocks_ready = true,
                Ok(draw::GeometryState::Pending) => {}
                Err(error) => {
                    // There is no resident textured tile yet. Let the same
                    // observation draw the colored fallback.
                    self.blocks.take().unwrap().remove();
                    self.grid_cells.clear();
                    self.stamp = None;
                    self.fallback = true;
                    self.error = Some(format!("Texture unavailable: {}", error.detail));
                }
            }
        }
        match terrain::read_region(self.query(), self.stamp) {
            Ok(terrain::ReadResult::Pending) => {
                self.pending_frames += 1;
                if self.pending_frames < 2 {
                    return (false, false);
                }
            }
            Ok(terrain::ReadResult::Unchanged(_)) => {}
            Ok(terrain::ReadResult::Changed(region)) => {
                self.pending_frames = 0;
                if u64::from(budget::remaining_fuel()) < render_gate() {
                    return (false, false);
                }
                let previous_revision = self.stamp.map(|stamp| stamp.revision);
                let before = u64::from(budget::remaining_fuel());
                self.render(
                    preview,
                    schematic,
                    dimension,
                    lightness.filter(|_| !self.fallback),
                    whole,
                    region,
                );
                note_render_cost(before.saturating_sub(u64::from(budget::remaining_fuel())));
                return (
                    true,
                    self.stamp.map(|stamp| stamp.revision) != previous_revision,
                );
            }
            Err(e) => {
                // A deferred or out-of-range observation does not invalidate the
                // already visible guide.
                self.error = Some(format!("Terrain: {e:?}"));
            }
        }
        self.pending_frames = 0;
        (true, false)
    }

    fn render(
        &mut self,
        preview: &Preview,
        schematic: &Schematic,
        dimension: i32,
        lightness: Option<f32>,
        whole: bool,
        region: terrain::Region,
    ) {
        let [sx, sy, sz] = [0, 1, 2].map(|i| self.hi[i] - self.lo[i]);
        let volume = (sx * sy * sz) as usize;
        let mut ids = Vec::with_capacity(volume);
        let mut statuses = Vec::with_capacity(volume);
        let mut ignored = Vec::with_capacity(volume);
        let p = preview.placement.unwrap();
        let mut comparisons = std::collections::HashMap::new();
        for x in 0..sx {
            for z in 0..sz {
                for y in 0..sy {
                    let world = [self.lo[0] + x, self.lo[1] + y, self.lo[2] + z];
                    let local = p.world_to_local(schematic.size(), world).unwrap();
                    let id = schematic.palette_index(local).unwrap();
                    let expected = &preview.palette[id as usize];
                    let (status, skip) = if schematic.palette()[id as usize].is_placement_artifact() {
                        (CellStatus::Correct, false)
                    } else {
                        let cell = ids.len();
                        if region
                            .loaded
                            .get(cell / 64)
                            .is_some_and(|bits| bits & (1 << (cell % 64)) != 0)
                            && let Some(&actual) = region.states.get(cell)
                        {
                            *comparisons.entry((id, actual)).or_insert_with(|| {
                                compare_cell(&region, cell, expected, schematic.is_air_index(id))
                            })
                        } else {
                            (CellStatus::Unloaded, false)
                        }
                    };
                    ids.push(id);
                    statuses.push(status);
                    ignored.push(skip);
                }
            }
        }
        if self.stamp.is_some() && self.statuses == statuses && self.ignored == ignored {
            self.stamp = Some(region.stamp);
            return;
        }
        let mut used_palette: Vec<_> = ids
            .iter()
            .copied()
            .filter(|&id| !schematic.is_air_index(id))
            .collect();
        used_palette.sort_unstable();
        used_palette.dedup();
        self.grid_palette = used_palette;
        self.error = None;
        self.mismatches.clear();
        self.ghost_dirty = true;
        let mut cells = Vec::with_capacity(volume);
        let mut highlights = draw::Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
        let mut used_texture = false;
        let mut materials = vec![[0; 3]; preview.palette.len()];
        let mut tally = Tally::default();
        for x in 0..sx {
            for z in 0..sz {
                for y in 0..sy {
                    let id = ids[cells.len()] as usize;
                    let status = statuses[cells.len()];
                    let skip = ignored[cells.len()];
                    let air = schematic.is_air_index(id as u32);
                    let ignored = schematic.palette()[id].is_placement_artifact();
                    let mut word = DEFAULT_LIGHT;
                    if ((!air && !ignored) || status == CellStatus::Extra) && !skip {
                        tally.add(status);
                        if status != CellStatus::Correct {
                            self.mismatches
                                .push(([self.lo[0] + x, self.lo[1] + y, self.lo[2] + z], status));
                        }
                    }
                    let color = match status {
                        CellStatus::Missing | CellStatus::Unloaded if !air && !ignored => {
                            materials[id][usize::from(status == CellStatus::Unloaded) * 2] += 1;
                            if lightness.is_some() {
                                let palette_index =
                                    self.grid_palette.binary_search(&(id as u32)).unwrap();
                                word |= palette_index as u32 + 1;
                                used_texture = true;
                                None
                            } else if whole {
                                None
                            } else {
                                Some((0.16, 0.60, 0.94, 0.30))
                            }
                        }
                        // Counted for the material list, but never marked.
                        CellStatus::Wrong => {
                            materials[id][0] += 1;
                            (!skip).then_some((1., 0.22, 0.42, 0.44))
                        }
                        CellStatus::WrongState => {
                            materials[id][0] += 1;
                            (!skip).then_some((1., 0.69, 0.08, 0.42))
                        }
                        CellStatus::Extra => (!skip).then_some((0.98, 0.14, 0.17, 0.40)),
                        CellStatus::Correct if !air && !ignored => {
                            materials[id][1] += 1;
                            None
                        }
                        _ => None,
                    };
                    cells.push(word);
                    if let Some(color) = color
                        && highlights.indices.len() + 36 <= MARKER_INDEX_BUDGET
                    {
                        let exposed =
                            exposed_marker_faces([x, y, z], [sx, sy, sz], &statuses, status);
                        add_box(&mut highlights, [x, y, z], color, exposed);
                    }
                    // Rails and chests are thin or have no clear edge against the ground, so a
                    // missing one gets a frame around its real shape.
                    if status == CellStatus::Missing
                        && !air
                        && !ignored
                        && !skip
                        && let Some(bounds) = shapes::outline(&schematic.palette()[id])
                    {
                        add_outline(&mut highlights, [x, y, z], bounds, GHOST_FRAME);
                    }
                }
            }
        }

        let anchor = draw::BlockAnchor {
            dimension,
            position: self.lo.into(),
        };
        if let Some(lightness) = lightness.filter(|_| used_texture) {
            if let Some(pair) = &self.blocks {
                let edits = self
                    .grid_cells
                    .iter()
                    .zip(&cells)
                    .enumerate()
                    .filter_map(|(index, (&old, &new))| {
                        if old == new {
                            return None;
                        }
                        let y = index as u32 % sy as u32;
                        let z = index as u32 / sy as u32 % sz as u32;
                        let x = index as u32 / (sy * sz) as u32;
                        Some(draw::BlockEdit {
                            position: (x, y, z),
                            cell: draw::BlockCell {
                                palette: new & 0xffff,
                                sky_light: None,
                                block_light: None,
                            },
                        })
                    })
                    .collect::<Vec<_>>();
                if !edits.is_empty() {
                    match draw::geometry_status(pair.geometry) {
                        Ok(draw::GeometryState::Ready) => {
                            if let Err(error) = draw::update_blocks(pair.geometry, &edits) {
                                self.error = Some(format!("Texture update: {}", error.detail));
                                return;
                            }
                        }
                        Ok(draw::GeometryState::Pending) => return,
                        Err(error) => {
                            self.error = Some(format!("Texture update: {}", error.detail));
                            return;
                        }
                    }
                }
                self.grid_cells = cells;
            } else {
                let palette = std::iter::once(draw::BlockState::Named(draw::NamedBlock {
                    name: "minecraft:air".into(),
                    states: "{}".into(),
                }))
                .chain(
                    self.grid_palette
                        .iter()
                        .map(|&id| draw::BlockState::Named(preview.palette[id as usize].clone())),
                )
                .collect();
                let grid = draw::BlockGrid {
                    size: (sx as u32, sy as u32, sz as u32),
                    palette,
                    cells: cells.clone(),
                };
                match create(draw::GeometrySpec::Blocks(grid), anchor, lightness, true) {
                    Ok(pair) => {
                        self.blocks = Some(pair);
                        self.grid_cells = cells;
                    }
                    Err(error) => {
                        self.fallback = error.code != draw::ErrorCode::BudgetExceeded;
                        self.error = Some(format!("Texture unavailable: {}", error.detail));
                        return;
                    }
                }
            }
        } else if let Some(pair) = self.blocks.take() {
            pair.remove();
            self.grid_cells.clear();
            self.blocks_ready = false;
        }
        if !self.publish_markers(highlights, anchor) {
            // The host had no room for the boxes this frame. Leave the section unstamped so
            // it is checked again, instead of losing its boxes until a block changes.
            return;
        }
        self.markers_stale = false;
        self.stamp = Some(region.stamp);
        self.statuses = statuses;
        self.ignored = ignored;
        self.built = materials.iter().map(|counts| u64::from(counts[1])).sum();
        self.materials = materials;
        self.tally = tally;
    }

    /// Replaces the section's boxes and outlines. False when the host had no room this frame
    /// and the same mesh should be sent again later.
    fn publish_markers(&mut self, mesh: draw::Mesh, anchor: draw::BlockAnchor) -> bool {
        if mesh.vertices.is_empty() {
            if let Some(previous) = self.markers.take() {
                previous.remove();
            }
            return true;
        }
        match create(draw::GeometrySpec::Triangles(mesh), anchor, 0., true) {
            Ok(pair) => {
                if let Some(previous) = self.markers.replace(pair) {
                    previous.remove();
                }
                true
            }
            Err(error) => {
                let later = error.code == draw::ErrorCode::BudgetExceeded;
                self.error = Some(format!("Overlay: {}", error.detail));
                !later
            }
        }
    }

    /// Takes in what the watch around the camera saw of one cell: the status, the counts for
    /// the material list and the list of wrong cells change now, without waiting for the
    /// next sample of the whole section. The boxes follow in `rebuild_markers`.
    pub(crate) fn apply_hot(
        &mut self,
        preview: &Preview,
        schematic: &Schematic,
        world: [i32; 3],
        status: CellStatus,
        skip: bool,
    ) -> bool {
        if self.stamp.is_none() || !self.contains(world) {
            return false;
        }
        let [sy, sz] = [1, 2].map(|axis| self.hi[axis] - self.lo[axis]);
        let [x, y, z] = [0, 1, 2].map(|axis| world[axis] - self.lo[axis]);
        let index = ((x * sz + z) * sy + y) as usize;
        let (Some(&old), Some(&old_skip)) = (self.statuses.get(index), self.ignored.get(index))
        else {
            return false;
        };
        if old == status && old_skip == skip {
            return false;
        }
        let Some(id) = preview
            .placement
            .and_then(|placement| placement.world_to_local(schematic.size(), world))
            .and_then(|local| schematic.palette_index(local))
        else {
            return false;
        };
        let id = id as usize;
        let air = schematic.is_air_index(id as u32);
        let artifact = schematic.palette()[id].is_placement_artifact();
        if artifact {
            return false;
        }
        let counted = |status: CellStatus, skip: bool| (!air || status == CellStatus::Extra) && !skip && !artifact;
        if counted(old, old_skip) {
            self.tally.remove(old);
        }
        if counted(status, skip) {
            self.tally.add(status);
        }
        if let Some(counts) = self.materials.get_mut(id) {
            if let Some(slot) = material_slot(old, air, false) {
                counts[slot] = counts[slot].saturating_sub(1);
            }
            if let Some(slot) = material_slot(status, air, false) {
                counts[slot] = counts[slot].saturating_add(1);
            }
        }
        self.built = self.materials.iter().map(|counts| u64::from(counts[1])).sum();
        self.mismatches.retain(|(cell, _)| *cell != world);
        if counted(status, skip) && status != CellStatus::Correct {
            self.mismatches.push((world, status));
        }
        self.statuses[index] = status;
        self.ignored[index] = skip;
        self.markers_stale = true;
        true
    }

    /// Draws the boxes and outlines again from the section's statuses, after `apply_hot`
    /// changed some. Only the cells that are not right are visited, so it costs little.
    /// False when the host had no room this frame; it is tried again.
    pub(crate) fn rebuild_markers(
        &mut self,
        preview: &Preview,
        schematic: &Schematic,
        dimension: i32,
    ) -> bool {
        if !self.markers_stale {
            return true;
        }
        let size = [0, 1, 2].map(|axis| self.hi[axis] - self.lo[axis]);
        let mut mesh = draw::Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
        for &(world, status) in &self.mismatches {
            let local = [0, 1, 2].map(|axis| world[axis] - self.lo[axis]);
            if let Some(color) = status_color(status)
                && mesh.indices.len() + 36 <= MARKER_INDEX_BUDGET
            {
                let exposed = exposed_marker_faces(local, size, &self.statuses, status);
                add_box(&mut mesh, local, color, exposed);
            }
            if status == CellStatus::Missing
                && let Some(id) = preview
                    .placement
                    .and_then(|placement| placement.world_to_local(schematic.size(), world))
                    .and_then(|local| schematic.palette_index(local))
                && let Some(bounds) = shapes::outline(&schematic.palette()[id as usize])
            {
                add_outline(&mut mesh, local, bounds, GHOST_FRAME);
            }
        }
        let anchor = draw::BlockAnchor {
            dimension,
            position: self.lo.into(),
        };
        if !self.publish_markers(mesh, anchor) {
            return false;
        }
        self.markers_stale = false;
        true
    }

    /// Gives a tile per-material counts without a world scan.
    #[cfg(test)]
    pub(crate) fn set_material_counts(&mut self, counts: Vec<[u16; 3]>) {
        self.built = counts.iter().map(|counts| u64::from(counts[1])).sum();
        self.materials = counts;
    }
}

/// Water, lava and powder snow come from buckets and the world, so the guide never counts
/// them as missing, wrong or extra. The material list still counts them.
pub(crate) fn ignored_material(name: &str) -> bool {
    matches!(
        name.strip_prefix("minecraft:").unwrap_or(name),
        "water" | "flowing_water" | "lava" | "flowing_lava" | "powder_snow"
    )
}

/// `compare`, and whether the cell is one the guide reports nothing about: a planned liquid
/// or powder snow cell that is not right, or air planned where the world has one.
pub(crate) fn compare_cell(
    region: &terrain::Region,
    cell: usize,
    expected: &draw::NamedBlock,
    air: bool,
) -> (CellStatus, bool) {
    let status = compare(region, cell, expected, air);
    let ignored = match status {
        CellStatus::Correct | CellStatus::Unloaded => false,
        _ if !air => ignored_material(&expected.name),
        _ => region
            .states
            .get(cell)
            .and_then(|&state| region.palette.get(usize::from(state)))
            .and_then(|block| block.identifier.as_deref())
            .is_some_and(ignored_material),
    };
    (status, ignored)
}

fn compare(
    region: &terrain::Region,
    cell: usize,
    expected: &draw::NamedBlock,
    air: bool,
) -> CellStatus {
    if !region
        .loaded
        .get(cell / 64)
        .is_some_and(|word| word & (1 << (cell % 64)) != 0)
    {
        return CellStatus::Unloaded;
    }
    let Some(&index) = region.states.get(cell) else {
        return CellStatus::Unloaded;
    };
    let Some(block) = region.palette.get(index as usize) else {
        return CellStatus::Unloaded;
    };
    let Some(name) = block.identifier.as_deref() else {
        return CellStatus::Unloaded;
    };
    let actual_air = matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    );
    match (air, actual_air) {
        (true, true) => CellStatus::Correct,
        (true, false) => CellStatus::Extra,
        (false, true) => CellStatus::Missing,
        // Fast Place builds a double slab as the full block of its material.
        (false, false) if built_as_full_block(&expected.name, name) => CellStatus::Correct,
        // A powered or lit block counts as its plain block; the world lights it.
        (false, false)
            if name != expected.name
                && crate::unlit_name(name).unwrap_or(name)
                    != crate::unlit_name(&expected.name).unwrap_or(&expected.name) =>
        {
            CellStatus::Wrong
        }
        (false, false) => match block.canonical_state.as_deref() {
            Some(state) if preview_states_match(name, &expected.states, state) => {
                CellStatus::Correct
            }
            Some(_) => CellStatus::WrongState,
            None => CellStatus::Unloaded,
        },
    }
}

/// Whether `actual` is the full block a planned double slab is built as, following the
/// registry's naming of a material's full block: `dark_oak_double_slab` as `dark_oak_planks`,
/// `stone_brick_double_slab` as `stone_bricks`, `normal_stone_double_slab` as `stone`.
/// Incomplete: petrified oak and end stone brick double slabs name no matching block.
fn built_as_full_block(expected: &str, actual: &str) -> bool {
    let Some(material) = expected
        .strip_prefix("minecraft:")
        .and_then(|name| name.strip_suffix("_double_slab"))
    else {
        return false;
    };
    let Some(actual) = actual.strip_prefix("minecraft:") else {
        return false;
    };
    actual == material
        || ["s", "_planks", "_block"]
            .iter()
            .any(|suffix| actual.strip_suffix(suffix) == Some(material))
        || material
            .strip_suffix(actual)
            .is_some_and(|prefix| prefix.ends_with('_'))
}

/// Whether two states differ only in properties the world sets after placement, such as
/// power, lit or growth, so a built guide counts them as correct. Open state stays compared:
/// V2 Strict reconciles a trapdoor whose guide cell reads as wrong.
fn same_placed_state(expected: &str, actual: &str) -> bool {
    let placed = |state: &str| {
        let mut values: Map<String, serde_json::Value> = serde_json::from_str(state).ok()?;
        values.retain(|key, _| {
            key == "open_bit" || !crate::WORLD_CONTROLLED_STATES.contains(&key.as_str())
        });
        Some(values)
    };
    placed(expected).is_some_and(|expected| Some(expected) == placed(actual))
}

/// Chests face the player who places them, so the guide cannot ask for one direction: a chest
/// of the right kind counts as built whichever way it faces.
fn faces_the_placer(name: &str) -> bool {
    let short = name.strip_prefix("minecraft:").unwrap_or(name);
    let short = short.strip_prefix("waxed_").unwrap_or(short);
    matches!(short, "chest" | "trapped_chest" | "ender_chest") || short.ends_with("copper_chest")
}

/// Whether two states are equal once the direction properties are set aside.
fn same_apart_from_direction(expected: &str, actual: &str) -> bool {
    let without = |state: &str| {
        let mut values: Map<String, serde_json::Value> = serde_json::from_str(state).ok()?;
        values.remove("minecraft:cardinal_direction");
        values.remove("facing_direction");
        Some(values)
    };
    without(expected).is_some_and(|expected| Some(expected) == without(actual))
}

/// Pane arms and wall geometry are resolved by adjacent blocks when placed.
fn preview_states_match(name: &str, expected: &str, actual: &str) -> bool {
    if expected == actual || same_placed_state(expected, actual) {
        return true;
    }
    if faces_the_placer(name) && same_apart_from_direction(expected, actual) {
        return true;
    }
    let wall = name.ends_with("_wall");
    let wire = name == "minecraft:trip_wire";
    let leaves =
        name.ends_with("_leaves") || matches!(name, "minecraft:leaves" | "minecraft:leaves2");
    let redstone = name == "minecraft:redstone_wire";
    let observer = name == "minecraft:observer";
    let hopper = name == "minecraft:hopper";
    let fence = name.ends_with("_fence");
    let gate = name.ends_with("fence_gate");
    let stair = name.ends_with("_stairs");
    if !stair
        && !wall
        && !fence
        && !gate
        && !wire
        && !leaves
        && !redstone
        && !observer
        && !hopper
        && !name.ends_with("glass_pane")
        && name != "minecraft:iron_bars"
    {
        return false;
    }
    let without_connections = |state| {
        let mut values: Map<String, serde_json::Value> = serde_json::from_str(state).ok()?;
        for key in ["north", "east", "south", "west"] {
            let key = if wall {
                format!("wall_connection_type_{key}")
            } else {
                format!("minecraft:connection_{key}")
            };
            values.remove(&key);
        }
        if stair {
            values.remove("minecraft:corner");
        }
        if wall {
            values.remove("wall_post_bit");
        }
        if wire {
            for key in ["attached_bit", "powered_bit", "suspended_bit"] {
                values.remove(key);
            }
        }
        values.retain(|key, _| {
            !((leaves && matches!(key.as_str(), "persistent_bit" | "update_bit"))
                || (redstone && key.starts_with("redstone_"))
                || (observer && key == "powered_bit")
                || (hopper && matches!(key.as_str(), "toggle_bit" | "facing_direction")))
        });
        if gate {
            values.remove("in_wall_bit");
            values.remove("open_bit");
        }
        Some(values)
    };
    without_connections(expected)
        .zip(without_connections(actual))
        .is_some_and(|(a, b)| a == b)
}

#[test]
fn double_slabs_built_as_their_full_block_count_as_correct() {
    assert!(built_as_full_block(
        "minecraft:dark_oak_double_slab",
        "minecraft:dark_oak_planks"
    ));
    assert!(built_as_full_block(
        "minecraft:stone_brick_double_slab",
        "minecraft:stone_bricks"
    ));
    assert!(built_as_full_block(
        "minecraft:brick_double_slab",
        "minecraft:brick_block"
    ));
    assert!(built_as_full_block(
        "minecraft:normal_stone_double_slab",
        "minecraft:stone"
    ));
    assert!(built_as_full_block(
        "minecraft:andesite_double_slab",
        "minecraft:andesite"
    ));
    assert!(!built_as_full_block(
        "minecraft:dark_oak_double_slab",
        "minecraft:dark_oak_stairs"
    ));
    assert!(!built_as_full_block(
        "minecraft:dark_oak_double_slab",
        "minecraft:oak_planks"
    ));
    assert!(!built_as_full_block("minecraft:stone", "minecraft:stone"));
}

#[test]
fn a_placed_chest_counts_as_built_whichever_way_it_faces() {
    let north = r#"{"minecraft:cardinal_direction":{"type":"string","value":"north"}}"#;
    let east = r#"{"minecraft:cardinal_direction":{"type":"string","value":"east"}}"#;
    for name in [
        "minecraft:chest",
        "minecraft:trapped_chest",
        "minecraft:ender_chest",
        "minecraft:exposed_copper_chest",
    ] {
        assert!(preview_states_match(name, north, east), "{name}");
    }
    // Other directional blocks still compare their direction, and a chest still compares
    // anything that is not a direction.
    assert!(!preview_states_match("minecraft:furnace", north, east));
    assert!(!preview_states_match(
        "minecraft:chest",
        r#"{"minecraft:cardinal_direction":{"type":"string","value":"north"},"x":{"type":"int","value":1}}"#,
        east
    ));
}

#[test]
fn world_set_states_and_lit_variants_count_as_correct_in_the_guide() {
    let lamp_lit = r#"{"growth":{"type":"int","value":7}}"#;
    assert!(preview_states_match(
        "minecraft:wheat",
        lamp_lit,
        r#"{"growth":{"type":"int","value":3}}"#
    ));
    assert!(preview_states_match(
        "minecraft:golden_rail",
        r#"{"rail_data_bit":{"type":"byte","value":1},"rail_direction":{"type":"int","value":1}}"#,
        r#"{"rail_data_bit":{"type":"byte","value":0},"rail_direction":{"type":"int","value":0}}"#
    ));
    // Orientation and open state stay compared.
    assert!(!preview_states_match(
        "minecraft:oak_stairs",
        r#"{"upside_down_bit":{"type":"byte","value":1},"weirdo_direction":{"type":"int","value":2}}"#,
        r#"{"upside_down_bit":{"type":"byte","value":0},"weirdo_direction":{"type":"int","value":2}}"#
    ));
    assert!(!preview_states_match(
        "minecraft:oak_trapdoor",
        r#"{"direction":{"type":"int","value":1},"open_bit":{"type":"byte","value":1}}"#,
        r#"{"direction":{"type":"int","value":1},"open_bit":{"type":"byte","value":0}}"#
    ));
    assert_eq!(
        crate::unlit_name("minecraft:lit_redstone_lamp"),
        Some("minecraft:redstone_lamp")
    );
}

#[test]
fn natural_and_persistent_leaves_are_the_same_material() {
    let natural =
        r#"{"persistent_bit":{"type":"byte","value":0},"update_bit":{"type":"byte","value":1}}"#;
    let placed =
        r#"{"persistent_bit":{"type":"byte","value":1},"update_bit":{"type":"byte","value":0}}"#;
    assert!(preview_states_match(
        "minecraft:oak_leaves",
        natural,
        placed
    ));
}

#[cfg(test)]
#[test]
fn pane_arms_do_not_mark_the_correct_pane_yellow() {
    let connected = r#"{"minecraft:connection_north":{"type":"byte","value":1}}"#;
    assert!(preview_states_match(
        "minecraft:light_gray_stained_glass_pane",
        "{}",
        connected
    ));
    // Connections are world-set for any block; a placement-set difference still counts.
    assert!(!preview_states_match(
        "minecraft:stone",
        "{}",
        r#"{"pillar_axis":{"type":"string","value":"x"}}"#
    ));
    assert!(!preview_states_match(
        "minecraft:glass_pane",
        "{}",
        r#"{"other":0}"#
    ));
}

pub(crate) fn canonical(state: &BlockState) -> String {
    let mut properties = Map::new();
    for (name, value) in &state.states {
        let typed = match value {
            StateValue::Bool(value) => json!({"type": "byte", "value": u8::from(*value)}),
            StateValue::Byte(value) => json!({"type": "byte", "value": value}),
            StateValue::Int(value) => json!({"type": "int", "value": value}),
            StateValue::Str(value) => json!({"type": "string", "value": value}),
        };
        properties.insert(name.clone(), typed);
    }
    serde_json::Value::Object(properties).to_string()
}

fn position([x, y, z]: [i32; 3]) -> terrain::Position {
    terrain::Position { x, y, z }
}

/// Creates one retained draw. Block grids draw opaque, like real blocks, blended toward
/// light blue by `lightness` with voxel edge lines; other geometry uses its vertex colours.
fn create(
    spec: draw::GeometrySpec,
    anchor: draw::BlockAnchor,
    lightness: f32,
    visible: bool,
) -> Result<DrawPair, draw::DrawError> {
    let textured = matches!(&spec, draw::GeometrySpec::Blocks(_));
    let geometry = draw::create_geometry(&spec)?;
    let mut params = vec![0.; 16];
    if textured {
        params[OUTLINE_PARAM] = 1.;
        params[WASH_PARAM] = lightness;
        params[WASH_COLOR_PARAM..WASH_COLOR_PARAM + 3].copy_from_slice(&GHOST_WASH);
    }
    let placement = draw::Instance {
        geometry,
        material: None,
        clip: None,
        params,
        anchor: draw::Anchor::Block(anchor),
        transform: draw::Transform {
            translation: (0., 0., 0.),
            rotation: (0., 0., 0., 1.),
            scale: (1., 1., 1.),
        },
        tint: (1., 1., 1., if visible { 1. } else { 0. }),
        depth: draw::Depth::Test,
        blend: draw::Blend::Premultiplied,
        width: 1.,
        width_mode: draw::WidthMode::World,
        join: draw::Join::Bevel,
        cap: draw::Cap::Butt,
    };
    match draw::create(&placement) {
        Ok(instance) => Ok(DrawPair {
            geometry,
            instance,
            placement,
        }),
        Err(error) => {
            let _ = draw::remove(geometry);
            Err(error)
        }
    }
}

// Same-status neighbors share no inner overlay surface.
const MARKER_NEIGHBORS: [[i32; 3]; 6] = [
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
    [0, -1, 0],
    [0, 1, 0],
];

fn exposed_marker_faces(
    position: [i32; 3],
    size: [i32; 3],
    statuses: &[CellStatus],
    status: CellStatus,
) -> u8 {
    let [_, sy, sz] = size;
    MARKER_NEIGHBORS
        .iter()
        .enumerate()
        .fold(0, |faces, (face, delta)| {
            let neighbor: [i32; 3] = std::array::from_fn(|i| position[i] + delta[i]);
            let covered = (0..3).all(|i| (0..size[i]).contains(&neighbor[i]))
                && statuses[((neighbor[0] * sz + neighbor[2]) * sy + neighbor[1]) as usize]
                    == status;
            if covered { faces } else { faces | (1 << face) }
        })
}

/// The twelve edges of `bounds` (block-local min then max) as thin boxes.
fn add_frame(mesh: &mut draw::Mesh, [x, y, z]: [i32; 3], bounds: [f32; 6], color: (f32, f32, f32, f32)) {
    const EDGE: f32 = FRAME_EDGE;
    let origin = [x as f32, y as f32, z as f32];
    let low: [f32; 3] = std::array::from_fn(|axis| origin[axis] + bounds[axis] - MARKER_INSET);
    let high: [f32; 3] = std::array::from_fn(|axis| origin[axis] + bounds[axis + 3] + MARKER_INSET);
    for along in 0..3 {
        let [a, b] = match along {
            0 => [1, 2],
            1 => [0, 2],
            _ => [0, 1],
        };
        for at_a in [false, true] {
            for at_b in [false, true] {
                let mut min = low;
                let mut max = high;
                for (axis, at_high) in [(a, at_a), (b, at_b)] {
                    if at_high {
                        min[axis] = high[axis] - EDGE;
                    } else {
                        max[axis] = low[axis] + EDGE;
                    }
                }
                add_bounds(mesh, min, max, color);
            }
        }
    }
}

/// Outlines a rail or chest, unless the mesh is already as big as one geometry may be. A flat
/// rail gets only a ring on its top, a ninth of the cost of twelve edges.
fn add_outline(
    mesh: &mut draw::Mesh,
    cell: [i32; 3],
    bounds: [f32; 6],
    color: (f32, f32, f32, f32),
) -> bool {
    let flat = bounds[4] - bounds[1] <= FLAT_HEIGHT;
    let cost = if flat { FLAT_FRAME_INDICES } else { FRAME_INDICES };
    if mesh.indices.len() + cost > MARKER_INDEX_BUDGET {
        return false;
    }
    if flat {
        add_flat_frame(mesh, cell, bounds, color);
    } else {
        add_frame(mesh, cell, bounds, color);
    }
    true
}

/// Four strips round the top of `bounds`, each visible from above and below.
fn add_flat_frame(
    mesh: &mut draw::Mesh,
    [x, y, z]: [i32; 3],
    bounds: [f32; 6],
    color: (f32, f32, f32, f32),
) {
    let origin = [x as f32, y as f32, z as f32];
    let lo = [origin[0] + bounds[0], origin[2] + bounds[2]];
    let hi = [origin[0] + bounds[3], origin[2] + bounds[5]];
    let top = origin[1] + bounds[4] + MARKER_INSET + 0.004;
    let strip = FLAT_STRIP.min((hi[0] - lo[0]) / 2.).min((hi[1] - lo[1]) / 2.);
    let strips = [
        ([lo[0], lo[1]], [hi[0], lo[1] + strip]),
        ([lo[0], hi[1] - strip], [hi[0], hi[1]]),
        ([lo[0], lo[1] + strip], [lo[0] + strip, hi[1] - strip]),
        ([hi[0] - strip, lo[1] + strip], [hi[0], hi[1] - strip]),
    ];
    for ([x0, z0], [x1, z1]) in strips {
        let base = mesh.vertices.len() as u32;
        for (px, pz) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
            mesh.vertices.push(draw::Vertex {
                position: (px, top, pz),
                color,
                uv: (0., 0.),
                data: (0., 0., 0., 0.),
            });
        }
        mesh.indices
            .extend([0, 1, 2, 1, 3, 2, 0, 2, 1, 1, 2, 3].map(|i| base + i));
    }
}

fn add_bounds(mesh: &mut draw::Mesh, min: [f32; 3], max: [f32; 3], color: (f32, f32, f32, f32)) {
    let base = mesh.vertices.len() as u32;
    for dz in [min[2], max[2]] {
        for dy in [min[1], max[1]] {
            for dx in [min[0], max[0]] {
                mesh.vertices.push(draw::Vertex {
                    position: (dx, dy, dz),
                    color,
                    uv: (0., 0.),
                    data: (0., 0., 0., 0.),
                });
            }
        }
    }
    let triangles = [
        0, 2, 1, 1, 2, 3, 4, 5, 6, 5, 7, 6, 0, 4, 2, 2, 4, 6, 1, 3, 5, 3, 7, 5, 0, 1, 4, 1, 5, 4,
        2, 6, 3, 3, 6, 7,
    ];
    mesh.indices.extend(triangles.iter().map(|i| base + i));
}

fn add_box(mesh: &mut draw::Mesh, [x, y, z]: [i32; 3], color: (f32, f32, f32, f32), faces: u8) {
    if faces == 0 {
        return;
    }
    let base = mesh.vertices.len() as u32;
    let extent = [-MARKER_INSET, 1. + MARKER_INSET];
    for dz in extent {
        for dy in extent {
            for dx in extent {
                mesh.vertices.push(draw::Vertex {
                    position: (x as f32 + dx, y as f32 + dy, z as f32 + dz),
                    color,
                    uv: (0., 0.),
                    data: (0., 0., 0., 0.),
                });
            }
        }
    }
    let triangles = [
        0, 2, 1, 1, 2, 3, 4, 5, 6, 5, 7, 6, 0, 4, 2, 2, 4, 6, 1, 3, 5, 3, 7, 5, 0, 1, 4, 1, 5, 4,
        2, 6, 3, 3, 6, 7,
    ];
    for (face, indices) in triangles.chunks_exact(6).enumerate() {
        if faces & (1 << face) != 0 {
            mesh.indices.extend(indices.iter().map(|i| base + i));
        }
    }
}

#[cfg(test)]
#[test]
fn stair_layers_compare_half_and_facing_independently_of_derived_corner() {
    let planned = r#"{"minecraft:corner":{"value":"none"},"upside_down_bit":{"value":0},"weirdo_direction":{"value":2}}"#;
    let joined = planned.replace("none", "outer_left");
    assert!(preview_states_match(
        "minecraft:oak_stairs",
        planned,
        &joined
    ));
    assert!(!preview_states_match(
        "minecraft:oak_stairs",
        planned,
        &joined.replace("value\":0", "value\":1")
    ));
}

#[cfg(test)]
#[test]
fn a_fence_is_correct_before_its_neighbors_connect() {
    let connected = r#"{"minecraft:connection_east":{"type":"byte","value":1}}"#;
    assert!(preview_states_match(
        "minecraft:pale_oak_fence",
        connected,
        "{}"
    ));
}

#[cfg(test)]
fn region_of(names: &[&str]) -> terrain::Region {
    terrain::Region {
        stamp: terrain::Stamp {
            session: 0,
            dimension: 0,
            registry: 0,
            revision: 0,
        },
        chunks: Vec::new(),
        layer_count: 1,
        loaded: vec![u64::MAX],
        states: (0..names.len() as u16).collect(),
        palette: names
            .iter()
            .enumerate()
            .map(|(index, name)| terrain::BlockState {
                runtime_id: index as u32,
                identifier: Some((*name).into()),
                canonical_state: Some("{}".into()),
                tags: Vec::new(),
                physics: None,
                collision: terrain::ShapeBinding::NotRequested,
                selection: terrain::ShapeBinding::NotRequested,
            })
            .collect(),
        shape_offsets: Vec::new(),
        shape_boxes: Vec::new(),
        overrides: Vec::new(),
    }
}

#[cfg(test)]
fn block(name: &str) -> draw::NamedBlock {
    draw::NamedBlock {
        name: name.into(),
        states: "{}".into(),
    }
}

#[test]
fn water_lava_and_powder_snow_are_never_reported_but_other_blocks_still_are() {
    let world = region_of(&[
        "minecraft:air",
        "minecraft:water",
        "minecraft:stone",
        "minecraft:lava",
        "minecraft:powder_snow",
    ]);
    // Planned liquid or powder snow: missing, or a block in its place, is not reported.
    assert_eq!(
        compare_cell(&world, 0, &block("minecraft:water"), false),
        (CellStatus::Missing, true)
    );
    assert_eq!(
        compare_cell(&world, 2, &block("minecraft:flowing_lava"), false),
        (CellStatus::Wrong, true)
    );
    assert_eq!(
        compare_cell(&world, 0, &block("minecraft:powder_snow"), false),
        (CellStatus::Missing, true)
    );
    // The right liquid is simply correct.
    assert_eq!(
        compare_cell(&world, 1, &block("minecraft:water"), false),
        (CellStatus::Correct, false)
    );
    // Water, lava or powder snow where air is planned is not an extra block; stone is.
    for cell in [1, 3, 4] {
        assert_eq!(
            compare_cell(&world, cell, &block("minecraft:air"), true),
            (CellStatus::Extra, true)
        );
    }
    assert_eq!(
        compare_cell(&world, 2, &block("minecraft:air"), true),
        (CellStatus::Extra, false)
    );
    // A planned stone block is still missing or wrong when the world has air or water there.
    assert_eq!(
        compare_cell(&world, 0, &block("minecraft:stone"), false),
        (CellStatus::Missing, false)
    );
    assert_eq!(
        compare_cell(&world, 1, &block("minecraft:stone"), false),
        (CellStatus::Wrong, false)
    );
}

#[test]
fn a_missing_liquid_does_not_hold_back_the_next_layer() {
    // The tile tally leaves ignored cells out, which is what `layer_complete` reads.
    let mut tally = Tally::default();
    for (status, ignored) in [
        (CellStatus::Correct, false),
        (CellStatus::Missing, true),
        (CellStatus::Extra, true),
    ] {
        if !ignored {
            tally.add(status);
        }
    }
    let tile = Tile {
        lo: [0; 3],
        hi: [4, 1, 4],
        stamp: Some(terrain::Stamp {
            session: 0,
            dimension: 0,
            registry: 0,
            revision: 0,
        }),
        tally,
        ..Tile::default()
    };
    assert!(layer_complete(&[tile], 4, 4));
}

#[test]
fn flush_batches_shrink_as_the_frame_runs_out_of_fuel_and_as_edits_cost_more() {
    // Unmeasured edits are assumed to cost 700, so a fresh frame still sends a large batch.
    assert_eq!(flush_batch(2_000_000, 0), 1024);
    assert!(flush_batch(900_000, 0) < flush_batch(1_500_000, 0));
    // The same fuel buys fewer edits once they are measured as dear.
    assert!(flush_batch(1_000_000, 2_000) < flush_batch(1_000_000, 300));
    // The reserve is never spent on edits: even a barely eligible frame sends a small floor.
    assert_eq!(flush_batch(400_000, 700), 64);
    assert_eq!(flush_batch(0, 700), 64);
    // The batch, priced at its padded cost, never reaches into the reserve.
    for remaining in [450_000u64, 700_000, 1_000_000, 1_800_000] {
        for cost in [100u64, 300, 700, 1_500, 5_000] {
            let batch = flush_batch(remaining, cost) as u64;
            if batch > 64 {
                assert!(batch * cost * 3 / 2 <= remaining - FLUSH_RESERVE);
            }
        }
    }
}

#[cfg(test)]
#[test]
fn a_frame_is_twelve_thin_boxes_inside_the_given_bounds() {
    let mut mesh = draw::Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    add_frame(&mut mesh, [10, 64, -3], [0., 0., 0., 1., 0.125, 1.], GHOST_FRAME);
    assert_eq!(mesh.vertices.len(), 12 * 8);
    assert_eq!(mesh.indices.len(), 12 * 36);
    let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
    for vertex in &mesh.vertices {
        let position = [vertex.position.0, vertex.position.1, vertex.position.2];
        for axis in 0..3 {
            low[axis] = low[axis].min(position[axis]);
            high[axis] = high[axis].max(position[axis]);
        }
    }
    assert!((low[1] - (64. - MARKER_INSET)).abs() < 1e-4 && (high[1] - (64.125 + MARKER_INSET)).abs() < 1e-4);
    assert!((low[0] - (10. - MARKER_INSET)).abs() < 1e-4 && (high[2] - (-2. + MARKER_INSET)).abs() < 1e-4);
}

#[cfg(test)]
fn tile_of_the_reference_guide() -> (crate::Viewer, Tile, [i32; 3], usize) {
    let mut viewer = crate::tests::reference_viewer();
    let placement = viewer.placement.unwrap();
    viewer.preview.reposition(placement, LayerMode::All);
    let size = viewer.schematic.size();
    let (lo, hi) = viewer.preview.bounds(size);
    let mut tile = Tile {
        lo,
        hi,
        stamp: Some(terrain::Stamp {
            session: 0,
            dimension: 0,
            registry: 0,
            revision: 0,
        }),
        materials: vec![[0; 3]; viewer.schematic.palette().len()],
        ..Tile::default()
    };
    let mut first = None;
    for x in lo[0]..hi[0] {
        for z in lo[2]..hi[2] {
            for y in lo[1]..hi[1] {
                let world = [x, y, z];
                let local = placement.world_to_local(size, world).unwrap();
                let id = viewer.schematic.palette_index(local).unwrap() as usize;
                let air = viewer.schematic.is_air_index(id as u32);
                let artifact = viewer.schematic.palette()[id].is_placement_artifact();
                let status = if air || artifact {
                    CellStatus::Correct
                } else {
                    CellStatus::Missing
                };
                if !air && !artifact {
                    tile.tally.add(status);
                    tile.mismatches.push((world, status));
                    if let Some(slot) = material_slot(status, air, false) {
                        tile.materials[id][slot] += 1;
                    }
                    first.get_or_insert((world, id));
                }
                tile.statuses.push(status);
                tile.ignored.push(false);
            }
        }
    }
    let (world, id) = first.expect("a block");
    (viewer, tile, world, id)
}

#[test]
fn a_hot_change_moves_the_counts_and_the_wrong_cell_list_at_once() {
    let (viewer, mut tile, world, id) = tile_of_the_reference_guide();
    let missing = tile.tally.missing;
    let (preview, schematic) = (&viewer.preview, &viewer.schematic);
    // Placed correctly: one less missing, one more built, no longer in the list.
    assert!(tile.apply_hot(preview, schematic, world, CellStatus::Correct, false));
    assert_eq!(tile.tally.missing, missing - 1);
    assert_eq!(tile.tally.correct, 1);
    assert_eq!(tile.built, 1);
    assert_eq!(tile.materials[id][1], 1);
    assert!(tile.mismatches.iter().all(|(cell, _)| *cell != world));
    assert_eq!(tile.status_at(world), Some(CellStatus::Correct));
    assert!(tile.markers_stale);
    // The same observation again changes nothing.
    assert!(!tile.apply_hot(preview, schematic, world, CellStatus::Correct, false));
    // Replaced by a wrong block: it is wrong, it is not built, it is listed.
    assert!(tile.apply_hot(preview, schematic, world, CellStatus::Wrong, false));
    assert_eq!((tile.tally.correct, tile.tally.wrong), (0, 1));
    assert_eq!(tile.built, 0);
    assert!(tile.mismatches.contains(&(world, CellStatus::Wrong)));
    // Broken again: back to missing, and the wrong entry is gone.
    assert!(tile.apply_hot(preview, schematic, world, CellStatus::Missing, false));
    assert_eq!((tile.tally.missing, tile.tally.wrong), (missing, 0));
    assert_eq!(tile.mismatches.iter().filter(|(cell, _)| *cell == world).count(), 1);
    assert!(tile.mismatches.contains(&(world, CellStatus::Missing)));
    assert_eq!(tile.materials[id][0], viewer_total(&viewer, id));
}

#[cfg(test)]
fn viewer_total(viewer: &crate::Viewer, id: usize) -> u16 {
    let name = &viewer.schematic.palette()[id].name;
    viewer
        .schematic
        .palette()
        .iter()
        .enumerate()
        .filter(|(_, block)| &block.name == name && false)
        .count() as u16
        + (0..viewer.schematic.palette().len())
            .filter(|&other| other == id)
            .count() as u16 * 0
        + tile_missing_for(viewer, id)
}

#[cfg(test)]
fn tile_missing_for(viewer: &crate::Viewer, id: usize) -> u16 {
    let placement = viewer.placement.unwrap();
    let size = viewer.schematic.size();
    let (lo, hi) = viewer.preview.bounds(size);
    let mut count = 0;
    for x in lo[0]..hi[0] {
        for z in lo[2]..hi[2] {
            for y in lo[1]..hi[1] {
                let local = placement.world_to_local(size, [x, y, z]).unwrap();
                count += u16::from(viewer.schematic.palette_index(local) == Some(id as u32));
            }
        }
    }
    count
}

#[test]
fn a_hot_change_outside_the_section_or_in_an_unchecked_one_is_ignored() {
    let (viewer, mut tile, world, _) = tile_of_the_reference_guide();
    let (preview, schematic) = (&viewer.preview, &viewer.schematic);
    let far = [world[0] + 1000, world[1], world[2]];
    assert!(!tile.apply_hot(preview, schematic, far, CellStatus::Correct, false));
    tile.stamp = None;
    assert!(!tile.apply_hot(preview, schematic, world, CellStatus::Correct, false));
    assert!(!tile.markers_stale);
}

#[test]
fn a_flat_rail_frame_is_four_cheap_strips_and_a_section_never_outgrows_the_host_limit() {
    let mut mesh = draw::Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    assert!(add_outline(&mut mesh, [3, 64, 3], [0., 0., 0., 1., 0.0625, 1.], GHOST_FRAME));
    assert_eq!(mesh.vertices.len(), 16);
    assert_eq!(mesh.indices.len(), FLAT_FRAME_INDICES);
    for vertex in &mesh.vertices {
        let (x, y, z) = vertex.position;
        assert!((3.0..=4.0).contains(&x) && (3.0..=4.0).contains(&z), "{x} {z}");
        assert!((64.0..=64.2).contains(&y), "{y}");
    }
    // A chest keeps the twelve edges.
    let mut chest = draw::Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    assert!(add_outline(&mut chest, [0, 0, 0], [0.06, 0., 0.06, 0.94, 0.875, 0.94], GHOST_FRAME));
    assert_eq!(chest.indices.len(), FRAME_INDICES);
    // Thousands of rails stop adding outlines before the geometry limit, instead of failing.
    let mut many = draw::Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    let mut drawn = 0;
    for index in 0..20_000 {
        drawn += usize::from(add_outline(&mut many, [index, 0, 0], [0., 0., 0., 1., 0.0625, 1.], GHOST_FRAME));
    }
    assert!(many.indices.len() <= MARKER_INDEX_BUDGET && many.indices.len() < 196_608);
    assert!(drawn > 2_000 && drawn < 20_000);
    assert!(many.vertices.len() < 65_536);
}

#[test]
fn the_render_gate_follows_what_a_section_check_cost() {
    note_render_cost(0);
    RENDER_COST.with(|seen| seen.set(0));
    assert_eq!(render_gate(), 575_000);
    note_render_cost(1_000_000);
    assert_eq!(render_gate(), 1_650_000);
    note_render_cost(5_000_000);
    assert_eq!(render_gate(), 1_800_000);
    // A cheap check afterwards does not make it forget at once.
    note_render_cost(100_000);
    assert!(render_gate() > 1_700_000);
    RENDER_COST.with(|seen| seen.set(0));
}
