use super::*;

#[test]
fn replacement_targeting_only_offers_wrong_plant_cells() {
    use schematica_core::{BlockState, CellStatus};
    let mut viewer = reference_viewer();
    viewer.schematic = Schematic::new("Replacement", [2, 1, 1]);
    viewer
        .schematic
        .set([0, 0, 0], BlockState::new("minecraft:poppy"));
    viewer
        .schematic
        .set([1, 0, 0], BlockState::new("minecraft:stone"));
    viewer.placement = Some(Placement::new([0; 3]));
    assert!(viewer.accepts_plant_replacement([0, 0, 0], CellStatus::Wrong));
    assert!(!viewer.accepts_plant_replacement([0, 0, 0], CellStatus::Correct));
    assert!(!viewer.accepts_plant_replacement([1, 0, 0], CellStatus::Wrong));
    assert!(!viewer.accepts_plant_replacement([3, 0, 0], CellStatus::Wrong));
}

#[test]
fn placement_lock_is_scoped_to_non_air_guide_cells() {
    let mut viewer = reference_viewer();
    viewer.options.placement_lock = true;
    viewer.options.fast_place = true;
    viewer.shown = true;
    viewer.render_all = true;
    viewer.schematic = Schematic::new("Lock", [3, 1, 1]);
    viewer.schematic.set(
        [1, 0, 0],
        schematica_core::BlockState::new("minecraft:stone"),
    );
    viewer.placement = Some(Placement::new([10, 64, 20]));
    assert!(viewer.lock_destination(Some([11, 64, 20])));
    assert!(!viewer.lock_destination(Some([10, 64, 20])));
    assert!(!viewer.lock_destination(Some([14, 64, 20])));
    assert!(!viewer.lock_destination(None));
    viewer.shown = false;
    assert!(!viewer.lock_destination(Some([11, 64, 20])));
}

#[test]
fn looking_from_inside_a_ghost_starts_at_the_next_voxel() {
    let mut crossed = Vec::new();
    let chosen = next_guide_cell([0.1, 0.5, 0.5], [1.0, 0.0, 0.0], 6., |cell| {
        crossed.push(cell);
        cell == [2, 0, 0]
    });
    assert_eq!(chosen, Some([2, 0, 0]));
    assert_eq!(crossed, [[1, 0, 0], [2, 0, 0]]);
}

#[test]
fn hold_aim_stops_at_a_real_block_and_ignores_off_ray_ghosts() {
    use schematica_core::BlockState;
    let mut viewer = reference_viewer();
    viewer.schematic = Schematic::new("Aim", [4, 1, 4]);
    viewer.placement = Some(Placement::new([0, 0, 0]));
    viewer.pose = Some(gameplay::Snapshot {
        session: 1,
        dimension: 0,
        eye: gameplay::Vector3 {
            x: 0.5,
            y: 0.5,
            z: 3.5,
        },
        yaw: 0.,
        pitch: 0.,
        frame_seconds: 0.016,
    });
    for cell in [[1, 0, 2], [0, 0, 1]] {
        viewer
            .schematic
            .set(cell, BlockState::new("minecraft:stone"));
    }
    assert_eq!(viewer.aimed_next_ghost(None), Some([0, 0, 1]));
    assert_eq!(viewer.aimed_next_ghost(Some([0, 0, 2])), None);
    viewer.schematic.set([0, 0, 1], BlockState::air());
    assert_eq!(viewer.aimed_next_ghost(None), None);
}

#[test]
fn aim_passes_through_the_empty_part_of_a_thin_ghost_to_the_one_behind() {
    use schematica_core::{BlockState, StateValue};
    let mut viewer = reference_viewer();
    viewer.schematic = Schematic::new("Thin", [1, 1, 4]);
    viewer.placement = Some(Placement::new([0, 0, 0]));
    let look_from = |y: f32| gameplay::Snapshot {
        session: 1,
        dimension: 0,
        eye: gameplay::Vector3 { x: 0.5, y, z: 3.5 },
        yaw: 0.,
        pitch: 0.,
        frame_seconds: 0.016,
    };
    viewer.schematic.set(
        [0, 0, 2],
        BlockState::new("minecraft:stone_block_slab")
            .with("minecraft:vertical_half", StateValue::Str("bottom".into())),
    );
    viewer.schematic.set([0, 0, 0], BlockState::new("minecraft:stone"));
    // Looking over the low slab reaches the block behind it; looking at the slab picks it.
    viewer.pose = Some(look_from(0.8));
    assert_eq!(viewer.aimed_next_ghost(None), Some([0, 0, 0]));
    viewer.pose = Some(look_from(0.3));
    assert_eq!(viewer.aimed_next_ghost(None), Some([0, 0, 2]));
}

