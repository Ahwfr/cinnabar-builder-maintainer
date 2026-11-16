//! The tabbed window opened with P: a dimmed backdrop and one card whose top strip holds the
//! tabs, centred, with an underline that slides to the open tab. Its layout is built here and
//! precomputed into `ui/browser.json`, so opening it costs no callback fuel. Add a tab to
//! [`Tab`] and `layout::TABS`, then give it a body in `layout`.
//!
//! Colors that change at runtime switch between pre-coloured elements with visibility
//! bindings instead of rebinding colour values.

use crate::Viewer;
use serde_json::{Map, Value, json};

/// The precomputed panel content. Regenerate with `SCHEMATICA_BROWSER_REGENERATE=1 cargo test`.
pub const LAYOUT: &str = include_str!("../ui/browser.json");
/// The physical key that opens and closes the window.
pub const KEY: &str = "KeyP";
/// Material rows in view; the mouse wheel moves through the list a view at a time.
pub const ROWS: usize = 8;
/// Library rows on the Import tab: every schematic one library can hold.
const LIBRARY_ROWS: usize = schematica_core::blueprint::MAX_LIBRARY;
/// The materials search field's control id.
const SEARCH_ID: &str = "bw-search";
/// The Fast Place mode button's control id.
const MODE_ID: &str = "bw-fp-mode";
/// Tab width and the gap between tabs; the underline moves by their sum.
const TAB_WIDTH: f64 = 100.;
const TAB_GAP: f64 = 4.;

/// The window's tabs, in display order.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    Materials,
    Import,
    Settings,
    Controls,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Materials, Tab::Import, Tab::Settings, Tab::Controls];

    /// The panel control id of this tab's button.
    fn id(self) -> &'static str {
        match self {
            Tab::Materials => "bw-tab-materials",
            Tab::Import => "bw-tab-import",
            Tab::Settings => "bw-tab-settings",
            Tab::Controls => "bw-tab-controls",
        }
    }

    fn index(self) -> usize {
        Tab::ALL.iter().position(|tab| *tab == self).unwrap_or(0)
    }
}

/// The Settings tab's switches: option key and title. Controls are named `bw-set-<key>`.
const SETTINGS: [(&str, &str); 9] = [
    ("hold-place", "Hold to place"),
    ("look", "Textured ghost blocks"),
    ("assist", "Target info"),
    ("remember-placement", "Remember positions"),
    ("paste-destroy", "Destroy overlaps on paste"),
    ("fast-place", "Fast Place"),
    ("correct-aim", "Auto orientation"),
    ("bridge-place", "Nearby placement assist"),
    ("auto-layer", "Auto next layer"),
];
const SETTING_PREFIX: &str = "bw-set-";

/// Builds [`LAYOUT`]; compiled only for the test that regenerates and checks it.
#[cfg(test)]
mod layout {
    use super::{
        LIBRARY_ROWS, MODE_ID, ROWS, SEARCH_ID, SETTING_PREFIX, SETTINGS, TAB_GAP, TAB_WIDTH, Tab,
    };
    use serde_json::{Value, json};

    /// Tab titles, matching [`Tab::ALL`].
    const TABS: [&str; 4] = ["Materials", "Import", "Settings", "Controls"];

    const WINDOW: [f64; 2] = [460., 330.];
    /// The tab strip across the top of the card.
    const STRIP: f64 = 34.;
    const TAB_HEIGHT: f64 = 24.;
    const PAD: f64 = 16.;
    const WIDTH: f64 = WINDOW[0] - PAD * 2.;
    const RADIUS: f64 = 4.;
    const FONT: &str = "mod_panel_sans";

    const BACKDROP: [f64; 4] = [0., 0., 0., 0.35];
    const CARD: [f64; 4] = [0.12, 0.12, 0.13, 0.97];
    const BORDER: [f64; 4] = [1., 1., 1., 0.14];
    const BUTTON: [f64; 4] = [0.19, 0.19, 0.21, 1.];
    const FIELD: [f64; 4] = [0.09, 0.09, 0.10, 1.];
    const ACCENT: [f64; 4] = [0.27, 0.45, 0.78, 1.];
    const TEXT: [f64; 4] = [0.93, 0.93, 0.95, 1.];
    const MUTED: [f64; 4] = [0.58, 0.58, 0.62, 1.];
    const GREEN: [f64; 4] = [0.60, 0.92, 0.66, 1.];
    const DANGER: [f64; 4] = [0.95, 0.47, 0.47, 1.];
    const LINE: [f64; 4] = [1., 1., 1., 0.07];
    const LOADED: [f64; 4] = [0.33, 0.72, 0.44, 0.16];
    const SWITCH_OFF: [f64; 4] = [0.30, 0.30, 0.33, 1.];
    const SWITCH_ON: [f64; 4] = [0.33, 0.72, 0.44, 1.];
    /// Switch track; the host slides the knob from x 2 to x 12 as a toggle eases on.
    const SWITCH: [f64; 2] = [26., 16.];

    fn anchored(mut node: Value, size: [f64; 2], offset: [f64; 2]) -> Value {
        node["size"] = json!(size);
        node["offset"] = json!(offset);
        node["anchor_from"] = json!("top_left");
        node["anchor_to"] = json!("top_left");
        node
    }

    fn rect(size: [f64; 2], offset: [f64; 2], color: [f64; 4], radius: f64) -> Value {
        anchored(
            json!({"type":"custom","renderer":"cinnabar_rounded_rectangle","color":color,"radius":radius}),
            size,
            offset,
        )
    }

