//! A Creative catalog selection may outlive its press while the server confirms inventory.

use super::*;
use bevy::prelude::{Resource, Vec3};
use inventory::inventory_ledger::{
    CellGesture, CreativeDestination, InventoryTarget, PLAYER_INVENTORY_SLOT_COUNT,
    PlayerInventorySlot,
};
use mod_host::CreativePlacementOffer;
use protocol::NetworkItemStack;
use semantic_input::identity::{InputAuthorityGeneration, InputSequence};
use simulation_clock::identity::PositionAuthorityGeneration;
use std::time::Duration;

#[derive(Resource, Default)]
pub(crate) struct CreativePlacement {
    pub offer: Option<CreativePlacementOffer>,
    pub locked: bool,
    pub hold_active: bool,
    /// Block names of recent offers, newest last. Swapping a block into the hotbar replaces a
    /// slot holding none of them, so alternating materials stay in the hotbar.
    recent: Vec<String>,
    pending: Option<Pending>,
    sneak: SneakHold,
}

/// Sneak Fast Place holds so a click on an interactive support (a chest, furnace, lever or
/// button) places the block instead of using the support.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SneakHold {
    #[default]
    Idle,
    /// Held until the waiting press is sent.
    Holding,
    /// Held until the movement sample of this tick, which carries the click, is written.
    Releasing(u64),
}

impl CreativePlacement {
    /// Whether Fast Place holds sneak for the coming movement frame. A release ends once the tick
    /// carrying the click is written, and a hold ends when its press is gone.
    pub(crate) fn holds_sneak(&mut self, latest_written_tick: Option<u64>) -> bool {
        match self.sneak {
            SneakHold::Idle => false,
            SneakHold::Holding if self.pending.is_some() => true,
            SneakHold::Releasing(tick)
                if latest_written_tick.is_none_or(|written| written < tick) =>
            {
                true
            }
            SneakHold::Holding | SneakHold::Releasing(_) => {
                self.sneak = SneakHold::Idle;
                false
            }
        }
    }
}

type StackKey = (i32, u32, i32);

struct Pending {
    interacting: bool,
    game_mode: PlayerGameMode,
    offer: CreativePlacementOffer,
    support: ([i32; 3], u8, u32),
    scope: (u64, i32),
    input_generation: InputAuthorityGeneration,
    position_generation: PositionAuthorityGeneration,
    expires: Duration,
    slot: u8,
    original_slot: u8,
    key: StackKey,
    request: Option<i32>,
    catalog: Option<Catalog>,
    replacement: Option<StackKey>,
    selecting: bool,
    aim: Option<TemporaryAim>,
    bridge: bool,
    trigger: ItemUseTrigger,
    /// When the attempt started; its repeat interval counts from here.
    started: Duration,
}

#[derive(Clone, Copy)]
struct TemporaryAim {
    placement: super::orientation::PlacementAim,
    tick: u64,
}

impl TemporaryAim {
    fn written(self, movement: &MovementTicker) -> bool {
        let [pitch, yaw] = super::orientation::wire_facing(self.placement.rotation);
        movement
            .latest_written_facing()
            .is_some_and(|(tick, [sent_pitch, sent_yaw])| {
                let yaw_error = (sent_yaw - yaw + 180.0).rem_euclid(360.0) - 180.0;
                tick >= self.tick && (sent_pitch - pitch).abs() <= 0.05 && yaw_error.abs() <= 0.05
            })
    }
}

#[derive(Clone, Copy, Debug)]
enum Catalog {
    Opening(u32),
    CarriedOpening(u8),
    Taking(i32),
    Moving(i32),
    ClearingCursor(i32),
    Closing,
}

enum CatalogStep {
    Waiting,
    Ready,
    Failed,
}

