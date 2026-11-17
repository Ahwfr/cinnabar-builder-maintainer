use super::*;

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
        players: Vec::new(),
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
            structure_paste: false,
            remember_placement: false,
            saved_placements: BTreeMap::new(),
            bridge_place: true,
            opacity: 65,
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
            structure_paste: false,
            remember_placement: false,
            saved_placements: BTreeMap::new(),
            bridge_place: true,
            opacity: 65,
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
    viewer.tiles.resize_with(10, Tile::default);
    viewer.tiles[0].stamp = Some(terrain::Stamp {
        session: 1,
        dimension: 0,
        registry: 1,
        revision: 0,
    });
    for pending in [true, true, false] {
        let mut bindings = serde_json::Map::new();
        browser::bindings(&viewer, &mut bindings, &mut Vec::new());
        assert_eq!(viewer.material_counts_pending(), pending);
        assert_eq!(bindings["#bw_row0_needed"], if pending { "…" } else { "0" });
    }
}