    /// A rounded rectangle with the window's thin light border.
    fn framed(size: [f64; 2], offset: [f64; 2], color: [f64; 4]) -> Value {
        let mut node = rect(size, offset, color, RADIUS);
        node["border_color"] = json!(BORDER);
        node["border_width"] = json!(1);
        node
    }

    /// A label; a `#name` text is bound to that name.
    fn text(text: &str, size: [f64; 2], offset: [f64; 2], color: [f64; 4], align: &str) -> Value {
        let mut label = anchored(
            json!({"type":"label","text":text,"color":color,"shadow":false,
                "font_type":FONT,"text_alignment":align,"localize":false}),
            size,
            offset,
        );
        if text.starts_with('#') {
            label["bindings"] = json!([{ "binding_name": text }]);
        }
        label
    }

    fn named(name: impl Into<String>, node: Value) -> Value {
        json!({ name.into(): node })
    }

    fn bind(mut node: Value, binding: &str, target: &str) -> Value {
        let entry = json!({"binding_name": binding, "binding_name_override": target});
        match node["bindings"].as_array_mut() {
            Some(bindings) => bindings.push(entry),
            None => node["bindings"] = json!([entry]),
        }
        node
    }

    fn visible(node: Value, binding: &str) -> Value {
        bind(node, binding, "#visible")
    }

    /// Panel controls the layout routes clicks to, in declaration order.
    #[derive(Default)]
    struct Controls(Vec<Value>);

    impl Controls {
        fn push(&mut self, control: Value) -> usize {
            self.0.push(control);
            self.0.len() - 1
        }
        fn button(&mut self, id: &str) -> usize {
            self.push(json!({"kind":"button","id":id,"label":id}))
        }
        fn toggle(&mut self, id: &str, title: &str) -> usize {
            self.push(json!({"kind":"toggle","id":id,"label":title,"value":false}))
        }
        fn text(&mut self, id: &str, title: &str) -> usize {
            self.push(
                json!({"kind":"text","id":id,"label":title,"value":"","max_chars":24,"live":true}),
            )
        }
    }

    /// Wraps per-state contents in a routed button: idle, hover (a light wash) and pressed
    /// (a dark shade), each its own copy so every state draws.
    fn routed(
        index: usize,
        route: &str,
        size: [f64; 2],
        offset: [f64; 2],
        contents: impl Fn() -> Vec<Value>,
    ) -> Value {
        let state = |overlay: Option<Value>| {
            let mut items = contents();
            if let Some(overlay) = overlay {
                items.insert(1.min(items.len()), named("overlay", overlay));
            }
            anchored(json!({"type":"panel","controls":items}), size, [0., 0.])
        };
        let hover = bind(
            rect(size, [0., 0.], [1., 1., 1., 0.07], RADIUS),
            &format!("#row_{index}_hover"),
            "#alpha",
        );
        anchored(
            json!({"type":"button","default_control":"idle","hover_control":"hover",
            "pressed_control":"pressed",
            "button_mappings":[{"from_button_id":"button.menu_select",
                "to_button_id":format!("mod.{route}:{index}"),"mapping_type":"pressed"}],
            "controls":[
                {"idle": state(None)},
                {"hover": state(Some(hover))},
                {"pressed": state(Some(rect(size, [0., 0.], [0., 0., 0., 0.25], RADIUS)))},
            ]}),
            size,
            offset,
        )
    }

    /// A framed button with a centred caption. With `green`, a second caption in green shows
    /// while `<green>_on` is true and the normal one while `<green>_off` is.
    #[allow(clippy::too_many_arguments)]
    fn button(
        controls: &mut Controls,
        id: &str,
        caption: &str,
        size: [f64; 2],
        offset: [f64; 2],
        fill: [f64; 4],
        ink: [f64; 4],
        green: Option<&str>,
    ) -> Value {
        let index = controls.button(id);
        routed(index, "control", size, offset, || {
            let label = |color| {
                text(
                    caption,
                    [size[0], 10.],
                    [0., (size[1] - 10.) / 2.],
                    color,
                    "center",
                )
            };
            let mut items = vec![named("face", framed(size, [0., 0.], fill))];
            match green {
                Some(state) => {
                    items.push(named(
                        "caption",
                        visible(label(ink), &format!("{state}_off")),
                    ));
                    items.push(named(
                        "caption_on",
                        visible(label(GREEN), &format!("{state}_on")),
                    ));
                }
                None => items.push(named("caption", label(ink))),
            }
            items
        })
    }

    /// The tab strip: tabs centred for however many there are, and an underline whose
    /// offset eases to the open tab.
    fn strip(controls: &mut Controls) -> Vec<Value> {
        let count = Tab::ALL.len() as f64;
        let group = count * TAB_WIDTH + (count - 1.) * TAB_GAP;
        let left = (WINDOW[0] - group) / 2.;
        let mut nodes: Vec<Value> = Tab::ALL
            .iter()
            .zip(TABS)
            .enumerate()
            .map(|(index, (tab, title))| {
                let x = left + index as f64 * (TAB_WIDTH + TAB_GAP);
                let routed_index = controls.button(tab.id());
                let size = [TAB_WIDTH, TAB_HEIGHT];
                let node = routed(routed_index, "control", size, [x, 5.], || {
                    let label = |color| text(title, [TAB_WIDTH, 10.], [0., 7.], color, "center");
                    vec![
                        named(
                            "open",
                            visible(label(TEXT), &format!("#bw_tab{index}_open")),
                        ),
                        named(
                            "closed",
                            visible(label(MUTED), &format!("#bw_tab{index}_closed")),
                        ),
                    ]
                });
                named(tab.id(), node)
            })
            .collect();
        let spacer = bind(
            anchored(json!({"type":"panel"}), [0., 2.], [0., 0.]),
            "#bw_tab_x",
            "#size_binding_x_absolute",
        );
        let underline = rect([TAB_WIDTH, 2.], [0., 0.], ACCENT, 1.);
        nodes.push(named(
            "underline",
            anchored(
                json!({"type":"stack_panel","orientation":"horizontal",
                    "controls":[{"spacer":spacer},{"bar":underline}]}),
                [group, 2.],
                [left, STRIP - 3.],
            ),
        ));
        nodes.push(named(
            "divider",
            rect([WINDOW[0], 1.], [0., STRIP - 1.], LINE, 0.),
        ));
        nodes
    }

