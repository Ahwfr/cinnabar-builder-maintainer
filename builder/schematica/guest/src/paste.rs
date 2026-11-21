//! Explicit operator paste through Bedrock's advertised editing commands.

use crate::{Viewer, gameplay, structure_paste::StructurePaste};
use schematica_core::{BlockState, Placement, Schematic, StateValue};

mod repair;
pub(crate) use repair::Repair;

const COMMAND_BYTES: usize = 440;

pub(super) struct Paste {
    scope: (u64, i32),
    /// The placement the player confirmed; a change to it cancels the whole run.
    base: Placement,
    /// The copy being pasted now. Equal to `base` unless a grid paste is under way.
    placement: Placement,
    /// Grid copies still to paste after this one; empty for an ordinary paste.
    queue: Vec<Placement>,
    destroy: bool,
    structure: StructurePaste,
    /// Grid copies already sent, kept so the check afterwards covers all of them.
    finished: Vec<Placement>,
}

/// An export under way. It runs beside everything else: it sends no blocks to the world, so
/// the guide stays usable and Fast Place keeps working while it uploads.
pub(super) struct ExportRun {
    scope: (u64, i32),
    /// The placement the export was confirmed for; a change to it cancels the export.
    base: Placement,
    structure: StructurePaste,
    /// The parts still to send after the current one, and the names of all of them.
    queue: Vec<StructurePaste>,
    names: Vec<String>,
    /// What the Controls tab shows while it runs.
    pub(super) progress: String,
}

impl Viewer {
    pub(super) fn paste_button(&mut self) {
        if self.repair.take().is_some() {
            self.paste_confirm = false;
            let _ = crate::panel::close();
            self.paste_result("Paste check canceled | Missing blocks were not placed".into());
            return;
        }
        if let Some(run) = self.paste.take() {
            self.cancel_structure = Some(run.scope);
            self.paste_confirm = false;
            let _ = crate::panel::close();
            self.paste_result("Paste canceled | Completed blocks were kept".into());
            return;
        }
        if self.export_run.is_some() {
            self.paste_notice = Some(("Paste | Wait for the export to finish".into(), 3.0));
            return;
        }
        let Some((scope, placement)) = self.world.zip(self.placement) else {
            return;
        };
        if !std::mem::take(&mut self.paste_confirm) {
            self.paste_confirm = true;
            self.export_confirming = false;
            return;
        }
        self.paste = Some(Paste {
            scope,
            base: placement,
            placement,
            queue: Vec::new(),
            destroy: self.options.paste_destroy,
            structure: StructurePaste::new(&self.schematic, placement, self.options.paste_destroy),
            finished: Vec::new(),
        });
        let _ = crate::panel::close();
    }

    /// Saves the guide as structures in the world, for a structure block's Load mode. A first
    /// click asks to confirm; the second starts the upload, which runs in the background.
    pub(super) fn export_button(&mut self) {
        if let Some(run) = self.export_run.take() {
            self.cancel_structure = Some(run.scope);
            self.paste_confirm = false;
            let _ = crate::panel::close();
            self.paste_result("Export canceled | Nothing was saved".into());
            return;
        }
        if self.paste.is_some() || self.repair.is_some() {
            self.paste_notice = Some(("Export | Wait for the paste to finish".into(), 3.0));
            return;
        }
        let Some((scope, placement)) = self.world.zip(self.placement) else {
            return;
        };
        if !std::mem::take(&mut self.paste_confirm) || !self.export_confirming {
            self.paste_confirm = true;
            self.export_confirming = true;
            return;
        }
        self.export_confirming = false;
        let (min, max) = placement.world_bounds(self.schematic.size());
        let base = structure_name(&self.schematic.name);
        let parts = export_parts(min, max, &base);
        let names: Vec<String> = parts.iter().map(|(name, ..)| name.clone()).collect();
        let mut queue: Vec<StructurePaste> = parts
            .into_iter()
            .map(|(name, lo, hi)| StructurePaste::saved(lo, hi, name))
            .collect();
        queue.reverse();
        let Some(first) = queue.pop() else {
            return;
        };
        self.export_run = Some(ExportRun {
            scope,
            base: placement,
            structure: first,
            queue,
            names,
            progress: "Starting".into(),
        });
        let _ = crate::panel::close();
        self.paste_notice = Some(("Exporting | You can keep building while it uploads".into(), 5.0));
    }

