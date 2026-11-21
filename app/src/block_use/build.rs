//! Immediate build preparation shared by ordered presses and held placement.

use super::*;
use gameplay::{
    mining::FrozenMiningSelection,
    movement::{MiningEffects, UnsentSampleView},
};

/// Current target and movement facts for one immediate build call.
pub(crate) struct BuildInput {
    observed: FrozenBlockObservation,
    pub state: UnsentSampleView,
    pub trigger: ItemUseTrigger,
    pub due: std::time::Duration,
    pub clock: RepeatClock,
    view: LocalViewPose,
    pub(super) expected: Option<mod_host::CreativePlacementOffer>,
    pub(super) interaction: Option<mod_host::CreativePlacementOffer>,
    pub(super) air_place: bool,
}

/// A proposal freezes selection, placement and inventory effects before its first send.
struct BuildPlan {
    server_selection: FrozenMiningSelection,
    inventory_revision: u64,
    local_use: LocalUse,
    /// The clicked block's own use owns the press, so the held item's use skips it.
    takes_press: bool,
    destination: [i32; 3],
    predicted: Option<Vec<([i32; 3], u32, u32)>>,
    local_runtime_id: u64,
    swing_effects: MiningEffects,
    block_network_id: u32,
    change: Option<protocol::PredictedSlotChange>,
    cooldown: Option<client_world::ResolvedUseCooldown>,
    repeats_while_held: bool,
    /// The door or bed a non-block item places, when it is not a block item.
    paired: Option<u32>,
}

impl BuildInput {
    /// Groups the current press facts for shared prediction and transport admission.
    pub(super) fn new(
        observed: FrozenBlockObservation,
        state: UnsentSampleView,
        trigger: ItemUseTrigger,
        due: std::time::Duration,
        clock: RepeatClock,
        view: LocalViewPose,
    ) -> Self {
        Self {
            observed,
            state,
            trigger,
            due,
            clock,
            view,
            expected: None,
            interaction: None,
            air_place: false,
        }
    }
}

/// Resolves item-form stacks whose Bedrock placement lacks an advertised block runtime ID.
pub(super) fn held_placement_block(
    collisions: &PhysicsCollisionRegistries,
    stream: &chunk_pipeline::WorldStream,
    block_id: i32,
    item_identifier: impl FnOnce() -> Option<std::sync::Arc<str>>,
) -> Option<u32> {
    held_block_store_id(stream, block_id).or_else(|| {
        let identifier = item_identifier()?;
        let mode = stream.network_id_mode();
        if let Some(block) = gameplay::block_use::paired_item_block(collisions, mode, &identifier) {
            return Some(block);
        }
        if gameplay::placement_plants::supported(&identifier) {
            return collisions.block_default_runtime_id(mode, &identifier);
        }
        let name = match identifier.as_ref() {
            "minecraft:cauldron" => "minecraft:cauldron",
            "minecraft:string" => "minecraft:trip_wire",
            "minecraft:redstone" => "minecraft:redstone_wire",
            "minecraft:repeater" => "minecraft:unpowered_repeater",
            "minecraft:comparator" => "minecraft:unpowered_comparator",
            _ => return None,
        };
        collisions.block_default_runtime_id(stream.network_id_mode(), name)
    })
}

/// [`held_placement_block`], plus the blocks of items Fast Place builds with that carry no
/// block, such as signs, banners and heads.
pub(super) fn fast_place_block(
    collisions: &PhysicsCollisionRegistries,
    stream: &chunk_pipeline::WorldStream,
    block_id: i32,
    item_identifier: impl Fn() -> Option<std::sync::Arc<str>>,
) -> Option<u32> {
    held_placement_block(collisions, stream, block_id, &item_identifier).or_else(|| {
        gameplay::placement_items::routed_item_block(
            collisions,
            stream.network_id_mode(),
            &item_identifier()?,
        )
    })
}