    /// A tab body shown only while its tab is open.
    fn body(tab: usize, nodes: Vec<Value>) -> Value {
        visible(
            anchored(json!({"type":"panel","controls":nodes}), WINDOW, [0., 0.]),
            &format!("#bw_body{tab}"),
        )
    }

    /// A centred title and summary below the tab strip.
    fn heading(prefix: &str) -> Vec<Value> {
        vec![
            named(
                "title",
                text(
                    &format!("#{prefix}_title"),
                    [WIDTH, 10.],
                    [PAD, STRIP + 8.],
                    TEXT,
                    "center",
                ),
            ),
            named(
                "summary",
                text(
                    &format!("#{prefix}_summary"),
                    [WIDTH, 10.],
                    [PAD, STRIP + 22.],
                    MUTED,
                    "center",
                ),
            ),
        ]
    }

    fn footer(hint: &str) -> Value {
        named(
            "hint",
            text(hint, [WIDTH, 10.], [PAD, WINDOW[1] - 22.], MUTED, "center"),
        )
    }

    /// Materials: progress, a search field and an alphabetical list.
    fn materials(controls: &mut Controls) -> Vec<Value> {
        let mut nodes = heading("bw");
        // The green bar fills its bound-width parent.
        let green = json!({"type":"custom","renderer":"cinnabar_rounded_rectangle",
            "color":SWITCH_ON,"radius":3,"size":["100%","100%"],"offset":[0,0],
            "anchor_from":"top_left","anchor_to":"top_left"});
        let fill = bind(
            anchored(
                json!({"type":"panel","controls":[{"green":green}]}),
                [WIDTH, 6.],
                [0., 0.],
            ),
            "#bw_progress",
            "#size_binding_x",
        );
        nodes.push(named(
            "progress",
            anchored(
                json!({"type":"panel","controls":[
                    {"track": rect([WIDTH, 6.], [0., 0.], [1., 1., 1., 0.08], 3.)},
                    {"fill": fill},
                ]}),
                [WIDTH, 6.],
                [PAD, STRIP + 40.],
            ),
        ));
        nodes.push(named(
            "progress_label",
            text(
                "#bw_progress_label",
                [WIDTH, 10.],
                [PAD, STRIP + 50.],
                MUTED,
                "center",
            ),
        ));
        let search = controls.text(SEARCH_ID, "Search");
        let field = [WIDTH, 20.];
        nodes.push(named(
            "search",
            routed(search, "edit", field, [PAD, STRIP + 66.], || {
                vec![
                    named("face", framed(field, [0., 0.], FIELD)),
                    named(
                        "placeholder",
                        visible(
                            text(
                                "Search blocks...",
                                [WIDTH - 16., 10.],
                                [8., 5.],
                                MUTED,
                                "left",
                            ),
                            "#bw_search_empty",
                        ),
                    ),
                    named(
                        "value",
                        visible(
                            text(
                                &format!("#row_{search}_value"),
                                [WIDTH - 16., 10.],
                                [8., 5.],
                                TEXT,
                                "left",
                            ),
                            "#bw_search_full",
                        ),
                    ),
                ]
            }),
        ));
        let head = STRIP + 94.;
        nodes.extend([
            named(
                "head_block",
                text("Block", [200., 10.], [PAD + 18., head], MUTED, "left"),
            ),
            named(
                "head_needed",
                text(
                    "Needed",
                    [70., 10.],
                    [WINDOW[0] - PAD - 150., head],
                    MUTED,
                    "right",
                ),
            ),
            named(
                "head_total",
                text(
                    "Total",
                    [70., 10.],
                    [WINDOW[0] - PAD - 70., head],
                    MUTED,
                    "right",
                ),
            ),
        ]);
        for row in 0..ROWS {
            let y = head + 16. + 16. * row as f64;
            let field = |name: &str| format!("#bw_row{row}_{name}");
            let icon = bind(
                anchored(
                    json!({"type":"custom","renderer":"inventory_item_renderer"}),
                    [12., 12.],
                    [0., 1.],
                ),
                &field("icon"),
                "#item_renderer_data",
            );
            let mut items = vec![
                named("icon", icon),
                named(
                    "name",
                    text(&field("name"), [240., 10.], [18., 2.], TEXT, "left"),
                ),
                named(
                    "needed",
                    text(
                        &field("needed"),
                        [70., 10.],
                        [WIDTH - 150., 2.],
                        TEXT,
                        "right",
                    ),
                ),
                named(
                    "total",
                    text(
                        &field("total"),
                        [70., 10.],
                        [WIDTH - 70., 2.],
                        MUTED,
                        "right",
                    ),
                ),
            ];
            if row > 0 {
                items.push(named("rule", rect([WIDTH, 1.], [0., -1.], LINE, 0.)));
            }
            let line = anchored(
                json!({"type":"panel","controls":items}),
                [WIDTH, 14.],
                [PAD, y],
            );
            nodes.push(named(format!("row{row}"), visible(line, &field("visible"))));
        }
        nodes.push(named(
            "empty",
            visible(
                text(
                    "#bw_empty",
                    [WIDTH, 10.],
                    [PAD, head + 40.],
                    MUTED,
                    "center",
                ),
                "#bw_empty_visible",
            ),
        ));
        nodes.push(footer("Scroll for more  ·  P to close"));
        nodes
    }

