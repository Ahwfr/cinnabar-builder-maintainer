//! The tabbed window opened with the menu key (M by default): a dimmed backdrop and one card whose top strip holds the
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
/// Tabs the Cinnamatica client draws natively with OreUI; the rest keep this JSON-UI layout.
const NATIVE_TABS: [Tab; 4] = Tab::ALL;
/// The binding that asks the client for its native window, named as the client reads it.
const NATIVE_BINDING: &str = "#cinnamatica_native";

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
const SETTINGS: [(&str, &str); 10] = [
    ("hold-place", "Hold to place"),
    ("placement-lock", "Placement lock"),
    ("look", "Textured ghost blocks"),
    ("assist", "Target info"),
    ("remember-placement", "Remember positions"),
    ("paste-destroy", "Replace wrong blocks"),
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
    /// The line under the tab strip; tab bodies start below it.
    const STRIP: f64 = 30.;
    const TAB_HEIGHT: f64 = 24.;
    const PAD: f64 = 12.;
    const WIDTH: f64 = WINDOW[0] - PAD * 2.;
    /// The heading row: the title, then the summary under it.
    const TITLE_Y: f64 = 36.;
    const SUMMARY_Y: f64 = 47.;
    const FOOTER_Y: f64 = 314.;
    const FONT: &str = "mod_panel_sans";

    /// An opaque colour from `0xRRGGBB`.
    const fn hex(rgb: u32) -> [f64; 4] {
        [
            ((rgb >> 16) & 255) as f64 / 255.,
            ((rgb >> 8) & 255) as f64 / 255.,
            (rgb & 255) as f64 / 255.,
            1.,
        ]
    }

    const fn alpha(color: [f64; 4], alpha: f64) -> [f64; 4] {
        [color[0], color[1], color[2], alpha]
    }

    const BG: [f64; 4] = alpha(hex(0x2B2E36), 0.6);
    const CARD: [f64; 4] = hex(0x363A44);
    const PANEL: [f64; 4] = hex(0x2E3139);
    const BORDER: [f64; 4] = hex(0x5A5F6B);
    const TAB_ACTIVE: [f64; 4] = hex(0x3D414B);
    const INPUT: [f64; 4] = hex(0x3A3D45);
    const INPUT_EDGE: [f64; 4] = hex(0x7A7F8A);
    const TEXT: [f64; 4] = hex(0xF2F3F5);
    const MUTED: [f64; 4] = hex(0xC4C7CE);
    const FOOTER: [f64; 4] = hex(0xB9BCC3);
    /// The active tab underline, ON, progress, Loaded and zero needed.
    const ACCENT: [f64; 4] = hex(0x7FD88A);
    const GREEN: [f64; 4] = ACCENT;
    const OFF: [f64; 4] = hex(0xEE9A9A);
    const NEEDED: [f64; 4] = hex(0xF0B0A0);
    /// A light hover wash and a dark pressed shade over clickable areas.
    const WASH: [f64; 4] = [1., 1., 1., 0.06];
    const SHADE: [f64; 4] = [0., 0., 0., 0.25];
    const SWITCH_OFF: [f64; 4] = BORDER;
    /// Switch track; the host slides the knob from x 2 to x 12 as a toggle eases on.
    const SWITCH: [f64; 2] = [26., 16.];

    /// What each Settings row does, shown in the hint box while the row is hovered.
    const HINTS: [(&str, &str); 12] = [
        (
            "fast-place",
            "Places the targeted ghost block for you, picking the material from the hotbar, inventory or Creative catalog.",
        ),
        (
            MODE_ID,
            "V1 Classic places like a normal click. V2 Strict checks the block's state and can place in the air.",
        ),
        (
            "hold-place",
            "Keeps placing while right-click is held, following the crosshair.",
        ),
        (
            "placement-lock",
            "Blocks normal placement outside the targeted ghost or with the wrong block. Not the same as Lock guide position.",
        ),
        (
            "correct-aim",
            "Turns the facing sent with a placement to match the schematic, without moving your camera.",
        ),
        (
            "bridge-place",
            "Lets a single click use a nearby ghost and support face. Hold to place uses the direct aim.",
        ),
        (
            "look",
            "Textured ghosts look like the real blocks. Off shows simple coloured markers.",
        ),
        (
            "assist",
            "Shows the targeted material and its coordinates on screen.",
        ),
        (
            "lightness",
            "How strongly the ghost blocks are washed with light blue.",
        ),
        (
            "remember-placement",
            "Restores where a schematic was placed when you come back to the same world.",
        ),
        (
            "auto-layer",
            "Moves to the next layer once the current single layer is complete.",
        ),
        (
            "paste-destroy",
            "Replace mismatched blocks and orientations. Preserve already correct blocks.",
        ),
    ];

    /// A bevelled button's face, light top edge, dark bottom edge and hover face.
    struct Style {
        face: [f64; 4],
        hi: [f64; 4],
        lo: [f64; 4],
        hover: [f64; 4],
    }

    const PLAIN: Style = Style {
        face: hex(0x74787F),
        hi: hex(0xA6A9B0),
        lo: hex(0x5A5F6B),
        hover: hex(0x7D8FC9),
    };
    const DELETE: Style = Style {
        face: hex(0x9A5656),
        hi: hex(0xBF8282),
        lo: hex(0x6A3A3A),
        hover: hex(0xAA6262),
    };
    const BUTTON_EDGE: [f64; 4] = hex(0x3A3D45);

    /// Rounds layout numbers so the precomputed document stays small.
    fn round(values: &[f64]) -> Value {
        json!(
            values
                .iter()
                .map(|value| (value * 1000.).round() / 1000.)
                .collect::<Vec<_>>()
        )
    }

    fn anchored(mut node: Value, size: [f64; 2], offset: [f64; 2]) -> Value {
        node["size"] = round(&size);
        node["offset"] = round(&offset);
        node["anchor_from"] = json!("top_left");
        node["anchor_to"] = json!("top_left");
        node
    }

    fn panel(controls: Vec<Value>, size: [f64; 2], offset: [f64; 2]) -> Value {
        anchored(json!({"type":"panel","controls":controls}), size, offset)
    }

    fn rect(size: [f64; 2], offset: [f64; 2], color: [f64; 4], radius: f64) -> Value {
        anchored(
            json!({"type":"custom","renderer":"cinnabar_rounded_rectangle",
                "color":round(&color),"radius":radius}),
            size,
            offset,
        )
    }

    /// A square box with a 1 px border.
    fn outlined(size: [f64; 2], offset: [f64; 2], fill: [f64; 4], border: [f64; 4]) -> Value {
        let mut node = rect(size, offset, fill, 0.);
        node["border_color"] = round(&border);
        node["border_width"] = json!(1);
        node
    }

    /// A group box: the panel fill with the window's border colour.
    fn framed(size: [f64; 2], offset: [f64; 2]) -> Value {
        outlined(size, offset, PANEL, BORDER)
    }

    /// A label; a `#name` text is bound to that name.
    fn text(text: &str, size: [f64; 2], offset: [f64; 2], color: [f64; 4], align: &str) -> Value {
        let mut label = anchored(
            json!({"type":"label","text":text,"color":round(&color),"shadow":false,
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

    /// The click target routed to control `index`. Only its hover and pressed overlays live
    /// in the button; what it sits on is drawn once beside it, which keeps the document small.
    fn hit(index: usize, route: &str, size: [f64; 2], offset: [f64; 2], hover: Value) -> Value {
        anchored(
            json!({"type":"button","default_control":"idle","hover_control":"hover",
            "pressed_control":"pressed",
            "button_mappings":[{"from_button_id":"button.menu_select",
                "to_button_id":format!("mod.{route}:{index}"),"mapping_type":"pressed"}],
            "controls":[
                {"idle": panel(Vec::new(), size, [0., 0.])},
                {"hover": hover},
                {"pressed": rect(size, [0., 0.], SHADE, 0.)},
            ]}),
            size,
            offset,
        )
    }

    /// The light hover wash, fading with the host's eased hover of control `index`.
    fn wash(index: usize, size: [f64; 2]) -> Value {
        bind(
            rect(size, [0., 0.], WASH, 0.),
            &format!("#row_{index}_hover"),
            "#alpha",
        )
    }

    /// A centred caption for a control of `size`.
    fn caption(text_value: &str, size: [f64; 2], color: [f64; 4]) -> Value {
        text(
            text_value,
            [size[0], 10.],
            [0., (size[1] - 10.) / 2.],
            color,
            "center",
        )
    }

    /// A square bevelled button: a face, a hover face, a light top and a dark bottom edge,
    /// then `labels` on top.
    fn button(
        controls: &mut Controls,
        id: &str,
        size: [f64; 2],
        offset: [f64; 2],
        style: &Style,
        labels: Vec<Value>,
    ) -> Value {
        let index = controls.button(id);
        let [width, height] = size;
        let hover = rect([width - 2., height - 2.], [1., 1.], style.hover, 0.);
        let mut items = vec![
            named("face", outlined(size, [0., 0.], style.face, BUTTON_EDGE)),
            named("hit", hit(index, "control", size, [0., 0.], hover)),
            named("hi", rect([width - 2., 1.], [1., 1.], style.hi, 0.)),
            named(
                "lo",
                rect([width - 2., 1.], [1., height - 2.], style.lo, 0.),
            ),
        ];
        items.extend(
            labels
                .into_iter()
                .enumerate()
                .map(|(index, label)| named(format!("label{index}"), label)),
        );
        panel(items, size, offset)
    }

    /// A button with one caption.
    fn plain(
        controls: &mut Controls,
        id: &str,
        title: &str,
        size: [f64; 2],
        offset: [f64; 2],
    ) -> Value {
        button(
            controls,
            id,
            size,
            offset,
            &PLAIN,
            vec![caption(title, size, TEXT)],
        )
    }

    /// The tab strip: tabs centred for however many there are, the open one shaded, and an
    /// underline whose offset eases to the open tab.
    fn strip(controls: &mut Controls) -> Vec<Value> {
        let count = Tab::ALL.len() as f64;
        let group = count * TAB_WIDTH + (count - 1.) * TAB_GAP;
        let left = (WINDOW[0] - group) / 2.;
        let top = STRIP - TAB_HEIGHT;
        let size = [TAB_WIDTH, TAB_HEIGHT];
        let mut nodes: Vec<Value> = Tab::ALL
            .iter()
            .zip(TABS)
            .enumerate()
            .map(|(index, (tab, title))| {
                let x = left + index as f64 * (TAB_WIDTH + TAB_GAP);
                let routed = controls.button(tab.id());
                let state = |name: &str| format!("#bw_tab{index}_{name}");
                let items = vec![
                    named(
                        "shade",
                        visible(rect(size, [0., 0.], TAB_ACTIVE, 0.), &state("open")),
                    ),
                    named(
                        "hit",
                        hit(routed, "control", size, [0., 0.], wash(routed, size)),
                    ),
                    named("open", visible(caption(title, size, TEXT), &state("open"))),
                    named(
                        "closed",
                        visible(caption(title, size, MUTED), &state("closed")),
                    ),
                ];
                named(tab.id(), panel(items, size, [x, top]))
            })
            .collect();
        let spacer = bind(
            anchored(json!({"type":"panel"}), [0., 2.], [0., 0.]),
            "#bw_tab_x",
            "#size_binding_x_absolute",
        );
        let underline = rect([TAB_WIDTH, 2.], [0., 0.], ACCENT, 0.);
        nodes.push(named(
            "underline",
            anchored(
                json!({"type":"stack_panel","orientation":"horizontal",
                    "controls":[{"spacer":spacer},{"bar":underline}]}),
                [group, 2.],
                [left, STRIP - 2.],
            ),
        ));
        nodes.push(named(
            "divider",
            rect([WINDOW[0] - 20., 1.], [10., STRIP], BORDER, 0.),
        ));
        nodes
    }

    /// A tab body shown only while its tab is open.
    fn body(tab: usize, nodes: Vec<Value>) -> Value {
        visible(panel(nodes, WINDOW, [0., 0.]), &format!("#bw_body{tab}"))
    }

    /// The tab's title and its summary under it, on the left of the heading row; `width`
    /// leaves room for a control on the right.
    fn heading(prefix: &str, width: f64) -> Vec<Value> {
        vec![
            named(
                "title",
                text(
                    &format!("#{prefix}_title"),
                    [width, 10.],
                    [PAD, TITLE_Y],
                    TEXT,
                    "left",
                ),
            ),
            named(
                "summary",
                text(
                    &format!("#{prefix}_summary"),
                    [width, 10.],
                    [PAD, SUMMARY_Y],
                    MUTED,
                    "left",
                ),
            ),
        ]
    }

    /// The footer: context on the left, how to close on the right.
    fn footer(hint: &str) -> Value {
        named(
            "footer",
            panel(
                vec![
                    named("hint", text(hint, [300., 10.], [0., 0.], FOOTER, "left")),
                    named(
                        "close",
                        text(
                            "M or Esc to close",
                            [130., 10.],
                            [WIDTH - 130., 0.],
                            FOOTER,
                            "right",
                        ),
                    ),
                ],
                [WIDTH, 10.],
                [PAD, FOOTER_Y],
            ),
        )
    }

    /// A routed text field: a dark box, a placeholder while empty and the typed value.
    fn field(
        controls: &mut Controls,
        id: &str,
        placeholder: &str,
        size: [f64; 2],
        offset: [f64; 2],
    ) -> Value {
        let index = controls.text(id, placeholder);
        let line = [size[0] - 12., 10.];
        let top = (size[1] - 10.) / 2.;
        panel(
            vec![
                named("face", outlined(size, [0., 0.], INPUT, INPUT_EDGE)),
                named("hit", hit(index, "edit", size, [0., 0.], wash(index, size))),
                named(
                    "placeholder",
                    visible(
                        text(placeholder, line, [6., top], MUTED, "left"),
                        "#bw_search_empty",
                    ),
                ),
                named(
                    "value",
                    visible(
                        text(
                            &format!("#row_{index}_value"),
                            line,
                            [6., top],
                            TEXT,
                            "left",
                        ),
                        "#bw_search_full",
                    ),
                ),
            ],
            size,
            offset,
        )
    }

    /// Materials: search, progress and a paged, alphabetical list with icons.
    fn materials(controls: &mut Controls) -> Vec<Value> {
        let mut nodes = heading("bw", 280.);
        nodes.push(named(
            "search",
            field(controls, SEARCH_ID, "Filter...", [148., 18.], [300., 36.]),
        ));
        // The green bar fills its bound-width parent.
        let track = [288., 8.];
        let green = json!({"type":"custom","renderer":"cinnabar_rounded_rectangle",
            "color":round(&ACCENT),"radius":0,"size":["100%","100%"],"offset":[0,0],
            "anchor_from":"top_left","anchor_to":"top_left"});
        let fill = bind(
            panel(vec![named("green", green)], track, [0., 0.]),
            "#bw_progress",
            "#size_binding_x",
        );
        nodes.push(named(
            "progress",
            panel(
                vec![
                    named("track", rect(track, [0., 0.], INPUT, 0.)),
                    named("fill", fill),
                ],
                track,
                [PAD, 62.],
            ),
        ));
        nodes.push(named(
            "progress_label",
            text(
                "#bw_progress_label",
                [140., 10.],
                [308., 61.],
                MUTED,
                "left",
            ),
        ));
        nodes.push(named("list", framed([WIDTH, 228.], [PAD, 76.])));
        nodes.extend([
            named(
                "head_name",
                text("Material", [200., 10.], [42., 82.], MUTED, "left"),
            ),
            named(
                "head_have",
                text("Have", [80., 10.], [200., 82.], MUTED, "right"),
            ),
            named(
                "head_needed",
                text("Left", [80., 10.], [280., 82.], MUTED, "right"),
            ),
            named(
                "head_total",
                text("Total", [70., 10.], [370., 82.], MUTED, "right"),
            ),
            named("head_rule", rect([WIDTH - 8., 1.], [16., 96.], BORDER, 0.)),
        ]);
        for row in 0..ROWS {
            let field = |name: &str| format!("#bw_row{row}_{name}");
            let icon = bind(
                anchored(
                    json!({"type":"custom","renderer":"inventory_item_renderer"}),
                    [16., 16.],
                    [8., 4.],
                ),
                &field("icon"),
                "#item_renderer_data",
            );
            let needed = |color, flag: &str| {
                visible(
                    text(&field("needed"), [80., 10.], [268., 7.], color, "right"),
                    &field(flag),
                )
            };
            let items = vec![
                named("icon", icon),
                named(
                    "name",
                    text(&field("name"), [158., 10.], [30., 7.], TEXT, "left"),
                ),
                named(
                    "have",
                    text(&field("have"), [80., 10.], [188., 7.], TEXT, "right"),
                ),
                named("needed", needed(NEEDED, "need")),
                named("done", needed(GREEN, "done")),
                named(
                    "total",
                    text(&field("total"), [70., 10.], [358., 7.], MUTED, "right"),
                ),
            ];
            let line = panel(items, [WIDTH, 24.], [PAD, 98. + 24. * row as f64]);
            nodes.push(named(format!("row{row}"), visible(line, &field("visible"))));
        }
        nodes.extend([
            named(
                "page",
                text("#bw_page", [140., 10.], [300., 292.], MUTED, "right"),
            ),
            named(
                "empty",
                visible(
                    text("#bw_empty", [WIDTH, 10.], [PAD, 180.], MUTED, "center"),
                    "#bw_empty_visible",
                ),
            ),
            footer("Have is what you carry; Left is what still has to be placed, Done means all of it is in the world"),
        ]);
        nodes
    }

    /// Import: the import button, an active banner and a 2×4 grid of library cards.
    fn import(controls: &mut Controls) -> Vec<Value> {
        let mut nodes = heading("bw_lib", 300.);
        nodes.push(named(
            "import",
            plain(
                controls,
                "import",
                "Import schematic",
                [108., 20.],
                [340., 34.],
            ),
        ));
        let banner = [WIDTH, 18.];
        nodes.push(named(
            "banner",
            visible(
                panel(
                    vec![
                        named(
                            "box",
                            outlined(banner, [0., 0.], alpha(ACCENT, 0.12), ACCENT),
                        ),
                        named("tag", text("ACTIVE", [50., 10.], [8., 4.], ACCENT, "left")),
                        named(
                            "text",
                            text("#bw_lib_banner", [370., 10.], [58., 4.], TEXT, "left"),
                        ),
                    ],
                    banner,
                    [PAD, 58.],
                ),
                "#bw_lib_banner_visible",
            ),
        ));
        let card = [214., 50.];
        let small = [52., 16.];
        for row in 0..LIBRARY_ROWS {
            let offset = [PAD + (row % 2) as f64 * 222., 82. + (row / 2) as f64 * 56.];
            let state = |name: &str| format!("#bw_lib{row}_{name}");
            let status = |color, flag: &str| {
                visible(
                    caption(&format!("#entryStatus{row}"), small, color),
                    &state(flag),
                )
            };
            let items = vec![
                named("box", framed(card, [0., 0.])),
                named(
                    "ring",
                    visible(
                        outlined(card, [0., 0.], [0., 0., 0., 0.], ACCENT),
                        &state("loaded"),
                    ),
                ),
                named(
                    "name",
                    text(
                        &format!("#bw_lib_name{row}"),
                        [150., 10.],
                        [8., 5.],
                        TEXT,
                        "left",
                    ),
                ),
                named(
                    "loaded",
                    visible(
                        text("Loaded", [50., 10.], [156., 5.], GREEN, "right"),
                        &state("loaded"),
                    ),
                ),
                named(
                    "meta",
                    text(&state("meta"), [198., 10.], [8., 18.], MUTED, "left"),
                ),
                named(
                    "load",
                    button(
                        controls,
                        &format!("toggle-{row}"),
                        small,
                        [8., 30.],
                        &PLAIN,
                        vec![status(TEXT, "active_off"), status(GREEN, "active_on")],
                    ),
                ),
                named(
                    "delete",
                    button(
                        controls,
                        &format!("delete-{row}"),
                        small,
                        [64., 30.],
                        &DELETE,
                        vec![caption(&state("delete"), small, TEXT)],
                    ),
                ),
            ];
            nodes.push(named(
                format!("entry{row}"),
                visible(panel(items, card, offset), &format!("#visible{row}")),
            ));
            nodes.push(named(
                format!("slot{row}"),
                visible(
                    panel(
                        vec![
                            named("box", outlined(card, [0., 0.], [0., 0., 0., 0.], BORDER)),
                            named("empty", caption("Empty slot", card, MUTED)),
                        ],
                        card,
                        offset,
                    ),
                    &state("empty"),
                ),
            ));
        }
        nodes.push(footer("Loading closes this window"));
        nodes
    }

    /// One settings row: its caption and a pill switch, all one click target. The green fill
    /// fades and the knob slides with the host's eased toggle progress. An empty `caption`
    /// leaves the title to the surrounding group. Returns the row and its control index,
    /// whose hover shows the row's hint.
    fn switch_row(
        controls: &mut Controls,
        key: &str,
        title: &str,
        caption_text: &str,
        offset: [f64; 2],
        width: f64,
    ) -> (Value, usize) {
        let index = controls.toggle(&format!("{SETTING_PREFIX}{key}"), title);
        let row = |field: &str| format!("#row_{index}_{field}");
        let size = [width, 22.];
        let track = rect(SWITCH, [0., 0.], SWITCH_OFF, SWITCH[1] / 2.);
        let fill = bind(
            rect(SWITCH, [0., 0.], ACCENT, SWITCH[1] / 2.),
            &row("toggle_progress"),
            "#alpha",
        );
        let knob = bind(
            rect([12., 12.], [2., 2.], TEXT, 6.),
            &row("knob_offset"),
            "#offset",
        );
        let items = vec![
            named(
                "hit",
                hit(index, "control", size, [0., 0.], wash(index, size)),
            ),
            named(
                "title",
                text(caption_text, [width - 40., 10.], [4., 6.], TEXT, "left"),
            ),
            named(
                "switch",
                panel(
                    vec![
                        named("track", track),
                        named("fill", fill),
                        named("knob", knob),
                    ],
                    SWITCH,
                    [width - SWITCH[0] - 2., 3.],
                ),
            ),
        ];
        (panel(items, size, offset), index)
    }

    /// A settings row whose button cycles a value: the title, then the bound caption.
    fn cycle_row(
        controls: &mut Controls,
        id: &str,
        title: &str,
        caption_binding: &str,
        offset: [f64; 2],
        width: f64,
    ) -> (Value, usize) {
        let size = [84., 18.];
        let index = controls.0.len();
        let items = vec![
            named(
                "title",
                text(title, [width - 90., 10.], [4., 6.], TEXT, "left"),
            ),
            named(
                "button",
                button(
                    controls,
                    id,
                    size,
                    [width - size[0] - 2., 2.],
                    &PLAIN,
                    vec![caption(caption_binding, size, TEXT)],
                ),
            ),
        ];
        (panel(items, [width, 22.], offset), index)
    }

    /// A group box with its title in the top left.
    fn group(name: &str, title: &str, size: [f64; 2], offset: [f64; 2]) -> [Value; 2] {
        [
            named(format!("{name}_box"), framed(size, offset)),
            named(
                format!("{name}_title"),
                text(
                    title,
                    [120., 10.],
                    [offset[0] + 8., offset[1] + 6.],
                    MUTED,
                    "left",
                ),
            ),
        ]
    }

    /// Settings: Fast Place, Display and Behavior groups, and a box explaining the hovered
    /// setting.
    fn settings(controls: &mut Controls) -> Vec<Value> {
        let mut nodes = heading("bw_set", 300.);
        let width = 198.;
        // Each row and the control index whose hover shows its hint.
        let mut rows: Vec<(String, Value, usize)> = Vec::new();
        let switch = |controls: &mut Controls, key: &str, offset| {
            let title = SETTINGS
                .iter()
                .find(|(setting, _)| *setting == key)
                .map_or(key, |(_, title)| title);
            // The Fast Place group's title names its master switch.
            let caption_text = if key == "fast-place" { "" } else { title };
            let (row, index) = switch_row(controls, key, title, caption_text, offset, width);
            (key.to_owned(), row, index)
        };

        nodes.extend(group("fast", "Fast Place", [214., 172.], [PAD, 60.]));
        rows.push(switch(controls, "fast-place", [20., 62.]));
        let (row, index) = cycle_row(
            controls,
            MODE_ID,
            "Fast Place mode",
            "#bw_fp_mode",
            [20., 86.],
            width,
        );
        rows.push((MODE_ID.into(), row, index));
        for (line, key) in [
            "hold-place",
            "placement-lock",
            "correct-aim",
            "bridge-place",
        ]
        .into_iter()
        .enumerate()
        {
            rows.push(switch(controls, key, [20., 114. + 28. * line as f64]));
        }

        nodes.extend(group("display", "Display", [214., 128.], [234., 60.]));
        for (line, key) in ["look", "assist"].into_iter().enumerate() {
            rows.push(switch(controls, key, [242., 78. + 26. * line as f64]));
        }
        for (line, (id, title, binding)) in [("lightness", "Ghost lightness", "#bw_lightness")]
            .into_iter()
            .enumerate()
        {
            let (row, index) = cycle_row(
                controls,
                id,
                title,
                binding,
                [242., 130. + 26. * line as f64],
                width,
            );
            rows.push((id.into(), row, index));
        }

        nodes.extend(group("behavior", "Behavior", [214., 110.], [234., 194.]));
        for (line, key) in ["remember-placement", "auto-layer", "paste-destroy"]
            .into_iter()
            .enumerate()
        {
            rows.push(switch(controls, key, [242., 214. + 28. * line as f64]));
        }

        let hint_box = [214., 66.];
        let hint_offset = [PAD, 238.];
        nodes.push(named("hint_box", framed(hint_box, hint_offset)));
        nodes.push(named(
            "hint_idle",
            text(
                "Hover a setting to see what it does",
                [198., 10.],
                [20., 266.],
                MUTED,
                "center",
            ),
        ));
        for (key, row, index) in rows {
            let hint = HINTS
                .iter()
                .find(|(hint_key, _)| *hint_key == key)
                .map_or("", |(_, hint)| hint);
            let hover = format!("#row_{index}_hover");
            nodes.push(named(format!("set_{key}"), row));
            nodes.push(named(
                format!("hint_{key}"),
                panel(
                    vec![
                        named(
                            "cover",
                            bind(rect([212., 64.], [1., 1.], PANEL, 0.), &hover, "#alpha"),
                        ),
                        named(
                            "text",
                            bind(
                                text(hint, [198., 56.], [8., 6.], TEXT, "left"),
                                &hover,
                                "#alpha",
                            ),
                        ),
                    ],
                    hint_box,
                    hint_offset,
                ),
            ));
        }
        nodes.push(footer("Changes save immediately"));
        nodes
    }

    /// Controls: Position, Rotate & Mirror and Guide groups, and Paste with its confirmation.
    fn keys(controls: &mut Controls) -> Vec<Value> {
        let mut nodes = heading("bw_keys", WIDTH);
        let wide = [130., 20.];
        let half = [63., 20.];
        let left = |x: f64| x + 5.;
        let right = |x: f64| x + 72.;
        let columns = [
            ("position", "Position", 12.),
            ("rotate", "Rotate & Mirror", 160.),
            ("guide", "Guide", 308.),
        ];
        for (name, title, x) in columns {
            nodes.extend(group(name, title, [140., 196.], [x, 70.]));
        }
        let [(_, _, a), (_, _, b), (_, _, c)] = columns;
        let buttons = [
            ("place", "Place here", wide, [left(a), 90.]),
            ("ccw", "Rotate left", half, [left(b), 90.]),
            ("cw", "Rotate right", half, [right(b), 90.]),
            ("mirror", "Mirror", wide, [left(b), 114.]),
            ("previous-layer", "Prev", half, [left(b), 154.]),
            ("next-layer", "Next", half, [right(b), 154.]),
            ("mode", "#bw_layer_mode", wide, [left(b), 178.]),
            ("visibility", "#bw_visibility", wide, [left(c), 142.]),
        ];
        for (id, title, size, offset) in buttons {
            nodes.push(named(id, plain(controls, id, title, size, offset)));
        }
        nodes.push(named(
            "layers",
            text("Layers", [120., 10.], [left(b) + 3., 140.], MUTED, "left"),
        ));
        let lock = |title, color, flag: &str| visible(caption(title, wide, color), flag);
        nodes.push(named(
            "lock",
            button(
                controls,
                "lock",
                wide,
                [left(c), 90.],
                &PLAIN,
                vec![
                    lock("Lock guide: OFF", OFF, "#bw_lock_off"),
                    lock("Lock guide: ON", GREEN, "#bw_lock_on"),
                ],
            ),
        ));
        nodes.push(named(
            "lock_hint",
            text(
                "Stops moves and rotation. Not Placement lock.",
                [126., 24.],
                [left(c) + 2., 114.],
                MUTED,
                "left",
            ),
        ));
        nodes.push(named("paste_box", framed([WIDTH, 30.], [PAD, 272.])));
        nodes.push(named(
            "paste",
            plain(controls, "paste", "#bw_paste", [90., 20.], [20., 277.]),
        ));
        nodes.push(named(
            "export",
            plain(
                controls,
                "bw-export",
                "#bw_export",
                [95., 20.],
                [118., 277.],
            ),
        ));
        nodes.push(named(
            "paste_hint",
            text("#bw_paste_hint", [220., 10.], [221., 282.], MUTED, "left"),
        ));
        nodes.push(footer("Numpad 8/4/6/2 move  ·  7/9 down/up  ·  1/3 rotate"));
        nodes
    }

    /// The JSON-UI document and its routed controls.
    fn document() -> (String, Vec<Value>) {
        let mut controls = Controls::default();
        let mut card = vec![named(
            "background",
            outlined(WINDOW, [0., 0.], CARD, BORDER),
        )];
        card.extend(strip(&mut controls));
        card.push(named("materials", body(0, materials(&mut controls))));
        card.push(named("import", body(1, import(&mut controls))));
        card.push(named("settings", body(2, settings(&mut controls))));
        card.push(named("controls", body(3, keys(&mut controls))));
        let document = json!({
            "namespace": "browser",
            "root": {"type":"screen","size":["100%","100%"],"controls":[
                {"backdrop": {"type":"custom","renderer":"cinnabar_rounded_rectangle",
                    "size":["100%","100%"],"color":round(&BG),"radius":0}},
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

/// Counts for the current Materials page, accumulated across callbacks.
#[derive(Default)]
pub(super) struct MaterialPageCounts {
    materials: [Option<usize>; ROWS],
    palette_identity: (usize, usize),
    palette_cursor: usize,
    palette_indices: [Vec<usize>; ROWS],
    /// Blocks already in the world as planned, per row; what is left is the total less these.
    built: [u64; ROWS],
    cursor: usize,
    /// Finished counts by material, kept while the palette and the placed totals are unchanged,
    /// so a page seen before (or one the search brings back) shows at once instead of
    /// counting every tile again.
    known: std::collections::BTreeMap<usize, u64>,
    known_key: (usize, usize, u64),
}

/// Spare fuel in this callback. Native tests have no budget to read and are never limited.
fn spare_fuel() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        crate::budget::remaining_fuel()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        u64::MAX
    }
}

/// Fuel the Materials page leaves for the rest of the frame, and the most it counts per call.
const COUNT_RESERVE: u64 = 1_100_000;
const PALETTE_PER_CALL: usize = 512;
const TILES_PER_CALL: usize = 32;

impl Viewer {
    pub(super) fn material_counts_pending(&self) -> bool {
        self.browser_tab == Tab::Materials
            && (self.material_names_pending()
                || (self.browser_counts.borrow().cursor < self.tiles.len()
                    && self.material_scan_available()))
    }

    fn material_names_pending(&self) -> bool {
        !self.materials.is_empty()
            && self
                .browser_names
                .borrow()
                .as_ref()
                .is_none_or(|(signature, rows)| {
                    signature.1 != self.materials.as_ptr() as usize
                        || rows.len() < self.materials.len()
                })
    }

    fn material_scan_available(&self) -> bool {
        !self.dirty && self.tiles.iter().any(|tile| tile.stamp.is_some())
    }

    /// Material indices in alphabetical order, keeping only names containing the search.
    /// The sorted names are built once per material list, since schematics with many
    /// different blocks made rebuilding them on every scroll exhaust the frame's fuel.
    fn browser_materials(&self) -> Vec<(usize, std::rc::Rc<str>)> {
        let search = self.browser_search.trim().to_lowercase();
        let signature = (self.materials.len(), self.materials.as_ptr() as usize);
        let mut cache = self.browser_names.borrow_mut();
        let (cached_signature, names) = cache.get_or_insert_with(|| (signature, Vec::new()));
        if *cached_signature != signature {
            *cached_signature = signature;
            names.clear();
        }
        let start = names.len();
        let end = (start + 64).min(self.materials.len());
        for index in start..end {
            let name = std::rc::Rc::<str>::from(display_name(&self.materials[index].name));
            let offset = names.partition_point(|line| line.1 <= name);
            let lower = name.to_lowercase();
            names.insert(offset, (index, name, lower));
        }
        if end < self.materials.len() {
            return Vec::new();
        }
        cache.as_ref().map_or_else(Vec::new, |(_, names)| {
            names
                .iter()
                .filter(|(_, _, lower)| search.is_empty() || lower.contains(&search))
                .map(|(index, name, _)| (*index, name.clone()))
                .collect()
        })
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

    /// Handles one click in the window: a two-click Delete, a tab switch, a settings switch
    /// carrying its new state, the Fast Place mode button, or another menu action.
    pub(super) fn browser_event(&mut self, id: &str, value: f32) {
        // Delete takes a second click on the same row; any other click cancels it.
        let confirming = self.delete_confirm.take();
        if let Some(row) = id
            .strip_prefix("delete-")
            .and_then(|row| row.parse::<usize>().ok())
        {
            if confirming == Some(row) {
                self.menu_action(id);
            } else {
                self.delete_confirm = Some(row);
            }
        } else if let Some(tab) = Tab::ALL.into_iter().find(|tab| tab.id() == id) {
            self.browser_tab = tab;
        } else if let Some(key) = id.strip_prefix(SETTING_PREFIX) {
            self.browser_setting(key, value >= 0.5);
        } else if id == "bw-export" {
            self.export_button();
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
            "placement-lock" => options.placement_lock,
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
    fn browser_needed(
        &self,
        materials: [Option<usize>; ROWS],
        scanned: bool,
    ) -> [Option<u64>; ROWS] {
        if !scanned {
            return materials.map(|index| index.map(|index| self.materials[index].total));
        }
        let identity = self.preview.palette_identity();
        let built_key = self.tiles.iter().map(|tile| tile.built).sum::<u64>();
        let key = (identity.0, identity.1, built_key);
        let mut counts = self.browser_counts.borrow_mut();
        if counts.known_key != key {
            counts.known.clear();
            counts.known_key = key;
            counts.materials = [None; ROWS];
            counts.palette_cursor = 0;
            counts.palette_indices = Default::default();
            counts.built = [0; ROWS];
            counts.cursor = 0;
        }
        // Every row on this page was counted before: show it without walking the tiles.
        if materials
            .iter()
            .all(|line| line.is_none_or(|index| counts.known.contains_key(&index)))
        {
            return materials.map(|line| line.and_then(|index| counts.known.get(&index).copied()));
        }
        if counts.materials != materials || counts.palette_identity != identity {
            let known = std::mem::take(&mut counts.known);
            *counts = MaterialPageCounts {
                materials,
                palette_identity: identity,
                known,
                known_key: key,
                ..Default::default()
            };
        }
        let names = materials.map(|line| line.map(|index| self.materials[index].name.as_str()));
        let start = counts.palette_cursor;
        let end = (start + PALETTE_PER_CALL).min(identity.1);
        for (offset, name) in self.preview.palette_names(start, end).enumerate() {
            for (row, expected) in names.iter().enumerate() {
                if *expected == Some(name) {
                    counts.palette_indices[row].push(start + offset);
                }
            }
        }
        counts.palette_cursor = end;
        if end < identity.1 {
            return [None; ROWS];
        }
        // Count as many tiles as this frame can spare, so the window stays responsive while a
        // large schematic's visible rows are counted.
        let mut counted = 0;
        while counts.cursor < self.tiles.len()
            && counted < TILES_PER_CALL
            && (counted == 0 || spare_fuel() >= COUNT_RESERVE)
        {
            let tile = &self.tiles[counts.cursor];
            for (row, material) in materials.iter().enumerate() {
                if material.is_some() {
                    let add = u64::from(tile.material_counts(&counts.palette_indices[row])[1]);
                    counts.built[row] += add;
                }
            }
            counts.cursor += 1;
            counted += 1;
        }
        if counts.cursor == self.tiles.len() {
            // What is left is everything not yet in the world as planned. Counting only the
            // cells known to be missing showed 0 for blocks in unloaded or unchecked parts
            // of the guide.
            let left: [Option<u64>; ROWS] = std::array::from_fn(|row| {
                materials[row].map(|index| self.materials[index].total.saturating_sub(counts.built[row]))
            });
            for (row, line) in materials.iter().enumerate() {
                if let (Some(index), Some(value)) = (line, left[row]) {
                    counts.known.insert(*index, value);
                }
            }
            left
        } else {
            [None; ROWS]
        }
    }
}

/// How many frames pass between reads of the player's inventory for the Have column.
const INVENTORY_EVERY: u32 = 20;

/// What the player carries, by item identifier, read every few frames while the list is open.
#[derive(Default)]
pub struct InventoryCache {
    calls: u32,
    counts: std::collections::HashMap<String, u32>,
}

impl InventoryCache {
    /// Fills the cache without the host, and keeps it from being read again.
    #[cfg(test)]
    pub(crate) fn set_for_test(&mut self, counts: &[(&str, u32)]) {
        self.counts = counts.iter().map(|(name, count)| ((*name).to_owned(), *count)).collect();
        self.calls = 1;
    }
}

/// The item that holds a block: the same name, except for the few blocks whose item is called
/// something else (lit and powered variants, crops, signs).
pub(crate) fn item_for_block(block: &str) -> String {
    let name = block.strip_prefix("minecraft:").unwrap_or(block);
    let item = match name {
        "reeds" => "sugar_cane",
        "redstone_wire" => "redstone",
        "unpowered_repeater" | "powered_repeater" => "repeater",
        "unpowered_comparator" | "powered_comparator" => "comparator",
        "lit_furnace" => "furnace",
        "lit_blast_furnace" => "blast_furnace",
        "lit_smoker" => "smoker",
        "lit_redstone_lamp" => "redstone_lamp",
        "lit_redstone_ore" => "redstone_ore",
        "unlit_redstone_torch" => "redstone_torch",
        "standing_sign" | "wall_sign" => "oak_sign",
        other => {
            let sign = other
                .strip_suffix("_standing_sign")
                .or_else(|| other.strip_suffix("_wall_sign"));
            return match sign {
                Some(wood) => format!("minecraft:{wood}_sign"),
                None => format!("minecraft:{other}"),
            };
        }
    };
    format!("minecraft:{item}")
}

#[cfg(target_arch = "wasm32")]
fn read_inventory() -> Option<std::collections::HashMap<String, u32>> {
    let equipment = crate::player_state::read_equipment().ok().flatten()?;
    let mut counts = std::collections::HashMap::new();
    for slot in equipment.inventory.iter().chain(std::iter::once(&equipment.offhand)) {
        if let Some(item) = &slot.item
            && let Some(identifier) = &item.identifier
        {
            *counts.entry(identifier.clone()).or_default() += u32::from(item.count);
        }
    }
    Some(counts)
}

#[cfg(not(target_arch = "wasm32"))]
fn read_inventory() -> Option<std::collections::HashMap<String, u32>> {
    None
}

impl Viewer {
    /// How many of `block`'s item the player carries, refreshed every few frames.
    fn carried(&self, block: &str) -> u32 {
        let mut cache = self.inventory.borrow_mut();
        if cache.calls % INVENTORY_EVERY == 0
            && let Some(counts) = read_inventory()
        {
            cache.counts = counts;
        }
        cache.calls = cache.calls.wrapping_add(1);
        cache.counts.get(&item_for_block(block)).copied().unwrap_or(0)
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
    out.insert("#bw_tab".into(), json!(viewer.browser_tab.index()));
    out.insert(
        NATIVE_BINDING.into(),
        json!(NATIVE_TABS.contains(&viewer.browser_tab)),
    );
    match viewer.browser_tab {
        Tab::Materials => materials(viewer, out),
        Tab::Import => import(viewer, out),
        Tab::Settings => settings(viewer, out, controls),
        Tab::Controls => keys(viewer, out),
    }
}

/// Cuts `text` to `limit` characters, ending in `...` when it was longer.
fn shorten(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut short: String = text.chars().take(limit.saturating_sub(3)).collect();
    short.push_str("...");
    short
}

fn materials(viewer: &Viewer, out: &mut Map<String, Value>) {
    let scanned = viewer.material_scan_available();
    let total: u64 = viewer.materials.iter().map(|line| line.total).sum();
    // Per-material counts walk every tile, so only the rows in view compute them; the
    // totals read each tile's built count. What is left is the total less what is built, so
    // blocks in unloaded or unchecked parts of the guide count as left, not as done.
    let needed: u64 = if scanned {
        total.saturating_sub(viewer.tiles.iter().map(|tile| tile.built).sum())
    } else {
        total
    };
    let title = if viewer.loaded {
        shorten(&viewer.schematic.name, 40)
    } else {
        "No schematic loaded".into()
    };
    let summary = if viewer.materials.is_empty() {
        "Load a schematic in Import".into()
    } else {
        format!("{} materials", viewer.materials.len())
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
            "{}% built  ·  {placed} / {total}",
            (progress * 100.).floor()
        )
    } else if viewer.placement.is_some() && viewer.shown {
        "Checking the guide…".into()
    } else {
        "Place it to track progress".into()
    };
    out.insert("#bw_title".into(), json!(title));
    out.insert("#bw_summary".into(), json!(summary));
    out.insert("#bw_progress".into(), json!(progress));
    out.insert("#bw_progress_label".into(), json!(progress_label));
    let searching = !viewer.browser_search.is_empty();
    out.insert("#bw_search_empty".into(), json!(!searching));
    out.insert("#bw_search_full".into(), json!(searching));
    // The native window's search field shows the search it continues from.
    out.insert("#bw_search_text".into(), json!(viewer.browser_search));
    let lines = viewer.browser_materials();
    let pages = lines.len().div_ceil(ROWS).max(1);
    let page = viewer.material_page.min(pages - 1);
    let first = page * ROWS;
    let preparing = viewer.material_names_pending();
    let empty = !viewer.materials.is_empty() && lines.is_empty();
    out.insert("#bw_empty_visible".into(), json!(empty));
    out.insert(
        "#bw_empty".into(),
        json!(if preparing {
            "Preparing materials…"
        } else {
            "No matching materials"
        }),
    );
    out.insert(
        "#bw_page".into(),
        json!(if lines.is_empty() {
            String::new()
        } else {
            format!("Page {} / {pages}", page + 1)
        }),
    );
    let needed = viewer.browser_needed(
        std::array::from_fn(|row| lines.get(first + row).map(|(index, _)| *index)),
        scanned,
    );
    for (row, needed) in needed.into_iter().enumerate() {
        let entry = lines.get(first + row);
        let line = entry.map(|(index, _)| &viewer.materials[*index]);
        out.insert(format!("#bw_row{row}_visible"), json!(line.is_some()));
        out.insert(
            format!("#bw_row{row}_icon"),
            crate::menu::material_icon(line),
        );
        out.insert(
            format!("#bw_row{row}_name"),
            json!(entry.map_or(String::new(), |(_, name)| shorten(name, 22))),
        );
        out.insert(
            format!("#bw_row{row}_needed"),
            json!(needed.map_or_else(
                || if line.is_some() { "…" } else { "" }.into(),
                |needed| if needed == 0 { "Done".into() } else { needed.to_string() }
            )),
        );
        out.insert(
            format!("#bw_row{row}_need"),
            json!(needed.is_some_and(|n| n > 0)),
        );
        out.insert(format!("#bw_row{row}_done"), json!(needed == Some(0)));
        out.insert(
            format!("#bw_row{row}_have"),
            json!(line.map_or(String::new(), |line| viewer.carried(&line.name).to_string())),
        );
        out.insert(
            format!("#bw_row{row}_total"),
            json!(line.map_or(String::new(), |line| line.total.to_string())),
        );
    }
}

fn import(viewer: &Viewer, out: &mut Map<String, Value>) {
    let mut count = 0;
    let mut banner = None;
    for row in 0..LIBRARY_ROWS {
        let index = viewer.visible_entry(row);
        count += usize::from(index.is_some());
        let loaded = viewer.loaded && index.is_some() && viewer.active_entry == index;
        let active = loaded && viewer.shown && viewer.placement.is_some();
        let name = index
            .and_then(|index| viewer.library.get(index))
            .map_or("", String::as_str);
        if loaded {
            banner = Some((shorten(name, 28), active));
        }
        out.insert(format!("#visible{row}"), json!(index.is_some()));
        out.insert(format!("#bw_lib{row}_empty"), json!(index.is_none()));
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
        out.insert(format!("#bw_lib{row}_active_on"), json!(active));
        out.insert(format!("#bw_lib{row}_active_off"), json!(!active));
        out.insert(
            format!("#bw_lib{row}_delete"),
            json!(if viewer.delete_confirm == Some(row) {
                "Confirm?"
            } else {
                "Delete"
            }),
        );
        out.insert(format!("#bw_lib_name{row}"), json!(shorten(name, 18)));
        // Size and volume come from the header; only the loaded entry has counted blocks.
        let meta = index
            .and_then(|index| viewer.library_sizes.get(index))
            .map_or(String::new(), |[x, y, z]| {
                let mut meta = format!("{x}x{y}x{z}  ·  vol {}", x * y * z);
                if loaded {
                    let blocks: u64 = viewer.materials.iter().map(|line| line.total).sum();
                    meta = format!("{x}x{y}x{z}  ·  {blocks} blocks  ·  vol {}", x * y * z);
                }
                meta
            });
        out.insert(format!("#bw_lib{row}_meta"), json!(meta));
    }
    out.insert("#bw_lib_title".into(), json!("Library"));
    out.insert(
        "#bw_lib_summary".into(),
        json!(format!("{count} / {LIBRARY_ROWS} slots used")),
    );
    out.insert("#bw_lib_banner_visible".into(), json!(banner.is_some()));
    out.insert(
        "#bw_lib_banner".into(),
        json!(banner.map_or(String::new(), |(name, active)| if active {
            format!("{name} is loaded and visible")
        } else {
            format!("{name} is loaded and hidden")
        })),
    );
}

fn keys(viewer: &Viewer, out: &mut Map<String, Value>) {
    out.insert(
        "#bw_keys_title".into(),
        json!(if viewer.loaded {
            shorten(&viewer.schematic.name, 40)
        } else {
            "Controls".into()
        }),
    );
    out.insert(
        "#bw_keys_summary".into(),
        json!(viewer.placement.map_or_else(
            || "Load a schematic in Import".into(),
            |p| format!(
                "Origin {}, {}, {}  ·  Rot {}  ·  Mirror {}  ·  {}",
                p.origin[0],
                p.origin[1],
                p.origin[2],
                u16::from(p.quarter_turns) * 90,
                format!("{:?}", p.mirror).to_lowercase(),
                viewer.layers_text()
            ),
        )),
    );
    out.insert("#bw_lock_on".into(), json!(viewer.locked));
    out.insert("#bw_lock_off".into(), json!(!viewer.locked));
    out.insert(
        "#bw_visibility".into(),
        json!(if viewer.shown {
            "Hide guide"
        } else {
            "Show guide"
        }),
    );
    out.insert(
        "#bw_layer_mode".into(),
        json!(["All layers", "Single layer", "All below"][usize::from(viewer.layer_mode) % 3]),
    );
    out.insert(
        "#bw_paste".into(),
        json!(if viewer.paste.is_some() {
            "Cancel Paste"
        } else if viewer.paste_confirm && !viewer.export_confirming {
            "Confirm paste"
        } else {
            "Paste"
        }),
    );
    out.insert(
        "#bw_export".into(),
        json!(if viewer.export_run.is_some() {
            "Cancel Export"
        } else if viewer.paste_confirm && viewer.export_confirming {
            "Confirm export"
        } else {
            "Export"
        }),
    );
    let overlaps = if viewer.options.paste_destroy {
        "ON"
    } else {
        "OFF"
    };
    out.insert(
        "#bw_paste_hint".into(),
        json!(if viewer.paste.is_some() {
            "Stops the remaining paste".to_string()
        } else if let Some(run) = &viewer.export_run {
            format!("Exporting in the background  ·  {}", run.progress)
        } else if viewer.paste_confirm && viewer.export_confirming {
            "Click Export again to save a structure block structure".to_string()
        } else if viewer.paste_confirm {
            format!("Click again to confirm  ·  Replace wrong blocks: {overlaps}")
        } else {
            format!("Replace wrong blocks: {overlaps}")
        }),
    );
}

fn settings(viewer: &Viewer, out: &mut Map<String, Value>, controls: &mut Vec<Value>) {
    out.insert("#bw_set_title".into(), json!("Settings"));
    out.insert("#bw_set_summary".into(), json!("Saves immediately"));
    out.insert(
        "#bw_fp_mode".into(),
        json!(if viewer.options.strict_place {
            "V2 Strict"
        } else {
            "V1 Classic"
        }),
    );
    out.insert(
        "#bw_lightness".into(),
        json!(format!("{}%", viewer.options.lightness)),
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
        assert_eq!(out["#bw_row0_name"], &*names[0]);
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

    /// The ids of every routed control in the precomputed layout.
    fn routed_ids() -> Vec<String> {
        let panel: Value = serde_json::from_str(LAYOUT).unwrap();
        panel["controls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|control| control["id"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn controls_tab_reads_out_the_placement_and_routes_its_actions() {
        let mut viewer = crate::tests::reference_viewer();
        viewer.browser_event(Tab::Controls.id(), 1.);
        viewer.placement = Some(schematica_core::Placement {
            origin: [-1042, 64, 288],
            quarter_turns: 1,
            mirror: schematica_core::Mirror::None,
        });
        let mut out = Map::new();
        bindings(&viewer, &mut out, &mut Vec::new());
        assert_eq!(out["#bw_body3"], true);
        let readout = out["#bw_keys_summary"].as_str().unwrap();
        assert!(
            readout.starts_with("Origin -1042, 64, 288  ·  Rot 90  ·  Mirror none"),
            "{readout}"
        );
        assert_eq!(out["#bw_lock_off"], !viewer.locked);
        assert_eq!(out["#bw_paste_hint"], "Replace wrong blocks: OFF");
        let ids = routed_ids();
        for id in [
            "place",
            "ccw",
            "cw",
            "mirror",
            "mode",
            "next-layer",
            "lock",
            "visibility",
            "paste",
        ] {
            assert!(ids.iter().any(|routed| routed == id), "{id}");
        }
    }

    #[test]
    fn needed_counts_turn_green_at_zero() {
        let mut viewer = crate::tests::reference_viewer();
        let first = viewer.browser_materials()[0].0;
        viewer.materials[first].total = 0;
        let mut out = Map::new();
        bindings(&viewer, &mut out, &mut Vec::new());
        assert_eq!(out["#bw_row0_done"], true);
        assert_eq!(out["#bw_row0_need"], false);
        assert_eq!(out["#bw_row1_need"], true);
        assert_eq!(out["#bw_page"], "Page 1 / 1");
    }

    #[test]
    fn library_cards_show_size_loaded_state_and_banner() {
        let mut viewer = crate::tests::reference_viewer();
        viewer.browser_event(Tab::Import.id(), 1.);
        viewer.library_sizes = vec![[4, 5, 6], [2, 2, 2], [1, 1, 1]];
        viewer.active_entry = Some(0);
        let mut out = Map::new();
        bindings(&viewer, &mut out, &mut Vec::new());
        assert_eq!(out["#bw_lib_summary"], "3 / 8 slots used");
        assert_eq!(out["#bw_lib0_loaded"], true);
        assert_eq!(out["#bw_lib_banner_visible"], true);
        let blocks: u64 = viewer.materials.iter().map(|line| line.total).sum();
        assert_eq!(
            out["#bw_lib0_meta"],
            format!("4x5x6  ·  {blocks} blocks  ·  vol 120")
        );
        assert_eq!(out["#bw_lib1_meta"], "2x2x2  ·  vol 8");
        assert_eq!(out["#bw_lib3_empty"], true);
    }

    #[test]
    fn delete_needs_a_second_click_on_the_same_row() {
        let mut viewer = crate::tests::reference_viewer();
        viewer.browser_event(Tab::Import.id(), 1.);
        viewer.browser_event("delete-1", 1.);
        assert!(viewer.options.deleted_entries.is_empty());
        let mut out = Map::new();
        bindings(&viewer, &mut out, &mut Vec::new());
        assert_eq!(out["#bw_lib1_delete"], "Confirm?");

        // Another click cancels the pending delete.
        viewer.browser_event(Tab::Import.id(), 1.);
        viewer.browser_event("delete-1", 1.);
        viewer.browser_event("delete-2", 1.);
        assert!(viewer.options.deleted_entries.is_empty());

        viewer.browser_event("delete-2", 1.);
        assert_eq!(viewer.options.deleted_entries, vec![2]);
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
        assert_eq!(out["#bw_lightness"], "15%");
        for expected in ["25%", "35%", "5%", "10%", "15%"] {
            viewer.browser_event("lightness", 1.);
            let mut out = Map::new();
            bindings(&viewer, &mut out, &mut Vec::new());
            assert_eq!(out["#bw_lightness"], expected);
        }
        let ids = routed_ids();
        assert!(ids.iter().any(|routed| routed == "lightness"));
        assert!(!ids.iter().any(|routed| routed == "opacity"));
        for removed in ["ui-layout", "hotkeys", "x-minus", "verifier"] {
            assert!(!ids.iter().any(|routed| routed == removed), "{removed}");
        }
    }

    #[test]
    fn only_native_tabs_ask_for_the_native_window() {
        let mut viewer = crate::tests::reference_viewer();
        for tab in Tab::ALL {
            viewer.browser_event(tab.id(), 1.);
            let mut out = Map::new();
            bindings(&viewer, &mut out, &mut Vec::new());
            assert_eq!(out[NATIVE_BINDING], NATIVE_TABS.contains(&tab), "{tab:?}");
            assert_eq!(out["#bw_tab"], tab.index());
        }
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
