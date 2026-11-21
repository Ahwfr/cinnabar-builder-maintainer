//! Finds a support click and reported facing that produce the planned block state.

use super::*;
use bevy::{
    math::EulerRot,
    prelude::{Quat, Vec3},
};
use gameplay::movement::UnsentSampleView;
use mod_host::CreativePlacementOffer;

pub(super) type Support = ([i32; 3], u8, u32);

#[derive(Clone, Copy)]
pub(super) struct PlacementAim {
    pub rotation: Quat,
    pub support_ray: Option<Vec3>,
    pub interaction_ray: Vec3,
    hidden_height: Option<f64>,
    pub support: Support,
}

impl PlacementAim {
    pub(super) fn observed(rotation: Quat, support: Support, interaction_ray: Vec3) -> Self {
        Self {
            rotation,
            support,
            support_ray: None,
            interaction_ray,
            hidden_height: None,
        }
    }
}

/// The same look representation as the normal movement and placement systems.
pub(super) fn wire_facing(rotation: Quat) -> [f32; 2] {
    let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
    [
        -pitch.to_degrees(),
        gameplay::movement::wire_yaw(180.0 - yaw.to_degrees()),
    ]
}

/// The horizontal look quadrant, numbered as placement numbers yaw quadrants.
pub(super) fn look_quadrant(rotation: Quat) -> usize {
    (wire_facing(rotation)[1] / 90.0 + 0.5)
        .floor()
        .rem_euclid(4.0) as usize
}

/// The planned `facing_direction`, 0 down, 1 up, then north, south, west and east.
fn planned_face(planned: &serde_json::Map<String, serde_json::Value>) -> Option<usize> {
    let value = planned
        .get("minecraft:facing_direction")
        .or_else(|| planned.get("facing_direction"))?;
    match value["value"].as_str() {
        Some(name) => ["down", "up", "north", "south", "west", "east"]
            .iter()
            .position(|candidate| *candidate == name),
        None => usize::try_from(value["value"].as_u64()?)
            .ok()
            .filter(|face| *face <= 5),
    }
}

/// The face to click for blocks that point out of the face they are placed against (end rods,
/// lightning rods, amethyst buds and clusters, ladders), whose planned facing is that face.
pub(super) fn wanted_face(offer: &CreativePlacementOffer) -> Option<u8> {
    let name = offer.name.as_str();
    if ceiling_lever(offer) {
        return Some(0);
    }
    // Wall signs, wall banners and heads face out of the face they are placed against.
    let face_derived = name.contains("wall_")
        || name.ends_with("_skull")
        || name.ends_with("_head")
        || name == "minecraft:end_rod"
        || name == "minecraft:ladder"
        || name.ends_with("lightning_rod")
        || name.ends_with("amethyst_bud")
        || name == "minecraft:amethyst_cluster";
    if !face_derived {
        return None;
    }
    let planned =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&offer.states).ok()?;
    planned_face(&planned).and_then(|face| u8::try_from(face).ok())
}

/// A lever planned on a ceiling, which only a click on the underside of the block above places.
fn ceiling_lever(offer: &CreativePlacementOffer) -> bool {
    offer.name == "minecraft:lever"
        && serde_json::from_str::<serde_json::Value>(&offer.states).is_ok_and(|states| {
            states["lever_direction"]["value"]
                .as_str()
                .is_some_and(|direction| direction.starts_with("down"))
        })
}

/// The look quadrant whose placement gives a block the client cannot predict (observers,
/// droppers, furnaces and similar) the facing the guide plans. None when yaw cannot decide it,
/// such as an upward or downward facing, or when the block's facing rule is not known.
pub(super) fn wanted_quadrant(offer: &CreativePlacementOffer) -> Option<usize> {
    let planned =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&offer.states).ok()?;
    if let Some(direction) = planned
        .get("minecraft:cardinal_direction")
        .and_then(|state| state["value"].as_str())
    {
        // Anvils face a quarter turn left of the player (`anvil_states` in gameplay); every
        // other block with this state faces the player.
        let table = if offer.name.ends_with("anvil") {
            ["east", "south", "west", "north"]
        } else {
            ["north", "east", "south", "west"]
        };
        return table.iter().position(|name| *name == direction);
    }
    let face = planned_face(&planned)?;
    // These face the player when placed; observers and pistons face away from the player.
    let toward = match offer.name.as_str() {
        "minecraft:dropper"
        | "minecraft:dispenser"
        | "minecraft:barrel"
        | "minecraft:command_block"
        | "minecraft:repeating_command_block"
        | "minecraft:chain_command_block" => face,
        "minecraft:observer" | "minecraft:piston" | "minecraft:sticky_piston" => face ^ 1,
        _ => return None,
    };
    [2, 5, 3, 4]
        .iter()
        .position(|candidate| *candidate == toward)
}