    /// Import: the import button and every library entry; the loaded one is tinted green.
    fn import(controls: &mut Controls) -> Vec<Value> {
        let mut nodes = heading("bw_lib");
        let size = [180., 22.];
        nodes.push(named(
            "import",
            button(
                controls,
                "import",
                "Import schematics...",
                size,
                [(WINDOW[0] - size[0]) / 2., STRIP + 40.],
                ACCENT,
                TEXT,
                None,
            ),
        ));
        for row in 0..LIBRARY_ROWS {
            let y = STRIP + 72. + 24. * row as f64;
            let state = |name: &str| format!("#bw_lib{row}_{name}");
            let name = |color, flag: &str| {
                visible(
                    text(
                        &format!("#bw_lib_name{row}"),
                        [220., 10.],
                        [0., 5.],
                        color,
                        "left",
                    ),
                    &state(flag),
                )
            };
            let mut items = vec![
                named(
                    "highlight",
                    visible(
                        rect([WIDTH + 12., 22.], [-6., -1.], LOADED, RADIUS),
                        &state("loaded"),
                    ),
                ),
                named("name", name(TEXT, "plain")),
                named("name_loaded", name(GREEN, "loaded")),
                named(
                    "state",
                    text(
                        &format!("#entryMeta{row}"),
                        [120., 10.],
                        [WIDTH - 250., 5.],
                        MUTED,
                        "right",
                    ),
                ),
                named(
                    "load",
                    button(
                        controls,
                        &format!("toggle-{row}"),
                        &format!("#entryStatus{row}"),
                        [60., 20.],
                        [WIDTH - 124., 0.],
                        BUTTON,
                        TEXT,
                        Some(&state("active")),
                    ),
                ),
                named(
                    "delete",
                    button(
                        controls,
                        &format!("delete-{row}"),
                        "Delete",
                        [60., 20.],
                        [WIDTH - 60., 0.],
                        BUTTON,
                        DANGER,
                        None,
                    ),
                ),
            ];
            if row > 0 {
                items.push(named("rule", rect([WIDTH, 1.], [0., -2.], LINE, 0.)));
            }
            let line = anchored(
                json!({"type":"panel","controls":items}),
                [WIDTH, 20.],
                [PAD, y],
            );
            nodes.push(named(
                format!("entry{row}"),
                visible(line, &format!("#visible{row}")),
            ));
        }
        nodes.push(footer("Up to 8 schematics  ·  P to close"));
        nodes
    }

    /// One settings row: its title and a pill switch, all one click target. The green fill
    /// fades and the knob slides with the host's eased toggle progress.
    fn switch_row(
        controls: &mut Controls,
        key: &str,
        title: &str,
        offset: [f64; 2],
        width: f64,
    ) -> Value {
        let index = controls.toggle(&format!("{SETTING_PREFIX}{key}"), title);
        let row = |field: &str| format!("#row_{index}_{field}");
        routed(index, "control", [width, 22.], offset, || {
            let track = rect(SWITCH, [0., 0.], SWITCH_OFF, SWITCH[1] / 2.);
            let fill = bind(
                rect(SWITCH, [0., 0.], SWITCH_ON, SWITCH[1] / 2.),
                &row("toggle_progress"),
                "#alpha",
            );
            let knob = bind(
                rect([12., 12.], [2., 2.], TEXT, 6.),
                &row("knob_offset"),
                "#offset",
            );
            let switch = anchored(
                json!({"type":"panel","controls":[{"track":track},{"fill":fill},{"knob":knob}]}),
                SWITCH,
                [width - SWITCH[0], 3.],
            );
            vec![
                named(
                    "title",
                    text(title, [width - 40., 10.], [0., 6.], TEXT, "left"),
                ),
                named("switch", switch),
            ]
        })
    }

    /// Settings: two columns of switches and the Fast Place mode button.
    fn settings(controls: &mut Controls) -> Vec<Value> {
        let mut nodes = heading("bw_set");
        let column = (WIDTH - 24.) / 2.;
        let rows = SETTINGS.len().div_ceil(2);
        for (index, (key, title)) in SETTINGS.iter().enumerate() {
            let x = PAD + (index / rows) as f64 * (column + 24.);
            let y = STRIP + 40. + 30. * (index % rows) as f64;
            nodes.push(named(
                format!("set_{key}"),
                switch_row(controls, key, title, [x, y], column),
            ));
            if index % rows > 0 {
                nodes.push(named(
                    format!("rule_{key}"),
                    rect([column, 1.], [x, y - 4.], LINE, 0.),
                ));
            }
        }
        let y = STRIP + 40. + 30. * rows as f64 + 8.;
        nodes.push(named(
            "mode_rule",
            rect([WIDTH, 1.], [PAD, y - 6.], LINE, 0.),
        ));
        nodes.push(named(
            "mode_title",
            text("Fast Place mode", [200., 10.], [PAD, y + 6.], TEXT, "left"),
        ));
        nodes.push(named(
            "mode_hint",
            text(
                "V1 places like normal  ·  V2 adds air placement",
                [WIDTH, 10.],
                [PAD, y + 26.],
                MUTED,
                "left",
            ),
        ));
        nodes.push(named(
            "mode",
            button(
                controls,
                MODE_ID,
                "#bw_fp_mode",
                [120., 22.],
                [WINDOW[0] - PAD - 120., y],
                BUTTON,
                TEXT,
                None,
            ),
        ));
        nodes.push(footer("Changes save right away  ·  P to close"));
        nodes
    }

