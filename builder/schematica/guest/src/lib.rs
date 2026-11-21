wit_bindgen::generate!({
    path: "wit",
    world: "cinnabar:extension/extension@0.1.0",
    generate_all,
});

mod actions;
mod browser;
mod library;
mod menu;
mod native_bindings;
mod overlay;
mod paste;
mod shapes;
mod status;
mod structure_paste;

// Embed the revision in the guest itself so the importer labels the code it actually packages.
#[cfg(target_arch = "wasm32")]
const BUILD_REVISION: &str = match option_env!("SCHEMATICA_BUILD_REVISION") {
    Some(revision) => revision,
    None => "",
};

#[cfg(target_arch = "wasm32")]
#[used]
#[unsafe(link_section = "schematica:build-revision")]
static GUEST_REVISION: [u8; BUILD_REVISION.len()] = {
    let mut bytes = [0; BUILD_REVISION.len()];
    bytes.copy_from_slice(BUILD_REVISION.as_bytes());
    bytes
};

#[cfg(target_arch = "wasm32")]
#[used]
#[unsafe(link_section = "schematica:build-version")]
static GUEST_VERSION: [u8; env!("CARGO_PKG_VERSION").len()] = {
    let mut bytes = [0; env!("CARGO_PKG_VERSION").len()];
    bytes.copy_from_slice(env!("CARGO_PKG_VERSION").as_bytes());
    bytes
};

use cinnabar::{
    extension::{budget, gameplay, hud, input, item_use, panel, player_state, settings, terrain},
    world_draw::drawing,
};
use overlay::{Preview, Tile, WholeGhost};
use schematica_core::{LayerMode, MaterialLine, Mirror, Placement, Schematic, blueprint};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    f32::consts::TAU,
};

mod capture;
mod saved_placements;
mod tool;