/// Whether clicking `face` with the item placing `placed` builds `expected` when no prediction
/// exists. A wall banner or coral fan comes from its floor item clicked on a side face; the floor
/// form of a block that also has a wall form needs the top face.
fn unpredicted_face_places(
    collisions: &PhysicsCollisionRegistries,
    mode: assets::NetworkIdMode,
    placed: &str,
    expected: &mod_host::CreativePlacementOffer,
    face: u8,
) -> bool {
    use gameplay::placement_items::{standing_form, wall_form};
    if placed == expected.name {
        return wall_form(collisions, mode, placed).is_none() || face == 1;
    }
    (2..=5).contains(&face)
        && standing_form(collisions, mode, &expected.name).as_deref() == Some(placed)
}

/// True the first time a refusal repeats the same cause, so a held press logs each distinct
/// refusal once instead of every tick.
fn refusal_once(key: String) -> bool {
    static LAST: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
    let Ok(mut last) = LAST.lock() else {
        return true;
    };
    if *last == key {
        return false;
    }
    *last = key;
    true
}

/// Computes a proposal without sending packets, committing inventory or changing the world.
fn prepare(
    player_runtime: &client_state::PlayerRuntime,
    context: &BlockUseContext,
    runtime: &mut BlockUseRuntime,
    item_use: &mut client_interaction::item_use::ItemUseRuntime,
    movement: &MovementTicker,
    attempt: &mut BuildInput,
) -> Option<sim::CollisionQuery<BuildPlan>> {
    let mut observed = attempt.observed.clone();
    let state = &attempt.state;
    let tick = state.tick.saturating_add(1);
    let game_mode = player_runtime.facts.player_game_mode();
    let caps = player_runtime.facts.game_mode_capabilities()?;
    let stream = context.client_world.stream.as_ref()?;
    let server_selection = observed.selection.clone();
    let inventory_revision = context
        .ui
        .inventory_ledger(player_runtime)
        .authoritative_slot_revision(server_selection.slot)
        .unwrap_or(0);
    observed.selection = runtime
        .inventory
        .selection(&server_selection, inventory_revision);
    let caps = caps.for_edit(stream.authority().held_edit_item(
        &observed.selection.item,
        protocol::ItemBlockList::CanPlaceOn,
        observed.target.runtime_id,
    ));
    item_use.synchronize(context.ui.session_id());
    let cooldown = stream
        .authority()
        .item_use_profile(observed.selection.item.network_id())
        .cooldown;
    let ready = cooldown.is_none_or(|cooldown| item_use.cooldown_ready(cooldown, tick));
    let identifier = stream
        .authority()
        .item_identifier(observed.selection.item.network_id());
    let bucket = identifier
        .as_deref()
        .is_some_and(assets::facts::has_bucket_name);
    let brush = identifier.as_deref() == Some(assets::facts::BRUSH_IDENTIFIER);
    let traits = context
        .ui
        .inventory_ledger(player_runtime)
        .item_facts()
        .get(observed.selection.item.network_id())
        .traits;
    let prepared = PaletteWorld::new(
        stream.collision_store(),
        context.collisions.registry(stream.network_id_mode()),
        stream.current_dimension(),
        Default::default(),
    )
    .observe(|world| {
        let surroundings =
            use_surroundings(context, world, &observed, state.position, state.sneaking);
        let mut local_use = LocalUse::resolve(
            &observed.selection.item,
            observed.target.position,
            observed.target.face,
            &surroundings,
            &caps,
        );
        if !ready && local_use != LocalUse::Interact {
            local_use = LocalUse::Nothing;
        }
        if let Some(expected) = &attempt.interaction {
            let mode = stream.network_id_mode();
            let block = world.primary_runtime_id(observed.target.position).ok()?;
            if local_use != LocalUse::Interact || observed.target.position != expected.position
                || !super::orientation::needs_toggle(expected,
                    context.collisions.block_identifier(mode, block),
                    context.collisions.block_canonical_state(mode, block)) {
                return None;
            }
        }
        if let Some(expected) = attempt.expected.as_ref()
            .filter(|expected| gameplay::placement_plants::supported(&expected.name)) {
            let mode = stream.network_id_mode();
            let current = world.primary_runtime_id(expected.position).ok()?;
            if context.collisions.block_identifier(mode, current) == Some(expected.name.as_str())
                && context.collisions.block_canonical_state(mode, current)
                    .is_some_and(|state| super::orientation::strict_state_matches(expected, state)) {
                // Inventory confirmation may outlive a server update for this cell.
                return None;
            }
        }
        // Door and bed items are not block items; native `DoorItem` and `BedItem` place through
        // the item's own use and report the same success, which starts the repeat delay.
        let paired = paired_item_block(&context.collisions, stream, &observed.selection.item);
        let placed_block = paired.or_else(|| {
            let block_id = observed.selection.item.block_runtime_id();
            if attempt.expected.is_some() {
                fast_place_block(&context.collisions, stream, block_id, || identifier.clone())
            } else {
                held_placement_block(&context.collisions, stream, block_id, || identifier.clone())
            }
        });
        let placement = (ready && caps.can_build && local_use != LocalUse::Interact)
            .then_some(placed_block)
            .flatten()
            .and_then(|block| {
                predicted_placement(
                    &context.collisions,
                    stream,
                    world,
                    block,
                    &observed,
                    &surroundings,
                    attempt.view,
                )
            });
        let paired_fits = paired.is_some_and(|block| {
            ready && caps.can_build && local_use != LocalUse::Interact
                && observed.selection.item.count() > 0
                && paired_item_fits(&context.collisions, stream, world, block, &observed, &surroundings, attempt.view)
        });
        if let Some(expected) = &attempt.expected {
            let mode = stream.network_id_mode();
            // Blocks with no placement rule (containers, rails, pots, shelves, lamps and the
            // like) cannot be predicted. Send the click at the ghost cell and let the server
            // decide the state, as beds do, rather than refusing to place them at all. A click
            // into air (V2 Strict) is the same: the server places the block in the clicked
            // cell itself, so that cell is the destination.
            let unpredicted = placement.is_none()
                && paired.is_none()
                && ready
                && caps.can_build
                && local_use != LocalUse::Interact
                && observed.selection.item.count() > 0
                && placed_block.is_some_and(|block| {
                    // A block whose survival rule is unresolved (mangrove roots, mushrooms and
                    // the like) has a known placement but no support rule to predict it with,
                    // so prediction refuses it. The server decides these as well.
                    let fact = context.collisions.block_facts(mode, block);
                    (fact.placement == assets::facts::PlacementRule::Unresolved
                        || fact.survival == assets::registry::SurvivalRule::Unresolved)
                        && context.collisions.block_identifier(mode, block).is_some_and(|name| {
                            unpredicted_face_places(&context.collisions, mode, name, expected,
                                observed.target.face)
                        })
                })
                && world.is_air(expected.position).unwrap_or(false)
                && (if attempt.air_place {
                    observed.target.position == expected.position
                } else {
                    surroundings.destination(observed.target.position, observed.target.face).0
                        == expected.position
                })
                // A block pointing out of its support needs that face clicked, and a block with
                // a planned horizontal facing waits for the view to give it.
                && (!(expected.correct_aim || expected.strict_state)
                    || super::orientation::wanted_face(expected)
                        .is_none_or(|face| observed.target.face == face))
                && (!(expected.correct_aim || expected.strict_state)
                    || super::orientation::wanted_quadrant(expected).is_none_or(|want| {
                        super::orientation::look_quadrant(attempt.view.rotation()) == want
                    }));
            let supported_door = placement.is_none()
                && expected.name.ends_with("_door")
                && (!expected.strict_state || super::orientation::door_matches_facing(expected, attempt.view.rotation()))
                && paired_fits
                && paired.and_then(|block| context.collisions.block_identifier(mode, block)) == Some(expected.name.as_str())
                && surroundings.destination(observed.target.position, observed.target.face).0 == expected.position;
            if supported_door || unpredicted {
                local_use = LocalUse::Place;
            } else {
            let Some(placement) = placement.as_ref() else {
                if refusal_once(format!("p|{}|{:?}|{:?}|{}", expected.name, expected.position,
                    observed.target.position, observed.target.face)) {
                    bevy::log::info!(block = %expected.name, states = %expected.states,
                        ghost = ?expected.position, support = ?observed.target.position,
                        face = observed.target.face, click = ?observed.target.relative_hit,
                        "Fast Place could not predict this placement");
                }
                return None;
            };
            let actual_name = context.collisions.block_identifier(mode, placement.block);
            let actual_states = context.collisions.block_canonical_state(mode, placement.block);
            if !matches!(game_mode, Some(PlayerGameMode::Creative | PlayerGameMode::Survival))
                || observed.selection.item.count() == 0
                || (!world.is_air(expected.position).ok()?
                    && !(gameplay::placement_plants::supported(&expected.name)
                        && gameplay::placement_plants::replaceable(context.collisions.block_facts(mode,
                            world.primary_runtime_id(expected.position).ok()?))))
                || placement.position != expected.position
                || actual_name != Some(expected.name.as_str())
                || ((expected.strict_state || expected.correct_aim)
                    && !actual_states.is_some_and(|actual| {
                        super::orientation::strict_state_matches(expected, actual)
                    }))
            {
                let (yaw, pitch, _) = attempt.view.rotation().to_euler(bevy::math::EulerRot::YXZ);
                if refusal_once(format!("d|{}|{:?}|{:?}|{}|{:?}", expected.name, expected.position,
                    observed.target.position, observed.target.face, actual_states)) {
                    bevy::log::info!(block = %expected.name, states = %expected.states, ghost = ?expected.position,
                        actual_name, actual_states, destination = ?placement.position,
                        additional_cells = placement.additional.len(), face = observed.target.face,
                        click = ?observed.target.relative_hit,
                        yaw = gameplay::movement::wire_yaw(180.0 - yaw.to_degrees()), pitch = -pitch.to_degrees(),
                        "Fast Place predicted a different block or state");
                }
                return None;
            }
            if expected.states.contains("weirdo_direction") {
                bevy::log::debug!(ghost = ?expected.position, planned = %expected.states,
                    predicted = ?actual_states, face = observed.target.face,
                    click_y = observed.target.relative_hit[1],
                    "Fast Place stair placement prediction");
            }
            }
        }
        if placement.is_some() && observed.selection.item.count() > 0 {
            local_use = LocalUse::Place;
        }
        // Without a verified state, such as any bed, the server's block update draws the cells.
        if paired_fits && local_use == LocalUse::Nothing {
            local_use = LocalUse::Place;
        }
        // A predicted placement can become the next ray hit before the release edge reaches
        // this thread. Do not turn that same press into a container interaction with the block
        // we just placed; adjacent held placements remain eligible for bridging.
        if attempt.trigger == ItemUseTrigger::SimulationTick
            && attempt.interaction.is_none()
            && local_use == LocalUse::Interact
            && runtime
                .intention
                .suppress_post_place_interaction(observed.target.position)
        {
            local_use = LocalUse::Nothing;
        }
        // Seed items plant through their own use rather than as block items.
        let planted = (ready
            && caps.can_build
            && local_use == LocalUse::Nothing
            && placement.is_none()
            && observed.selection.item.block_runtime_id() == 0
            && observed.selection.item.count() > 0)
            .then(|| {
                predicted_seed_planting(&context.collisions, stream, world, &context.ui, &observed)
            })
            .flatten();
        if planted.is_some() {
            local_use = LocalUse::Place;
        }
        if ready {
            // An unreadable cell counts as air, so the brush predicts nothing there.
            let clicked_air = world.is_air(observed.target.position).unwrap_or(true);
            local_use = local_use.or_brush(brush, clicked_air, &observed.selection.item, &caps);
        }
        let destination = planted.map(|(cell, _)| cell).unwrap_or_else(|| {
            placement.as_ref().map_or_else(
                || {
                    surroundings
                        .destination(observed.target.position, observed.target.face)
                        .0
                },
                |placement| placement.position,
            )
        });
        if attempt.expected.is_none()
            && context.creative.as_ref().is_some_and(|policy| policy.locked)
            && local_use != LocalUse::Interact
            && (placed_block.is_some() || local_use == LocalUse::Place)
        {
            // Validate the actual destination and material without requiring a
            // particular neighbor-derived state for ordinary placement.
            let block = placement.as_ref().map(|placed| placed.block)
                .or_else(|| planted.map(|(_, block)| block))
                .or_else(|| (local_use == LocalUse::Place).then_some(placed_block).flatten());
            let mode = stream.network_id_mode();
            let admitted = context.creative.as_ref().and_then(|policy| policy.offer.as_ref());
            if !block.zip(admitted).is_some_and(|(block, offer)| {
                destination == offer.position
                    && context.collisions.block_identifier(mode, block) == Some(offer.name.as_str())
            }) {
                return None;
            }
        }
        let predicted = placement
            .filter(|_| local_use == LocalUse::Place)
            .map(|placement| {
                std::iter::once((placement.position, 0, placement.block))
                    .chain(
                        placement
                            .additional
                            .into_iter()
                            .map(|(position, block)| (position, 0, block)),
                    )
                    .collect::<Vec<_>>()
            })
            .or_else(|| planted.map(|(cell, crop)| vec![(cell, 0, crop)]));
        let predicted = predicted.or_else(|| {
            (local_use == LocalUse::Interact)
                .then(|| {
                    predicted_toggle(
                        &context.collisions,
                        stream,
                        world,
                        observed.target.position,
                        observed.target.runtime_id,
                    )
                })
                .flatten()
                .map(|(position, block)| vec![(position, 0, block)])
        });
        for (position, _, _) in predicted.iter().flatten() {
            world.primary_runtime_id(*position).ok()?;
        }
        let repeats_while_held = gameplay::block_use::repeats_while_held(
            &observed.selection.item,
            bucket,
            local_use,
            surroundings.clicked,
            traits,
        );
        Some((
            local_use,
            gameplay::block_use::block_takes_use_press(surroundings.clicked, state.sneaking),
            destination,
            predicted,
            repeats_while_held,
            paired,
        ))
    })
    .ok()?;
    let (local_use, takes_press, destination, predicted, repeats_while_held, paired) =
        prepared.value?;
    let identity = observed.ray.world_identity.merge(&prepared.identity).ok()?;
    let local_runtime_id = stream.local_player_runtime_id();
    // The tick has not simulated yet, so its swing reads the effects in force before it.
    let swing_effects = context
        .effects
        .mining_tick(tick, movement.completed_tick())
        .0;
    let block_network_id = stream.block_network_id(observed.target.runtime_id)?;
    let legacy_request_id = protocol::next_legacy_request_id()?;
    // For an air click the server owns both the block and the Survival inventory update.
    let change = (!attempt.air_place
        && local_use == LocalUse::Place
        && game_mode != Some(PlayerGameMode::Creative))
    .then(|| {
        runtime
            .inventory
            .prepare_change(&observed.selection, legacy_request_id)
    });
    attempt.observed = observed;
    Some(sim::CollisionQuery {
        identity,
        value: BuildPlan {
            server_selection,
            inventory_revision,
            local_use,
            takes_press,
            destination,
            predicted,
            local_runtime_id,
            swing_effects,
            block_network_id,
            change,
            cooldown,
            repeats_while_held,
            paired,
        },
    })
}

