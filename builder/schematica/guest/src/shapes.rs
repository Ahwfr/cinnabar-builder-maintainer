//! The real selection shape of a planned block, so the aim ray passes through the empty part of
//! a thin block (a slab, trapdoor, door, pane, sign) instead of treating its whole cell as solid.
//!
//! These are the boxes the game itself uses for the common thin blocks. Where a block is not
//! listed, or a state is missing or unknown, the answer is `None` and the whole cell counts, as
//! before. Boxes are block-local: `[min x, min y, min z, max x, max y, max z]` in 0 to 1.

use schematica_core::{BlockState, StateValue};

pub(crate) type Shape = Vec<[f32; 6]>;

const FULL_HEIGHT: f32 = 1.0;

fn int(block: &BlockState, key: &str) -> Option<i32> {
    match block.states.get(key)? {
        StateValue::Bool(value) => Some(i32::from(*value)),
        StateValue::Byte(value) => Some(i32::from(*value)),
        StateValue::Int(value) => Some(*value),
        StateValue::Str(_) => None,
    }
}

fn flag(block: &BlockState, key: &str) -> Option<bool> {
    int(block, key).map(|value| value != 0)
}

fn text<'a>(block: &'a BlockState, key: &str) -> Option<&'a str> {
    match block.states.get(key)? {
        StateValue::Str(value) => Some(value.as_str()),
        _ => None,
    }
}

/// A slab of the given thickness against one side of the cell. Sides are 0 west, 1 east,
/// 2 north, 3 south, 4 down and 5 up.
fn plate(side: u8, thickness: f32) -> [f32; 6] {
    let far = 1. - thickness;
    match side {
        0 => [0., 0., 0., thickness, 1., 1.],
        1 => [far, 0., 0., 1., 1., 1.],
        2 => [0., 0., 0., 1., 1., thickness],
        3 => [0., 0., far, 1., 1., 1.],
        4 => [0., 0., 0., 1., thickness, 1.],
        _ => [0., far, 0., 1., 1., 1.],
    }
}

/// The side of the cell a wall-mounted block sits against, from its `facing_direction`: the
/// face the block looks out of is 2 north, 3 south, 4 west and 5 east, so it sits on the
/// opposite side.
fn mounted_side(facing: i32) -> Option<u8> {
    Some(match facing {
        2 => 3,
        3 => 2,
        4 => 1,
        5 => 0,
        _ => return None,
    })
}