pub(super) fn reference_viewer() -> Viewer {
    let entries = [
        ("minecraft:oak_planks", 104),
        ("minecraft:cobblestone", 49),
        ("minecraft:oak_log", 12),
        ("minecraft:glass_pane", 4),
        ("minecraft:wooden_door", 1),
        ("minecraft:torch", 1),
    ];
    let mut schematic = Schematic::new("Lakeside Cabin", [7, 5, 7]);
    let mut cursor = 0;
    for (name, count) in entries {
        for _ in 0..count {
            schematic.set(
                [cursor % 7, cursor / 49, (cursor / 7) % 7],
                schematica_core::BlockState::new(name),
            );
            cursor += 1;
        }
    }
    let materials = entries
        .into_iter()
        .map(|(name, total)| MaterialLine {
            name: name.into(),
            total: total as u64,
            missing: total as u64,
        })
        .collect();
    let mut viewer = Viewer::new(
        schematic,
        materials,
        vec![
            "Lakeside Cabin".into(),
            "Sugar Cane Farm".into(),
            "Passenger Wagon".into(),
        ],
        Options {
            textured: true,
            assist: false,
            fast_place: true,
            correct_aim: false,
            strict_place: false,
            placement_lock: false,
            hold_place: false,
            auto_layer: true,
            paste_destroy: false,
            remember_placement: false,
            saved_placements: BTreeMap::new(),
            tool_item: false,
            bridge_place: true,
            lightness: DEFAULT_LIGHTNESS,
            library_revision: 0,
            deleted_entries: Vec::new(),
        },
        true,
    );
    let mut placement = Placement::new([120, 64, -48]);
    placement.quarter_turns = 1;
    viewer.placement = Some(placement);
    viewer.active_entry = Some(0);
    viewer
}

#[test]
fn deleting_compacts_the_library_and_reimport_restores_entries() {
    let mut viewer = reference_viewer();
    viewer.menu_action("delete-0");
    assert!(!viewer.loaded);
    assert!(viewer.placement.is_none());
    assert_eq!(viewer.visible_entry(0), Some(1));
    viewer.menu_action("toggle-0");
    assert_eq!(viewer.request, Some(1));
    viewer.options.select_library(0);
    assert_eq!(viewer.options.deleted_entries, vec![0]);
    viewer.options.select_library(1);
    assert!(viewer.options.deleted_entries.is_empty());
    assert_eq!(viewer.library[0], "Lakeside Cabin");
}

#[test]
fn menu_edits_turn_the_guide_and_select_layer_ranges() {
    let mut viewer = Viewer::new(
        Schematic::new("fixture", [2, 4, 3]),
        Vec::new(),
        vec!["Demo".into(), "Wagon".into()],
        Options {
            textured: true,
            assist: false,
            fast_place: false,
            correct_aim: false,
            strict_place: false,
            placement_lock: false,
            hold_place: false,
            auto_layer: true,
            paste_destroy: false,
            remember_placement: false,
            saved_placements: BTreeMap::new(),
            tool_item: false,
            bridge_place: true,
            lightness: DEFAULT_LIGHTNESS,
            library_revision: 0,
            deleted_entries: Vec::new(),
        },
        true,
    );
    viewer.placement = Some(Placement::new([12, 64, -5]));
    assert!(viewer.options.textured);
    viewer.menu_action("ccw");
    let placement = viewer.placement.unwrap();
    assert_eq!(placement.quarter_turns, 3);
    assert_eq!(placement.footprint(viewer.schematic.size()), [3, 4, 2]);
    assert!(viewer.dirty);
    viewer.menu_action("cw");
    assert_eq!(viewer.placement.unwrap().quarter_turns, 0);

    viewer.menu_action("mode");
    viewer.menu_action("next-layer");
    assert_eq!(viewer.layers(), LayerMode::Single(1));
    viewer.menu_action("mode");
    assert_eq!(viewer.layers(), LayerMode::Range { min: 0, max: 1 });
    viewer.menu_action("mode");
    assert_eq!(viewer.layers(), LayerMode::All);
}
#[test]
fn mirror_flips_and_restores_unless_the_guide_is_locked() {
    let mut viewer = reference_viewer();
    let turns = viewer.placement.unwrap().quarter_turns;
    viewer.menu_action("mirror");
    assert_ne!(viewer.placement.unwrap().mirror, Mirror::None);
    viewer.menu_action("mirror");
    assert_eq!(viewer.placement.unwrap().mirror, Mirror::None);
    assert_eq!(viewer.placement.unwrap().quarter_turns, turns);
    viewer.locked = true;
    viewer.menu_action("mirror");
    assert_eq!(viewer.placement.unwrap().mirror, Mirror::None);
}

