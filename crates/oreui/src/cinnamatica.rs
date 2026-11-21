//! The Cinnamatica window drawn natively over the world from a personal panel's bindings and
//! controls. The mod keeps its logic: every click here becomes the same control event its
//! JSON-UI window sends, so only tabs the mod marks native are drawn here.

use std::collections::BTreeMap;

use ui::mod_panel::{Control, SurfaceValue};
use ui::{IconRef, UiPoint, UiRect};
use ui_engine::engine::{
    rect,
    screen::{PaintError, PaintFrame},
};

use super::{
    paint::{Bounds, Canvas},
    settings::controls::toggle_at,
    theme::{
        self, BODY, BORDER, CAPTION, DESTRUCTIVE_TINT, EDGE, FIELD_CARET, FIELD_PLACEHOLDER,
        HEADER5, NEUTRAL20, NEUTRAL90, PRIMARY_ROLE, SECTION_HEADER, SUCCESS_TINT, TEXT,
        TEXT_DIMMER, TEXT_DIMMEST,
    },
    transitions::switch_target,
    widgets::{self, Interaction, Variant, button_face},
};

/// The tab strip: control id and title, in the mod's tab order.
const TABS: [(&str, &str); 4] = [
    ("bw-tab-materials", "Materials"),
    ("bw-tab-import", "Import"),
    ("bw-tab-settings", "Settings"),
    ("bw-tab-controls", "Controls"),
];
/// Tab indices in [`TABS`].
const MATERIALS_TAB: usize = 0;
const IMPORT_TAB: usize = 1;
const SETTINGS_TAB: usize = 2;
const CONTROLS_TAB: usize = 3;
/// Material rows in view; the mod pages through the list with the mouse wheel.
const MATERIAL_ROWS: usize = 8;
/// The materials search field's control id.
pub const SEARCH_ID: &str = "bw-search";
/// The longest search the mod's field accepts.
const SEARCH_LIMIT: usize = 24;
/// Library cards on the Import tab: every schematic one library holds.
const LIBRARY_ROWS: usize = 8;

/// One Settings row: a switch bound to `bw-set-<key>`, or a button cycling a value whose
/// caption is a binding.
#[derive(Clone, Copy)]
enum Row {
    Switch(&'static str),
    Cycle {
        id: &'static str,
        title: &'static str,
        caption: &'static str,
    },
}

/// The Settings tab's groups and rows; switch titles come from the panel's controls.
const GROUPS: [(&str, &[Row]); 3] = [
    (
        "Fast Place",
        &[
            Row::Switch("fast-place"),
            Row::Cycle {
                id: "bw-fp-mode",
                title: "Fast Place mode",
                caption: "#bw_fp_mode",
            },
            Row::Switch("hold-place"),
            Row::Switch("placement-lock"),
            Row::Switch("correct-aim"),
            Row::Switch("bridge-place"),
        ],
    ),
    (
        "Display",
        &[
            Row::Switch("look"),
            Row::Switch("assist"),
            Row::Cycle {
                id: "lightness",
                title: "Ghost lightness",
                caption: "#bw_lightness",
            },
        ],
    ),
    (
        "Behavior",
        &[
            Row::Switch("remember-placement"),
            Row::Switch("auto-layer"),
            Row::Switch("paste-destroy"),
        ],
    ),
];
const SETTING_PREFIX: &str = "bw-set-";

const PANEL_WIDTH: f32 = 72.0;
const HEADER: f32 = 4.4;
const TAB_HEIGHT: f32 = 4.0;
const PAD: f32 = 1.6;
const ROW: f32 = 3.6;
const SECTION: f32 = 2.8;
const FOOTER: f32 = 5.6;
const CYCLE_WIDTH: f32 = 12.0;
const SWITCH_WIDTH: f32 = 6.4;
const BUTTON: f32 = 3.2;
const GAP: f32 = 0.8;

/// What one paint reads: the panel's current bindings and controls, and the item artwork
/// lookup for material icons.
pub struct CinnamaticaInput<'a> {
    pub bindings: &'a BTreeMap<String, SurfaceValue>,
    pub controls: &'a [Control],
    pub icon: &'a dyn Fn(&str, u32) -> Option<IconRef>,
}

