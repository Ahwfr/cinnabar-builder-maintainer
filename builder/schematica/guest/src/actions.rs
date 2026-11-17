use crate::*;

impl Viewer {
    /// Runs one window control: a library row, a setting, or a placement action.
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
                self.request = Some(index);
            }
            return;
        }
        let key = match id {
            "import" => {
                if let Err(error) = panel::open_settings("schematic_import") {
                    label(&error);
                }
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
                return;
            }
            "place" => {
                self.dirty |= self.move_here();
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
            "paste" => {
                self.paste_button();
                return;
            }
            "correct-aim" => {
                self.options.correct_aim = !self.options.correct_aim;
                self.options.save();
                return;
            }
            "placement-lock" => {
                self.options.placement_lock = !self.options.placement_lock;
                self.options.save();
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
            "mirror" => {
                if !self.locked
                    && let Some(p) = &mut self.placement
                {
                    // The button mirrors along world Z. The core mirrors before rotating,
                    // so swap local axes on odd turns.
                    let flip = if p.quarter_turns % 2 == 0 {
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
            "cw" => "KeyR",
            "ccw" => "RotateLeft",
            "mode" => "KeyL",
            "previous-layer" => "BracketLeft",
            "next-layer" => "BracketRight",
            "visibility" => "KeyH",
            "look" => "KeyB",
            _ => return,
        };
        self.dirty |= self.control(key);
    }
}