/// The selection boxes of a planned block, or `None` when the whole cell should count.
pub(crate) fn selection(block: &BlockState) -> Option<Shape> {
    let name = block.name.strip_prefix("minecraft:").unwrap_or(&block.name);
    if name.ends_with("_slab") && !name.contains("double") {
        return match text(block, "minecraft:vertical_half")? {
            "bottom" => Some(vec![[0., 0., 0., 1., 0.5, 1.]]),
            "top" => Some(vec![[0., 0.5, 0., 1., 1., 1.]]),
            _ => None,
        };
    }
    if name.ends_with("trapdoor") {
        let open = flag(block, "open_bit")?;
        let down = flag(block, "upside_down_bit")?;
        let direction = int(block, "direction")?;
        let side = if open {
            u8::try_from(direction).ok().filter(|side| *side < 4)?
        } else if down {
            5
        } else {
            4
        };
        return Some(vec![plate(side, 0.1875)]);
    }
    if name.ends_with("_door") || name == "wooden_door" || name == "iron_door" {
        let open = flag(block, "open_bit")?;
        let mirrored = flag(block, "door_hinge_bit")?;
        // The closed side is south 0 west, west 2 north, north 1 east and east 3 south. Opening
        // swings the door a quarter turn: one way for a left hinge, the other for a right one.
        let closed = match text(block, "minecraft:cardinal_direction")? {
            "south" => 0,
            "west" => 2,
            "north" => 1,
            "east" => 3,
            _ => return None,
        };
        let side = if open {
            let turn = [(0, 2), (2, 1), (1, 3), (3, 0)];
            let (_, left) = turn.iter().copied().find(|(side, _)| *side == closed)?;
            let right = turn.iter().copied().find(|(_, to)| *to == closed)?.0;
            if mirrored { right } else { left }
        } else {
            closed
        };
        return Some(vec![plate(side, 0.1875)]);
    }
    if name.ends_with("fence_gate") {
        let along_x = matches!(text(block, "minecraft:cardinal_direction")?, "south" | "north");
        return Some(vec![if along_x {
            [0., 0., 0.375, 1., FULL_HEIGHT, 0.625]
        } else {
            [0.375, 0., 0., 0.625, FULL_HEIGHT, 1.]
        }]);
    }
    if name == "carpet" || name.ends_with("_carpet") {
        return Some(vec![[0., 0., 0., 1., 0.0625, 1.]]);
    }
    if name.contains("pressure_plate") {
        return Some(vec![[0.0625, 0., 0.0625, 0.9375, 0.0625, 0.9375]]);
    }
    if name == "snow_layer" {
        let height = int(block, "height")?;
        return Some(vec![[0., 0., 0., 1., (height + 1).min(8) as f32 / 8., 1.]]);
    }
    if name == "ladder" {
        return Some(vec![plate(mounted_side(int(block, "facing_direction")?)?, 0.1875)]);
    }
    if name.ends_with("wall_sign") {
        let side = mounted_side(int(block, "facing_direction")?)?;
        let [x0, _, z0, x1, _, z1] = plate(side, 0.125);
        return Some(vec![[x0, 0.28, z0, x1, 0.78, z1]]);
    }
    if name.ends_with("standing_sign") {
        return Some(vec![[0.25, 0., 0.25, 0.75, 1., 0.75]]);
    }
    if name.ends_with("_button") {
        let side = match int(block, "facing_direction")? {
            0 => 5,
            1 => 4,
            other => mounted_side(other)?,
        };
        let [x0, y0, z0, x1, y1, z1] = plate(side, 0.125);
        let narrow = |low: f32, high: f32, edge: f32| {
            if high - low < 1. { (low, high) } else { (edge, 1. - edge) }
        };
        let (x0, x1) = narrow(x0, x1, 0.3125);
        let (y0, y1) = narrow(y0, y1, 0.375);
        let (z0, z1) = narrow(z0, z1, 0.3125);
        return Some(vec![[x0, y0, z0, x1, y1, z1]]);
    }
    if name == "torch" || name.ends_with("_torch") {
        let floor = matches!(text(block, "torch_facing_direction"), Some("top" | "unknown"));
        return Some(vec![if floor {
            [0.375, 0., 0.375, 0.625, 0.625, 0.625]
        } else {
            [0.25, 0.2, 0.25, 0.75, 0.9, 0.75]
        }]);
    }
    if name == "rail" || name.ends_with("_rail") {
        // Only the flat shapes are known. A rail that climbs counts as the whole cell.
        let direction = int(block, "rail_direction")?;
        let flat = matches!(direction, 0 | 1) || (name == "rail" && direction >= 6);
        return flat.then(|| vec![[0., 0., 0., 1., 0.125, 1.]]);
    }
    if name == "redstone_wire" {
        return Some(vec![[0., 0., 0., 1., 0.0625, 1.]]);
    }
    if name.contains("repeater") || name.contains("comparator") {
        return Some(vec![[0., 0., 0., 1., 0.125, 1.]]);
    }
    if name == "chest" || name == "trapped_chest" || name == "ender_chest" || name.ends_with("copper_chest") {
        return Some(vec![[0.0625, 0., 0.0625, 0.9375, 0.875, 0.9375]]);
    }
    if name == "bed" {
        return Some(vec![[0., 0., 0., 1., 0.5625, 1.]]);
    }
    if name == "lantern" || name == "soul_lantern" {
        return Some(vec![if flag(block, "hanging")? {
            [0.3125, 0.125, 0.3125, 0.6875, 0.625, 0.6875]
        } else {
            [0.3125, 0., 0.3125, 0.6875, 0.5, 0.6875]
        }]);
    }
    if name.ends_with("_wall") && !name.contains("sign") && !name.contains("banner") {
        return wall(block);
    }
    if name.ends_with("fence") {
        return Some(arms(block, 0.375, 0.625));
    }
    if name.ends_with("glass_pane") || name == "iron_bars" || name.ends_with("_pane") {
        return Some(arms(block, 0.4375, 0.5625));
    }
    None
}

/// The box to outline for a ghost rail or chest: one box around the whole block. A rail that
/// climbs has no exact shape here, so it gets a rough box half a block high.
pub(crate) fn outline(block: &BlockState) -> Option<[f32; 6]> {
    let name = block.name.strip_prefix("minecraft:").unwrap_or(&block.name);
    let rail = name == "rail" || name.ends_with("_rail");
    let chest = name == "chest" || name == "trapped_chest" || name == "ender_chest" || name.ends_with("copper_chest");
    if !rail && !chest {
        return None;
    }
    match selection(block) {
        Some(shape) => shape.first().copied(),
        None if rail => Some([0., 0., 0., 1., 0.5, 1.]),
        None => None,
    }
}

