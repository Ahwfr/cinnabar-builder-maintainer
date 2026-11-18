use crate::{budget, drawing as draw, terrain};
use schematica_core::{BlockState, CellStatus, LayerMode, Placement, Schematic, StateValue, Tally};
use serde_json::{Map, json};
use std::collections::BTreeMap;

mod coverage;
pub(crate) use coverage::on_distant_scan_frame;

const SMALL_TILE_EDGE: i32 = 4;
const LARGE_TILE_EDGE: i32 = 5;
const MAX_VISIBLE_TILES: i32 = 128;
const WINDOW_TILES: i32 = 5;
const DEFAULT_LIGHT: u32 = 0xffff0000;
// Lowering triangle vertices to WASM host calls dominates dense mismatch tiles.
// The verifier and material counts still track every cell.
/// Host block-grid parameter slots (`mod_api::drawing::BLOCK_GRID_*_PARAM`).
const OUTLINE_PARAM: usize = 10;
const WASH_PARAM: usize = 11;
const WASH_COLOR_PARAM: usize = 12;
/// The light blue that ghost blocks blend toward: #33B3E6, the hue Litematica uses for
/// missing blocks.
const GHOST_WASH: [f32; 3] = [0.2, 0.702, 0.902];
/// How far status marker boxes extend past the block on each side.
const MARKER_INSET: f32 = 0.0075;

#[derive(Default)]
pub struct Preview {
    placement: Option<Placement>,
    palette: Vec<draw::NamedBlock>,
    layers: Option<LayerMode>,
}

impl Preview {
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
    pub fallback: bool,
    pub error: Option<String>,
    materials: Vec<[u16; 3]>,
    statuses: Vec<CellStatus>,
    pub mismatches: Vec<([i32; 3], CellStatus)>,
    markers: Option<DrawPair>,
    blocks: Option<DrawPair>,
    blocks_ready: bool,
    pending_frames: u8,
    pub(super) ghost_dirty: bool,
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
    pair: Option<DrawPair>,
    retiring: Option<DrawPair>,
    pub failed: bool,
    pub error: Option<String>,
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
        let hidden = ghost_hidden(status, self.hidden[index]);
        if hidden != self.hidden[index] {
            self.hidden[index] = hidden;
            self.changes.insert(pos, if hidden { 0 } else { id + 1 });
        }
    }

    pub fn flush(&mut self) {
        let Some(pair) = &self.pair else {
            return;
        };
        match draw::geometry_status(pair.geometry) {
            Ok(draw::GeometryState::Ready) if !self.changes.is_empty() => {
                let edits: Vec<_> = self
                    .changes
                    .iter()
                    .take(192)
                    .map(|(&[x, y, z], &palette)| draw::BlockEdit {
                        position: (x, y, z),
                        cell: draw::BlockCell {
                            palette,
                            sky_light: None,
                            block_light: None,
                        },
                    })
                    .collect();
                if draw::update_blocks(pair.geometry, &edits).is_ok() {
                    for edit in edits {
                        let (x, y, z) = edit.position;
                        self.changes.remove(&[x, y, z]);
                    }
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
        self.materials.clear();
        self.mismatches.clear();
        self.tally = Tally::default();
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
                if budget::remaining_fuel() < 575_000 {
                    return (false, false);
                }
                let previous_revision = self.stamp.map(|stamp| stamp.revision);
                self.render(
                    preview,
                    schematic,
                    dimension,
                    lightness.filter(|_| !self.fallback),
                    whole,
                    region,
                );
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
        let p = preview.placement.unwrap();
        let mut comparisons = std::collections::HashMap::new();
        for x in 0..sx {
            for z in 0..sz {
                for y in 0..sy {
                    let world = [self.lo[0] + x, self.lo[1] + y, self.lo[2] + z];
                    let local = p.world_to_local(schematic.size(), world).unwrap();
                    let id = schematic.palette_index(local).unwrap();
                    let expected = &preview.palette[id as usize];
                    let status = if schematic.palette()[id as usize].is_placement_artifact() {
                        CellStatus::Correct
                    } else {
                        let cell = ids.len();
                        if region
                            .loaded
                            .get(cell / 64)
                            .is_some_and(|bits| bits & (1 << (cell % 64)) != 0)
                            && let Some(&actual) = region.states.get(cell)
                        {
                            *comparisons.entry((id, actual)).or_insert_with(|| {
                                compare(&region, cell, expected, schematic.is_air_index(id))
                            })
                        } else {
                            CellStatus::Unloaded
                        }
                    };
                    ids.push(id);
                    statuses.push(status);
                }
            }
        }
        if self.stamp.is_some() && self.statuses == statuses {
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
                    let air = schematic.is_air_index(id as u32);
                    let ignored = schematic.palette()[id].is_placement_artifact();
                    let mut word = DEFAULT_LIGHT;
                    if (!air && !ignored) || status == CellStatus::Extra {
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
                        CellStatus::Wrong => {
                            materials[id][0] += 1;
                            Some((1., 0.22, 0.42, 0.44))
                        }
                        CellStatus::WrongState => {
                            materials[id][0] += 1;
                            Some((1., 0.69, 0.08, 0.42))
                        }
                        CellStatus::Extra => Some((0.98, 0.14, 0.17, 0.40)),
                        CellStatus::Correct if !air && !ignored => {
                            materials[id][1] += 1;
                            None
                        }
                        _ => None,
                    };
                    cells.push(word);
                    if let Some(color) = color {
                        let exposed =
                            exposed_marker_faces([x, y, z], [sx, sy, sz], &statuses, status);
                        add_box(&mut highlights, [x, y, z], color, exposed);
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
        if !highlights.vertices.is_empty() {
            match create(draw::GeometrySpec::Triangles(highlights), anchor, 0., true) {
                Ok(pair) => {
                    if let Some(previous) = self.markers.replace(pair) {
                        previous.remove();
                    }
                }
                Err(e) => self.error = Some(format!("Overlay: {}", e.detail)),
            }
        } else if let Some(previous) = self.markers.take() {
            previous.remove();
        }
        self.stamp = Some(region.stamp);
        self.statuses = statuses;
        self.materials = materials;
        self.tally = tally;
    }
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
        (false, false) if name != expected.name => CellStatus::Wrong,
        (false, false) => match block.canonical_state.as_deref() {
            Some(state) if preview_states_match(name, &expected.states, state) => {
                CellStatus::Correct
            }
            Some(_) => CellStatus::WrongState,
            None => CellStatus::Unloaded,
        },
    }
}

/// Pane arms and wall geometry are resolved by adjacent blocks when placed.
fn preview_states_match(name: &str, expected: &str, actual: &str) -> bool {
    if expected == actual {
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
                || (hopper && key == "toggle_bit"))
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

#[cfg(test)]
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
    assert!(!preview_states_match("minecraft:stone", "{}", connected));
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