/// A click target: the control id and the value its event carries.
#[derive(Clone, Debug, PartialEq)]
pub struct CinnamaticaHit {
    pub id: String,
    pub value: f32,
}

/// The window's last hit rects (window-logical) and the tracked pointer.
#[derive(Default)]
pub struct CinnamaticaScreen {
    hits: Vec<(CinnamaticaHit, UiRect)>,
    pointer: Option<UiPoint>,
    /// The search being typed while the search field has focus.
    search: Option<String>,
}

impl CinnamaticaScreen {
    /// Draws the window; its targets land in window-logical px.
    pub fn paint(
        &mut self,
        input: CinnamaticaInput<'_>,
        frame: &mut PaintFrame<'_>,
    ) -> Result<(), PaintError> {
        let hovered = self.pointer.and_then(|point| {
            self.hits
                .iter()
                .find_map(|(hit, bounds)| bounds.contains(point).then(|| hit.id.clone()))
        });
        let mut canvas = Canvas::new(
            &mut *frame.nodes,
            &mut *frame.next,
            &mut *frame.layouts,
            frame.font,
            frame.metrics,
            frame.solid_page,
            None,
        );
        canvas.bundle = theme::Bundle::Gameplay;
        let window = Window {
            input: &input,
            hovered: hovered.as_deref(),
            search: self.search.as_deref(),
        };
        let hits = window.draw(&mut canvas, frame.content)?;
        let [left, top] = [frame.safe_area.left(), frame.safe_area.top()];
        self.hits = hits
            .into_iter()
            .filter_map(|(hit, b)| {
                rect(b[0] + left, b[1] + top, b[2] + left, b[3] + top)
                    .ok()
                    .map(|bounds| (hit, bounds))
            })
            .collect();
        Ok(())
    }

    /// What a press at the window-logical `position` hits.
    pub fn hit(&self, position: UiPoint) -> Option<CinnamaticaHit> {
        self.hits
            .iter()
            .find_map(|(hit, bounds)| bounds.contains(position).then(|| hit.clone()))
    }

    /// Tracks the pointer for next frame's hover state.
    pub fn set_pointer(&mut self, position: Option<UiPoint>) {
        self.pointer = position;
    }

    /// Whether the search field has focus and takes typed keys.
    pub fn searching(&self) -> bool {
        self.search.is_some()
    }

    /// Focuses the search field, continuing from the mod's current `value`.
    pub fn begin_search(&mut self, value: &str) {
        self.search = Some(value.chars().take(SEARCH_LIMIT).collect());
    }

    /// Ends typing; the search keeps its last value.
    pub fn end_search(&mut self) {
        self.search = None;
    }

    /// Applies one key with its typed `text`; returns the new search when it changed.
    /// Enter and Escape end typing.
    pub fn search_key(&mut self, key: &str, text: Option<&str>) -> Option<String> {
        let search = self.search.as_mut()?;
        match key {
            "Enter" | "NumpadEnter" | "Escape" => {
                self.search = None;
                return None;
            }
            "Backspace" => {
                search.pop()?;
            }
            _ => {
                let text = text.filter(|text| text.chars().all(|ch| !ch.is_control()))?;
                let room = SEARCH_LIMIT.saturating_sub(search.chars().count());
                if room == 0 || text.is_empty() {
                    return None;
                }
                search.extend(text.chars().take(room));
            }
        }
        Some(search.clone())
    }

    /// The last frame's targets.
    #[cfg(any(test, feature = "test-support"))]
    pub fn hits(&self) -> &[(CinnamaticaHit, UiRect)] {
        &self.hits
    }
}

/// One frame's reading of the panel.
struct Window<'a> {
    input: &'a CinnamaticaInput<'a>,
    hovered: Option<&'a str>,
    /// The search being typed, while the field has focus.
    search: Option<&'a str>,
}

type Hits = Vec<(CinnamaticaHit, Bounds)>;

