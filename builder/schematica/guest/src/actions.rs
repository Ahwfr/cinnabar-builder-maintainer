use crate::*;

impl Viewer {
    pub(super) fn menu_action(&mut self, id: &str) {
        if let Some(row) = id
            .strip_prefix("delete-")
            .and_then(|value| value.parse().ok())
        {
            self.delete_entry(row);
            return;
        }
        if let Some(index) = id
            .strip_prefix("toggle-")
            .and_then(|value| value.parse::<usize>().ok())
        {
            let Some(index) = self.visible_entry(index) else {
                return;
            };
            if self.active_entry == Some(index) && self.loaded && self.placement.is_some() {
                self.shown = !self.shown;
                self.dirty = true;
            } else if index < self.library.len() {
                self.selected = Some(index);
                self.request = Some(index);
            }
            return;
        }
        if let Some(index) = id
            .strip_prefix("unload-")
            .and_then(|value| value.parse::<usize>().ok())
        {
            if self.active_entry == Some(index) && !self.locked {
                self.remember_unload();
                self.placement = None;
                self.active_entry = None;
                self.loaded = false;
                self.dirty = true;
            }
            return;
        }
        if let Some(row) = id
            .strip_prefix("verify-row-")
            .and_then(|value| value.parse::<usize>().ok())
        {
            if row < menu::ROWS && !self.dirty {
                let index = self.verifier_page * menu::ROWS + row;
                let cell = self.mismatches().nth(index).map(|&(cell, _)| cell);
                if let Some(cell) = cell {
                    self.verifier_target = Some(cell);
                    self.verifier_marker_changed = true;
                    let _ = panel::close();
                }
            }
            return;
        }
        if let Some(index) = id
            .strip_prefix("select-")
            .and_then(|value| value.parse::<usize>().ok())
        {
            let Some(index) = self.visible_entry(index) else {
                return;
            };
            if index < self.library.len() {
                self.selected = Some(index);
                if self.active_entry != Some(index) {
                    self.request = Some(index);
                }
            }
            return;
        }
        let key = match id {
            "main" => {
                self.screen = Screen::Main;
                return;
            }
            "hotkeys" => {
                let _ = panel::open_settings("keyboard_and_mouse_forced_index");
                return;
            }
            "import" => {
                if let Err(error) = panel::open_settings("schematic_import") {
                    label(&error);
                }
                return;
            }
            "reset-hotkeys" => {
                self.options.hotkeys.clear();
                self.options.save();
                return;
            }
            "verifier" => {
                self.screen = Screen::Verifier;
                self.verifier_page = 0;
                return;
            }
            "lock" => {
                if self.placement.is_some() {
                    self.locked = !self.locked;
                }
                return;
            }
            "opacity" => {
                self.options.opacity = match self.options.opacity {
                    0..=25 => 45,
                    26..=45 => 65,
                    46..=65 => 90,
                    _ => 25,
                };
                self.options.save();
                self.dirty = true;
                return;
            }
            "verify-previous" => {
                self.verifier_page = self.verifier_page.saturating_sub(1);
                return;
            }
            "verify-next" => {
                self.verifier_page = (self.verifier_page + 1)
                    .min(self.mismatch_count().div_ceil(menu::ROWS).saturating_sub(1));
                return;
            }
            "verify-clear" => {
                self.verifier_target = None;
                self.verifier_marker_changed = true;
                return;
            }
            "place" => {
                self.dirty |= self.move_here();
                return;
            }
            "load" => {
                if let Some(index) = self.selected {
                    self.request = Some(index);
                    let _ = panel::close();
                    label("Schematica | Loading selected blueprint");
                }
                return;
            }
            "library" => {
                self.screen = Screen::Load;
                self.selected = None;
                return;
            }
            "settings" => {
                self.screen = Screen::Settings;
                return;
            }
            "assist" => {
                self.options.assist = !self.options.assist;
                if !self.options.assist {
                    self.show_label("");
                }
                self.options.save();
                return;
            }
            "fast-place" => {
                (self.options.fast_place, self.options.strict_place) =
                    match (self.options.fast_place, self.options.strict_place) {
                        (false, _) => (true, false),
                        (true, false) => (true, true),
                        (true, true) => (false, false),
                    };
                self.options.save();
                return;
            }
            "paste" => {
                self.paste_button();
                return;
            }
            "correct-aim" => {
                self.options.correct_aim = !self.options.correct_aim;
                self.options.save();
                return;
            }
            "ui-layout" => {
                self.options.fullscreen_ui = !self.options.fullscreen_ui;
                self.options.save();
                menu::prepare(self);
                return;
            }
            "hold-place" => {
                self.options.hold_place = !self.options.hold_place;
                self.options.save();
                return;
            }
            "auto-layer" => {
                self.options.auto_layer = !self.options.auto_layer;
                self.options.save();
                return;
            }
            "paste-destroy" => {
                self.options.paste_destroy = !self.options.paste_destroy;
                self.paste_confirm = false;
                self.options.save();
                return;
            }
            "bridge-place" => {
                self.options.bridge_place = !self.options.bridge_place;
                self.options.save();
                return;
            }
            "remember-placement" => {
                self.options.remember_placement = !self.options.remember_placement;
                if self.placement.is_some() {
                    self.remember_placement();
                }
                self.options.save();
                return;
            }
            "unload" => {
                if self.locked {
                    return;
                }
                self.remember_unload();
                self.placement = None;
                self.active_entry = None;
                self.loaded = false;
                self.auto_place = false;
                self.dirty = true;
                let _ = panel::close();
                return;
            }
            "materials" => {
                self.screen = Screen::Materials;
                self.material_page = 0;
                return;
            }
            "back" => {
                self.screen = Screen::Control;
                return;
            }
            "sort" => {
                self.sort_name = !self.sort_name;
                if self.sort_name {
                    self.materials.sort_by(|a, b| a.name.cmp(&b.name));
                } else {
                    self.materials
                        .sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.name.cmp(&b.name)));
                }
                self.material_page = 0;
                return;
            }
            "flip-axis" => {
                self.flip_z = !self.flip_z;
                return;
            }
            "rotate-axis" => {
                self.rotate_ccw = !self.rotate_ccw;
                return;
            }
            "rotate" if self.rotate_ccw => "RotateLeft",
            "rotate" => "KeyR",
            "cw" => "KeyR",
            "ccw" => "RotateLeft",
            "x-minus" => "ArrowLeft",
            "x-plus" => "ArrowRight",
            "y-minus" => "PageDown",
            "y-plus" => "PageUp",
            "z-minus" => "ArrowUp",
            "z-plus" => "ArrowDown",
            "mirror-cycle" => "KeyM",
            "materials-up" | "materials-down" => {
                self.scroll_materials(if id == "materials-up" { -1 } else { 1 });
                return;
            }
            "mirror" => {
                if !self.locked
                    && let Some(p) = &mut self.placement
                {
                    // The button's axis is in world space. The core mirrors
                    // before rotating, so swap local axes on odd turns.
                    let flip = if self.flip_z ^ (p.quarter_turns % 2 != 0) {
                        Mirror::Z
                    } else {
                        Mirror::X
                    };
                    if p.mirror == Mirror::None {
                        p.mirror = flip;
                    } else {
                        if p.mirror != flip {
                            p.quarter_turns = (p.quarter_turns + 2) % 4;
                        }
                        p.mirror = Mirror::None;
                    }
                    self.dirty = true;
                }
                return;
            }
            "mode" => "KeyL",
            "previous-layer" => "BracketLeft",
            "next-layer" => "BracketRight",
            "visibility" => "KeyH",
            "look" => "KeyB",
            "previous-page" => {
                self.material_page = self.material_page.saturating_sub(1);
                return;
            }
            "next-page" => {
                let last = self.materials.len().div_ceil(menu::ROWS).saturating_sub(1);
                self.material_page = (self.material_page + 1).min(last);
                return;
            }
            "close" => {
                let _ = panel::close();
                return;
            }
            _ => return,
        };
        self.dirty |= self.control(key);
    }
}
