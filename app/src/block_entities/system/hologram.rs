//! Real chest and shulker models for the blocks of a mod's retained block grids.
//!
//! The Schematica hologram is a retained block grid meshed by the terrain pipeline, which has
//! no geometry for block-entity blocks. Its mesh leaves chests and shulker boxes out
//! (`mod_render::retained::blocks::entity_overlaid`); this module finds them in the grids and
//! submits the same models the game draws for placed ones, so a ghost chest looks like a chest.

use std::{collections::HashMap, sync::Arc};

use bevy::math::Vec3;
use mod_render::retained::{
    Anchor, Resources, Transform,
    blocks::{BlockGeometry, BlockGrid, BlockState, entity_overlaid},
};
use render_model::{ChestModel, ChestPair, ChestVariant, Facing, ShulkerModel};

/// Pieces farther than this from the eye are not submitted.
const RADIUS_BLOCKS: f32 = 64.0;
/// Pieces submitted per frame, nearest-first not guaranteed; keeps a chest warehouse bounded.
const MAX_PIECES: usize = 768;
/// Pieces remembered per grid; a grid with more draws only the first of them.
const MAX_PIECES_PER_GRID: usize = 50_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Piece {
    Chest(ChestModel),
    Shulker(ShulkerModel),
}

struct Cached {
    geometry: Arc<BlockGeometry>,
    origin: [i32; 3],
    pieces: Arc<[([i32; 3], Piece)]>,
}

/// Scan results per grid generation. A new generation is a new `Arc`, so a stale entry is
/// never reused.
#[derive(Default)]
pub(super) struct HologramPieces {
    cache: Vec<Cached>,
}

impl HologramPieces {
    /// Appends the pieces of every visible grid within range of `eye` to `out`.
    pub(super) fn gather(
        &mut self,
        retained: &[Arc<Resources>],
        dimension: i32,
        eye: Vec3,
        out: &mut Vec<([i32; 3], Piece)>,
    ) {
        let mut next = Vec::with_capacity(self.cache.len());
        for resources in retained {
            for instance in resources.instances.values() {
                let Anchor::Block {
                    dimension: anchored,
                    position,
                } = instance.anchor
                else {
                    continue;
                };
                if anchored != dimension || instance.tint[3] <= 0. || !is_identity(&instance.transform)
                {
                    continue;
                }
                let Some(blocks) = resources
                    .geometry
                    .get(&instance.geometry)
                    .and_then(|geometry| geometry.blocks())
                else {
                    continue;
                };
                let pieces = self
                    .cache
                    .iter()
                    .chain(&next)
                    .find(|cached| Arc::ptr_eq(&cached.geometry, blocks) && cached.origin == position)
                    .map_or_else(
                        || Arc::<[_]>::from(scan(&blocks.grid, position)),
                        |cached| Arc::clone(&cached.pieces),
                    );
                for &(at, piece) in pieces.iter() {
                    if out.len() >= MAX_PIECES {
                        break;
                    }
                    let center = Vec3::new(at[0] as f32 + 0.5, at[1] as f32 + 0.5, at[2] as f32 + 0.5);
                    if center.distance_squared(eye) <= RADIUS_BLOCKS * RADIUS_BLOCKS {
                        out.push((at, piece));
                    }
                }
                next.push(Cached {
                    geometry: Arc::clone(blocks),
                    origin: position,
                    pieces,
                });
            }
        }
        self.cache = next;
    }
}

/// Only an unrotated, unscaled grid maps its cells onto world blocks one to one.
fn is_identity(transform: &Transform) -> bool {
    transform.translation == [0.; 3]
        && transform.rotation == [0., 0., 0., 1.]
        && transform.scale == [1.; 3]
}

