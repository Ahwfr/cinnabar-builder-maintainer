use crate::*;

impl Viewer {
    pub(super) fn show_label(&mut self, message: &str) {
        if self.render_all && message != self.last_label {
            label(message);
            self.last_label = message.to_owned();
        }
    }

    pub(super) fn publish_label(&mut self, placement_cell: Option<[i32; 3]>) {
        if self.paste.is_some() {
            return;
        }
        if let Some((message, _)) = &self.paste_notice {
            self.show_label(&message.clone());
            return;
        }
        if let Some(text) = self.tool_label() {
            self.show_label(&text);
            return;
        }
        if !self.options.assist {
            self.show_label("");
            return;
        }
        let target = placement_cell
            .filter(|pos| self.tiles.iter().any(|tile| tile.is_missing(*pos)))
            .and_then(|pos| self.planned_block(pos).map(|block| (block, pos, true)))
            .or_else(|| self.aimed_block().map(|(block, pos)| (block, pos, false)));
        if let Some((block, [x, y, z], fast)) = target {
            let name = block.name.strip_prefix("minecraft:").unwrap_or(&block.name);
            let name = name.replace('_', " ");
            let action = if fast && self.options.hold_place {
                "  |  Hold right-click"
            } else if fast {
                "  |  Right-click"
            } else {
                ""
            };
            // What the guide's scan reads in the cell, so a ghost that will not go away or a
            // chest it cannot see can be diagnosed from the aim label.
            let world = self
                .tiles
                .iter()
                .find_map(|tile| tile.status_at([x, y, z]))
                .map_or("outside", |status| match status {
                    schematica_core::CellStatus::Correct => "built",
                    schematica_core::CellStatus::Missing => "empty",
                    schematica_core::CellStatus::Wrong => "other block",
                    schematica_core::CellStatus::WrongState => "wrong state",
                    schematica_core::CellStatus::Extra => "extra",
                    schematica_core::CellStatus::Unloaded => "not loaded",
                });
            self.show_label(&format!(
                "{name}  |  {x}, {y}, {z}  |  world: {world}{action}"
            ));
            return;
        }
        let pending = self
            .tiles
            .iter()
            .filter(|tile| tile.stamp.is_none())
            .count();
        let message = if let Some(p) = self.placement {
            let needed: u64 = self.tiles.iter().map(|t| t.tally.missing).sum();
            let wrong: u64 = self.tiles.iter().map(|t| t.tally.wrong).sum();
            let extra: u64 = self.tiles.iter().map(|t| t.tally.extra).sum();
            let unknown: u64 = self.tiles.iter().map(|t| t.tally.unloaded).sum();
            let layer = self.layers_text();
            let note = if !self.shown {
                "hidden".into()
            } else if let Some(error) = self.tiles.iter().find_map(|t| t.error.as_ref()) {
                error.clone()
            } else {
                format!("{pending} tiles pending")
            };
            format!(
                "{} {:?} | {} | missing {} wrong {} extra {} unknown {} | {}",
                self.schematic.name, p.origin, layer, needed, wrong, extra, unknown, note
            )
        } else {
            self.options.menu_hint(&self.schematic.name)
        };
        self.show_label(&message);
    }
}

impl Options {
    pub(super) fn menu_hint(&self, title: &str) -> String {
        let menu = native_bindings::caption(native_bindings::Action::Main);
        format!("{title} | {menu} Menu")
    }
}