    /// One Controls row: the action and its key in a small chip.
    fn keys(controls: &mut Controls) -> Vec<Value> {
        let mut nodes = heading("bw_keys");
        let rows = [
            [
                ("place", "Place here"),
                ("ccw", "Rotate left"),
                ("cw", "Rotate right"),
            ],
            [("x-minus", "X −"), ("y-minus", "Y −"), ("z-minus", "Z −")],
            [("x-plus", "X +"), ("y-plus", "Y +"), ("z-plus", "Z +")],
            [
                ("previous-layer", "Previous layer"),
                ("mode", "Layer mode"),
                ("next-layer", "Next layer"),
            ],
            [
                ("mirror", "Mirror"),
                ("lock", "#bw_lock"),
                ("visibility", "#bw_visibility"),
            ],
            [
                ("verifier", "Verifier"),
                ("paste", "#bw_paste"),
                ("hotkeys", "Hotkeys"),
            ],
        ];
        let width = (WIDTH - 16.) / 3.;
        for (row, commands) in rows.into_iter().enumerate() {
            for (col, (id, title)) in commands.into_iter().enumerate() {
                let size = [width, 24.];
                let offset = [
                    PAD + col as f64 * (width + 8.),
                    STRIP + 40. + row as f64 * 30.,
                ];
                let index = controls.button(id);
                let click = json!({
                    "type": "button", "size": size,
                    "default_control": "idle", "hover_control": "hover",
                    "pressed_control": "pressed",
                    "button_mappings": [{"from_button_id":"button.menu_select",
                        "to_button_id":format!("mod.control:{index}"), "mapping_type":"pressed"}],
                    "controls": [
                        {"idle": {"type":"panel", "size":size}},
                        {"hover": rect(size, [0., 0.], [1., 1., 1., 0.08], RADIUS)},
                        {"pressed": rect(size, [0., 0.], [0., 0., 0., 0.25], RADIUS)},
                    ]
                });
                nodes.push(named(
                    id,
                    anchored(
                        json!({"type":"panel", "controls":[
                            {"face": framed(size, [0., 0.], BUTTON)},
                            {"caption": text(title, [width,10.], [0.,7.], TEXT, "center")},
                            {"click": click},
                        ]}),
                        size,
                        offset,
                    ),
                ));
            }
        }
        nodes.push(footer("P closes this window"));
        nodes
    }

    /// The JSON-UI document and its routed controls.
    fn document() -> (String, Vec<Value>) {
        let mut controls = Controls::default();
        let mut card = vec![named("background", framed(WINDOW, [0., 0.], CARD))];
        card.extend(strip(&mut controls));
        card.push(named("materials", body(0, materials(&mut controls))));
        card.push(named("import", body(1, import(&mut controls))));
        card.push(named("settings", body(2, settings(&mut controls))));
        card.push(named("controls", body(3, keys(&mut controls))));
        let document = json!({
            "namespace": "browser",
            "root": {"type":"screen","size":["100%","100%"],"controls":[
                {"backdrop": {"type":"custom","renderer":"cinnabar_rounded_rectangle",
                    "size":["100%","100%"],"color":BACKDROP,"radius":0}},
                {"window": {"type":"panel","size":WINDOW,"anchor_from":"center","anchor_to":"center",
                    "controls":card}},
            ]},
        });
        (document.to_string(), controls.0)
    }

    /// The whole panel payload stored in [`super::LAYOUT`].
    pub(super) fn layout() -> String {
        let (document, controls) = document();
        let ease = json!({"duration":0.18,"easing":"out_cubic"});
        json!({
            "reference_size": [WINDOW[0] + 40., WINDOW[1] + 40.],
            "motion": true,
            "controls": controls,
            "surface": {"screen":"browser.root","document":document,"bindings":{},
                "transitions":{"#bw_tab_x":ease,"#bw_progress":ease}},
        })
        .to_string()
    }
}

/// `minecraft:mossy_stone_bricks` reads as `Mossy stone bricks`.
fn display_name(identifier: &str) -> String {
    let name = identifier
        .split_once(':')
        .map_or(identifier, |(_, name)| name);
    let mut words = name.replace('_', " ");
    if let Some(first) = words.get(..1) {
        let upper = first.to_ascii_uppercase();
        words.replace_range(..1, &upper);
    }
    words
}

impl Viewer {
    /// Material indices in alphabetical order, keeping only names containing the search.
    fn browser_materials(&self) -> Vec<(usize, String)> {
        let search = self.browser_search.trim().to_lowercase();
        let mut lines: Vec<_> = self
            .materials
            .iter()
            .enumerate()
            .map(|(index, line)| (index, display_name(&line.name)))
            .filter(|(_, name)| search.is_empty() || name.to_lowercase().contains(&search))
            .collect();
        lines.sort_by(|a, b| a.1.cmp(&b.1));
        lines
    }

