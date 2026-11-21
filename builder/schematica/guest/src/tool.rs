//! The tool item, following Litematica's defaults: while a stick is held, mouse buttons and
//! modified scrolling drive the tool mode instead of the world.
//!
//! - Left/right click set selection corner 1/2 in the area modes, and move the placement to the
//!   aimed cell in the schematic modes.
//! - Alt+middle click picks the Fill/Replace block, Shift+middle the block to replace.
//! - Ctrl+scroll cycles the mode, Alt+scroll nudges the selection or placement (Move moves the
//!   selected blocks), and Execute Operation (unbound, as in Litematica) runs Paste, Grid Paste,
//!   Fill, Replace Block and Delete.
//! - Rebuild edits the schematic with the ordinary attack/use clicks while the stick is not held.
//!
//! The pure rules are here so they can be tested natively; host calls stay in the methods.

use crate::capture::{self, Capture, Progress};
use crate::{
    Viewer, budget, drawing as draw, gameplay, input, item_use, native_bindings, paste,
    player_state, terrain,
};
use schematica_core::{BlockState, MaterialLine, Placement, Schematic, StateValue};
use std::collections::{BTreeMap, HashMap, VecDeque};

/// The default tool item, as in Litematica.
pub(crate) const TOOL_ITEM: &str = "minecraft:stick";

/// Frames between checks of the selected hotbar slot.
const POLL_FRAMES: u8 = 3;

/// Copies a grid paste makes, as `[columns along +X, rows along +Z]`; Ctrl+M cycles them.
pub(crate) const GRID_SIZES: [[i32; 2]; 6] = [[2, 1], [2, 2], [3, 3], [4, 4], [5, 5], [8, 8]];

/// The largest box one `/fill` may change on Bedrock.
const FILL_EDGE: i32 = 32;

/// Keys withheld from the game while the menu key is held, so M chords never also open chat
/// or reload shaders.
pub(crate) const MENU_CHORD_KEYS: [&str; 10] = [
    "KeyT", "KeyR", "KeyG", "KeyL", "KeyP", "KeyC", "KeyV", "KeyS", "PageUp", "PageDown",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Mode {
    #[default]
    AreaSelection,
    Placement,
    Fill,
    ReplaceBlock,
    Paste,
    GridPaste,
    Move,
    Delete,
    Rebuild,
}

impl Mode {
    /// Litematica's order.
    pub(crate) const ALL: [Mode; 9] = [
        Mode::AreaSelection,
        Mode::Placement,
        Mode::Fill,
        Mode::ReplaceBlock,
        Mode::Paste,
        Mode::GridPaste,
        Mode::Move,
        Mode::Delete,
        Mode::Rebuild,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Mode::AreaSelection => "Area Selection",
            Mode::Placement => "Schematic Placement",
            Mode::Fill => "Fill",
            Mode::ReplaceBlock => "Replace Block",
            Mode::Paste => "Paste Schematic",
            Mode::GridPaste => "Grid Paste",
            Mode::Move => "Move",
            Mode::Delete => "Delete",
            Mode::Rebuild => "Rebuild",
        }
    }

    pub(crate) fn uses_area(self) -> bool {
        matches!(
            self,
            Mode::AreaSelection | Mode::Fill | Mode::ReplaceBlock | Mode::Move | Mode::Delete
        )
    }

    pub(crate) fn uses_primary(self) -> bool {
        matches!(self, Mode::Fill | Mode::ReplaceBlock)
    }

    pub(crate) fn uses_secondary(self) -> bool {
        self == Mode::ReplaceBlock
    }

    pub(crate) fn cycle(self, forward: bool) -> Self {
        let count = Self::ALL.len();
        let index = Self::ALL.iter().position(|mode| *mode == self).unwrap_or(0);
        Self::ALL[if forward {
            (index + 1) % count
        } else {
            (index + count - 1) % count
        }]
    }

    fn number(self) -> usize {
        Self::ALL.iter().position(|mode| *mode == self).unwrap_or(0) + 1
    }
}

/// Paste replace behaviour, as Litematica's `pasteReplaceBehavior`. Ctrl+M cycles it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Replace {
    /// Only empty cells change.
    #[default]
    None,
    /// Every planned block is set, also over existing blocks.
    All,
}

/// Where each copy of a grid paste starts: row-major from the placement, one footprint apart.
pub(crate) fn grid_origins(origin: [i32; 3], footprint: [i32; 3], grid: [i32; 2]) -> Vec<[i32; 3]> {
    let mut origins = Vec::new();
    for row in 0..grid[1].max(1) {
        for column in 0..grid[0].max(1) {
            origins.push([
                origin[0] + column * footprint[0].max(1),
                origin[1],
                origin[2] + row * footprint[2].max(1),
            ]);
        }
    }
    origins
}

/// The closest axis direction to where the camera looks, as Litematica's nudges use.
pub(crate) fn look_direction(yaw: f32, pitch: f32) -> [i32; 3] {
    let forward = [
        -yaw.sin() * pitch.cos(),
        pitch.sin(),
        -yaw.cos() * pitch.cos(),
    ];
    let axis = (0..3)
        .max_by(|a, b| forward[*a].abs().total_cmp(&forward[*b].abs()))
        .unwrap_or(0);
    let mut step = [0; 3];
    step[axis] = if forward[axis] < 0. { -1 } else { 1 };
    step
}