/// Prefers a single look for picking and facing, then tries a separate reported yaw.
pub(super) fn correction(
    player: &client_state::PlayerRuntime,
    context: &BlockUseContext,
    observed: &FrozenBlockObservation,
    state: &UnsentSampleView,
    expected: &CreativePlacementOffer,
    bridge: bool,
) -> Option<PlacementAim> {
    if expected.states == "{}"
        || expected.name.ends_with("_wall")
        || matches!(
            expected.name.as_str(),
            "minecraft:cauldron" | "minecraft:trip_wire"
        )
        || player.facts.mount_unique_id().is_some()
    {
        return None;
    }
    let stream = context.client_world.stream.as_ref()?;
    let mode = stream.network_id_mode();
    let caps = player.facts.game_mode_capabilities()?;
    if !caps.can_build {
        return None;
    }
    let held = super::build::held_placement_block(
        &context.collisions,
        stream,
        observed.selection.item.block_runtime_id(),
        || {
            stream
                .authority()
                .item_identifier(observed.selection.item.network_id())
        },
    )?;
    let pick = crate::interaction_authority::pick_context(
        stream,
        &context.collisions,
        player.facts.player_game_mode(),
        Some(observed.selection.item.block_runtime_id()),
    );
    let authority = stream.authority();
    let overlays = authority.decode_ids(stream.current_dimension());
    let world = PaletteWorld::new(
        stream.collision_store(),
        context.collisions.registry(mode),
        stream.current_dimension(),
        pick,
    )
    .with_cell_overlays(&overlays);
    if !world.is_air(expected.position).ok()?
        && !(gameplay::placement_plants::supported(&expected.name)
            && gameplay::placement_plants::replaceable(
                context
                    .collisions
                    .block_facts(mode, world.primary_runtime_id(expected.position).ok()?),
            ))
    {
        return None;
    }
    let planned = placement_properties(expected)?;
    if context.collisions.block_facts(mode, held).placement
        == assets::facts::PlacementRule::Unresolved
    {
        // Nothing predicts this block's state, but its facing follows the look direction.
        // Turn to the planned facing and keep the support the player already aims at.
        let eye = Vec3::from_array(observed.ray.origin);
        let forward = Vec3::from_array(observed.ray.direction).try_normalize()?;
        let view = *context.view;
        if let Some(face) = wanted_face(expected) {
            // The block points out of the face it is placed against, so the click must land on
            // that neighbour's face. Report that click without turning the camera.
            if observed.target.face == face
                && observed.target.position == placement_cell(expected.position, face ^ 1)
            {
                return bridge.then(|| {
                    PlacementAim::observed(
                        view.rotation(),
                        (
                            observed.target.position,
                            observed.target.face,
                            observed.target.runtime_id,
                        ),
                        forward,
                    )
                });
            }
            if world
                .is_air(placement_cell(expected.position, face ^ 1))
                .unwrap_or(true)
            {
                return None;
            }
            let hit = hidden_support_hit_at(&world, expected.position, face, 0.5)?;
            let mut candidate = observed.clone();
            apply_support_hit(&mut candidate, hit);
            if !gameplay::interaction_authority::pick_point_in_range(
                candidate.target.hit_world,
                state.position,
                observed.reach,
            ) {
                return None;
            }
            return Some(PlacementAim {
                rotation: view.rotation(),
                support_ray: None,
                interaction_ray: Vec3::from_array(candidate.target.hit_world) - eye,
                hidden_height: Some(0.5),
                support: (candidate.target.position, face, candidate.target.runtime_id),
            });
        }
        let want = wanted_quadrant(expected)?;
        let support = (
            observed.target.position,
            observed.target.face,
            observed.target.runtime_id,
        );
        if look_quadrant(view.rotation()) == want {
            return bridge.then(|| PlacementAim::observed(view.rotation(), support, forward));
        }
        let rotation =
            separate_facing(view.rotation(), |rotation| look_quadrant(rotation) == want)?;
        return Some(PlacementAim::observed(rotation, support, forward));
    }
    let stair = planned.contains_key("weirdo_direction");
    let planned_upper = planned_upper(&planned);
    let door = expected.name.ends_with("_door");
    let matches = |candidate: &FrozenBlockObservation, view: LocalViewPose| {
        if !support_produces_half(
            planned_upper,
            candidate.target.face,
            candidate.target.relative_hit[1],
        ) || !support_produces_axis(expected, candidate.target.face)
        {
            return false;
        }
        if stair && !stable_stair_yaw(view.rotation()) {
            return false;
        }
        let surroundings =
            use_surroundings(context, &world, candidate, state.position, state.sneaking);
        if LocalUse::resolve(
            &candidate.selection.item,
            candidate.target.position,
            candidate.target.face,
            &surroundings,
            &caps,
        ) == LocalUse::Interact
        {
            return false;
        }
        let prediction = predicted_placement(
            &context.collisions,
            stream,
            &world,
            held,
            candidate,
            &surroundings,
            view,
        );
        if door && prediction.is_none() {
            return door_matches_facing(expected, view.rotation())
                && surroundings
                    .destination(candidate.target.position, candidate.target.face)
                    .0
                    == expected.position
                && paired_item_fits(
                    &context.collisions,
                    stream,
                    &world,
                    held,
                    candidate,
                    &surroundings,
                    view,
                );
        }
        prediction.is_some_and(|placement| {
            placement.position == expected.position
                && context.collisions.block_identifier(mode, placement.block)
                    == Some(expected.name.as_str())
                && context
                    .collisions
                    .block_canonical_state(mode, placement.block)
                    .is_some_and(|actual| matches_planned_state(&planned, actual))
        })
    };
    let eye = Vec3::from_array(observed.ray.origin);
    let forward = Vec3::from_array(observed.ray.direction).try_normalize()?;
    let mut view = *context.view;
    if matches(observed, view) {
        return bridge.then(|| {
            PlacementAim::observed(
                view.rotation(),
                (
                    observed.target.position,
                    observed.target.face,
                    observed.target.runtime_id,
                ),
                forward,
            )
        });
    }
    // Changing yaw alone is cheaper and more stable than finding a different support ray.
    if let Some(rotation) = separate_facing(view.rotation(), |rotation| {
        view.set_rotation(rotation);
        matches(observed, view)
    }) {
        return Some(PlacementAim::observed(
            rotation,
            (
                observed.target.position,
                observed.target.face,
                observed.target.runtime_id,
            ),
            forward,
        ));
    }
    let mut best = None;
    let mut best_dot = f32::NEG_INFINITY;
    let mut separate = None;
    let mut separate_dot = f32::NEG_INFINITY;
    for face in (0..6).filter(|&face| (!door || face == 1) && support_produces_axis(expected, face))
    {
        let clicked = placement_cell(expected.position, face ^ 1);
        if world.is_air(clicked).unwrap_or(true) {
            continue;
        }
        for direction in support_directions(clicked, face, eye) {
            let dot = direction.dot(forward);
            if dot <= best_dot {
                continue;
            }
            let actor = gameplay::melee::pick_actor(
                authority.remote_actors(),
                authority.local_mount_unique_id(),
                observed.ray.origin,
                direction.to_array(),
                observed.reach,
            );
            let Some(hit) = support_hit(&world, eye, direction, observed.reach, actor) else {
                continue;
            };
            if hit.block_pos != clicked || hit.face != face || hit.overlay.is_some() {
                continue;
            }
            let mut candidate = observed.clone();
            candidate.target.position = hit.block_pos;
            candidate.target.face = hit.face;
            candidate.target.runtime_id = hit.runtime_id;
            candidate.target.relative_hit = [
                hit.hit_local.x as f32,
                hit.hit_local.y as f32,
                hit.hit_local.z as f32,
            ];
            candidate.target.hit_world = [
                hit.hit_world.x as f32,
                hit.hit_world.y as f32,
                hit.hit_world.z as f32,
            ];
            if !gameplay::interaction_authority::pick_point_in_range(
                candidate.target.hit_world,
                state.position,
                observed.reach,
            ) {
                continue;
            }
            let rotation = client_presentation::camera::look_at_target(eye, eye + direction);
            let mut view = *context.view;
            view.set_rotation(rotation);
            let aim = PlacementAim {
                rotation: view.rotation(),
                support_ray: Some(direction),
                interaction_ray: direction,
                hidden_height: None,
                support: (clicked, face, hit.runtime_id),
            };
            if matches(&candidate, view) {
                best_dot = dot;
                best = Some(aim);
            } else if best.is_none()
                && dot > separate_dot
                && let Some(rotation) = separate_facing(aim.rotation, |rotation| {
                    view.set_rotation(rotation);
                    matches(&candidate, view)
                })
            {
                separate_dot = dot;
                separate = Some(PlacementAim { rotation, ..aim });
            }
        }
    }
    best.or(separate)
        .or_else(|| {
            if bridge || expected.strict_state {
                for face in (0..6)
                    .filter(|&face| (!door || face == 1) && support_produces_axis(expected, face))
                {
                    for height in [0.25, 0.75, 0.5] {
                        let Some(hit) =
                            hidden_support_hit_at(&world, expected.position, face, height)
                        else {
                            continue;
                        };
                        let mut candidate = observed.clone();
                        apply_support_hit(&mut candidate, hit);
                        if !gameplay::interaction_authority::pick_point_in_range(
                            candidate.target.hit_world,
                            state.position,
                            observed.reach,
                        ) {
                            continue;
                        }
                        let mut view = *context.view;
                        let rotation = if matches(&candidate, view) {
                            Some(view.rotation())
                        } else {
                            separate_facing(view.rotation(), |rotation| {
                                view.set_rotation(rotation);
                                matches(&candidate, view)
                            })
                        };
                        if let Some(rotation) = rotation {
                            return Some(PlacementAim {
                                rotation,
                                support_ray: None,
                                interaction_ray: Vec3::from_array(candidate.target.hit_world) - eye,
                                hidden_height: Some(height),
                                support: (
                                    candidate.target.position,
                                    face,
                                    candidate.target.runtime_id,
                                ),
                            });
                        }
                    }
                }
            }
            None
        })
        .or_else(|| {
            if !expected.strict_state {
                return None;
            }
            let face = air_place_face(expected)?;
            let air = stream.air_block_id();
            let hit = super::target::air_hit(
                &world,
                expected.position,
                face,
                air,
                eye,
                state.position,
                observed.reach,
            )?;
            let mut candidate = observed.clone();
            apply_support_hit(&mut candidate, hit);
            let direction = (Vec3::from_array(candidate.target.hit_world) - eye).try_normalize()?;
            let mut view = *context.view;
            let rotation = if matches(&candidate, view) {
                view.rotation()
            } else {
                separate_facing(view.rotation(), |rotation| {
                    view.set_rotation(rotation);
                    matches(&candidate, view)
                })?
            };
            Some(PlacementAim::observed(
                rotation,
                (expected.position, face, air),
                direction,
            ))
        })
}

