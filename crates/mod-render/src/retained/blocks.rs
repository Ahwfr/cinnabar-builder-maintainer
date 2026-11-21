//! Dense guest-owned block grids, meshed with the terrain implementation on a worker.

use super::{DrawError, ErrorCode, Geometry, error};
use assets::{NetworkIdMode, RuntimeAssets};
use mod_api::drawing::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

#[cfg(test)]
mod tests;

/// The plain full cube a hologram draws in place of a block-entity-rendered block when no
/// closer match is known.
const ENTITY_DRAWN_STAND_IN: &str = "minecraft:white_concrete";

/// The ordinary full block whose look is closest to a block-entity-rendered `name`, so a chest
/// is a brown cube and a shulker box has its own color instead of everything being white.
/// The cube is only a stand-in: the model itself is drawn by the block-entity renderer, which
/// a hologram does not have.
fn entity_drawn_stand_in(name: &str) -> String {
    let short = name.strip_prefix("minecraft:").unwrap_or(name);
    let short = short.strip_prefix("waxed_").unwrap_or(short);
    let copper = |family: &str| match family {
        "exposed" => "exposed_copper",
        "weathered" => "weathered_copper",
        "oxidized" => "oxidized_copper",
        _ => "copper_block",
    };
    let stand_in = if let Some(color) = short.strip_suffix("_shulker_box") {
        match color {
            "undyed" => "purple_concrete".to_owned(),
            color => format!("{color}_concrete"),
        }
    } else if let Some(family) = short
        .strip_suffix("copper_chest")
        .or_else(|| short.strip_suffix("copper_golem_statue"))
    {
        copper(family.trim_end_matches('_')).to_owned()
    } else {
        match short {
            "chest" | "trapped_chest" => "spruce_planks",
            "ender_chest" => "obsidian",
            "decorated_pot" => "terracotta",
            "conduit" => "prismarine",
            "frame" | "glow_frame" | "chalkboard" => "oak_planks",
            "bed" | "standing_banner" | "wall_banner" => "white_wool",
            "skeleton_skull" => "bone_block",
            "wither_skeleton_skull" => "coal_block",
            "zombie_head" => "green_concrete",
            "creeper_head" => "lime_concrete",
            "piglin_head" => "pink_concrete",
            "player_head" => "terracotta",
            "dragon_head" | "end_portal" | "end_gateway" => "black_concrete",
            _ => return ENTITY_DRAWN_STAND_IN.to_owned(),
        }
        .to_owned()
    };
    format!("minecraft:{stand_in}")
}

/// Whether the client draws `name` as a real block-entity model over a hologram (chests and
/// shulker boxes), so the terrain mesh leaves a hole instead of a stand-in cube.
pub fn entity_overlaid(name: &str) -> bool {
    let short = name.strip_prefix("minecraft:").unwrap_or(name);
    let short = short.strip_prefix("waxed_").unwrap_or(short);
    matches!(short, "chest" | "trapped_chest" | "ender_chest")
        || short.ends_with("copper_chest")
        || short.ends_with("shulker_box")
}

/// Whether a block of this palette reads the shelter of its whole vertical section column.
/// Only seasonal leaves do; a state that is a bare runtime ID is not known, so it counts.
fn reads_column_shelter(palette: &[BlockState]) -> bool {
    palette.iter().any(|state| match state {
        BlockState::RuntimeId(_) => true,
        BlockState::Named { name, .. } => name.contains("leaves"),
    })
}

/// Whether the block-entity renderer, not the terrain mesher, draws `name`.
fn entity_drawn(name: &str) -> bool {
    assets::facts::block_render_fact(name).kind == assets::facts::BlockRenderKind::EntityDrawn
}

/// A state is resolved in the current session, never a different world's ID space.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BlockState {
    RuntimeId(u32),
    Named { name: String, states: String },
}

/// Independent light channels; absent values use the terrain full-bright defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BlockCell {
    pub palette: u32,
    pub sky_light: Option<u8>,
    pub block_light: Option<u8>,
}

/// Dense x,z,y cells with an explicit palette. Everything outside the grid is air.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BlockGrid {
    /// Host generation used to fence process-local completion feedback.
    pub revision: u64,
    pub size: [u32; 3],
    pub palette: Vec<BlockState>,
    pub cells: Vec<BlockCell>,
}