fn advance_catalog(player: &mut client_state::PlayerRuntime, pending: &mut Pending) -> CatalogStep {
    let Some(stage) = pending.catalog else {
        return CatalogStep::Ready;
    };
    let ledger = player.inventory_commands();
    if matches!(stage, Catalog::Closing) {
        return if ledger.background_personal_busy() {
            CatalogStep::Waiting
        } else {
            pending.catalog = None;
            CatalogStep::Ready
        };
    }
    if !ledger.background_personal_active() {
        return CatalogStep::Failed;
    }
    let next = match stage {
        Catalog::CarriedOpening(source) if ledger.background_personal_ready() => ledger
            .begin_hotbar_swap(InventoryTarget::Player(source), pending.slot)
            .map(Catalog::Moving),
        Catalog::Opening(id) if ledger.background_personal_ready() => ledger
            .begin_creative_take(id, CreativeDestination::Cursor, None)
            .map(Catalog::Taking),
        Catalog::Taking(request) if !ledger.has_pending_request(request) => {
            let slot_unchanged = match pending.replacement {
                Some(previous) => ledger.backing_stack(pending.slot).map(key) == Some(previous),
                None => matches!(
                    ledger.slot_state(pending.slot),
                    Some(PlayerInventorySlot::Empty)
                ),
            };
            if ledger.cursor_stack().map(key) != Some(pending.key) || !slot_unchanged {
                ledger.request_background_personal_close();
                return CatalogStep::Failed;
            }
            ledger
                .begin_target_gesture(InventoryTarget::Player(pending.slot), CellGesture::Click)
                .map(Catalog::Moving)
        }
        Catalog::Moving(request) if !ledger.has_pending_request(request) => {
            let confirmed = ledger.backing_stack(pending.slot).map(key) == Some(pending.key);
            if !confirmed {
                ledger.request_background_personal_close();
                return CatalogStep::Failed;
            }
            if pending.game_mode == PlayerGameMode::Creative
                && let Some(replaced) = pending.replacement
                && ledger.cursor_stack().map(key) == Some(replaced)
            {
                let spare =
                    (protocol::HOTBAR_SLOT_COUNT..PLAYER_INVENTORY_SLOT_COUNT as u8).find(|slot| {
                        matches!(ledger.slot_state(*slot), Some(PlayerInventorySlot::Empty))
                    });
                let request = match spare {
                    Some(slot) => ledger
                        .begin_target_gesture(InventoryTarget::Player(slot), CellGesture::Click),
                    None => ledger.begin_destroy_cursor(None),
                };
                request.map(Catalog::ClearingCursor)
            } else {
                ledger.request_background_personal_close();
                Ok(Catalog::Closing)
            }
        }
        Catalog::ClearingCursor(request) if !ledger.has_pending_request(request) => {
            ledger.request_background_personal_close();
            Ok(Catalog::Closing)
        }
        _ => return CatalogStep::Waiting,
    };
    match next {
        Ok(next) => {
            bevy::log::debug!(
                slot = pending.slot,
                ?next,
                "Fast Place: advancing Creative catalog"
            );
            pending.catalog = Some(next);
            CatalogStep::Waiting
        }
        Err(error) => {
            bevy::log::info!(slot = pending.slot, ?stage, %error, "Fast Place: Creative catalog gesture failed");
            ledger.request_background_personal_close();
            CatalogStep::Failed
        }
    }
}

/// A planned double slab is built from the full block drawn the same way, such as dark oak
/// planks for a dark oak double slab; every other offer is kept.
fn full_block_offer(
    collisions: &PhysicsCollisionRegistries,
    stream: &chunk_pipeline::WorldStream,
    mut offer: mod_host::CreativePlacementOffer,
) -> mod_host::CreativePlacementOffer {
    let mode = stream.network_id_mode();
    let full = collisions
        .block_default_runtime_id(mode, &offer.name)
        .and_then(|slab| {
            gameplay::placement_items::double_slab_full_block(
                collisions,
                stream.runtime_assets(),
                mode,
                slab,
            )
        })
        .and_then(|block| {
            Some((
                collisions.block_identifier(mode, block)?,
                collisions.block_canonical_state(mode, block)?,
            ))
        });
    if let Some((name, states)) = full {
        bevy::log::debug!(planned = %offer.name, full = name, "Fast Place builds a double slab as its full block");
        offer.name = name.to_owned();
        offer.states = states.to_owned();
    }
    offer
}