/// Keeps stair corrections away from the boundary between two wire-facing quadrants.
fn stable_stair_yaw(rotation: Quat) -> bool {
    let offset = (wire_facing(rotation)[1] + 45.0).rem_euclid(90.0);
    (2.0..=88.0).contains(&offset)
}

/// Changes only horizontal facing while keeping the sampled click and pitch.
fn separate_facing(rotation: Quat, mut matches: impl FnMut(Quat) -> bool) -> Option<Quat> {
    let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
    let quadrant = std::f32::consts::FRAC_PI_2;
    let cardinal = (yaw / quadrant).round() * quadrant;
    [0.0, quadrant, -quadrant, std::f32::consts::PI]
        .into_iter()
        .map(|offset| Quat::from_euler(EulerRot::YXZ, cardinal + offset, pitch, 0.0))
        .find(|&candidate| matches(candidate))
}

/// DoorItem uses a quarter-turn offset from the reported horizontal facing.
pub(super) fn door_matches_facing(offer: &CreativePlacementOffer, rotation: Quat) -> bool {
    let Some(planned) = placement_properties(offer) else {
        return false;
    };
    let yaw = wire_facing(rotation)[1];
    let quadrant = (yaw / 90.0 + 0.5).floor().rem_euclid(4.0) as usize;
    let direction = ["west", "north", "east", "south"][quadrant];
    planned
        .get("minecraft:cardinal_direction")
        .is_none_or(|state| state["value"].as_str() == Some(direction))
}