/// Reads all four authored links when the imported state supplies them.
fn authored_pane_links(canonical: &str) -> Option<u8> {
    let state: serde_json::Value = serde_json::from_str(canonical).ok()?;
    ["north", "east", "south", "west"]
        .into_iter()
        .enumerate()
        .try_fold(0, |mask, (shift, face)| {
            let value = state
                .get(format!("minecraft:connection_{face}"))?
                .get("value")?;
            let connected = value.as_bool().or_else(|| match value.as_u64()? {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            })?;
            Some(mask | (u8::from(connected) << shift))
        })
}

/// An atomic change to a cell in an existing grid palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockEdit {
    pub position: [u32; 3],
    pub cell: BlockCell,
}

#[derive(Debug)]
struct MeshPalette {
    air: u32,
    ids: Vec<u32>,
    pane_links: Vec<(u32, u8)>,
}

/// One immutable section's original terrain streams and grid-local origin.
#[derive(Debug, PartialEq, Eq)]
pub struct BlockSection {
    pub origin: [i32; 3],
    pub mesh: meshing::ChunkMesh,
}

/// Completed terrain output. Counts include every section, model and liquid.
#[derive(Debug, PartialEq, Eq)]
pub struct BlockMesh {
    pub sections: Vec<Arc<BlockSection>>,
    pub triangles: usize,
    pub bytes: usize,
}

/// Retains the registry and meshing settings to which packed material addresses belong.
#[derive(Clone)]
struct MeshContext {
    assets: Arc<RuntimeAssets>,
    mode: NetworkIdMode,
    settings: meshing::MeshRenderSettings,
}
impl std::fmt::Debug for MeshContext {
    /// Asset contents stay outside resource diagnostics.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MeshContext")
    }
}

/// A single worker claim and completion shared across snapshots and instances.
#[derive(Debug)]
pub struct BlockGeometry {
    pub grid: BlockGrid,
    claimed: AtomicBool,
    result: OnceLock<Result<Arc<BlockMesh>, DrawError>>,
    render_error: OnceLock<DrawError>,
    render_ready: AtomicBool,
    remote: OnceLock<Result<(), ErrorCode>>,
    context: OnceLock<MeshContext>,
    palette: OnceLock<Arc<MeshPalette>>,
    previous: Option<Arc<BlockMesh>>,
    dirty: Option<BTreeSet<[i32; 3]>>,
    /// When the mod's edit reached the host, for timing the path to the screen.
    created: std::time::Instant,
}

impl PartialEq for BlockGeometry {
    /// Completion changes readiness, never immutable resource identity.
    fn eq(&self, other: &Self) -> bool {
        self.grid == other.grid
    }
}

impl BlockGeometry {
    /// Pins the first session asset context; changed contexts require a new generation.
    pub fn bind_context(
        &self,
        assets: &Arc<RuntimeAssets>,
        mode: NetworkIdMode,
        settings: meshing::MeshRenderSettings,
    ) -> bool {
        let context = self.context.get_or_init(|| MeshContext {
            assets: Arc::clone(assets),
            mode,
            settings,
        });
        Arc::ptr_eq(&context.assets, assets) && context.mode == mode && context.settings == settings
    }

    /// Milliseconds since this generation reached the host.
    #[must_use]
    pub fn age_ms(&self) -> f64 {
        self.created.elapsed().as_secs_f64() * 1000.0
    }

    /// Publishes complete native GPU arena admission for this generation.
    pub fn complete_render(&self) {
        self.render_ready.store(true, Ordering::Release);
    }

    /// Records a GPU admission failure for this immutable generation.
    pub fn fail_render(&self, error: DrawError) {
        let _ = self.render_error.set(error);
    }
    /// Reports a later GPU admission failure through the host status operation.
    pub fn render_error(&self) -> Option<&DrawError> {
        self.render_error.get()
    }

    /// Returns the local or process-reported completion without copying mesh data.
    pub fn status(&self) -> Option<Result<(), ErrorCode>> {
        self.render_error()
            .map(|error| Err(error.code))
            .or_else(|| match self.result() {
                Some(Err(error)) => Some(Err(error.code)),
                Some(Ok(_)) => self.render_ready.load(Ordering::Acquire).then_some(Ok(())),
                None => None,
            })
            .or_else(|| self.remote.get().copied())
    }