fn key(stack: &NetworkItemStack) -> StackKey {
    (stack.network_id, stack.metadata, stack.block_runtime_id)
}

fn support(observed: &FrozenBlockObservation) -> ([i32; 3], u8, u32) {
    (
        observed.target.position,
        observed.target.face,
        observed.target.runtime_id,
    )
}

/// Starts an assisted item selection, leaving ordinary placement available otherwise.
pub(super) fn try_start(
    player: &mut client_state::PlayerRuntime,
    context: &mut BlockUseContext,
    observed: &FrozenBlockObservation,
    movement: &mut MovementTicker,
    bridge: bool,
    trigger: ItemUseTrigger,
) -> bool {
    let Some(policy) = context.creative.as_deref_mut() else {
        return false;
    };
    let Some(stream) = context.client_world.stream.as_ref() else {
        return false;
    };
    let Some(offer) = policy.offer.clone() else {
        clear(Some(policy), player, movement);
        bevy::log::debug!(support = ?observed.target.position, face = observed.target.face, "Fast Place: native hit has no schematic offer");
        return false;
    };
    let offer = full_block_offer(&context.collisions, stream, offer);
    bevy::log::debug!(block = %offer.name, states = %offer.states, ghost = ?offer.position,
        support = ?observed.target.position, face = observed.target.face, "Fast Place: examining use press");
    let Some(game_mode @ (PlayerGameMode::Creative | PlayerGameMode::Survival)) =
        player.facts.player_game_mode()
    else {
        return false;
    };
    let interacting = observed.target.position == offer.position
        && super::orientation::needs_toggle(
            &offer,
            context
                .collisions
                .block_identifier(stream.network_id_mode(), observed.target.runtime_id),
            context
                .collisions
                .block_canonical_state(stream.network_id_mode(), observed.target.runtime_id),
        );
    let destination = PaletteWorld::new(
        stream.collision_store(),
        context.collisions.registry(stream.network_id_mode()),
        stream.current_dimension(),
        Default::default(),
    )
    .observe(|world| world.primary_runtime_id(offer.position).ok())
    .ok()
    .and_then(|observed| observed.value);
    let mode = stream.network_id_mode();
    if gameplay::placement_plants::supported(&offer.name)
        && destination.is_some_and(|block| {
            context.collisions.block_identifier(mode, block) == Some(offer.name.as_str())
                && context
                    .collisions
                    .block_canonical_state(mode, block)
                    .is_some_and(|state| super::orientation::strict_state_matches(&offer, state))
        })
    {
        // The tile verifier can lag behind the server. Do not replace an already
        // correct plant or let the same held gesture fall back to vanilla.
        return true;
    }
    if !interacting
        && !destination.is_some_and(|block| {
            let fact = context.collisions.block_facts(mode, block);
            fact.traits.contains(assets::registry::BlockTraits::AIR)
                || (gameplay::placement_plants::supported(&offer.name)
                    && gameplay::placement_plants::replaceable(fact))
        })
    {
        bevy::log::debug!(ghost = ?offer.position, "Fast Place: destination is occupied or not loaded");
        return false;
    }
    let ledger = player.inventory.ledger();
    // A wall sign, banner or coral fan is built from the item of its floor-standing form.
    let item_block =
        gameplay::placement_items::standing_form(&context.collisions, mode, &offer.name)
            .unwrap_or_else(|| offer.name.clone());
    let quality = |stack: &NetworkItemStack| {
        let block = super::build::fast_place_block(
            &context.collisions,
            stream,
            stack.block_runtime_id,
            || stream.authority().item_identifier(stack.network_id),
        )?;
        (context.collisions.block_identifier(mode, block)? == item_block.as_str()).then(|| {
            context.collisions.block_canonical_state(mode, block) == Some(offer.states.as_str())
        })
    };
    let selected = observed.selection.slot;
    policy.hold_active |= offer.repeat_while_held;
    if policy.recent.last() != Some(&offer.name) {
        policy.recent.retain(|name| *name != offer.name);
        policy.recent.push(offer.name.clone());
        if policy.recent.len() > protocol::HOTBAR_SLOT_COUNT as usize {
            policy.recent.remove(0);
        }
    }
    if interacting {
        policy.pending = Some(Pending {
            interacting: true,
            game_mode,
            offer,
            support: support(observed),
            scope: (
                stream.authority().actor_session_id(),
                stream.current_dimension(),
            ),
            input_generation: observed.frame.input_authority_generation,
            position_generation: observed.frame.position_authority_generation,
            expires: context.time.elapsed() + Duration::from_secs(2),
            slot: selected,
            original_slot: selected,
            key: (0, 0, 0),
            request: None,
            catalog: None,
            replacement: None,
            selecting: true,
            aim: None,
            bridge,
            trigger,
            started: context.time.elapsed(),
        });
        return true;
    }
    let use_air = offer.strict_state
        && bridge
        && observed.target.runtime_id == stream.air_block_id()
        && observed.target.position == offer.position;
    let replacing_plant = gameplay::placement_plants::supported(&offer.name)
        && observed.target.position == offer.position
        && gameplay::placement_plants::replaceable(
            context
                .collisions
                .block_facts(mode, observed.target.runtime_id),
        );
    if observed.target.face > 5
        || (!use_air
            && !replacing_plant
            && placement_cell(observed.target.position, observed.target.face) != offer.position)
    {
        bevy::log::info!(
            ghost = ?offer.position,
            support = ?observed.target.position,
            face = observed.target.face,
            "Fast Place needs the support face adjoining the ghost"
        );
        clear(Some(policy), player, movement);
        return false;
    }
    if policy.pending.is_some() {
        return true;
    }

    let hotbar = (0..protocol::HOTBAR_SLOT_COUNT)
        .filter_map(|slot| {
            let stack = ledger.backing_stack(slot)?;
            Some((quality(stack)?, slot, key(stack)))
        })
        .max_by_key(|(exact, slot, _)| (*exact, *slot == selected));
    let empty = (0..protocol::HOTBAR_SLOT_COUNT)
        .find(|slot| matches!(ledger.slot_state(*slot), Some(PlayerInventorySlot::Empty)));
    // Without an empty slot, replace the hotbar slot whose block was offered least recently
    // (or never), keeping the selected slot on a tie. Replacing the selected slot each time
    // made two alternating materials swap through the inventory on every placement.
    let victim = (0..protocol::HOTBAR_SLOT_COUNT)
        .filter_map(|slot| {
            let stack = ledger.backing_stack(slot)?;
            let name = super::build::fast_place_block(
                &context.collisions,
                stream,
                stack.block_runtime_id,
                || stream.authority().item_identifier(stack.network_id),
            )
            .and_then(|block| context.collisions.block_identifier(mode, block));
            let rank = name
                .and_then(|name| policy.recent.iter().position(|recent| recent == name))
                .map_or(-1, |index| index as i32);
            Some((rank, slot != selected, slot))
        })
        .min()
        .map(|(_, _, slot)| slot);
    let empty = empty.or(victim);
    let carried = (protocol::HOTBAR_SLOT_COUNT..PLAYER_INVENTORY_SLOT_COUNT as u8)
        .filter_map(|slot| {
            let stack = ledger.backing_stack(slot)?;
            Some((quality(stack)?, slot, key(stack)))
        })
        .max_by_key(|(exact, _, _)| *exact);
    let catalog = ledger.creative_catalog().and_then(|catalog| {
        catalog
            .items
            .iter()
            .filter_map(|item| {
                Some((
                    quality(&item.stack)?,
                    item.creative_network_id,
                    key(&item.stack),
                ))
            })
            .max_by_key(|(exact, _, _)| *exact)
    });
    let mut replacement = None;
    let (slot, key, request, catalog) = if let Some((_, slot, key)) = hotbar {
        (slot, key, None, None)
    } else if let Some((_, source, item)) = carried {
        let slot = empty.unwrap_or(selected);
        if !player
            .inventory_commands()
            .request_background_personal_open(stream.local_player_runtime_id())
        {
            bevy::log::info!(block = %offer.name, source, slot,
                "Fast Place could not open inventory for the carried block");
            return false;
        }
        (slot, item, None, Some(Catalog::CarriedOpening(source)))
    } else {
        if game_mode != PlayerGameMode::Creative {
            bevy::log::info!(block = %offer.name, "Fast Place needs this block in the player's inventory");
            return false;
        }
        let Some((_, id, item)) = catalog else {
            bevy::log::info!(
                block = %offer.name,
                empty_slot = empty.is_some(),
                "Fast Place could not find a usable hotbar or Creative catalog item"
            );
            return false;
        };
        let slot = empty.unwrap_or(selected);
        if !matches!(ledger.slot_state(slot), Some(PlayerInventorySlot::Empty)) {
            let Some(previous) = ledger.backing_stack(slot) else {
                return false;
            };
            replacement = Some(key(previous));
        }
        if !player
            .inventory_commands()
            .request_background_personal_open(stream.local_player_runtime_id())
        {
            let blocker = player
                .inventory
                .ledger()
                .background_personal_open_blocker(stream.local_player_runtime_id());
            bevy::log::info!(block = %offer.name, slot, blocker,
                "Fast Place could not open Creative inventory");
            return false;
        }
        (slot, item, None, Some(Catalog::Opening(id)))
    };
    bevy::log::debug!(block = %offer.name, slot, ?request, ?catalog, ?game_mode, item_network_id = key.0,
        metadata = key.1, item_block_runtime_id = key.2, "Fast Place waiting for inventory and selection");
    policy.pending = Some(Pending {
        interacting: false,
        game_mode,
        offer,
        support: support(observed),
        scope: (
            stream.authority().actor_session_id(),
            stream.current_dimension(),
        ),
        input_generation: observed.frame.input_authority_generation,
        position_generation: observed.frame.position_authority_generation,
        expires: context.time.elapsed() + Duration::from_secs(2),
        slot,
        original_slot: selected,
        key,
        request,
        catalog,
        replacement,
        selecting: slot == selected && request.is_none() && catalog.is_none(),
        aim: None,
        bridge,
        trigger,
        started: context.time.elapsed(),
    });
    true
}