/// Stock Bedrock resolves a click into air as a top-face placement. A ceiling lever has no
/// air placement.
pub(super) fn air_place_face(offer: &CreativePlacementOffer) -> Option<u8> {
    let planned =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&offer.states).ok()?;
    let horizontal_pillar = matches!(
        planned
            .get("pillar_axis")
            .and_then(|state| state["value"].as_str()),
        Some("x" | "z")
    );
    // DoorItem needs a solid floor. An air click cannot make a freestanding door.
    (!ceiling_lever(offer)
        && !horizontal_pillar
        && !offer.name.ends_with("_door")
        && planned_upper(&planned) != Some(true))
    .then_some(1)
}

fn planned_upper(planned: &serde_json::Map<String, serde_json::Value>) -> Option<bool> {
    planned
        .get("upside_down_bit")
        .and_then(|property| property["value"].as_u64())
        .map(|value| value == 1)
        .or_else(|| {
            planned.get("minecraft:vertical_half").and_then(|property| {
                match property["value"].as_str()? {
                    "top" => Some(true),
                    "bottom" => Some(false),
                    _ => None,
                }
            })
        })
}

/// Imported schematics can omit properties which Bedrock derives from neighbors.
fn matches_planned_state(
    planned: &serde_json::Map<String, serde_json::Value>,
    actual: &str,
) -> bool {
    let Ok(actual) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(actual)
    else {
        return false;
    };
    planned
        .iter()
        .all(|(name, value)| name == "minecraft:corner" || actual.get(name) == Some(value))
}

pub(super) fn strict_state_matches(offer: &CreativePlacementOffer, actual: &str) -> bool {
    placement_properties(offer).is_some_and(|planned| matches_planned_state(&planned, actual))
}

/// Only reconcile a wooden trapdoor whose placement state already matches.
/// A predicted toggle immediately satisfies this predicate, preventing hold spam.
pub(super) fn needs_toggle(
    offer: &CreativePlacementOffer,
    actual_name: Option<&str>,
    actual: Option<&str>,
) -> bool {
    if !offer.correct_aim
        || !offer.name.ends_with("trapdoor")
        || offer.name == "minecraft:iron_trapdoor"
        || actual_name != Some(offer.name.as_str())
    {
        return false;
    }
    let Some(actual) = actual else { return false };
    let Ok(planned) = serde_json::from_str::<serde_json::Value>(&offer.states) else {
        return false;
    };
    let Ok(current) = serde_json::from_str::<serde_json::Value>(actual) else {
        return false;
    };
    let bits = planned["open_bit"]["value"]
        .as_u64()
        .zip(current["open_bit"]["value"].as_u64());
    bits.is_some_and(|(want, have)| want <= 1 && have <= 1 && want != have)
        && strict_state_matches(offer, actual)
}