#[test]
fn bridge_finds_a_ghost_one_block_off_the_crosshair() {
    let eye = [0.5, 64.5, 0.0];
    let forward = [0.0, 0.0, 1.0];
    let near = [1, 64, 3];
    let far = [5, 64, 1];
    assert_eq!(
        super::nearby_ghost(eye, forward, None, |pos| pos == near || pos == far),
        Some(near)
    );
    assert_eq!(
        super::nearby_ghost(eye, forward, None, |pos| pos == far),
        Some(far)
    );
    assert_eq!(
        super::nearby_ghost(eye, forward, None, |pos| pos == [6, 64, 1]
            || pos == [0, 64, -1]),
        None
    );
}

#[test]
fn bridge_uses_the_far_side_of_the_aimed_support_in_both_depth_directions() {
    for (eye, forward, ghost) in [
        ([0.5, 64.5, 0.5], [0.0, 0.0, 1.0], [0, 64, 4]),
        ([0.5, 64.5, 6.5], [0.0, 0.0, -1.0], [0, 64, 2]),
    ] {
        let selected = super::nearby_ghost(eye, forward, Some([0.5, 64.5, 3.5]), |pos| {
            pos == [0, 64, 2] || pos == [0, 64, 4]
        });
        assert_eq!(selected, Some(ghost));
    }
}

#[test]
fn numpad_nudges_follow_the_nearest_camera_heading() {
    use std::f32::consts::FRAC_PI_2;
    for (turn, forward, left) in [
        (0., [0, -1], [-1, 0]),
        (1., [-1, 0], [0, 1]),
        (2., [0, 1], [1, 0]),
        (3., [1, 0], [0, -1]),
    ] {
        assert_eq!(crate::view_step(turn * FRAC_PI_2, "ViewForward"), forward);
        assert_eq!(crate::view_step(turn * FRAC_PI_2, "ViewLeft"), left);
        assert_eq!(
            crate::view_step(turn * FRAC_PI_2, "ViewBack"),
            forward.map(|axis| -axis)
        );
        assert_eq!(
            crate::view_step(turn * FRAC_PI_2, "ViewRight"),
            left.map(|axis| -axis)
        );
    }
}

#[test]
fn a_bed_head_offers_nothing_while_its_foot_does() {
    use schematica_core::{BlockState, StateValue};
    let mut viewer = reference_viewer();
    viewer.schematic = Schematic::new("Bed", [2, 1, 1]);
    viewer.placement = Some(Placement::new([0; 3]));
    let bed = BlockState::new("minecraft:bed");
    viewer.schematic.set(
        [0, 0, 0],
        bed.clone().with("head_piece_bit", StateValue::Byte(0)),
    );
    viewer
        .schematic
        .set([1, 0, 0], bed.with("head_piece_bit", StateValue::Byte(1)));
    assert!(viewer.placement_block([0, 0, 0]).is_some());
    assert!(viewer.placement_block([1, 0, 0]).is_none());
}

#[test]
fn aiming_at_a_door_upper_half_uses_its_matching_lower_half() {
    use schematica_core::{BlockState, StateValue};
    let mut viewer = reference_viewer();
    viewer.schematic = Schematic::new("Door", [1, 2, 1]);
    viewer.placement = Some(Placement::new([0; 3]));
    let door = BlockState::new("minecraft:dark_oak_door");
    viewer.schematic.set(
        [0, 0, 0],
        door.clone().with("upper_block_bit", StateValue::Byte(0)),
    );
    viewer
        .schematic
        .set([0, 1, 0], door.with("upper_block_bit", StateValue::Byte(1)));
    let (block, cell) = viewer.placement_block([0, 1, 0]).unwrap();
    assert_eq!(cell, [0, 0, 0]);
    assert_eq!(block.states["upper_block_bit"], StateValue::Byte(0));
    viewer
        .schematic
        .set([0, 0, 0], BlockState::new("minecraft:stone"));
    assert!(viewer.placement_block([0, 1, 0]).is_none());
}