    /// Moves the material list a view up or down; returns whether it moved.
    pub(super) fn browser_scroll(&mut self, down: bool) -> bool {
        if self.browser_tab != Tab::Materials {
            return false;
        }
        let last = self
            .browser_materials()
            .len()
            .div_ceil(ROWS)
            .saturating_sub(1);
        let next = if down {
            (self.material_page + 1).min(last)
        } else {
            self.material_page.saturating_sub(1)
        };
        let changed = next != self.material_page;
        self.material_page = next;
        changed
    }

    /// Takes a new search text; returns whether the list changed.
    pub(super) fn browser_search_changed(&mut self, id: &str, value: String) -> bool {
        if id != SEARCH_ID || value == self.browser_search {
            return false;
        }
        self.browser_search = value;
        self.material_page = 0;
        true
    }

    /// Handles one click in the window: a tab switch, a settings switch carrying its new
    /// state, the Fast Place mode button, or a library action.
    pub(super) fn browser_event(&mut self, id: &str, value: f32) {
        if let Some(tab) = Tab::ALL.into_iter().find(|tab| tab.id() == id) {
            self.browser_tab = tab;
        } else if let Some(key) = id.strip_prefix(SETTING_PREFIX) {
            self.browser_setting(key, value >= 0.5);
        } else if id == MODE_ID {
            self.options.strict_place = !self.options.strict_place;
            self.options.save();
        } else if id != SEARCH_ID {
            self.menu_action(id);
        }
    }

    /// The current state of a Settings tab switch.
    fn browser_setting_state(&self, key: &str) -> bool {
        let options = &self.options;
        match key {
            "look" => options.textured,
            "assist" => options.assist,
            "remember-placement" => options.remember_placement,
            "paste-destroy" => options.paste_destroy,
            "fast-place" => options.fast_place,
            "hold-place" => options.hold_place,
            "correct-aim" => options.correct_aim,
            "bridge-place" => options.bridge_place,
            "auto-layer" => options.auto_layer,
            _ => false,
        }
    }

    /// Sets one option. Fast Place is set directly, keeping its mode; the rest reuse their
    /// menu actions, which flip the option with its side effects, only on a change.
    fn browser_setting(&mut self, key: &str, on: bool) {
        if self.browser_setting_state(key) == on {
            return;
        }
        if key == "fast-place" {
            self.options.fast_place = on;
            self.options.save();
        } else {
            self.menu_action(key);
        }
    }

    /// Blocks still needed for one material: scanned counts once the guide is scanned,
    /// otherwise every block of that material.
    fn browser_needed(&self, line: &schematica_core::MaterialLine, scanned: bool) -> u64 {
        if !scanned {
            return line.total;
        }
        let indices = self.preview.material_indices(&line.name);
        self.tiles
            .iter()
            .map(|tile| u64::from(tile.material_counts(&indices)[0]))
            .sum()
    }
}

/// Fills the window's bindings and switch values: tabs, then the open tab's content.
pub fn bindings(viewer: &Viewer, out: &mut Map<String, Value>, controls: &mut Vec<Value>) {
    for (index, tab) in Tab::ALL.into_iter().enumerate() {
        let open = tab == viewer.browser_tab;
        out.insert(format!("#bw_tab{index}_open"), json!(open));
        out.insert(format!("#bw_tab{index}_closed"), json!(!open));
        out.insert(format!("#bw_body{index}"), json!(open));
    }
    out.insert(
        "#bw_tab_x".into(),
        json!(viewer.browser_tab.index() as f64 * (TAB_WIDTH + TAB_GAP)),
    );
    match viewer.browser_tab {
        Tab::Materials => materials(viewer, out),
        Tab::Import => import(viewer, out),
        Tab::Settings => settings(viewer, out, controls),
        Tab::Controls => keys(viewer, out),
    }
}

fn materials(viewer: &Viewer, out: &mut Map<String, Value>) {
    let scanned = !viewer.dirty && viewer.tiles.iter().any(|tile| tile.stamp.is_some());
    let total: u64 = viewer.materials.iter().map(|line| line.total).sum();
    // Per-material counts walk every tile, so only the rows in view compute them; the
    // totals read the tiles' running tallies.
    let needed: u64 = if scanned {
        viewer
            .tiles
            .iter()
            .map(|tile| tile.tally.missing + tile.tally.wrong)
            .sum()
    } else {
        total
    };
    let title = if viewer.loaded {
        viewer.schematic.name.clone()
    } else {
        "No schematic loaded".into()
    };
    let summary = if viewer.materials.is_empty() {
        "Load a schematic to see the blocks it needs.".into()
    } else {
        format!(
            "{needed} blocks needed  ·  {} different blocks",
            viewer.materials.len()
        )
    };
    let placed = total.saturating_sub(needed);
    let progress = if total == 0 {
        0.
    } else {
        placed as f64 / total as f64
    };
    let progress_label = if viewer.materials.is_empty() {
        String::new()
    } else if scanned {
        format!(
            "{}% built  ·  {placed} of {total} placed",
            (progress * 100.).floor()
        )
    } else {
        "Place the schematic to track progress".into()
    };
    out.insert("#bw_title".into(), json!(title));
    out.insert("#bw_summary".into(), json!(summary));
    out.insert("#bw_progress".into(), json!(progress));
    out.insert("#bw_progress_label".into(), json!(progress_label));
    let searching = !viewer.browser_search.is_empty();
    out.insert("#bw_search_empty".into(), json!(!searching));
    out.insert("#bw_search_full".into(), json!(searching));
    let lines = viewer.browser_materials();
    let pages = lines.len().div_ceil(ROWS).max(1);
    let first = viewer.material_page.min(pages - 1) * ROWS;
    let empty = if !viewer.materials.is_empty() && lines.is_empty() {
        format!("No blocks match \"{}\"", viewer.browser_search.trim())
    } else {
        String::new()
    };
    out.insert("#bw_empty_visible".into(), json!(!empty.is_empty()));
    out.insert("#bw_empty".into(), json!(empty));
    for row in 0..ROWS {
        let entry = lines.get(first + row);
        let line = entry.map(|(index, _)| &viewer.materials[*index]);
        out.insert(format!("#bw_row{row}_visible"), json!(line.is_some()));
        out.insert(
            format!("#bw_row{row}_icon"),
            crate::menu::material_icon(line),
        );
        out.insert(
            format!("#bw_row{row}_name"),
            json!(entry.map_or("", |(_, name)| name.as_str())),
        );
        out.insert(
            format!("#bw_row{row}_needed"),
            json!(line.map_or(String::new(), |line| {
                viewer.browser_needed(line, scanned).to_string()
            })),
        );
        out.insert(
            format!("#bw_row{row}_total"),
            json!(line.map_or(String::new(), |line| line.total.to_string())),
        );
    }
}