/// `/fill` commands that cover a box, each within the Bedrock volume limit.
pub(crate) fn fill_commands(
    min: [i32; 3],
    size: [i32; 3],
    block: &str,
    replace: Option<&str>,
) -> Vec<String> {
    let mut commands = Vec::new();
    for x in (0..size[0]).step_by(FILL_EDGE as usize) {
        for y in (0..size[1]).step_by(FILL_EDGE as usize) {
            for z in (0..size[2]).step_by(FILL_EDGE as usize) {
                let lo = [min[0] + x, min[1] + y, min[2] + z];
                let hi = [0, 1, 2].map(|i| (lo[i] + FILL_EDGE).min(min[i] + size[i]) - 1);
                let mut command = format!(
                    "/fill {} {} {} {} {} {} {block}",
                    lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
                );
                match replace {
                    Some(old) => command.push_str(&format!(" replace {old}")),
                    None => command.push_str(" replace"),
                }
                commands.push(command);
            }
        }
    }
    commands
}

/// The other half of a door or tall plant, when the cell is one half of such a pair.
pub(crate) fn partner_cell(schematic: &Schematic, local: [i32; 3]) -> Option<[i32; 3]> {
    let upper_of = |block: &BlockState| match block.states.get("upper_block_bit")? {
        StateValue::Bool(value) => Some(*value),
        StateValue::Byte(value) => Some(*value == 1),
        StateValue::Int(value) => Some(*value == 1),
        StateValue::Str(_) => None,
    };
    let block = schematic.get(local)?;
    let upper = upper_of(block)?;
    let other = [local[0], local[1] + if upper { -1 } else { 1 }, local[2]];
    let partner = schematic.get(other)?;
    (partner.name == block.name && upper_of(partner)? != upper).then_some(other)
}

/// Keeps a bill of materials in step with an edit: `delta` blocks of `name`, largest first.
pub(crate) fn adjust_materials(lines: &mut Vec<MaterialLine>, name: &str, delta: i64) {
    match lines.iter().position(|line| line.name == name) {
        Some(index) => {
            let line = &mut lines[index];
            line.total = line.total.saturating_add_signed(delta);
            line.missing = line.missing.saturating_add_signed(delta).min(line.total);
            if line.total == 0 {
                lines.remove(index);
            }
        }
        None if delta > 0 => lines.push(MaterialLine {
            name: name.to_owned(),
            total: delta as u64,
            missing: delta as u64,
        }),
        None => {}
    }
    lines.sort_by_key(|line| std::cmp::Reverse(line.total));
}

/// A block read from the world for the Fill/Replace block slots.
struct Pick {
    position: [i32; 3],
    primary: bool,
    frames: u8,
}

/// Blocks a world edit pastes after its fills: a copied schematic and where it goes.
struct PasteJob {
    schematic: Schematic,
    placement: Placement,
    cursor: usize,
    palette: HashMap<u32, Option<String>>,
}

/// A world edit: commands, then optionally a schematic paste, then a selection update.
struct Operation {
    scope: (u64, i32),
    title: &'static str,
    commands: VecDeque<String>,
    paste: Option<PasteJob>,
    sent: usize,
    /// Moves the selection by this offset when the edit finishes.
    then_move: Option<[i32; 3]>,
}

/// What happens to a finished capture.
enum CaptureUse {
    /// Becomes the guide, as Litematica's in-memory schematic.
    Guide,
    /// Moves the selected blocks by this offset.
    Move([i32; 3]),
}

struct Outline {
    scope: (u64, i32),
    corners: [Option<[i32; 3]>; 2],
    handles: Vec<(draw::Handle, draw::Handle)>,
}

#[derive(Default)]
pub(crate) struct ToolState {
    pub(crate) mode: Mode,
    pub(crate) grid: usize,
    pub(crate) replace: Replace,
    /// Whether the selected hotbar slot holds the tool item, as last observed.
    pub(crate) held: bool,
    /// The keys currently withheld from the game.
    reserved: Vec<String>,
    /// The held keys under which the wheel is claimed from the hotbar; empty for none.
    scroll_claim: Vec<String>,
    poll: u8,
    /// The selected slot and equipment token last read, so unchanged equipment is not copied.
    seen: Option<(u64, i32, u8, u64)>,
    /// The selection's corners.
    pub(crate) corners: [Option<[i32; 3]>; 2],
    pub(crate) primary: Option<BlockState>,
    pub(crate) secondary: Option<BlockState>,
    pick: Option<Pick>,
    capture: Option<(Capture, CaptureUse)>,
    operation: Option<Operation>,
    outline: Option<Outline>,
    /// The last host error from reading the held item, shown once.
    pub(crate) error: Option<String>,
}

impl ToolState {
    pub(crate) fn grid_size(&self) -> [i32; 2] {
        GRID_SIZES[self.grid % GRID_SIZES.len()]
    }

    pub(crate) fn cycle_mode(&mut self, forward: bool) {
        self.mode = self.mode.cycle(forward);
    }