/// A post with an arm toward each side that connects. A block saved without any connection
/// counts as connected on every side, because a planned fence or pane often carries none.
fn arms(block: &BlockState, low: f32, high: f32) -> Shape {
    let connected = [
        flag(block, "connection_north"),
        flag(block, "connection_east"),
        flag(block, "connection_south"),
        flag(block, "connection_west"),
    ];
    let any = connected.iter().any(|side| *side == Some(true));
    let side = |index: usize| !any || connected[index] == Some(true);
    let mut boxes = vec![[low, 0., low, high, FULL_HEIGHT, high]];
    if side(0) {
        boxes.push([low, 0., 0., high, FULL_HEIGHT, 0.5]);
    }
    if side(1) {
        boxes.push([0.5, 0., low, 1., FULL_HEIGHT, high]);
    }
    if side(2) {
        boxes.push([low, 0., 0.5, high, FULL_HEIGHT, 1.]);
    }
    if side(3) {
        boxes.push([0., 0., low, 0.5, FULL_HEIGHT, high]);
    }
    boxes
}

/// A wall's post and the arms it connects to.
fn wall(block: &BlockState) -> Option<Shape> {
    let kinds = [
        text(block, "wall_connection_type_north"),
        text(block, "wall_connection_type_east"),
        text(block, "wall_connection_type_south"),
        text(block, "wall_connection_type_west"),
    ];
    let any = kinds.iter().any(|kind| matches!(kind, Some("short" | "tall")));
    let side = |index: usize| !any || matches!(kinds[index], Some("short" | "tall"));
    let mut boxes = vec![[0.25, 0., 0.25, 0.75, FULL_HEIGHT, 0.75]];
    let (low, high) = (0.3125, 0.6875);
    if side(0) {
        boxes.push([low, 0., 0., high, FULL_HEIGHT, 0.5]);
    }
    if side(1) {
        boxes.push([0.5, 0., low, 1., FULL_HEIGHT, high]);
    }
    if side(2) {
        boxes.push([low, 0., 0.5, high, FULL_HEIGHT, 1.]);
    }
    if side(3) {
        boxes.push([0., 0., low, 0.5, FULL_HEIGHT, high]);
    }
    Some(boxes)
}