fn import(viewer: &Viewer, out: &mut Map<String, Value>) {
    let mut count = 0;
    for row in 0..LIBRARY_ROWS {
        let index = viewer.visible_entry(row);
        count += usize::from(index.is_some());
        let loaded = viewer.loaded && index.is_some() && viewer.active_entry == index;
        let active = loaded && viewer.shown && viewer.placement.is_some();
        // Only what these rows show; the old menu's dashboard data is much larger.
        out.insert(format!("#visible{row}"), json!(index.is_some()));
        out.insert(
            format!("#entryMeta{row}"),
            json!(if loaded { "Loaded" } else { "" }),
        );
        out.insert(
            format!("#entryStatus{row}"),
            json!(if active {
                "Active"
            } else if loaded {
                "Hidden"
            } else {
                "Load"
            }),
        );
        out.insert(format!("#bw_lib{row}_loaded"), json!(loaded));
        out.insert(format!("#bw_lib{row}_plain"), json!(!loaded));
        out.insert(format!("#bw_lib{row}_active_on"), json!(active));
        out.insert(format!("#bw_lib{row}_active_off"), json!(!active));
        // Longer than the old menu's names; the window's rows have the room.
        let name = index
            .and_then(|index| viewer.library.get(index))
            .map_or(String::new(), |name| name.chars().take(32).collect());
        out.insert(format!("#bw_lib_name{row}"), json!(name));
    }
    out.insert("#bw_lib_title".into(), json!("Library"));
    out.insert(
        "#bw_lib_summary".into(),
        json!(if count == 0 {
            "Import .schem, .schematic, .litematic or .mcstructure files.".to_owned()
        } else {
            format!("{count} of {LIBRARY_ROWS} schematics  ·  Load one to build it")
        }),
    );
}

fn keys(viewer: &Viewer, out: &mut Map<String, Value>) {
    out.insert("#bw_keys_title".into(), json!("Hologram controls"));
    out.insert(
        "#bw_keys_summary".into(),
        json!(viewer.placement.map_or_else(
            || "Load a schematic to adjust its placement".into(),
            |p| format!(
                "{} {} {}  ·  {}°  ·  {:?}  ·  {}",
                p.origin[0],
                p.origin[1],
                p.origin[2],
                p.quarter_turns * 90,
                p.mirror,
                viewer.layers_text()
            ),
        )),
    );
    out.insert(
        "#bw_lock".into(),
        json!(if viewer.locked { "Unlock" } else { "Lock" }),
    );
    out.insert(
        "#bw_visibility".into(),
        json!(if viewer.shown {
            "Hide guide"
        } else {
            "Show guide"
        }),
    );
    out.insert(
        "#bw_paste".into(),
        json!(if viewer.paste_confirm {
            "Confirm paste"
        } else {
            "Paste"
        }),
    );
}

