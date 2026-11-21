use super::*;
use ui::{DpiScale, SafeArea, TextLayoutCache};
use ui_engine::{test_support::fixture_font, text_metrics::TextMetrics};

/// The mod's Settings panel: every switch, with fast-place on and hold-place off.
fn controls() -> Vec<Control> {
    GROUPS
        .iter()
        .flat_map(|(_, rows)| rows.iter())
        .filter_map(|row| match row {
            Row::Switch(key) => Some(Control::Toggle {
                id: format!("{SETTING_PREFIX}{key}"),
                label: (*key).to_owned(),
                value: *key == "fast-place",
            }),
            Row::Cycle { .. } => None,
        })
        .collect()
}

fn paint(screen: &mut CinnamaticaScreen, tab: f64, size: [f32; 2]) {
    let mut bindings = BTreeMap::from([
        ("#bw_tab".to_owned(), SurfaceValue::Number(tab)),
        (
            "#bw_fp_mode".to_owned(),
            SurfaceValue::Text("V2 Strict".into()),
        ),
        ("#bw_lightness".to_owned(), SurfaceValue::Text("45%".into())),
        ("#bw_paste".to_owned(), SurfaceValue::Text("Paste".into())),
        (
            "#bw_lib_banner_visible".to_owned(),
            SurfaceValue::Bool(true),
        ),
    ]);
    // Two materials, the second already complete.
    for row in 0..2 {
        bindings.insert(format!("#bw_row{row}_visible"), SurfaceValue::Bool(true));
        bindings.insert(
            format!("#bw_row{row}_name"),
            SurfaceValue::Text("Stone".into()),
        );
        bindings.insert(format!("#bw_row{row}_done"), SurfaceValue::Bool(row == 1));
    }
    // Two library entries; the rest are empty slots.
    for row in 0..2 {
        bindings.insert(format!("#visible{row}"), SurfaceValue::Bool(true));
        bindings.insert(
            format!("#entryStatus{row}"),
            SurfaceValue::Text("Load".into()),
        );
        bindings.insert(
            format!("#bw_lib{row}_delete"),
            SurfaceValue::Text("Delete".into()),
        );
    }
    let controls = controls();
    let font = fixture_font();
    let mut layouts = TextLayoutCache::new(64, 1024 * 1024);
    let (mut nodes, mut next) = (Vec::new(), 1);
    let metrics = TextMetrics::for_viewport(
        size.map(|value| value as u32),
        DpiScale::new(1.0).unwrap(),
        Some(2),
    );
    screen
        .paint(
            CinnamaticaInput {
                bindings: &bindings,
                controls: &controls,
                icon: &|_, _| None,
            },
            &mut PaintFrame {
                layouts: &mut layouts,
                font: &font,
                metrics,
                solid_page: 0,
                safe_area: SafeArea::ZERO,
                content: size,
                translate: &|_| None,
                language: [0; 3],
                now: 0.0,
                nodes: &mut nodes,
                next: &mut next,
                renderers: &mut [],
            },
        )
        .unwrap();
}

fn target(screen: &CinnamaticaScreen, id: &str) -> Option<(f32, UiRect)> {
    screen
        .hits()
        .iter()
        .find_map(|(hit, bounds)| (hit.id == id).then_some((hit.value, *bounds)))
}

#[test]
fn settings_rows_send_the_mods_control_events() {
    let mut screen = CinnamaticaScreen::default();
    let size = [1280.0, 720.0];
    paint(&mut screen, SETTINGS_TAB as f64, size);
    for (_, rows) in GROUPS {
        for row in rows {
            let id = match row {
                Row::Switch(key) => format!("{SETTING_PREFIX}{key}"),
                Row::Cycle { id, .. } => (*id).to_owned(),
            };
            let (_, bounds) = target(&screen, &id).unwrap_or_else(|| panic!("{id} missing"));
            assert!(
                bounds.min().x() >= 0.0
                    && bounds.min().y() >= 0.0
                    && bounds.max().x() <= size[0]
                    && bounds.max().y() <= size[1],
                "{id} off screen"
            );
        }
    }
    // A switch click carries the state it flips to.
    assert_eq!(target(&screen, "bw-set-fast-place").unwrap().0, 0.0);
    assert_eq!(target(&screen, "bw-set-hold-place").unwrap().0, 1.0);
    for (id, _) in TABS {
        assert!(target(&screen, id).is_some(), "{id}");
    }
    let (_, lightness) = target(&screen, "lightness").unwrap();
    let centre = UiPoint::new(
        (lightness.min().x() + lightness.max().x()) * 0.5,
        (lightness.min().y() + lightness.max().y()) * 0.5,
    )
    .unwrap();
    assert_eq!(screen.hit(centre).unwrap().id, "lightness");
    for removed in ["hotkeys", "ui-layout"] {
        assert!(target(&screen, removed).is_none(), "{removed}");
    }
}