thread_local! {
    static OPEN_AFTER_LOAD: Cell<bool> = const { Cell::new(false) };
    /// Where the guide of a copied selection goes: the box's minimum corner.
    static ADOPT_ORIGIN: Cell<Option<[i32; 3]>> = const { Cell::new(None) };
    static RUNTIME: RefCell<(Runtime, Vec<&'static [u8]>, Options, Option<usize>)> =
        const { RefCell::new((Runtime::Start, Vec::new(), Options { hold_place: false, placement_lock: false, auto_layer: true, paste_destroy: false, textured: true, assist: false, fast_place: false, strict_place: false, correct_aim: false, bridge_place: true, lightness: DEFAULT_LIGHTNESS, library_revision: 0, deleted_entries: Vec::new(), remember_placement: true, saved_placements: BTreeMap::new(), tool_item: true }, None)) };
}

/// The "Ghost lightness" cycle: percent the ghost blocks blend toward light blue.
pub(crate) const LIGHTNESS_LEVELS: [u8; 5] = [5, 10, 15, 25, 35];
const DEFAULT_LIGHTNESS: u8 = LIGHTNESS_LEVELS[2];

/// Reads the saved ghost lightness, bounded to the cycle. Older settings files hold a
/// see-through "opacity" or a white-blend "lightness" instead; neither applies any more,
/// so they fall back to the default.
fn saved_lightness(settings: &serde_json::Value) -> u8 {
    settings["ghost_tint"]
        .as_u64()
        .map_or(DEFAULT_LIGHTNESS, |lightness| {
            lightness.clamp(
                u64::from(LIGHTNESS_LEVELS[0]),
                u64::from(LIGHTNESS_LEVELS[LIGHTNESS_LEVELS.len() - 1]),
            ) as u8
        })
}

#[derive(Clone)]
struct Options {
    hold_place: bool,
    placement_lock: bool,
    auto_layer: bool,
    paste_destroy: bool,
    textured: bool,
    assist: bool,
    fast_place: bool,
    strict_place: bool,
    correct_aim: bool,
    bridge_place: bool,
    /// Percent the ghost blocks blend toward light blue; one of `LIGHTNESS_LEVELS`.
    lightness: u8,
    library_revision: u64,
    deleted_entries: Vec<usize>,
    remember_placement: bool,
    saved_placements: BTreeMap<String, serde_json::Value>,
    /// The tool item (a stick) drives builder modes while held. On by default, as in Litematica.
    tool_item: bool,
}

impl Options {
    fn save(&self) {
        #[cfg(not(test))]
        let _ = settings::save(
            &serde_json::json!({
                "hold_place": self.hold_place,
                "auto_layer": self.auto_layer,
                "paste_destroy": self.paste_destroy,
                "textured":self.textured, "assist":self.assist, "fast_place": self.fast_place,
                "correct_aim": self.correct_aim,
                "strict_place": self.strict_place,
                "placement_lock": self.placement_lock,
                "bridge_place": self.bridge_place,
                "remember_placement": self.remember_placement, "saved_placements": self.saved_placements,
                "ghost_tint": self.lightness,
                "tool_item_enabled": self.tool_item,
                "library_revision": self.library_revision, "deleted_entries": self.deleted_entries,
            })
            .to_string(),
        );
    }
}

enum Runtime {
    Start,
    Loading(blueprint::Decoder<'static>),
    Counting {
        schematic: Schematic,
        counts: Vec<u64>,
        totals: BTreeMap<String, u64>,
        cursor: usize,
    },
    Finishing(Schematic, BTreeMap<String, u64>),
    Ready(Viewer),
    Invalid,
}

/// Material list identity (length and storage) and its `(index, name, lowercase name)`
/// entries in alphabetical order.
type BrowserNames = ((usize, usize), Vec<(usize, std::rc::Rc<str>, String)>);
/// Material list identity, palette identity, and each material's palette indices.
/// Fuel a window refresh needs before it runs in the current frame.
const REFRESH_FUEL: u64 = 600_000;

struct Schematica;

impl Guest for Schematica {
    fn init() {
        if let Ok(raw) = settings::load()
            && raw.len() <= 16 * 1024
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw)
        {
            RUNTIME.with(|state| {
                let options = &mut state.borrow_mut().2;
                options.hold_place = value["hold_place"].as_bool().unwrap_or(false);
                options.auto_layer = value["auto_layer"].as_bool().unwrap_or(true);
                options.paste_destroy = value["paste_destroy"].as_bool().unwrap_or(false);
                options.textured = value["textured"].as_bool().unwrap_or(options.textured);
                options.assist = value["assist"].as_bool().unwrap_or(options.assist);
                options.fast_place = value["fast_place"].as_bool().unwrap_or(options.fast_place);
                options.strict_place = value["strict_place"].as_bool().unwrap_or(false);
                options.placement_lock = value["placement_lock"].as_bool().unwrap_or(false);
                options.correct_aim = value["correct_aim"]
                    .as_bool()
                    .unwrap_or(options.correct_aim);
                options.bridge_place = value["bridge_place"].as_bool().unwrap_or(true);
                options.remember_placement = value["remember_placement"].as_bool().unwrap_or(true);
                options.saved_placements = saved_placements::load(&value["saved_placements"]);
                options.library_revision = value["library_revision"].as_u64().unwrap_or(0);
                options.deleted_entries = value["deleted_entries"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| entry.as_u64())
                    .filter(|index| *index < blueprint::MAX_LIBRARY as u64)
                    .map(|index| index as usize)
                    .take(blueprint::MAX_LIBRARY)
                    .collect();
                options.lightness = saved_lightness(&value);
                // Litematica enables the tool item by default.
                options.tool_item = value["tool_item_enabled"].as_bool().unwrap_or(true);
            });
        }
        RUNTIME.with(|runtime| label(&runtime.borrow().2.menu_hint("Cinnamatica")));
    }

    fn frame() {
        RUNTIME.with(|runtime| {
            let (runtime, catalog, options, active_entry) = &mut *runtime.borrow_mut();
            if matches!(*runtime, Runtime::Start) {
                static SLOT: [u8; blueprint::SLOT_BYTES] = blueprint::empty_slot();
                let bytes = std::hint::black_box(&SLOT).as_slice();
                let bytes = blueprint::embedded_payload(bytes).unwrap_or_default();
                match blueprint::library(bytes) {
                    Ok(entries) => {
                        options.select_library(blueprint::library_revision(bytes));
                        *catalog = entries;
                        let mut viewer = Viewer::new(
                            Schematic::new("No schematic loaded", [1; 3]),
                            Vec::new(),
                            library_names(catalog),
                            options.clone(),
                            false,
                        );
                        viewer.library_sizes = library_sizes(catalog);
                        *runtime = Runtime::Ready(viewer);
                    }
                    Err(_) => invalidate(runtime),
                }
            }
            if matches!(
                runtime,
                Runtime::Loading(_) | Runtime::Counting { .. } | Runtime::Finishing(..)
            ) && let Ok(keys) = input::read_controls()
                && keys.focused
                && keys.gameplay
            {
                if native_bindings::menu_pressed(&keys) {
                    OPEN_AFTER_LOAD.set(true);
                } else if keys.keys_pressed.iter().any(|key| key == "Escape") {
                    OPEN_AFTER_LOAD.set(false);
                }
            }
            match &mut *runtime {
                Runtime::Ready(viewer) => {
                    if let Some(index) = viewer.request {
                        if viewer.release_for_switch() {
                            let _ = hud::set_item_highlights(&[]);
                            viewer.remember_placement();
                            *options = viewer.options.clone();
                            *active_entry = Some(index);
                            match blueprint::Decoder::new(catalog[index]) {
                                Ok(decoder) => *runtime = Runtime::Loading(decoder),
                                Err(_) => invalidate(runtime),
                            }
                        }
                    } else if viewer.adopt.is_some() {
                        if viewer.release_for_switch() {
                            let _ = hud::set_item_highlights(&[]);
                            viewer.remember_placement();
                            *options = viewer.options.clone();
                            *active_entry = None;
                            let (schematic, totals) = viewer.adopt.take().unwrap();
                            *runtime = Runtime::Finishing(schematic, totals);
                        }
                    } else {
                        viewer.frame();
                    }
                }
                Runtime::Loading(decoder) => {
                    for _ in 0..32 {
                        if budget::remaining_fuel() < 500_000 {
                            break;
                        }
                        match decoder.step() {
                            Ok(false) => {}
                            Ok(true) => {
                                let Runtime::Loading(decoder) =
                                    std::mem::replace(&mut *runtime, Runtime::Invalid)
                                else {
                                    unreachable!()
                                };
                                match decoder.finish() {
                                    Ok((schematic, counts)) => {
                                        *runtime = Runtime::Counting {
                                            schematic,
                                            counts,
                                            totals: BTreeMap::new(),
                                            cursor: 0,
                                        };
                                    }
                                    Err(_) => invalidate(runtime),
                                }
                                break;
                            }
                            Err(_) => {
                                invalidate(runtime);
                                break;
                            }
                        }
                    }
                }
                Runtime::Counting {
                    schematic,
                    counts,
                    totals,
                    cursor,
                } => {
                    let stop = (*cursor + 24).min(counts.len());
                    while *cursor < stop && budget::remaining_fuel() >= 500_000 {
                        let state = &schematic.palette()[*cursor];
                        let count = counts[*cursor];
                        if count > 0 && !state.is_air() {
                            *totals.entry(state.name.clone()).or_insert(0) += count;
                        }
                        *cursor += 1;
                    }
                    if *cursor == counts.len() {
                        let Runtime::Counting {
                            schematic, totals, ..
                        } = std::mem::replace(&mut *runtime, Runtime::Invalid)
                        else {
                            unreachable!()
                        };
                        *runtime = Runtime::Finishing(schematic, totals);
                    }
                }
                Runtime::Finishing(..) => {
                    let Runtime::Finishing(schematic, totals) =
                        std::mem::replace(&mut *runtime, Runtime::Invalid)
                    else {
                        unreachable!()
                    };
                    let mut materials: Vec<_> = totals
                        .into_iter()
                        .map(|(name, total)| MaterialLine {
                            name,
                            total,
                            missing: total,
                        })
                        .collect();
                    materials.sort_by_key(|line| std::cmp::Reverse(line.total));
                    let mut viewer = Viewer::new(
                        schematic,
                        materials,
                        library_names(catalog),
                        options.clone(),
                        true,
                    );
                    viewer.active_entry = *active_entry;
                    viewer.library_sizes = library_sizes(catalog);
                    *runtime = Runtime::Ready(viewer);
                    label(&options.menu_hint("Cinnamatica"));
                }
                Runtime::Invalid => {}
                Runtime::Start => unreachable!(),
            }
        });
    }
}

fn library_names(catalog: &[&[u8]]) -> Vec<String> {
    catalog
        .iter()
        .map(|entry| blueprint::title(entry).unwrap_or_else(|_| "Invalid schematic".into()))
        .collect()
}

/// Each entry's size, read from its header for the P window's library cards.
fn library_sizes(catalog: &[&[u8]]) -> Vec<[i32; 3]> {
    catalog
        .iter()
        .map(|entry| blueprint::size(entry).unwrap_or([0; 3]))
        .collect()
}

fn invalidate(runtime: &mut Runtime) {
    *runtime = Runtime::Invalid;
    label("Cinnamatica | Build the component with a valid embedded blueprint.");
}

export!(Schematica);

fn label(message: &str) {
    let mut end = message.len().min(256);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    let _ = hud::set_label(&message[..end]);
}

struct Viewer {
    schematic: Schematic,
    materials: Vec<schematica_core::MaterialLine>,
    last_item_highlights: Vec<String>,
    highlight_dirty: bool,
    layer_counts: Vec<u64>,
    layer_count_cursor: usize,
    highlight_scan: Option<(usize, Vec<u64>)>,
    highlight_again: bool,
    highlight_delay: u8,
    placement: Option<Placement>,
    world: Option<(u64, i32)>,
    placement_key: Option<String>,
    pose: Option<gameplay::Snapshot>,
    tile_focus: Option<[i32; 3]>,
    distant_scan_phase: u8,
    preview: Preview,
    whole: WholeGhost,
    /// Single-cell updates around the camera, ahead of the tile verifier.
    hot: overlay::HotWatch,
    /// Cells the hot watch saw change, whose tiles are verified next.
    hot_dirty: Vec<([i32; 3], u8)>,
    open_retries: u8,
    tiles: Vec<Tile>,
    retired: Vec<Tile>,
    cursor: usize,
    layer: i32,
    layer_mode: u8,
    shown: bool,
    render_all: bool,
    locked: bool,
    menu_armed: bool,
    last_target: Option<[i32; 3]>,
    recently_targeted: Option<([i32; 3], u8)>,
    scan_priority: bool,
    nearby_aim: Option<(([f32; 3], [f32; 3]), Option<[i32; 3]>, Option<[i32; 3]>)>,
    options: Options,
    material_page: usize,
    /// The open tab of the window.
    browser_tab: browser::Tab,
    /// Text typed into the window's material search.
    browser_search: String,
    /// The library row whose Delete was clicked once and awaits a second click.
    delete_confirm: Option<usize>,
    /// The window's alphabetical material names, keyed by the material list they describe.
    browser_names: std::cell::RefCell<Option<BrowserNames>>,
    /// Each material's palette indices, keyed by the material list and palette they describe.
    /// A window refresh held over from a frame without enough fuel left for it.
    browser_refresh_pending: bool,
    browser_counts: std::cell::RefCell<browser::MaterialPageCounts>,
    inventory: std::cell::RefCell<browser::InventoryCache>,
    library: Vec<String>,
    /// Library entry sizes, matching `library`.
    library_sizes: Vec<[i32; 3]>,
    active_entry: Option<usize>,
    request: Option<usize>,
    loaded: bool,
    auto_place: bool,
    dirty: bool,
    paste_confirm: bool,
    export_run: Option<paste::ExportRun>,
    /// The pending confirmation is for Export, not Paste.
    export_confirming: bool,
    paste_notice: Option<(String, f32)>,
    cancel_structure: Option<(u64, i32)>,
    paste: Option<paste::Paste>,
    repair: Option<paste::Repair>,
    last_label: String,
    tool: tool::ToolState,
    /// A copied selection waiting to replace the schematic.
    adopt: Option<(Schematic, BTreeMap<String, u64>)>,
    /// An M chord pressed another key, so releasing M does not open the window.
    menu_chorded: bool,
    /// The window tab an M chord asked for, applied once the window is open.
    open_tab: Option<browser::Tab>,
}

/// What a key pressed while M is held did.
enum Chord {
    None,
    Done,
    Open(browser::Tab),
}

impl Viewer {
    fn new(
        schematic: Schematic,
        materials: Vec<MaterialLine>,
        library: Vec<String>,
        options: Options,
        loaded: bool,
    ) -> Self {
        Self {
            schematic,
            materials,
            placement: None,
            world: None,
            placement_key: None,
            pose: None,
            tile_focus: None,
            distant_scan_phase: 0,
            preview: Preview::default(),
            whole: WholeGhost::default(),
            hot: overlay::HotWatch::default(),
            hot_dirty: Vec::new(),
            open_retries: 0,
            tiles: Vec::new(),
            retired: Vec::new(),
            cursor: 0,
            layer: 0,
            layer_mode: 0,
            shown: true,
            render_all: true,
            locked: false,
            menu_armed: false,
            last_target: None,
            recently_targeted: None,
            scan_priority: false,
            nearby_aim: None,
            options,
            material_page: 0,
            browser_tab: browser::Tab::default(),
            browser_search: String::new(),
            delete_confirm: None,
            browser_names: Default::default(),
            browser_refresh_pending: false,
            browser_counts: Default::default(),
            inventory: Default::default(),
            library_sizes: Vec::new(),
            library,
            last_item_highlights: Vec::new(),
            highlight_dirty: true,
            layer_counts: Vec::new(),
            layer_count_cursor: 0,
            highlight_scan: None,
            highlight_again: false,
            highlight_delay: 0,
            active_entry: None,
            request: None,
            loaded,
            auto_place: loaded,
            dirty: false,
            paste_confirm: false,
            export_run: None,
            export_confirming: false,
            paste_notice: None,
            cancel_structure: None,
            paste: None,
            repair: None,
            last_label: String::new(),
            tool: Default::default(),
            adopt: None,
            menu_chorded: false,
            open_tab: None,
        }
    }

    fn frame(&mut self) {
        let Ok(world) = gameplay::read_frame() else {
            label("Cinnamatica | Grant terrain access in Settings > Mods.");
            return;
        };
        let active_world = world.is_some();
        if let Some(world) = world {
            let scope = (world.session, world.dimension);
            if self.world != Some(scope) {
                self.paste = None;
                self.export_run = None;
                self.repair = None;
                self.paste_confirm = false;
                let entering_from_world = self.world.is_some();
                // Cinnabar invalidates drawing handles on dimension changes.
                self.tiles.clear();
                self.retired.clear();
                self.whole = WholeGhost::default();
                self.placement = None;
                self.locked = false;
                self.world = Some(scope);
                self.placement_key = gameplay::read_local_world_key()
                    .ok()
                    .flatten()
                    .map(|key| format!("{key:016x}/{}", scope.1));
                self.restore_placement(entering_from_world);
                if let Some(origin) = ADOPT_ORIGIN.take() {
                    self.placement = Some(Placement::new(origin));
                    self.auto_place = false;
                    self.shown = true;
                    self.dirty = true;
                }
                self.last_label.clear();
                self.last_item_highlights.clear();
                self.highlight_dirty = true;
            }
            self.pose = Some(world);
        }

        let Ok(keys) = input::read_controls() else {
            label("Cinnamatica | Grant controls access in Settings > Mods.");
            return;
        };
        if !keys.focused {
            self.menu_armed = false;
            self.open_retries = 0;
            return;
        }
        if keys.panel_open || keys.keys_pressed.iter().any(|key| key == "Escape") {
            self.open_retries = 0;
        }
        if self.open_retries > 0 {
            self.open_retries -= 1;
            // Preparing a large panel can be time-sliced by the host. An open
            // request from a continued frame is stale, even if its panel committed.
            // Retry the inexpensive visibility write in a separate fresh callback.
            let _ = panel::open();
            return;
        }
        native_bindings::sample();
        let alt = keys
            .keys_held
            .iter()
            .any(|key| key == "AltLeft" || key == "AltRight");
        if keys.panel_open {
            self.menu_armed = false;
            // The menu key closes the window, except while it types into the search field.
            let editing = input::read_native_editing().unwrap_or(false);
            if !alt && !editing && native_bindings::menu_pressed(&keys) {
                let _ = panel::close();
                return;
            }
            let mut changed = keys.wheel != 0. && self.browser_scroll(keys.wheel < 0.);
            if let Some(tab) = self.open_tab.take() {
                self.browser_tab = tab;
                changed = true;
            }
            for event in input::read_field_events().unwrap_or_default() {
                if let input::FieldEvent::Change(text) | input::FieldEvent::Submit(text) = event {
                    changed |= self.browser_search_changed(&text.id, text.value);
                }
            }
            for event in keys.events {
                self.browser_event(&event.id, event.value);
                changed = true;
            }
            if changed && self.dirty {
                self.remember_placement();
            }
            // Loading a schematic closes the window so the guide can appear.
            if self.request.is_some() {
                let _ = panel::close();
                return;
            }
            // A refresh rebuilds every binding; hold it for a frame with fuel to spare.
            if changed || self.browser_refresh_pending || self.material_counts_pending() {
                self.browser_refresh_pending = budget::remaining_fuel() < REFRESH_FUEL;
                if !self.browser_refresh_pending {
                    menu::refresh(self);
                }
            }
            return;
        }
        let gameplay = active_world && keys.gameplay;
        // Opening a panel needs focused input, not a current placement snapshot.
        // The host withholds key edges while another screen owns input.
        if keys.keys_pressed.iter().any(|key| key == "Escape") {
            OPEN_AFTER_LOAD.set(false);
        }
        if OPEN_AFTER_LOAD.replace(false) || self.builder_hotkey(&keys, alt, gameplay) {
            if self.browser_tab == browser::Tab::Materials {
                self.browser_tab = browser::Tab::Import;
            }
            self.material_page = 0;
            *self.browser_counts.get_mut() = Default::default();
            // The window's search field opens empty.
            self.browser_search.clear();
            self.delete_confirm = None;
            if menu::prepare(self) {
                self.open_retries = 12;
                menu::request_open();
            }
            return;
        }
        let publish_highlights = self.highlight_dirty && !self.dirty && self.highlight_delay == 0;
        self.highlight_delay = self.highlight_delay.saturating_sub(1);
        if !gameplay {
            if publish_highlights {
                self.publish_item_highlights();
                self.highlight_delay = 5;
            }
            return;
        }
        self.paste_confirm = false;
        if let Some(scope) = self.cancel_structure.take()
            && self.world == Some(scope)
            && gameplay::request_command("/scriptevent cinnamatica:structure cancel")
                .is_err_and(|error| error == "command rate limit reached")
        {
            self.cancel_structure = Some(scope);
        }
        // Pasting and verification share the frame; never suspend world observations
        // until the entire command stream has finished.
        if self.tool_frame(&keys) {
            return;
        }
        let pasting = self.paste_step() || self.repair_step();
        self.export_step();
        if let Some((_, seconds)) = &mut self.paste_notice {
            *seconds -= keys.seconds;
            if *seconds <= 0.0 {
                self.paste_notice = None;
            }
        }
        let caps = drawing::capabilities();
        if !caps.contains(drawing::Permissions::WORLD_DRAW) {
            label("Cinnamatica | Grant world_draw in Settings > Mods.");
            return;
        }

        for (binding, command) in native_bindings::TRANSFORMS {
            if native_bindings::pressed(binding, &keys) {
                self.dirty |= self.control(command);
            }
        }
        if alt {
            for key in &keys.keys_pressed {
                self.dirty |= self.control(key);
            }
        }
        // Litematica's block info overlay: hold I to read the aimed planned block.
        if !alt
            && keys.keys_held.iter().any(|key| key == "KeyI")
            && let Some((block, pos)) = self.aimed_block()
        {
            self.paste_notice = Some((format!("{block} at {pos:?}"), 0.05));
        }
        if self.options.assist && alt && keys.keys_pressed.iter().any(|key| key == "KeyU") {
            let message = self.aimed_block().map_or_else(
                || "Cinnamatica | Aim at a planned block".into(),
                |(block, pos)| format!("Cinnamatica | {} at {pos:?}", block.name),
            );
            label(&message);
            return;
        }
        if self.auto_place {
            self.auto_place = false;
            self.dirty |= self.control("KeyP");
        }
        if self.dirty {
            self.remember_placement();
            self.rebuild();
            self.dirty = false;
            return;
        }
        self.update_preview_window();
        let world_target = gameplay::read_block_target()
            .ok()
            .flatten()
            .map(|pos| [pos.x, pos.y, pos.z].map(|axis| axis.floor() as i32));
        if world_target != self.last_target {
            if let Some(previous) = self.last_target {
                self.recently_targeted = Some((previous, 10));
            }
            self.last_target = world_target;
        }
        let recent = self.recently_targeted.map(|(pos, _)| pos);
        self.recently_targeted = self
            .recently_targeted
            .and_then(|(pos, frames)| (frames > 1).then_some((pos, frames - 1)));
        let aimed = self.aimed_next_ghost(world_target);
        let eye_cell = self
            .aim_ray()
            .map(|(eye, _)| eye.map(|axis| axis.floor() as i32));
        let inside_ghost = eye_cell
            .is_some_and(|cell| self.planned_block(cell).is_some() && self.may_need_block(cell));
        let use_held = keys.keys_held.iter().any(|key| key == "MouseRight")
            || native_bindings::is_held("use", &keys);
        let native_cell = item_use::read_placement_cell()
            .ok()
            .flatten()
            .map(|(x, y, z)| [x, y, z]);
        if self.lock_destination(native_cell) {
            let _ = item_use::set_placement_lock(true);
        }
        // Existing vegetation is a real ray obstruction. Offer that guided cell
        // before looking past it; the native predictor alone decides whether its
        // current block is replaceable and the requested plant can survive.
        let replacement_cell = world_target.filter(|cell| {
            self.tiles
                .iter()
                .find_map(|tile| tile.status_at(*cell))
                .is_some_and(|status| self.accepts_plant_replacement(*cell, status))
        });
        let placement_cell = if !pasting
            && !self.tool_active()
            && self.options.fast_place
            && self.shown
            && self.placement.is_some()
        {
            let native = native_cell
                .filter(|pos| self.planned_block(*pos).is_some() && self.may_need_block(*pos));
            if replacement_cell.is_some() {
                replacement_cell
            } else if self.options.hold_place {
                // A held click must acquire each new ghost on the current crosshair ray.
                // Nearby assist may choose an off-ray cell only in the single-click mode.
                aimed
            } else if self.options.strict_place || inside_ghost {
                aimed
                    .or_else(|| native.filter(|pos| Some(*pos) != eye_cell))
                    .or_else(|| {
                        self.options
                            .bridge_place
                            .then(|| self.bridge_ghost(world_target, use_held))
                            .flatten()
                    })
            } else {
                native.or_else(|| {
                    (self.options.correct_aim && self.options.bridge_place)
                        .then(|| self.bridge_ghost(world_target, use_held))
                        .flatten()
                })
            }
        } else {
            None
        };
        if let Some(placement) = self.placement
            && let Some((block, [x, y, z])) =
                placement_cell.and_then(|pos| self.placement_block(pos))
        {
            let block = unlit_variant(placement.transform_state(block));
            let states = overlay::canonical(&block);
            let offer = if self.options.strict_place {
                item_use::offer_strict_placement((x, y, z), &block.name, &states)
            } else if self.options.correct_aim || inside_ghost {
                item_use::offer_oriented_placement((x, y, z), &block.name, &states)
            } else {
                item_use::offer_creative_placement((x, y, z), &block.name, &states)
            };
            if let Err(error) = offer {
                self.show_label(&format!("Fast Place | {error}"));
                return;
            }
            let _ = item_use::set_placement_repeat(self.options.hold_place);
        }
        if publish_highlights && budget::remaining_fuel() >= 600_000 {
            self.publish_item_highlights();
            self.highlight_delay = 5;
            return;
        }
        if !self.retired.is_empty() {
            // A tile owns up to four handles. Stay under the host's 32 drawing
            // calls per callback even when moving or hiding a large guide.
            self.clear_retired();
            self.publish_label(placement_cell);
            return;
        }
        if !self.tiles.is_empty()
            && !self.preview.palette_ready(&self.schematic)
            && !self.preview.prepare(&self.schematic)
        {
            return;
        }
        let textured =
            self.options.textured && caps.contains(drawing::Permissions::RENDER_MATERIAL);
        if self.shown && !self.tiles.is_empty() {
            let lightness = f32::from(self.options.lightness) / 100.;
            let dimension = self.world.unwrap().1;
            if textured
                && !self
                    .whole
                    .prepare(&self.preview, &self.schematic, dimension, lightness)
            {
                self.show_label("Preparing full schematic preview");
                return;
            }
            let whole = self.whole.active();
            let tile_lightness = (textured && !whole).then_some(lightness);
            if whole && let Some(eye) = self.aim_ray().map(|(eye, _)| eye) {
                // Blocks placed or broken around the camera update their own ghost cell
                // and status box now, without waiting for their tile's turn.
                for cell in self.hot.step(
                    &self.preview,
                    &self.schematic,
                    &mut self.whole,
                    &self.tiles,
                    dimension,
                    eye,
                ) {
                    self.hot_dirty.retain(|(known, _)| *known != cell);
                    // Kept until the tile that holds it has been checked again, not for a few
                    // frames: the tile needs several to receive a new sample and draw it, and
                    // a change dropped before then waited for the slow lap round the guide.
                    self.hot_dirty.push((cell, 120));
                }
                let excess = self.hot_dirty.len().saturating_sub(16);
                self.hot_dirty.drain(..excess);
            }
            // The tile of the oldest unverified change is stepped first.
            let hot_cell = self.hot_dirty.first().map(|&(cell, _)| cell);
            for (_, frames) in &mut self.hot_dirty {
                *frames -= 1;
            }
            self.hot_dirty.retain(|&(_, frames)| frames > 0);
            // Alternate the hot tile and the background cursor. One tile can consume
            // most of a frame when many cells change or need discrepancy markers.
            let priority = [hot_cell, recent, world_target, aimed]
                .into_iter()
                .flatten()
                .find_map(|cell| self.tiles.iter().position(|tile| tile.contains(cell)));
            let hot_tile = hot_cell
                .and_then(|cell| self.tiles.iter().position(|tile| tile.contains(cell)));
            self.scan_priority = !self.scan_priority;
            // A tile with a block that changed under the player goes first on every frame,
            // until it has been checked again; otherwise it alternates with the lap.
            let index = if let Some(hot) = hot_tile {
                hot
            } else if self.scan_priority {
                priority.unwrap_or(self.cursor)
            } else {
                self.cursor
            };
            let mut changed = false;
            // Subscriptions deliver on the next callback. Give the distant
            // scan two consecutive frames without lowering a near region too.
            let distant = self.tile_focus.filter(|_| {
                whole
                    && hot_tile.is_none()
                    && overlay::on_distant_scan_frame(&mut self.distant_scan_phase)
            });
            if let Some(focus) = distant {
                if budget::remaining_fuel() >= 740_000 {
                    self.whole
                        .scan_distant(&self.preview, &self.schematic, focus);
                }
            } else if budget::remaining_fuel() >= 650_000 {
                let (advance, updated) = self.tiles[index].step(
                    &self.preview,
                    &self.schematic,
                    dimension,
                    tile_lightness,
                    whole,
                );
                changed = updated;
                if updated && let Some(hot) = hot_tile {
                    // Checked again. A few more frames cover a sample taken just before the
                    // block changed, then the changes it held stop waiting.
                    let tile = &self.tiles[hot];
                    for (cell, frames) in &mut self.hot_dirty {
                        if tile.contains(*cell) {
                            *frames = (*frames).min(6);
                        }
                    }
                }
                if self.tiles[index].ghost_dirty && whole && budget::remaining_fuel() >= 225_000 {
                    self.whole
                        .compare_tile(&self.preview, &self.schematic, &self.tiles[index]);
                    self.tiles[index].ghost_dirty = false;
                }
                if advance && index == self.cursor {
                    self.cursor = (self.cursor + 1) % self.tiles.len();
                }
                // Keep the two hot subscriptions warm using stamp-only reads. They
                // will be rendered in their own frame when there is enough fuel.
                let other = if index == self.cursor {
                    priority
                } else {
                    Some(self.cursor)
                };
                if let Some(next) = other.filter(|&next| next != index)
                    && budget::remaining_fuel() >= 500_000
                {
                    self.tiles[next].prefetch();
                }
            }
            // A tile that rendered while the frame was short of fuel still owes its ghost
            // changes; do not leave them until its next lap.
            if whole
                && budget::remaining_fuel() >= 450_000
                && let Some(owing) = self.tiles.iter().position(|tile| tile.ghost_dirty)
            {
                self.whole
                    .compare_tile(&self.preview, &self.schematic, &self.tiles[owing]);
                self.tiles[owing].ghost_dirty = false;
            }
            if whole && budget::remaining_fuel() >= 400_000 {
                self.whole.flush();
                if self.whole.failed {
                    for tile in &mut self.tiles {
                        tile.stamp = None;
                    }
                }
            }
            if changed {
                self.highlight_dirty = true;
                self.highlight_again |= self.highlight_scan.is_some();
                let [width, height, depth] = self.schematic.size();
                if self.options.auto_layer
                    && self.layer_mode == 1
                    && self.layer + 1 < height
                    && overlay::layer_complete(&self.tiles, width, depth)
                {
                    self.layer += 1;
                    self.dirty = true;
                }
            }
        }
        if pasting {
            // Keep progress/cancellation feedback produced by paste_step.
        } else if self.options.textured && !textured && self.placement.is_some() {
            self.show_label("Cinnamatica | Textures need render_material");
        } else {
            self.publish_label(placement_cell);
        }
    }

    /// Resolves Litematica's M chords: the menu opens when M is released on its own, and
    /// M+T, M+R, M+G, M+L, M+P, M+C, M+V and M+Page Up/Down run their feature instead.
    /// Runs a pressed gameplay binding; returns whether it asks to open the window.
    fn builder_hotkey(&mut self, keys: &input::Controls, alt: bool, gameplay: bool) -> bool {
        use native_bindings::Action;
        if self.menu_armed {
            let main = native_bindings::main_key();
            if keys.keys_held.iter().any(|key| *key == main) {
                for key in &keys.keys_pressed {
                    if *key == main || !gameplay {
                        continue;
                    }
                    match self.menu_chord(key) {
                        Chord::None => {}
                        Chord::Done => self.menu_chorded = true,
                        Chord::Open(tab) => {
                            self.menu_armed = false;
                            self.open_tab = Some(tab);
                            self.tool_reserve_for_menu();
                            return true;
                        }
                    }
                }
                return false;
            }
            self.menu_armed = false;
            self.tool_reserve_for_menu();
            return !std::mem::take(&mut self.menu_chorded);
        }
        let modified = alt
            || ["Control", "Shift", "Super"]
                .iter()
                .any(|modifier| keys.keys_held.iter().any(|key| key.starts_with(modifier)));
        let action = native_bindings::action(keys, &mut false);
        if modified {
            // The tool rows may carry modifiers; the menu key keeps its chords plain.
            match action {
                Some(Action::Tool) if gameplay => self.toggle_tool(),
                Some(Action::Execute) if gameplay => self.tool_execute(),
                Some(Action::ToolNext) if gameplay && self.options.tool_item => {
                    self.tool.cycle_mode(true);
                    self.last_label.clear();
                }
                Some(Action::ToolPrevious) if gameplay && self.options.tool_item => {
                    self.tool.cycle_mode(false);
                    self.last_label.clear();
                }
                _ => {}
            }
            return false;
        }
        match action {
            Some(Action::Main) => {
                self.menu_armed = true;
                self.menu_chorded = false;
                self.tool_reserve_for_menu();
                return false;
            }
            None => return false,
            _ if !gameplay => return false,
            Some(Action::Render) => self.toggle_render_all(),
            Some(Action::Ghosts) => self.dirty |= self.control("KeyB"),
            Some(Action::Tool) => self.toggle_tool(),
            Some(Action::Execute) => self.tool_execute(),
            Some(Action::ToolNext | Action::ToolPrevious) if self.options.tool_item => {
                self.tool.cycle_mode(action == Some(Action::ToolNext));
                self.last_label.clear();
            }
            Some(Action::ToolNext | Action::ToolPrevious) => {}
            Some(Action::NextMode) => self.dirty |= self.control("KeyL"),
            Some(Action::PreviousMode) => {
                self.layer_mode = (self.layer_mode + 2) % 3;
                self.dirty = true;
            }
            Some(Action::NextLayer) => self.dirty |= self.control("BracketRight"),
            Some(Action::PreviousLayer) => self.dirty |= self.control("BracketLeft"),
        }
        false
    }

    fn toggle_render_all(&mut self) {
        self.render_all = !self.render_all;
        self.dirty = true;
        self.last_label.clear();
        if !self.render_all {
            label("");
        }
    }

    /// One key pressed while the menu key is held, with Litematica's default meanings.
    fn menu_chord(&mut self, key: &str) -> Chord {
        match key {
            "KeyT" => self.toggle_tool(),
            "KeyR" => self.toggle_render_all(),
            "KeyG" => self.dirty |= self.control("KeyH"),
            "PageUp" => self.dirty |= self.control("KeyL"),
            "PageDown" => {
                self.layer_mode = (self.layer_mode + 2) % 3;
                self.dirty = true;
            }
            "KeyL" | "KeyV" => return Chord::Open(browser::Tab::Materials),
            "KeyP" => return Chord::Open(browser::Tab::Controls),
            "KeyC" => return Chord::Open(browser::Tab::Settings),
            "KeyS" => return Chord::Open(browser::Tab::Import),
            _ => return Chord::None,
        }
        Chord::Done
    }

    fn control(&mut self, key: &str) -> bool {
        match key {
            "KeyP" => {
                if !self.loaded || self.locked {
                    return false;
                }
                let Some(frame) = &self.pose else {
                    return false;
                };
                let yaw = frame.yaw.rem_euclid(TAU);
                let [width, _, depth] = self.placement.map_or(self.schematic.size(), |p| {
                    p.footprint(self.schematic.size())
                });
                let distance = (width.max(depth) as f32 / 2. + 3.).min(36.);
                let origin = [
                    (frame.eye.x - yaw.sin() * distance).floor() as i32 - width / 2,
                    (frame.eye.y - 1.62).round() as i32,
                    (frame.eye.z - yaw.cos() * distance).floor() as i32 - depth / 2,
                ];
                self.placement
                    .get_or_insert_with(|| Placement::new(origin))
                    .origin = origin;
                self.shown = true;
            }
            "KeyH" => self.shown = !self.shown,
            "KeyB" => {
                self.options.textured = !self.options.textured;
                self.options.save();
            }
            "KeyL" => self.layer_mode = (self.layer_mode + 1) % 3,
            "BracketLeft" => {
                if self.layer_mode == 0 {
                    self.layer_mode = 1;
                }
                self.layer = (self.layer - 1).max(0);
            }
            "BracketRight" => {
                if self.layer_mode == 0 {
                    self.layer_mode = 1;
                }
                self.layer = (self.layer + 1).min(self.schematic.size()[1] - 1);
            }
            _ => {
                if self.locked {
                    return false;
                }
                let Some(p) = &mut self.placement else {
                    return false;
                };
                match key {
                    "ViewForward" | "ViewLeft" | "ViewRight" | "ViewBack" => {
                        let Some(view) = &self.pose else { return false };
                        let [x, z] = view_step(view.yaw, key);
                        p.origin[0] = (p.origin[0] + x).clamp(-30_000_000, 30_000_000);
                        p.origin[2] = (p.origin[2] + z).clamp(-30_000_000, 30_000_000);
                    }
                    "KeyR" => p.rotate_cw(),
                    "RotateLeft" => p.rotate_ccw(),
                    "KeyM" => {
                        p.mirror = match p.mirror {
                            Mirror::None => Mirror::X,
                            Mirror::X => Mirror::Z,
                            Mirror::Z => Mirror::None,
                        }
                    }
                    "ArrowLeft" => p.origin[0] -= 1,
                    "ArrowRight" => p.origin[0] += 1,
                    "ArrowUp" => p.origin[2] -= 1,
                    "ArrowDown" => p.origin[2] += 1,
                    "PageUp" => p.origin[1] += 1,
                    "PageDown" => p.origin[1] -= 1,
                    _ => return false,
                }
            }
        }
        true
    }

    fn move_here(&mut self) -> bool {
        if !self.loaded || self.locked {
            return false;
        }
        let Some(pose) = &self.pose else { return false };
        let placement = self.placement.get_or_insert_with(|| Placement::new([0; 3]));
        let [width, _, depth] = placement.footprint(self.schematic.size());
        let quadrant = ((180. - pose.yaw.to_degrees()) / 90.).floor() as i32 & 3;
        let (dx, dz) = [(-width, 1), (-width, -depth), (1, -depth), (1, 1)][quadrant as usize];
        placement.origin = [
            pose.eye.x.floor() as i32 + dx,
            (pose.eye.y - 1.62).round() as i32,
            pose.eye.z.floor() as i32 + dz,
        ];
        self.shown = true;
        true
    }

    fn planned_block(&self, world: [i32; 3]) -> Option<&schematica_core::BlockState> {
        if !self.shown {
            return None;
        }
        let local = self
            .placement?
            .world_to_local(self.schematic.size(), world)?;
        let block = self.schematic.get(local)?;
        (self.layers().includes(local[1]) && !block.is_air() && !block.is_placement_artifact())
            .then_some(block)
    }

    /// A paired item (door or tall plant) starts from its lower ghost cell.
    fn placement_block(&self, pos: [i32; 3]) -> Option<(&schematica_core::BlockState, [i32; 3])> {
        use schematica_core::StateValue;
        let block = self.planned_block(pos)?;
        // A bed's head is placed by its foot; the head cell offers nothing.
        if matches!(
            block.states.get("head_piece_bit"),
            Some(StateValue::Bool(true) | StateValue::Byte(1) | StateValue::Int(1))
        ) {
            return None;
        }
        if !matches!(
            block.states.get("upper_block_bit"),
            Some(StateValue::Bool(true) | StateValue::Byte(1) | StateValue::Int(1))
        ) {
            return Some((block, pos));
        }
        let [x, y, z] = pos;
        let lower = [x, y - 1, z];
        let local = self
            .placement?
            .world_to_local(self.schematic.size(), lower)?;
        let base = self.schematic.get(local)?;
        (base.name == block.name
            && matches!(
                base.states.get("upper_block_bit"),
                Some(StateValue::Bool(false) | StateValue::Byte(0) | StateValue::Int(0))
            ))
        .then_some((base, lower))
    }

    fn aim_ray(&self) -> Option<([f32; 3], [f32; 3])> {
        let pose = self.pose.as_ref()?;
        if !self.shown || self.placement.is_none() {
            return None;
        }
        let forward = [
            -pose.yaw.sin() * pose.pitch.cos(),
            pose.pitch.sin(),
            -pose.yaw.cos() * pose.pitch.cos(),
        ];
        Some(([pose.eye.x, pose.eye.y, pose.eye.z], forward))
    }

    fn aimed_block(&self) -> Option<(&schematica_core::BlockState, [i32; 3])> {
        let (eye, forward) = self.aim_ray()?;
        for step in 0..=80 {
            let world =
                [0, 1, 2].map(|i| (eye[i] + forward[i] * (step as f32 / 16.)).floor() as i32);
            if let Some(block) = self.planned_block(world) {
                return Some((block, world));
            }
        }
        None
    }

    fn is_missing(&self, pos: [i32; 3]) -> bool {
        self.tiles.iter().any(|tile| tile.is_missing(pos))
    }

    fn may_need_block(&self, pos: [i32; 3]) -> bool {
        use schematica_core::CellStatus;
        // Liquids have no item to place, so a held click on one only makes the client refuse
        // it again and again.
        !self
            .planned_block(pos)
            .is_some_and(|block| unplaceable(&block.name))
            && matches!(
                self.tiles.iter().find_map(|tile| tile.status_at(pos)),
                None | Some(CellStatus::Missing | CellStatus::Unloaded)
            )
    }

    /// Lock only ordinary placement into planned blocks, leaving the rest of the world usable.
    fn lock_destination(&self, cell: Option<[i32; 3]>) -> bool {
        self.options.placement_lock
            && self.options.fast_place
            && self.shown
            && self.render_all
            && cell.is_some_and(|cell| self.planned_block(cell).is_some())
    }

    fn accepts_plant_replacement(
        &self,
        cell: [i32; 3],
        status: schematica_core::CellStatus,
    ) -> bool {
        status == schematica_core::CellStatus::Wrong
            && self
                .planned_block(cell)
                .is_some_and(|block| SOIL_PLANT_IDENTIFIERS.contains(&block.name.as_str()))
    }

    fn aimed_next_ghost(&self, obstruction: Option<[i32; 3]>) -> Option<[i32; 3]> {
        let (eye, forward) = self.aim_ray()?;
        next_guide_cell(eye, forward, 6., |cell| {
            Some(cell) == obstruction
                || (self.planned_block(cell).is_some_and(|block| {
                    // A thin block only counts where the ray really crosses it, so aiming
                    // through the gap under a trapdoor or past a pane reaches what is behind.
                    shapes::selection(block)
                        .is_none_or(|shape| shapes::ray_hits(eye, forward, cell, &shape, 6.))
                }) && self.may_need_block(cell))
        })
        .filter(|cell| Some(*cell) != obstruction)
    }

    /// Finds the nearest missing guide cell within the crosshair's placement area.
    fn bridge_ghost(&mut self, support: Option<[i32; 3]>, use_held: bool) -> Option<[i32; 3]> {
        let ray = self.aim_ray()?;
        if let Some((old_ray, old_support, cell)) = self.nearby_aim
            && old_ray == ray
            && old_support == support
            && cell.is_none_or(|pos| self.is_missing(pos))
        {
            return cell;
        }
        self.nearby_aim = None;
        if !use_held || budget::remaining_fuel() < 750_000 {
            return None;
        }
        let (eye, forward) = ray;
        let center = support.map(|pos| pos.map(|v| v as f32 + 0.5));
        let cell = nearby_ghost(eye, forward, center, |pos| {
            self.planned_block(pos).is_some() && self.is_missing(pos)
        });
        self.nearby_aim = Some((ray, support, cell));
        cell
    }

    fn layers(&self) -> LayerMode {
        match self.layer_mode {
            1 => LayerMode::Single(self.layer),
            2 => LayerMode::Range {
                min: 0,
                max: self.layer,
            },
            _ => LayerMode::All,
        }
    }

    fn layers_text(&self) -> String {
        let height = self.schematic.size()[1];
        match self.layers() {
            LayerMode::All => format!("All {height} layers"),
            LayerMode::Single(y) => format!("Layer {} of {height}", y + 1),
            LayerMode::Range { min, max } => format!("Layers {}-{} of {height}", min + 1, max + 1),
        }
    }

    fn rebuild(&mut self) {
        self.hot.clear();
        self.hot_dirty.clear();
        self.whole.restart(
            self.placement.is_some() && self.shown && self.render_all && self.options.textured,
        );
        self.nearby_aim = None;
        self.layer_counts.clear();
        self.layer_count_cursor = 0;
        self.highlight_scan = None;
        self.highlight_again = false;
        self.highlight_delay = 0;
        self.highlight_dirty = true;
        self.retired
            .extend(self.tiles.drain(..).filter(Tile::has_draws));
        self.cursor = 0;
        if let Some(p) = self.placement.filter(|_| self.shown && self.render_all) {
            self.preview.reposition(p, self.layers());
            self.tile_focus = self.preview_focus();
            self.tiles = self.preview.tiles(self.schematic.size(), self.tile_focus);
            self.sort_tiles_near_camera();
        }
    }

    fn sort_tiles_near_camera(&mut self) {
        if let Some(pose) = &self.pose {
            let eye = [pose.eye.x, pose.eye.y, pose.eye.z];
            self.tiles
                .sort_by(|a, b| a.distance_squared(eye).total_cmp(&b.distance_squared(eye)));
        }
    }

    /// Publishes required item names for the selected layers, less verified placed cells.
    fn publish_item_highlights(&mut self) {
        let mut names = Vec::new();
        if self.loaded && self.placement.is_some() {
            if self.layers() == LayerMode::All
                && self.layer_counts.is_empty()
                && self.last_item_highlights.is_empty()
            {
                // Material totals are decoded before placement. Publish them first so
                // opening a container before terrain verification still shows matches.
                let mut all = self
                    .materials
                    .iter()
                    .map(|line| line.name.clone())
                    .collect::<Vec<_>>();
                all.sort_unstable();
                all.dedup();
                all.truncate(1024);
                if !all.is_empty() && hud::set_item_highlights(&all).is_ok() {
                    self.last_item_highlights = all;
                }
                return;
            }
            if self.layer_counts.is_empty() {
                self.layer_counts.resize(self.schematic.palette().len(), 0);
            }
            let cells = self.schematic.layer_cells(self.layers());
            let end = (self.layer_count_cursor + 2048).min(cells.len());
            for &index in &cells[self.layer_count_cursor..end] {
                self.layer_counts[index as usize] += 1;
            }
            self.layer_count_cursor = end;
            if end < cells.len() {
                return;
            }
            let (cursor, needed) = self
                .highlight_scan
                .get_or_insert_with(|| (0, self.layer_counts.clone()));
            let end = (*cursor + 4).min(self.tiles.len());
            for tile in &self.tiles[*cursor..end] {
                tile.subtract_placed_materials(needed);
            }
            *cursor = end;
            if end < self.tiles.len() {
                return;
            }
            let (_, needed) = self.highlight_scan.take().unwrap();
            names.extend(needed.into_iter().enumerate().filter_map(|(index, count)| {
                let block = &self.schematic.palette()[index];
                (count > 0 && !block.is_air() && !block.is_placement_artifact())
                    .then(|| block.name.clone())
            }));
            names.sort_unstable();
            names.dedup();
            names.truncate(1024);
        }
        if names != self.last_item_highlights {
            if hud::set_item_highlights(&names).is_err() {
                return;
            }
            self.last_item_highlights = names;
        }
        self.highlight_dirty = std::mem::take(&mut self.highlight_again);
    }

    fn preview_focus(&self) -> Option<[i32; 3]> {
        let eye = self.pose.as_ref().map_or_else(
            || self.placement.unwrap().origin.map(|axis| axis as f32),
            |pose| [pose.eye.x, pose.eye.y, pose.eye.z],
        );
        self.preview.focus(self.schematic.size(), eye)
    }

    fn update_preview_window(&mut self) {
        if self.placement.is_none() || !self.shown || !self.render_all {
            return;
        }
        let focus = self.preview_focus();
        if focus == self.tile_focus {
            return;
        }
        self.tile_focus = focus;
        self.highlight_scan = None;
        self.highlight_dirty = true;
        let mut previous: BTreeMap<_, _> = self
            .tiles
            .drain(..)
            .map(|tile| (tile.origin(), tile))
            .collect();
        self.tiles = self
            .preview
            .tiles(self.schematic.size(), focus)
            .into_iter()
            .map(|tile| previous.remove(&tile.origin()).unwrap_or(tile))
            .collect();
        self.retired
            .extend(previous.into_values().filter(Tile::has_draws));
        self.cursor = 0;
        self.sort_tiles_near_camera();
    }

    fn clear_retired(&mut self) {
        for _ in 0..6 {
            let Some(mut tile) = self.retired.pop() else {
                break;
            };
            tile.clear();
        }
    }

    fn release_for_switch(&mut self) -> bool {
        self.hot.clear();
        self.hot_dirty.clear();
        self.whole.clear();
        self.retired
            .extend(self.tiles.drain(..).filter(Tile::has_draws));
        self.clear_retired();
        self.retired.is_empty()
    }
}

/// One horizontal grid step relative to the nearest cardinal camera heading.
fn view_step(yaw: f32, direction: &str) -> [i32; 2] {
    let quadrant = (yaw / std::f32::consts::FRAC_PI_2).round() as i32;
    let [x, z] = [[0, -1], [-1, 0], [0, 1], [1, 0]][quadrant.rem_euclid(4) as usize];
    match direction {
        "ViewLeft" => [z, -x],
        "ViewRight" => [-z, x],
        "ViewBack" => [-x, -z],
        _ => [x, z],
    }
}

/// Walk voxel crossings after the camera cell. The host validates current terrain and reach.
fn next_guide_cell(
    eye: [f32; 3],
    ray: [f32; 3],
    reach: f32,
    mut accepts: impl FnMut([i32; 3]) -> bool,
) -> Option<[i32; 3]> {
    let mut cell = eye.map(|value| value.floor() as i32);
    let steps = ray.map(|value| {
        if value > 0. {
            1
        } else if value < 0. {
            -1
        } else {
            0
        }
    });
    let delta = ray.map(|value| {
        if value == 0. {
            f32::INFINITY
        } else {
            1. / value.abs()
        }
    });
    let mut next: [f32; 3] = std::array::from_fn(|axis| match steps[axis] {
        1 => (cell[axis] as f32 + 1. - eye[axis]) * delta[axis],
        -1 => (eye[axis] - cell[axis] as f32) * delta[axis],
        _ => f32::INFINITY,
    });
    loop {
        let crossing = next[0].min(next[1]).min(next[2]);
        if !crossing.is_finite() || crossing > reach {
            return None;
        }
        for axis in 0..3 {
            if next[axis] == crossing {
                cell[axis] += steps[axis];
                next[axis] += delta[axis];
            }
        }
        if accepts(cell) {
            return Some(cell);
        }
    }
}

/// Searches a bounded corridor around the crosshair, ranked by distance to the ray.
fn nearby_ghost(
    eye: [f32; 3],
    forward: [f32; 3],
    support: Option<[f32; 3]>,
    mut is_missing: impl FnMut([i32; 3]) -> bool,
) -> Option<[i32; 3]> {
    const CENTER_REACH: f32 = 6.2;
    let mut best = None;
    let desired_depth = support.map_or(0.0, |p| {
        (0..3).map(|i| (p[i] - eye[i]) * forward[i]).sum::<f32>() + 1.0
    });
    let mut score = (25.0, f32::INFINITY);
    let center = eye.map(|coord| coord.floor() as i32);
    for x in -6..=6 {
        for y in -6..=6 {
            for z in -6..=6 {
                let pos = [center[0] + x, center[1] + y, center[2] + z];
                let delta = [0, 1, 2].map(|i| (f64::from(pos[i]) + 0.5 - f64::from(eye[i])) as f32);
                let distance = delta.iter().map(|d| d * d).sum::<f32>();
                let depth = (0..3).map(|i| delta[i] * forward[i]).sum::<f32>();
                if depth < 0.0 || distance > CENTER_REACH * CENTER_REACH {
                    continue;
                }
                let offset = (0..3)
                    .map(|i| (delta[i] - depth * forward[i]).powi(2))
                    .sum();
                let candidate = (offset, (depth - desired_depth).abs());
                if candidate < score && is_missing(pos) {
                    score = candidate;
                    best = Some(pos);
                }
            }
        }
    }
    best
}

/// Blocks that have no item to place: liquids and technical blocks. They are never offered,
/// because the client cannot find an item for them.
pub(crate) fn unplaceable(name: &str) -> bool {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    matches!(
        name,
        "water"
            | "flowing_water"
            | "lava"
            | "flowing_lava"
            | "air"
            | "bubble_column"
            | "fire"
            | "soul_fire"
            | "moving_block"
            | "piston_arm_collision"
            | "sticky_piston_arm_collision"
            | "portal"
            | "end_portal"
            | "end_gateway"
            | "structure_void"
    )
}

/// Powered and lit blocks cannot be held: the item places the unpowered, unlit block and the
/// world powers it. Offer that block, so a comparator that is receiving a signal, or a lit
/// furnace, can be placed at all.
fn unlit_variant(mut block: schematica_core::BlockState) -> schematica_core::BlockState {
    let Some(name) = unlit_name(&block.name) else {
        return block;
    };
    block.name = name.to_owned();
    block.states.remove("output_lit_bit");
    block.states.remove("output_subtract_bit");
    block.states.retain(|key, _| key != "powered_bit");
    block
}

/// The unpowered, unlit block a powered or lit one is placed as, if it has one.
pub(crate) fn unlit_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "minecraft:powered_comparator" => "minecraft:unpowered_comparator",
        "minecraft:powered_repeater" => "minecraft:unpowered_repeater",
        "minecraft:lit_furnace" => "minecraft:furnace",
        "minecraft:lit_blast_furnace" => "minecraft:blast_furnace",
        "minecraft:lit_smoker" => "minecraft:smoker",
        "minecraft:lit_redstone_lamp" => "minecraft:redstone_lamp",
        "minecraft:lit_redstone_ore" => "minecraft:redstone_ore",
        "minecraft:lit_deepslate_redstone_ore" => "minecraft:deepslate_redstone_ore",
        // The redstone torch item places a lit torch; the world turns it off when powered.
        "minecraft:unlit_redstone_torch" => "minecraft:redstone_torch",
        _ => return None,
    })
}

#[cfg(test)]
mod tests;

// Generated from the native predictor, so target selection cannot silently drift
// into offering wrong solid blocks or omitting a supported plant.
include!(concat!(env!("OUT_DIR"), "/soil_plants.rs"));
include!(concat!(env!("OUT_DIR"), "/world_states.rs"));