    /// Applies only feedback for the exact submitted grid generation.
    pub fn accept_status(&self, revision: u64, status: Result<(), ErrorCode>) {
        if self.grid.revision == revision {
            if let Err(code) = status {
                self.fail_render(error(code, "block renderer"));
            } else {
                let _ = self.remote.set(status);
            }
        }
    }

    /// Builds changed sections on a worker, sharing unchanged native terrain streams.
    pub fn mesh(
        &self,
        assets: &RuntimeAssets,
        mode: NetworkIdMode,
        named: impl Fn(&str, &str) -> Option<u32>,
        settings: meshing::MeshRenderSettings,
    ) -> Result<BlockMesh, DrawError> {
        if self.palette.get().is_none() {
            let palette = self.grid.resolve_palette(assets, mode, named)?;
            let _ = self.palette.set(Arc::new(palette));
        }
        self.grid.mesh_changed(
            assets,
            mode,
            self.palette.get().expect("resolved palette"),
            settings,
            self.previous.as_deref(),
            self.dirty.as_ref(),
        )
    }
    /// Claims this generation once; callers must complete every successful claim.
    pub fn claim(&self) -> bool {
        !self.claimed.swap(true, Ordering::AcqRel)
    }

    /// Publishes the worker's whole result, including admission failures.
    pub fn complete(&self, result: Result<BlockMesh, DrawError>) {
        let _ = self.result.set(result.map(Arc::new));
    }

    /// Reads readiness without locking, copying cells or performing meshing work.
    pub fn result(&self) -> Option<&Result<Arc<BlockMesh>, DrawError>> {
        self.result.get()
    }
}

impl BlockGrid {
    /// Validates one edit before copying cells and returns its dense storage index.
    fn edit_index(&self, edit: BlockEdit) -> Result<usize, DrawError> {
        if (0..3).any(|axis| edit.position[axis] >= self.size[axis])
            || edit.cell.palette as usize >= self.palette.len()
            || edit.cell.sky_light.is_some_and(|v| v > MAX_BLOCK_LIGHT)
            || edit.cell.block_light.is_some_and(|v| v > MAX_BLOCK_LIGHT)
        {
            return Err(error(ErrorCode::Invalid, "block edit"));
        }
        let [x, y, z] = edit.position.map(|v| v as usize);
        Ok((x * self.size[2] as usize + z) * self.size[1] as usize + y)
    }

    /// Returns the dense x,z,y index, or air outside the supplied bounds.
    pub fn cell(&self, position: [i32; 3]) -> Option<BlockCell> {
        if (0..3).any(|axis| position[axis] < 0 || position[axis] as u32 >= self.size[axis]) {
            return None;
        }
        let [x, y, z] = position.map(|value| value as usize);
        self.cells
            .get((x * self.size[2] as usize + z) * self.size[1] as usize + y)
            .copied()
    }

    /// Checks all allocations before retaining a grid; expensive mesh building happens later.
    pub(super) fn admit(self) -> Result<Geometry, DrawError> {
        let cells = self
            .size
            .into_iter()
            .try_fold(1usize, |total, value| {
                if value == 0 || value > MAX_BLOCK_EXTENT {
                    return None;
                }
                total.checked_mul(value as usize)
            })
            .ok_or(error(ErrorCode::BudgetExceeded, "block grid extent"))?;
        if cells > MAX_BLOCK_CELLS || self.palette.len() > MAX_BLOCK_PALETTE {
            return Err(error(
                ErrorCode::BudgetExceeded,
                "block grid cells or palette",
            ));
        }
        let strings = self
            .palette
            .iter()
            .try_fold(0usize, |total, state| {
                total.checked_add(match state {
                    BlockState::RuntimeId(_) => 0,
                    BlockState::Named { name, states } => name.len().checked_add(states.len())?,
                })
            })
            .ok_or(error(ErrorCode::BudgetExceeded, "block states"))?;
        if strings > MAX_BLOCK_STATE_BYTES {
            return Err(error(ErrorCode::BudgetExceeded, "block states"));
        }
        if self.cells.len() != cells
            || self.palette.is_empty()
            || self.cells.iter().any(|cell| {
                cell.palette as usize >= self.palette.len()
                    || cell.sky_light.is_some_and(|v| v > MAX_BLOCK_LIGHT)
                    || cell.block_light.is_some_and(|v| v > MAX_BLOCK_LIGHT)
            })
        {
            return Err(error(ErrorCode::Invalid, "block cells or light"));
        }
        let bounds = [[0.; 3], self.size.map(|v| v as f32)];
        Ok(Geometry {
            radius: bounds[1].iter().map(|v| v * v).sum::<f32>().sqrt(),
            bounds,
            bytes: cells * size_of::<BlockCell>()
                + self.palette.len() * size_of::<BlockState>()
                + strings,
            blocks: Some(Arc::new(BlockGeometry {
                grid: self,
                claimed: AtomicBool::new(false),
                result: OnceLock::new(),
                render_error: OnceLock::new(),
                render_ready: AtomicBool::new(false),
                remote: OnceLock::new(),
                context: OnceLock::new(),
                palette: OnceLock::new(),
                previous: None,
                dirty: None,
                created: std::time::Instant::now(),
            })),
            vertices: Arc::from([]),
            indices: Arc::from([]),
            distances: Arc::from([]),
            closed: false,
            polyline: false,
            expanded_vertices: 0,
            mesh_vertices: 0,
        })
    }