/// Finds every chest and shulker box in `grid`, whose cell `[0, 0, 0]` sits at `origin`.
fn scan(grid: &BlockGrid, origin: [i32; 3]) -> Vec<([i32; 3], Piece)> {
    let kinds: Vec<Option<Piece>> = grid.palette.iter().map(piece_for).collect();
    if kinds.iter().all(Option::is_none) {
        return Vec::new();
    }
    let [sx, sy, sz] = grid.size.map(|extent| extent as usize);
    let mut found = Vec::new();
    'cells: for x in 0..sx {
        for z in 0..sz {
            let row = (x * sz + z) * sy;
            for y in 0..sy {
                let Some(cell) = grid.cells.get(row + y) else {
                    continue;
                };
                if let Some(Some(piece)) = kinds.get(cell.palette as usize) {
                    if found.len() >= MAX_PIECES_PER_GRID {
                        break 'cells;
                    }
                    found.push((
                        [
                            origin[0] + x as i32,
                            origin[1] + y as i32,
                            origin[2] + z as i32,
                        ],
                        *piece,
                    ));
                }
            }
        }
    }
    pair_chests(&mut found);
    found
}

fn piece_for(state: &BlockState) -> Option<Piece> {
    let BlockState::Named { name, states } = state else {
        return None;
    };
    if !entity_overlaid(name) {
        return None;
    }
    let short = name.strip_prefix("minecraft:").unwrap_or(name);
    if short.ends_with("shulker_box") {
        return Some(Piece::Shulker(ShulkerModel {
            color: assets::block_entity_geometry::shulker_color_from_block_name(name)?,
            facing: 1,
            open: 0.,
        }));
    }
    let variant = if short == "ender_chest" {
        ChestVariant::Ender
    } else {
        ChestVariant::from_block_name(name)?
    };
    Some(Piece::Chest(ChestModel {
        variant,
        facing: facing(states).unwrap_or(Facing::North),
        pair: ChestPair::Single,
        lid: 0.,
    }))
}

/// Reads the facing out of a canonical state document such as
/// `{"minecraft:cardinal_direction":{"type":"string","value":"north"}}`.
fn facing(states: &str) -> Option<Facing> {
    let document: serde_json::Value = serde_json::from_str(states).ok()?;
    let value = |key: &str| {
        let entry = document.get(key)?;
        Some(entry.get("value").unwrap_or(entry).clone())
    };
    value("minecraft:cardinal_direction")
        .and_then(|value| value.as_str().and_then(Facing::from_cardinal))
        .or_else(|| {
            value("facing_direction")
                .and_then(|value| value.as_i64())
                .and_then(Facing::from_facing_direction)
        })
}