/// Clears any queued press if gameplay lost input ownership.
pub(super) fn clear(
    policy: Option<&mut CreativePlacement>,
    player: &mut client_state::PlayerRuntime,
    movement: &mut MovementTicker,
) {
    if let Some(policy) = policy {
        policy.hold_active = false;
        if policy
            .pending
            .take()
            .is_some_and(|pending| pending.aim.is_some())
        {
            movement.end_placement_facing();
        }
        player
            .inventory
            .ledger_mut()
            .request_background_personal_close();
    }
}

/// Resumes one admitted press after backing inventory and equipment are authoritative.
#[allow(
    clippy::too_many_arguments,
    reason = "shares the vanilla build context and its owners"
)]
pub(super) fn resume(
    player: &mut client_state::PlayerRuntime,
    context: &mut BlockUseContext,
    runtime: &mut BlockUseRuntime,
    item_use: &mut client_interaction::item_use::ItemUseRuntime,
    swings: &mut SwingTracker,
    movement: &mut MovementTicker,
    pick: Option<FramePick>,
    held: bool,
    mode: PlayerInputMode,
    authority: (InputAuthorityGeneration, InputSequence),
) -> bool {
    let Some(policy) = context.creative.as_deref_mut() else {
        return false;
    };
    let Some(mut pending) = policy.pending.take() else {
        return false;
    };
    if policy.offer.as_ref() != Some(&pending.offer) {
        bevy::log::debug!(ghost = ?pending.offer.position, current = ?policy.offer, "Fast Place: schematic offer changed before placement");
        movement.end_placement_facing();
        player
            .inventory
            .ledger_mut()
            .request_background_personal_close();
        return true;
    }
    let Some(stream) = context.client_world.stream.as_ref() else {
        movement.end_placement_facing();
        player
            .inventory
            .ledger_mut()
            .request_background_personal_close();
        return true;
    };
    if player.facts.player_game_mode() != Some(pending.game_mode)
        || pending.scope
            != (
                stream.authority().actor_session_id(),
                stream.current_dimension(),
            )
        || context.time.elapsed() >= pending.expires
        || (pending.trigger == ItemUseTrigger::SimulationTick && !held)
        || pending.input_generation != authority.0
        || pending.position_generation != movement.position_authority_identity().1
        || context.client_world.respawn.input_held()
        || (!pending.selecting && player.selected_hotbar_slot() != Some(pending.original_slot))
    {
        bevy::log::info!(block = %pending.offer.name, "Fast Place cancelled after its target, input, mode or deadline changed");
        movement.end_placement_facing();
        player
            .inventory
            .ledger_mut()
            .request_background_personal_close();
        return true;
    }
    if let Some(aim) = pending.aim {
        if !movement.placement_facing_active() {
            movement.end_placement_facing();
            return true;
        }
        if !aim.written(movement) {
            policy.pending = Some(pending);
            return true;
        }
    }
    match advance_catalog(player, &mut pending) {
        CatalogStep::Waiting => {
            policy.pending = Some(pending);
            return true;
        }
        CatalogStep::Failed => {
            bevy::log::info!(block = %pending.offer.name, "Fast Place: Creative inventory did not supply the block");
            return true;
        }
        CatalogStep::Ready => {}
    }
    let ledger = player.inventory.ledger();
    if pending
        .request
        .is_some_and(|id| ledger.has_pending_request(id))
    {
        policy.pending = Some(pending);
        return true;
    }
    if !pending.interacting && ledger.backing_stack(pending.slot).map(key) != Some(pending.key) {
        movement.end_placement_facing();
        bevy::log::info!(block = %pending.offer.name, slot = pending.slot, "Fast Place inventory did not confirm the requested block");
        return true;
    }
    if !pending.selecting {
        bevy::log::debug!(slot = pending.slot, request = ?pending.request, "Fast Place: backing stack ready, queuing equipment selection");
        player
            .inventory
            .queue_local_hotbar_selection(pending.slot, Some(pending.game_mode));
        item_use.retire_reselected_use(pending.slot);
        pending.selecting = true;
    }
    if player.inventory.pending_hotbar_selection().is_some() {
        policy.pending = Some(pending);
        return true;
    }
    if player.selected_hotbar_slot() != Some(pending.slot) {
        movement.end_placement_facing();
        return true;
    }
    let Some(state) = movement.build_action_state().filter(|_| {
        movement.accepts_block_interactions()
            && !movement.has_held_release()
            && movement.between_ticks_sample().is_some()
    }) else {
        policy.pending = Some(pending);
        return true;
    };
    let observed = if pending.bridge {
        super::target::observe_bridge_target(
            player,
            context,
            &context.origin,
            mode,
            authority,
            movement.position_authority_identity().1,
            &state,
        )
    } else {
        observe_use_target(
            player,
            context,
            &context.origin,
            mode,
            pending.game_mode == PlayerGameMode::Survival,
            authority,
            movement.position_authority_identity().1,
            runtime,
            pick,
            ItemUseTrigger::PlayerInput,
            &state,
        )
    };
    let Some(observed) = observed else {
        movement.end_placement_facing();
        bevy::log::debug!(
            slot = pending.slot,
            "Fast Place: equipment selected but native support could not be observed"
        );
        return true;
    };
    if pending.aim.is_none()
        && support(&observed) != pending.support
        && (!pending.offer.correct_aim
            || placement_cell(observed.target.position, observed.target.face)
                != pending.offer.position)
    {
        movement.end_placement_facing();
        bevy::log::info!(block = %pending.offer.name, "Fast Place support changed before placement");
        return true;
    }
    if !pending.interacting
        && pending.offer.correct_aim
        && pending.aim.is_none()
        && let Some(placement) = super::orientation::correction(
            player,
            context,
            &observed,
            &state,
            &pending.offer,
            pending.bridge,
        )
        .or_else(|| {
            if !pending.bridge {
                return None;
            }
            let eye = Vec3::from_array(observed.ray.origin);
            let direction = Vec3::from_array(observed.ray.direction);
            Some(super::orientation::PlacementAim::observed(
                client_presentation::camera::look_at_target(eye, eye + direction),
                support(&observed),
                direction,
            ))
        })
        && movement.begin_placement_facing(
            super::orientation::wire_facing(placement.rotation),
            (placement.rotation * Vec3::NEG_Z).to_array(),
            placement.interaction_ray.to_array(),
        )
    {
        let aim = TemporaryAim {
            placement,
            tick: movement.next_tick(),
        };
        pending.aim = Some(aim);
        bevy::log::debug!(ghost = ?pending.offer.position, support = ?placement.support,
            tick = aim.tick, support_ray = ?placement.support_ray,
            facing = ?super::orientation::wire_facing(placement.rotation),
            "Fast Place: sending placement facing");
        if let Some(policy) = context.creative.as_deref_mut() {
            policy.pending = Some(pending);
        }
        return true;
    }
    let mut view = *context.view;
    let observed = if let Some(aim) = pending.aim {
        let Some(adjusted) =
            super::orientation::recapture(player, context, observed, &state, aim.placement)
        else {
            movement.end_placement_facing();
            return true;
        };
        view.set_rotation(aim.placement.rotation);
        adjusted
    } else {
        observed
    };
    let use_air = observed.target.position == pending.offer.position
        && observed.target.runtime_id == stream.air_block_id();
    // A click on a chest, furnace, lever or the like would use it; sneak first so it places.
    let interactive_support = !pending.interacting
        && !use_air
        && context
            .collisions
            .block_facts(stream.network_id_mode(), observed.target.runtime_id)
            .traits
            .contains(assets::registry::BlockTraits::INTERACTIVE);
    if interactive_support && !state.sneaking {
        if let Some(policy) = context.creative.as_deref_mut() {
            bevy::log::debug!(ghost = ?pending.offer.position, support = ?observed.target.position,
                "Fast Place: holding sneak to place against an interactive block");
            policy.sneak = SneakHold::Holding;
            policy.pending = Some(pending);
        }
        return true;
    }
    let started = pending.started;
    runtime.forget_press_resolution();
    runtime.selection_changed(&observed.selection);
    let clock = RepeatClock::for_state(context.time.elapsed(), &state, Some(pending.game_mode));
    let mut build =
        build::BuildInput::new(observed, state, pending.trigger, clock.now, clock, view);
    build.air_place = pending.offer.strict_state && use_air;
    if !pending.interacting {
        build.expected = Some(pending.offer);
    } else {
        build.interaction = Some(pending.offer);
    }
    bevy::log::debug!(
        slot = pending.slot,
        "Fast Place: equipment admitted, resuming native placement"
    );
    build::advance(
        player, context, runtime, item_use, swings, movement, &mut build,
    );
    if let Some(policy) = context.creative.as_deref_mut()
        && policy.sneak == SneakHold::Holding
    {
        policy.sneak = SneakHold::Releasing(movement.next_tick());
    }
    // The facing tick and any inventory wait overlap the repeat interval, so oriented blocks
    // (logs, slabs, stairs) repeat as fast as plain ones.
    runtime.credit_assisted_start(started);
    movement.end_placement_facing();
    runtime.clear_press();
    true
}

#[cfg(test)]
mod sneak_hold_tests {
    use super::{CreativePlacement, SneakHold};

    /// Sneak stays held until the tick carrying the click is written, and a hold without a
    /// waiting press ends at once.
    #[test]
    fn a_sneak_release_waits_for_the_click_tick() {
        let mut policy = CreativePlacement {
            sneak: SneakHold::Releasing(10),
            ..Default::default()
        };
        assert!(policy.holds_sneak(None));
        assert!(policy.holds_sneak(Some(9)));
        assert!(!policy.holds_sneak(Some(10)));
        assert!(!policy.holds_sneak(Some(9)));
        policy.sneak = SneakHold::Holding;
        assert!(!policy.holds_sneak(None));
        assert_eq!(policy.sneak, SneakHold::Idle);
    }
}