    /// Builds the same section snapshots, neighbour culling, light and packed streams as terrain.
    /// Call only on a worker. Unknown palette entries fail the whole generation.
    pub fn mesh(
        &self,
        assets: &RuntimeAssets,
        mode: NetworkIdMode,
        named: impl Fn(&str, &str) -> Option<u32>,
        settings: meshing::MeshRenderSettings,
    ) -> Result<BlockMesh, DrawError> {
        let palette = self.resolve_palette(assets, mode, named)?;
        self.mesh_changed(assets, mode, &palette, settings, None, None)
    }

    /// Resolves the palette, drawing a block the block-entity renderer owns (chests, beds,
    /// banners, heads and the rest of the `EntityDrawn` render kind) as a plain cube, since
    /// the terrain mesher gives it no geometry. Only this mesh changes; the grid keeps the block.
    fn resolve_palette(
        &self,
        assets: &RuntimeAssets,
        mode: NetworkIdMode,
        named: impl Fn(&str, &str) -> Option<u32>,
    ) -> Result<MeshPalette, DrawError> {
        let air = assets
            .air_network_id(mode)
            .ok_or(error(ErrorCode::Unsupported, "terrain registry"))?;
        let palette: Vec<u32> = self
            .palette
            .iter()
            .map(|state| {
                let id = match state {
                    BlockState::RuntimeId(id) => Some(*id),
                    BlockState::Named { name, .. } if entity_overlaid(name) => Some(air),
                    BlockState::Named { name, .. } if entity_drawn(name) => {
                        named(&entity_drawn_stand_in(name), "{}")
                            .or_else(|| named(ENTITY_DRAWN_STAND_IN, "{}"))
                    }
                    BlockState::Named { name, states } => named(name, states),
                };
                id.filter(|id| assets.is_known(mode, *id))
                    .ok_or(error(ErrorCode::Invalid, "block state"))
            })
            .collect::<Result<_, _>>()?;
        let pane_links: Vec<_> = self
            .palette
            .iter()
            .zip(&palette)
            .filter_map(|(state, &id)| {
                let BlockState::Named { name, states } = state else {
                    return None;
                };
                (name.ends_with("glass_pane") || name == "minecraft:iron_bars")
                    .then(|| authored_pane_links(states))
                    .flatten()
                    .map(|mask| (id, mask))
            })
            .collect();
        Ok(MeshPalette {
            air,
            ids: palette,
            pane_links,
        })
    }

