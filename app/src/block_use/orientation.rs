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
    pub support: Support,
}

/// The same look representation as the normal movement and placement systems.
pub(super) fn wire_facing(rotation: Quat) -> [f32; 2] {
    let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
    [
        -pitch.to_degrees(),
        gameplay::movement::wire_yaw(180.0 - yaw.to_degrees()),
    ]
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
        || context.input.movement() != [0.0; 2]
        || state.displacement[0].abs() > 0.01
        || state.displacement[2].abs() > 0.01
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
    let held = held_block_store_id(stream, observed.selection.item.block_runtime_id())?;
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
    if !world.is_air(expected.position).ok()? {
        return None;
    }
    let planned =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&expected.states)
            .ok()?;
    let matches = |candidate: &FrozenBlockObservation, view| {
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
        predicted_placement(
            &context.collisions,
            stream,
            &world,
            held,
            candidate,
            &surroundings,
            view,
        )
        .is_some_and(|placement| {
            placement.position == expected.position
                && context.collisions.block_identifier(mode, placement.block)
                    == Some(expected.name.as_str())
                && context
                    .collisions
                    .block_canonical_state(mode, placement.block)
                    .is_some_and(|actual| matches_planned_state(&planned, actual))
        })
    };
    if !bridge && matches(observed, *context.view) {
        return None;
    }
    let eye = Vec3::from_array(observed.ray.origin);
    let forward = Vec3::from_array(observed.ray.direction).try_normalize()?;
    let mut best = None;
    let mut best_dot = f32::NEG_INFINITY;
    let mut separate = None;
    let mut separate_dot = f32::NEG_INFINITY;
    for face in 0..6 {
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
    best.or(separate).or_else(|| {
        let mut view = *context.view;
        let rotation = if matches(observed, view) {
            view.rotation()
        } else {
            separate_facing(view.rotation(), |rotation| {
                view.set_rotation(rotation);
                matches(observed, view)
            })?
        };
        Some(PlacementAim {
            rotation,
            support_ray: None,
            support: (
                observed.target.position,
                observed.target.face,
                observed.target.runtime_id,
            ),
        })
    })
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
    observed.ray.world_identity = hit.identity.clone();
    observed.target = gameplay::mining::FrozenMiningTarget {
        position: hit.block_pos,
        face: hit.face,
        relative_hit: [
            hit.hit_local.x as f32,
            hit.hit_local.y as f32,
            hit.hit_local.z as f32,
        ],
        hit_world,
        runtime_id: hit.runtime_id,
        identity: hit.identity,
    };
    Some(observed)
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
pub(super) fn hidden_support_hit(
    world: &PaletteWorld<'_>,
    ghost: [i32; 3],
    face: u8,
) -> Option<sim::BlockHit> {
    let support = placement_cell(ghost, face ^ 1);
    let origin = sim::Vec3::new(
        ghost[0] as f64 + 0.5,
        ghost[1] as f64 + 0.5,
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