#[test]
fn small_windows_keep_every_target_inside() {
    let mut screen = CinnamaticaScreen::default();
    let size = ui::MIN_WINDOW_LOGICAL_SIZE;
    paint(&mut screen, SETTINGS_TAB as f64, size);
    for (hit, bounds) in screen.hits() {
        assert!(
            bounds.max().x() <= size[0] + 0.5 && bounds.max().y() <= size[1] + 0.5,
            "{} off screen",
            hit.id
        );
    }
}

/// Every target lies within `size`.
fn assert_inside(screen: &CinnamaticaScreen, size: [f32; 2]) {
    for (hit, bounds) in screen.hits() {
        assert!(
            bounds.min().x() >= 0.0
                && bounds.min().y() >= 0.0
                && bounds.max().x() <= size[0] + 0.5
                && bounds.max().y() <= size[1] + 0.5,
            "{} off screen",
            hit.id
        );
    }
}

#[test]
fn controls_tab_sends_every_placement_action() {
    for size in [[1280.0, 720.0], ui::MIN_WINDOW_LOGICAL_SIZE] {
        let mut screen = CinnamaticaScreen::default();
        paint(&mut screen, CONTROLS_TAB as f64, size);
        for id in [
            "place",
            "ccw",
            "cw",
            "mirror",
            "previous-layer",
            "next-layer",
            "mode",
            "lock",
            "visibility",
            "paste",
            "bw-export",
        ] {
            assert!(target(&screen, id).is_some(), "{id}");
        }
        for removed in ["x-minus", "z-plus", "verifier"] {
            assert!(target(&screen, removed).is_none(), "{removed}");
        }
        assert_inside(&screen, size);
    }
}

#[test]
fn import_tab_routes_import_and_only_filled_cards() {
    for size in [[1280.0, 720.0], ui::MIN_WINDOW_LOGICAL_SIZE] {
        let mut screen = CinnamaticaScreen::default();
        paint(&mut screen, IMPORT_TAB as f64, size);
        // The host opens the file picker only after a click on a control named `import`.
        assert!(target(&screen, "import").is_some());
        for row in 0..LIBRARY_ROWS {
            let filled = row < 2;
            assert_eq!(target(&screen, &format!("toggle-{row}")).is_some(), filled);
            assert_eq!(target(&screen, &format!("delete-{row}")).is_some(), filled);
        }
        assert_inside(&screen, size);
    }
}

#[test]
fn materials_tab_routes_the_search_field() {
    for size in [[1280.0, 720.0], ui::MIN_WINDOW_LOGICAL_SIZE] {
        let mut screen = CinnamaticaScreen::default();
        paint(&mut screen, MATERIALS_TAB as f64, size);
        assert!(target(&screen, SEARCH_ID).is_some());
        assert_inside(&screen, size);
    }
}

#[test]
fn search_typing_reports_each_change_and_ends_on_enter() {
    let mut screen = CinnamaticaScreen::default();
    assert_eq!(
        screen.search_key("KeyA", Some("a")),
        None,
        "unfocused field ignores keys"
    );
    screen.begin_search("sp");
    assert!(screen.searching());
    assert_eq!(screen.search_key("KeyR", Some("r")).as_deref(), Some("spr"));
    assert_eq!(screen.search_key("Backspace", None).as_deref(), Some("sp"));
    assert_eq!(screen.search_key("ShiftLeft", None), None);
    assert_eq!(screen.search_key("Enter", None), None);
    assert!(!screen.searching());
    screen.begin_search(&"x".repeat(40));
    assert_eq!(
        screen.search_key("KeyA", Some("a")),
        None,
        "the field is full"
    );
}