    /// Reuses immutable section streams outside the complete edit dependency set.
    fn mesh_changed(
        &self,
        assets: &RuntimeAssets,
        mode: NetworkIdMode,
        palette: &MeshPalette,
        settings: meshing::MeshRenderSettings,
        previous: Option<&BlockMesh>,
        dirty: Option<&BTreeSet<[i32; 3]>>,
    ) -> Result<BlockMesh, DrawError> {
        let counts = self
            .size
            .map(|v| v.div_ceil(world::SUB_CHUNK_SIDE as u32) as i32);
        let keys = (0..counts[0])
            .flat_map(|x| (0..counts[2]).flat_map(move |z| (0..counts[1]).map(move |y| [x, y, z])));
        let section_origin = |key: [i32; 3]| key.map(|v| v * world::SUB_CHUNK_SIDE as i32);
        let mut sections = BTreeMap::new();
        let prepare = |sections: &mut BTreeMap<_, _>, key: [i32; 3]| {
            if (0..3).all(|axis| (0..counts[axis]).contains(&key[axis])) {
                sections.entry(key).or_insert_with(|| {
                    let origin = section_origin(key);
                    let ids = std::array::from_fn(|index| {
                        let local = [
                            (index >> 8) as i32,
                            (index & 15) as i32,
                            ((index >> 4) & 15) as i32,
                        ];
                        self.cell(std::array::from_fn(|axis| origin[axis] + local[axis]))
                            .map_or(palette.air, |cell| palette.ids[cell.palette as usize])
                    });
                    world::SubChunk::from_runtime_ids(&ids)
                });
            }
        };
        let previous: BTreeMap<_, _> = previous
            .into_iter()
            .flat_map(|mesh| &mesh.sections)
            .map(|section| (section.origin, section))
            .collect();
        let classifier = meshing::BlockClassifier::new(palette.air);
        let mut result = BlockMesh {
            sections: Vec::new(),
            triangles: 0,
            bytes: 0,
        };
        let bounds = meshing::MeshOutputBounds::new(assets);
        for key in keys {
            let origin = section_origin(key);
            if dirty.is_some_and(|dirty| !dirty.contains(&key)) {
                if let Some(old) = previous.get(&origin) {
                    result.triangles += triangles(&old.mesh);
                    result.bytes += meshing::chunk_publication_byte_len(
                        &old.mesh,
                        &meshing::PackedBiomeRecord::fallback(),
                    ) as usize;
                    result.sections.push(Arc::clone(old));
                }
                continue;
            }

            // Decode source voxels only for changed sections and their dependencies.
            // Unchanged sections retain their compiled terrain streams.
            prepare(&mut sections, key);
            let section = &sections[&key];
            if bounds.for_sub_chunk(section, assets, mode) > MAX_BLOCK_MESH_BYTES as u64 {
                return Err(error(ErrorCode::BudgetExceeded, "block section mesh"));
            }
            if section.storages()[0].palette().values() == [palette.air] {
                continue;
            }
            let seasonal =
                meshing::lighting::mesh_dependency_mask(&classifier, assets, mode, section)
                    .seasonal_foliage;
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        prepare(&mut sections, [key[0] + x, key[1] + y, key[2] + z]);
                    }
                }
            }
            if seasonal {
                for above in key[1] + 2..counts[1] {
                    prepare(&mut sections, [key[0], above, key[2]]);
                }
            }
            let mut neighbours =
                world::MeshNeighbourhood::new(&sections[&key]).with_block_origin(origin);
            if seasonal {
                for above in key[1] + 2..counts[1] {
                    neighbours
                        .insert_column_above(above - key[1], &sections[&[key[0], above, key[2]]]);
                }
            }
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        let delta = [x, y, z];
                        if let Some(adjacent) = sections.get(&std::array::from_fn(|axis| {
                            key[axis] + i32::from(delta[axis])
                        })) {
                            neighbours.insert(delta, adjacent);
                        }
                    }
                }
            }
            let light = |position: [i32; 3]| {
                let cell = self.cell(std::array::from_fn(|axis| position[axis] + origin[axis]));
                meshing::lighting::MeshLightSample::try_new(
                    cell.and_then(|v| v.block_light).unwrap_or(0),
                    cell.and_then(|v| v.sky_light).unwrap_or(MAX_BLOCK_LIGHT),
                )
                .expect("admitted light")
            };
            let mesh = meshing::mesh_sub_chunk_in_neighbourhood_with_options(
                &classifier,
                assets,
                mode,
                &neighbours,
                &light,
                meshing::MeshOptions {
                    rendering: settings,
                    pane_links: &palette.pane_links,
                    ..Default::default()
                },
            );
            result.triangles += triangles(&mesh);
            result.bytes +=
                meshing::chunk_publication_byte_len(&mesh, &meshing::PackedBiomeRecord::fallback())
                    as usize;
            if result.triangles > MAX_BLOCK_TRIANGLES || result.bytes > MAX_BLOCK_MESH_BYTES {
                return Err(error(ErrorCode::BudgetExceeded, "block mesh"));
            }
            if !mesh.is_empty() {
                result
                    .sections
                    .push(Arc::new(BlockSection { origin, mesh }));
            }
        }
        if result.triangles > MAX_BLOCK_TRIANGLES || result.bytes > MAX_BLOCK_MESH_BYTES {
            return Err(error(ErrorCode::BudgetExceeded, "block mesh"));
        }
        Ok(result)
    }
}