    /// Sends the next piece of the export, if one is under way. Runs every frame beside the
    /// rest of the guide and never blocks it.
    pub(super) fn export_step(&mut self) {
        let Some(mut run) = self.export_run.take() else {
            return;
        };
        if self.world != Some(run.scope) || self.placement != Some(run.base) {
            self.cancel_structure = Some(run.scope);
            self.paste_result("Export canceled because the world or placement changed".into());
            return;
        }
        if !gameplay::can_paste_blocks() {
            self.paste_result(
                "Export needs Creative, operator cheats, and setblock and fill mod permissions"
                    .into(),
            );
            return;
        }
        match run.structure.step(&self.schematic, run.base) {
            Ok(Some(progress)) => run.progress = progress,
            Ok(None) => {
                if let Some(next) = run.queue.pop() {
                    run.structure = next;
                    run.progress = "Next part".into();
                } else {
                    self.paste_result(export_message(&run.names));
                    return;
                }
            }
            Err(error) => {
                self.paste_result(format!("Structure export stopped | {error}"));
                return;
            }
        }
        self.export_run = Some(run);
    }

    /// Starts a paste of `grid` copies side by side; the first copy is the placement itself.
    pub(super) fn paste_grid(&mut self, grid: [i32; 2]) {
        let Some((scope, placement)) = self.world.zip(self.placement) else {
            return;
        };
        if self.paste.is_some() || self.export_run.is_some() {
            return;
        }
        let footprint = placement.footprint(self.schematic.size());
        let mut queue: Vec<Placement> =
            crate::tool::grid_origins(placement.origin, footprint, grid)
                .into_iter()
                .map(|origin| Placement {
                    origin,
                    ..placement
                })
                .collect();
        queue.remove(0);
        queue.reverse();
        // The copies share one host session: the first opens it and the last closes it.
        let structure = StructurePaste::new(&self.schematic, placement, self.options.paste_destroy)
            .session(true, queue.is_empty());
        self.paste = Some(Paste {
            scope,
            base: placement,
            placement,
            queue,
            destroy: self.options.paste_destroy,
            structure,
            finished: Vec::new(),
        });
    }

    /// Reports a finished paste and starts the check for blocks the server did not keep.
    fn finish_paste(&mut self, run: Paste, message: String) {
        self.paste_result(message);
        self.repair = Some(Repair::new(&run, self.schematic.size()));
    }

    fn paste_result(&mut self, message: String) {
        self.show_label(&message);
        self.paste_notice = Some((message, 6.0));
    }

    /// Hides the ghosts of the cells in `lo..hi` that a paste has just sent to the server.
    fn expect_pasted(&mut self, lo: [i32; 3], hi: [i32; 3]) {
        for x in lo[0]..hi[0] {
            for z in lo[2]..hi[2] {
                for y in lo[1]..hi[1] {
                    if crate::budget::remaining_fuel() < 400_000 {
                        return;
                    }
                    self.whole
                        .expect_placed(&self.preview, &self.schematic, [x, y, z]);
                }
            }
        }
    }

    pub(super) fn paste_step(&mut self) -> bool {
        let Some(mut run) = self.paste.take() else {
            return false;
        };
        if self.world != Some(run.scope) || self.placement != Some(run.base) {
            self.paste_result("Paste canceled because the world or placement changed".into());
            return true;
        }
        if !gameplay::can_paste_blocks() {
            self.paste_result(
                "Paste needs Creative, operator cheats, and setblock and fill mod permissions"
                    .into(),
            );
            return true;
        }
        let result = run.structure.step(&self.schematic, run.placement);
        if let Some((lo, hi)) = run.structure.take_sent_box() {
            self.expect_pasted(lo, hi);
        }
        match result {
            Ok(Some(progress)) => {
                self.show_label(&progress);
                self.paste = Some(run);
            }
            Ok(None) if run.advance() => {
                // The copies share the host session: the last one closes it.
                run.structure = StructurePaste::new(&self.schematic, run.placement, run.destroy)
                    .session(false, run.queue.is_empty());
                self.show_label("Grid paste | Next copy");
                self.paste = Some(run);
            }
            Ok(None) => self.finish_paste(
                run,
                "Structure sent | The host reports placement in chat. Checking for missing blocks"
                    .into(),
            ),
            Err(error) => self.paste_result(format!("Structure paste stopped | {error}")),
        }
        true
    }
}

