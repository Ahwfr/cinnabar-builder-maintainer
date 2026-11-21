use mod_host::CreativePlacementOffer;

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

/// A guide recording world-set state (growth, open, power, rail shape) matches the plain block a
/// placement produces, while a wrong facing still does not.
#[test]
fn world_controlled_states_do_not_block_fast_place_but_orientation_still_does() {
    use crate::block_use::orientation::strict_state_matches;
    let wheat = offer(
        "minecraft:wheat",
        serde_json::json!({"growth": {"type":"int","value":7}}),
    );
    assert!(strict_state_matches(
        &wheat,
        r#"{"growth":{"type":"int","value":0}}"#
    ));

    let door = offer(
        "minecraft:oak_door",
        serde_json::json!({
            "minecraft:cardinal_direction": {"type":"string","value":"north"},
            "open_bit": {"type":"byte","value":1},
            "upper_block_bit": {"type":"byte","value":0},
        }),
    );
    let placed = r#"{"minecraft:cardinal_direction":{"type":"string","value":"north"},"open_bit":{"type":"byte","value":0},"upper_block_bit":{"type":"byte","value":0},"door_hinge_bit":{"type":"byte","value":0}}"#;
    assert!(strict_state_matches(&door, placed));
    let wrong_way = placed.replace("north", "south");
    assert!(!strict_state_matches(&door, &wrong_way));

    let rail = offer(
        "minecraft:golden_rail",
        serde_json::json!({
            "rail_direction": {"type":"int","value":5},
            "rail_data_bit": {"type":"byte","value":1},
        }),
    );
    assert!(strict_state_matches(
        &rail,
        r#"{"rail_direction":{"type":"int","value":0},"rail_data_bit":{"type":"byte","value":0}}"#
    ));

    let stairs = offer(
        "minecraft:oak_stairs",
        serde_json::json!({
            "weirdo_direction": {"type":"int","value":2},
            "upside_down_bit": {"type":"byte","value":1},
        }),
    );
    assert!(!strict_state_matches(
        &stairs,
        r#"{"weirdo_direction":{"type":"int","value":2},"upside_down_bit":{"type":"byte","value":0}}"#
    ));
}

/// A hopper keeps whatever facing the click gives it.
#[test]
fn hoppers_ignore_the_planned_facing() {
    use crate::block_use::orientation::{air_place_face, strict_state_matches};
    let hopper = offer(
        "minecraft:hopper",
        serde_json::json!({"facing_direction": {"type":"int","value":3}, "toggle_bit": {"type":"byte","value":0}}),
    );
    assert!(strict_state_matches(
        &hopper,
        r#"{"facing_direction":{"type":"int","value":0},"toggle_bit":{"type":"byte","value":0}}"#
    ));
    assert_eq!(air_place_face(&hopper), Some(1));
}

/// A ceiling lever is placed only by clicking the underside of the block above; floor levers
/// keep their current faces.
#[test]
fn ceiling_levers_need_the_underside_of_the_block_above() {
    use crate::block_use::orientation::{air_place_face, support_produces_axis, wanted_face};
    let lever = |direction: &str| {
        offer(
            "minecraft:lever",
            serde_json::json!({"lever_direction": {"type":"string","value":direction}, "open_bit": {"type":"byte","value":0}}),
        )
    };
    let ceiling = lever("down_east_west");
    assert_eq!(wanted_face(&ceiling), Some(0));
    assert!(support_produces_axis(&ceiling, 0));
    assert!(!support_produces_axis(&ceiling, 1));
    assert_eq!(air_place_face(&ceiling), None);
    let floor = lever("up_north_south");
    assert_eq!(wanted_face(&floor), None);
    assert_eq!(air_place_face(&floor), Some(1));
}

/// Wall-mounted signs, banners, heads and coral fans need a side face; their floor forms need
/// the top, and a hanging sign hangs under a block.
#[test]
fn wall_and_floor_forms_need_their_own_support_face() {
    use crate::block_use::orientation::support_produces_axis;
    let wall_sign = offer(
        "minecraft:darkoak_wall_sign",
        serde_json::json!({"facing_direction": {"type":"int","value":3}}),
    );
    assert!(support_produces_axis(&wall_sign, 3));
    assert!(!support_produces_axis(&wall_sign, 1));
    let banner = offer(
        "minecraft:standing_banner",
        serde_json::json!({"ground_sign_direction": {"type":"int","value":4}}),
    );
    assert!(support_produces_axis(&banner, 1));
    assert!(!support_produces_axis(&banner, 2));
    let fan = offer(
        "minecraft:tube_coral_wall_fan",
        serde_json::json!({"coral_direction": {"type":"int","value":1}, "dead_bit": {"type":"byte","value":0}}),
    );
    assert!(support_produces_axis(&fan, 4));
    assert!(!support_produces_axis(&fan, 1));
    let skull = offer(
        "minecraft:skeleton_skull",
        serde_json::json!({"facing_direction": {"type":"int","value":1}}),
    );
    assert!(support_produces_axis(&skull, 1));
    assert!(!support_produces_axis(&skull, 2));
    let hanging = offer(
        "minecraft:oak_hanging_sign",
        serde_json::json!({"hanging": {"type":"byte","value":1}, "ground_sign_direction": {"type":"int","value":0}}),
    );
    assert!(support_produces_axis(&hanging, 0));
    assert!(!support_produces_axis(&hanging, 1));
}