/// Keeps placement-controlled properties, leaving connected geometry and trapdoor opening to gameplay.
fn placement_properties(
    offer: &CreativePlacementOffer,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let mut planned: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&offer.states).ok()?;
    let wall = offer.name.ends_with("_wall");
    let pane = offer.name.ends_with("glass_pane") || offer.name == "minecraft:iron_bars";
    let wire = offer.name == "minecraft:trip_wire";
    let leaves = offer.name.as_str().ends_with("_leaves")
        || matches!(
            offer.name.as_str(),
            "minecraft:leaves" | "minecraft:leaves2"
        );
    let redstone = offer.name.as_str() == "minecraft:redstone_wire";
    let observer = offer.name.as_str() == "minecraft:observer";
    let hopper = offer.name == "minecraft:hopper";
    let dispenser = matches!(
        offer.name.as_str(),
        "minecraft:dropper" | "minecraft:dispenser"
    );
    let repeater = offer.name.ends_with("_repeater");
    let comparator = offer.name.ends_with("_comparator");
    let fence = offer.name.ends_with("_fence");
    let trapdoor = offer.name.ends_with("trapdoor");
    let gate = offer.name.ends_with("fence_gate");
    let door = offer.name.ends_with("_door");
    let soil_plant = gameplay::placement_plants::supported(&offer.name);
    planned.retain(|key, _| {
        !(gameplay::placement_world_states::world_controlled_name(key)
            || (wall && (key == "wall_post_bit" || key.starts_with("wall_connection_type_")))
            || ((pane || wire || fence) && key.starts_with("minecraft:connection_"))
            || ((trapdoor || gate || door) && key == "open_bit")
            || (door && key == "door_hinge_bit")
            || (soil_plant && key == "age_bit")
            || (gate && key == "in_wall_bit")
            || (leaves && matches!(key.as_str(), "persistent_bit" | "update_bit"))
            || (redstone && key.starts_with("redstone_"))
            || (observer && key == "powered_bit")
            // A hopper takes whatever facing the click gives it.
            || (hopper && matches!(key.as_str(), "toggle_bit" | "facing_direction"))
            || (dispenser && key == "triggered_bit")
            || (repeater && key == "repeater_delay")
            || (comparator && matches!(key.as_str(), "output_lit_bit" | "output_subtract_bit"))
            || (wire
                && matches!(
                    key.as_str(),
                    "attached_bit" | "powered_bit" | "suspended_bit"
                )))
    });
    Some(planned)
}

/// Recasts the saved support ray from the current eye before submitting the click.
pub(super) fn recapture(
    player: &client_state::PlayerRuntime,
    context: &BlockUseContext,
    mut observed: FrozenBlockObservation,
    state: &UnsentSampleView,
    aim: PlacementAim,
) -> Option<FrozenBlockObservation> {
    let stream = context.client_world.stream.as_ref()?;
    let authority = stream.authority();
    let overlays = authority.decode_ids(stream.current_dimension());
    let pick = crate::interaction_authority::pick_context(
        stream,
        &context.collisions,
        player.facts.player_game_mode(),
        Some(observed.selection.item.block_runtime_id()),
    );
    let world = PaletteWorld::new(
        stream.collision_store(),
        context.collisions.registry(stream.network_id_mode()),
        stream.current_dimension(),
        pick,
    )
    .with_cell_overlays(&overlays);
    let (position, face, block) = aim.support;
    if block == stream.air_block_id() {
        let hit = super::target::air_hit(
            &world,
            position,
            face,
            block,
            Vec3::from_array(observed.ray.origin),
            state.position,
            observed.reach,
        )?;
        observed.ray.direction = (Vec3::new(
            hit.hit_world.x as f32,
            hit.hit_world.y as f32,
            hit.hit_world.z as f32,
        ) - Vec3::from_array(observed.ray.origin))
        .try_normalize()?
        .to_array();
        apply_support_hit(&mut observed, hit);
        return Some(observed);
    }
    if let Some(height) = aim.hidden_height {
        let (support, face, runtime_id) = aim.support;
        let ghost = placement_cell(support, face);
        if !world.is_air(ghost).ok()? {
            return None;
        }
        let hit = hidden_support_hit_at(&world, ghost, face, height)?;
        if hit.runtime_id != runtime_id {
            return None;
        }
        apply_support_hit(&mut observed, hit);
        return gameplay::interaction_authority::pick_point_in_range(
            observed.target.hit_world,
            state.position,
            observed.reach,
        )
        .then_some(observed);
    }
    let Some(direction) = aim.support_ray else {
        let target = &observed.target;
        return ((target.position, target.face, target.runtime_id) == aim.support)
            .then_some(observed);
    };
    let direction = direction.to_array();
    let actor = gameplay::melee::pick_actor(
        authority.remote_actors(),
        authority.local_mount_unique_id(),
        observed.ray.origin,
        direction,
        observed.reach,
    );
    let hit = support_hit(
        &world,
        Vec3::from_array(observed.ray.origin),
        Vec3::from_array(direction),
        observed.reach,
        actor,
    )?;
    if (hit.block_pos, hit.face, hit.runtime_id) != aim.support || hit.overlay.is_some() {
        return None;
    }
    let hit_world = [
        hit.hit_world.x as f32,
        hit.hit_world.y as f32,
        hit.hit_world.z as f32,
    ];
    if !gameplay::interaction_authority::pick_point_in_range(
        hit_world,
        state.position,
        observed.reach,
    ) {
        return None;
    }
    observed.ray.direction = direction;
    apply_support_hit(&mut observed, hit);
    Some(observed)
}