impl Paste {
    /// Moves on to the next grid copy, keeping the totals. False when none is left.
    fn advance(&mut self) -> bool {
        let Some(next) = self.queue.pop() else {
            return false;
        };
        self.finished.push(self.placement);
        self.placement = next;
        true
    }
}

/// The name a structure block types to load this schematic: lowercase letters, digits and
/// underscores, from the schematic's own name without its file extension.
pub(crate) fn structure_name(schematic_name: &str) -> String {
    let stem = schematic_name
        .rsplit_once('.')
        .filter(|(_, extension)| {
            ["mcstructure", "nbt", "litematic", "schem", "schematic", "gz"]
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
        .map_or(schematic_name, |(stem, _)| stem);
    let mut name = String::new();
    for character in stem.chars() {
        let character = character.to_ascii_lowercase();
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            name.push(character);
        } else if !name.ends_with('_') && !name.is_empty() {
            name.push('_');
        }
    }
    let name = name.trim_end_matches('_');
    let name: String = name.chars().take(24).collect();
    let name = name.trim_end_matches('_');
    if name.is_empty() { "schematic".into() } else { name.into() }
}

/// Structure blocks load at most 64 blocks along x and z, and 384 up. A larger schematic is
/// cut into parts named `base_x_z` (and `base_x_y_z` when it is also tall), counted from the
/// minimum corner.
pub(crate) const PART_EDGE: i32 = 64;
const PART_HEIGHT: i32 = 384;

pub(crate) fn export_parts(
    min: [i32; 3],
    max: [i32; 3],
    base: &str,
) -> Vec<(String, [i32; 3], [i32; 3])> {
    let counts: [i32; 3] = std::array::from_fn(|axis| {
        let edge = if axis == 1 { PART_HEIGHT } else { PART_EDGE };
        (max[axis] - min[axis] + edge - 1) / edge
    });
    let mut parts = Vec::new();
    for x in 0..counts[0] {
        for y in 0..counts[1] {
            for z in 0..counts[2] {
                let index = [x, y, z];
                let lo: [i32; 3] = std::array::from_fn(|axis| {
                    min[axis] + index[axis] * if axis == 1 { PART_HEIGHT } else { PART_EDGE }
                });
                let hi: [i32; 3] = std::array::from_fn(|axis| {
                    (lo[axis] + if axis == 1 { PART_HEIGHT } else { PART_EDGE }).min(max[axis])
                });
                let name = if counts == [1, 1, 1] {
                    base.to_string()
                } else if counts[1] == 1 {
                    format!("{base}_{x}_{z}")
                } else {
                    format!("{base}_{x}_{y}_{z}")
                };
                parts.push((name, lo, hi));
            }
        }
    }
    parts
}

fn export_message(names: &[String]) -> String {
    match names {
        [only] => format!(
            "Exported | In a structure block choose Load and type {only}"
        ),
        [first, .., last] => format!(
            "Exported {} parts | Load {first} to {last}, each {PART_EDGE} wide, the number counts east then south"
        , names.len()),
        [] => "Exported nothing".into(),
    }
}

pub(crate) fn coordinates(index: usize, width: usize, depth: usize) -> [i32; 3] {
    [
        (index % width) as i32,
        (index / (width * depth)) as i32,
        ((index / width) % depth) as i32,
    ]
}

pub(crate) fn command(
    schematic: &Schematic,
    placement: Placement,
    local: [i32; 3],
    index: u32,
    state: &str,
    destroy: bool,
) -> (Option<String>, usize) {
    let [x, y, z] = local;
    let size = schematic.size();
    let start = placement.local_to_world(size, local);
    if destroy {
        let [x, y, z] = start;
        let block = format!("{x} {y} {z} {state}");
        let edit = format!("/execute unless block {block} run setblock {block} replace");
        return ((edit.len() <= COMMAND_BYTES).then_some(edit), 1);
    }
    let mode = "keep";
    let mut end = x;
    while end + 1 < size[0] && schematic.palette_index([end + 1, y, z]) == Some(index) {
        end += 1;
    }
    if end > x {
        let tail = placement.local_to_world(size, [end, y, z]);
        let [ax, ay, az] = std::array::from_fn(|axis| start[axis].min(tail[axis]));
        let [bx, by, bz] = std::array::from_fn(|axis| start[axis].max(tail[axis]));
        let fill = format!("/fill {ax} {ay} {az} {bx} {by} {bz} {state} {mode}");
        if fill.len() <= COMMAND_BYTES {
            return (Some(fill), (end - x + 1) as usize);
        }
    }
    let [x, y, z] = start;
    let setblock = format!("/setblock {x} {y} {z} {state} {mode}");
    ((setblock.len() <= COMMAND_BYTES).then_some(setblock), 1)
}

pub(super) fn placement_property(name: &str) -> bool {
    !name.starts_with("minecraft:connection_")
        && name != "minecraft:corner"
        && !name.starts_with("wall_connection_type_")
        && name != "wall_post_bit"
}

pub(crate) fn format_state(block: &BlockState) -> Option<String> {
    let safe = |text: &str| {
        !text.is_empty()
            && text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_:+.-".contains(&b))
    };
    if !block.name.starts_with("minecraft:") || !safe(&block.name) {
        return None;
    }
    let mut properties = Vec::new();
    for (name, value) in &block.states {
        if !placement_property(name) {
            continue;
        }
        if !safe(name) {
            return None;
        }
        let value = match value {
            StateValue::Str(value) if safe(value) => format!("\"{value}\""),
            StateValue::Str(_) => return None,
            StateValue::Byte(0) => "false".into(),
            StateValue::Byte(1) => "true".into(),
            value => value.to_string(),
        };
        properties.push(format!("\"{name}\"={value}"));
    }
    Some(if properties.is_empty() {
        block.name.clone()
    } else {
        format!("{} [{}]", block.name, properties.join(","))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_preserve_halves_and_keep_existing_blocks() {
        let block = BlockState::new("minecraft:stone_stairs")
            .with("upside_down_bit", StateValue::Byte(1))
            .with("weirdo_direction", StateValue::Int(2));
        assert_eq!(
            format_state(&block).unwrap(),
            "minecraft:stone_stairs [\"upside_down_bit\"=true,\"weirdo_direction\"=2]"
        );
        assert!(format_state(&BlockState::new("minecraft:stone\n/fill")).is_none());
    }

    #[test]
    fn structure_names_are_lowercase_words_a_structure_block_accepts() {
        assert_eq!(structure_name("My House (v2).litematic"), "my_house_v2");
        assert_eq!(structure_name("castle.mcstructure"), "castle");
        assert_eq!(structure_name("***.schem"), "schematic");
        assert_eq!(structure_name(""), "schematic");
        let long = structure_name("a_very_long_schematic_name_that_goes_on_and_on.nbt");
        assert!(long.len() <= 24 && !long.ends_with('_'), "{long}");
        assert!(long.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'));
    }

    #[test]
    fn small_schematics_export_as_one_structure_and_large_ones_are_cut_at_64() {
        assert_eq!(
            export_parts([10, 64, -5], [30, 80, 20], "hut"),
            vec![("hut".to_string(), [10, 64, -5], [30, 80, 20])]
        );
        let parts = export_parts([0, 0, 0], [130, 50, 64], "wall");
        let names: Vec<_> = parts.iter().map(|part| part.0.as_str()).collect();
        assert_eq!(names, ["wall_0_0", "wall_1_0", "wall_2_0"]);
        assert_eq!(parts[0].1, [0, 0, 0]);
        assert_eq!(parts[0].2, [64, 50, 64]);
        assert_eq!(parts[2].1, [128, 0, 0]);
        assert_eq!(parts[2].2, [130, 50, 64]);
        // Every cell belongs to exactly one part.
        let volume: i32 = parts
            .iter()
            .map(|(_, lo, hi)| (hi[0] - lo[0]) * (hi[1] - lo[1]) * (hi[2] - lo[2]))
            .sum();
        assert_eq!(volume, 130 * 50 * 64);
        let tall = export_parts([0, -64, 0], [10, 400, 10], "tower");
        assert_eq!(tall.iter().map(|part| part.0.as_str()).collect::<Vec<_>>(), ["tower_0_0_0", "tower_0_1_0"]);
        assert!(tall.iter().all(|(_, lo, hi)| hi[1] - lo[1] <= 384));
    }

    #[test]
    fn the_export_message_names_what_to_load() {
        assert!(export_message(&["hut".into()]).contains("type hut"));
        let many = export_message(&["w_0_0".into(), "w_1_0".into()]);
        assert!(many.contains("2 parts") && many.contains("w_0_0") && many.contains("w_1_0"), "{many}");
    }
}