/// Whether the ray from `eye` along `forward` passes through any of the boxes of the cell
/// within `reach` blocks.
pub(crate) fn ray_hits(eye: [f32; 3], forward: [f32; 3], cell: [i32; 3], shape: &Shape, reach: f32) -> bool {
    shape.iter().any(|b| {
        let mut near = 0f32;
        let mut far = reach;
        for axis in 0..3 {
            let low = cell[axis] as f32 + b[axis];
            let high = cell[axis] as f32 + b[axis + 3];
            if forward[axis].abs() < 1e-6 {
                if eye[axis] < low || eye[axis] > high {
                    return false;
                }
                continue;
            }
            let (a, c) = ((low - eye[axis]) / forward[axis], (high - eye[axis]) / forward[axis]);
            near = near.max(a.min(c));
            far = far.min(a.max(c));
            if near > far {
                return false;
            }
        }
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(name: &str, states: &[(&str, StateValue)]) -> BlockState {
        states
            .iter()
            .fold(BlockState::new(name), |block, (key, value)| block.with(key, value.clone()))
    }

    #[test]
    fn a_bottom_slab_is_the_lower_half_and_a_ray_over_it_passes() {
        let slab = block(
            "minecraft:stone_block_slab",
            &[("minecraft:vertical_half", StateValue::Str("bottom".into()))],
        );
        let shape = selection(&slab).unwrap();
        assert_eq!(shape, vec![[0., 0., 0., 1., 0.5, 1.]]);
        // Flying through the upper half of the cell misses; through the lower half hits.
        assert!(!ray_hits([0.5, 0.8, -2.5], [0., 0., 1.], [0, 0, 0], &shape, 6.));
        assert!(ray_hits([0.5, 0.3, -2.5], [0., 0., 1.], [0, 0, 0], &shape, 6.));
    }

    #[test]
    fn doors_and_trapdoors_match_the_game_boxes() {
        // Closed south-facing door, left hinge: the west side. Opened it swings to the north.
        let closed = block(
            "minecraft:wooden_door",
            &[
                ("door_hinge_bit", StateValue::Byte(0)),
                ("open_bit", StateValue::Byte(0)),
                ("minecraft:cardinal_direction", StateValue::Str("south".into())),
            ],
        );
        assert_eq!(selection(&closed).unwrap(), vec![[0., 0., 0., 0.1875, 1., 1.]]);
        let opened = block(
            "minecraft:wooden_door",
            &[
                ("door_hinge_bit", StateValue::Byte(0)),
                ("open_bit", StateValue::Byte(1)),
                ("minecraft:cardinal_direction", StateValue::Str("south".into())),
            ],
        );
        assert_eq!(selection(&opened).unwrap(), vec![[0., 0., 0., 1., 1., 0.1875]]);
        let mirrored = block(
            "minecraft:wooden_door",
            &[
                ("door_hinge_bit", StateValue::Byte(1)),
                ("open_bit", StateValue::Byte(1)),
                ("minecraft:cardinal_direction", StateValue::Str("south".into())),
            ],
        );
        assert_eq!(selection(&mirrored).unwrap(), vec![[0., 0., 0.8125, 1., 1., 1.]]);
        let trapdoor = |direction: i32, open: i8, down: i8| {
            selection(&block(
                "minecraft:trapdoor",
                &[
                    ("direction", StateValue::Int(direction)),
                    ("open_bit", StateValue::Byte(open)),
                    ("upside_down_bit", StateValue::Byte(down)),
                ],
            ))
            .unwrap()
        };
        assert_eq!(trapdoor(0, 0, 0), vec![[0., 0., 0., 1., 0.1875, 1.]]);
        assert_eq!(trapdoor(0, 0, 1), vec![[0., 0.8125, 0., 1., 1., 1.]]);
        assert_eq!(trapdoor(0, 1, 0), vec![[0., 0., 0., 0.1875, 1., 1.]]);
        assert_eq!(trapdoor(1, 1, 0), vec![[0.8125, 0., 0., 1., 1., 1.]]);
        assert_eq!(trapdoor(2, 1, 0), vec![[0., 0., 0., 1., 1., 0.1875]]);
        assert_eq!(trapdoor(3, 1, 0), vec![[0., 0., 0.8125, 1., 1., 1.]]);
    }

    #[test]
    fn ladders_signs_and_panes_leave_room_to_aim_past_them() {
        let ladder = block("minecraft:ladder", &[("facing_direction", StateValue::Int(2))]);
        assert_eq!(selection(&ladder).unwrap(), vec![[0., 0., 0.8125, 1., 1., 1.]]);
        let sign = block("minecraft:wall_sign", &[("facing_direction", StateValue::Int(4))]);
        assert_eq!(selection(&sign).unwrap(), vec![[0.875, 0.28, 0., 1., 0.78, 1.]]);
        let pane = block(
            "minecraft:glass_pane",
            &[
                ("connection_north", StateValue::Byte(0)),
                ("connection_south", StateValue::Byte(0)),
                ("connection_east", StateValue::Byte(1)),
                ("connection_west", StateValue::Byte(1)),
            ],
        );
        let shape = selection(&pane).unwrap();
        // A pane running east to west is thin north to south: a ray offset from its middle passes.
        assert!(!ray_hits([-2.5, 0.5, 0.2], [1., 0., 0.], [0, 0, 0], &shape, 6.));
        assert!(ray_hits([-2.5, 0.5, 0.5], [1., 0., 0.], [0, 0, 0], &shape, 6.));
        // A pane saved without connections is treated as connected all round.
        let bare = selection(&block("minecraft:glass_pane", &[])).unwrap();
        assert_eq!(bare.len(), 5);
    }

    #[test]
    fn only_rails_and_chests_get_an_outline_box() {
        let flat = block("minecraft:golden_rail", &[("rail_direction", StateValue::Int(1))]);
        assert_eq!(outline(&flat), Some([0., 0., 0., 1., 0.125, 1.]));
        let climbing = block("minecraft:rail", &[("rail_direction", StateValue::Int(3))]);
        assert_eq!(outline(&climbing), Some([0., 0., 0., 1., 0.5, 1.]));
        assert_eq!(
            outline(&BlockState::new("minecraft:chest")),
            Some([0.0625, 0., 0.0625, 0.9375, 0.875, 0.9375])
        );
        assert!(outline(&BlockState::new("minecraft:stone")).is_none());
    }

    #[test]
    fn unknown_blocks_and_missing_states_keep_the_whole_cell() {
        assert!(selection(&BlockState::new("minecraft:stone")).is_none());
        assert!(selection(&BlockState::new("minecraft:oak_stairs")).is_none());
        // A slab with no half is not guessed.
        assert!(selection(&BlockState::new("minecraft:stone_block_slab")).is_none());
        // A climbing rail is not known, a flat one is.
        let climbing = block("minecraft:rail", &[("rail_direction", StateValue::Int(3))]);
        assert!(selection(&climbing).is_none());
        let flat = block("minecraft:rail", &[("rail_direction", StateValue::Int(0))]);
        assert!(selection(&flat).is_some());
    }
}