/// Freezes the support geometry and its world identity for packet admission.
fn apply_support_hit(observed: &mut FrozenBlockObservation, hit: sim::BlockHit) {
    observed.ray.world_identity = hit.identity.clone();
    observed.target = gameplay::mining::FrozenMiningTarget {
        position: hit.block_pos,
        face: hit.face,
        relative_hit: [
            hit.hit_local.x as f32,
            hit.hit_local.y as f32,
            hit.hit_local.z as f32,
        ],
        hit_world: [
            hit.hit_world.x as f32,
            hit.hit_world.y as f32,
            hit.hit_world.z as f32,
        ],
        runtime_id: hit.runtime_id,
        identity: hit.identity,
    };
}

/// Uses the native downward support pick only after both direct picks miss.
pub(super) fn support_hit(
    world: &PaletteWorld<'_>,
    eye: Vec3,
    direction: Vec3,
    reach: f64,
    actor: Option<gameplay::melee::ActorHit>,
) -> Option<sim::BlockHit> {
    let widen = |v: Vec3| sim::Vec3::new(f64::from(v.x), f64::from(v.y), f64::from(v.z));
    let (origin, ray) = (widen(eye), widen(direction));
    let direct = world
        .block_interaction_ray_current(origin, ray, reach)
        .ok()?;
    use gameplay::melee::Crosshair;
    match gameplay::melee::classify_block_hit(actor, eye.to_array(), direct.as_ref(), reach) {
        Crosshair::Block => direct,
        Crosshair::Miss if direct.is_none() && actor.is_none() => {
            world.block_use_miss_support_current(origin, ray).ok()?
        }
        _ => None,
    }
}

/// Samples the real support's selection shape from the adjacent empty destination.
/// Height-sensitive blocks must be clicked on a support face producing their intended half.
pub(super) fn planned_placement_half(offer: &CreativePlacementOffer) -> Option<bool> {
    placement_properties(offer).as_ref().and_then(planned_upper)
}

/// Pillars derive their axis from the clicked face, independently of reported yaw.
pub(super) fn support_produces_axis(offer: &CreativePlacementOffer, face: u8) -> bool {
    if let Some(wanted) = wanted_face(offer) {
        return face == wanted;
    }
    let Ok(states) = serde_json::from_str::<serde_json::Value>(&offer.states) else {
        return false;
    };
    // A hanging sign hangs under a block or from a side; a floor sign, banner or coral fan
    // stands on top, and a wall coral fan needs a side.
    if offer.name.ends_with("hanging_sign") {
        return if states["hanging"]["value"].as_u64() == Some(1) {
            face == 0
        } else {
            (2..=5).contains(&face)
        };
    }
    if !states["ground_sign_direction"].is_null() || !states["coral_fan_direction"].is_null() {
        return face == 1;
    }
    if !states["coral_direction"].is_null() {
        return (2..=5).contains(&face);
    }
    match states["pillar_axis"]["value"].as_str() {
        Some("x") => matches!(face, 4 | 5),
        Some("z") => matches!(face, 2 | 3),
        Some("y") => matches!(face, 0 | 1),
        _ => true,
    }
}

pub(super) fn support_produces_half(upper: Option<bool>, face: u8, y: f32) -> bool {
    upper.is_none_or(|want_upper| match face {
        0 => want_upper,
        1 => !want_upper,
        _ => (y - 0.5).abs() > 0.01 && (y > 0.5) == want_upper,
    })
}

pub(super) fn hidden_support_hit(
    world: &PaletteWorld<'_>,
    ghost: [i32; 3],
    face: u8,
    upper: Option<bool>,
) -> Option<sim::BlockHit> {
    [0.5, 0.25, 0.75]
        .into_iter()
        .filter_map(|height| hidden_support_hit_at(world, ghost, face, height))
        .find(|hit| support_produces_half(upper, hit.face, hit.hit_local.y as f32))
}

