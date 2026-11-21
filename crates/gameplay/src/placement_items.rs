//! Which item builds a planned block, derived from registry names, the item route table and
//! the installed pack's block visuals rather than per-block lists.

use assets::{
    BlockFace, NetworkIdMode, RuntimeAssets, VisualKind,
    facts::{BlockRenderKind, PlacementRule},
};

use crate::movement::PhysicsCollisionRegistries;

/// The floor-standing block a wall-mounted `name` shares its item with, such as
/// `minecraft:wall_banner` to `minecraft:standing_banner` or `minecraft:tube_coral_wall_fan`
/// to `minecraft:tube_coral_fan`. The counterpart must be a registered block.
pub fn standing_form(
    collisions: &PhysicsCollisionRegistries,
    mode: NetworkIdMode,
    name: &str,
) -> Option<String> {
    if !name.contains("wall_") {
        return None;
    }
    ["standing_", ""]
        .into_iter()
        .map(|replacement| name.replacen("wall_", replacement, 1))
        .find(|candidate| {
            collisions
                .block_default_runtime_id(mode, candidate)
                .is_some()
        })
}

/// The wall-mounted block that shares the item of the floor-standing `name`, the inverse of
/// [`standing_form`].
pub fn wall_form(
    collisions: &PhysicsCollisionRegistries,
    mode: NetworkIdMode,
    name: &str,
) -> Option<String> {
    let candidate = if name.contains("standing_") {
        name.replacen("standing_", "wall_", 1)
    } else {
        let (stem, last) = name.rsplit_once('_')?;
        format!("{stem}_wall_{last}")
    };
    (collisions
        .block_default_runtime_id(mode, &candidate)
        .is_some()
        && standing_form(collisions, mode, &candidate).as_deref() == Some(name))
    .then_some(candidate)
}

/// The block a stack of `identifier` places when the stack names no block runtime ID: the
/// route table's block, or the same-named block for block-entity-drawn blocks such as heads
/// and hanging signs, whose items carry no block.
pub fn routed_item_block(
    collisions: &PhysicsCollisionRegistries,
    mode: NetworkIdMode,
    identifier: &str,
) -> Option<u32> {
    if let Some(block) = assets::facts::item_block_route(identifier)
        .and_then(|name| collisions.block_default_runtime_id(mode, name))
    {
        return Some(block);
    }
    let block = collisions.block_default_runtime_id(mode, identifier)?;
    (collisions.block_facts(mode, block).render.kind == BlockRenderKind::EntityDrawn)
        .then_some(block)
}

/// The full block drawn exactly like the double slab `double_slab`, such as dark oak planks for
/// a dark oak double slab. Candidates are full cubes with the same six face materials that are
/// not slabs themselves. The pick is the name made most of the slab's material, so stone wins
/// over infested stone for a normal stone double slab, then the shorter name. `None` when no block matches or the block is no double slab.
pub fn double_slab_full_block(
    collisions: &PhysicsCollisionRegistries,
    runtime: &RuntimeAssets,
    mode: NetworkIdMode,
    double_slab: u32,
) -> Option<u32> {
    if collisions.block_facts(mode, double_slab).placement != PlacementRule::DoubleSlab {
        return None;
    }
    let name = collisions.block_identifier(mode, double_slab)?;
    let material = name
        .strip_prefix("minecraft:")
        .unwrap_or(name)
        .replace("_double_slab", "");
    let faces = |block: u32| {
        let visual = runtime.resolve_known(mode, block)?;
        (visual.kind() == VisualKind::Cube)
            .then(|| BlockFace::ALL.map(|face| visual.face(face).material_id()))
    };
    let wanted = faces(double_slab)?;
    collisions
        .block_names()
        .filter_map(|candidate| {
            let block = collisions.block_default_runtime_id(mode, candidate)?;
            let rule = collisions.block_facts(mode, block).placement;
            (!matches!(rule, PlacementRule::Slab | PlacementRule::DoubleSlab)
                && collisions.block_is_full_cube(mode, block)
                && faces(block) == Some(wanted))
            .then(|| {
                let short = candidate.strip_prefix("minecraft:").unwrap_or(candidate);
                let coverage = longest_shared_run(&material, short) * 1024 / short.len().max(1);
                ((coverage, std::cmp::Reverse(short.len())), block)
            })
        })
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, block)| block)
}

/// The length of the longest substring `a` and `b` share.
fn longest_shared_run(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut best = 0;
    let mut previous = vec![0; b.len() + 1];
    for &x in a {
        let mut current = vec![0; b.len() + 1];
        for (j, &y) in b.iter().enumerate() {
            if x == y {
                current[j + 1] = previous[j] + 1;
                best = best.max(current[j + 1]);
            }
        }
        previous = current;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::longest_shared_run;

    #[test]
    fn shared_runs_prefer_the_slab_material() {
        assert_eq!(longest_shared_run("dark_oak", "dark_oak_planks"), 8);
        assert_eq!(longest_shared_run("normal_stone", "stone"), 5);
        assert_eq!(longest_shared_run("normal_stone", "infested_stone"), 6);
    }
}