/// Prepares one build and commits its effects only after successful transport admission.
pub(crate) fn advance(
    player_runtime: &mut client_state::PlayerRuntime,
    context: &mut BlockUseContext,
    runtime: &mut BlockUseRuntime,
    item_use: &mut client_interaction::item_use::ItemUseRuntime,
    swings: &mut SwingTracker,
    movement: &mut MovementTicker,
    attempt: &mut BuildInput,
) -> bool {
    let Some(plan) = prepare(
        player_runtime,
        context,
        runtime,
        item_use,
        movement,
        attempt,
    ) else {
        return false;
    };
    if !crate::interaction_authority::world_identity_current(
        &context.client_world,
        &context.collisions,
        &plan.identity,
    ) {
        return false;
    }
    let plan = &plan.value;
    let observed = &attempt.observed;
    let state = &attempt.state;
    let trigger = attempt.trigger;
    let due = attempt.due;
    let clock = attempt.clock;
    let tick = state.tick.saturating_add(1);
    let Some(stream) = context.client_world.stream.as_ref() else {
        return false;
    };
    if trigger == ItemUseTrigger::SimulationTick && !plan.repeats_while_held {
        runtime.record(trigger, due, tick, LocalUse::Nothing, false, clock);
        return false;
    }
    // Ordered presses can precede block maintenance. Refresh the authority and selected-item
    // history before admission so stale publication cannot reject the new placement.
    swings.sync_ticks_for_item(
        movement.interaction_authority_identity(),
        movement.completed_tick(),
        &context.effects,
        crate::melee::selected_attack_timing(player_runtime, &context.client_world)
            .and_then(|timing| timing.swing_duration_ticks),
    );
    let target = gameplay::block_use::PlacementTarget {
        position: observed.target.position,
        face: observed.target.face,
    };
    if !runtime.may_attempt(tick, plan.local_use, target, swings) {
        return false;
    }
    let start_destination = (!attempt.air_place && runtime.last_success_destination().is_none())
        .then_some(plan.destination);
    let mut before_swing = swings.clone();
    let packets = use_packets(
        (observed, plan.block_network_id),
        state.position,
        trigger,
        plan.local_use,
        start_destination,
        plan.change.clone(),
        plan.local_runtime_id,
        |tick| swings.try_swing_before_tick(tick, plan.swing_effects),
        tick,
    );
    if let Some(expected) = &attempt.expected {
        for (index, packet) in packets.iter().enumerate() {
            bevy::log::debug!(index, packet = ?packet.kind(), tick, "Fast Place: outbound batch order");
        }
        let (yaw, pitch, _) = attempt.view.rotation().to_euler(bevy::math::EulerRot::YXZ);
        let placed_runtime_id = plan
            .predicted
            .as_ref()
            .and_then(|cells| {
                cells
                    .iter()
                    .find(|cell| cell.0 == expected.position && cell.1 == 0)
            })
            .and_then(|cell| stream.block_network_id(cell.2));
        bevy::log::debug!(tick, destination = ?expected.position, block = %expected.name,
            states = %expected.states, ?placed_runtime_id, support = ?observed.target.position,
            face = observed.target.face, click = ?observed.target.relative_hit,
            slot = observed.selection.slot, item = observed.selection.item.network_id(),
            count = observed.selection.item.count(), player_position = ?state.position,
            support_runtime_id = plan.block_network_id, consumes_item = plan.change.is_some(),
            yaw = gameplay::movement::wire_yaw(180.0 - yaw.to_degrees()), pitch = -pitch.to_degrees(),
            "Fast Place: submitting native placement batch");
    }
    let result = (!packets.is_empty()).then(|| context.network.send_inventory_packets(packets));
    let sent = matches!(result, Some(Ok(())));
    if let Some(expected) = &attempt.expected {
        bevy::log::debug!(tick, destination = ?expected.position, sent,
            queue_full = matches!(result, Some(Err(client_session::BatchSendError::Full))),
            "Fast Place: placement batch admission");
    }
    if sent {
        runtime.intention.record(
            trigger == ItemUseTrigger::SimulationTick,
            plan.destination,
            plan.local_use,
            plan.local_use != LocalUse::Interact
                && plan
                    .paired
                    .or_else(|| {
                        held_block_store_id(stream, observed.selection.item.block_runtime_id())
                    })
                    .is_some_and(|block| {
                        context
                            .collisions
                            .block_has_build_intention(stream.network_id_mode(), block)
                    }),
            state.sneaking,
            observed.target.hit_world,
        );
    }
    if !runtime.admit(
        trigger,
        due,
        tick,
        plan.local_use,
        plan.takes_press,
        clock,
        sent,
    ) {
        runtime.refuse_transport(
            tick,
            plan.local_use,
            matches!(result, Some(Err(client_session::BatchSendError::Full))),
        );
        before_swing.defer_unadmitted_attempt(swings);
        *swings = before_swing;
        return false;
    }
    if trigger == ItemUseTrigger::SimulationTick && plan.local_use == LocalUse::Nothing {
        runtime.record_failed_repeat(tick, target);
    }
    // The use starts on its build tick, before physics simulates it, so that tick moves
    // at the use's speed and its input carries the start.
    if plan.local_use == LocalUse::UseItem {
        let profile = stream
            .authority()
            .item_use_profile(observed.selection.item.network_id());
        if item_use.start_use_on(
            observed.selection.clone(),
            profile.ticks.unwrap_or_default(),
            profile.movement,
            tick,
        ) {
            movement.mark_started_using_item(tick);
        }
        // Vanilla's client sweeps dust off the brushed block from its use on the block,
        // at most once per effects cooldown.
        if item_use.brush_effects_due(tick) {
            context.brush_dust.write(crate::particles::LocalBrushDust {
                block: observed.target.position,
                face: observed.target.face,
                relative_hit: observed.target.relative_hit,
                runtime_id: observed.target.runtime_id,
            });
        }
    }
    // Vanilla's `GameMode::useItemOn` starts the item's cooldown after any successful use on.
    if (plan.local_use == LocalUse::UseItem
        || plan.local_use == LocalUse::Place
            && plan
                .change
                .as_ref()
                .is_none_or(|change| !change.to.is_empty()))
        && let Some(cooldown) = plan.cooldown
    {
        item_use.start_cooldown(cooldown, tick);
    }
    if let Some(change) = &plan.change {
        player_runtime
            .inventory
            .ledger_mut()
            .settle_use_slot_change(plan.server_selection.slot, plan.inventory_revision, change);
        runtime.inventory.commit(
            plan.server_selection.clone(),
            plan.inventory_revision,
            change.clone(),
        );
    }
    // Vanilla voices placement only from the server's level sound event, never locally.
    // Java re-equips the held item after each placement.
    if plan.local_use == LocalUse::Place
        && let Some(stream) = context.client_world.stream.as_mut()
    {
        stream.reset_local_java_equip();
    }
    // Vanilla places locally as it sends; a correction replaces the prediction.
    if let (true, Some(cells), Some(stream)) = (
        sent && !attempt.air_place,
        plan.predicted.as_ref(),
        context.client_world.stream.as_mut(),
    ) {
        let applied = stream.predict_blocks(cells);
        if applied {
            runtime.prediction_frames.committed(
                context.frame.0,
                cells
                    .iter()
                    .filter_map(|cell| stream.prediction_generation(cell.0)),
            );
        }
        bevy::log::debug!(?cells, applied, "local block prediction committed");
    }
    true
}