fn hidden_support_hit_at(
    world: &PaletteWorld<'_>,
    ghost: [i32; 3],
    face: u8,
    height: f64,
) -> Option<sim::BlockHit> {
    let support = placement_cell(ghost, face ^ 1);
    let origin = sim::Vec3::new(
        ghost[0] as f64 + 0.5,
        ghost[1] as f64 + height,
        ghost[2] as f64 + 0.5,
    );
    let direction = sim::Vec3::new(
        f64::from(support[0] - ghost[0]),
        f64::from(support[1] - ghost[1]),
        f64::from(support[2] - ghost[2]),
    );
    world
        .block_interaction_ray_current(origin, direction, 1.5)
        .ok()?
        .filter(|hit| hit.block_pos == support && hit.face == face && hit.overlay.is_none())
}

/// Samples visible faces and the ordinary Bedrock forward-bridge pick.
pub(super) fn support_directions(
    support: [i32; 3],
    face: u8,
    eye: Vec3,
) -> impl Iterator<Item = Vec3> {
    let offset = placement_cell([0; 3], face);
    let indirect =
        (face >= 2).then(|| Vec3::new(offset[0] as f32, -2.0, offset[2] as f32).normalize());
    face_samples(support, face, eye)
        .filter_map(move |point| (point - eye).try_normalize())
        .chain(indirect)
}