impl Window<'_> {
    fn text(&self, name: &str) -> &str {
        match self.input.bindings.get(name) {
            Some(SurfaceValue::Text(text)) => text,
            _ => "",
        }
    }

    fn flag(&self, name: &str) -> bool {
        matches!(
            self.input.bindings.get(name),
            Some(SurfaceValue::Bool(true))
        )
    }

    /// A button sending `id`, recorded as a target.
    fn button(
        &self,
        canvas: &mut Canvas<'_>,
        id: &str,
        label: &str,
        b: Bounds,
        variant: Variant,
        hits: &mut Hits,
    ) -> Result<(), PaintError> {
        button_face(canvas, b, variant, label, self.interaction(id), true)?;
        hits.push((hit(id, 1.0), b));
        Ok(())
    }

    fn tab(&self) -> usize {
        match self.input.bindings.get("#bw_tab") {
            Some(SurfaceValue::Number(tab)) => (*tab as usize).min(TABS.len() - 1),
            _ => SETTINGS_TAB,
        }
    }

    /// A switch's title and state from the panel's toggle controls.
    fn switch(&self, key: &str) -> Option<(&str, bool)> {
        let id = format!("{SETTING_PREFIX}{key}");
        self.input
            .controls
            .iter()
            .find_map(|control| match control {
                Control::Toggle {
                    id: control,
                    label,
                    value,
                } if *control == id => Some((label.as_str(), *value)),
                _ => None,
            })
    }

    fn interaction(&self, id: &str) -> Interaction {
        Interaction {
            action: None,
            hovered: self.hovered == Some(id),
            pressed: false,
            focused: false,
        }
    }

    /// Fits the card to the window, scaling it down when the window is short.
    fn draw(&self, canvas: &mut Canvas<'_>, size: [f32; 2]) -> Result<Hits, PaintError> {
        widgets::screen_overlay(canvas, size)?;
        let rows = GROUPS[0].1.len() as f32;
        let right = (GROUPS[1].1.len() + GROUPS[2].1.len()) as f32;
        let body = (SECTION + ROW * rows).max(SECTION * 2.0 + ROW * right);
        let height = HEADER + TAB_HEIGHT + PAD * 2.0 + body + FOOTER;
        let margin = canvas.r(2.4);
        let scale = ((size[1] - margin * 2.0) / canvas.r(height))
            .min((size[0] - margin * 2.0) / canvas.r(PANEL_WIDTH))
            .min(1.0);
        canvas.with_scale(scale, |canvas| self.draw_card(canvas, size, height))
    }

    fn draw_card(
        &self,
        canvas: &mut Canvas<'_>,
        size: [f32; 2],
        height: f32,
    ) -> Result<Hits, PaintError> {
        let width = canvas.r(PANEL_WIDTH);
        let left = (size[0] - width) * 0.5;
        let top = (size[1] - canvas.r(height)) * 0.5;
        let card = [left, top, left + width, top + canvas.r(height)];
        widgets::panel(canvas, card)?;
        let mut hits = Hits::new();

        // The header bar.
        let role = canvas.role(NEUTRAL20);
        let header = [card[0], card[1], card[2], card[1] + canvas.r(HEADER)];
        canvas.fill(header, role.fill)?;
        canvas.bevel(header, role.specular[0], role.specular[1])?;
        canvas.text_centred("Cinnamatica", header, HEADER5, role.text, false)?;

        // The tabs.
        let pad = canvas.r(PAD);
        let strip_top = header[3] + pad;
        let gap = canvas.r(0.8);
        let tab_width = (width - pad * 2.0 - gap * (TABS.len() - 1) as f32) / TABS.len() as f32;
        let open = self.tab();
        for (index, (id, title)) in TABS.into_iter().enumerate() {
            let x = card[0] + pad + index as f32 * (tab_width + gap);
            let b = [
                x,
                strip_top,
                x + tab_width,
                strip_top + canvas.r(TAB_HEIGHT),
            ];
            let variant = if index == open {
                Variant::Primary
            } else {
                Variant::Neutral
            };
            button_face(canvas, b, variant, title, self.interaction(id), true)?;
            hits.push((hit(id, 1.0), b));
        }

        // The open tab's body.
        let body_top = strip_top + canvas.r(TAB_HEIGHT) + pad;
        match open {
            MATERIALS_TAB => self.materials(canvas, card, body_top, &mut hits)?,
            IMPORT_TAB => self.import(canvas, card, body_top, &mut hits)?,
            SETTINGS_TAB => self.settings(canvas, card, body_top, &mut hits)?,
            CONTROLS_TAB => self.controls(canvas, card, body_top, &mut hits)?,
            _ => {}
        }

        // The footer: how to close.
        let footer_top = card[3] - canvas.r(FOOTER) + canvas.r(1.8);
        canvas.text_line(
            "Settings save immediately  ·  M or Esc to close",
            [card[0] + pad, footer_top],
            card[2] - card[0] - pad * 2.0,
            CAPTION,
            TEXT_DIMMEST,
        )?;
        Ok(hits)
    }

    /// The tab's title and its summary under it.
    fn heading(
        &self,
        canvas: &mut Canvas<'_>,
        prefix: &str,
        at: [f32; 2],
        width: f32,
    ) -> Result<f32, PaintError> {
        let [x, y] = at;
        canvas.text_line(
            self.text(&format!("#{prefix}_title")),
            [x, y],
            width,
            SECTION_HEADER,
            TEXT,
        )?;
        canvas.text_line(
            self.text(&format!("#{prefix}_summary")),
            [x, y + canvas.r(2.0)],
            width,
            CAPTION,
            TEXT_DIMMER,
        )?;
        Ok(y + canvas.r(4.4))
    }

    /// Materials: a search field, build progress and a page of materials with their icons.
    fn materials(
        &self,
        canvas: &mut Canvas<'_>,
        card: Bounds,
        top: f32,
        hits: &mut Hits,
    ) -> Result<(), PaintError> {
        let pad = canvas.r(PAD);
        let [left, right] = [card[0] + pad, card[2] - pad];
        let field = [right - canvas.r(22.0), top, right, top + canvas.r(BUTTON)];
        self.search_field(canvas, field, hits)?;
        let y = self.heading(canvas, "bw", [left, top], field[0] - left - pad)?;

        // Progress: a bar, the built fraction beside it and the page on the right.
        let bar = [
            left,
            y + canvas.r(0.4),
            left + canvas.r(28.0),
            y + canvas.r(1.6),
        ];
        canvas.fill(bar, NEUTRAL90)?;
        let fraction = match self.input.bindings.get("#bw_progress") {
            Some(SurfaceValue::Number(value)) => value.clamp(0.0, 1.0) as f32,
            _ => 0.0,
        };
        if fraction > 0.0 {
            let filled = [
                bar[0],
                bar[1],
                bar[0] + (bar[2] - bar[0]) * fraction,
                bar[3],
            ];
            canvas.fill(filled, PRIMARY_ROLE.fill)?;
        }
        canvas.frame(bar, EDGE, BORDER)?;
        canvas.text_line(
            self.text("#bw_progress_label"),
            [bar[2] + pad, y],
            right - bar[2] - pad * 2.0 - canvas.r(10.0),
            CAPTION,
            TEXT_DIMMER,
        )?;
        self.right_text(
            canvas,
            self.text("#bw_page"),
            [right, y],
            CAPTION,
            TEXT_DIMMEST,
        )?;

        // The list: column titles, a rule and one row per material.
        let needed_right = right - canvas.r(12.0);
        let head = y + canvas.r(2.6);
        canvas.text_line(
            "Material",
            [left + canvas.r(3.2), head],
            canvas.r(20.0),
            CAPTION,
            TEXT_DIMMEST,
        )?;
        self.right_text(
            canvas,
            "Left",
            [needed_right, head],
            CAPTION,
            TEXT_DIMMEST,
        )?;
        self.right_text(
            canvas,
            "Total",
            [right - canvas.r(0.8), head],
            CAPTION,
            TEXT_DIMMEST,
        )?;
        let rule = head + canvas.r(2.2);
        canvas.fill([left, rule, right, rule + canvas.r(EDGE)], BORDER)?;
        let row_height = canvas.r(2.6);
        let first = rule + canvas.r(0.4);
        for row in 0..MATERIAL_ROWS {
            let field = |name: &str| format!("#bw_row{row}_{name}");
            if !self.flag(&field("visible")) {
                continue;
            }
            let at = first + row_height * row as f32;
            let b = [left, at, right, at + row_height];
            if row % 2 == 1 {
                canvas.fill(b, [255, 255, 255, 8])?;
            }
            let icon = canvas.r(2.0);
            let middle = (b[1] + b[3]) * 0.5;
            if let Some(SurfaceValue::Item(item)) = self.input.bindings.get(&field("icon"))
                && let Some(art) = (self.input.icon)(&item.item.identifier, item.item.metadata)
            {
                let x = b[0] + canvas.r(0.4);
                canvas.icon_ref(art, [x, middle - icon * 0.5, x + icon, middle + icon * 0.5])?;
            }
            let line = middle - canvas.r(BODY.line) * 0.5;
            canvas.text_line(
                self.text(&field("name")),
                [left + canvas.r(3.2), line],
                needed_right - left - canvas.r(12.0),
                BODY,
                TEXT,
            )?;
            let needed = if self.flag(&field("done")) {
                SUCCESS_TINT
            } else {
                DESTRUCTIVE_TINT
            };
            self.right_text(
                canvas,
                self.text(&field("needed")),
                [needed_right, line],
                BODY,
                needed,
            )?;
            self.right_text(
                canvas,
                self.text(&field("total")),
                [right - canvas.r(0.8), line],
                BODY,
                TEXT_DIMMER,
            )?;
        }
        if self.flag("#bw_empty_visible") {
            let message = [left, first, right, first + row_height * 3.0];
            canvas.text_centred(self.text("#bw_empty"), message, BODY, TEXT_DIMMEST, false)?;
        }
        Ok(())
    }

    /// The search field: the typed search with a caret while focused, otherwise the mod's
    /// search or a placeholder. Clicking it focuses typing.
    fn search_field(
        &self,
        canvas: &mut Canvas<'_>,
        b: Bounds,
        hits: &mut Hits,
    ) -> Result<(), PaintError> {
        let focused = self.search.is_some();
        canvas.fill(b, NEUTRAL90)?;
        let ring = if focused || self.hovered == Some(SEARCH_ID) {
            TEXT_DIMMER
        } else {
            BORDER
        };
        canvas.frame(b, EDGE, ring)?;
        let at = [
            b[0] + canvas.r(0.8),
            (b[1] + b[3]) * 0.5 - canvas.r(BODY.line) * 0.5,
        ];
        let width = b[2] - b[0] - canvas.r(1.6);
        let value = self.search.unwrap_or_else(|| self.text("#bw_search_text"));
        if value.is_empty() && !focused {
            canvas.text_line("Filter...", at, width, BODY, FIELD_PLACEHOLDER)?;
        } else {
            canvas.text_line(value, at, width, BODY, TEXT)?;
            if focused {
                let caret = at[0] + canvas.measure(value, BODY)?.min(width);
                canvas.fill(
                    [
                        caret,
                        at[1],
                        caret + canvas.r(EDGE),
                        at[1] + canvas.r(BODY.line),
                    ],
                    FIELD_CARET,
                )?;
            }
        }
        hits.push((hit(SEARCH_ID, 0.0), b));
        Ok(())
    }

    /// Draws one line of `value` ending at `at`.
    fn right_text(
        &self,
        canvas: &mut Canvas<'_>,
        value: &str,
        at: [f32; 2],
        style: theme::Type,
        color: theme::Rgba,
    ) -> Result<(), PaintError> {
        if value.is_empty() {
            return Ok(());
        }
        let width = canvas.measure(value, style)?;
        canvas.text_line(value, [at[0] - width, at[1]], width, style, color)?;
        Ok(())
    }

    /// Import: the import button, the active banner and a two-column grid of library cards.
    fn import(
        &self,
        canvas: &mut Canvas<'_>,
        card: Bounds,
        top: f32,
        hits: &mut Hits,
    ) -> Result<(), PaintError> {
        let pad = canvas.r(PAD);
        let [left, right] = [card[0] + pad, card[2] - pad];
        let import = [right - canvas.r(18.0), top, right, top + canvas.r(BUTTON)];
        self.button(
            canvas,
            "import",
            "Import schematic",
            import,
            Variant::Primary,
            hits,
        )?;
        let mut y = self.heading(canvas, "bw_lib", [left, top], import[0] - left - pad)?;
        if self.flag("#bw_lib_banner_visible") {
            let banner = [left, y, right, y + canvas.r(2.8)];
            canvas.fill(
                banner,
                [SUCCESS_TINT[0], SUCCESS_TINT[1], SUCCESS_TINT[2], 40],
            )?;
            canvas.frame(banner, EDGE, SUCCESS_TINT)?;
            let line = y + canvas.r(0.4);
            canvas.text_line(
                "ACTIVE",
                [left + canvas.r(0.8), line],
                canvas.r(6.0),
                CAPTION,
                SUCCESS_TINT,
            )?;
            canvas.text_line(
                self.text("#bw_lib_banner"),
                [left + canvas.r(7.2), line],
                right - left - canvas.r(8.0),
                CAPTION,
                TEXT,
            )?;
            y = banner[3] + canvas.r(0.6);
        }
        let gap = canvas.r(GAP);
        let column = (right - left - gap) * 0.5;
        let height = canvas.r(4.8);
        for row in 0..LIBRARY_ROWS {
            let x = left + (row % 2) as f32 * (column + gap);
            let at = y + (row / 2) as f32 * (height + canvas.r(0.6));
            self.library_card(canvas, row, [x, at, x + column, at + height], hits)?;
        }
        Ok(())
    }

    /// One library entry: its name and size on the left, Load and Delete on the right.
    fn library_card(
        &self,
        canvas: &mut Canvas<'_>,
        row: usize,
        b: Bounds,
        hits: &mut Hits,
    ) -> Result<(), PaintError> {
        let state = |name: &str| format!("#bw_lib{row}_{name}");
        if !self.flag(&format!("#visible{row}")) {
            canvas.frame(b, EDGE, BORDER)?;
            canvas.text_centred("Empty slot", b, CAPTION, TEXT_DIMMEST, false)?;
            return Ok(());
        }
        let loaded = self.flag(&state("loaded"));
        canvas.fill(b, NEUTRAL90)?;
        canvas.frame(b, EDGE, if loaded { SUCCESS_TINT } else { BORDER })?;
        let inset = canvas.r(0.8);
        let small = canvas.r(7.2);
        let buttons = b[2] - inset - small * 2.0 - canvas.r(0.4);
        let text_width = buttons - b[0] - inset * 2.0;
        canvas.text_line(
            self.text(&format!("#bw_lib_name{row}")),
            [b[0] + inset, b[1] + canvas.r(0.4)],
            text_width,
            BODY,
            if loaded { SUCCESS_TINT } else { TEXT },
        )?;
        canvas.text_line(
            self.text(&state("meta")),
            [b[0] + inset, b[1] + canvas.r(2.4)],
            text_width,
            CAPTION,
            TEXT_DIMMER,
        )?;
        let middle = (b[1] + b[3]) * 0.5;
        let face = |x: f32| [x, middle - canvas.r(1.5), x + small, middle + canvas.r(1.5)];
        let load = face(buttons);
        let delete = face(buttons + small + canvas.r(0.4));
        let variant = if self.flag(&state("active_on")) {
            Variant::Primary
        } else {
            Variant::Secondary
        };
        self.button(
            canvas,
            &format!("toggle-{row}"),
            self.text(&format!("#entryStatus{row}")),
            load,
            variant,
            hits,
        )?;
        self.button(
            canvas,
            &format!("delete-{row}"),
            self.text(&state("delete")),
            delete,
            Variant::Destructive,
            hits,
        )
    }

    /// Controls: Position, Rotate & Mirror and Guide columns, and Paste with its confirmation.
    fn controls(
        &self,
        canvas: &mut Canvas<'_>,
        card: Bounds,
        top: f32,
        hits: &mut Hits,
    ) -> Result<(), PaintError> {
        let pad = canvas.r(PAD);
        let [left, right] = [card[0] + pad, card[2] - pad];
        let y = self.heading(canvas, "bw_keys", [left, top], right - left)?;
        let column = (right - left - pad * 2.0) / 3.0;
        let gap = canvas.r(GAP);
        let half = (column - gap) * 0.5;
        let step = canvas.r(BUTTON) + gap;
        let rows = y + canvas.r(SECTION);
        let [height, layers, gap_after_hint] = [canvas.r(BUTTON), canvas.r(2.4), canvas.r(1.6)];
        let wide = |x: f32, line: f32| {
            let y = rows + step * line;
            [x, y, x + column, y + height]
        };
        let pair = |x: f32, line: f32| {
            let b = wide(x, line);
            (
                [b[0], b[1], b[0] + half, b[3]],
                [b[2] - half, b[1], b[2], b[3]],
            )
        };
        let columns = [left, left + column + pad, left + (column + pad) * 2.0];
        for (title, x) in ["Position", "Rotate & Mirror", "Guide"]
            .into_iter()
            .zip(columns)
        {
            canvas.text_line(title, [x, y + canvas.r(0.4)], column, SECTION_HEADER, TEXT)?;
        }
        let neutral = Variant::Secondary;

        let [a, b, c] = columns;
        self.button(canvas, "place", "Place here", wide(a, 0.0), neutral, hits)?;
        canvas.text(
            "Numpad 8/4/6/2 move, 7/9 down/up, 1/3 rotate.",
            [a, rows + step + canvas.r(0.2)],
            column,
            CAPTION,
            TEXT_DIMMEST,
            false,
        )?;

        let (ccw, cw) = pair(b, 0.0);
        self.button(canvas, "ccw", "Rotate left", ccw, neutral, hits)?;
        self.button(canvas, "cw", "Rotate right", cw, neutral, hits)?;
        self.button(canvas, "mirror", "Mirror", wide(b, 1.0), neutral, hits)?;
        canvas.text_line(
            "Layers",
            [b, rows + step * 2.0 + canvas.r(0.6)],
            column,
            CAPTION,
            TEXT_DIMMER,
        )?;
        let (previous, next) = pair(b, 2.0);
        let shift = |bounds: Bounds| [bounds[0], bounds[1] + layers, bounds[2], bounds[3] + layers];
        self.button(
            canvas,
            "previous-layer",
            "Prev",
            shift(previous),
            neutral,
            hits,
        )?;
        self.button(canvas, "next-layer", "Next", shift(next), neutral, hits)?;
        let mode = shift(wide(b, 3.0));
        self.button(
            canvas,
            "mode",
            self.text("#bw_layer_mode"),
            mode,
            neutral,
            hits,
        )?;

        let locked = self.flag("#bw_lock_on");
        self.button(
            canvas,
            "lock",
            if locked {
                "Lock guide: ON"
            } else {
                "Lock guide: OFF"
            },
            wide(c, 0.0),
            if locked { Variant::Primary } else { neutral },
            hits,
        )?;
        canvas.text(
            "Stops moves and rotation. Not Placement lock.",
            [c, rows + step + canvas.r(0.2)],
            column,
            CAPTION,
            TEXT_DIMMEST,
            false,
        )?;
        let after = |line: f32| {
            let bounds = wide(c, line);
            [
                bounds[0],
                bounds[1] + gap_after_hint,
                bounds[2],
                bounds[3] + gap_after_hint,
            ]
        };
        self.button(
            canvas,
            "visibility",
            self.text("#bw_visibility"),
            after(2.0),
            neutral,
            hits,
        )?;

        let paste_top = mode[3] + canvas.r(1.2);
        let paste = [
            left,
            paste_top,
            left + canvas.r(16.0),
            paste_top + canvas.r(BUTTON),
        ];
        let confirming = self.text("#bw_paste") != "Paste";
        self.button(
            canvas,
            "paste",
            self.text("#bw_paste"),
            paste,
            if confirming {
                Variant::Destructive
            } else {
                neutral
            },
            hits,
        )?;
        let mode_left = paste[2] + pad;
        let mode_right = mode_left + canvas.r(14.0);
        self.button(
            canvas,
            "bw-export",
            self.text("#bw_export"),
            [mode_left, paste_top, mode_right, paste[3]],
            neutral,
            hits,
        )?;
        canvas.text_line(
            self.text("#bw_paste_hint"),
            [mode_right + pad, paste_top + canvas.r(0.6)],
            right - mode_right - pad,
            CAPTION,
            TEXT_DIMMER,
        )?;
        Ok(())
    }

    /// Fast Place on the left; Display and Behavior stacked on the right.
    fn settings(
        &self,
        canvas: &mut Canvas<'_>,
        card: Bounds,
        top: f32,
        hits: &mut Hits,
    ) -> Result<(), PaintError> {
        let pad = canvas.r(PAD);
        let column = (card[2] - card[0] - pad * 3.0) * 0.5;
        let left = card[0] + pad;
        let right = left + column + pad;
        self.group(canvas, GROUPS[0], [left, top], column, hits)?;
        let below = self.group(canvas, GROUPS[1], [right, top], column, hits)?;
        self.group(canvas, GROUPS[2], [right, below], column, hits)?;
        Ok(())
    }

    /// Draws one titled group of rows; returns its bottom edge.
    fn group(
        &self,
        canvas: &mut Canvas<'_>,
        (title, rows): (&str, &[Row]),
        at: [f32; 2],
        width: f32,
        hits: &mut Hits,
    ) -> Result<f32, PaintError> {
        let [x, mut y] = at;
        canvas.text_line(title, [x, y + canvas.r(0.4)], width, SECTION_HEADER, TEXT)?;
        y += canvas.r(SECTION);
        for row in rows {
            let b = [x, y, x + width, y + canvas.r(ROW)];
            match *row {
                Row::Switch(key) => self.switch_row(canvas, key, b, hits)?,
                Row::Cycle { id, title, caption } => {
                    self.cycle_row(canvas, id, title, self.text(caption), b, hits)?;
                }
            }
            y = b[3];
        }
        Ok(y + canvas.r(0.8))
    }

    /// A row titled by its switch, which flips when any part of the row is clicked.
    fn switch_row(
        &self,
        canvas: &mut Canvas<'_>,
        key: &str,
        b: Bounds,
        hits: &mut Hits,
    ) -> Result<(), PaintError> {
        let Some((title, on)) = self.switch(key) else {
            return Ok(());
        };
        let id = format!("{SETTING_PREFIX}{key}");
        let state = self.interaction(&id);
        if state.hovered {
            canvas.fill(b, [255, 255, 255, 18])?;
        }
        let middle = (b[1] + b[3]) * 0.5;
        canvas.text_line(
            title,
            [b[0] + canvas.r(0.4), middle - canvas.r(BODY.line) * 0.5],
            b[2] - b[0] - canvas.r(SWITCH_WIDTH + 1.6),
            BODY,
            TEXT_DIMMER,
        )?;
        let at = [b[2] - canvas.r(SWITCH_WIDTH), middle - canvas.r(1.6)];
        toggle_at(canvas, at, switch_target(on), state, true)?;
        hits.push((hit(&id, if on { 0.0 } else { 1.0 }), b));
        Ok(())
    }

    /// A row with a title and a button cycling its value.
    #[allow(clippy::too_many_arguments)]
    fn cycle_row(
        &self,
        canvas: &mut Canvas<'_>,
        id: &str,
        title: &str,
        caption: &str,
        b: Bounds,
        hits: &mut Hits,
    ) -> Result<(), PaintError> {
        let middle = (b[1] + b[3]) * 0.5;
        canvas.text_line(
            title,
            [b[0] + canvas.r(0.4), middle - canvas.r(BODY.line) * 0.5],
            b[2] - b[0] - canvas.r(CYCLE_WIDTH + 1.6),
            BODY,
            TEXT_DIMMER,
        )?;
        let button = [
            b[2] - canvas.r(CYCLE_WIDTH),
            middle - canvas.r(1.5),
            b[2],
            middle + canvas.r(1.5),
        ];
        button_face(
            canvas,
            button,
            Variant::Secondary,
            caption,
            self.interaction(id),
            true,
        )?;
        hits.push((hit(id, 1.0), button));
        Ok(())
    }
}

fn hit(id: &str, value: f32) -> CinnamaticaHit {
    CinnamaticaHit {
        id: id.to_owned(),
        value,
    }
}

#[cfg(test)]
mod tests;