#[test]
fn material_page_counts_finish_over_multiple_frames() {
    let mut viewer = reference_viewer();
    viewer.tiles.resize_with(70, Tile::default);
    viewer.tiles[0].stamp = Some(terrain::Stamp {
        session: 1,
        dimension: 0,
        registry: 1,
        revision: 0,
    });
    // Thirty-two tiles are counted per call: three calls for seventy. Nothing is built, so
    // the first row (cobblestone, 49 in the schematic) has all 49 left, not 0.
    for pending in [true, true, false] {
        let mut bindings = serde_json::Map::new();
        browser::bindings(&viewer, &mut bindings, &mut Vec::new());
        assert_eq!(viewer.material_counts_pending(), pending);
        assert_eq!(bindings["#bw_row0_needed"], if pending { "…" } else { "49" });
    }
    // A finished page is remembered: asking again counts nothing.
    let mut bindings = serde_json::Map::new();
    browser::bindings(&viewer, &mut bindings, &mut Vec::new());
    assert_eq!(bindings["#bw_row0_needed"], "49");
    assert_eq!(bindings["#bw_row0_need"], true);
    assert_eq!(bindings["#bw_row0_done"], false);
    assert!(!viewer.material_counts_pending());
    assert_eq!(bindings["#bw_progress_label"], "0% built  ·  0 / 171");
}

#[test]
fn what_is_left_is_the_total_less_what_is_built_so_unchecked_cells_never_read_as_done() {
    let mut viewer = reference_viewer();
    viewer.tiles.resize_with(3, Tile::default);
    viewer.tiles[0].stamp = Some(terrain::Stamp {
        session: 1,
        dimension: 0,
        registry: 1,
        revision: 0,
    });
    // Only the first tile has been checked, and it holds 40 built blocks. The other two
    // tiles were never checked and add nothing to what is built.
    viewer.tiles[0].built = 40;
    let mut bindings = serde_json::Map::new();
    browser::bindings(&viewer, &mut bindings, &mut Vec::new());
    assert_eq!(bindings["#bw_progress_label"], "23% built  ·  40 / 171");
    // Placing or breaking a block changes what is built, so the page's counts start again.
    for _ in 0..4 {
        browser::bindings(&viewer, &mut bindings, &mut Vec::new());
    }
    assert!(!viewer.material_counts_pending());
    viewer.tiles[0].built = 41;
    let mut bindings = serde_json::Map::new();
    browser::bindings(&viewer, &mut bindings, &mut Vec::new());
    assert_eq!(bindings["#bw_progress_label"], "23% built  ·  41 / 171");
    assert_eq!(bindings["#bw_row0_needed"], "49");
}

#[test]
fn aiming_at_a_tall_plant_upper_half_places_the_matching_pair() {
    use schematica_core::{BlockState, StateValue};
    let mut viewer = reference_viewer();
    viewer.schematic = Schematic::new("Plant", [1, 2, 1]);
    viewer.placement = Some(Placement::new([0; 3]));
    let plant = BlockState::new("minecraft:sunflower");
    viewer.schematic.set(
        [0, 0, 0],
        plant.clone().with("upper_block_bit", StateValue::Byte(0)),
    );
    viewer.schematic.set(
        [0, 1, 0],
        plant.with("upper_block_bit", StateValue::Byte(1)),
    );
    let (block, pos) = viewer.placement_block([0, 1, 0]).unwrap();
    assert_eq!(pos, [0, 0, 0]);
    assert_eq!(block.states["upper_block_bit"], StateValue::Byte(0));
}

#[test]
fn ghost_lightness_loads_bounded_and_ignores_older_settings() {
    use serde_json::json;
    assert_eq!(DEFAULT_LIGHTNESS, 15);
    assert_eq!(
        saved_lightness(&json!({ "opacity": 90 })),
        DEFAULT_LIGHTNESS
    );
    assert_eq!(
        saved_lightness(&json!({ "lightness": 60 })),
        DEFAULT_LIGHTNESS
    );
    assert_eq!(saved_lightness(&json!({ "ghost_tint": 25 })), 25);
    assert_eq!(saved_lightness(&json!({ "ghost_tint": 99 })), 35);
    assert_eq!(saved_lightness(&json!({ "ghost_tint": 0 })), 5);
}

#[test]
fn liquids_have_no_item_and_a_powered_torch_is_offered_as_the_item_it_places() {
    use schematica_core::BlockState;
    for name in ["minecraft:water", "minecraft:flowing_water", "minecraft:lava", "minecraft:flowing_lava"] {
        assert!(unplaceable(name), "{name}");
    }
    for name in ["minecraft:stone", "minecraft:waxed_copper_bulb", "minecraft:redstone_torch"] {
        assert!(!unplaceable(name), "{name}");
    }
    // The unlit torch has no item; the redstone torch item places it and the world turns it off.
    let torch = unlit_variant(BlockState::new("minecraft:unlit_redstone_torch"));
    assert_eq!(torch.name, "minecraft:redstone_torch");
    assert_eq!(unlit_variant(BlockState::new("minecraft:stone")).name, "minecraft:stone");
}