fn settings(viewer: &Viewer, out: &mut Map<String, Value>, controls: &mut Vec<Value>) {
    out.insert("#bw_set_title".into(), json!("Settings"));
    out.insert(
        "#bw_set_summary".into(),
        json!("Placement assists and how the guide looks"),
    );
    out.insert(
        "#bw_fp_mode".into(),
        json!(if viewer.options.strict_place {
            "V2 Strict"
        } else {
            "V1 Classic"
        }),
    );
    for (key, _) in SETTINGS {
        controls.push(json!({
            "id": format!("{SETTING_PREFIX}{key}"),
            "value": viewer.browser_setting_state(key),
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precomputed_layout_matches_the_code() {
        if std::env::var_os("SCHEMATICA_BROWSER_REGENERATE").is_some() {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/ui/browser.json");
            std::fs::write(path, layout::layout()).unwrap();
            return;
        }
        assert_eq!(LAYOUT, layout::layout(), "regenerate ui/browser.json");
        // The host refuses surface documents over 192 KiB.
        let panel: Value = serde_json::from_str(LAYOUT).unwrap();
        let document = panel["surface"]["document"].as_str().unwrap();
        assert!(
            document.len() <= 192 * 1024,
            "document is {} bytes",
            document.len()
        );
    }

    #[test]
    fn layout_routes_every_control() {
        let panel: Value = serde_json::from_str(LAYOUT).unwrap();
        let ids: Vec<_> = panel["controls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|control| control["id"].as_str().unwrap())
            .collect();
        for tab in Tab::ALL {
            assert!(ids.contains(&tab.id()));
        }
        // The host only lets the picker open from a click on a control named `import`.
        for id in ["import", SEARCH_ID, MODE_ID] {
            assert!(ids.contains(&id), "{id} missing");
        }
        for row in 0..LIBRARY_ROWS {
            assert!(ids.contains(&format!("toggle-{row}").as_str()));
            assert!(ids.contains(&format!("delete-{row}").as_str()));
        }
        for (key, _) in SETTINGS {
            assert!(ids.contains(&format!("{SETTING_PREFIX}{key}").as_str()));
        }
    }

    #[test]
    fn materials_are_alphabetical_searchable_and_scroll() {
        let mut viewer = crate::tests::reference_viewer();
        let mut out = Map::new();
        bindings(&viewer, &mut out, &mut Vec::new());
        let names: Vec<_> = viewer
            .browser_materials()
            .into_iter()
            .map(|(_, name)| name)
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        assert_eq!(out["#bw_row0_name"], names[0]);
        assert_eq!(out["#bw_body0"], true);
        assert_eq!(out["#bw_search_empty"], true);

        let start = names[0].to_lowercase()[..3].to_owned();
        assert!(viewer.browser_search_changed(SEARCH_ID, start.to_uppercase()));
        assert!(
            viewer
                .browser_materials()
                .iter()
                .all(|(_, name)| name.to_lowercase().contains(&start))
        );
        assert!(viewer.browser_search_changed(SEARCH_ID, "zzz-none".into()));
        let mut out = Map::new();
        bindings(&viewer, &mut out, &mut Vec::new());
        assert_eq!(out["#bw_row0_visible"], false);
        assert_eq!(out["#bw_empty_visible"], true);

        assert!(viewer.browser_search_changed(SEARCH_ID, String::new()));
        let line = viewer.materials[0].clone();
        viewer.materials = (0..ROWS + 1).map(|_| line.clone()).collect();
        assert!(viewer.browser_scroll(true));
        let mut out = Map::new();
        bindings(&viewer, &mut out, &mut Vec::new());
        assert_eq!(out["#bw_row0_visible"], true);
        assert_eq!(out["#bw_row1_visible"], false);
    }

    #[test]
    fn tabs_switch_bodies_and_slide_the_underline() {
        let mut viewer = crate::tests::reference_viewer();
        viewer.browser_event(Tab::Import.id(), 1.);
        let mut out = Map::new();
        bindings(&viewer, &mut out, &mut Vec::new());
        assert_eq!(out["#bw_body0"], false);
        assert_eq!(out["#bw_body1"], true);
        assert_eq!(out["#bw_tab1_open"], true);
        assert_eq!(out["#bw_tab0_closed"], true);
        assert_eq!(out["#bw_tab_x"], TAB_WIDTH + TAB_GAP);
        assert_eq!(out["#bw_lib_title"], "Library");
        assert!(
            !viewer.browser_scroll(true),
            "only the materials tab scrolls"
        );
    }

    #[test]
    fn controls_tab_routes_placement_and_layer_actions() {
        let mut viewer = crate::tests::reference_viewer();
        viewer.browser_event(Tab::Controls.id(), 1.);
        let mut out = Map::new();
        bindings(&viewer, &mut out, &mut Vec::new());
        assert_eq!(out["#bw_body3"], true);
        assert_eq!(out["#bw_keys_title"], "Hologram controls");
        for id in ["x-minus", "cw", "mirror", "mode", "next-layer", "paste"] {
            let panel: Value = serde_json::from_str(LAYOUT).unwrap();
            assert!(
                panel["controls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|control| control["id"] == id),
                "{id}"
            );
        }
    }

    #[test]
    fn settings_switches_and_fast_place_mode() {
        let mut viewer = crate::tests::reference_viewer();
        viewer.browser_event(Tab::Settings.id(), 1.);
        let mut controls = Vec::new();
        bindings(&viewer, &mut Map::new(), &mut controls);
        assert_eq!(controls.len(), SETTINGS.len());
        for (key, _) in SETTINGS {
            let id = format!("{SETTING_PREFIX}{key}");
            let on = viewer.browser_setting_state(key);
            viewer.browser_event(&id, f32::from(u8::from(!on)));
            assert_eq!(
                viewer.browser_setting_state(key),
                !on,
                "{key} did not change"
            );
            // A repeated event with the same state changes nothing.
            viewer.browser_event(&id, f32::from(u8::from(!on)));
            assert_eq!(
                viewer.browser_setting_state(key),
                !on,
                "{key} flipped twice"
            );
        }
        let strict = viewer.options.strict_place;
        viewer.browser_event(MODE_ID, 1.);
        assert_eq!(viewer.options.strict_place, !strict);
        let mut out = Map::new();
        bindings(&viewer, &mut out, &mut Vec::new());
        let label = if strict { "V1 Classic" } else { "V2 Strict" };
        assert_eq!(out["#bw_fp_mode"], label);
    }

    #[test]
    fn identifiers_read_as_plain_words() {
        assert_eq!(
            display_name("minecraft:mossy_stone_bricks"),
            "Mossy stone bricks"
        );
        assert_eq!(display_name("glass"), "Glass");
    }
}