/// Samples both halves and the center of the face, omitting faces the eye is behind.
fn face_samples(clicked: [i32; 3], face: u8, eye: Vec3) -> impl Iterator<Item = Vec3> {
    const SAMPLES: [f32; 5] = [0.5, 0.25, 0.75, 0.0625, 0.9375];
    let axis = [1, 2, 0][usize::from(face / 2)];
    let u = (axis + 1) % 3;
    let v = (axis + 2) % 3;
    let side = f32::from(face & 1);
    let mut origin = clicked.map(|n| n as f32);
    origin[axis] += side;
    let visible = if side == 0.0 {
        eye[axis] < origin[axis]
    } else {
        eye[axis] > origin[axis]
    };
    SAMPLES
        .into_iter()
        .filter(move |_| visible)
        .flat_map(move |a| {
            SAMPLES.into_iter().map(move |b| {
                let mut point = origin;
                point[u] += a;
                point[v] += b;
                Vec3::from_array(point)
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(name: &str, states: serde_json::Value) -> CreativePlacementOffer {
        CreativePlacementOffer {
            position: [0; 3],
            name: name.into(),
            states: states.to_string(),
            correct_aim: true,
            strict_state: true,
            repeat_while_held: true,
        }
    }

    #[test]
    fn unpredicted_blocks_get_the_click_face_or_look_quadrant_their_planned_state_needs() {
        let rod = offer(
            "minecraft:end_rod",
            serde_json::json!({
                "minecraft:facing_direction": {"type":"int","value":4}
            }),
        );
        assert_eq!(wanted_face(&rod), Some(4));
        assert!(support_produces_axis(&rod, 4));
        assert!(!support_produces_axis(&rod, 1));
        let dropper = offer(
            "minecraft:dropper",
            serde_json::json!({
                "facing_direction": {"type":"int","value":3}
            }),
        );
        assert_eq!(wanted_quadrant(&dropper), Some(2));
        let observer = offer(
            "minecraft:observer",
            serde_json::json!({
                "minecraft:facing_direction": {"type":"string","value":"north"}
            }),
        );
        assert_eq!(wanted_quadrant(&observer), Some(2));
        let up = offer(
            "minecraft:dropper",
            serde_json::json!({
                "facing_direction": {"type":"int","value":1}
            }),
        );
        assert_eq!(wanted_quadrant(&up), None);
    }

    #[test]
    fn facing_families_turn_to_the_quadrant_their_own_rule_needs() {
        let cardinal = |name: &str, direction: &str| {
            offer(
                name,
                serde_json::json!({
                    "minecraft:cardinal_direction": {"type":"string","value":direction}
                }),
            )
        };
        // Blocks that face the player: north is the look quadrant 0, then east, south, west.
        for name in [
            "minecraft:furnace",
            "minecraft:smoker",
            "minecraft:blast_furnace",
            "minecraft:chest",
            "minecraft:carved_pumpkin",
            "minecraft:stonecutter_block",
            "minecraft:lectern",
            "minecraft:campfire",
        ] {
            for (quadrant, direction) in ["north", "east", "south", "west"].iter().enumerate() {
                assert_eq!(
                    wanted_quadrant(&cardinal(name, direction)),
                    Some(quadrant),
                    "{name} {direction}"
                );
            }
        }
        // Anvils face a quarter turn left of the player, as `anvil_states` places them.
        for name in ["minecraft:anvil", "minecraft:chipped_anvil", "minecraft:damaged_anvil"] {
            for (quadrant, direction) in ["east", "south", "west", "north"].iter().enumerate() {
                assert_eq!(
                    wanted_quadrant(&cardinal(name, direction)),
                    Some(quadrant),
                    "{name} {direction}"
                );
            }
        }
        // Barrels and command blocks face the player like droppers do.
        for name in [
            "minecraft:barrel",
            "minecraft:command_block",
            "minecraft:repeating_command_block",
            "minecraft:chain_command_block",
        ] {
            for (face, quadrant) in [(2, 0), (5, 1), (3, 2), (4, 3)] {
                let planned = offer(
                    name,
                    serde_json::json!({"facing_direction": {"type":"int","value":face}}),
                );
                assert_eq!(wanted_quadrant(&planned), Some(quadrant), "{name} {face}");
            }
            let up = offer(
                name,
                serde_json::json!({"facing_direction": {"type":"int","value":1}}),
            );
            assert_eq!(wanted_quadrant(&up), None, "{name} up");
        }
    }

    #[test]
    fn stripped_log_axis_requires_a_matching_support_face() {
        for (axis, faces) in [("x", [4, 5]), ("y", [0, 1]), ("z", [2, 3])] {
            let planned = offer(
                "minecraft:stripped_spruce_log",
                serde_json::json!({"pillar_axis":{"type":"string","value":axis}}),
            );
            for face in 0..6 {
                assert_eq!(support_produces_axis(&planned, face), faces.contains(&face));
            }
        }
    }

    #[test]
    fn trapdoor_reconciliation_only_toggles_the_open_bit() {
        let planned = offer(
            "minecraft:spruce_trapdoor",
            serde_json::json!({
                "open_bit":{"type":"byte","value":1}, "direction":{"type":"int","value":2},
                "upside_down_bit":{"type":"byte","value":0}
            }),
        );
        let mut current: serde_json::Value = serde_json::from_str(&planned.states).unwrap();
        current["open_bit"]["value"] = serde_json::json!(0);
        assert!(needs_toggle(
            &planned,
            Some(&planned.name),
            Some(&current.to_string())
        ));
        assert!(!needs_toggle(
            &planned,
            Some(&planned.name),
            Some(&planned.states)
        ));
        current["direction"]["value"] = serde_json::json!(1);
        assert!(!needs_toggle(
            &planned,
            Some(&planned.name),
            Some(&current.to_string())
        ));
        assert!(!needs_toggle(
            &planned,
            Some("minecraft:stone"),
            Some(&current.to_string())
        ));
    }

    #[test]
    fn bridge_can_sample_a_side_hidden_by_the_support_underfoot() {
        let support = [4, 7, 9];
        let eye = Vec3::new(4.5, 9.0, 9.5);
        assert_eq!(face_samples(support, 5, eye).count(), 0);
        let candidates: Vec<_> = support_directions(support, 5, eye).collect();
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].x > 0.0 && candidates[0].y < -0.7);
    }

    #[test]
    fn stair_orientation_matches_even_when_bedrock_adds_a_corner_property() {
        let planned = serde_json::json!({
            "weirdo_direction": {"type":"int","value":2},
            "upside_down_bit": {"type":"byte","value":0},
        });
        let planned = planned.as_object().unwrap();
        let mut predicted = planned.clone();
        predicted.insert(
            "minecraft:corner".into(),
            serde_json::json!({"type":"string","value":"none"}),
        );
        assert!(matches_planned_state(
            planned,
            &serde_json::to_string(&predicted).unwrap()
        ));
        predicted["weirdo_direction"]["value"] = serde_json::json!(1);
        assert!(!matches_planned_state(
            planned,
            &serde_json::to_string(&predicted).unwrap()
        ));
    }

    #[test]
    fn sample_points_cover_both_halves_of_a_visible_side() {
        let face: Vec<_> = face_samples([4, 7, 9], 5, Vec3::new(7.0, 8.0, 9.5)).collect();
        assert!(face.iter().all(|p| p.x == 5.0));
        assert!(face.iter().any(|p| p.y < 7.5));
        assert!(face.iter().any(|p| p.y > 7.5));
        assert_eq!(
            face_samples([4, 7, 9], 4, Vec3::new(7.0, 8.0, 9.5)).count(),
            0
        );
    }
}
#[test]
fn reported_facing_can_be_opposite_the_support_click() {
    let rotation = Quat::from_euler(EulerRot::YXZ, 0.0, -0.4, 0.0);
    let [pitch, yaw] = wire_facing(rotation);
    let facing = separate_facing(rotation, |candidate| {
        let candidate_yaw = wire_facing(candidate)[1];
        ((candidate_yaw - yaw).abs() - 180.0).abs() < 0.01
    })
    .expect("opposite facing should be available for the same support click");
    assert!((wire_facing(facing)[0] - pitch).abs() < 0.01);
    assert!((rotation * Vec3::NEG_Z).z < 0.0);
    assert!((facing * Vec3::NEG_Z).z > 0.0);
}