/// Counts the terrain quads every native stream actually draws.
fn triangles(mesh: &meshing::ChunkMesh) -> usize {
    (mesh.cube_quads().len()
        + mesh.model_draw_refs().len()
        + mesh.transparent_model_draw_refs().len()
        + mesh.liquid_quads().len())
        * 2
}

impl super::Store {
    /// Replaces grid cells atomically and preserves unchanged section output.
    pub fn update_blocks(
        &mut self,
        id: super::Handle,
        edits: Vec<BlockEdit>,
        permissions: DrawingPermissions,
    ) -> Result<(), DrawError> {
        if !permissions.world_draw || !permissions.render_material {
            return Err(error(ErrorCode::Denied, "block geometry"));
        }
        self.check(id)?;
        if edits.len() > MAX_BLOCK_CELLS {
            return Err(error(ErrorCode::BudgetExceeded, "block edits"));
        }
        let current = self.current();
        let old = Arc::clone(
            current
                .geometry
                .get(&id)
                .ok_or(error(ErrorCode::StaleHandle, "geometry"))?,
        );
        drop(current);
        let block = old
            .blocks()
            .ok_or(error(ErrorCode::Invalid, "block geometry"))?;
        let previous = match block.result() {
            Some(Ok(mesh)) => Some(Arc::clone(mesh)),
            Some(Err(error)) => return Err(error.clone()),
            None => match block.remote.get() {
                Some(Ok(())) => None,
                Some(Err(code)) => return Err(error(*code, "block renderer")),
                None => return Err(error(ErrorCode::InUse, "block mesh pending")),
            },
        };
        let mut changes = BTreeMap::new();
        for edit in edits {
            changes.insert(block.grid.edit_index(edit)?, edit);
        }
        changes.retain(|&index, edit| block.grid.cells[index] != edit.cell);
        if changes.is_empty() {
            return Ok(());
        }
        let mut grid = block.grid.clone();
        grid.revision = grid
            .revision
            .checked_add(1)
            .ok_or(error(ErrorCode::BudgetExceeded, "block generations"))?;
        let mut dirty = BTreeSet::new();
        let side = world::SUB_CHUNK_SIDE as i32;
        let sections = grid.size[1].div_ceil(world::SUB_CHUNK_SIDE as u32) as i32;
        let whole_columns = reads_column_shelter(&grid.palette);
        for (index, edit) in changes {
            grid.cells[index] = edit.cell;
            // Neighbour samples cover culling, liquids and AO. Seasonal leaves also read
            // shelter throughout the same vertical section column, so a grid that has leaves
            // remeshes whole columns; any other grid only the sections the edit can reach.
            let p = edit.position.map(|v| v as i32);
            for dx in -1..=1 {
                for dz in -1..=1 {
                    let column = [(p[0] + dx).div_euclid(side), (p[2] + dz).div_euclid(side)];
                    if whole_columns {
                        for sy in 0..sections {
                            dirty.insert([column[0], sy, column[1]]);
                        }
                    } else {
                        for dy in -1..=1 {
                            let sy = (p[1] + dy).div_euclid(side);
                            if (0..sections).contains(&sy) {
                                dirty.insert([column[0], sy, column[1]]);
                            }
                        }
                    }
                }
            }
        }
        if dirty.is_empty() {
            return Ok(());
        }
        let mut geometry = grid.admit()?;
        let request =
            Arc::get_mut(geometry.blocks.as_mut().expect("block geometry")).expect("new request");
        if block.render_error().is_none()
            && let Some(context) = block.context.get()
        {
            let _ = request.context.set(context.clone());
            if let Some(palette) = block.palette.get() {
                let _ = request.palette.set(Arc::clone(palette));
            }
            request.dirty = previous.as_ref().map(|_| dirty);
            request.previous = previous;
        }
        let bytes = geometry.bytes;
        self.edit_blocks(bytes, |resources| {
            resources.geometry.insert(id, Arc::new(geometry));
            resources.geometry_revision += 1;
            Ok(())
        })
    }
}