/// Joins chests that the game would join once pasted: two of the same kind and facing side by
/// side along the axis their fronts are square to. The lower coordinate leads, as for a real
/// double chest without an explicit lead flag, so exactly one half draws the merged model.
fn pair_chests(found: &mut [([i32; 3], Piece)]) {
    let index: HashMap<[i32; 3], usize> = found
        .iter()
        .enumerate()
        .filter(|(_, (_, piece))| matches!(piece, Piece::Chest(model) if model.variant != ChestVariant::Ender))
        .map(|(slot, (at, _))| (*at, slot))
        .collect();
    for slot in 0..found.len() {
        let (at, Piece::Chest(model)) = found[slot] else {
            continue;
        };
        if model.variant == ChestVariant::Ender || model.pair != ChestPair::Single {
            continue;
        }
        let step = match model.facing {
            Facing::North | Facing::South => [1, 0, 0],
            Facing::East | Facing::West => [0, 0, 1],
        };
        let partner = [at[0] + step[0], at[1], at[2] + step[2]];
        let Some(&other) = index.get(&partner) else {
            continue;
        };
        let Piece::Chest(next) = found[other].1 else {
            continue;
        };
        if next.variant != model.variant || next.facing != model.facing || next.pair != ChestPair::Single
        {
            continue;
        }
        found[slot].1 = Piece::Chest(ChestModel {
            pair: ChestPair::Lead { partner },
            ..model
        });
        found[other].1 = Piece::Chest(ChestModel {
            pair: ChestPair::Follower,
            ..next
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chest(facing: Facing) -> Piece {
        Piece::Chest(ChestModel {
            variant: ChestVariant::Normal,
            facing,
            pair: ChestPair::Single,
            lid: 0.,
        })
    }

    fn pair_of(found: &[([i32; 3], Piece)], slot: usize) -> ChestPair {
        match found[slot].1 {
            Piece::Chest(model) => model.pair,
            Piece::Shulker(_) => ChestPair::Single,
        }
    }

    #[test]
    fn facing_reads_bedrock_and_wrapped_state_documents() {
        assert_eq!(
            facing(r#"{"minecraft:cardinal_direction":{"type":"string","value":"east"}}"#),
            Some(Facing::East)
        );
        assert_eq!(
            facing(r#"{"minecraft:cardinal_direction":"south"}"#),
            Some(Facing::South)
        );
        assert_eq!(
            facing(r#"{"facing_direction":{"type":"int","value":4}}"#),
            Some(Facing::West)
        );
        assert_eq!(facing("{}"), None);
        assert_eq!(facing("not json"), None);
    }

    #[test]
    fn neighbours_square_to_their_front_make_a_double_chest_with_the_lower_one_leading() {
        let mut found = vec![
            ([4, 70, 9], chest(Facing::North)),
            ([5, 70, 9], chest(Facing::North)),
            ([8, 70, 2], chest(Facing::East)),
            ([8, 70, 3], chest(Facing::East)),
        ];
        pair_chests(&mut found);
        assert_eq!(pair_of(&found, 0), ChestPair::Lead { partner: [5, 70, 9] });
        assert_eq!(pair_of(&found, 1), ChestPair::Follower);
        assert_eq!(pair_of(&found, 2), ChestPair::Lead { partner: [8, 70, 3] });
        assert_eq!(pair_of(&found, 3), ChestPair::Follower);
    }

    #[test]
    fn chests_that_cannot_join_stay_single() {
        let mut found = vec![
            // Back to back along the front axis.
            ([0, 70, 0], chest(Facing::North)),
            ([0, 70, 1], chest(Facing::North)),
            // Side by side but facing differently.
            ([5, 70, 0], chest(Facing::North)),
            ([6, 70, 0], chest(Facing::South)),
            // Stacked.
            ([10, 70, 0], chest(Facing::North)),
            ([10, 71, 0], chest(Facing::North)),
        ];
        pair_chests(&mut found);
        for slot in 0..found.len() {
            assert_eq!(pair_of(&found, slot), ChestPair::Single, "chest {slot}");
        }
    }

    #[test]
    fn three_in_a_row_pair_once_and_leave_the_last_single() {
        let mut found = vec![
            ([0, 70, 0], chest(Facing::North)),
            ([1, 70, 0], chest(Facing::North)),
            ([2, 70, 0], chest(Facing::North)),
        ];
        pair_chests(&mut found);
        assert_eq!(pair_of(&found, 0), ChestPair::Lead { partner: [1, 70, 0] });
        assert_eq!(pair_of(&found, 1), ChestPair::Follower);
        assert_eq!(pair_of(&found, 2), ChestPair::Single);
    }

    #[test]
    fn ender_chests_never_pair() {
        let ender = Piece::Chest(ChestModel {
            variant: ChestVariant::Ender,
            facing: Facing::North,
            pair: ChestPair::Single,
            lid: 0.,
        });
        let mut found = vec![([0, 70, 0], ender), ([1, 70, 0], ender)];
        pair_chests(&mut found);
        assert_eq!(pair_of(&found, 0), ChestPair::Single);
        assert_eq!(pair_of(&found, 1), ChestPair::Single);
    }

    #[test]
    fn only_unrotated_unscaled_grids_map_onto_world_blocks() {
        let mut transform = Transform::default();
        assert!(is_identity(&transform));
        transform.translation = [0.5, 0., 0.];
        assert!(!is_identity(&transform));
    }
}