    /// Ctrl+M: the sub mode of Grid Paste (grid size) and Paste (replace behaviour).
    pub(crate) fn cycle_sub_mode(&mut self) -> bool {
        match self.mode {
            Mode::GridPaste => self.grid = (self.grid + 1) % GRID_SIZES.len(),
            Mode::Paste => {
                self.replace = match self.replace {
                    Replace::None => Replace::All,
                    Replace::All => Replace::None,
                }
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn selection(&self) -> Option<([i32; 3], [i32; 3])> {
        selection_box(self.corners)
    }

    pub(crate) fn busy(&self) -> bool {
        self.capture.is_some() || self.operation.is_some()
    }
}

fn block_caption(block: &Option<BlockState>) -> String {
    block.as_ref().map_or_else(
        || "none".into(),
        |block| {
            block
                .name
                .strip_prefix("minecraft:")
                .unwrap_or(&block.name)
                .replace('_', " ")
        },
    )
}

impl Viewer {
    /// Whether the tool is enabled and its item is in the player's hand.
    pub(crate) fn tool_active(&self) -> bool {
        self.options.tool_item && self.tool.held
    }

    pub(crate) fn toggle_tool(&mut self) {
        self.options.tool_item = !self.options.tool_item;
        self.options.save();
        self.tool.held = false;
        self.tool.poll = 0;
        self.tool.seen = None;
        self.last_label.clear();
        let message = if self.options.tool_item {
            "Tool item on | Hold a stick"
        } else {
            "Tool item off"
        };
        self.show_label(message);
    }

    /// The HUD line while the tool is held, or while Rebuild edits without it.
    pub(crate) fn tool_label(&self) -> Option<String> {
        if !self.options.tool_item || (!self.tool.held && self.tool.mode != Mode::Rebuild) {
            return None;
        }
        if !self.tool.held && !self.loaded {
            return None;
        }
        let mode = self.tool.mode;
        let execute = native_bindings::caption_or(native_bindings::Action::Execute, "");
        let execute = if execute.is_empty() {
            "Execute: bind it in Keyboard settings".to_owned()
        } else {
            format!("{execute}: execute")
        };
        let selection = self.tool.selection().map_or_else(
            || match self.tool.corners {
                [Some(a), None] | [None, Some(a)] => format!("corner {a:?}"),
                _ => "no selection".into(),
            },
            |(_, [x, y, z])| format!("{x} x {y} x {z}"),
        );
        let detail = match mode {
            Mode::AreaSelection => {
                format!("{selection} | Left/Right: corners | Ctrl+Alt+S: copy to guide")
            }
            Mode::Placement => "Left/Right: move here | Alt+scroll: nudge".into(),
            Mode::Fill => format!(
                "{selection} | Block: {} (Alt+middle) | {execute}",
                block_caption(&self.tool.primary)
            ),
            Mode::ReplaceBlock => format!(
                "{selection} | {} to {} (Shift/Alt+middle) | {execute}",
                block_caption(&self.tool.secondary),
                block_caption(&self.tool.primary)
            ),
            Mode::Paste => format!(
                "Left/Right: move here | {} (Ctrl+M) | {execute}",
                match self.tool.replace {
                    Replace::None => "Keep existing",
                    Replace::All => "Replace all",
                }
            ),
            Mode::GridPaste => {
                let [columns, rows] = self.tool.grid_size();
                format!("{columns} x {rows} copies (Ctrl+M) | {execute}")
            }
            Mode::Move => format!("{selection} | Alt+scroll: move the blocks"),
            Mode::Delete => format!("{selection} | {execute}"),
            Mode::Rebuild if self.tool.held => {
                "Left/Right: move here | Put the stick away to edit".into()
            }
            Mode::Rebuild => "Left-click: remove | Right-click: set held block".into(),
        };
        Some(format!(
            "Tool {}/9 {} | {detail} | Ctrl+scroll: mode",
            mode.number(),
            mode.title()
        ))
    }

    /// Withholds exactly `keys` from the game, calling the host only when they change.
    fn tool_reserve(&mut self, mut keys: Vec<String>) {
        keys.sort();
        keys.dedup();
        if keys != self.tool.reserved && input::reserve_keys(&keys).is_ok() {
            self.tool.reserved = keys;
        }
    }

    /// Observes the held item. Cheap change tokens are read first; the inventory is copied only
    /// when the selected slot or the equipment changed. Nothing is read while the use control is
    /// held without the tool, so Hold to Place keeps the whole frame budget.
    fn tool_poll(&mut self, use_held: bool) {
        if !self.options.tool_item {
            self.tool.held = false;
            return;
        }
        if use_held && !self.tool.held {
            return;
        }
        if self.tool.poll > 0 {
            self.tool.poll -= 1;
            return;
        }
        self.tool.poll = POLL_FRAMES;
        let observed = player_state::read_selected_slot().and_then(|slot| {
            Ok(slot
                .zip(player_state::read_revisions()?)
                .map(|(slot, revisions)| {
                    (slot.session, slot.dimension, slot.slot, revisions.equipment)
                }))
        });
        let observed = match observed {
            Ok(observed) => observed,
            Err(error) => {
                if self.tool.error.as_deref() != Some(&error) {
                    self.show_label("Tool item | Grant Player status in Settings > Mods");
                    self.tool.error = Some(error);
                }
                self.tool.held = false;
                return;
            }
        };
        self.tool.error = None;
        if observed == self.tool.seen {
            return;
        }
        self.tool.seen = observed;
        let held = observed.is_some_and(|(_, _, slot, _)| {
            player_state::read_equipment()
                .ok()
                .flatten()
                .and_then(|equipment| {
                    let item = equipment.inventory.get(slot as usize)?.item.as_ref()?;
                    Some(item.identifier.as_deref() == Some(TOOL_ITEM))
                })
                .unwrap_or(false)
        });
        if held != self.tool.held {
            self.tool.held = held;
            self.last_label.clear();
        }
    }

    /// The keys the game must not see this frame.
    fn tool_wanted_keys(&self, rebuild_target: bool) -> Vec<String> {
        let mut wanted: Vec<String> = Vec::new();
        if self.menu_armed {
            wanted.extend(MENU_CHORD_KEYS.iter().map(|key| (*key).to_owned()));
        }
        if self.tool_active() {
            wanted.extend(["MouseLeft", "MouseRight", "MouseMiddle"].map(str::to_owned));
        } else if rebuild_target {
            wanted.extend(["MouseLeft", "MouseRight"].map(str::to_owned));
        }
        wanted
    }

    /// Claims the wheel from the hotbar while Ctrl or Alt is held with the tool. A claim names
    /// one set of keys that must all be held, so it follows whichever modifier is down.
    fn tool_claim_scroll(&mut self, keys: &input::Controls) {
        let held = |name: &str| keys.keys_held.iter().any(|key| key == name);
        let wanted: Vec<String> = if self.tool_active() {
            ["ControlLeft", "ControlRight", "AltLeft", "AltRight"]
                .into_iter()
                .find(|key| held(key))
                .map(|key| vec![key.to_owned()])
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if wanted != self.tool.scroll_claim && input::reserve_scroll(&wanted).is_ok() {
            self.tool.scroll_claim = wanted;
        }
    }

    /// Wheel notches this frame, positive away from the player; zero unless the wheel is claimed.
    fn tool_scroll(&self) -> f32 {
        if self.tool.scroll_claim.is_empty() {
            return 0.;
        }
        input::read_scroll().map_or(0., |scroll| scroll.line_y + scroll.pixel_y / 16.)
    }

    /// Keeps the menu-chord reservation while the gameplay frame is not reached.
    pub(crate) fn tool_reserve_for_menu(&mut self) {
        let mut wanted = self.tool.reserved.clone();
        if self.menu_armed {
            wanted.extend(MENU_CHORD_KEYS.iter().map(|key| (*key).to_owned()));
        } else {
            wanted.retain(|key| !MENU_CHORD_KEYS.contains(&key.as_str()));
        }
        self.tool_reserve(wanted);
    }

    /// Runs the tool for one gameplay frame. Returns whether a selection copy is using the
    /// frame, in which case the rest of the frame is skipped.
    pub(crate) fn tool_frame(&mut self, keys: &input::Controls) -> bool {
        let use_held = keys.keys_held.iter().any(|key| key == "MouseRight")
            || native_bindings::is_held("use", keys);
        self.tool_poll(use_held);
        self.tool_outline();
        self.tool_pick_step();
        self.tool_operation_step();
        let rebuild_target = self.options.tool_item
            && !self.tool.held
            && self.tool.mode == Mode::Rebuild
            && self.loaded
            && self.aimed_block().is_some();
        let wanted = self.tool_wanted_keys(rebuild_target);
        self.tool_reserve(wanted);
        self.tool_claim_scroll(keys);
        if self.tool.capture.is_some() {
            return self.tool_capture_step();
        }
        let pressed = |name: &str| keys.keys_pressed.iter().any(|key| key == name);
        let held = |name: &str| keys.keys_held.iter().any(|key| key.starts_with(name));
        if rebuild_target {
            if pressed("MouseLeft") {
                self.tool_rebuild_remove();
            } else if pressed("MouseRight") {
                self.tool_rebuild_set();
            }
            return false;
        }
        if !self.tool_active() {
            return false;
        }
        let (control, alt, shift) = (held("Control"), held("Alt"), held("Shift"));
        let wheel = self.tool_scroll();
        if wheel != 0. && (control || alt) {
            let amount = if wheel > 0. { 1 } else { -1 };
            if control {
                // Litematica: scrolling down goes forward.
                self.tool.cycle_mode(amount < 0);
            } else {
                self.tool_nudge(amount);
            }
            self.last_label.clear();
        }
        if control && pressed("KeyM") && self.tool.cycle_sub_mode() {
            self.last_label.clear();
        }
        if control && alt && pressed("KeyS") && self.tool.mode.uses_area() {
            self.tool_copy_selection();
        }
        let left = pressed("MouseLeft");
        let right = pressed("MouseRight");
        if pressed("MouseMiddle") {
            if alt && self.tool.mode.uses_primary() {
                self.tool_pick(true);
            } else if shift && self.tool.mode.uses_secondary() {
                self.tool_pick(false);
            }
        }
        if left || right {
            if self.tool.mode.uses_area() {
                self.tool_corner(if left { 0 } else { 1 });
            } else {
                self.tool_place_here();
            }
        }
        false
    }

    /// Runs Execute Operation, which Litematica keeps unbound by default.
    pub(crate) fn tool_execute(&mut self) {
        if !self.tool_active() {
            self.show_label("Execute | Hold the tool item (a stick)");
            return;
        }
        if self.tool.busy() || self.paste.is_some() || self.export_run.is_some() {
            self.show_label("Execute | Wait for the current operation");
            return;
        }
        match self.tool.mode {
            Mode::Paste => {
                if !self.loaded || self.placement.is_none() {
                    self.show_label("Execute | Load and place a schematic first");
                    return;
                }
                let destroy = self.options.paste_destroy;
                self.options.paste_destroy = self.tool.replace == Replace::All;
                self.paste_confirm = true;
                self.paste_button();
                self.options.paste_destroy = destroy;
            }
            Mode::GridPaste => {
                if !self.loaded || self.placement.is_none() {
                    self.show_label("Execute | Load and place a schematic first");
                    return;
                }
                self.paste_grid(self.tool.grid_size());
            }
            Mode::Fill | Mode::ReplaceBlock | Mode::Delete => self.tool_fill(),
            _ => self.show_label("Execute | This mode has no operation"),
        }
        self.last_label.clear();
    }

    fn tool_corner(&mut self, corner: usize) {
        let Some(block) = gameplay::read_block_target().ok().flatten() else {
            self.show_label("Tool | Aim at a block");
            return;
        };
        self.tool.corners[corner] =
            Some([block.x, block.y, block.z].map(|axis| axis.floor() as i32));
        self.last_label.clear();
    }

    /// Moves the guide to the cell next to the aimed block, or in front of the player.
    fn tool_place_here(&mut self) {
        if !self.loaded {
            self.show_label("Tool | Load a schematic first");
            return;
        }
        if self.locked {
            self.show_label("Tool | Placement is locked");
            return;
        }
        let cell = item_use::read_placement_cell()
            .ok()
            .flatten()
            .map(|(x, y, z)| [x, y, z]);
        let placed = match cell {
            Some(origin) => {
                self.placement
                    .get_or_insert_with(|| Placement::new(origin))
                    .origin = origin;
                self.shown = true;
                true
            }
            None => self.move_here(),
        };
        if placed {
            self.dirty = true;
            self.last_label.clear();
        }
    }

    /// Alt+scroll: moves the selection, the placement, or in Move mode the blocks themselves.
    fn tool_nudge(&mut self, amount: i32) {
        let Some(pose) = &self.pose else { return };
        let step = look_direction(pose.yaw, pose.pitch).map(|axis| axis * amount);
        if self.tool.mode == Mode::Move {
            self.tool_move(step);
        } else if self.tool.mode.uses_area() {
            for corner in self.tool.corners.iter_mut().flatten() {
                *corner = [0, 1, 2].map(|i| corner[i] + step[i]);
            }
        } else if !self.locked
            && let Some(placement) = &mut self.placement
        {
            placement.origin = [0, 1, 2].map(|i| placement.origin[i] + step[i]);
            self.dirty = true;
        }
    }

    fn tool_copy_selection(&mut self) {
        let Some(scope) = self.world else { return };
        let [Some(a), Some(b)] = self.tool.corners else {
            self.show_label("Tool | Set both corners first");
            return;
        };
        if self.tool.busy() || self.paste.is_some() || self.export_run.is_some() {
            self.show_label("Tool | Wait for the current operation");
            return;
        }
        match Capture::new(a, b, scope) {
            Ok((capture, _)) => self.tool.capture = Some((capture, CaptureUse::Guide)),
            Err(error) => self.show_label(&format!("Tool | {error}")),
        }
    }

    fn tool_move(&mut self, step: [i32; 3]) {
        let Some(scope) = self.world else { return };
        let [Some(a), Some(b)] = self.tool.corners else {
            self.show_label("Move | Set both corners first");
            return;
        };
        if self.tool.busy() || self.paste.is_some() || self.export_run.is_some() {
            self.show_label("Move | Wait for the current operation");
            return;
        }
        if !gameplay::can_paste_blocks() {
            self.show_label(
                "Move needs Creative, operator cheats, and setblock and fill permissions",
            );
            return;
        }
        match Capture::new(a, b, scope) {
            Ok((capture, _)) => self.tool.capture = Some((capture, CaptureUse::Move(step))),
            Err(error) => self.show_label(&format!("Move | {error}")),
        }
    }

    /// Fill, Replace Block and Delete over the selection.
    fn tool_fill(&mut self) {
        let Some(scope) = self.world else { return };
        let Some((min, size)) = self.tool.selection() else {
            self.show_label("Execute | Set both selection corners first");
            return;
        };
        if !gameplay::can_paste_blocks() {
            self.show_label("Execute needs Creative, operator cheats, and fill permission");
            return;
        }
        let format = |block: &Option<BlockState>| block.as_ref().and_then(paste::format_state);
        let (title, block, replace) = match self.tool.mode {
            Mode::Fill => ("Fill", format(&self.tool.primary), None),
            Mode::ReplaceBlock => {
                let Some(old) = format(&self.tool.secondary) else {
                    self.show_label("Replace | Shift+middle-click the block to replace");
                    return;
                };
                ("Replace", format(&self.tool.primary), Some(old))
            }
            _ => ("Delete", Some("minecraft:air".to_owned()), None),
        };
        let Some(block) = block else {
            self.show_label(&format!("{title} | Alt+middle-click a block to use"));
            return;
        };
        self.tool.operation = Some(Operation {
            scope,
            title,
            commands: fill_commands(min, size, &block, replace.as_deref()).into(),
            paste: None,
            sent: 0,
            then_move: None,
        });
    }

    fn tool_pick(&mut self, primary: bool) {
        let Some(block) = gameplay::read_block_target().ok().flatten() else {
            self.show_label("Tool | Aim at a block to pick it");
            return;
        };
        let position = [block.x, block.y, block.z].map(|axis| axis.floor() as i32);
        self.tool.pick = Some(Pick {
            position,
            primary,
            frames: 0,
        });
    }

    /// Reads a picked block once the host has sampled it.
    fn tool_pick_step(&mut self) {
        let Some(pick) = &mut self.tool.pick else {
            return;
        };
        let [x, y, z] = pick.position;
        let query = terrain::RegionQuery {
            min: terrain::Position { x, y, z },
            max: terrain::Position {
                x: x + 1,
                y: y + 1,
                z: z + 1,
            },
            geometry: terrain::Geometry::empty(),
        };
        let region = match terrain::read_region(query, None) {
            Ok(terrain::ReadResult::Changed(region)) => region,
            Ok(_) if pick.frames < 30 => {
                pick.frames += 1;
                return;
            }
            _ => {
                self.tool.pick = None;
                self.show_label("Tool | That block could not be read");
                return;
            }
        };
        let primary = pick.primary;
        self.tool.pick = None;
        let block = region
            .states
            .first()
            .and_then(|index| region.palette.get(*index as usize))
            .and_then(|state| {
                capture::block_from_terrain(
                    state.identifier.as_deref()?,
                    state.canonical_state.as_deref(),
                )
            });
        let Some(block) = block else {
            self.show_label("Tool | That block could not be read");
            return;
        };
        if primary {
            self.tool.primary = Some(block);
        } else {
            self.tool.secondary = Some(block);
        }
        self.last_label.clear();
    }

    /// Sends this frame's share of a running world edit.
    fn tool_operation_step(&mut self) {
        let Some(mut run) = self.tool.operation.take() else {
            return;
        };
        if self.world != Some(run.scope) {
            self.paste_notice = Some((
                format!("{} canceled because the world changed", run.title),
                6.0,
            ));
            return;
        }
        for _ in 0..4 {
            if budget::remaining_fuel() < 850_000 {
                break;
            }
            let (command, cells) = if let Some(command) = run.commands.front() {
                (command.clone(), 0)
            } else if let Some(job) = &mut run.paste {
                match next_paste_command(job) {
                    Some(next) => next,
                    None => {
                        run.paste = None;
                        continue;
                    }
                }
            } else {
                break;
            };
            match gameplay::request_command(&command) {
                Ok(()) => {
                    if run.commands.pop_front().is_none()
                        && let Some(job) = &mut run.paste
                    {
                        job.cursor += cells;
                    }
                    run.sent += 1;
                }
                Err(error) if error == "command rate limit reached" => break,
                Err(error) => {
                    self.paste_notice = Some((format!("{} stopped | {error}", run.title), 6.0));
                    return;
                }
            }
        }
        if run.commands.is_empty() && run.paste.is_none() {
            if let Some(step) = run.then_move {
                for corner in self.tool.corners.iter_mut().flatten() {
                    *corner = [0, 1, 2].map(|i| corner[i] + step[i]);
                }
            }
            self.paste_notice = Some((
                format!(
                    "{} sent {} commands | See chat for results",
                    run.title, run.sent
                ),
                6.0,
            ));
            self.last_label.clear();
            return;
        }
        self.show_label(&format!("{} | {} commands sent", run.title, run.sent));
        self.tool.operation = Some(run);
    }

    /// Runs a copy for this frame. Returns whether the frame is spent on it.
    fn tool_capture_step(&mut self) -> bool {
        let Some((mut capture, goal)) = self.tool.capture.take() else {
            return false;
        };
        if self.world != Some(capture.scope) {
            self.show_label("Copy canceled because the world changed");
            return false;
        }
        match capture.step() {
            Progress::Working(message) => {
                self.show_label(&message);
                self.tool.capture = Some((capture, goal));
                true
            }
            Progress::Failed(message) => {
                self.show_label(&message);
                false
            }
            Progress::Done {
                schematic,
                unloaded,
                unknown,
            } => {
                let Some((min, size)) = self.tool.selection() else {
                    return false;
                };
                match goal {
                    CaptureUse::Guide => self.adopt_copy(schematic, min, unloaded + unknown),
                    CaptureUse::Move(_) if unloaded + unknown > 0 => {
                        self.show_label("Move stopped | Part of the selection is not loaded");
                    }
                    CaptureUse::Move(step) => {
                        let Some(scope) = self.world else {
                            return false;
                        };
                        let target = [0, 1, 2].map(|i| min[i] + step[i]);
                        let mut commands = fill_commands(min, size, "minecraft:air", None);
                        commands.extend(fill_commands(target, size, "minecraft:air", None));
                        self.tool.operation = Some(Operation {
                            scope,
                            title: "Move",
                            commands: commands.into(),
                            paste: Some(PasteJob {
                                schematic,
                                placement: Placement::new(target),
                                cursor: 0,
                                palette: HashMap::new(),
                            }),
                            sent: 0,
                            then_move: Some(step),
                        });
                    }
                }
                true
            }
        }
    }

    fn adopt_copy(&mut self, schematic: Schematic, origin: [i32; 3], missing: u64) {
        let mut totals = BTreeMap::new();
        for (state, count) in schematic
            .palette()
            .iter()
            .zip(schematic.palette_counts())
            .filter(|(state, count)| *count > 0 && !state.is_air())
        {
            *totals.entry(state.name.clone()).or_insert(0) += count;
        }
        if totals.is_empty() {
            self.show_label("The selection holds only air");
            return;
        }
        crate::ADOPT_ORIGIN.set(Some(origin));
        self.adopt = Some((schematic, totals));
        let note = if missing > 0 {
            format!(" | {missing} cells were not loaded")
        } else {
            String::new()
        };
        self.paste_notice = Some((format!("Selection copied as the guide{note}"), 6.0));
    }

    /// Rebuild: removes the aimed guide block (both halves of a door or tall plant).
    fn tool_rebuild_remove(&mut self) {
        let Some((placement, world)) = self
            .placement
            .zip(self.aimed_block().map(|(_, world)| world))
        else {
            return;
        };
        let Some(local) = placement.world_to_local(self.schematic.size(), world) else {
            return;
        };
        let mut cells = vec![local];
        cells.extend(partner_cell(&self.schematic, local));
        for cell in cells {
            let Some(previous) = self.schematic.get(cell).cloned() else {
                continue;
            };
            if !previous.is_air() && self.schematic.set(cell, BlockState::air()) {
                adjust_materials(&mut self.materials, &previous.name, -1);
            }
        }
        self.dirty = true;
        self.last_label.clear();
    }

    /// Rebuild: replaces the aimed guide block with the held block.
    fn tool_rebuild_set(&mut self) {
        let Some((placement, world)) = self
            .placement
            .zip(self.aimed_block().map(|(_, world)| world))
        else {
            return;
        };
        let held = player_state::read_selected_slot()
            .ok()
            .flatten()
            .zip(player_state::read_equipment().ok().flatten())
            .and_then(|(selected, equipment)| {
                let slot = equipment.inventory.get(selected.slot as usize)?;
                let item = slot.item.as_ref()?;
                item.block.then(|| item.identifier.clone()).flatten()
            });
        let Some(name) = held else {
            self.show_label("Rebuild | Hold a block to set it");
            return;
        };
        let Some(local) = placement.world_to_local(self.schematic.size(), world) else {
            return;
        };
        let previous = self.schematic.get(local).cloned();
        if self.schematic.set(local, BlockState::new(name.clone())) {
            if let Some(previous) = previous.filter(|block| !block.is_air()) {
                adjust_materials(&mut self.materials, &previous.name, -1);
            }
            adjust_materials(&mut self.materials, &name, 1);
            self.dirty = true;
            self.last_label.clear();
        }
    }

    /// Keeps the selection box drawn while the tool is held in an area mode.
    fn tool_outline(&mut self) {
        let want = self
            .world
            .filter(|_| self.tool_active() && self.tool.mode.uses_area())
            .filter(|_| self.tool.corners.iter().any(Option::is_some))
            .map(|scope| (scope, self.tool.corners));
        let current = self
            .tool
            .outline
            .as_ref()
            .map(|outline| (outline.scope, outline.corners));
        if want == current {
            return;
        }
        if let Some(outline) = self.tool.outline.take() {
            // Handles from another dimension were already invalidated by the host.
            if Some(outline.scope) == self.world {
                for (geometry, instance) in outline.handles {
                    let _ = draw::remove(instance);
                    let _ = draw::remove(geometry);
                }
            }
        }
        let Some((scope, corners)) = want else {
            return;
        };
        let mut handles = Vec::new();
        let boxes = [
            selection_box(corners),
            corners[0].map(|corner| (corner, [1; 3])),
            corners[1].map(|corner| (corner, [1; 3])),
        ];
        // Litematica's colours: a white box, red corner 1, blue corner 2.
        let colors = [(1., 1., 1., 1.), (1., 0.2, 0.2, 1.), (0.3, 0.5, 1., 1.)];
        for (bounds, color) in boxes.into_iter().zip(colors) {
            let Some((min, size)) = bounds else {
                continue;
            };
            match draw_box(scope.1, min, size, color) {
                Ok(pair) => handles.push(pair),
                Err(_) => break,
            }
        }
        self.tool.outline = Some(Outline {
            scope,
            corners,
            handles,
        });
    }
}

fn selection_box(corners: [Option<[i32; 3]>; 2]) -> Option<([i32; 3], [i32; 3])> {
    match corners {
        [Some(a), Some(b)] => Some(capture::bounds(a, b)),
        _ => None,
    }
}

/// The 12 edges of a box as one path; four edges are drawn twice so it stays one line.
pub(crate) fn box_path(size: [f32; 3]) -> Vec<[f32; 3]> {
    let [x, y, z] = size;
    let corner = |i: usize| match i {
        0 => [0., 0., 0.],
        1 => [x, 0., 0.],
        2 => [x, 0., z],
        3 => [0., 0., z],
        4 => [0., y, 0.],
        5 => [x, y, 0.],
        6 => [x, y, z],
        _ => [0., y, z],
    };
    [0, 1, 2, 3, 0, 4, 5, 6, 7, 4, 5, 1, 2, 6, 7, 3]
        .into_iter()
        .map(corner)
        .collect()
}

fn draw_box(
    dimension: i32,
    min: [i32; 3],
    size: [i32; 3],
    color: (f32, f32, f32, f32),
) -> Result<(draw::Handle, draw::Handle), draw::DrawError> {
    const PAD: f32 = 0.005;
    let vertices = box_path(size.map(|axis| axis as f32 + 2. * PAD))
        .into_iter()
        .map(|[x, y, z]| draw::Vertex {
            position: (x - PAD, y - PAD, z - PAD),
            color,
            uv: (0., 0.),
            data: (0., 0., 0., 0.),
        })
        .collect();
    let geometry = draw::create_geometry(&draw::GeometrySpec::Line(draw::Polyline {
        vertices,
        closed: false,
    }))?;
    let instance = draw::Instance {
        geometry,
        material: None,
        clip: None,
        params: vec![0.; 16],
        anchor: draw::Anchor::Block(draw::BlockAnchor {
            dimension,
            position: (min[0], min[1], min[2]),
        }),
        transform: draw::Transform {
            translation: (0., 0., 0.),
            rotation: (0., 0., 0., 1.),
            scale: (1., 1., 1.),
        },
        tint: (1., 1., 1., 1.),
        depth: draw::Depth::Test,
        blend: draw::Blend::Premultiplied,
        width: 2.,
        width_mode: draw::WidthMode::Pixels,
        join: draw::Join::Miter,
        cap: draw::Cap::Butt,
    };
    match draw::create(&instance) {
        Ok(handle) => Ok((geometry, handle)),
        Err(error) => {
            let _ = draw::remove(geometry);
            Err(error)
        }
    }
}

/// The next command of a copied schematic and the cells it covers, skipping air and
/// unsupported cells. The destination was cleared first, so runs use `/fill ... keep`.
fn next_paste_command(job: &mut PasteJob) -> Option<(String, usize)> {
    let [width, height, depth] = job.schematic.size().map(|size| size as usize);
    let volume = width * height * depth;
    while job.cursor < volume {
        let local = paste::coordinates(job.cursor, width, depth);
        let index = job.schematic.palette_index(local)?;
        let block = &job.schematic.palette()[index as usize];
        if job.schematic.is_air_index(index) || block.is_placement_artifact() {
            job.cursor += 1;
            continue;
        }
        let formatted = job
            .palette
            .entry(index)
            .or_insert_with(|| paste::format_state(block))
            .clone();
        let Some(formatted) = formatted else {
            job.cursor += 1;
            continue;
        };
        match paste::command(
            &job.schematic,
            job.placement,
            local,
            index,
            &formatted,
            false,
        ) {
            (Some(command), cells) => return Some((command, cells)),
            (None, _) => job.cursor += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_follow_litematica_and_cycle_both_ways() {
        assert_eq!(Mode::default(), Mode::AreaSelection);
        let mut mode = Mode::AreaSelection;
        for _ in 0..Mode::ALL.len() {
            mode = mode.cycle(true);
        }
        assert_eq!(mode, Mode::AreaSelection);
        assert_eq!(Mode::AreaSelection.cycle(false), Mode::Rebuild);
        assert!(Mode::Fill.uses_area() && Mode::Fill.uses_primary());
        assert!(Mode::ReplaceBlock.uses_secondary());
        assert!(!Mode::Paste.uses_area() && !Mode::Rebuild.uses_area());
    }

    #[test]
    fn grid_copies_never_overlap_and_start_at_the_placement() {
        let origins = grid_origins([10, 64, -4], [5, 7, 3], [3, 2]);
        assert_eq!(origins.len(), 6);
        assert_eq!(origins[0], [10, 64, -4]);
        assert_eq!(origins[1], [15, 64, -4]);
        assert_eq!(origins[3], [10, 64, -1]);
        let unique: std::collections::BTreeSet<_> = origins.iter().collect();
        assert_eq!(unique.len(), origins.len());
    }

    #[test]
    fn sub_modes_cycle_grid_size_and_replace_behaviour() {
        let mut state = ToolState::default();
        assert!(!state.cycle_sub_mode());
        state.mode = Mode::GridPaste;
        for _ in 0..GRID_SIZES.len() {
            assert!(state.cycle_sub_mode());
        }
        assert_eq!(state.grid_size(), GRID_SIZES[0]);
        state.mode = Mode::Paste;
        state.cycle_sub_mode();
        assert_eq!(state.replace, Replace::All);
    }

    #[test]
    fn the_look_direction_is_the_nearest_axis() {
        use std::f32::consts::FRAC_PI_2;
        assert_eq!(look_direction(0., 0.), [0, 0, -1]);
        assert_eq!(look_direction(FRAC_PI_2, 0.), [-1, 0, 0]);
        assert_eq!(look_direction(0., 1.2), [0, 1, 0]);
        assert_eq!(look_direction(0., -1.2), [0, -1, 0]);
    }

    #[test]
    fn fills_stay_within_the_bedrock_limit_and_cover_the_box() {
        let commands = fill_commands([0, 60, 0], [40, 5, 33], "minecraft:stone", None);
        assert_eq!(commands.len(), 4);
        assert_eq!(commands[0], "/fill 0 60 0 31 64 31 minecraft:stone replace");
        assert_eq!(
            commands[3],
            "/fill 32 60 32 39 64 32 minecraft:stone replace"
        );
        let replace = fill_commands(
            [1, 2, 3],
            [1, 1, 1],
            "minecraft:glass",
            Some("minecraft:dirt"),
        );
        assert_eq!(
            replace,
            ["/fill 1 2 3 1 2 3 minecraft:glass replace minecraft:dirt"]
        );
    }

    #[test]
    fn the_box_path_visits_all_twelve_edges() {
        let path = box_path([2., 3., 4.]);
        let mut edges = std::collections::BTreeSet::new();
        for pair in path.windows(2) {
            let key = |p: [f32; 3]| p.map(|v| v as i32);
            let (a, b) = (key(pair[0]), key(pair[1]));
            edges.insert(if a < b { (a, b) } else { (b, a) });
        }
        assert_eq!(edges.len(), 12);
    }

    #[test]
    fn doors_and_tall_plants_remove_their_other_half() {
        let door = |upper: i8| {
            BlockState::new("minecraft:oak_door").with("upper_block_bit", StateValue::Byte(upper))
        };
        let mut schematic = Schematic::new("door", [1, 3, 1]);
        schematic.set([0, 0, 0], door(0));
        schematic.set([0, 1, 0], door(1));
        schematic.set([0, 2, 0], BlockState::new("minecraft:stone"));
        assert_eq!(partner_cell(&schematic, [0, 0, 0]), Some([0, 1, 0]));
        assert_eq!(partner_cell(&schematic, [0, 1, 0]), Some([0, 0, 0]));
        assert_eq!(partner_cell(&schematic, [0, 2, 0]), None);
    }

    #[test]
    fn edits_keep_the_material_list_in_step() {
        let line = |name: &str, total| MaterialLine {
            name: name.into(),
            total,
            missing: total,
        };
        let mut lines = vec![line("minecraft:stone", 5), line("minecraft:glass", 2)];
        adjust_materials(&mut lines, "minecraft:glass", -2);
        assert_eq!(lines.len(), 1);
        adjust_materials(&mut lines, "minecraft:glass", 6);
        assert_eq!(lines[0].name, "minecraft:glass");
        adjust_materials(&mut lines, "minecraft:missing", -1);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn a_copied_block_pastes_with_its_state() {
        let mut schematic = Schematic::new("copy", [2, 1, 1]);
        schematic.set(
            [1, 0, 0],
            BlockState::new("minecraft:oak_stairs").with("weirdo_direction", StateValue::Int(2)),
        );
        let mut job = PasteJob {
            schematic,
            placement: Placement::new([10, 64, 5]),
            cursor: 0,
            palette: HashMap::new(),
        };
        assert_eq!(
            next_paste_command(&mut job),
            Some((
                "/setblock 11 64 5 minecraft:oak_stairs [\"weirdo_direction\"=2] keep".into(),
                1
            ))
        );
        job.cursor += 1;
        assert!(next_paste_command(&mut job).is_none());
    }
}
